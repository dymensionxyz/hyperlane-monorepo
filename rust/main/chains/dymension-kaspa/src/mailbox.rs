use super::consts::*;
use crate::KaspaProvider;
use hyperlane_core::{
    utils::bytes_to_hex, BatchResult, ChainResult, ContractLocator, Decode, FixedPointNumber,
    HyperlaneChain, HyperlaneContract, HyperlaneDomain, HyperlaneMessage, HyperlaneProvider,
    Mailbox, QueueOperation, ReorgPeriod, TxCostEstimate, TxOutcome, H256, H512, U256,
};
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::{WithdrawalId, WithdrawalStatus};
use kaspa_consensus_core::tx::TransactionOutput;
use kaspa_wallet_core::tx::is_transaction_output_dust;
use tonic::async_trait;
use tracing::{debug, info, warn};

// Token message structure for parsing warp transfers
#[derive(Debug)]
struct TokenMessage {
    recipient: H256,
    amount_or_id: U256,
    metadata: Vec<u8>,
}

impl Decode for TokenMessage {
    fn read_from<R>(reader: &mut R) -> Result<Self, hyperlane_core::HyperlaneProtocolError>
    where
        R: std::io::Read,
    {
        let mut recipient = H256::zero();
        reader.read_exact(recipient.as_mut())?;

        let mut amount_or_id = [0_u8; 32];
        reader.read_exact(&mut amount_or_id)?;
        let amount_or_id = U256::from_big_endian(&amount_or_id);

        let mut metadata = vec![];
        reader.read_to_end(&mut metadata)?;

        Ok(Self {
            recipient,
            amount_or_id,
            metadata,
        })
    }
}

// pretends to be a mailbox
#[derive(Debug, Clone)]
pub struct KaspaMailbox {
    provider: KaspaProvider,
    domain: HyperlaneDomain,
    address: H256,
}

impl KaspaMailbox {
    /// new kaspa native mailbox instance
    pub fn new(provider: KaspaProvider, locator: ContractLocator) -> ChainResult<KaspaMailbox> {
        Ok(KaspaMailbox {
            provider,
            address: locator.address, // TODO: will be zero?
            domain: locator.domain.clone(),
        })
    }

    pub fn with_provider(&self, provider: KaspaProvider) -> Self {
        Self {
            provider,
            domain: self.domain.clone(),
            address: self.address,
        }
    }
}

impl HyperlaneChain for KaspaMailbox {
    /// Hardcoded // TODO: security implications?
    fn domain(&self) -> &HyperlaneDomain {
        &self.domain
    }

    fn provider(&self) -> Box<dyn HyperlaneProvider> {
        Box::new(self.provider.clone())
    }
}

impl HyperlaneContract for KaspaMailbox {
    /// Hardcoded // TODO: security implications?
    fn address(&self) -> H256 {
        self.address
    }
}

#[async_trait]
impl Mailbox for KaspaMailbox {
    // TODO: not sure where used
    // it should return the number of dispatched messages so far
    async fn count(&self, _reorg_period: &ReorgPeriod) -> ChainResult<u32> {
        return Ok(0);
    }

    // check if a message already delivered TO kaspa
    // not a precise answer since actually depends on subsequent confirmation step
    // so may often return false negative
    async fn delivered(&self, id: H256) -> ChainResult<bool> {
        info!("Kaspa mailbox, checking if message is delivered already (querying hub), id: {id:?}");
        let wid = WithdrawalId {
            message_id: bytes_to_hex(id.as_ref()),
        };
        let res = self
            .provider
            .hub_rpc()
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

    // there is no ism so return hardcode
    async fn default_ism(&self) -> ChainResult<H256> {
        Ok(KASPA_ISM_ADDRESS)
    }

    /// Get the recipient ism address
    // (Supposed to use app router to the get ISM on Kaspa which will handle a specific token contract)
    async fn recipient_ism(&self, _recipient: H256) -> ChainResult<H256> {
        Ok(KASPA_ISM_ADDRESS)
    }

    async fn process(
        &self,
        _message: &HyperlaneMessage,
        _metadata: &[u8], // contains sigs etc
        _tx_gas_limit: Option<U256>,
    ) -> ChainResult<TxOutcome> {
        /*
        There is a flow where the relayer will try to submit a batch and any failures will get retried via this method
        We should
         */
        unimplemented!("kas does not support single message processing")
    }

    /// True if the destination chain supports batching
    /// (i.e. if the mailbox contract will succeed on a `process_batch` call)
    fn supports_batching(&self) -> bool {
        true
    }

    // We hijack this https://github.com/dymensionxyz/hyperlane-monorepo/blob/4ecb864de578648e0c0ef39561f291cd7f4dfe7c/rust/main/agents/relayer/src/msg/op_submitter.rs#L1084
    async fn process_batch<'a>(&self, ops: Vec<&'a QueueOperation>) -> ChainResult<BatchResult> {
        info!(
            "Kaspa mailbox, processing/submitting kaspa batch of size: {}",
            ops.len()
        );

        if self.provider.has_pending_confirmation() {
            // All indexes are considered failed if there is a pending confirmation. they will be retried later.
            let failed_indexes: Vec<usize> = (0..ops.len()).collect();
            return Ok(BatchResult {
                failed_indexes,
                outcome: None,
            });
        }

        let messages: Vec<HyperlaneMessage> = ops
            .iter()
            .map(|op| op.try_batch().map(|item| item.data)) // TODO: please work...
            .collect::<ChainResult<Vec<HyperlaneMessage>>>()?;

        let processed_messages = self
            .provider
            .process_withdrawal_messages(messages.clone())
            .await?;
        info!("Kaspa mailbox, processed withdrawals TXs");

        // Note: this return value doesn't really correspond well to what we did, since we sent (possibly) multiple TXs to Kaspa
        // however, since the TXs must go in sequence, we can take the last one, knowing all the prior ones were accepted
        // failed indexes should say which hyperlane messages were accepted

        let failed = {
            let mut failed = vec![];
            for (i, msg) in messages.iter().enumerate() {
                if !processed_messages.contains(msg) {
                    failed.push(i);
                }
            }
            failed
        };

        Ok(BatchResult {
            outcome: Some(TxOutcome {
                transaction_id: H512::zero(),
                executed: false,
                gas_used: U256::zero(),
                gas_price: FixedPointNumber::from(0),
            }),
            failed_indexes: failed,
        })
    }

    async fn process_estimate_costs(
        &self,
        message: &HyperlaneMessage,
        _metadata: &[u8],
    ) -> ChainResult<TxCostEstimate> {
        // Try to parse the message body as a TokenMessage to get the transfer amount
        let token_msg = match TokenMessage::read_from(&mut message.body.as_slice()) {
            Ok(msg) => msg,
            Err(e) => {
                warn!(
                    "Failed to parse message body as TokenMessage: {:?}. Treating as non-dust.",
                    e
                );
                // If we can't parse it, assume it's not a warp transfer and return free gas
                return Ok(TxCostEstimate {
                    gas_limit: 0.into(),
                    gas_price: FixedPointNumber::from(0),
                    l2_gas_limit: None,
                });
            }
        };
        
        // Convert U256 to u64 for comparison
        let amount_u64 = if token_msg.amount_or_id > U256::from(u64::MAX) {
            // If amount is larger than u64::MAX, it's definitely not dust
            return Ok(TxCostEstimate {
                gas_limit: 0.into(),
                gas_price: FixedPointNumber::from(0),
                l2_gas_limit: None,
            });
        } else {
            token_msg.amount_or_id.as_u64()
        };
        
        // Get dust threshold from configuration
        let min_deposit_sompi = self.provider.get_min_deposit_sompi();
        
        // Check if the amount is dust using the same logic as hub_to_kaspa.rs
        if amount_u64 < min_deposit_sompi {
            debug!(
                "Detected dust amount in warp transfer: {} sompi (below minimum {}). Returning infinite gas cost to prevent relay.",
                amount_u64, min_deposit_sompi
            );
            // Return effectively infinite gas cost to prevent the relayer from processing this message
            Ok(TxCostEstimate {
                gas_limit: U256::MAX,
                gas_price: FixedPointNumber::from(u128::MAX),
                l2_gas_limit: None,
            })
        } else {
            // Also check using kaspa's built-in dust detection
            let tx_out = TransactionOutput::new(amount_u64, vec![].into());
            if is_transaction_output_dust(&tx_out) {
                debug!(
                    "Detected dust amount in warp transfer: {} sompi (failed Kaspa dust check). Returning infinite gas cost to prevent relay.",
                    amount_u64
                );
                return Ok(TxCostEstimate {
                    gas_limit: U256::MAX,
                    gas_price: FixedPointNumber::from(u128::MAX),
                    l2_gas_limit: None,
                });
            }
            
            debug!(
                "Warp transfer amount {} sompi is not dust. Returning zero gas cost.",
                amount_u64
            );
            // Return zero/free gas cost for non-dust amounts
            Ok(TxCostEstimate {
                gas_limit: 0.into(),
                gas_price: FixedPointNumber::from(0),
                l2_gas_limit: None,
            })
        }
    }

    // used in payload derivation: https://github.com/dymensionxyz/hyperlane-monorepo/blob/7d0ae7590decd9ea09f6c88f8eeeb49df0295e19/rust/main/agents/relayer/src/msg/pending_message.rs#L551
    // although not sure what payload is for, seems like for 'lander'
    async fn process_calldata(
        &self,
        _message: &HyperlaneMessage,
        _metadata: &[u8],
    ) -> ChainResult<Vec<u8>> {
        todo!() // we dont need this for now (original HL comment)
    }

    // again, seems for lander mode only
    fn delivered_calldata(&self, _message_id: H256) -> ChainResult<Option<Vec<u8>>> {
        todo!()
    }
}