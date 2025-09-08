use super::messages::PopulatedInput;
use corelib::escrow::EscrowPublic;
use eyre::{eyre, Result};
use kaspa_consensus_core::network::NetworkId;
use kaspa_consensus_core::tx::TransactionOutput;
use kaspa_txscript::standard::pay_to_address_script;
use tracing::info;

pub const MAX_MASS: u64 = 100000;

/// Calculate the maximum number of escrow inputs that fit within mass limit using binary search
pub fn calculate_batch_size(
    escrow_inputs: &[PopulatedInput],
    relayer_inputs: &[PopulatedInput],
    escrow: &EscrowPublic,
    relayer_address: &kaspa_addresses::Address,
    network_id: NetworkId,
    estimate_mass_fn: impl Fn(Vec<PopulatedInput>, Vec<TransactionOutput>, Vec<u8>, NetworkId, u16) -> Result<u64>,
) -> Result<usize> {
    if escrow_inputs.is_empty() {
        return Ok(0);
    }
    
    let total_relayer_balance = relayer_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();
    
    // First try all escrow inputs
    let total_escrow_balance = escrow_inputs.iter().map(|(_, e, _)| e.amount).sum::<u64>();
    
    let test_outputs = vec![
        TransactionOutput {
            value: total_escrow_balance,
            script_public_key: escrow.p2sh.clone(),
        },
        TransactionOutput {
            value: total_relayer_balance,
            script_public_key: pay_to_address_script(relayer_address),
        },
    ];
    
    let all_inputs: Vec<_> = escrow_inputs.iter().cloned()
        .chain(relayer_inputs.iter().cloned())
        .collect();
    
    match estimate_mass_fn(
        all_inputs,
        test_outputs,
        vec![],
        network_id,
        escrow.m() as u16,
    ) {
        Ok(mass) if mass <= MAX_MASS => {
            info!("Kaspa sweeping: all {} escrow inputs fit (mass: {})", escrow_inputs.len(), mass);
            return Ok(escrow_inputs.len());
        }
        Ok(mass) => {
            info!("Kaspa sweeping: all inputs exceed mass limit ({}), starting binary search", mass);
        }
        Err(e) => {
            info!("Kaspa sweeping: mass calculation failed: {}, starting binary search", e);
        }
    }
    
    // Binary search for maximum batch size
    let mut low = 1;
    let mut high = escrow_inputs.len();
    let mut best_size = 1;
    
    while low <= high {
        let mid = (low + high) / 2;
        let test_escrow_batch = escrow_inputs.iter().take(mid).cloned().collect::<Vec<_>>();
        let test_escrow_balance = test_escrow_batch.iter().map(|(_, e, _)| e.amount).sum::<u64>();
        
        let test_outputs = vec![
            TransactionOutput {
                value: test_escrow_balance,
                script_public_key: escrow.p2sh.clone(),
            },
            TransactionOutput {
                value: total_relayer_balance,
                script_public_key: pay_to_address_script(relayer_address),
            },
        ];
        
        let test_inputs: Vec<_> = test_escrow_batch
            .into_iter()
            .chain(relayer_inputs.iter().cloned())
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
                info!("Kaspa sweeping: batch size {} works (mass: {})", mid, mass);
            }
            Ok(mass) => {
                high = mid - 1;
                info!("Kaspa sweeping: batch size {} too large (mass: {})", mid, mass);
            }
            Err(e) => {
                high = mid - 1;
                info!("Kaspa sweeping: batch size {} failed: {}", mid, e);
            }
        }
    }
    
    if best_size == 0 {
        return Err(eyre!("Cannot create valid PSKT: even single escrow input exceeds mass limit"));
    }
    
    info!("Kaspa sweeping: optimal batch size: {}", best_size);
    Ok(best_size)
}