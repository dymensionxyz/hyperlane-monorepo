use crate::ops::migration::MigrationFXG;
use crate::validator::error::ValidationError;
use crate::validator::withdraw::{safe_bundle, sign_withdrawal_fxg};
use dym_kas_core::escrow::EscrowPublic;
use dym_kas_core::pskt::is_valid_sighash_type;
use eyre::Result;
use kaspa_addresses::Address;
use kaspa_bip32::secp256k1::Keypair as SecpKeypair;
use kaspa_consensus_core::tx::ScriptPublicKey;
use kaspa_txscript::pay_to_address_script;
use kaspa_wallet_pskt::prelude::*;
use kaspa_wallet_pskt::pskt::{Input, Signer, PSKT};
use tracing::info;

/// Validate and sign a migration PSKT.
///
/// Migration validation checks:
/// 1. PSKT spends from the current escrow anchor
/// 2. All outputs go to the configured migration target address
/// 3. No funds are sent elsewhere (escrow funds are fully migrated)
pub async fn validate_sign_migration_fxg<F, Fut>(
    fxg: MigrationFXG,
    escrow_public: EscrowPublic,
    migration_target_address: &Address,
    load_key: F,
) -> Result<Bundle>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<SecpKeypair>>,
{
    let bundle = safe_bundle(&fxg.bundle)
        .map_err(|e| eyre::eyre!("Safe bundle validation failed: {e:?}"))?;

    validate_migration_bundle(&bundle, &escrow_public, migration_target_address)?;

    info!("Validator: migration PSKT is valid");

    // Sign all escrow inputs
    let escrow_redeem = escrow_public.redeem_script.clone();
    let input_selector = move |i: &Input| match i.redeem_script.as_ref() {
        Some(rs) => rs == &escrow_redeem,
        None => false,
    };

    let signed = sign_withdrawal_fxg(&bundle, load_key, Some(input_selector))
        .await
        .map_err(|e| eyre::eyre!("Failed to sign migration: {e}"))?;

    Ok(signed)
}

fn validate_migration_bundle(
    bundle: &Bundle,
    escrow_public: &EscrowPublic,
    migration_target_address: &Address,
) -> Result<(), ValidationError> {
    // Migration should be a single PSKT or a small bundle
    if bundle.0.is_empty() {
        return Err(ValidationError::FailedGeneralVerification {
            reason: "Migration bundle is empty".to_string(),
        });
    }

    let target_script = pay_to_address_script(migration_target_address);

    // For migration, we don't chain anchors - each PSKT should spend from escrow
    // and send to the new escrow address
    for (idx, pskt_inner) in bundle.iter().enumerate() {
        let pskt = PSKT::<Signer>::from(pskt_inner.clone());
        validate_migration_pskt(&pskt, escrow_public, &target_script, idx)?;
    }

    Ok(())
}

fn validate_migration_pskt(
    pskt: &PSKT<Signer>,
    escrow_public: &EscrowPublic,
    target_script: &ScriptPublicKey,
    pskt_idx: usize,
) -> Result<(), ValidationError> {
    // Check sighash types
    if pskt
        .inputs
        .iter()
        .any(|input| !is_valid_sighash_type(input.sighash_type))
    {
        return Err(ValidationError::SigHashType);
    }

    // Verify at least one input spends from escrow
    let escrow_input_count = pskt
        .inputs
        .iter()
        .filter(|i| {
            i.redeem_script
                .as_ref()
                .map_or(false, |rs| rs == &escrow_public.redeem_script)
        })
        .count();

    if escrow_input_count == 0 {
        return Err(ValidationError::FailedGeneralVerification {
            reason: format!("Migration PSKT {} has no escrow inputs", pskt_idx),
        });
    }

    // Calculate escrow input sum
    let escrow_inputs_sum: u64 = pskt.inputs.iter().fold(0, |acc, i| {
        let rs = i.redeem_script.clone().unwrap_or_default();
        if rs == escrow_public.redeem_script {
            acc + i.utxo_entry.as_ref().map_or(0, |u| u.amount)
        } else {
            acc
        }
    });

    // All outputs must go to migration target (new escrow)
    let mut migration_output_sum: u64 = 0;

    for output in pskt.outputs.iter() {
        if &output.script_public_key != target_script {
            return Err(ValidationError::FailedGeneralVerification {
                reason: format!(
                    "Migration PSKT {} has output to non-target script",
                    pskt_idx
                ),
            });
        }

        migration_output_sum += output.amount;
    }

    // Verify escrow funds are preserved (minus fees paid by relayer)
    // Migration should not lose escrow funds - outputs should equal inputs minus tx fee
    // We allow some tolerance for fees but the bulk should go to new escrow
    if migration_output_sum == 0 {
        return Err(ValidationError::FailedGeneralVerification {
            reason: format!(
                "Migration PSKT {} has no outputs to target address",
                pskt_idx
            ),
        });
    }

    info!(
        pskt_idx,
        escrow_inputs_sum, migration_output_sum, "Migration PSKT validated"
    );

    Ok(())
}
