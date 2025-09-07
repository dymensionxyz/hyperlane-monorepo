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

/// Create a bundle that sweeps funds in the escrow address.
/// The function expects a set of inputs that are needed to be swept – [`escrow_inputs`].
/// And a set of relayer inputs to cover the transaction fee – [`relayer_inputs`].
/// Creates multiple PSKTs to respect mass limits, consolidating escrow inputs to 1 output per PSKT.
/// All relayer inputs are included in every PSKT to sweep relayer UTXOs as well.
pub async fn create_sweeping_bundle(
    relayer_wallet: &EasyKaspaWallet,
    escrow: &EscrowPublic,
    escrow_inputs: Vec<PopulatedInput>,
    relayer_inputs: Vec<PopulatedInput>,
) -> Result<Bundle> {
    if escrow_inputs.is_empty() {
        return Err(eyre!("No escrow inputs to sweep"));
    }
    
    let total_relayer_balance = relayer_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();
    
    // Calculate optimal batch sizes to respect mass limits
    // Conservative estimate based on Kaspa network: max mass ~100,000
    const BASE_MASS: u64 = 5000;
    const MASS_PER_ESCROW_INPUT: u64 = 2500; // multisig inputs are heavier
    const MASS_PER_RELAYER_INPUT: u64 = 1200;
    const MASS_PER_OUTPUT: u64 = 800;
    const MAX_MASS: u64 = 85000; // Conservative limit
    
    // Calculate how many escrow inputs we can fit per PSKT
    // Account for all relayer inputs being included in each PSKT
    let mass_for_two_outputs = 2 * MASS_PER_OUTPUT;
    let mass_for_all_relayer_inputs = relayer_inputs.len() as u64 * MASS_PER_RELAYER_INPUT;
    let available_mass = MAX_MASS.saturating_sub(BASE_MASS + mass_for_all_relayer_inputs + mass_for_two_outputs);
    let max_escrow_inputs_per_pskt = std::cmp::max(1, available_mass / MASS_PER_ESCROW_INPUT) as usize;
    
    let relayer_address = relayer_wallet.account().change_address()?;
    
    let mut bundle = Bundle::new();
    let mut remaining_escrow_inputs = escrow_inputs;
    
    // Process escrow inputs in batches
    while !remaining_escrow_inputs.is_empty() {
        let batch_size = std::cmp::min(max_escrow_inputs_per_pskt, remaining_escrow_inputs.len());
        let batch_escrow_inputs: Vec<_> = remaining_escrow_inputs.drain(0..batch_size).collect();
        let batch_escrow_balance = batch_escrow_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();
        
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
        
        // Add ALL relayer inputs to this PSKT (for sweeping)
        // Only include them in the first PSKT to avoid double-spending
        let include_relayer = bundle.0.is_empty(); // First PSKT
        if include_relayer {
            for (input, entry, _) in &relayer_inputs {
                let mut b = InputBuilder::default();
                b.previous_outpoint(input.previous_outpoint)
                    .sig_op_count(RELAYER_SIG_OP_COUNT)
                    .sighash_type(input_sighash_type())
                    .utxo_entry(entry.clone());
                
                pskt = pskt.input(b.build().map_err(|e| eyre!("Build relayer input: {}", e))?);
            }
        }
        
        // Add escrow output (consolidated from batch)
        let escrow_output_builder = OutputBuilder::default()
            .amount(batch_escrow_balance)
            .script_public_key(escrow.p2sh.clone())
            .build()
            .map_err(|e| eyre!("Build escrow output: {}", e))?;
        
        pskt = pskt.output(escrow_output_builder);
        
        // Add relayer output (consolidating all relayer inputs) - only on first PSKT
        if include_relayer && total_relayer_balance > 0 {
            let relayer_output_builder = OutputBuilder::default()
                .amount(total_relayer_balance)
                .script_public_key(pay_to_address_script(&relayer_address))
                .build()
                .map_err(|e| eyre!("Build relayer output: {}", e))?;
            
            pskt = pskt.output(relayer_output_builder);
        }
        
        bundle.add_pskt(pskt.no_more_inputs().no_more_outputs().signer());
    }
    
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
