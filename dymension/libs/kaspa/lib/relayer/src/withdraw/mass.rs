use super::messages::PopulatedInput;
use super::sweep::utxo_reference_from_populated_input;
use eyre::Result;
use kaspa_consensus_core::config::params::Params;
use kaspa_consensus_core::constants::TX_VERSION;
use kaspa_consensus_core::network::NetworkId;
use kaspa_consensus_core::subnets::SUBNETWORK_ID_NATIVE;
use kaspa_consensus_core::tx::{Transaction, TransactionOutput};
use kaspa_wallet_core::tx::MassCalculator;

pub fn estimate_mass(
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
            let utxo_ref = utxo_reference_from_populated_input(populated);
            (input, utxo_ref)
        })
        .unzip();

    let tx = Transaction::new(
        TX_VERSION,
        inputs,
        outputs.clone(),
        0, // no tx lock time
        SUBNETWORK_ID_NATIVE,
        0,
        payload,
    );

    let p = Params::from(network_id);
    let m = MassCalculator::new(&p);

    // mass calculation
    let mass = calc_mass_for_unsigned_consensus_transaction(
        &m,
        &tx,
        &utxo_references,
        &outputs,
        min_signatures,
    )?;

    Ok(mass)
}

/// Mass calculator implementation based on Kaspa's rusty-kaspa approach
/// This provides a method for calculating transaction mass by separating
/// compute and storage mass calculations
fn calc_mass_for_unsigned_consensus_transaction(
    mass_calculator: &MassCalculator,
    tx: &Transaction,
    utxo_references: &[kaspa_wallet_core::utxo::UtxoEntryReference],
    outputs: &[TransactionOutput],
    min_signatures: u16,
) -> Result<u64> {
    // Calculate storage mass using harmonic mean for outputs
    let storage_mass_outputs = calc_storage_mass_output_harmonic(mass_calculator, outputs)
        .ok_or_else(|| eyre::eyre!("Failed to calculate storage mass for outputs"))?;
    
    // Calculate storage mass for inputs using arithmetic mean
    let total_input_value: u64 = utxo_references.iter().map(|utxo| utxo.amount()).sum();
    let num_inputs = utxo_references.len() as u64;
    
    let storage_mass_inputs = if num_inputs > 0 && total_input_value > 0 {
        calc_storage_mass_input_mean_arithmetic(mass_calculator, total_input_value, num_inputs)
    } else {
        0
    };
    
    let total_storage_mass = storage_mass_inputs.saturating_add(storage_mass_outputs);
    
    // Calculate compute mass for the transaction structure
    let compute_mass = calc_compute_mass_for_unsigned_consensus_transaction(mass_calculator, tx, min_signatures);
    
    // Combine masses using maximum approach (as per Kaspa implementation)
    let combined_mass = std::cmp::max(compute_mass, total_storage_mass);
    
    Ok(combined_mass)
}

/// Calculate storage mass for outputs using harmonic approach
fn calc_storage_mass_output_harmonic(
    _mass_calculator: &MassCalculator,
    outputs: &[TransactionOutput],
) -> Option<u64> {
    // Get storage mass parameter from mass calculator
    // Using a reasonable default based on Kaspa network parameters
    let storage_mass_parameter = 10_000_000u64; // This should ideally come from consensus params
    
    outputs
        .iter()
        .map(|output| {
            if output.value > 0 {
                storage_mass_parameter.checked_div(output.value)
            } else {
                Some(storage_mass_parameter) // Handle zero-value outputs
            }
        })
        .try_fold(0u64, |total, current| {
            current.and_then(|current| total.checked_add(current))
        })
}

/// Calculate storage mass for inputs using arithmetic mean
fn calc_storage_mass_input_mean_arithmetic(
    _mass_calculator: &MassCalculator,
    total_input_value: u64,
    number_of_inputs: u64,
) -> u64 {
    let storage_mass_parameter = 10_000_000u64; // This should ideally come from consensus params
    
    if number_of_inputs == 0 || total_input_value == 0 {
        return 0;
    }
    
    let mean_input_value = total_input_value / number_of_inputs;
    if mean_input_value == 0 {
        return number_of_inputs.saturating_mul(storage_mass_parameter);
    }
    
    number_of_inputs.saturating_mul(storage_mass_parameter / mean_input_value)
}

/// Calculate compute mass for transaction structure
fn calc_compute_mass_for_unsigned_consensus_transaction(
    _mass_calculator: &MassCalculator,
    tx: &Transaction,
    min_signatures: u16,
) -> u64 {
    // Base transaction mass (constant overhead)
    let base_mass = 1000u64;
    
    // Mass per input (considering signature requirements)
    let input_mass = tx.inputs.len() as u64 * 1000u64;
    
    // Mass per output 
    let output_mass = tx.outputs.len() as u64 * 500u64;
    
    // Mass for signatures (estimated based on minimum required signatures)
    let signature_mass = min_signatures as u64 * 100u64;
    
    // Mass for payload
    let payload_mass = tx.payload.len() as u64;
    
    base_mass
        .saturating_add(input_mass)
        .saturating_add(output_mass)
        .saturating_add(signature_mass)
        .saturating_add(payload_mass)
}