use super::stats::RoundTripStats;
use corelib::user::deposit::deposit_with_payload;
use corelib::user::payload::make_deposit_payload_easy;
use corelib::wallet::EasyKaspaWallet;
use cosmrs::crypto::secp256k1::SigningKey;
use eyre::Result;
use hyperlane_core::H256;
use hyperlane_cosmos_native::signers::Signer;
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use k256::ecdsa::SigningKey as K256SigningKey;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionId;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use hyperlane_core::AccountAddressType;
use rand_core::OsRng;

pub struct TaskResources {
    rpc_hub: CosmosGrpcClient,
    w: EasyKaspaWallet,
    args: TaskArgs,
}

pub struct TaskArgs {
    pub domain_kas: u32,
    pub token_kas_placeholder: H256,
    pub domain_hub: u32,
    pub token_hub: H256,
    pub hub_user_addr_hub: H256,
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
    hub_key: K256SigningKey,
}

impl RoundTrip {
    pub fn new(res: Arc<TaskResources>, value: u64) -> Self {
        let hub_k = K256SigningKey::random(&mut OsRng);
        Self {
            res,
            value,
            stats: RoundTripStats::new(),
            hub_key: hub_k,
        }
    }
    fn hub_signer(&self) -> Signer {
        let priv_k = self.hub_key.to_bytes().to_vec();
        Signer::new(priv_k, "dym".to_string(), &AccountAddressType::Ethereum).unwrap()
    }

    async fn deposit(&self) -> Result<TransactionId, String> {
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
            &self.hub_signer(),
        );
        let tx_id = deposit_with_payload(&w.wallet, &s, a, amt, payload)
            .await
            .map_err(|e| e.to_string())?;
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
