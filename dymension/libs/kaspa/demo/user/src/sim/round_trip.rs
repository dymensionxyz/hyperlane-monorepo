use super::key_cosmos::EasyHubKey;
use super::stats::RoundTripStats;
use corelib::user::deposit::deposit_with_payload;
use corelib::user::payload::make_deposit_payload_easy;
use corelib::wallet::EasyKaspaWallet;
use eyre::Result;
use hyperlane_core::H256;
use hyperlane_core::U256;
use hyperlane_cosmos_native::CosmosNativeProvider;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionId;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use hyperlane_cosmos_native::remote_transfer::CosmosNativeRemoteTransfer;
use hyperlane_core::ContractLocator;
use tokio::sync::mpsc;
use tracing::error;
use hyperlane_core::HyperlaneDomain;
use hyperlane_core::KnownHyperlaneDomain;
use hyperlane_cosmos_rs::hyperlane::warp::v1::MsgRemoteTransfer;

pub struct TaskResources {
    pub hub: CosmosNativeProvider,
    pub w: EasyKaspaWallet,
    pub args: TaskArgs,
}

pub struct TaskArgs {
    pub domain_kas: u32,
    pub token_kas_placeholder: H256,
    pub domain_hub: u32,
    pub token_hub: H256,
    pub escrow_address: Address,
    pub hl_token_denom: String,
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
    tx: &mpsc::Sender<RoundTripStats>,
    task_id: u64,
) {
    let mut rt = RoundTrip::new(res, value, task_id);
    let (tx_id, deposit_time) = match rt.deposit().await {
        Ok((tx_id, deposit_time)) => (tx_id, deposit_time),
        Err(e) => {
            error!("deposit failed: {:?}", e);
            return;
        }
    };
    rt.stats.kaspa_deposit_tx_id = Some(tx_id);
    rt.stats.deposit_time = Some(deposit_time);
    match rt.await_hub_credit().await {
        Ok(()) => (),
        Err(e) => {
            rt.stats.deposit_credit_error = Some(e.to_string());
            return;
        }
    }
    rt.withdraw().await;
    rt.await_kaspa_credit().await;
    tx.send(rt.stats).await.unwrap();
}

struct RoundTrip {
    res: Arc<TaskResources>,
    value: u64,
    task_id: u64,
    stats: RoundTripStats,
    hub_key: EasyHubKey,
}

impl RoundTrip {
    pub fn new(res: Arc<TaskResources>, value: u64, task_id: u64) -> Self {
        let hub_k = EasyHubKey::new();
        Self {
            res,
            value,
            stats: RoundTripStats::new(task_id),
            hub_key: hub_k,
            task_id,
        }
    }

    async fn deposit(&mut self) -> Result<(TransactionId, Instant)> {
        let w = &self.res.w;
        let s = &w.secret;
        let a = self.res.args.escrow_address.clone();
        let amt = self.value;
        let payload = make_deposit_payload_easy(
            self.res.args.domain_kas,
            self.res.args.token_kas_placeholder,
            self.res.args.domain_hub,
            self.res.args.token_hub,
            amt,
            &self.hub_key.signer(),
        );
        let tx_id = deposit_with_payload(&w.wallet, &s, a, amt, payload).await?;
        Ok((tx_id, Instant::now()))
    }

    async fn await_hub_credit(&mut self) -> Result<()> {
        let a = self.hub_key.signer().address_string;
        loop {
            let balance = self
                .res
                .hub
                .rpc()
                .get_balance_denom(a.clone(), self.res.args.hl_token_denom.clone())
                .await?;
            if balance == U256::from(0) {
                // TODO: should avoid looping forever
                tokio::time::sleep(Duration::from_millis(1000)).await;
                continue;
            }
            if balance != U256::from(self.value) {
                let e = RoundTripError::HubBalanceMismatch {
                    balance,
                    expected: U256::from(self.value),
                };
                return Err(e.into());
            }
            break;
        }

        Ok(())
    }

    async fn withdraw(&self) -> Result<()> {
        
        let d = HyperlaneDomain::Known(KnownHyperlaneDomain::Osmosis);
        let l = ContractLocator::new(&d, H256::zero());
        let rtc = CosmosNativeRemoteTransfer::new(self.res.hub.clone(), l);
        let req = MsgRemoteTransfer {
            sender: self.hub_key.signer().address_string.clone(),
            token_id: self.res.args.hl_token_denom.clone(),
            destination_domain: self.res.args.domain_hub,
            recipient: self.res.args.escrow_address.clone(),
        };
        Ok(())
    }

    async fn await_kaspa_credit(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RoundTripError {
    #[error("hub balance mismatch: {balance} != {expected}")]
    HubBalanceMismatch { balance: U256, expected: U256 },
}

#[cfg(test)]
mod tests {}
