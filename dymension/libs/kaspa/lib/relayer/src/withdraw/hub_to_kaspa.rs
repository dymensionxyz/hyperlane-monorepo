use super::batch_utils::calculate_batch_size;
use super::messages::PopulatedInput;
use super::minimum::is_dust;
use crate::withdraw::sweep::utxo_reference_from_populated_input;
use corelib::escrow::EscrowPublic;
use corelib::finality;
use corelib::message::parse_hyperlane_metadata;
use corelib::util::{get_recipient_script_pubkey, input_sighash_type};
use corelib::wallet::EasyKaspaWallet;
use corelib::withdraw::WithdrawFXG;
use eyre::{eyre, Result};
use hyperlane_core::HyperlaneMessage;
use kaspa_addresses::{Address, Prefix};
use kaspa_consensus_core::config::params::Params;
use kaspa_consensus_core::constants::UNACCEPTED_DAA_SCORE;
use kaspa_consensus_core::network::NetworkId;
use kaspa_consensus_core::tx::{TransactionInput, TransactionOutpoint, TransactionOutput, UtxoEntry};
use kaspa_wallet_core::prelude::DynRpcApi;
use kaspa_wallet_core::tx::MassCalculator;
use kaspa_wallet_core::utxo::UtxoEntryReference;
use kaspa_wallet_pskt::bundle::Bundle;
use kaspa_wallet_pskt::prelude::{Creator, OutputBuilder, Signer, PSKT};
use kaspa_wallet_pskt::pskt::Combiner;
use kaspa_wallet_pskt::pskt::InputBuilder;
use hyperlane_core::U256;
use std::sync::Arc;
use tracing::info;
use corelib::consts::RELAYER_SIG_OP_COUNT;

/// Fetch UTXOs from the escrow address
pub async fn fetch_input_utxos(
    addr: &Address,
    kaspa_rpc: &Arc<DynRpcApi>,
    network_id: NetworkId,
) -> Result<Vec<UtxoEntryReference>> {
    let mut utxos = kaspa_rpc
        .get_utxos_by_addresses(vec![addr.clone()])
        .await
        .map_err(|e| eyre!("Get escrow UTXOs: {}", e))?;

    let b = kaspa_rpc
        .get_block_dag_info()
        .await
        .map_err(|e| eyre!("Get block DAG info: {}", e))?;

    // Descending order – older UTXOs first
    utxos.sort_by_key(|u| std::cmp::Reverse(u.utxo_entry.block_daa_score));
    utxos.retain(|u| {
        finality::is_mature(
            u.utxo_entry.block_daa_score,
            b.virtual_daa_score,
            network_id,
        )
    });

    let utxo_references: Vec<UtxoEntryReference> = utxos
        .into_iter()
        .map(|rpc_utxo| rpc_utxo.into())
        .collect();
    
    Ok(utxo_references)
}

/// Get current fee rate from Kaspa network
pub async fn get_normal_bucket_feerate(kaspa_rpc: &Arc<DynRpcApi>) -> Result<f64> {
    let fee_estimate = kaspa_rpc
        .get_fee_estimate()
        .await
        .map_err(|e| eyre!("Get fee estimate: {}", e))?;
    
    Ok(fee_estimate.normal_buckets.first().unwrap().feerate)
}

/// Create a withdrawal bundle that handles mass limits by creating multiple chained PSKTs.
/// The first PSKT attempts to use all inputs. If mass exceeds limits, creates multiple PSKTs
/// where each PSKT uses maximum possible inputs and outputs become inputs for the next PSKT.
/// All withdrawal messages are distributed across the PSKTs.
pub fn build_withdrawal_bundle(
    inputs: Vec<PopulatedInput>,
    outputs: Vec<TransactionOutput>,
    payload: Vec<u8>,
    escrow: &EscrowPublic,
    relayer_addr: &kaspa_addresses::Address,
    network_id: NetworkId,
    _min_deposit_sompi: U256,
    feerate: f64,
) -> Result<Bundle> {
    use kaspa_txscript::standard::pay_to_address_script;

    // Separate escrow and relayer inputs
    let (mut escrow_inputs, relayer_inputs): (Vec<_>, Vec<_>) = inputs.into_iter()
        .partition(|(_, _, redeem_script)| redeem_script.is_some());

    if escrow_inputs.is_empty() {
        return Err(eyre!("No escrow inputs available for withdrawal"));
    }

    // Check initial mass with all inputs and outputs
    let all_inputs: Vec<_> = escrow_inputs.iter().cloned()
        .chain(relayer_inputs.iter().cloned())
        .collect();

    match estimate_mass(
        all_inputs.clone(),
        outputs.clone(),
        payload.clone(),
        network_id,
        escrow.m() as u16,
    ) {
        Ok(mass) if mass <= super::batch_utils::MAX_MASS => {
            // All fits in single PSKT - build single withdrawal
            info!("All inputs/outputs fit in single PSKT (mass: {})", mass);
            let mut bundle = Bundle::new();
            bundle.add_pskt(build_pskt_from_components(
                all_inputs, outputs, payload, escrow
            )?);
            return Ok(bundle);
        }
        Ok(mass) => {
            info!("Mass {} exceeds limit, creating multiple chained PSKTs", mass);
        }
        Err(e) => {
            info!("Mass calculation failed: {}, creating multiple chained PSKTs", e);
        }
    }

    let mut bundle = Bundle::new();
    let mut remaining_outputs = outputs;
    let mut current_relayer_inputs = relayer_inputs;

    info!(
        "Creating withdrawal bundle with {} escrow inputs, {} outputs, {} relayer inputs",
        escrow_inputs.len(),
        remaining_outputs.len(),
        current_relayer_inputs.len()
    );

    while !escrow_inputs.is_empty() || !remaining_outputs.is_empty() {
        // Calculate optimal batch size for current escrow inputs
        let batch_size = if escrow_inputs.is_empty() {
            0
        } else {
            calculate_batch_size(
                &escrow_inputs,
                &current_relayer_inputs,
                escrow,
                relayer_addr,
                network_id,
                estimate_mass,
            )?
        };

        // Take batch of escrow inputs
        let batch_escrow_inputs: Vec<_> = if batch_size > 0 {
            escrow_inputs.drain(0..batch_size).collect()
        } else {
            vec![]
        };

        // Calculate how many withdrawal outputs we can include
        let batch_escrow_balance = batch_escrow_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();
        let relayer_balance = current_relayer_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();

        // Determine outputs for this PSKT
        let mut batch_outputs = Vec::new();
        let mut outputs_value = 0u64;

        // Add as many withdrawal outputs as we can fit
        let max_withdrawal_outputs = remaining_outputs.len();

        for _ in 0..max_withdrawal_outputs {
            if let Some(output) = remaining_outputs.pop() {
                let output_value = output.value;
                outputs_value += output_value;
                if outputs_value <= batch_escrow_balance {
                    batch_outputs.push(output);
                } else {
                    // Put it back if it doesn't fit
                    remaining_outputs.insert(0, output);
                    outputs_value -= output_value;
                    break;
                }
            }
        }

        // Calculate escrow change
        let escrow_change = batch_escrow_balance - outputs_value;

        // Add escrow change output (becomes input for next PSKT)
        if escrow_change > 0 {
            batch_outputs.push(TransactionOutput {
                value: escrow_change,
                script_public_key: escrow.p2sh.clone(),
            });
        }

        // Estimate mass and fee for this batch
        let batch_inputs: Vec<_> = batch_escrow_inputs.iter().cloned()
            .chain(current_relayer_inputs.iter().cloned())
            .collect();

        let batch_mass = estimate_mass(
            batch_inputs.clone(),
            batch_outputs.clone(),
            if remaining_outputs.is_empty() { payload.clone() } else { vec![] },
            network_id,
            escrow.m() as u16,
        )?;

        let batch_fee = (batch_mass as f64 * feerate).round() as u64;

        if relayer_balance < batch_fee {
            return Err(eyre!(
                "Insufficient relayer funds for batch: {} < {}",
                relayer_balance, batch_fee
            ));
        }

        let relayer_change = relayer_balance - batch_fee;

        // Add relayer change output
        batch_outputs.push(TransactionOutput {
            value: relayer_change,
            script_public_key: pay_to_address_script(relayer_addr),
        });

        // Build PSKT
        let pskt = build_pskt_from_components(
            batch_inputs,
            batch_outputs.clone(),
            if remaining_outputs.is_empty() { payload.clone() } else { vec![] },
            escrow,
        )?;

        let pskt_id = pskt.calculate_id();
        bundle.add_pskt(pskt);

        info!(
            "Created PSKT {} with {} escrow inputs, {} outputs, fee: {}",
            pskt_id,
            batch_escrow_inputs.len(),
            batch_outputs.len(),
            batch_fee
        );

        // Prepare inputs for next PSKT (use outputs from current PSKT)
        if !escrow_inputs.is_empty() || !remaining_outputs.is_empty() {
            // Find escrow and relayer outputs to use as inputs for next PSKT
            let mut next_escrow_inputs = Vec::new();
            let mut next_relayer_inputs = Vec::new();

            for (idx, output) in batch_outputs.iter().enumerate() {
                if output.script_public_key == escrow.p2sh {
                    // This is escrow output, add as escrow input for next PSKT
                    next_escrow_inputs.push((
                        TransactionInput::new(
                            TransactionOutpoint::new(pskt_id, idx as u32),
                            vec![],
                            u64::MAX,
                            escrow.n() as u8,
                        ),
                        UtxoEntry::new(
                            output.value,
                            output.script_public_key.clone(),
                            UNACCEPTED_DAA_SCORE,
                            false,
                        ),
                        Some(escrow.redeem_script.clone()),
                    ));
                } else if output.script_public_key == pay_to_address_script(relayer_addr) {
                    // This is relayer output, add as relayer input for next PSKT
                    next_relayer_inputs.push((
                        TransactionInput::new(
                            TransactionOutpoint::new(pskt_id, idx as u32),
                            vec![],
                            u64::MAX,
                            RELAYER_SIG_OP_COUNT,
                        ),
                        UtxoEntry::new(
                            output.value,
                            output.script_public_key.clone(),
                            UNACCEPTED_DAA_SCORE,
                            false,
                        ),
                        None,
                    ));
                }
            }

            // Update inputs for next iteration
            escrow_inputs.extend(next_escrow_inputs);
            current_relayer_inputs = next_relayer_inputs;
        }
    }

    info!("Withdrawal bundle created with {} PSKTs", bundle.0.len());
    Ok(bundle)
}

/// Build a single PSKT from components - internal helper function
fn build_pskt_from_components(
    inputs: Vec<PopulatedInput>,
    outputs: Vec<TransactionOutput>,
    payload: Vec<u8>,
    escrow: &EscrowPublic,
) -> Result<PSKT<Signer>> {
    let mut pskt = PSKT::<Creator>::default().constructor();

    // Add inputs
    for (input, entry, redeem_script) in inputs {
        let mut b = InputBuilder::default();
        b.previous_outpoint(input.previous_outpoint)
            .sighash_type(input_sighash_type())
            .utxo_entry(entry.clone());

        if let Some(redeem_script) = redeem_script {
            // Escrow input
            b.sig_op_count(escrow.n() as u8)
                .redeem_script(redeem_script);
        } else {
            // Relayer input
            b.sig_op_count(RELAYER_SIG_OP_COUNT);
        }

        pskt = pskt.input(b.build().map_err(|e| eyre!("Build input: {}", e))?);
    }

    // Add outputs
    for output in outputs {
        let output_builder = OutputBuilder::default()
            .amount(output.value)
            .script_public_key(output.script_public_key)
            .build()
            .map_err(|e| eyre!("Build output: {}", e))?;
        pskt = pskt.output(output_builder);
    }

    // Add payload if present
    if !payload.is_empty() {
        pskt = pskt.payload(payload);
    }

    Ok(pskt.no_more_inputs().no_more_outputs().signer())
}

/// Build withdrawal PSKTs as a bundle - primary interface for withdrawal operations
pub fn build_withdrawal_pskt(
    inputs: Vec<PopulatedInput>,
    outputs: Vec<TransactionOutput>,
    payload: Vec<u8>,
    escrow: &EscrowPublic,
    relayer_addr: &kaspa_addresses::Address,
    network_id: NetworkId,
    min_deposit_sompi: U256,
    feerate: f64,
) -> Result<Bundle> {
    build_withdrawal_bundle(
        inputs, outputs, payload, escrow, relayer_addr, network_id, min_deposit_sompi, feerate
    )
}

/// Return outputs generated based on the provided messages. Filter out messages
/// with dust amount.
pub fn get_outputs_from_msgs(
    messages: Vec<HyperlaneMessage>,
    prefix: Prefix,
    min_deposit_sompi: U256,
) -> (Vec<HyperlaneMessage>, Vec<TransactionOutput>) {
    let mut hl_msgs: Vec<HyperlaneMessage> = Vec::new();
    let mut outputs: Vec<TransactionOutput> = Vec::new();
    for m in messages {
        let tm = match parse_hyperlane_metadata(&m) {
            Ok(tm) => tm,
            Err(e) => {
                info!(
                    "Kaspa relayer, can't get TokenMessage from HyperlaneMessage body, skipping: {}",
                    e
                );
                continue;
            }
        };

        let recipient = get_recipient_script_pubkey(tm.recipient(), prefix);
        let o = TransactionOutput::new(tm.amount().as_u64(), recipient);

        if is_dust(&o, min_deposit_sompi) {
            info!("Kaspa relayer, withdrawal amount is less than dust amount, skipping, amount: {}, message id: {:?}", o.value, m.id());
            continue;
        }

        outputs.push(o);
        hl_msgs.push(m);
    }
    (hl_msgs, outputs)
}

pub(crate) fn extract_current_anchor(
    current_anchor: TransactionOutpoint,
    mut escrow_inputs: Vec<PopulatedInput>,
) -> Result<(PopulatedInput, Vec<PopulatedInput>)> {
    let anchor_index = escrow_inputs
        .iter()
        .position(|(input, _, _)| input.previous_outpoint == current_anchor)
        .ok_or(eyre!(
            "Current anchor not found in escrow UTXO set: {current_anchor:?}"
        ))?; // Should always be found

    let anchor_input = escrow_inputs.swap_remove(anchor_index);

    Ok((anchor_input, escrow_inputs))
}

pub(crate) fn estimate_mass(
    populated_inputs: Vec<PopulatedInput>,
    outputs: Vec<TransactionOutput>,
    payload: Vec<u8>,
    network_id: NetworkId,
    min_signatures: u16,
) -> Result<u64> {
    let (inputs, utxo_references): (Vec<_>, Vec<_>) = populated_inputs
        .into_iter()
        .map(|populated| {
            let input = populated.0.clone();
            let utxo_reference = utxo_reference_from_populated_input(populated);
            (input, utxo_reference)
        })
        .unzip();

    let tx = kaspa_consensus_core::tx::Transaction::new(
        kaspa_consensus_core::constants::TX_VERSION,
        inputs,
        outputs,
        0,
        kaspa_consensus_core::subnets::SUBNETWORK_ID_NATIVE,
        0,
        payload,
    );
    
    let params = kaspa_consensus_core::config::params::Params::from(network_id);
    let m = MassCalculator::new(&params);
    m.calc_overall_mass_for_unsigned_consensus_transaction(
        &tx,
        utxo_references.as_slice(),
        min_signatures,
    )
    .map_err(|e| eyre!(e))
}

pub async fn combine_bundles_with_fee(
    bundles_validators: Vec<Bundle>,
    _fxg: &WithdrawFXG,
    _multisig_threshold: usize,
    _escrow: &EscrowPublic,
    _easy_wallet: &EasyKaspaWallet,
) -> Result<Vec<kaspa_rpc_core::RpcTransaction>> {
    info!("Kaspa provider, processing withdrawal bundles");
    
    let mut final_transactions = Vec::new();
    
    for bundle in bundles_validators {
        for inner_pskt in bundle.iter() {
            let pskt_combiner = PSKT::<Combiner>::from(inner_pskt.clone());
            
            // TODO: Add validator signatures using proper API
            // TODO: Sign relayer inputs using proper API
            
            // Finalize
            let rpc_tx = finalize_pskt(pskt_combiner)?;
            final_transactions.push(rpc_tx);
        }
    }
    
    Ok(final_transactions)
}

pub fn finalize_pskt(
    _c: PSKT<Combiner>,
) -> Result<kaspa_rpc_core::RpcTransaction> {
    // TODO: Implement proper PSKT finalization using correct API
    Err(eyre!("PSKT finalization not yet implemented"))
}

