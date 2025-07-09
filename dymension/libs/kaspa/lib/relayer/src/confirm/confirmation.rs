use eyre::Result;
use tracing::info;

use api_rs::models::TxModel;
use kaspa_consensus_core::tx::TransactionOutpoint;

use kaspa_wallet_core::error::Error;

use corelib::api::client::HttpClient;

use corelib::{confirmation::ConfirmationFXG, payload::MessageID};
use hex;

/// WARNING: ONLY FOR UNHAPPY PATH
/// /// Prepare a progress indication and create a ConfirmationFXG for the Hub x/kas module
/// This function traces back from a new UTXO to the old UTXO and collects
/// all withdrawal payloads that were processed in between.
///
/// # Arguments
/// * `config` - The Kaspa API client configuration for querying transactions
/// * `anchor_utxo` - The anchor UTXO to trace to
/// * `new_utxo` - The new UTXO to trace from
///
/// # Returns
/// * `Result<ConfirmationFXG, Error>` - The confirmation FXG containing the progress indication with old and new outpoints
///   and a list of processed withdrawal ID
/// Trace transactions in reverse, from a recent unspent UTXO to an already spent UTXO
/// collecting payloads along the way.
/// Follows the transaction lineage of the escrow address.
///
/// # Arguments
/// * `config` - The Kaspa API client configuration for querying transactions
/// * `new_utxo` - The transaction ID to start tracing from
/// * `current_anchor_utxo` - The transaction ID to trace to
///
/// # Returns
/// * `Result<Vec<WithdrawalId>, Error>` - Vector of collected withdrawal IDs from the transactions
pub async fn expensive_trace_transactions(
    client: &HttpClient,
    escrow_addresses: &str,
    new_out: TransactionOutpoint,
    old_out: TransactionOutpoint,
) -> Result<ConfirmationFXG> {
    info!(
        "Starting transaction trace from candidate new anchor {:?} to old anchor {:?}",
        new_out, old_out
    );

    
    let mut processed_withdrawals: Vec<MessageID> = Vec::new();
    let mut lineage_utxos = Vec::new();
    
    // get the lineage utxos

    let res = recursive_trace_transactions(client, escrow_addresses, new_out, old_out, &mut lineage_utxos, &mut processed_withdrawals).await;
    if res.is_err() {
        return Err(eyre::eyre!("Failed to trace transactions: {}", res.err().unwrap()));
    }
    
    info!(
        "Trace completed. Found {} UTXOs in lineage with {} processed withdrawals",
        lineage_utxos.len(),
        processed_withdrawals.len()
    );

    Ok(ConfirmationFXG::from_msgs_outpoints(
        processed_withdrawals,
        lineage_utxos,
    ))
}

pub async fn recursive_trace_transactions(
    client: &HttpClient,
    escrow_addresses: &str,
    curr_utxo: TransactionOutpoint,
    anchor_utxo: TransactionOutpoint,
    lineage_utxos: &mut Vec<TransactionOutpoint>,
    processed_withdrawals: &mut Vec<MessageID>,
) -> Result<()> {
    // if curr_utxo is the anchor_utxo, return
    // this will wrap up the recursive call
    if curr_utxo == anchor_utxo {
        return Ok(());
    }

    info!("Tracing lineage from UTXO: {:?}", curr_utxo);

    // get the transaction
    let transaction = client
        .get_tx_by_id(&curr_utxo.transaction_id.to_string())
        .await?;

    // get the inputs of the current transaction
    let inputs = transaction
        .inputs
        .as_ref()
        .ok_or(Error::Custom("Inputs not found".to_string()))?;

    // follow inputs
    for input in inputs {
        info!("Checking input: {:?}", input.index);

        // if the input has my address, do recursive call
        let input_address = input
            .previous_outpoint_address
            .as_ref()
            .ok_or(Error::Custom("Input address not found".to_string()))?;

        // skip input if not my address
        if input_address != escrow_addresses {
            info!("Skipping input from non-escrow address: {:?}", input_address);
            continue;
        }


        // FIXME: we have wrapper for it?
        let input_utxo = TransactionOutpoint {
            transaction_id: kaspa_hashes::Hash::from_bytes(
                hex::decode(&input.previous_outpoint_hash)?.try_into().map_err(|_| {
                    eyre::eyre!("Invalid hex in previous_outpoint_hash")
                })?
            ),
            index: input.previous_outpoint_index.parse()?,
        };
        // do recursive call
        let res = Box::pin(recursive_trace_transactions(client, escrow_addresses, input_utxo, anchor_utxo, lineage_utxos, processed_withdrawals)).await;

        // if returns error, continue to other input
        if res.is_err() {
            continue;
        }

        // if returns OK, add the input to the lineage_UTXOs and return


        // FIXME: parse message IDs from the transaction
        /*
        // Parse the payload string to extract the message ID
        if let Some(payload) = transaction.payload.clone() {
            let unhexed_payload = hex::decode(&payload)
                .map_err(|e| eyre::eyre!("Failed to decode payload: {}", e))?;
            // Deserialize the payload bytes into MessageIDs
            let message_ids =
                corelib::payload::MessageIDs::from_bytes(&unhexed_payload).map_err(|e| {
                    eyre::eyre!(
                        "Failed to deserialize MessageIDs: Payload: {} Err: {}",
                        payload,
                        e
                    )
                })?;

            // Convert each message ID into a WithdrawalId and add to the list
            processed_withdrawals.extend(message_ids.0);
        } else {
            return Err(eyre::eyre!("No payload found in transaction"));
        }
         */


        lineage_utxos.push(input_utxo);
        return Ok(())
    }

    // if reached here, return error as we're not followed the lineage
    Err(eyre::eyre!("No lineage UTXOs found in transaction inputs"))
}
