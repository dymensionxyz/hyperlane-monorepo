use std::sync::Arc;
use eyre::Result;
use hex::ToHex;
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::{WithdrawalId, WithdrawalStatus};
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use kaspa_wallet_core::prelude::*;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_core::info;
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::TransactionOutpoint;
use kaspa_wallet_core::error::Error;
use hyperlane_core::{HyperlaneMessage, H256};

pub async fn check_balance<T: RpcApi + ?Sized>(
    source: &str,
    rpc: &T,
    addr: &Address,
) -> Result<u64, Error> {
    let balance = rpc
        .get_balance_by_address(addr.clone())
        .await
        .map_err(|e| Error::Custom(format!("Getting balance for address: {}", e)))?;

    info!("{} balance: {}", source, balance);
    Ok(balance)
}

// TODO: needed?
pub async fn check_balance_wallet(w: Arc<Wallet>) -> Result<(), Error> {
    let a = w.account()?;
    for _ in 0..10 {
        if a.balance().is_some() {
            break;
        }
        workflow_core::task::sleep(std::time::Duration::from_millis(200)).await;
    }

    if let Some(b) = a.balance() {
        info!("Wallet account balance:");
        info!("  Mature:   {} KAS", sompi_to_kaspa_string(b.mature));
        info!("  Pending:  {} KAS", sompi_to_kaspa_string(b.pending));
        info!("  Outgoing: {} KAS", sompi_to_kaspa_string(b.outgoing));
    } else {
        info!("Wallet account has no balance or is still syncing.");
    }

    Ok(())
}

pub fn get_recipient_address(recipient: H256, prefix: Prefix) -> Address {
    Address::new(
        prefix,
        Version::PubKey, // should always be PubKey
        recipient.as_bytes(),
    )
}

pub async fn filter_pending_withdrawals(
    withdrawals: Vec<HyperlaneMessage>,
    cosmos: &CosmosGrpcClient,
    height: Option<u32>,
) -> Result<(TransactionOutpoint, Vec<HyperlaneMessage>)> {
    // A list of withdrawal IDs to request their statuses from the Hub
    let withdrawal_ids: Vec<_> = withdrawals
        .iter()
        .map(|m| WithdrawalId {
            message_id: m.id().encode_hex(),
        })
        .collect();

    // Request withdrawal statuses from the Hub
    let resp = cosmos
        .withdrawal_status(withdrawal_ids, height)
        .await
        .map_err(|e| eyre::eyre!("Query outpoint from x/kas: {}", e))?;

    let outpoint_data = resp
        .outpoint
        .ok_or_else(|| eyre::eyre!("No outpoint data in response"))?;

    if outpoint_data.transaction_id.len() != 32 {
        return Err(eyre::eyre!(
            "Invalid transaction ID length: expected 32 bytes, got {}",
            outpoint_data.transaction_id.len()
        ));
    }

    // Convert the transaction ID to kaspa transaction ID
    let kaspa_tx_id = kaspa_hashes::Hash::from_bytes(
        outpoint_data
            .transaction_id
            .as_slice()
            .try_into()
            .map_err(|e| eyre::eyre!("Convert tx ID to Kaspa tx ID: {:}", e))?,
    );

    // resp.status is a list of the same length as withdrawals. If status == WithdrawalStatus::Unprocessed,
    // then the respective element of withdrawals is Unprocessed.
    let pending_withdrawals: Vec<_> = resp
        .status
        .into_iter()
        .enumerate()
        .filter_map(|(idx, status)| match status.try_into() {
            Ok(WithdrawalStatus::Unprocessed) => Some(withdrawals[idx].clone()),
            _ => None, // Ignore other statuses
        })
        .collect();

    Ok((
        TransactionOutpoint {
            transaction_id: kaspa_tx_id,
            index: outpoint_data.index,
        },
        pending_withdrawals,
    ))
}
