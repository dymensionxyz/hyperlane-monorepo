use super::stats::RoundTripStats;
use corelib::user::deposit::deposit_with_payload;
use corelib::wallet::EasyKaspaWallet;
use eyre::Result;
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionId;
use std::sync::Arc;
use tokio::sync::mpsc;
use std::str::FromStr;

pub struct TaskResources {
    rpc_hub: CosmosGrpcClient,
    w: EasyKaspaWallet,
    escrow_address: Address,
}

/*
Stages
    1. Deposit using whale, to new hub user
    2. Poll for hub user balance to be credited
    3. Withdraw from hub user to a kaspa user
    4. Poll for kaspa user balance to be credited

    Measure the time gaps, and record failures
 */
pub async fn do_round_trip(
    res: Arc<TaskResources>,
    value: u64,
    tx: mpsc::Sender<RoundTripStats>,
    task_id: u64,
) {
    let mut rt = RoundTrip::new(res, value);
    rt.deposit().await;
    rt.await_hub_credit().await;
    rt.withdraw().await;
    rt.await_kaspa_credit().await;
    tx.send(rt.stats).await.unwrap();
}

struct RoundTrip {
    res: Arc<TaskResources>,
    value: u64,
    stats: RoundTripStats,
}

impl RoundTrip {
    pub fn new(res: Arc<TaskResources>, value: u64) -> Self {
        Self { res, value, stats: RoundTripStats::new() }
    }
    async fn deposit(&self) -> Result<TransactionId, String> {
        let w = &self.res.w;
        let s = &w.secret;
        let a = self.res.escrow_address.clone();
        let amt = self.value;
        let payload = vec![];
        let tx_id = deposit_with_payload(&w.wallet, &s, a, amt, payload).await?;
        Ok(tx_id)
    }

    async fn await_hub_credit(&self) -> Result<()> {
        Ok(())
    }

    async fn withdraw(&self) -> Result<()> {
        Ok(())
    }

    async fn await_kaspa_credit(&self) -> Result<()> {
        Ok(())
    }
}
