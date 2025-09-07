use crate::withdraw::messages::PopulatedInput;
use corelib::consts::RELAYER_SIG_OP_COUNT;
use corelib::escrow::EscrowPublic;
use corelib::util::input_sighash_type;
use corelib::wallet::EasyKaspaWallet;
use eyre::{eyre, Result};
use hardcode::tx::RELAYER_SWEEPING_PRIORITY_FEE;
use kaspa_consensus_client::{
    TransactionOutpoint as ClientTransactionOutpoint, UtxoEntry as ClientUtxoEntry,
};
use kaspa_consensus_core::constants::UNACCEPTED_DAA_SCORE;
use kaspa_consensus_core::tx::{TransactionInput, TransactionOutpoint, UtxoEntry};
use kaspa_wallet_core::utxo::UtxoEntryReference;
use kaspa_wallet_pskt::bundle::Bundle;
use kaspa_wallet_pskt::prelude::{Creator, OutputBuilder, Signer, PSKT};
use kaspa_wallet_pskt::pskt::InputBuilder;
use tracing::info;

/// Create a bundle that sweeps funds in the escrow address.
/// The function expects a set of inputs that are needed to be swept – [`escrow_inputs`].
/// And a set of relayer inputs to cover the transaction fee – [`relayer_inputs`].
/// Creates multiple PSKTs to respect mass limits.
/// Each PSKT includes only necessary relayer inputs to pay fees.
/// Each PSKT has exactly 2 outputs: consolidated escrow and relayer change.
pub async fn create_sweeping_bundle(
    relayer_wallet: &EasyKaspaWallet,
    escrow: &EscrowPublic,
    escrow_inputs: Vec<PopulatedInput>,
    mut relayer_inputs: Vec<PopulatedInput>,
) -> Result<Bundle> {
    use super::hub_to_kaspa::estimate_mass;
    use kaspa_consensus_core::constants::UNACCEPTED_DAA_SCORE;
    use kaspa_consensus_core::tx::{TransactionInput, TransactionOutpoint};
    
    if escrow_inputs.is_empty() {
        return Err(eyre!("No escrow inputs to sweep"));
    }
    
    let total_escrow_inputs = escrow_inputs.len();
    let total_escrow_balance = escrow_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();
    let total_relayer_inputs = relayer_inputs.len();
    let total_relayer_balance = relayer_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();
    
    info!(
        "Kaspa sweeping: starting sweep with {} escrow inputs ({} sompi), {} relayer inputs ({} sompi)",
        total_escrow_inputs, total_escrow_balance, total_relayer_inputs, total_relayer_balance
    );
    
    let relayer_address = relayer_wallet.account().change_address()?;
    
    // Get fee rate for fee estimation
    let feerate = relayer_wallet.api().get_fee_estimate().await?
        .normal_buckets.first().unwrap().feerate;
    
    info!("Kaspa sweeping: using feerate {}", feerate);
    
    // Kaspa network max mass is 100,000
    const MAX_MASS: u64 = 100000;
    const SAFETY_MARGIN: u64 = 5000; // Safety margin for mass calculation variations
    const TARGET_MASS: u64 = MAX_MASS - SAFETY_MARGIN;
    
    let mut bundle = Bundle::new();
    let mut remaining_escrow_inputs = escrow_inputs;
    let mut remaining_relayer_inputs = relayer_inputs;
    
    // Process escrow inputs in batches
    while !remaining_escrow_inputs.is_empty() {
        // First, find optimal batch size of escrow inputs without any relayer inputs
        let mut best_batch_size = 1;
        let mut low = 1;
        let mut high = std::cmp::min(remaining_escrow_inputs.len(), 50); // Cap at 50 for safety
        
        while low <= high {
            let mid = (low + high) / 2;
            let test_batch = remaining_escrow_inputs.iter().take(mid).cloned().collect::<Vec<_>>();
            let test_balance = test_batch.iter().map(|(_, e, _)| e.amount).sum::<u64>();
            
            // Create test outputs (minimal relayer output for now)
            let test_outputs = vec![
                TransactionOutput {
                    value: test_balance,
                    script_public_key: escrow.p2sh.clone(),
                },
                TransactionOutput {
                    value: 1000, // Minimal output
                    script_public_key: pay_to_address_script(&relayer_address),
                },
            ];
            
            // Calculate mass with just escrow inputs
            match estimate_mass(
                test_batch,
                test_outputs,
                vec![], // No payload for sweeping
                relayer_wallet.net.network_id,
                escrow.m() as u16,
            ) {
                Ok(mass) if mass < TARGET_MASS - 10000 => { // Leave room for relayer inputs
                    // This batch size works
                    best_batch_size = mid;
                    low = mid + 1; // Try larger batch
                }
                _ => {
                    // Too large or error
                    high = mid - 1; // Try smaller batch
                }
            }
        }
        
        if best_batch_size == 0 {
            return Err(eyre!(
                "Cannot create valid PSKT: even single escrow input exceeds mass limit"
            ));
        }
        
        // Take the batch of escrow inputs
        let batch_escrow_inputs: Vec<_> = remaining_escrow_inputs.drain(0..best_batch_size).collect();
        let batch_escrow_balance = batch_escrow_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();
        
        info!(
            "Kaspa sweeping: processing batch of {} escrow inputs ({} sompi), {} escrow inputs remaining",
            batch_escrow_inputs.len(), batch_escrow_balance, remaining_escrow_inputs.len()
        );
        
        // Now estimate fee and select minimal relayer inputs
        let mut selected_relayer_inputs = Vec::new();
        let mut selected_relayer_balance = 0u64;
        let mut estimated_fee = 0u64;
        
        // Iteratively add relayer inputs until we have enough to cover fees
        for _ in 0..5 { // Max 5 iterations to avoid infinite loop
            // Create test outputs with current relayer balance
            let test_outputs = vec![
                TransactionOutput {
                    value: batch_escrow_balance,
                    script_public_key: escrow.p2sh.clone(),
                },
                TransactionOutput {
                    value: selected_relayer_balance.saturating_sub(estimated_fee),
                    script_public_key: pay_to_address_script(&relayer_address),
                },
            ];
            
            // Combine escrow and selected relayer inputs
            let test_inputs: Vec<_> = batch_escrow_inputs.clone()
                .into_iter()
                .chain(selected_relayer_inputs.clone())
                .collect();
            
            // Calculate mass and fee
            match estimate_mass(
                test_inputs,
                test_outputs,
                vec![], // No payload for sweeping
                relayer_wallet.net.network_id,
                escrow.m() as u16,
            ) {
                Ok(mass) => {
                    estimated_fee = (mass as f64 * feerate).ceil() as u64 + RELAYER_SWEEPING_PRIORITY_FEE;
                    
                    // Check if we have enough to cover fees
                    if selected_relayer_balance >= estimated_fee {
                        break; // We have enough
                    }
                    
                    // Need more relayer inputs
                    if remaining_relayer_inputs.is_empty() {
                        return Err(eyre!(
                            "Insufficient relayer inputs to cover fees: need {} but only have {}",
                            estimated_fee,
                            selected_relayer_balance
                        ));
                    }
                    
                    // Add another relayer input
                    let next_input = remaining_relayer_inputs.remove(0);
                    let next_amount = next_input.1.amount;
                    selected_relayer_balance += next_amount;
                    selected_relayer_inputs.push(next_input);
                    
                    info!(
                        "Kaspa sweeping: added relayer input ({} sompi), total selected: {} sompi, estimated fee: {} sompi",
                        next_amount, selected_relayer_balance, estimated_fee
                    );
                }
                Err(e) => return Err(eyre!("Failed to estimate mass: {}", e)),
            }
        }
        
        // Calculate relayer output amount (minus fees)
        let relayer_output_amount = selected_relayer_balance - estimated_fee;
        
        // Log before creating PSKT
        info!(
            "Kaspa sweeping: creating PSKT with {} escrow inputs, {} relayer inputs, estimated fee: {} sompi, relayer change: {} sompi",
            batch_escrow_inputs.len(), selected_relayer_inputs.len(), estimated_fee, relayer_output_amount
        );
        
        // Create PSKT for this batch
        let mut pskt = PSKT::<Creator>::default().constructor();
        
        // Add escrow inputs for this batch
        for (input, entry, _) in batch_escrow_inputs {
            let mut b = InputBuilder::default();
            b.previous_outpoint(input.previous_outpoint)
                .sig_op_count(escrow.n() as u8)
                .sighash_type(input_sighash_type())
                .redeem_script(escrow.redeem_script.clone())
                .utxo_entry(entry);
            
            pskt = pskt.input(b.build().map_err(|e| eyre!("Build escrow input: {}", e))?);
        }
        
        // Add selected relayer inputs
        for (input, entry, _) in selected_relayer_inputs {
            let mut b = InputBuilder::default();
            b.previous_outpoint(input.previous_outpoint)
                .sig_op_count(RELAYER_SIG_OP_COUNT)
                .sighash_type(input_sighash_type())
                .utxo_entry(entry);
            
            pskt = pskt.input(b.build().map_err(|e| eyre!("Build relayer input: {}", e))?);
        }
        
        // Add escrow output (consolidated from batch)
        let escrow_output_builder = OutputBuilder::default()
            .amount(batch_escrow_balance)
            .script_public_key(escrow.p2sh.clone())
            .build()
            .map_err(|e| eyre!("Build escrow output: {}", e))?;
        
        pskt = pskt.output(escrow_output_builder);
        
        // Add relayer output (minus fees)  
        let relayer_output_builder = OutputBuilder::default()
            .amount(relayer_output_amount)
            .script_public_key(pay_to_address_script(&relayer_address))
            .build()
            .map_err(|e| eyre!("Build relayer output: {}", e))?;
        
        pskt = pskt.output(relayer_output_builder);
        
        let pskt_signer = pskt.no_more_inputs().no_more_outputs().signer();
        let pskt_id = pskt_signer.calculate_id();
        bundle.add_pskt(pskt_signer);
        
        info!(
            "Kaspa sweeping: successfully created PSKT {}",
            pskt_id
        );
        
        // Add the relayer output as a new relayer input for the next iteration
        if !remaining_escrow_inputs.is_empty() && relayer_output_amount > 0 {
            remaining_relayer_inputs.insert(0, (
                TransactionInput::new(
                    TransactionOutpoint::new(pskt_id, 1), // Index 1 is relayer output
                    vec![], // Empty signature script for unsigned
                    u64::MAX,
                    RELAYER_SIG_OP_COUNT,
                ),
                UtxoEntry::new(
                    relayer_output_amount,
                    pay_to_address_script(&relayer_address),
                    UNACCEPTED_DAA_SCORE,
                    false,
                ),
                None, // No redeem script for relayer
            ));
            
            info!(
                "Kaspa sweeping: chaining relayer output {} sompi to next PSKT",
                relayer_output_amount
            );
        }
    }
    
    info!(
        "Kaspa sweeping: completed sweep with {} PSKTs in bundle",
        bundle.0.len()
    );
    
    Ok(bundle)
}

use kaspa_consensus_core::tx::TransactionOutput;
use kaspa_txscript::standard::pay_to_address_script;

/// Add the redeem script, sig op count, and sig hash type to every input.
/// Otherwise, the transaction will fail. Outputs stay the same.
fn format_sweeping_bundle(bundle: Bundle, escrow: &EscrowPublic) -> Result<Bundle> {
    let mut new_bundle = Bundle::new();
    for inner in bundle.iter() {
        let mut pskt = PSKT::<Creator>::default().constructor();

        for input in inner.inputs.iter() {
            let utxo_entry = input
                .utxo_entry
                .clone()
                .ok_or_else(|| eyre::eyre!("missing utxo_entry"))?;

            let mut b = InputBuilder::default();

            b.previous_outpoint(input.previous_outpoint)
                .sig_op_count(RELAYER_SIG_OP_COUNT)
                .sighash_type(input_sighash_type());

            // Add redeem script and correct sig op count for escrow inputs
            if utxo_entry.script_public_key == escrow.p2sh {
                b.redeem_script(escrow.redeem_script.clone())
                    .sig_op_count(escrow.n() as u8);
            }

            b.utxo_entry(utxo_entry);

            pskt = pskt.input(
                b.build()
                    .map_err(|e| eyre::eyre!("Build pskt input: {}", e))?,
            );
        }

        for output in inner.outputs.iter() {
            let b = OutputBuilder::default()
                .amount(output.amount)
                .script_public_key(output.script_public_key.clone())
                .build()
                .map_err(|e| eyre::eyre!("Build pskt output for withdrawal: {}", e))?;

            pskt = pskt.output(b);
        }

        new_bundle.add_pskt(pskt.no_more_inputs().no_more_outputs().signer());
    }
    Ok(new_bundle)
}

pub fn create_inputs_from_sweeping_bundle(
    sweeping_bundle: &Bundle,
    escrow: &EscrowPublic,
) -> Result<Vec<PopulatedInput>> {
    let last_pskt = sweeping_bundle
        .iter()
        .last()
        .cloned()
        .ok_or_else(|| eyre!("Empty sweeping bundle"))?;

    let sweep_tx = PSKT::<Signer>::from(last_pskt);
    let tx_id = sweep_tx.calculate_id();

    // Expect exactly two outputs: {escrow, relayer} in some order.
    let (relayer_idx, relayer_output, escrow_idx, escrow_output) = match sweep_tx.outputs.as_slice() {
        [o0, o1] if o0.script_public_key == escrow.p2sh => (1u32, o1, 0u32, o0),
        [o0, o1] if o1.script_public_key == escrow.p2sh => (0u32, o0, 1u32, o1),
        _ => {
            return Err(eyre!(
                "Resulting sweeping TX must have exactly two outputs: swept escrow UTXO and relayer change"
            ))
        }
    };

    let relayer_input: PopulatedInput = (
        TransactionInput::new(
            TransactionOutpoint::new(tx_id, relayer_idx),
            vec![], // signature_script is empty for unsigned transactions
            u64::MAX,
            RELAYER_SIG_OP_COUNT,
        ),
        UtxoEntry::new(
            relayer_output.amount,
            relayer_output.script_public_key.clone(),
            UNACCEPTED_DAA_SCORE,
            false,
        ),
        None, // relayer has no redeem script
    );

    let escrow_input: PopulatedInput = (
        TransactionInput::new(
            TransactionOutpoint::new(tx_id, escrow_idx),
            vec![], // signature_script is empty for unsigned transactions
            u64::MAX,
            escrow.n() as u8,
        ),
        UtxoEntry::new(
            escrow_output.amount,
            escrow.p2sh.clone(),
            UNACCEPTED_DAA_SCORE,
            false,
        ),
        Some(escrow.redeem_script.clone()), // escrow has redeem script
    );

    Ok(vec![relayer_input, escrow_input])
}

pub(crate) fn utxo_reference_from_populated_input(
    (input, entry, _redeem_script): PopulatedInput,
) -> UtxoEntryReference {
    UtxoEntryReference::from(ClientUtxoEntry {
        address: None,
        outpoint: ClientTransactionOutpoint::from(input.previous_outpoint),
        amount: entry.amount,
        script_public_key: entry.script_public_key.clone(),
        block_daa_score: entry.block_daa_score,
        is_coinbase: entry.is_coinbase,
    })
}
