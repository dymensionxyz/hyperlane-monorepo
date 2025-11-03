use std::ops::Add;

use async_trait::async_trait;
use eyre::{bail, Result};
use tracing::{debug, instrument, trace};

use hyperlane_core::{
    identifiers::UniqueIdentifier, Decode, Encode, GasPaymentKey, HyperlaneDomain,
    HyperlaneLogStore, HyperlaneMessage, HyperlaneProtocolError, HyperlaneSequenceAwareIndexerStoreReader,
    HyperlaneWatermarkedLogStore, Indexed, InterchainGasExpenditure, InterchainGasPayment,
    InterchainGasPaymentMeta, LogMeta, MerkleTreeInsertion, PendingOperationStatus, H256, H512,
};

use crate::db::{
    storage_types::{InterchainGasExpenditureData, InterchainGasPaymentData},
    DbError, HyperlaneDb, TypedDB, DB,
};

// these keys MUST not be given multiple uses in case multiple agents are
// started with the same database and domain.

const MESSAGE_ID: &str = "message_id_";
const MESSAGE_DISPATCHED_BLOCK_NUMBER: &str = "message_dispatched_block_number_";
const MESSAGE: &str = "message_";
const NONCE_PROCESSED: &str = "nonce_processed_";
const GAS_PAYMENT_BY_SEQUENCE: &str = "gas_payment_by_sequence_";
const GAS_PAYMENT_BLOCK_BY_SEQUENCE: &str = "gas_payment_block_by_sequence_";
const HIGHEST_SEEN_MESSAGE_NONCE: &str = "highest_seen_message_nonce_";
const GAS_PAYMENT_FOR_MESSAGE_ID: &str = "gas_payment_sequence_for_message_id_v2_";
const GAS_PAYMENT_META_PROCESSED: &str = "gas_payment_meta_processed_v3_";
const GAS_EXPENDITURE_FOR_MESSAGE_ID: &str = "gas_expenditure_for_message_id_v2_";
const STATUS_BY_MESSAGE_ID: &str = "status_by_message_id_";
const PENDING_MESSAGE_RETRY_COUNT_FOR_MESSAGE_ID: &str =
    "pending_message_retry_count_for_message_id_";
const MERKLE_TREE_INSERTION: &str = "merkle_tree_insertion_";
const MERKLE_LEAF_INDEX_BY_MESSAGE_ID: &str = "merkle_leaf_index_by_message_id_";
const MERKLE_TREE_INSERTION_BLOCK_NUMBER_BY_LEAF_INDEX: &str =
    "merkle_tree_insertion_block_number_by_leaf_index_";
const LATEST_INDEXED_GAS_PAYMENT_BLOCK: &str = "latest_indexed_gas_payment_block";
const PAYLOAD_UUIDS_BY_MESSAGE_ID: &str = "payload_uuids_by_message_id_";

// Separate storage for Kaspa deposits and withdrawals
// Withdrawals are indexed by message_id only
const KASPA_WITHDRAWAL_MESSAGE: &str = "kaspa_withdrawal_message_";
const KASPA_WITHDRAWAL_BLOCK_NUMBER: &str = "kaspa_withdrawal_block_number_";
const KASPA_WITHDRAWAL_KASPA_TX: &str = "kaspa_withdrawal_kaspa_tx_";

// Deposits are indexed by both message_id and tx_hash
const KASPA_DEPOSIT_MESSAGE: &str = "kaspa_deposit_message_";
const KASPA_DEPOSIT_MESSAGE_ID_BY_TX_HASH: &str = "kaspa_deposit_message_id_by_tx_hash_";
//const KASPA_DEPOSIT_STATUS: &str = "kaspa_deposit_status_";
const KASPA_DEPOSIT_HUB_TX: &str = "kaspa_deposit_hub_tx_";

/// Rocks DB result type
pub type DbResult<T> = std::result::Result<T, DbError>;

/// DB handle for storing Kaspa-related data tied to a specific Mailbox.
#[derive(Debug, Clone)]
pub struct KaspaRocksDB(HyperlaneDomain, TypedDB);

impl std::ops::Deref for KaspaRocksDB {
    type Target = TypedDB;

    fn deref(&self) -> &Self::Target {
        &self.1
    }
}

impl AsRef<TypedDB> for KaspaRocksDB {
    fn as_ref(&self) -> &TypedDB {
        &self.1
    }
}

impl AsRef<DB> for KaspaRocksDB {
    fn as_ref(&self) -> &DB {
        self.1.as_ref()
    }
}

impl KaspaRocksDB {
    /// Instantiated new `KaspaRocksDB`
    pub fn new(domain: &HyperlaneDomain, db: DB) -> Self {
        Self(domain.clone(), TypedDB::new(domain, db))
    }

    /// Get the domain this database is scoped to
    pub fn domain(&self) -> &HyperlaneDomain {
        &self.0
    }

    /// Store a raw committed message. If message already exists, then do nothing.
    ///
    /// Keys --> Values:
    /// - `nonce` --> `id`
    /// - `id` --> `message`
    /// - `nonce` --> `dispatched block number`
    pub fn store_message(
        &self,
        message: &HyperlaneMessage,
        dispatched_block_number: u64,
    ) -> DbResult<bool> {
        if let Ok(Some(_)) = self.retrieve_message_id_by_nonce(&message.nonce) {
            trace!(hyp_message=?message, "Message already stored in db");
            return Ok(false);
        }
        self.upsert_message(message, dispatched_block_number)?;
        Ok(true)
    }

    /// Store a raw committed message.
    ///
    /// Keys --> Values:
    /// - `nonce` --> `id`
    /// - `id` --> `message`
    /// - `nonce` --> `dispatched block number`
    pub fn upsert_message(
        &self,
        message: &HyperlaneMessage,
        dispatched_block_number: u64,
    ) -> DbResult<()> {
        let id = message.id();

        // - `id` --> `message`
        self.store_message_by_id(&id, message)?;
        // - `nonce` --> `id`
        self.store_message_id_by_nonce(&message.nonce, &id)?;
        // Update the max seen nonce to allow forward-backward iteration in the processor
        self.try_update_max_seen_message_nonce(message.nonce)?;
        // - `nonce` --> `dispatched block number`
        self.store_dispatched_block_number_by_nonce(&message.nonce, &dispatched_block_number)?;
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

    /// If the provided gas payment, identified by its metadata, has not been
    /// processed, processes the gas payment and records it as processed.
    /// Returns whether the gas payment was processed for the first time.
    pub fn process_indexed_gas_payment(
        &self,
        indexed_payment: Indexed<InterchainGasPayment>,
        log_meta: &LogMeta,
    ) -> DbResult<bool> {
        let payment = *(indexed_payment.inner());
        let gas_processing_successful = self.process_gas_payment(payment, log_meta)?;

        // only store the payment and return early if there's no sequence
        let Some(gas_payment_sequence) = indexed_payment.sequence else {
            return Ok(gas_processing_successful);
        };
        // otherwise store the indexing decorator as well
        if let Ok(Some(_)) = self.retrieve_gas_payment_by_sequence(&gas_payment_sequence) {
            trace!(
                ?indexed_payment,
                ?log_meta,
                "Attempted to process an already-processed indexed gas payment"
            );
            // Return false to indicate the gas payment was already processed
            return Ok(false);
        }

        self.store_gas_payment_by_sequence(&gas_payment_sequence, indexed_payment.inner())?;
        self.store_gas_payment_block_by_sequence(&gas_payment_sequence, &log_meta.block_number)?;

        Ok(gas_processing_successful)
    }

    /// If the provided gas payment, identified by its metadata, has not been
    /// processed, processes the gas payment and records it as processed.
    /// Returns whether the gas payment was processed for the first time.
    pub fn process_gas_payment(
        &self,
        payment: InterchainGasPayment,
        log_meta: &LogMeta,
    ) -> DbResult<bool> {
        let payment_meta = log_meta.into();
        // If the gas payment has already been processed, do nothing
        if self
            .retrieve_processed_by_gas_payment_meta(&payment_meta)?
            .unwrap_or(false)
        {
            trace!(
                ?payment,
                ?log_meta,
                "Attempted to process an already-processed gas payment"
            );
            // Return false to indicate the gas payment was already processed
            return Ok(false);
        }
        // Set the gas payment as processed
        self.store_processed_by_gas_payment_meta(&payment_meta, &true)?;

        // Update the total gas payment for the message to include the payment
        self.update_gas_payment_by_gas_payment_key(payment)?;

        // Return true to indicate the gas payment was processed for the first time
        Ok(true)
    }

    /// Store the merkle tree insertion event, and also store a mapping from message_id to leaf_index
    pub fn process_tree_insertion(
        &self,
        insertion: &MerkleTreeInsertion,
        insertion_block_number: u64,
    ) -> DbResult<bool> {
        if let Ok(Some(_)) = self.retrieve_merkle_tree_insertion_by_leaf_index(&insertion.index()) {
            debug!(insertion=?insertion, "Tree insertion already stored in db");
            return Ok(false);
        }
        self.store_tree_insertion(insertion, insertion_block_number)
    }

    /// Store the merkle tree insertion event, and also store a mapping from message_id to leaf_index.
    /// Overwrites existing insertions
    pub fn store_tree_insertion(
        &self,
        insertion: &MerkleTreeInsertion,
        insertion_block_number: u64,
    ) -> DbResult<bool> {
        // even if double insertions are ok, store the leaf by `leaf_index` (guaranteed to be unique)
        // rather than by `message_id` (not guaranteed to be recurring), so that leaves can be retrieved
        // based on insertion order.
        self.store_merkle_tree_insertion_by_leaf_index(&insertion.index(), insertion)?;

        self.store_merkle_leaf_index_by_message_id(&insertion.message_id(), &insertion.index())?;

        self.store_merkle_tree_insertion_block_number_by_leaf_index(
            &insertion.index(),
            &insertion_block_number,
        )?;
        // Return true to indicate the tree insertion was processed
        Ok(true)
    }

    /// Processes the gas expenditure and store the total expenditure for the
    /// message.
    pub fn process_gas_expenditure(&self, expenditure: InterchainGasExpenditure) -> DbResult<()> {
        // Update the total gas expenditure for the message to include the payment
        self.update_gas_expenditure_by_message_id(expenditure)
    }

    /// Update the total gas payment for a message to include gas_payment
    fn update_gas_payment_by_gas_payment_key(&self, event: InterchainGasPayment) -> DbResult<()> {
        let gas_payment_key = event.into();
        let existing_payment =
            match self.retrieve_gas_payment_by_gas_payment_key(gas_payment_key)? {
                Some(payment) => payment,
                None => InterchainGasPayment::from_gas_payment_key(gas_payment_key),
            };
        let total = existing_payment.add(event);

        debug!(?event, new_total_gas_payment=?total, "Storing gas payment");
        self.store_interchain_gas_payment_data_by_gas_payment_key(&gas_payment_key, &total.into())?;

        Ok(())
    }

    /// Update the total gas spent for a message
    fn update_gas_expenditure_by_message_id(
        &self,
        event: InterchainGasExpenditure,
    ) -> DbResult<()> {
        let existing_expenditure = self.retrieve_gas_expenditure_by_message_id(event.message_id)?;
        let total = existing_expenditure.add(event);

        debug!(?event, new_total_gas_expenditure=?total, "Storing gas expenditure");
        self.store_interchain_gas_expenditure_data_by_message_id(
            &total.message_id,
            &InterchainGasExpenditureData {
                tokens_used: total.tokens_used,
                gas_used: total.gas_used,
            },
        )?;
        Ok(())
    }

    /// Retrieve the total gas payment for a message
    pub fn retrieve_gas_payment_by_gas_payment_key(
        &self,
        gas_payment_key: GasPaymentKey,
    ) -> DbResult<Option<InterchainGasPayment>> {
        Ok(self
            .retrieve_interchain_gas_payment_data_by_gas_payment_key(&gas_payment_key)?
            .map(|payment| {
                payment.complete(gas_payment_key.message_id, gas_payment_key.destination)
            }))
    }

    /// Retrieve the total gas payment for a message
    pub fn retrieve_gas_expenditure_by_message_id(
        &self,
        message_id: H256,
    ) -> DbResult<InterchainGasExpenditure> {
        Ok(self
            .retrieve_interchain_gas_expenditure_data_by_message_id(&message_id)?
            .unwrap_or_default()
            .complete(message_id))
    }
}

#[async_trait]
impl HyperlaneLogStore<HyperlaneMessage> for KaspaRocksDB {
    /// Store a list of dispatched messages and their associated metadata.
    #[instrument(skip_all)]
    async fn store_logs(&self, messages: &[(Indexed<HyperlaneMessage>, LogMeta)]) -> Result<u32> {
        let mut stored: u32 = 0;
        for (message, meta) in messages {
            let stored_message = self.store_message(message.inner(), meta.block_number)?;
            if stored_message {
                stored = stored.saturating_add(1);
            }
        }
        if stored > 0 {
            debug!(messages = stored, "Wrote new messages to database");
        }
        Ok(stored)
    }
}

async fn store_and_count_new<T: Copy>(
    store: &KaspaRocksDB,
    logs: &[(T, LogMeta)],
    log_type: &str,
    process: impl Fn(&KaspaRocksDB, T, &LogMeta) -> DbResult<bool>,
) -> Result<u32> {
    let mut new_logs: u32 = 0;
    for (log, meta) in logs {
        if process(store, *log, meta)? {
            new_logs = new_logs.saturating_add(1);
        }
    }
    if new_logs > 0 {
        debug!(new_logs, log_type, "Wrote new logs to database");
    }
    Ok(new_logs)
}

#[async_trait]
impl HyperlaneLogStore<InterchainGasPayment> for KaspaRocksDB {
    /// Store a list of interchain gas payments and their associated metadata.
    #[instrument(skip_all)]
    async fn store_logs(
        &self,
        payments: &[(Indexed<InterchainGasPayment>, LogMeta)],
    ) -> Result<u32> {
        store_and_count_new(
            self,
            payments,
            "gas payments",
            KaspaRocksDB::process_indexed_gas_payment,
        )
        .await
    }
}

#[async_trait]
impl HyperlaneLogStore<MerkleTreeInsertion> for KaspaRocksDB {
    /// Store every tree insertion event
    #[instrument(skip_all)]
    async fn store_logs(&self, leaves: &[(Indexed<MerkleTreeInsertion>, LogMeta)]) -> Result<u32> {
        let mut insertions: u32 = 0;
        for (insertion, meta) in leaves {
            if self.process_tree_insertion(insertion.inner(), meta.block_number)? {
                insertions = insertions.saturating_add(1);
            }
        }
        Ok(insertions)
    }
}

#[async_trait]
impl HyperlaneSequenceAwareIndexerStoreReader<HyperlaneMessage> for KaspaRocksDB {
    /// Gets data by its sequence.
    async fn retrieve_by_sequence(&self, sequence: u32) -> Result<Option<HyperlaneMessage>> {
        let message = self.retrieve_message_by_nonce(sequence)?;
        Ok(message)
    }

    /// Gets the block number at which the log occurred.
    async fn retrieve_log_block_number_by_sequence(&self, sequence: u32) -> Result<Option<u64>> {
        let number = self.retrieve_dispatched_block_number_by_nonce(&sequence)?;
        Ok(number)
    }
}

#[async_trait]
impl HyperlaneSequenceAwareIndexerStoreReader<MerkleTreeInsertion> for KaspaRocksDB {
    /// Gets data by its sequence.
    async fn retrieve_by_sequence(&self, sequence: u32) -> Result<Option<MerkleTreeInsertion>> {
        let insertion = self.retrieve_merkle_tree_insertion_by_leaf_index(&sequence)?;
        Ok(insertion)
    }

    /// Gets the block number at which the log occurred.
    async fn retrieve_log_block_number_by_sequence(&self, sequence: u32) -> Result<Option<u64>> {
        let number = self.retrieve_merkle_tree_insertion_block_number_by_leaf_index(&sequence)?;
        Ok(number)
    }
}

// TODO: replace this blanket implementation to be able to do sequence-aware indexing
#[async_trait]
impl HyperlaneSequenceAwareIndexerStoreReader<InterchainGasPayment> for KaspaRocksDB {
    /// Gets data by its sequence.
    async fn retrieve_by_sequence(&self, sequence: u32) -> Result<Option<InterchainGasPayment>> {
        Ok(self.retrieve_gas_payment_by_sequence(&sequence)?)
    }

    /// Gets the block number at which the log occurred.
    async fn retrieve_log_block_number_by_sequence(&self, sequence: u32) -> Result<Option<u64>> {
        Ok(self.retrieve_gas_payment_block_by_sequence(&sequence)?)
    }
}

#[async_trait]
impl HyperlaneWatermarkedLogStore<InterchainGasPayment> for KaspaRocksDB {
    /// Gets the block number high watermark
    async fn retrieve_high_watermark(&self) -> Result<Option<u32>> {
        let watermark = self.retrieve_decodable("", LATEST_INDEXED_GAS_PAYMENT_BLOCK)?;
        Ok(watermark)
    }

    /// Stores the block number high watermark
    async fn store_high_watermark(&self, block_number: u32) -> Result<()> {
        let result = self.store_encodable("", LATEST_INDEXED_GAS_PAYMENT_BLOCK, &block_number)?;
        Ok(result)
    }
}

// Keep this implementation for type compatibility with the `contract_syncs` sync builder
#[async_trait]
impl HyperlaneWatermarkedLogStore<HyperlaneMessage> for KaspaRocksDB {
    /// Gets the block number high watermark
    async fn retrieve_high_watermark(&self) -> Result<Option<u32>> {
        bail!("Not implemented")
    }

    /// Stores the block number high watermark
    async fn store_high_watermark(&self, _block_number: u32) -> Result<()> {
        bail!("Not implemented")
    }
}

// Keep this implementation for type compatibility with the `contract_syncs` sync builder
#[async_trait]
impl HyperlaneWatermarkedLogStore<MerkleTreeInsertion> for KaspaRocksDB {
    /// Gets the block number high watermark
    async fn retrieve_high_watermark(&self) -> Result<Option<u32>> {
        bail!("Not implemented")
    }

    /// Stores the block number high watermark
    async fn store_high_watermark(&self, _block_number: u32) -> Result<()> {
        bail!("Not implemented")
    }
}

impl HyperlaneDb for KaspaRocksDB {
    fn retrieve_highest_seen_message_nonce(&self) -> DbResult<Option<u32>> {
        self.retrieve_highest_seen_message_nonce_number()
    }

    fn retrieve_message_by_nonce(&self, nonce: u32) -> DbResult<Option<HyperlaneMessage>> {
        self.retrieve_message_by_nonce(nonce)
    }

    fn domain(&self) -> &HyperlaneDomain {
        self.domain()
    }

    fn store_message_id_by_nonce(&self, nonce: &u32, id: &H256) -> DbResult<()> {
        self.store_value_by_key(MESSAGE_ID, nonce, id)
    }

    fn retrieve_message_id_by_nonce(&self, nonce: &u32) -> DbResult<Option<H256>> {
        self.retrieve_value_by_key(MESSAGE_ID, nonce)
    }

    fn store_message_by_id(&self, id: &H256, message: &HyperlaneMessage) -> DbResult<()> {
        self.store_value_by_key(MESSAGE, id, message)
    }

    fn retrieve_message_by_id(&self, id: &H256) -> DbResult<Option<HyperlaneMessage>> {
        self.retrieve_value_by_key(MESSAGE, id)
    }

    fn store_dispatched_block_number_by_nonce(
        &self,
        nonce: &u32,
        block_number: &u64,
    ) -> DbResult<()> {
        self.store_value_by_key(MESSAGE_DISPATCHED_BLOCK_NUMBER, nonce, block_number)
    }

    fn retrieve_dispatched_block_number_by_nonce(&self, nonce: &u32) -> DbResult<Option<u64>> {
        self.retrieve_value_by_key(MESSAGE_DISPATCHED_BLOCK_NUMBER, nonce)
    }

    /// Store whether a message was processed by its nonce
    fn store_processed_by_nonce(&self, nonce: &u32, processed: &bool) -> DbResult<()> {
        self.store_value_by_key(NONCE_PROCESSED, nonce, processed)
    }

    fn retrieve_processed_by_nonce(&self, nonce: &u32) -> DbResult<Option<bool>> {
        self.retrieve_value_by_key(NONCE_PROCESSED, nonce)
    }

    fn store_processed_by_gas_payment_meta(
        &self,
        meta: &InterchainGasPaymentMeta,
        processed: &bool,
    ) -> DbResult<()> {
        self.store_value_by_key(GAS_PAYMENT_META_PROCESSED, meta, processed)
    }

    fn retrieve_processed_by_gas_payment_meta(
        &self,
        meta: &InterchainGasPaymentMeta,
    ) -> DbResult<Option<bool>> {
        self.retrieve_value_by_key(GAS_PAYMENT_META_PROCESSED, meta)
    }

    fn store_interchain_gas_expenditure_data_by_message_id(
        &self,
        message_id: &H256,
        data: &InterchainGasExpenditureData,
    ) -> DbResult<()> {
        self.store_value_by_key(GAS_EXPENDITURE_FOR_MESSAGE_ID, message_id, data)
    }

    fn retrieve_interchain_gas_expenditure_data_by_message_id(
        &self,
        message_id: &H256,
    ) -> DbResult<Option<InterchainGasExpenditureData>> {
        self.retrieve_value_by_key(GAS_EXPENDITURE_FOR_MESSAGE_ID, message_id)
    }

    /// Store the status of an operation by its message id
    fn store_status_by_message_id(
        &self,
        message_id: &H256,
        status: &PendingOperationStatus,
    ) -> DbResult<()> {
        self.store_value_by_key(STATUS_BY_MESSAGE_ID, message_id, status)
    }

    /// Retrieve the status of an operation by its message id
    fn retrieve_status_by_message_id(
        &self,
        message_id: &H256,
    ) -> DbResult<Option<PendingOperationStatus>> {
        self.retrieve_value_by_key(STATUS_BY_MESSAGE_ID, message_id)
    }

    fn store_interchain_gas_payment_data_by_gas_payment_key(
        &self,
        key: &GasPaymentKey,
        data: &InterchainGasPaymentData,
    ) -> DbResult<()> {
        self.store_value_by_key(GAS_PAYMENT_FOR_MESSAGE_ID, key, data)
    }

    fn retrieve_interchain_gas_payment_data_by_gas_payment_key(
        &self,
        key: &GasPaymentKey,
    ) -> DbResult<Option<InterchainGasPaymentData>> {
        self.retrieve_value_by_key(GAS_PAYMENT_FOR_MESSAGE_ID, key)
    }

    fn store_gas_payment_by_sequence(
        &self,
        sequence: &u32,
        payment: &InterchainGasPayment,
    ) -> DbResult<()> {
        self.store_value_by_key(GAS_PAYMENT_BY_SEQUENCE, sequence, payment)
    }

    fn retrieve_gas_payment_by_sequence(
        &self,
        sequence: &u32,
    ) -> DbResult<Option<InterchainGasPayment>> {
        self.retrieve_value_by_key(GAS_PAYMENT_BY_SEQUENCE, sequence)
    }

    fn store_gas_payment_block_by_sequence(
        &self,
        sequence: &u32,
        block_number: &u64,
    ) -> DbResult<()> {
        self.store_value_by_key(GAS_PAYMENT_BLOCK_BY_SEQUENCE, sequence, block_number)
    }

    fn retrieve_gas_payment_block_by_sequence(&self, sequence: &u32) -> DbResult<Option<u64>> {
        self.retrieve_value_by_key(GAS_PAYMENT_BLOCK_BY_SEQUENCE, sequence)
    }

    /// Store the retry count for a pending message by its message id
    fn store_pending_message_retry_count_by_message_id(
        &self,
        message_id: &H256,
        count: &u32,
    ) -> DbResult<()> {
        self.store_value_by_key(
            PENDING_MESSAGE_RETRY_COUNT_FOR_MESSAGE_ID,
            message_id,
            count,
        )
    }

    /// Retrieve the retry count for a pending message by its message id
    fn retrieve_pending_message_retry_count_by_message_id(
        &self,
        message_id: &H256,
    ) -> DbResult<Option<u32>> {
        self.retrieve_value_by_key(PENDING_MESSAGE_RETRY_COUNT_FOR_MESSAGE_ID, message_id)
    }

    fn store_merkle_tree_insertion_by_leaf_index(
        &self,
        leaf_index: &u32,
        insertion: &MerkleTreeInsertion,
    ) -> DbResult<()> {
        self.store_value_by_key(MERKLE_TREE_INSERTION, leaf_index, insertion)
    }

    /// Retrieve the merkle tree insertion event by its leaf index
    fn retrieve_merkle_tree_insertion_by_leaf_index(
        &self,
        leaf_index: &u32,
    ) -> DbResult<Option<MerkleTreeInsertion>> {
        self.retrieve_value_by_key(MERKLE_TREE_INSERTION, leaf_index)
    }

    fn store_merkle_leaf_index_by_message_id(
        &self,
        message_id: &H256,
        leaf_index: &u32,
    ) -> DbResult<()> {
        self.store_value_by_key(MERKLE_LEAF_INDEX_BY_MESSAGE_ID, message_id, leaf_index)
    }

    /// Retrieve the merkle leaf index of a message in the merkle tree
    fn retrieve_merkle_leaf_index_by_message_id(&self, message_id: &H256) -> DbResult<Option<u32>> {
        self.retrieve_value_by_key(MERKLE_LEAF_INDEX_BY_MESSAGE_ID, message_id)
    }

    fn store_merkle_tree_insertion_block_number_by_leaf_index(
        &self,
        leaf_index: &u32,
        block_number: &u64,
    ) -> DbResult<()> {
        self.store_value_by_key(
            MERKLE_TREE_INSERTION_BLOCK_NUMBER_BY_LEAF_INDEX,
            leaf_index,
            block_number,
        )
    }

    fn retrieve_merkle_tree_insertion_block_number_by_leaf_index(
        &self,
        leaf_index: &u32,
    ) -> DbResult<Option<u64>> {
        self.retrieve_value_by_key(MERKLE_TREE_INSERTION_BLOCK_NUMBER_BY_LEAF_INDEX, leaf_index)
    }

    fn store_highest_seen_message_nonce_number(&self, nonce: &u32) -> DbResult<()> {
        // There's no unit struct Encode/Decode impl, so just use `bool` and always use the `Default::default()` key
        self.store_value_by_key(HIGHEST_SEEN_MESSAGE_NONCE, &bool::default(), nonce)
    }

    /// Retrieve the nonce of the highest processed message we're aware of
    fn retrieve_highest_seen_message_nonce_number(&self) -> DbResult<Option<u32>> {
        // There's no unit struct Encode/Decode impl, so just use `bool` and always use the `Default::default()` key
        self.retrieve_value_by_key(HIGHEST_SEEN_MESSAGE_NONCE, &bool::default())
    }

    fn store_payload_uuids_by_message_id(
        &self,
        message_id: &H256,
        payload_uuids: Vec<UniqueIdentifier>,
    ) -> DbResult<()> {
        self.store_value_by_key(PAYLOAD_UUIDS_BY_MESSAGE_ID, message_id, &payload_uuids)
    }

    fn retrieve_payload_uuids_by_message_id(
        &self,
        message_id: &H256,
    ) -> DbResult<Option<Vec<UniqueIdentifier>>> {
        self.retrieve_value_by_key(PAYLOAD_UUIDS_BY_MESSAGE_ID, message_id)
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
        self.upsert_message(&message, 0u64)?;

        // Store deposit message by message_id
        self.store_value_by_key(KASPA_DEPOSIT_MESSAGE, &id, &message)?;

        // Store mapping from tx_hash to message_id for retrieval by tx_hash
        self.store_encodable(KASPA_DEPOSIT_MESSAGE_ID_BY_TX_HASH, tx_hash.as_bytes(), &id)?;

        Ok(())
    }

    /// Retrieve a Kaspa deposit message by message_id
    pub fn retrieve_kaspa_deposit_by_message_id(&self, message_id: &H256) -> DbResult<Option<HyperlaneMessage>> {
        self.retrieve_value_by_key(KASPA_DEPOSIT_MESSAGE, message_id)
    }

    /// Retrieve a Kaspa deposit message by kaspa transaction hash
    pub fn retrieve_kaspa_deposit_by_tx_hash(&self, tx_hash: &str) -> DbResult<Option<HyperlaneMessage>> {
        // First get the message_id from tx_hash (stored as bytes)
        let message_id: Option<H256> = self.retrieve_decodable(KASPA_DEPOSIT_MESSAGE_ID_BY_TX_HASH, tx_hash.as_bytes())?;

        match message_id {
            Some(id) => self.retrieve_kaspa_deposit_by_message_id(&id),
            None => Ok(None),
        }
    }

    /// Store a withdrawal message indexed by message_id
    pub fn store_withdrawal_message(
        &self,
        message: HyperlaneMessage,
        dispatched_block_number: u64,
    ) -> DbResult<()> {
        let id = message.id();

        debug!(
            message_id = ?id,
            nonce = message.nonce,
            "Storing Kaspa withdrawal"
        );

        // Store the message in the general message storage
        self.upsert_message(&message, dispatched_block_number)?;

        // Store withdrawal message by message_id
        self.store_value_by_key(KASPA_WITHDRAWAL_MESSAGE, &id, &message)?;
        // Store withdrawal block number by message_id
        self.store_value_by_key(KASPA_WITHDRAWAL_BLOCK_NUMBER, &id, &dispatched_block_number)?;

        Ok(())
    }

    /// Retrieve a Kaspa withdrawal message by message_id
    pub fn retrieve_kaspa_withdrawal_by_message_id(&self, message_id: &H256) -> DbResult<Option<HyperlaneMessage>> {
        self.retrieve_value_by_key(KASPA_WITHDRAWAL_MESSAGE, message_id)
    }

    /// Store Hub transaction ID for a deposit indexed by kaspa_tx
    pub fn store_deposit_hub_tx(&self, kaspa_tx: &str, hub_tx: &str) -> DbResult<()> {
        debug!(
            kaspa_tx = %kaspa_tx,
            hub_tx = %hub_tx,
            "Storing deposit Hub transaction ID"
        );

        // Parse hub_tx as H512 first
        let hub_tx_h512: H512 = hub_tx.parse().map_err(|e| {
            DbError::from(HyperlaneProtocolError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid hub_tx format: {}", e)
            )))
        })?;

        // Check if first 32 bytes (64 hex chars) are all zeros
        let bytes = hub_tx_h512.as_bytes();
        let first_half_is_zero = bytes[..32].iter().all(|&b| b == 0);

        if first_half_is_zero {
            // Convert second half to H256 and store
            let mut h256_bytes = [0u8; 32];
            h256_bytes.copy_from_slice(&bytes[32..]);
            let hub_tx_h256 = H256::from(h256_bytes);
            debug!(
                kaspa_tx = %kaspa_tx,
                hub_tx_h256 = ?hub_tx_h256,
                "Storing hub_tx as H256 (first half was zeros)"
            );
            self.store_encodable(KASPA_DEPOSIT_HUB_TX, kaspa_tx.as_bytes(), &hub_tx_h256)
        } else {
            // Store full H512
            self.store_encodable(KASPA_DEPOSIT_HUB_TX, kaspa_tx.as_bytes(), &hub_tx_h512)
        }
    }

    /// Retrieve Hub transaction ID for a deposit by kaspa_tx
    pub fn retrieve_deposit_hub_tx(&self, kaspa_tx: &str) -> DbResult<Option<String>> {
        // Try to retrieve as H256 first
        let hub_tx_h256: Option<H256> = self.retrieve_decodable(KASPA_DEPOSIT_HUB_TX, kaspa_tx.as_bytes())?;
        if let Some(h256) = hub_tx_h256 {
            return Ok(Some(format!("{:x}", h256)));
        }

        // If not found as H256, try as H512
        let hub_tx_h512: Option<H512> = self.retrieve_decodable(KASPA_DEPOSIT_HUB_TX, kaspa_tx.as_bytes())?;
        Ok(hub_tx_h512.map(|h| format!("{:x}", h)))
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
                format!("Invalid kaspa_tx format: {}", e)
            )))
        })?;
        self.store_value_by_key(KASPA_WITHDRAWAL_KASPA_TX, message_id, &kaspa_tx_h256)
    }

    /// Retrieve Kaspa transaction ID for a withdrawal by message_id
    pub fn retrieve_withdrawal_kaspa_tx(&self, message_id: &H256) -> DbResult<Option<String>> {
        let kaspa_tx_h256: Option<H256> = self.retrieve_value_by_key(KASPA_WITHDRAWAL_KASPA_TX, message_id)?;
        Ok(kaspa_tx_h256.map(|h| format!("{:x}", h)))
    }
}

// Implement the KaspaDb trait from hyperlane-core to allow dymension-kaspa
// to access kaspa_db functionality without creating circular dependencies
impl hyperlane_core::KaspaDb for KaspaRocksDB {
    fn store_withdrawal_message(
        &self,
        message: HyperlaneMessage,
        dispatched_block_number: u64,
    ) -> Result<()> {
        Ok(self.store_withdrawal_message(message, dispatched_block_number)?)
    }

    fn retrieve_kaspa_withdrawal_by_message_id(
        &self,
        message_id: &H256,
    ) -> Result<Option<HyperlaneMessage>> {
        Ok(self.retrieve_kaspa_withdrawal_by_message_id(message_id)?)
    }

    fn store_deposit_message(
        &self,
        message: HyperlaneMessage,
        tx_hash: String,
    ) -> Result<()> {
        Ok(self.store_deposit_message(message, tx_hash)?)
    }

    fn retrieve_kaspa_deposit_by_message_id(
        &self,
        message_id: &H256,
    ) -> Result<Option<HyperlaneMessage>> {
        Ok(self.retrieve_kaspa_deposit_by_message_id(message_id)?)
    }

    fn retrieve_kaspa_deposit_by_tx_hash(
        &self,
        tx_hash: &str,
    ) -> Result<Option<HyperlaneMessage>> {
        Ok(self.retrieve_kaspa_deposit_by_tx_hash(tx_hash)?)
    }

    fn store_deposit_hub_tx(
        &self,
        kaspa_tx: &str,
        hub_tx: &str,
    ) -> Result<()> {
        Ok(self.store_deposit_hub_tx(kaspa_tx, hub_tx)?)
    }

    fn retrieve_deposit_hub_tx(
        &self,
        kaspa_tx: &str,
    ) -> Result<Option<String>> {
        Ok(self.retrieve_deposit_hub_tx(kaspa_tx)?)
    }

    fn store_withdrawal_kaspa_tx(
        &self,
        message_id: &H256,
        kaspa_tx: &str,
    ) -> Result<()> {
        Ok(self.store_withdrawal_kaspa_tx(message_id, kaspa_tx)?)
    }

    fn retrieve_withdrawal_kaspa_tx(
        &self,
        message_id: &H256,
    ) -> Result<Option<String>> {
        Ok(self.retrieve_withdrawal_kaspa_tx(message_id)?)
    }
}
