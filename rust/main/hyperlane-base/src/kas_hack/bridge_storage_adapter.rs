/// Adapter to make KaspaRocksDB implement BridgeStorage trait
use dym_kas_core::bridge_types::{convert, BridgeMessage, DepositResult};
use eyre::Result;
use hyperlane_core::{KaspaDb, H256};
use kaspa_storage::BridgeStorage;
use std::fmt::Debug;

use super::kaspa_db::KaspaRocksDB;

/// Adapter that allows KaspaRocksDB to be used as BridgeStorage
/// This converts between Hyperlane types (used by KaspaRocksDB) and bridge-agnostic types
#[derive(Clone)]
pub struct KaspaBridgeStorageAdapter {
    db: KaspaRocksDB,
}

impl Debug for KaspaBridgeStorageAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KaspaBridgeStorageAdapter")
            .field("db", &"KaspaRocksDB")
            .finish()
    }
}

impl KaspaBridgeStorageAdapter {
    pub fn new(db: KaspaRocksDB) -> Self {
        Self { db }
    }
}

impl BridgeStorage for KaspaBridgeStorageAdapter {
    fn store_withdrawal(&self, message_id: &[u8; 32], message: &BridgeMessage) -> Result<()> {
        let hl_message = convert::bridge_to_hyperlane(message);
        let h256_id = H256::from(*message_id);

        let stored_id = hl_message.id();
        if stored_id != h256_id {
            return Err(eyre::eyre!(
                "Message ID mismatch: expected {:?}, got {:?}",
                h256_id,
                stored_id
            ));
        }

        self.db.store_withdrawal_message(hl_message)?;
        Ok(())
    }

    fn retrieve_withdrawal(&self, message_id: &[u8; 32]) -> Result<Option<BridgeMessage>> {
        let h256_id = H256::from(*message_id);
        match self.db.retrieve_kaspa_withdrawal_by_message_id(&h256_id)? {
            Some(hl_message) => Ok(Some(convert::hyperlane_to_bridge(&hl_message))),
            None => Ok(None),
        }
    }

    fn store_deposit(
        &self,
        message_id: &[u8; 32],
        kaspa_tx_id: &str,
        deposit: &DepositResult,
    ) -> Result<()> {
        let hl_message = convert::bridge_to_hyperlane(&deposit.message);
        let h256_id = H256::from(*message_id);

        let stored_id = hl_message.id();
        if stored_id != h256_id {
            return Err(eyre::eyre!(
                "Message ID mismatch: expected {:?}, got {:?}",
                h256_id,
                stored_id
            ));
        }

        self.db
            .store_deposit_message(hl_message, kaspa_tx_id.to_string())?;
        Ok(())
    }

    fn retrieve_deposit_by_message_id(
        &self,
        message_id: &[u8; 32],
    ) -> Result<Option<DepositResult>> {
        let h256_id = H256::from(*message_id);
        match self.db.retrieve_kaspa_deposit_by_message_id(&h256_id)? {
            Some(hl_message) => {
                let bridge_msg = convert::hyperlane_to_bridge(&hl_message);
                Ok(Some(DepositResult {
                    tx_hash: String::new(),
                    utxo_index: 0,
                    amount: 0,
                    accepting_block_hash: String::new(),
                    containing_block_hash: String::new(),
                    message: bridge_msg,
                    confirmation_count: 0,
                }))
            }
            None => Ok(None),
        }
    }

    fn retrieve_deposit_by_tx_hash(&self, kaspa_tx_id: &str) -> Result<Option<DepositResult>> {
        match self.db.retrieve_kaspa_deposit_by_tx_hash(kaspa_tx_id)? {
            Some(hl_message) => {
                let bridge_msg = convert::hyperlane_to_bridge(&hl_message);
                Ok(Some(DepositResult {
                    tx_hash: kaspa_tx_id.to_string(),
                    utxo_index: 0,
                    amount: 0,
                    accepting_block_hash: String::new(),
                    containing_block_hash: String::new(),
                    message: bridge_msg,
                    confirmation_count: 0,
                }))
            }
            None => Ok(None),
        }
    }

    fn store_deposit_hub_tx(&self, kaspa_tx: &str, hub_tx: &[u8; 32]) -> Result<()> {
        let h256_hub_tx = H256::from(*hub_tx);
        self.db.store_deposit_hub_tx(kaspa_tx, &h256_hub_tx)?;
        Ok(())
    }

    fn retrieve_deposit_hub_tx(&self, kaspa_tx_id: &str) -> Result<Option<[u8; 32]>> {
        match self.db.retrieve_deposit_hub_tx(kaspa_tx_id)? {
            Some(h256) => Ok(Some(h256.into())),
            None => Ok(None),
        }
    }

    fn store_withdrawal_kaspa_tx(&self, message_id: &[u8; 32], kaspa_tx: &str) -> Result<()> {
        let h256_id = H256::from(*message_id);
        self.db.store_withdrawal_kaspa_tx(&h256_id, kaspa_tx)?;
        Ok(())
    }

    fn retrieve_withdrawal_kaspa_tx(&self, message_id: &[u8; 32]) -> Result<Option<String>> {
        let h256_id = H256::from(*message_id);
        self.db.retrieve_withdrawal_kaspa_tx(&h256_id)
    }

    fn update_processed_deposit(
        &self,
        kaspa_tx_id: &str,
        _deposit: &DepositResult,
        hub_tx: &[u8; 32],
    ) -> Result<()> {
        let h256_hub_tx = H256::from(*hub_tx);
        self.db.store_deposit_hub_tx(kaspa_tx_id, &h256_hub_tx)?;
        Ok(())
    }
}
