use crate::sim::util::som_to_kas;
use eyre::Error;
use kaspa_consensus_core::tx::TransactionId;
use std::time::Instant;
use tracing::info;

pub fn render_stats(stats: Vec<RoundTripStats>, total_spend: u64, total_ops: u64) {
    info!("Total spend: {}", som_to_kas(total_spend));
    info!("Total ops: {}", total_ops);
    for s in stats {
        info!("{:#?}", s);
        info!("stage: {:?}", s.stage());
    }
}

#[derive(Debug, Clone, Default)]
pub struct RoundTripStats {
    pub op_id: u64,
    pub kaspa_deposit_tx_id: Option<TransactionId>,
    pub deposit_time: Option<Instant>,
    pub deposit_credit_error: Option<String>,
}


#[derive(Debug, Clone, Copy)]
enum Stage {
    PreDeposit,
    PostDepositNotCredited,
    PreWithdrawal,
    PostWithdrawalNotCredited,
}

impl RoundTripStats {
    pub fn new(op_id: u64) -> Self {
        let mut d = RoundTripStats::default();
        d.op_id = op_id;
        d
    }
    pub fn stage(&self) -> Stage {
        if !self.deposit_time.is_some() {
            return Stage::PreDeposit;
        }
        if self.deposit_credit_error.is_some() {
            return Stage::PostDepositNotCredited;
        }
        Stage::PreWithdrawal
    }
}
