use eyre::Result;
use kaspa_core::bridge_types::{BridgeMessage, DepositResult};
use std::fmt::Debug;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<rocksdb::Error> for StorageError {
    fn from(e: rocksdb::Error) -> Self {
        StorageError::Database(e.to_string())
    }
}

impl From<bincode::Error> for StorageError {
    fn from(e: bincode::Error) -> Self {
        StorageError::Serialization(e.to_string())
    }
}

impl From<eyre::Report> for StorageError {
    fn from(e: eyre::Report) -> Self {
        StorageError::Other(e.to_string())
    }
}

/// Bridge-agnostic storage trait for Kaspa bridge data
/// This trait has NO dependencies on Hyperlane types
pub trait BridgeStorage: Send + Sync + Debug {
    /// Store a withdrawal message indexed by message_id
    fn store_withdrawal(&self, message_id: &[u8; 32], message: &BridgeMessage) -> Result<()>;

    /// Retrieve a withdrawal message by message_id
    fn retrieve_withdrawal(&self, message_id: &[u8; 32]) -> Result<Option<BridgeMessage>>;

    /// Store a deposit indexed by both message_id and kaspa tx_hash
    fn store_deposit(
        &self,
        message_id: &[u8; 32],
        kaspa_tx_id: &str,
        deposit: &DepositResult,
    ) -> Result<()>;

    /// Retrieve a deposit by message_id
    fn retrieve_deposit_by_message_id(
        &self,
        message_id: &[u8; 32],
    ) -> Result<Option<DepositResult>>;

    /// Retrieve a deposit by kaspa transaction hash
    fn retrieve_deposit_by_tx_hash(&self, kaspa_tx_id: &str) -> Result<Option<DepositResult>>;

    /// Store Hub transaction ID for a deposit indexed by kaspa_tx
    fn store_deposit_hub_tx(&self, kaspa_tx_id: &str, hub_tx: &[u8; 32]) -> Result<()>;

    /// Retrieve Hub transaction ID for a deposit by kaspa_tx
    fn retrieve_deposit_hub_tx(&self, kaspa_tx_id: &str) -> Result<Option<[u8; 32]>>;

    /// Store Kaspa transaction ID for a withdrawal indexed by message_id
    fn store_withdrawal_kaspa_tx(&self, message_id: &[u8; 32], kaspa_tx: &str) -> Result<()>;

    /// Retrieve Kaspa transaction ID for a withdrawal by message_id
    fn retrieve_withdrawal_kaspa_tx(&self, message_id: &[u8; 32]) -> Result<Option<String>>;

    /// Update a deposit after successful submission to Hub
    fn update_processed_deposit(
        &self,
        kaspa_tx_id: &str,
        deposit: &DepositResult,
        hub_tx: &[u8; 32],
    ) -> Result<()>;
}
