/// Implementation of HubClient trait for Cosmos/Dymension Hub
use async_trait::async_trait;
use dym_kas_core::bridge_types::{convert, DepositResult};
use eyre::Result;
use hyperlane_core::{ChainCommunicationError, Mailbox};
use hyperlane_cosmos::{native::ModuleQueryClient, CosmosProvider};
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::{WithdrawalId, WithdrawalStatus};
use kaspa_bridge::HubClient;
use std::sync::Arc;

/// Hub client implementation using Cosmos provider and mailbox
pub struct CosmosHubClient<M: Mailbox> {
    hub_mailbox: Arc<M>,
    cosmos_rpc: CosmosProvider<ModuleQueryClient>,
}

impl<M: Mailbox> CosmosHubClient<M> {
    pub fn new(hub_mailbox: Arc<M>, cosmos_rpc: CosmosProvider<ModuleQueryClient>) -> Self {
        Self {
            hub_mailbox,
            cosmos_rpc,
        }
    }
}

#[async_trait]
impl<M: Mailbox + Send + Sync> HubClient for CosmosHubClient<M> {
    async fn query_message_delivered(&self, message_id: &[u8; 32]) -> Result<bool> {
        let message_id_hex = format!("0x{}", hex::encode(message_id));
        let wid = WithdrawalId {
            message_id: message_id_hex,
        };

        let res = self
            .cosmos_rpc
            .query()
            .withdrawal_status(vec![wid], None)
            .await
            .map_err(|e| eyre::eyre!("Query withdrawal status: {}", e))?;

        match res
            .status
            .first()
            .and_then(|s| WithdrawalStatus::try_from(*s).ok())
        {
            Some(WithdrawalStatus::Processed) => Ok(true),
            _ => Ok(false),
        }
    }

    async fn submit_deposit(&self, deposit: &DepositResult) -> Result<String> {
        let hl_message = convert::bridge_to_hyperlane(&deposit.message);

        let metadata = vec![];

        let outcome = self
            .hub_mailbox
            .process(&hl_message, &metadata, None)
            .await
            .map_err(|e: ChainCommunicationError| eyre::eyre!("Submit deposit to Hub: {}", e))?;

        if !outcome.executed {
            return Err(eyre::eyre!(
                "Deposit transaction was not executed on Hub: gas_used={}",
                outcome.gas_used
            ));
        }

        let hub_tx_hash = hyperlane_cosmos::native::h512_to_h256(outcome.transaction_id);
        Ok(format!("{:x}", hub_tx_hash))
    }
}
