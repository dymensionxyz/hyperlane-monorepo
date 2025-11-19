/// Main bridge coordinator for Kaspa-Hub bridge operations
use eyre::Result;
use kaspa_core::{
    bridge_types::{BridgeMessage, DepositResult},
    config::{BridgeConnectionConfig, DepositPollConfig, WithdrawalConfig},
    operations::deposit_tracker::DepositTracker,
};
use kaspa_storage::BridgeStorage;
use std::sync::Arc;

use crate::hub_client::HubClient;

/// Main bridge struct that coordinates all bridge operations
pub struct KaspaBridge<S: BridgeStorage, H: HubClient> {
    storage: Arc<S>,
    hub_client: Arc<H>,
    deposit_tracker: DepositTracker,
    connection_config: BridgeConnectionConfig,
    deposit_poll_config: DepositPollConfig,
    withdrawal_config: WithdrawalConfig,
}

impl<S: BridgeStorage, H: HubClient> KaspaBridge<S, H> {
    pub fn new(
        storage: Arc<S>,
        hub_client: Arc<H>,
        connection_config: BridgeConnectionConfig,
        deposit_poll_config: DepositPollConfig,
        withdrawal_config: WithdrawalConfig,
    ) -> Self {
        Self {
            storage,
            hub_client,
            deposit_tracker: DepositTracker::new(),
            connection_config,
            deposit_poll_config,
            withdrawal_config,
        }
    }

    /// Get reference to storage
    pub fn storage(&self) -> &Arc<S> {
        &self.storage
    }

    /// Get reference to hub client
    pub fn hub_client(&self) -> &Arc<H> {
        &self.hub_client
    }

    /// Get reference to connection config
    pub fn connection_config(&self) -> &BridgeConnectionConfig {
        &self.connection_config
    }

    /// Get reference to deposit poll config
    pub fn deposit_poll_config(&self) -> &DepositPollConfig {
        &self.deposit_poll_config
    }

    /// Get reference to withdrawal config
    pub fn withdrawal_config(&self) -> &WithdrawalConfig {
        &self.withdrawal_config
    }

    /// Get mutable reference to deposit tracker
    pub fn deposit_tracker_mut(&mut self) -> &mut DepositTracker {
        &mut self.deposit_tracker
    }

    /// Store a withdrawal message
    pub fn store_withdrawal(&self, message: &BridgeMessage) -> Result<()> {
        let message_id = message.id();
        self.storage.store_withdrawal(&message_id, message)?;
        tracing::info!(
            message_id = hex::encode(message_id),
            "Stored withdrawal message"
        );
        Ok(())
    }

    /// Retrieve a withdrawal message by ID
    pub fn retrieve_withdrawal(&self, message_id: &[u8; 32]) -> Result<Option<BridgeMessage>> {
        self.storage.retrieve_withdrawal(message_id)
    }

    /// Store a deposit result
    pub fn store_deposit(&self, deposit: &DepositResult) -> Result<()> {
        let message_id = deposit.message.id();
        self.storage
            .store_deposit(&message_id, &deposit.tx_hash, deposit)?;
        tracing::info!(
            message_id = hex::encode(message_id),
            tx_hash = deposit.tx_hash,
            "Stored deposit"
        );
        Ok(())
    }

    /// Retrieve deposit by message ID
    pub fn retrieve_deposit_by_message_id(
        &self,
        message_id: &[u8; 32],
    ) -> Result<Option<DepositResult>> {
        self.storage.retrieve_deposit_by_message_id(message_id)
    }

    /// Retrieve deposit by Kaspa transaction hash
    pub fn retrieve_deposit_by_tx_hash(&self, tx_hash: &str) -> Result<Option<DepositResult>> {
        self.storage.retrieve_deposit_by_tx_hash(tx_hash)
    }

    /// Submit deposit to Hub chain
    /// Returns Hub transaction hash
    pub async fn submit_deposit_to_hub(&self, deposit: &DepositResult) -> Result<String> {
        let message_id = deposit.message.id();

        if let Some(existing_hub_tx) = self.storage.retrieve_deposit_hub_tx(&deposit.tx_hash)? {
            tracing::debug!(
                tx_hash = deposit.tx_hash,
                hub_tx = hex::encode(existing_hub_tx),
                "Deposit already submitted to Hub"
            );
            return Ok(hex::encode(existing_hub_tx));
        }

        let hub_tx_hash = self.hub_client.submit_deposit(deposit).await?;
        let hub_tx_bytes = hex::decode(&hub_tx_hash)?;
        if hub_tx_bytes.len() != 32 {
            return Err(eyre::eyre!(
                "Hub transaction hash must be 32 bytes, got {}",
                hub_tx_bytes.len()
            ));
        }

        let hub_tx_array: [u8; 32] = hub_tx_bytes.try_into().unwrap();
        self.storage
            .store_deposit_hub_tx(&deposit.tx_hash, &hub_tx_array)?;
        self.storage
            .update_processed_deposit(&deposit.tx_hash, deposit, &hub_tx_array)?;

        tracing::info!(
            message_id = hex::encode(message_id),
            tx_hash = deposit.tx_hash,
            hub_tx = hub_tx_hash,
            "Submitted deposit to Hub"
        );

        Ok(hub_tx_hash)
    }

    /// Check if withdrawal message has been delivered on Hub
    pub async fn is_withdrawal_delivered(&self, message_id: &[u8; 32]) -> Result<bool> {
        self.hub_client.query_message_delivered(message_id).await
    }

    /// Store Kaspa transaction for a withdrawal
    pub fn store_withdrawal_kaspa_tx(
        &self,
        message_id: &[u8; 32],
        kaspa_tx_id: &str,
    ) -> Result<()> {
        self.storage
            .store_withdrawal_kaspa_tx(message_id, kaspa_tx_id)?;
        tracing::info!(
            message_id = hex::encode(message_id),
            kaspa_tx = kaspa_tx_id,
            "Stored Kaspa transaction for withdrawal"
        );
        Ok(())
    }

    /// Retrieve Kaspa transaction for a withdrawal
    pub fn retrieve_withdrawal_kaspa_tx(&self, message_id: &[u8; 32]) -> Result<Option<String>> {
        self.storage.retrieve_withdrawal_kaspa_tx(message_id)
    }
}
