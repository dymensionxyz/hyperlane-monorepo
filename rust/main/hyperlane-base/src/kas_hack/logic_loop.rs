use crate::contract_sync::cursors::Indexable;
use dym_kas_core::{
    confirmation::ConfirmationFXG, deposit::DepositFXG, finality::is_safe_against_reorg,
};
use dym_kas_relayer::confirm::expensive_trace_transactions;
use dym_kas_relayer::deposit::{on_new_deposit as relayer_on_new_deposit, KaspaTxError};
use dymension_kaspa::{Deposit, KaspaProvider};
use ethers::utils::hex::ToHex;
use eyre::Result;
use hyperlane_core::{
    ChainCommunicationError, ChainResult, Checkpoint, CheckpointWithMessageId, HyperlaneChain,
    HyperlaneLogStore, Indexed, LogMeta, Mailbox, MultisigSignedCheckpoint, Signature,
    SignedCheckpointWithMessageId, TxOutcome, H256,
};
use hyperlane_cosmos::native::{h512_to_cosmos_hash, CosmosNativeMailbox};
use kaspa_consensus_core::tx::TransactionOutpoint;
use kaspa_core::time::unix_now;
use std::{collections::HashSet, fmt::Debug, hash::Hash, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle, time};
use tokio_metrics::TaskMonitor;
use tracing::{debug, error, info, info_span, warn, Instrument};

use super::{
    deposit_operation::{DepositOpQueue, DepositOperation},
    error::KaspaDepositError,
};
use dymension_kaspa::conf::KaspaTimeConfig;

pub struct Foo<C: MetadataConstructor> {
    provider: Box<KaspaProvider>,
    hub_mailbox: Arc<CosmosNativeMailbox>,
    metadata_constructor: C,
    deposit_cache: DepositCache,
    deposit_queue: Mutex<DepositOpQueue>,
    config: KaspaTimeConfig,
}

impl<C: MetadataConstructor> Foo<C>
where
    C: Send + Sync + 'static,
{
    pub fn new(
        provider: Box<KaspaProvider>,
        hub_mailbox: Arc<CosmosNativeMailbox>,
        metadata_constructor: C,
    ) -> Self {
        let config = provider
            .kaspa_time_config()
            .unwrap_or_else(KaspaTimeConfig::default);
        Self {
            provider,
            hub_mailbox,
            metadata_constructor,
            deposit_cache: DepositCache::new(),
            deposit_queue: Mutex::new(DepositOpQueue::new()),
            config,
        }
    }

    pub fn run_loops(self, task_monitor: TaskMonitor) -> JoinHandle<()> {
        let foo = Arc::new(self);

        {
            let foo_clone = foo.clone();
            let name = "dymension_kaspa_deposit_loop";
            tokio::task::Builder::new()
                .name(name)
                .spawn(TaskMonitor::instrument(
                    &task_monitor,
                    async move {
                        foo_clone.deposit_loop().await;
                    }
                    .instrument(info_span!("Kaspa Monitor")),
                ))
                .expect("Failed to spawn kaspa monitor task");
        }

        {
            let foo_clone = foo.clone();
            let name = "dymension_kaspa_progress_indication_loop";
            tokio::task::Builder::new()
                .name(name)
                .spawn(TaskMonitor::instrument(
                    &task_monitor,
                    async move {
                        foo_clone.progress_indication_loop().await;
                    }
                    .instrument(info_span!("Kaspa Monitor")),
                ))
                .expect("Failed to spawn kaspa progress indication task")
        }
    }

    // https://github.com/dymensionxyz/hyperlane-monorepo/blob/20b9e669afcfb7728e66b5932e85c0f7fcbd50c1/dymension/libs/kaspa/lib/relayer/note.md#L102-L119
    async fn deposit_loop(&self) {
        info!("Dymension, starting deposit loop with queue");
        let lower_bound_unix_time: Option<i64> =
            match self.provider.must_relayer_stuff().deposit_look_back_mins {
                Some(offset) => {
                    let secs = offset * 60;
                    let d = Duration::new(secs, 0);
                    Some(unix_now() as i64 - d.as_millis() as i64)
                }
                None => None,
            };
        loop {
            self.process_deposit_queue().await;
            let deposits_res = self
                .provider
                .rest()
                .get_deposits(
                    &self.provider.escrow_address().to_string(),
                    lower_bound_unix_time,
                )
                .await;
            let deposits = match deposits_res {
                Ok(deposits) => deposits,
                Err(e) => {
                    error!(error = ?e, "Dymension, query new Kaspa deposits failed");
                    time::sleep(self.config.poll_interval()).await;
                    continue;
                }
            };
            info!(
                deposit_count = deposits.len(),
                "Dymension, queried kaspa deposits"
            );
            self.handle_new_deposits(deposits).await;

            time::sleep(self.config.poll_interval()).await;
        }
    }

    async fn handle_new_deposits(&self, deposits: Vec<Deposit>) {
        let mut deposits_new = Vec::new();
        let escrow_address = self.provider.escrow_address().to_string();

        for d in deposits.into_iter() {
            if !self.deposit_cache.has_seen(&d).await {
                self.deposit_cache.mark_as_seen(d.clone()).await;
                match self.is_deposit(&d, &escrow_address).await {
                    Ok(true) => {
                        info!(deposit = ?d, "Dymension, new deposit seen");
                        deposits_new.push(d);
                    }
                    Ok(false) => {
                        info!(deposit_id = %d.id, "Dymension, skipping deposit with invalid or missing Hyperlane payload");
                    }
                    Err(e) => {
                        error!(deposit_id = %d.id, error = ?e, "Dymension, failed to check if deposit is genuine, skipping");
                    }
                }
            }
        }

        if !deposits_new.is_empty() {
            if let Err(e) = self.provider.update_balance_metrics().await {
                error!("Failed to update balance metrics: {:?}", e);
            }
        }

        for d in &deposits_new {
            let operation =
                DepositOperation::new(d.clone(), self.provider.escrow_address().to_string());
            self.process_deposit_operation(operation).await;
        }
    }

    async fn is_deposit(&self, deposit: &Deposit, _escrow_address: &str) -> Result<bool> {
        use dym_kas_core::message::ParsedHL;

        let payload = match &deposit.payload {
            Some(payload) => payload,
            None => {
                info!(deposit_id = %deposit.id, "Deposit has no payload, skipping");
                return Ok(false);
            }
        };

        match ParsedHL::parse_string(payload) {
            Ok(parsed_hl) => {
                info!(
                    deposit_id = %deposit.id,
                    message_id = ?parsed_hl.hl_message.id(),
                    "Valid Hyperlane message found in deposit payload"
                );
                Ok(true)
            }
            Err(e) => {
                info!(
                    deposit_id = %deposit.id,
                    error = ?e,
                    "Invalid Hyperlane payload, skipping deposit"
                );
                Ok(false)
            }
        }
    }

    async fn process_deposit_queue(&self) {
        let mut queue = self.deposit_queue.lock().await;

        while let Some(operation) = queue.pop_ready() {
            drop(queue);
            self.process_deposit_operation(operation).await;
            queue = self.deposit_queue.lock().await;
        }
    }

    async fn process_deposit_operation(&self, mut operation: DepositOperation) {
        info!(deposit_id = %operation.deposit.id, "Processing deposit operation");

        let operation_start_time = operation.created_at;

        let deposit_amount = if let Some(payload) = &operation.deposit.payload {
            match dym_kas_core::message::ParsedHL::parse_string(payload) {
                Ok(parsed_hl) => parsed_hl.token_message.amount().low_u64(),
                Err(e) => {
                    tracing::error!(
                        "Failed to parse deposit payload for amount, using 0: {:?}",
                        e
                    );
                    0
                }
            }
        } else {
            tracing::error!("Deposit has no payload, using 0 for amount");
            0
        };


        let new_deposit_res = relayer_on_new_deposit(
            &operation.escrow_address,
            &operation.deposit,
            &self.provider.rest().client.client,
        )
        .await;

        match new_deposit_res {
            Ok(Some(fxg)) => {
                info!(fxg = ?fxg, "Dymension, built new deposit FXG");

                let delivered_res = self.hub_mailbox.delivered(fxg.hl_message.id()).await;
                match delivered_res {
                    Ok(true) => {
                        info!(
                            message_id = ?fxg.hl_message.id(),
                            "Dymension, deposit already delivered, skipping"
                        );
                        return;
                    }
                    Err(e) => {
                        error!(error = ?e, "Dymension, check if deposit is delivered");
                        let deposit_id = format!("{:?}", operation.deposit.id);
                        self.provider
                            .metrics()
                            .record_deposit_failed(&deposit_id, deposit_amount);
                        operation.mark_failed(&self.config);
                        self.deposit_queue.lock().await.requeue(operation);
                        return;
                    }
                    _ => {}
                };

                let res = self.get_deposit_validator_sigs_and_send_to_hub(&fxg).await;
                match res {
                    Ok(outcome) => {
                        let tx_hash =
                            hyperlane_cosmos::native::h512_to_cosmos_hash(outcome.transaction_id)
                                .encode_hex_upper::<String>();
                        let deposit_amount = fxg.amount.low_u64();
                        let deposit_id = format!("{:?}", operation.deposit.id);

                        if !outcome.executed {
                            error!(
                                message_id = ?fxg.hl_message.id(),
                                tx_hash = %tx_hash,
                                gas_used = %outcome.gas_used,
                                "Dymension, deposit process() failed - TX was not executed on-chain"
                            );

                            self.provider
                                .metrics()
                                .record_deposit_failed(&deposit_id, deposit_amount);

                            operation.mark_failed(&self.config);
                            self.deposit_queue.lock().await.requeue(operation);
                        } else {
                            info!(
                                fxg = ?fxg,
                                tx_hash = %tx_hash,
                                "Dymension, got sigs and sent new deposit to hub"
                            );

                            let latency_ms = operation_start_time.elapsed().as_millis() as i64;

                            self.provider
                                .metrics()
                                .record_deposit_processed(&deposit_id, deposit_amount);
                            self.provider.metrics().update_deposit_latency(latency_ms);

                            operation.reset_attempts();
                        }
                    }
                    Err(e) => {
                        let kaspa_err = self.chain_error_to_kaspa_error(&e);
                        if kaspa_err.is_retryable() {
                            error!(
                                error = ?e,
                                "Dymension, gather sigs and send deposit to hub (retryable)"
                            );
                            let deposit_id = format!("{:?}", operation.deposit.id);
                            self.provider
                                .metrics()
                                .record_deposit_failed(&deposit_id, deposit_amount);
                            operation.mark_failed(&self.config);
                            self.deposit_queue.lock().await.requeue(operation);
                        } else {
                            error!(
                                error = ?e,
                                "Dymension, gather sigs and send deposit to hub (non-retryable)"
                            );
                            let deposit_id = format!("{:?}", operation.deposit.id);
                            self.provider
                                .metrics()
                                .record_deposit_failed(&deposit_id, deposit_amount);
                            info!(
                                deposit_id = %operation.deposit.id,
                                "Dropping operation due to non-retryable error"
                            );
                        }
                    }
                }
            }
            Ok(None) => {
                info!("Dymension, F() new deposit returned none, will retry");
                let deposit_id = format!("{:?}", operation.deposit.id);
                self.provider
                    .metrics()
                    .record_deposit_failed(&deposit_id, deposit_amount);
                operation.mark_failed(&self.config);
                self.deposit_queue.lock().await.requeue(operation);
            }
            Err(e) => {
                let kaspa_err = KaspaDepositError::from(e);

                let deposit_id = format!("{:?}", operation.deposit.id);
                self.provider
                    .metrics()
                    .record_deposit_failed(&deposit_id, deposit_amount);

                if let Some(retry_delay_secs) = kaspa_err.retry_delay_hint() {
                    let delay = Duration::from_secs_f64(retry_delay_secs);
                    operation.mark_failed_with_custom_delay(delay, &kaspa_err.to_string());
                } else {
                    error!(
                        error = ?kaspa_err,
                        "Dymension, F() new deposit processing error, will retry"
                    );
                    operation.mark_failed(&self.config);
                }
                self.deposit_queue.lock().await.requeue(operation);
            }
        }
    }

    async fn progress_indication_loop(&self) {
        loop {
            let confirmation = self.provider.get_pending_confirmation().await;

            match confirmation {
                Some(confirmation) => {
                    let res = self.confirm_withdrawal_on_hub(confirmation.clone()).await;
                    match res {
                        Ok(_) => {
                            info!(confirmation = ?confirmation, "Dymension, confirmed withdrawal on hub");
                            self.provider.metrics().update_confirmations_pending(0);
                            self.provider.consume_pending_confirmation();

                            if let Err(e) = self.update_hub_anchor_point_metric().await {
                                error!(error = ?e, "Failed to update hub anchor point metric after successful confirmation");
                            }
                        }
                        Err(KaspaTxError::NotFinalError {
                            retry_after_secs, ..
                        }) => {
                            info!(
                                retry_after_secs = retry_after_secs,
                                "Dymension, withdrawal not final yet, sleeping before retry"
                            );
                            self.provider.metrics().update_confirmations_pending(1);
                            time::sleep(Duration::from_secs_f64(retry_after_secs)).await;
                            continue;
                        }
                        Err(e) => {
                            error!("Dymension, confirm withdrawal on hub: {:?}", e);
                            self.provider.metrics().record_confirmation_failed();
                        }
                    }
                }
                None => {
                    info!("Dymension, no pending confirmation found.");
                }
            }

            time::sleep(self.config.poll_interval()).await;
        }
    }

    async fn get_deposit_validator_sigs_and_send_to_hub(
        &self,
        fxg: &DepositFXG,
    ) -> ChainResult<TxOutcome> {
        let mut sigs = self.provider.validators().get_deposit_sigs(fxg).await?;
        info!(
            "Dymension, got deposit sigs: number of sigs: {:?}",
            sigs.len()
        );

        let formatted_sigs = self.format_checkpoint_signatures(
            &mut sigs,
            self.provider.validators().multisig_threshold_hub_ism() as usize,
        )?;

        self.hub_mailbox
            .process(&fxg.hl_message, &formatted_sigs, None)
            .await
    }

    fn chain_error_to_kaspa_error(&self, error: &ChainCommunicationError) -> KaspaDepositError {
        KaspaDepositError::ProcessingError(error.to_string())
    }

    async fn _deposits_to_logs<T>(&self, _deposits: Vec<Deposit>) -> Vec<(Indexed<T>, LogMeta)>
    where
        T: Indexable + Debug + Send + Sync + Clone + Eq + Hash + 'static,
    {
        unimplemented!()
    }

    async fn _dedupe_and_store_logs<T, S>(
        &self,
        store: &S,
        logs: Vec<(Indexed<T>, LogMeta)>,
    ) -> Vec<(Indexed<T>, LogMeta)>
    where
        T: Indexable + Debug + Send + Sync + Clone + Eq + Hash + 'static,
        S: HyperlaneLogStore<T> + Clone + 'static,
    {
        let deduped_logs = HashSet::<_>::from_iter(logs);
        let logs = Vec::from_iter(deduped_logs);

        if let Err(err) = store.store_logs(&logs).await {
            debug!(error = ?err, "Error storing logs in db");
        }

        logs
    }

    pub async fn sync_hub_if_needed(&self) -> Result<()> {
        info!("Checking if hub is out of sync with Kaspa escrow account.");
        use hyperlane_cosmos::{native::ModuleQueryClient, CosmosProvider};
        let provider = self.hub_mailbox.provider();
        let cosmos_provider = provider
            .as_any()
            .downcast_ref::<CosmosProvider<ModuleQueryClient>>()
            .expect("Hub mailbox provider must be CosmosProvider");
        let resp = cosmos_provider.query().outpoint(None).await?;
        let old_anchor = resp
            .outpoint
            .map(|o| TransactionOutpoint {
                transaction_id: kaspa_hashes::Hash::from_bytes(
                    o.transaction_id.as_slice().try_into().unwrap(),
                ),
                index: o.index,
            })
            .ok_or_else(|| eyre::eyre!("No outpoint found"))?;

        let escrow_address = self.provider.escrow_address();

        info!(
            "Dymension, current anchor: {:?}, escrow address: {:?}",
            old_anchor, escrow_address
        );

        let all_escrow_utxos = self
            .provider
            .rpc()
            .get_utxos_by_addresses(vec![escrow_address.clone()])
            .await?;

        info!(
            "Queried utxos for escrow address: {:?}",
            all_escrow_utxos.len()
        );

        let hub_is_synced = all_escrow_utxos.iter().any(|utxo| {
            let ok = utxo.outpoint.transaction_id == old_anchor.transaction_id
                && utxo.outpoint.index == old_anchor.index;
            if ok {
                info!(utxo = ?utxo, "Dymension, found utxo matching current anchor");
            }
            ok
        });
        if !hub_is_synced {
            info!("Dymension is not synced, preparing progress indication and submitting to hub");

            let mut good = false;
            for utxo in all_escrow_utxos {
                let new_anchor_candidate = TransactionOutpoint::from(utxo.outpoint);
                let fxg = expensive_trace_transactions(
                    &self.provider.rest().client.client,
                    &escrow_address.to_string(),
                    new_anchor_candidate.clone(),
                    old_anchor,
                )
                .await;
                if !fxg.is_ok() {
                    error!(
                        "Dymension, tracing kaspa withdrawals for syncing: {:?}, candidate: {:?}",
                        fxg.err(),
                        new_anchor_candidate,
                    );
                    continue;
                }
                info!("Traced sequence of kaspa withdrawals for syncing");

                self.confirm_withdrawal_on_hub(fxg.unwrap()).await?;
                good = true;
                break;
            }
            if !good {
                return Err(eyre::eyre!("Dymension, no good utxo found for syncing"));
            }
        }
        info!("Dymension hub is synced, proceeding with other tasks");

        if let Err(e) = self.update_hub_anchor_point_metric().await {
            error!(error = ?e, "Failed to update hub anchor point metric after syncing");
        }

        Ok(())
    }

    async fn update_hub_anchor_point_metric(&self) -> Result<()> {
        use hyperlane_cosmos::{native::ModuleQueryClient, CosmosProvider};
        let provider = self.hub_mailbox.provider();
        let cosmos_provider = provider
            .as_any()
            .downcast_ref::<CosmosProvider<ModuleQueryClient>>()
            .expect("Hub mailbox provider must be CosmosProvider");
        let resp = cosmos_provider.query().outpoint(None).await?;

        if let Some(outpoint) = resp.outpoint {
            let tx_id = kaspa_hashes::Hash::from_bytes(
                outpoint
                    .transaction_id
                    .as_slice()
                    .try_into()
                    .map_err(|e| eyre::eyre!("Invalid transaction ID bytes: {:?}", e))?,
            );
            let current_timestamp = kaspa_core::time::unix_now();

            self.provider.metrics().update_hub_anchor_point(
                &tx_id.to_string(),
                outpoint.index as u64,
                current_timestamp,
            );

            info!(
                tx_id = %tx_id,
                outpoint_index = outpoint.index,
                "Updated hub anchor point metric"
            );
        } else {
            error!("No anchor point found in hub response");
        }

        Ok(())
    }

    async fn confirm_withdrawal_on_hub(&self, fxg: ConfirmationFXG) -> Result<(), KaspaTxError> {
        let new_anchor = fxg.outpoints.last().ok_or_else(|| {
            KaspaTxError::ProcessingError(eyre::eyre!("No outpoints in confirmation FXG"))
        })?;

        let finality_status = is_safe_against_reorg(
            &self.provider.rest().client.client,
            &new_anchor.transaction_id.to_string(),
            None,
        )
        .await
        .map_err(|e| KaspaTxError::ProcessingError(e))?;

        if !finality_status.is_final() {
            return Err(KaspaTxError::NotFinalError {
                confirmations: finality_status.confirmations,
                required_confirmations: finality_status.required_confirmations,
                retry_after_secs: (finality_status.required_confirmations
                    - finality_status.confirmations) as f64
                    * 0.1,
            });
        }

        info!(
            confirmations = finality_status.confirmations,
            required = finality_status.required_confirmations,
            "Finality check passed for withdrawal confirmation"
        );

        let mut sigs = self
            .provider
            .validators()
            .get_confirmation_sigs(&fxg)
            .await
            .map_err(|e| {
                KaspaTxError::ProcessingError(eyre::eyre!("Failed to get confirmation sigs: {}", e))
            })?;

        info!(sig_count = sigs.len(), "Dymension, got confirmation sigs");
        let formatted_sigs = self
            .format_ad_hoc_signatures(
                &mut sigs,
                self.provider.validators().multisig_threshold_hub_ism() as usize,
            )
            .map_err(|e| {
                KaspaTxError::ProcessingError(eyre::eyre!("Failed to format signatures: {}", e))
            })?;

        info!(
            "Dymension, formatted confirmation sigs: {:?}",
            formatted_sigs
        );

        let outcome = self
            .hub_mailbox
            .indicate_progress(&formatted_sigs, &fxg.progress_indication)
            .await
            .map_err(|e| {
                KaspaTxError::ProcessingError(eyre::eyre!("Indicate progress failed: {}", e))
            })?;

        let tx_hash = h512_to_cosmos_hash(outcome.transaction_id).encode_hex_upper::<String>();

        if !outcome.executed {
            return Err(KaspaTxError::ProcessingError(eyre::eyre!(
                "Indicate progress failed, TX was not executed on-chain, tx hash: {tx_hash}"
            )));
        }

        info!(
            "Dymension, indicated progress on hub: {:?}, outcome: {:?}, tx hash: {:?}",
            fxg.progress_indication, outcome, tx_hash,
        );

        Ok(())
    }

    // TODO: can probably just use the ad hoc method
    fn format_checkpoint_signatures(
        &self,
        sigs: &mut Vec<SignedCheckpointWithMessageId>,
        require: usize,
    ) -> ChainResult<Vec<u8>> {
        if sigs.len() < require {
            return Err(ChainCommunicationError::InvalidRequest {
                msg: format!(
                    "insufficient validator signatures: got {}, need {}",
                    sigs.len(),
                    require
                ),
            });
        }

        let checkpoint = MultisigSignedCheckpoint::try_from(sigs).map_err(|_| {
            ChainCommunicationError::InvalidRequest {
                msg: "to convert sigs to checkpoint".to_string(),
            }
        })?;
        let metadata = self.metadata_constructor.metadata(&checkpoint)?;
        Ok(metadata.to_vec())
    }

    fn format_ad_hoc_signatures(
        &self,
        sigs: &mut Vec<Signature>,
        require: usize,
    ) -> ChainResult<Vec<u8>> {
        if sigs.len() < require {
            return Err(ChainCommunicationError::InvalidRequest {
                msg: format!(
                    "insufficient validator signatures: got {}, need {}",
                    sigs.len(),
                    require
                ),
            });
        }

        let checkpoint = MultisigSignedCheckpoint {
            checkpoint: CheckpointWithMessageId {
                checkpoint: Checkpoint {
                    merkle_tree_hook_address: H256::default(),
                    mailbox_domain: 0,
                    root: H256::default(),
                    index: 0,
                },
                message_id: H256::default(),
            },
            signatures: sigs.clone(),
        };

        let metadata = self.metadata_constructor.metadata(&checkpoint)?;
        Ok(metadata.to_vec())
    }
}

pub struct DepositCache {
    seen: Mutex<HashSet<Deposit>>,
}

impl DepositCache {
    pub fn new() -> Self {
        Self {
            seen: Mutex::new(HashSet::new()),
        }
    }

    async fn has_seen(&self, deposit: &Deposit) -> bool {
        let seen_guard = self.seen.lock().await;
        seen_guard.contains(deposit)
    }

    async fn mark_as_seen(&self, deposit: Deposit) {
        let mut seen_guard = self.seen.lock().await;
        seen_guard.insert(deposit);
    }
}

pub trait MetadataConstructor {
    fn metadata(&self, checkpoint: &MultisigSignedCheckpoint) -> Result<Vec<u8>>;
}
