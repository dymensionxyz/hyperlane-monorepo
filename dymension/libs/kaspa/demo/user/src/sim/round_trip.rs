use super::key_cosmos::EasyHubKey;
use super::stats::RoundTripStats;
use corelib::user::deposit::deposit_with_payload;
use corelib::user::payload::make_deposit_payload_easy;
use corelib::wallet::EasyKaspaWallet;
use eyre::Result;
use hyperlane_core::H256;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionId;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct TaskResources {
    // rpc_hub: CosmosGrpcClient,
    pub w: EasyKaspaWallet,
    pub args: TaskArgs,
}

pub struct TaskArgs {
    pub domain_kas: u32,
    pub token_kas_placeholder: H256,
    pub domain_hub: u32,
    pub token_hub: H256,
    pub escrow_address: Address,
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
    tx: mpsc::Sender<RoundTripStats>,
) {
    let mut rt = RoundTrip::new(res);
    let _ = rt.deposit().await;
    let _ = rt.await_hub_credit().await;
    let _ = rt.withdraw().await;
    let _ = rt.await_kaspa_credit().await;
    tx.send(rt.stats).await.unwrap();
}

struct RoundTrip {
    res: Arc<TaskResources>,
    stats: RoundTripStats,
    hub_key: EasyHubKey,
}

impl RoundTrip {
    pub fn new(res: Arc<TaskResources>) -> Self {
        let hub_k = EasyHubKey::new();
        Self {
            res,
            stats: RoundTripStats::new(),
            hub_key: hub_k,
        }
    }

    async fn deposit(&mut self) -> Result<TransactionId> {
        let w = &self.res.w;
        let s = &w.secret;
        let a = self.res.args.escrow_address.clone();
        // let amt = self.value;
        let amt = 20000001;
        let payload = make_deposit_payload_easy(
            self.res.args.domain_kas,
            self.res.args.token_kas_placeholder,
            self.res.args.domain_hub,
            self.res.args.token_hub,
            amt,
            &self.hub_key.signer(),
        );
        let tx_id = deposit_with_payload(&w.wallet, &s, a, amt, payload).await?;
        self.stats.kaspa_deposit_tx_id = tx_id;
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

#[cfg(test)]
mod tests {
    use super::*;
}
