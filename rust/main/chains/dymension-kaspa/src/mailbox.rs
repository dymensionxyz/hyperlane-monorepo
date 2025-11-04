use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::consts::*;
use crate::KaspaProvider;
use dym_kas_relayer::withdraw::minimum::is_small_value;
use hyperlane_core::{
    utils::bytes_to_hex, BatchResult, ChainResult, ContractLocator, Decode, FixedPointNumber,
    HyperlaneChain, HyperlaneContract, HyperlaneDomain, HyperlaneMessage, HyperlaneProvider,
    Mailbox, QueueOperation, ReorgPeriod, TxCostEstimate, TxOutcome, H256, H512, U256,
};
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::{WithdrawalId, WithdrawalStatus};
use hyperlane_warp_route::TokenMessage;
use tonic::async_trait;
use tracing::{error, info, warn};

// pretends to be a mailbox
#[derive(Clone)]
pub struct KaspaMailbox {
    provider: KaspaProvider,
    domain: HyperlaneDomain,
    address: H256,
    operation_timestamps: Arc<Mutex<HashMap<String, std::time::Instant>>>,
    /// Optional Kaspa database for tracking deposits/withdrawals
    kaspa_db: Option<Arc<dyn hyperlane_core::KaspaDb>>,
}

// Manual Debug implementation since dyn KaspaDb doesn't implement Debug
impl std::fmt::Debug for KaspaMailbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KaspaMailbox")
            .field("provider", &self.provider)
            .field("domain", &self.domain)
            .field("address", &self.address)
            .field("operation_timestamps", &self.operation_timestamps)
            .field("kaspa_db", &self.kaspa_db.is_some())
            .finish()
    }
}

impl KaspaMailbox {
    pub fn new(provider: KaspaProvider, locator: ContractLocator) -> ChainResult<KaspaMailbox> {
        Ok(KaspaMailbox {
            provider,
            address: locator.address,
            domain: locator.domain.clone(),
            operation_timestamps: Arc::new(Mutex::new(HashMap::new())),
            kaspa_db: None,
        })
    }

    pub fn with_provider(&self, provider: KaspaProvider) -> Self {
        Self {
            provider,
            domain: self.domain.clone(),
            address: self.address,
            operation_timestamps: self.operation_timestamps.clone(),
            kaspa_db: self.kaspa_db.clone(),
        }
    }

    /// Set the Kaspa database instance
    pub fn with_kaspa_db(mut self, kaspa_db: Arc<dyn hyperlane_core::KaspaDb>) -> Self {
        self.kaspa_db = Some(kaspa_db);
        self
    }

    /// Get a reference to the Kaspa database if set
    pub fn kaspa_db(&self) -> Option<&Arc<dyn hyperlane_core::KaspaDb>> {
        self.kaspa_db.as_ref()
    }
}

/// Updates a trait object Mailbox with kaspa_db if it's a KaspaMailbox.
/// Returns the updated mailbox as a trait object.
pub fn update_mailbox_with_db(
    mailbox: Arc<dyn Mailbox>,
    kaspa_db: Arc<dyn hyperlane_core::KaspaDb>,
) -> ChainResult<Arc<dyn Mailbox>> {
    let kas_mailbox = mailbox.downcast_arc::<KaspaMailbox>().map_err(|_| {
        hyperlane_core::ChainCommunicationError::CustomError(
            "Failed to downcast mailbox to KaspaMailbox".to_string(),
        )
    })?;

    let updated_mailbox = Arc::try_unwrap(kas_mailbox)
        .unwrap_or_else(|arc| (*arc).clone())
        .with_kaspa_db(kaspa_db);

    Ok(Arc::new(updated_mailbox))
}

impl HyperlaneChain for KaspaMailbox {
    fn domain(&self) -> &HyperlaneDomain {
        &self.domain
    }

    fn provider(&self) -> Box<dyn HyperlaneProvider> {
        Box::new(self.provider.clone())
    }
}

impl HyperlaneContract for KaspaMailbox {
    fn address(&self) -> H256 {
        self.address
    }
}

#[async_trait]
impl Mailbox for KaspaMailbox {
    async fn count(&self, _reorg_period: &ReorgPeriod) -> ChainResult<u32> {
        return Ok(0);
    }

    // Not a precise answer since actually depends on subsequent confirmation step on Kaspa,
    // so may often return false negative (says not delivered when it actually is)
    async fn delivered(&self, id: H256) -> ChainResult<bool> {
        info!("Kaspa mailbox, checking if message is delivered already (querying hub), id: {id:?}");
        let wid = WithdrawalId {
            message_id: bytes_to_hex(id.as_ref()),
        };
        let res = self
            .provider
            .hub_rpc()
            .query()
            .withdrawal_status(vec![wid], None)
            .await?;
        match res
            .status
            .first()
            .map(|s| WithdrawalStatus::try_from(*s).ok())
        {
            Some(Some(WithdrawalStatus::Processed)) => Ok(true),
            _ => Ok(false),
        }
    }

    async fn default_ism(&self) -> ChainResult<H256> {
        Ok(KASPA_ISM_ADDRESS)
    }

    async fn recipient_ism(&self, _recipient: H256) -> ChainResult<H256> {
        Ok(KASPA_ISM_ADDRESS)
    }

    async fn process(
        &self,
        _message: &HyperlaneMessage,
        _metadata: &[u8],
        _tx_gas_limit: Option<U256>,
    ) -> ChainResult<TxOutcome> {
        unimplemented!("kas does not support single message processing")
    }

    fn supports_batching(&self) -> bool {
        true
    }

    // Hijacks the batch processing flow since Kaspa uses different TX submission model than EVM chains.
    // Instead of single mailbox.process() call, we build multiple Kaspa TXs that must execute in sequence.
    async fn process_batch<'a>(&self, ops: Vec<&'a QueueOperation>) -> ChainResult<BatchResult> {
        info!(
            "Kaspa mailbox, processing/submitting kaspa batch of size: {}",
            ops.len()
        );

        let msgs: Vec<HyperlaneMessage> = ops
            .iter()
            .map(|op| op.try_batch().map(|item| item.data))
            .collect::<ChainResult<Vec<HyperlaneMessage>>>()?;

        let current_ts = std::time::Instant::now();
        {
            let mut ts_map = self.operation_timestamps.lock().await;
            for msg in &msgs {
                let msg_id = format!("{:?}", msg.id());
                ts_map.entry(msg_id).or_insert(current_ts);
            }
        }

        // Store withdrawal messages in kaspa_db before processing
        if let Some(kaspa_db) = self.kaspa_db() {
            for msg in &msgs {
                let message_id = format!("0x{:x}", msg.id());
                match kaspa_db.store_withdrawal_message(msg.clone()) {
                    Ok(()) => {
                        info!(
                            message_id = %message_id,
                            "Stored withdrawal message in kaspa_db"
                        );
                    }
                    Err(e) => {
                        error!(
                            message_id = %message_id,
                            error = ?e,
                            "Failed to store withdrawal message in kaspa_db"
                        );
                    }
                }
            }
        } else {
            error!("Kaspa mailbox, no kaspa_db set, skipping storing withdrawal messages");
        }

        // Cannot process withdrawals while a confirmation is pending on the Hub.
        // All operations marked failed and will be retried after confirmation completes.
        if self.provider.has_pending_confirmation() {
            let failed_idxs: Vec<usize> = (0..ops.len()).collect();
            return Ok(BatchResult {
                failed_indexes: failed_idxs,
                outcome: None,
            });
        }

        let res_processed = self
            .provider
            .process_withdrawal_messages(msgs.clone())
            .await;

        let processed_messages = match res_processed {
            Ok(msgs) => {
                info!("Kaspa mailbox, processed withdrawals TXs");

                // Store kaspa_tx for each successfully processed message
                if let Some(kaspa_db) = self.kaspa_db() {
                    for (msg, kaspa_tx) in &msgs {
                        if !kaspa_tx.is_empty() {
                            let message_id = msg.id();
                            match kaspa_db.store_withdrawal_kaspa_tx(&message_id, kaspa_tx) {
                                Ok(()) => {
                                    info!(
                                        message_id = ?message_id,
                                        kaspa_tx = %kaspa_tx,
                                        "Stored kaspa_tx for withdrawal"
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        error = ?e,
                                        message_id = ?message_id,
                                        kaspa_tx = %kaspa_tx,
                                        "Failed to store kaspa_tx for withdrawal"
                                    );
                                }
                            }
                        }
                    }
                }

                // Calculate and record withdrawal latency for successfully processed messages
                let now = std::time::Instant::now();
                let mut ts_map = self.operation_timestamps.lock().await;
                for (msg, _) in &msgs {
                    let msg_id = format!("{:?}", msg.id());
                    if let Some(start_ts) = ts_map.remove(&msg_id) {
                        let latency = now.duration_since(start_ts);
                        let metrics = self.provider.metrics();
                        metrics.update_withdrawal_latency(latency.as_millis() as i64);
                    }
                }
                drop(ts_map);

                // Extract just the messages for further processing
                msgs.into_iter().map(|(msg, _)| msg).collect()
            }
            Err(e) => {
                error!("Kaspa mailbox, failed to process withdrawals TXs: {:?}", e);
                Vec::new()
            }
        };

        info!("Kaspa mailbox, processed withdrawals TXs");

        // Return value doesn't correspond 1:1 to what we did since we sent multiple Kaspa TXs.
        // However, since TXs must execute in sequence, we can use the last one knowing prior ones succeeded.
        // failed_indexes indicates which hyperlane messages were NOT accepted.
        let failed_idxs = {
            let mut failed = vec![];
            for (i, msg) in msgs.iter().enumerate() {
                if !processed_messages.contains(msg) {
                    failed.push(i);
                }
            }
            error!(
                "Kaspa mailbox, processed batch, failed indexes: {:?}",
                failed
            );
            failed
        };

        Ok(BatchResult {
            outcome: Some(TxOutcome {
                transaction_id: H512::zero(),
                executed: false,
                gas_used: U256::zero(),
                gas_price: FixedPointNumber::from(0),
            }),
            failed_indexes: failed_idxs,
        })
    }

    async fn process_estimate_costs(
        &self,
        msg: &HyperlaneMessage,
        _metadata: &[u8],
    ) -> ChainResult<TxCostEstimate> {
        let token_msg = match TokenMessage::read_from(&mut msg.body.as_slice()) {
            Ok(msg) => msg,
            Err(_e) => {
                return Ok(TxCostEstimate {
                    gas_limit: 0.into(),
                    gas_price: FixedPointNumber::from(0),
                    l2_gas_limit: None,
                });
            }
        };

        if is_small_value(
            token_msg.amount().as_u64(),
            self.provider.get_min_deposit_sompi(),
        ) {
            Ok(TxCostEstimate {
                gas_limit: U256::MAX,
                gas_price: FixedPointNumber::from(u128::MAX),
                l2_gas_limit: None,
            })
        } else {
            Ok(TxCostEstimate {
                gas_limit: 0.into(),
                gas_price: FixedPointNumber::from(0),
                l2_gas_limit: None,
            })
        }
    }

    // Only used in 'lander' mode, not applicable for Kaspa bridge
    async fn process_calldata(
        &self,
        _message: &HyperlaneMessage,
        _metadata: &[u8],
    ) -> ChainResult<Vec<u8>> {
        todo!()
    }

    // Only used in 'lander' mode, not applicable for Kaspa bridge
    fn delivered_calldata(&self, _message_id: H256) -> ChainResult<Option<Vec<u8>>> {
        todo!()
    }
}
