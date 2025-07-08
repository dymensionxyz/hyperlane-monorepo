use corelib::{confirmation::ConfirmationFXG, deposit::DepositFXG, withdraw::WithdrawFXG};

use eyre::Result;

use api_rs::apis::{
    configuration::Configuration,
    kaspa_transactions_api::{
        get_transaction_transactions_transaction_id_get,
        GetTransactionTransactionsTransactionIdGetParams,
    },
};
use corelib::payload::{MessageID, MessageIDs};
use kaspa_consensus_core::tx::TransactionOutpoint;
use kaspa_hashes::Hash as KaspaHash;
use std::collections::HashSet;
use tracing::{info, warn};

pub async fn validate_confirmed_withdrawals(
    fxg: &ConfirmationFXG,
    config: &Configuration,
) -> Result<bool> {
    info!("Validator: Starting validation of withdrawals confirmation");
    
    // Get outpoints from cache in reverse order for validation
    let outpoints = &fxg.cache.outpoints;
    if outpoints.len() < 2 {
        warn!("Validator: Insufficient outpoints in cache for validation");
        return Ok(false);
    }

    let mut collected_message_ids = Vec::new();
    let mut prev_address: Option<String> = None;
    
    // Iterate through outpoints in reverse (from new to old)
    for (i, curr_outpoint) in outpoints.iter().rev().enumerate() {
        info!(
            "Validator: Processing outpoint {} of {}: {:?}",
            i + 1,
            outpoints.len(),
            curr_outpoint
        );

        // Query the transaction
        let transaction = get_transaction_transactions_transaction_id_get(
            config,
            GetTransactionTransactionsTransactionIdGetParams {
                transaction_id: curr_outpoint.transaction_id.to_string(),
                block_hash: None,
                inputs: Some(true),
                outputs: Some(true),
                resolve_previous_outpoints: Some("light".to_string()),
            },
        )
        .await
        .map_err(|e| {
            eyre::eyre!(
                "Validator: Failed to get transaction {}: {}",
                curr_outpoint.transaction_id,
                e
            )
        })?;

        // 1.2 Validate the transaction (stub as requested)
        if !validate_transaction_stub(&transaction).await? {
            warn!("Validator: Transaction validation failed");
            return Ok(false);
        }

        // 1.3 Validate the previous transaction is in the inputs (if not the first iteration)
        if i > 0 {
            let prev_outpoint = &outpoints[outpoints.len() - i]; // Previous outpoint in forward order
            if !validate_previous_transaction_in_inputs(&transaction, prev_outpoint)? {
                warn!("Validator: Previous transaction not found in inputs");
                return Ok(false);
            }
        }

        // 1.4 Validate it has same address (lineage validation)
        let curr_address = get_output_address(&transaction, curr_outpoint)?;
        if let Some(ref prev_addr) = prev_address {
            if curr_address != *prev_addr {
                warn!("Validator: Address lineage broken: {} != {}", curr_address, prev_addr);
                return Ok(false);
            }
        }
        prev_address = Some(curr_address);

        // 1.5 Extract the messageID from the payload
        if let Some(ref payload) = transaction.payload {
            let message_ids = MessageIDs::from_bytes(payload.as_bytes())
                .map_err(|e| eyre::eyre!("Validator: Failed to deserialize MessageIDs: {}", e))?;
            collected_message_ids.extend(message_ids.0);
        } else {
            // Skip transactions without payloads (might be intermediate transactions)
            info!("Validator: Transaction has no payload, skipping message ID extraction");
        }
    }

    // Validate that progress_indication is correct according to collected data
    if !validate_progress_indication(fxg, &collected_message_ids)? {
        warn!("Validator: Progress indication validation failed");
        return Ok(false);
    }

    info!("Validator: All validations passed successfully");
    Ok(true)
}

/// Stub for transaction validation as requested
async fn validate_transaction_stub(
    _transaction: &api_rs::models::TxModel,
) -> Result<bool> {
    // TODO: Implement actual transaction validation logic
    // This could include:
    // - Verify transaction format
    // - Check transaction signatures
    // - Validate transaction amounts
    // - Check if transaction is accepted/confirmed
    Ok(true)
}

/// Validate that the previous transaction is referenced in the current transaction's inputs
fn validate_previous_transaction_in_inputs(
    transaction: &api_rs::models::TxModel,
    prev_outpoint: &TransactionOutpoint,
) -> Result<bool> {
    let inputs = transaction
        .inputs
        .as_ref()
        .ok_or_else(|| eyre::eyre!("Validator: Transaction inputs not found"))?;

    for input in inputs {
        if input.previous_outpoint_hash == prev_outpoint.transaction_id.to_string()
            && input.previous_outpoint_index == prev_outpoint.index.to_string()
        {
            return Ok(true);
        }
    }
    
    Ok(false)
}

/// Get the address of the output at the specified outpoint
fn get_output_address(
    transaction: &api_rs::models::TxModel,
    outpoint: &TransactionOutpoint,
) -> Result<String> {
    let outputs = transaction
        .outputs
        .as_ref()
        .ok_or_else(|| eyre::eyre!("Validator: Transaction outputs not found"))?;

    let output = outputs
        .get(outpoint.index as usize)
        .ok_or_else(|| eyre::eyre!("Validator: Output index {} not found", outpoint.index))?;

    let address = output
        .script_public_key_address
        .as_ref()
        .ok_or_else(|| eyre::eyre!("Validator: Script public key address not found"))?;

    Ok(address.clone())
}

/// Validate that the progress indication matches the collected data
fn validate_progress_indication(
    fxg: &ConfirmationFXG,
    collected_message_ids: &[MessageID],
) -> Result<bool> {
    // Validate old and new outpoints match cache
    let cache_outpoints = &fxg.cache.outpoints;
    if cache_outpoints.is_empty() {
        return Ok(false);
    }

    let expected_old = &cache_outpoints[0];
    let expected_new = &cache_outpoints[cache_outpoints.len() - 1];

    let progress_indication = &fxg.progress_indication;
    
    // Check old outpoint
    if let Some(ref old_outpoint) = progress_indication.old_outpoint {
        let old_tx_id = KaspaHash::from_bytes(
            old_outpoint
                .transaction_id
                .as_slice()
                .try_into()
                .map_err(|_| eyre::eyre!("Validator: Invalid old outpoint transaction ID length"))?,
        );
        
        if old_tx_id != expected_old.transaction_id || old_outpoint.index != expected_old.index {
            warn!("Validator: Old outpoint mismatch in progress indication");
            return Ok(false);
        }
    } else {
        warn!("Validator: Old outpoint missing in progress indication");
        return Ok(false);
    }

    // Check new outpoint
    if let Some(ref new_outpoint) = progress_indication.new_outpoint {
        let new_tx_id = KaspaHash::from_bytes(
            new_outpoint
                .transaction_id
                .as_slice()
                .try_into()
                .map_err(|_| eyre::eyre!("Validator: Invalid new outpoint transaction ID length"))?,
        );
        
        if new_tx_id != expected_new.transaction_id || new_outpoint.index != expected_new.index {
            warn!("Validator: New outpoint mismatch in progress indication");
            return Ok(false);
        }
    } else {
        warn!("Validator: New outpoint missing in progress indication");
        return Ok(false);
    }

    // Validate processed withdrawals match collected message IDs
    let expected_message_ids: HashSet<String> = collected_message_ids
        .iter()
        .map(|id| hex::encode(id.0.as_bytes()))
        .collect();

    let actual_message_ids: HashSet<String> = progress_indication
        .processed_withdrawals
        .iter()
        .map(|w| w.message_id.clone())
        .collect();

    if expected_message_ids != actual_message_ids {
        warn!(
            "Validator: Message IDs mismatch - expected: {:?}, actual: {:?}",
            expected_message_ids, actual_message_ids
        );
        return Ok(false);
    }

    Ok(true)
}
