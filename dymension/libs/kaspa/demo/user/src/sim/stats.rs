use kaspa_consensus_core::tx::TransactionId;

pub fn render_stats(stats: Vec<RoundTripStats>, total_spend: u64, total_ops: u64) {
    for s in stats {
        println!("kaspa_deposit_tx_id: {}", s.kaspa_deposit_tx_id);
    }
}

#[derive(Debug, Clone, Default)]
pub struct RoundTripStats {
    pub kaspa_deposit_tx_id: TransactionId,
}

impl RoundTripStats {
    pub fn new() -> Self {
        Self {
            kaspa_deposit_tx_id: TransactionId::default(),
        }
    }
}
