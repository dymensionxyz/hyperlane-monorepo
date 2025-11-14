use dym_kas_relayer::KaspaBridgeMetrics;
use hyperlane_core::HyperlaneMessage;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct WithdrawalMetadata {
    pub message: HyperlaneMessage,
    pub message_id: String,
    pub amount: Option<u64>,
}

impl WithdrawalMetadata {
    pub fn from_message(msg: HyperlaneMessage) -> Self {
        let message_id = format!("{:?}", msg.id());
        let amount = dym_kas_core::message::parse_withdrawal_amount(&msg);
        Self {
            message: msg,
            message_id,
            amount,
        }
    }

    pub fn from_messages(msgs: &[HyperlaneMessage]) -> Vec<Self> {
        msgs.iter().cloned().map(Self::from_message).collect()
    }
}

pub enum WithdrawalStage {
    Initiated,
    Processed,
    Failed,
}

pub fn record_withdrawal_batch_metrics(
    metrics: &KaspaBridgeMetrics,
    metadata: &[WithdrawalMetadata],
    stage: WithdrawalStage,
) {
    match stage {
        WithdrawalStage::Initiated => {
            if !metadata.is_empty() {
                metrics.record_withdrawal_batch_size(metadata.len() as u64);
            }
            for meta in metadata {
                if let Some(amount) = meta.amount {
                    metrics.record_withdrawal_initiated(&meta.message_id, amount);
                }
            }
        }
        WithdrawalStage::Processed => {
            for meta in metadata {
                if let Some(amount) = meta.amount {
                    metrics.record_withdrawal_processed(&meta.message_id, amount);
                }
            }
        }
        WithdrawalStage::Failed => {
            for meta in metadata {
                if let Some(amount) = meta.amount {
                    metrics.record_withdrawal_failed(&meta.message_id, amount);
                }
            }
        }
    }
}

pub fn calculate_failed_indexes(
    all_msgs: &[HyperlaneMessage],
    processed_msgs: &[HyperlaneMessage],
) -> Vec<usize> {
    let processed_ids: HashSet<_> = processed_msgs.iter().map(|m| m.id()).collect();
    all_msgs
        .iter()
        .enumerate()
        .filter_map(|(i, msg)| (!processed_ids.contains(&msg.id())).then_some(i))
        .collect()
}
