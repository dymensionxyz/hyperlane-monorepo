use crate::ops::migration::MigrationFXG;
use crate::ops::payload::MessageIDs;
use crate::validator::error::ValidationError;
use crate::validator::withdraw::{escrow_input_selector, safe_bundle, sign_withdrawal_fxg};
use dym_kas_core::escrow::EscrowPublic;
use dym_kas_core::pskt::is_valid_sighash_type;
use eyre::Result;
use hyperlane_cosmos::native::ModuleQueryClient;
use kaspa_addresses::Address;
use kaspa_bip32::secp256k1::Keypair as SecpKeypair;
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionOutpoint};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_txscript::pay_to_address_script;
use kaspa_wallet_pskt::prelude::*;
use kaspa_wallet_pskt::pskt::{Signer, PSKT};
use std::collections::HashSet;
use tracing::info;

/// Validate and sign a migration PSKT.
///
/// Migration validation checks:
/// 1. Query hub for current anchor and verify PSKT spends it
/// 2. Query Kaspa for ALL escrow UTXOs and verify PSKT spends ALL of them
/// 3. Verify exactly ONE output goes to the configured migration target address
/// 4. Verify payload is empty MessageIDs (no withdrawals processed)
pub async fn validate_sign_migration_fxg<F, Fut, R>(
    fxg: MigrationFXG,
    escrow_public: EscrowPublic,
    migration_target_address: &Address,
    hub_rpc: &ModuleQueryClient,
    kaspa_rpc: &R,
    load_key: F,
) -> Result<Bundle>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<SecpKeypair>>,
    R: RpcApi + ?Sized,
{
    let bundle = safe_bundle(&fxg.bundle)
        .map_err(|e| eyre::eyre!("Safe bundle validation failed: {e:?}"))?;

    validate_migration_bundle(
        &bundle,
        &escrow_public,
        migration_target_address,
        hub_rpc,
        kaspa_rpc,
    )
    .await?;

    info!("Validator: migration PSKT is valid");

    // Sign escrow inputs using the same selector as withdrawals
    let signed = sign_withdrawal_fxg(
        &bundle,
        load_key,
        Some(escrow_input_selector(&escrow_public)),
    )
    .await
    .map_err(|e| eyre::eyre!("Failed to sign migration: {e}"))?;

    Ok(signed)
}

async fn validate_migration_bundle<R>(
    bundle: &Bundle,
    escrow_public: &EscrowPublic,
    migration_target_address: &Address,
    hub_rpc: &ModuleQueryClient,
    kaspa_rpc: &R,
) -> Result<(), ValidationError>
where
    R: RpcApi + ?Sized,
{
    // Migration must be a single PSKT - no chaining needed for migration
    if bundle.0.len() != 1 {
        return Err(ValidationError::FailedGeneralVerification {
            reason: format!(
                "Migration bundle must contain exactly 1 PSKT, got {}",
                bundle.0.len()
            ),
        });
    }

    // Query hub for current anchor
    let hub_anchor = query_hub_anchor(hub_rpc).await?;
    info!(
        tx_id = %hub_anchor.transaction_id,
        index = hub_anchor.index,
        "Migration: got hub anchor"
    );

    // Query Kaspa for ALL escrow UTXOs
    let escrow_utxos = query_escrow_utxos(kaspa_rpc, &escrow_public.addr).await?;
    info!(
        utxo_count = escrow_utxos.len(),
        "Migration: got escrow UTXOs"
    );

    if escrow_utxos.is_empty() {
        return Err(ValidationError::FailedGeneralVerification {
            reason: "No UTXOs found at escrow address".to_string(),
        });
    }

    let target_script = pay_to_address_script(migration_target_address);
    let pskt_inner =
        bundle
            .iter()
            .next()
            .ok_or_else(|| ValidationError::FailedGeneralVerification {
                reason: "Bundle was empty".to_string(),
            })?;
    let pskt = PSKT::<Signer>::from(pskt_inner.clone());

    validate_migration_pskt(&pskt, &hub_anchor, &escrow_utxos, &target_script)?;

    Ok(())
}

async fn query_hub_anchor(
    hub_rpc: &ModuleQueryClient,
) -> Result<TransactionOutpoint, ValidationError> {
    let resp = hub_rpc
        .outpoint(None)
        .await
        .map_err(|e| ValidationError::HubQueryError {
            reason: format!("Query hub outpoint: {}", e),
        })?;

    let outpoint = resp
        .outpoint
        .ok_or_else(|| ValidationError::HubQueryError {
            reason: "No outpoint in hub response".to_string(),
        })?;

    if outpoint.transaction_id.len() != 32 {
        return Err(ValidationError::HubQueryError {
            reason: format!(
                "Invalid hub anchor transaction ID length: expected 32, got {}",
                outpoint.transaction_id.len()
            ),
        });
    }

    let tx_id =
        kaspa_hashes::Hash::from_bytes(outpoint.transaction_id.as_slice().try_into().map_err(
            |_| ValidationError::HubQueryError {
                reason: "Failed to convert hub anchor tx ID".to_string(),
            },
        )?);

    Ok(TransactionOutpoint::new(tx_id, outpoint.index))
}

async fn query_escrow_utxos<R>(
    kaspa_rpc: &R,
    escrow_addr: &Address,
) -> Result<Vec<TransactionOutpoint>, ValidationError>
where
    R: RpcApi + ?Sized,
{
    let utxos = kaspa_rpc
        .get_utxos_by_addresses(vec![escrow_addr.clone()])
        .await
        .map_err(|e| ValidationError::FailedGeneralVerification {
            reason: format!("Query Kaspa escrow UTXOs: {}", e),
        })?;

    Ok(utxos
        .into_iter()
        .map(|u| TransactionOutpoint::from(u.outpoint))
        .collect())
}

fn validate_migration_pskt(
    pskt: &PSKT<Signer>,
    hub_anchor: &TransactionOutpoint,
    expected_utxos: &[TransactionOutpoint],
    target_script: &ScriptPublicKey,
) -> Result<(), ValidationError> {
    // Check sighash types
    if pskt
        .inputs
        .iter()
        .any(|input| !is_valid_sighash_type(input.sighash_type))
    {
        return Err(ValidationError::SigHashType);
    }

    // Verify hub anchor is spent
    let spends_hub_anchor = pskt
        .inputs
        .iter()
        .any(|i| &i.previous_outpoint == hub_anchor);

    if !spends_hub_anchor {
        return Err(ValidationError::AnchorNotFound { o: *hub_anchor });
    }

    // Build set of expected inputs: all escrow UTXOs + hub anchor
    let mut expected_inputs: HashSet<TransactionOutpoint> =
        expected_utxos.iter().cloned().collect();
    expected_inputs.insert(*hub_anchor);

    // Collect all input outpoints from the PSKT
    let pskt_input_outpoints: HashSet<TransactionOutpoint> =
        pskt.inputs.iter().map(|i| i.previous_outpoint).collect();

    // Verify PSKT inputs match exactly: all expected inputs and nothing extra
    // This prevents malicious PSKTs from including unexpected inputs
    if pskt_input_outpoints != expected_inputs {
        // Find missing inputs
        for expected in &expected_inputs {
            if !pskt_input_outpoints.contains(expected) {
                return Err(ValidationError::FailedGeneralVerification {
                    reason: format!(
                        "Migration PSKT missing expected input: {}:{}",
                        expected.transaction_id, expected.index
                    ),
                });
            }
        }
        // Find unexpected inputs
        for actual in &pskt_input_outpoints {
            if !expected_inputs.contains(actual) {
                return Err(ValidationError::FailedGeneralVerification {
                    reason: format!(
                        "Migration PSKT has unexpected input: {}:{}",
                        actual.transaction_id, actual.index
                    ),
                });
            }
        }
    }

    // Verify exactly ONE output to migration target (and no other outputs)
    if pskt.outputs.len() != 1 {
        return Err(ValidationError::FailedGeneralVerification {
            reason: format!(
                "Migration PSKT must have exactly 1 output, got {}",
                pskt.outputs.len()
            ),
        });
    }
    if &pskt.outputs[0].script_public_key != target_script {
        return Err(ValidationError::FailedGeneralVerification {
            reason: "Migration PSKT output must go to target address".to_string(),
        });
    }

    // Verify payload is empty MessageIDs (migration TX processes no withdrawals)
    let expected_payload = MessageIDs::new(vec![]).to_bytes();
    let actual_payload = pskt.global.payload.clone().unwrap_or_default();

    if actual_payload != expected_payload {
        return Err(ValidationError::FailedGeneralVerification {
            reason: "Migration PSKT payload must be empty MessageIDs".to_string(),
        });
    }

    // Calculate total input and output sums
    let total_inputs_sum: u64 = pskt
        .inputs
        .iter()
        .map(|i| i.utxo_entry.as_ref().map_or(0, |u| u.amount))
        .sum();
    let output_sum: u64 = pskt.outputs.iter().map(|o| o.amount).sum();

    // Verify output doesn't exceed inputs (would indicate invalid transaction)
    if output_sum > total_inputs_sum {
        return Err(ValidationError::FailedGeneralVerification {
            reason: format!(
                "Migration PSKT output ({}) exceeds inputs ({})",
                output_sum, total_inputs_sum
            ),
        });
    }

    info!(
        total_inputs_sum,
        output_sum,
        fee = total_inputs_sum - output_sum,
        "Migration PSKT validated"
    );

    Ok(())
}
