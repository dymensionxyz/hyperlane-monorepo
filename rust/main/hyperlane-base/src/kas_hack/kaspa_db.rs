use eyre::Result;
use tracing::debug;

use hyperlane_core::{
    Decode, Encode, HyperlaneDomain, HyperlaneMessage, HyperlaneProtocolError, H256, H512,
};

use crate::db::{DbError, TypedDB, DB};

const MESSAGE_ID: &str = "message_id_";
const MESSAGE: &str = "message_";
const HIGHEST_SEEN_MESSAGE_NONCE: &str = "highest_seen_message_nonce_";
const KASPA_WITHDRAWAL_MESSAGE: &str = "kaspa_withdrawal_message_";
const KASPA_WITHDRAWAL_BLOCK_NUMBER: &str = "kaspa_withdrawal_block_number_";
const KASPA_WITHDRAWAL_KASPA_TX: &str = "kaspa_withdrawal_kaspa_tx_";
const KASPA_DEPOSIT_MESSAGE: &str = "kaspa_deposit_message_";
const KASPA_DEPOSIT_MESSAGE_ID_BY_TX_HASH: &str = "kaspa_deposit_message_id_by_tx_hash_";
const KASPA_DEPOSIT_HUB_TX: &str = "kaspa_deposit_hub_tx_";

/// Rocks DB result type
pub type DbResult<T> = std::result::Result<T, DbError>;

/// DB handle for storing Kaspa-related data.
#[derive(Debug, Clone)]
pub struct KaspaRocksDB(TypedDB);

impl std::ops::Deref for KaspaRocksDB {
    type Target = TypedDB;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<TypedDB> for KaspaRocksDB {
    fn as_ref(&self) -> &TypedDB {
        &self.0
    }
}

impl AsRef<DB> for KaspaRocksDB {
    fn as_ref(&self) -> &DB {
        self.0.as_ref()
    }
}

impl KaspaRocksDB {
    /// Instantiated new `KaspaRocksDB`
    pub fn new(domain: &HyperlaneDomain, db: DB) -> Self {
        Self(TypedDB::new(domain, db))
    }

    /// Store a raw committed message.
    pub fn upsert_message(
        &self,
        message: &HyperlaneMessage,
        // dispatched_block_number: u64,
    ) -> DbResult<()> {
        let id = message.id();

        // - `id` --> `message`
        self.store_message_by_id(&id, message)?;
        // - `nonce` --> `id`
        self.store_message_id_by_nonce(&message.nonce, &id)?;
        // Update the max seen nonce to allow forward-backward iteration in the processor
        self.try_update_max_seen_message_nonce(message.nonce)?;
        Ok(())
    }

    /// Retrieve a message by its nonce
    pub fn retrieve_message_by_nonce(&self, nonce: u32) -> DbResult<Option<HyperlaneMessage>> {
        let id = self.retrieve_message_id_by_nonce(&nonce)?;
        match id {
            None => Ok(None),
            Some(id) => self.retrieve_message_by_id(&id),
        }
    }

    /// Update the nonce of the highest processed message we're aware of
    pub fn try_update_max_seen_message_nonce(&self, nonce: u32) -> DbResult<()> {
        let current_max = self
            .retrieve_highest_seen_message_nonce()?
            .unwrap_or_default();
        if nonce >= current_max {
            self.store_highest_seen_message_nonce_number(&nonce)?;
        }
        Ok(())
    }
}

impl KaspaRocksDB {
    /// Store a value by key
    pub fn store_value_by_key<K: Encode, V: Encode>(
        &self,
        prefix: impl AsRef<[u8]>,
        key: &K,
        value: &V,
    ) -> DbResult<()> {
        self.store_encodable(prefix, key.to_vec(), value)
    }

    /// Retrieve a value by key
    pub fn retrieve_value_by_key<K: Encode, V: Decode>(
        &self,
        prefix: impl AsRef<[u8]>,
        key: &K,
    ) -> DbResult<Option<V>> {
        self.retrieve_decodable(prefix, key.to_vec())
    }

    // Methods used internally
    pub fn retrieve_message_id_by_nonce(&self, nonce: &u32) -> DbResult<Option<H256>> {
        self.retrieve_value_by_key(MESSAGE_ID, nonce)
    }

    pub fn store_message_by_id(&self, id: &H256, message: &HyperlaneMessage) -> DbResult<()> {
        self.store_value_by_key(MESSAGE, id, message)
    }

    pub fn store_message_id_by_nonce(&self, nonce: &u32, id: &H256) -> DbResult<()> {
        self.store_value_by_key(MESSAGE_ID, nonce, id)
    }

    pub fn retrieve_message_by_id(&self, id: &H256) -> DbResult<Option<HyperlaneMessage>> {
        self.retrieve_value_by_key(MESSAGE, id)
    }

    pub fn retrieve_highest_seen_message_nonce(&self) -> DbResult<Option<u32>> {
        self.retrieve_highest_seen_message_nonce_number()
    }

    pub fn store_highest_seen_message_nonce_number(&self, nonce: &u32) -> DbResult<()> {
        // There's no unit struct Encode/Decode impl, so just use `bool` and always use the `Default::default()` key
        self.store_value_by_key(HIGHEST_SEEN_MESSAGE_NONCE, &bool::default(), nonce)
    }

    pub fn retrieve_highest_seen_message_nonce_number(&self) -> DbResult<Option<u32>> {
        // There's no unit struct Encode/Decode impl, so just use `bool` and always use the `Default::default()` key
        self.retrieve_value_by_key(HIGHEST_SEEN_MESSAGE_NONCE, &bool::default())
    }

    /// Store a deposit message indexed by both message_id and tx_hash
    pub fn store_deposit_message(
        &self,
        message: HyperlaneMessage,
        tx_hash: String,
    ) -> DbResult<()> {
        let id = message.id();

        debug!(
            message_id = ?id,
            tx_hash = %tx_hash,
            nonce = message.nonce,
            "Storing Kaspa deposit"
        );

        // Store the message in the general message storage
        self.upsert_message(&message)?;
        // Store deposit message by message_id
        self.store_value_by_key(KASPA_DEPOSIT_MESSAGE, &id, &message)?;
        // Store mapping from tx_hash to message_id for retrieval by tx_hash
        self.store_encodable(KASPA_DEPOSIT_MESSAGE_ID_BY_TX_HASH, tx_hash.as_bytes(), &id)?;

        Ok(())
    }

    /// Retrieve a Kaspa deposit message by message_id
    pub fn retrieve_kaspa_deposit_by_message_id(
        &self,
        message_id: &H256,
    ) -> DbResult<Option<HyperlaneMessage>> {
        self.retrieve_value_by_key(KASPA_DEPOSIT_MESSAGE, message_id)
    }

    /// Retrieve a Kaspa deposit message by kaspa transaction hash
    pub fn retrieve_kaspa_deposit_by_tx_hash(
        &self,
        tx_hash: &str,
    ) -> DbResult<Option<HyperlaneMessage>> {
        // First get the message_id from tx_hash (stored as bytes)
        let message_id: Option<H256> =
            self.retrieve_decodable(KASPA_DEPOSIT_MESSAGE_ID_BY_TX_HASH, tx_hash.as_bytes())?;

        match message_id {
            Some(id) => self.retrieve_kaspa_deposit_by_message_id(&id),
            None => Ok(None),
        }
    }

    /// Store a withdrawal message indexed by message_id
    pub fn store_withdrawal_message(
        &self,
        message: HyperlaneMessage,
    ) -> DbResult<()> {
        let id = message.id();

        debug!(
            message_id = ?id,
            nonce = message.nonce,
            "Storing Kaspa withdrawal"
        );

        // Store the message in the general message storage
        self.upsert_message(&message)?;
        // Store withdrawal message by message_id
        self.store_value_by_key(KASPA_WITHDRAWAL_MESSAGE, &id, &message)?;

        Ok(())
    }

    /// Retrieve a Kaspa withdrawal message by message_id
    pub fn retrieve_kaspa_withdrawal_by_message_id(
        &self,
        message_id: &H256,
    ) -> DbResult<Option<HyperlaneMessage>> {
        self.retrieve_value_by_key(KASPA_WITHDRAWAL_MESSAGE, message_id)
    }

    /// Store Hub transaction ID for a deposit indexed by kaspa_tx
    pub fn store_deposit_hub_tx(&self, kaspa_tx: &str, hub_tx: &H256) -> DbResult<()> {
        debug!(
            kaspa_tx = %kaspa_tx,
            hub_tx = %hub_tx,
            "Storing deposit Hub transaction ID"
        );

        // Store full H256
        self.store_encodable(KASPA_DEPOSIT_HUB_TX, kaspa_tx.as_bytes(), hub_tx)
    }

    /// Retrieve Hub transaction ID for a deposit by kaspa_tx
    pub fn retrieve_deposit_hub_tx(&self, kaspa_tx: &str) -> DbResult<Option<H256>> {
        let hub_tx: Option<H256> =
            self.retrieve_decodable(KASPA_DEPOSIT_HUB_TX, kaspa_tx.as_bytes())?;
        Ok(hub_tx)
    }

    /// Store Kaspa transaction ID for a withdrawal indexed by message_id
    /// Kaspa tx is stored as H256 (64 hex characters)
    pub fn store_withdrawal_kaspa_tx(&self, message_id: &H256, kaspa_tx: &str) -> DbResult<()> {
        debug!(
            message_id = ?message_id,
            kaspa_tx = %kaspa_tx,
            "Storing withdrawal Kaspa transaction ID"
        );
        // Parse kaspa_tx as H256 and store
        let kaspa_tx_h256: H256 = kaspa_tx.parse().map_err(|e| {
            DbError::from(HyperlaneProtocolError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid kaspa_tx format: {}", e),
            )))
        })?;
        self.store_value_by_key(KASPA_WITHDRAWAL_KASPA_TX, message_id, &kaspa_tx_h256)
    }

    /// Retrieve Kaspa transaction ID for a withdrawal by message_id
    pub fn retrieve_withdrawal_kaspa_tx(&self, message_id: &H256) -> DbResult<Option<String>> {
        let kaspa_tx_h256: Option<H256> =
            self.retrieve_value_by_key(KASPA_WITHDRAWAL_KASPA_TX, message_id)?;
        Ok(kaspa_tx_h256.map(|h| format!("{:x}", h)))
    }
}

// Implement the KaspaDb trait from hyperlane-core to allow dymension-kaspa
// to access kaspa_db functionality without creating circular dependencies
impl hyperlane_core::KaspaDb for KaspaRocksDB {
    fn store_withdrawal_message(
        &self,
        message: HyperlaneMessage,
    ) -> Result<()> {
        Ok(self.store_withdrawal_message(message)?)
    }

    fn retrieve_kaspa_withdrawal_by_message_id(
        &self,
        message_id: &H256,
    ) -> Result<Option<HyperlaneMessage>> {
        Ok(self.retrieve_kaspa_withdrawal_by_message_id(message_id)?)
    }

    fn store_deposit_message(&self, message: HyperlaneMessage, tx_hash: String) -> Result<()> {
        Ok(self.store_deposit_message(message, tx_hash)?)
    }

    fn retrieve_kaspa_deposit_by_message_id(
        &self,
        message_id: &H256,
    ) -> Result<Option<HyperlaneMessage>> {
        Ok(self.retrieve_kaspa_deposit_by_message_id(message_id)?)
    }

    fn retrieve_kaspa_deposit_by_tx_hash(&self, tx_hash: &str) -> Result<Option<HyperlaneMessage>> {
        Ok(self.retrieve_kaspa_deposit_by_tx_hash(tx_hash)?)
    }

    fn store_deposit_hub_tx(&self, kaspa_tx: &str, hub_tx: &H256) -> Result<()> {
        Ok(self.store_deposit_hub_tx(kaspa_tx, hub_tx)?)
    }

    fn retrieve_deposit_hub_tx(&self, kaspa_tx: &str) -> Result<Option<H256>> {
        Ok(self.retrieve_deposit_hub_tx(kaspa_tx)?)
    }

    fn store_withdrawal_kaspa_tx(&self, message_id: &H256, kaspa_tx: &str) -> Result<()> {
        Ok(self.store_withdrawal_kaspa_tx(message_id, kaspa_tx)?)
    }

    fn retrieve_withdrawal_kaspa_tx(&self, message_id: &H256) -> Result<Option<String>> {
        Ok(self.retrieve_withdrawal_kaspa_tx(message_id)?)
    }
}
