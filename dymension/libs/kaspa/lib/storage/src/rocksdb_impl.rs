use crate::traits::BridgeStorage;
use eyre::Result;
use kaspa_core::bridge_types::{BridgeMessage, DepositResult};
use rocksdb::{Options, DB};
use std::path::Path;
use std::sync::Arc;
use tracing::debug;

const WITHDRAWAL_MESSAGE: &[u8] = b"withdrawal_message_";
const WITHDRAWAL_KASPA_TX: &[u8] = b"withdrawal_kaspa_tx_";
const DEPOSIT: &[u8] = b"deposit_";
const DEPOSIT_MESSAGE_ID_BY_TX_HASH: &[u8] = b"deposit_message_id_by_tx_hash_";
const DEPOSIT_HUB_TX: &[u8] = b"deposit_hub_tx_";

/// RocksDB implementation of BridgeStorage
#[derive(Debug, Clone)]
pub struct BridgeRocksDB {
    db: Arc<DB>,
}

impl BridgeRocksDB {
    /// Create new BridgeRocksDB instance
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?;
        Ok(Self { db: Arc::new(db) })
    }

    /// Create from existing DB instance
    pub fn from_db(db: Arc<DB>) -> Self {
        Self { db }
    }

    fn make_key(prefix: &[u8], key: &[u8]) -> Vec<u8> {
        let mut full_key = Vec::with_capacity(prefix.len() + key.len());
        full_key.extend_from_slice(prefix);
        full_key.extend_from_slice(key);
        full_key
    }

    fn store<T: serde::Serialize>(&self, prefix: &[u8], key: &[u8], value: &T) -> Result<()> {
        let full_key = Self::make_key(prefix, key);
        let encoded = bincode::serialize(value)?;
        self.db.put(full_key, encoded)?;
        Ok(())
    }

    fn retrieve<T: serde::de::DeserializeOwned>(
        &self,
        prefix: &[u8],
        key: &[u8],
    ) -> Result<Option<T>> {
        let full_key = Self::make_key(prefix, key);
        match self.db.get(full_key)? {
            Some(bytes) => {
                let value = bincode::deserialize(&bytes)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }
}

impl BridgeStorage for BridgeRocksDB {
    fn store_withdrawal(&self, message_id: &[u8; 32], message: &BridgeMessage) -> Result<()> {
        debug!(
            message_id = hex::encode(message_id),
            nonce = message.nonce,
            "Storing withdrawal"
        );
        self.store(WITHDRAWAL_MESSAGE, message_id, message)
    }

    fn retrieve_withdrawal(&self, message_id: &[u8; 32]) -> Result<Option<BridgeMessage>> {
        self.retrieve(WITHDRAWAL_MESSAGE, message_id)
    }

    fn store_deposit(
        &self,
        message_id: &[u8; 32],
        kaspa_tx_id: &str,
        deposit: &DepositResult,
    ) -> Result<()> {
        debug!(
            message_id = hex::encode(message_id),
            kaspa_tx_id = %kaspa_tx_id,
            nonce = deposit.message.nonce,
            "Storing deposit"
        );
        self.store(DEPOSIT, message_id, deposit)?;
        self.store(
            DEPOSIT_MESSAGE_ID_BY_TX_HASH,
            kaspa_tx_id.as_bytes(),
            message_id,
        )?;
        Ok(())
    }

    fn retrieve_deposit_by_message_id(
        &self,
        message_id: &[u8; 32],
    ) -> Result<Option<DepositResult>> {
        self.retrieve(DEPOSIT, message_id)
    }

    fn retrieve_deposit_by_tx_hash(&self, kaspa_tx_id: &str) -> Result<Option<DepositResult>> {
        let message_id: Option<[u8; 32]> =
            self.retrieve(DEPOSIT_MESSAGE_ID_BY_TX_HASH, kaspa_tx_id.as_bytes())?;

        match message_id {
            Some(id) => self.retrieve(DEPOSIT, &id),
            None => Ok(None),
        }
    }

    fn store_deposit_hub_tx(&self, kaspa_tx_id: &str, hub_tx: &[u8; 32]) -> Result<()> {
        debug!(
            kaspa_tx = %kaspa_tx_id,
            hub_tx = hex::encode(hub_tx),
            "Storing deposit Hub transaction ID"
        );
        self.store(DEPOSIT_HUB_TX, kaspa_tx_id.as_bytes(), hub_tx)
    }

    fn retrieve_deposit_hub_tx(&self, kaspa_tx_id: &str) -> Result<Option<[u8; 32]>> {
        self.retrieve(DEPOSIT_HUB_TX, kaspa_tx_id.as_bytes())
    }

    fn store_withdrawal_kaspa_tx(&self, message_id: &[u8; 32], kaspa_tx: &str) -> Result<()> {
        debug!(
            message_id = hex::encode(message_id),
            kaspa_tx = %kaspa_tx,
            "Storing withdrawal Kaspa transaction ID"
        );
        self.store(WITHDRAWAL_KASPA_TX, message_id, &kaspa_tx.to_string())
    }

    fn retrieve_withdrawal_kaspa_tx(&self, message_id: &[u8; 32]) -> Result<Option<String>> {
        self.retrieve(WITHDRAWAL_KASPA_TX, message_id)
    }

    fn update_processed_deposit(
        &self,
        kaspa_tx_id: &str,
        deposit: &DepositResult,
        hub_tx: &[u8; 32],
    ) -> Result<()> {
        let new_message_id = deposit.message.id();
        debug!(
            new_message_id = hex::encode(new_message_id),
            kaspa_tx_id = %kaspa_tx_id,
            hub_tx = hex::encode(hub_tx),
            nonce = deposit.message.nonce,
            "Updating deposit with new message and hub_tx"
        );

        self.store(DEPOSIT, &new_message_id, deposit)?;
        self.store(
            DEPOSIT_MESSAGE_ID_BY_TX_HASH,
            kaspa_tx_id.as_bytes(),
            &new_message_id,
        )?;
        self.store(DEPOSIT_HUB_TX, kaspa_tx_id.as_bytes(), hub_tx)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaspa_core::bridge_types::BridgeMessage;

    #[test]
    fn test_storage_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = BridgeRocksDB::new(temp_dir.path()).unwrap();

        let message = BridgeMessage::new(1, 100, 1, [1u8; 32], 2, [2u8; 32], vec![1, 2, 3, 4]);

        let message_id = message.id();

        storage.store_withdrawal(&message_id, &message).unwrap();

        let retrieved = storage.retrieve_withdrawal(&message_id).unwrap();
        assert_eq!(retrieved, Some(message));
    }
}
