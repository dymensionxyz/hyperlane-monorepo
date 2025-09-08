use super::messages::PopulatedInput;
use corelib::escrow::EscrowPublic;
use eyre::{eyre, Result};
use kaspa_addresses::Address;
use kaspa_consensus_core::network::NetworkId;
use kaspa_consensus_core::tx::TransactionOutput;
use kaspa_txscript::standard::pay_to_address_script;
use tracing::info;

/// Maximum mass allowed for a Kaspa transaction
pub const MAX_MASS: u64 = 100000;

/// Find the maximum number of inputs that fit within mass limit using binary search.
/// Works for both escrow inputs (with redeem scripts) and regular inputs.
/// 
/// This function performs a binary search to find the optimal batch size that
/// maximizes the number of inputs while staying under the Kaspa mass limit.
/// 
/// # Arguments
/// * `target_inputs` - The inputs to batch (e.g., escrow inputs to consolidate)
/// * `additional_inputs` - Additional inputs to include in every transaction (e.g., relayer inputs for fees)
/// * `escrow` - Escrow public key information (for multisig transactions)
/// * `output_addresses` - Addresses for the outputs (typically escrow and relayer addresses)
/// * `network_id` - Kaspa network ID
/// * `estimate_mass_fn` - Function to estimate transaction mass
pub fn find_optimal_batch_size(
    target_inputs: &[PopulatedInput],
    additional_inputs: &[PopulatedInput],
    escrow: &EscrowPublic,
    escrow_address: &Address,
    relayer_address: &Address,
    network_id: NetworkId,
    estimate_mass_fn: impl Fn(Vec<PopulatedInput>, Vec<TransactionOutput>, Vec<u8>, NetworkId, u16) -> Result<u64>,
) -> Result<usize> {
    if target_inputs.is_empty() {
        return Ok(0);
    }
    
    let total_additional_balance = additional_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();
    
    // First try all target inputs
    let total_target_balance = target_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();
    
    let test_outputs = vec![
        TransactionOutput {
            value: total_target_balance,
            script_public_key: pay_to_address_script(escrow_address),
        },
        TransactionOutput {
            value: total_additional_balance,
            script_public_key: pay_to_address_script(relayer_address),
        },
    ];
    
    let all_inputs: Vec<_> = target_inputs.iter().cloned()
        .chain(additional_inputs.iter().cloned())
        .collect();
    
    match estimate_mass_fn(
        all_inputs,
        test_outputs,
        vec![],
        network_id,
        escrow.m() as u16,
    ) {
        Ok(mass) if mass <= MAX_MASS => {
            info!("All {} inputs fit within mass limit (mass: {})", target_inputs.len(), mass);
            return Ok(target_inputs.len());
        }
        Ok(mass) => {
            info!("All inputs exceed mass limit ({}), starting binary search", mass);
        }
        Err(e) => {
            info!("Mass calculation failed: {}, starting binary search", e);
        }
    }
    
    // Binary search for maximum batch size
    let mut low = 1;
    let mut high = target_inputs.len();
    let mut best_size = 1;
    
    while low <= high {
        let mid = (low + high) / 2;
        let test_batch = target_inputs.iter().take(mid).cloned().collect::<Vec<_>>();
        let test_balance = test_batch.iter().map(|(_, e, _)| e.amount).sum::<u64>();
        
        let test_outputs = vec![
            TransactionOutput {
                value: test_balance,
                script_public_key: pay_to_address_script(escrow_address),
            },
            TransactionOutput {
                value: total_additional_balance,
                script_public_key: pay_to_address_script(relayer_address),
            },
        ];
        
        let test_inputs: Vec<_> = test_batch
            .into_iter()
            .chain(additional_inputs.iter().cloned())
            .collect();
        
        match estimate_mass_fn(
            test_inputs,
            test_outputs,
            vec![],
            network_id,
            escrow.m() as u16,
        ) {
            Ok(mass) if mass <= MAX_MASS => {
                best_size = mid;
                low = mid + 1;
                info!("Batch size {} works (mass: {})", mid, mass);
            }
            Ok(mass) => {
                high = mid - 1;
                info!("Batch size {} too large (mass: {})", mid, mass);
            }
            Err(e) => {
                high = mid - 1;
                info!("Batch size {} failed: {}", mid, e);
            }
        }
    }
    
    if best_size == 0 {
        return Err(eyre!("Cannot create valid transaction: even single input exceeds mass limit"));
    }
    
    info!("Optimal batch size: {}", best_size);
    Ok(best_size)
}

/// Splits inputs into multiple batches that each fit within the mass limit.
/// Returns a vector of batches, where each batch is a vector of inputs.
pub fn split_inputs_by_mass(
    mut target_inputs: Vec<PopulatedInput>,
    additional_inputs: &[PopulatedInput],
    escrow: &EscrowPublic,
    escrow_address: &Address,
    relayer_address: &Address,
    network_id: NetworkId,
    estimate_mass_fn: impl Fn(Vec<PopulatedInput>, Vec<TransactionOutput>, Vec<u8>, NetworkId, u16) -> Result<u64> + Copy,
) -> Result<Vec<Vec<PopulatedInput>>> {
    let mut batches = Vec::new();
    
    while !target_inputs.is_empty() {
        let batch_size = find_optimal_batch_size(
            &target_inputs,
            additional_inputs,
            escrow,
            escrow_address,
            relayer_address,
            network_id,
            estimate_mass_fn,
        )?;
        
        if batch_size == 0 {
            return Err(eyre!("Unable to create batch: inputs exceed mass limit"));
        }
        
        let batch: Vec<_> = target_inputs.drain(0..batch_size).collect();
        info!("Created batch with {} inputs", batch.len());
        batches.push(batch);
    }
    
    info!("Split inputs into {} batches", batches.len());
    Ok(batches)
}