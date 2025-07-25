use crate::sim::util::som_to_kas;
use kaspa_consensus_core::tx::TransactionId;
use std::time::Instant;
use tracing::info;

pub fn render_stats(stats: Vec<RoundTripStats>, total_spend: u64, total_ops: u64) {
    info!("Total spend: {}", som_to_kas(total_spend));
    info!("Total ops: {}", total_ops);
    for s in stats {
        info!("op_id: {}", s.op_id);
        info!("kaspa_deposit_tx_id: {:?}", s.kaspa_deposit_tx_id);
        info!("deposit_time: {:?}", s.deposit_time);
    }
}

#[derive(Debug, Clone, Default)]
pub struct RoundTripStats {
    pub op_id: u64,
    pub kaspa_deposit_tx_id: Option<TransactionId>,
    pub deposit_time: Option<Instant>,
}

impl RoundTripStats {
    pub fn new(op_id: u64) -> Self {
        let mut d = RoundTripStats::default();
        d.op_id = op_id;
        d
    }
}