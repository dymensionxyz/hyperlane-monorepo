use super::key_cosmos::EasyHubKey;
use super::stats::RoundTripStats;
use crate::x;
use corelib::user::deposit::deposit_with_payload;
use corelib::user::payload::make_deposit_payload_easy;
use corelib::wallet::EasyKaspaWallet;
use cosmrs::Any;
use eyre::Result;
use hyperlane_core::ContractLocator;
use hyperlane_core::HyperlaneDomain;
use hyperlane_core::KnownHyperlaneDomain;
use hyperlane_core::H256;
use hyperlane_core::U256;
use hyperlane_cosmos_native::remote_transfer::CosmosNativeRemoteTransfer;
use hyperlane_cosmos_native::CosmosNativeProvider;
use hyperlane_cosmos_rs::hyperlane::warp::v1::MsgRemoteTransfer;
use hyperlane_cosmos_rs::prost::{Message, Name};
use hyperlane_cosmos_rs::prost::cosmos_sdk::v1::Coin;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionId;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tendermint::abci::types::Code;
use tendermint::hash::Hash as TendermintHash;
use tokio::sync::mpsc;
use tracing::error;

#[derive(Debug, Clone)]
pub struct TaskResources {
    pub hub: CosmosNativeProvider,
    pub w: EasyKaspaWallet,
    pub args: TaskArgs,
}

#[derive(Debug, Clone)]
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
    res: TaskResources,
    value: u64,
    tx: &mpsc::Sender<RoundTripStats>,
    task_id: u64,
    hub_key: EasyHubKey,
) {
    let mut rt = RoundTrip::new(res, value, task_id, hub_key);
    let (tx_id, deposit_time) = match rt.deposit().await {
        Ok((tx_id, deposit_time)) => (tx_id, deposit_time),
        Err(e) => {
            error!("deposit failed: {:?}", e);
            return;
        }
    };
    rt.stats.kaspa_deposit_tx_id = Some(tx_id);
    rt.stats.kaspa_deposit_tx_time = Some(deposit_time);
    match rt.await_hub_credit().await {
        Ok(()) => (),
        Err(e) => {
            rt.stats.deposit_credit_error = Some(e.to_string());
            return;
        }
    };
    match rt.withdraw().await {
        Ok((tx_id, withdrawal_time)) => {
            rt.stats.hub_withdraw_tx_id = Some(tx_id);
            rt.stats.hub_withdraw_tx_time = Some(withdrawal_time);
        }
        Err(e) => {
            error!("withdrawal failed: {:?}", e);
            return;
        }
    };
    match rt.await_kaspa_credit().await {
        Ok(()) => (),
        Err(e) => {
            rt.stats.withdraw_credit_error = Some(e.to_string());
            return;
        }
    };
    tx.send(rt.stats).await.unwrap();
}

struct RoundTrip {
    res: TaskResources,
    value: u64,
    task_id: u64,
    stats: RoundTripStats,
    hub_key: EasyHubKey,
}

impl RoundTrip {
    pub fn new(res: TaskResources, value: u64, task_id: u64, hub_k: EasyHubKey) -> Self {
        res.hub.rpc().set_signer(hub_k.signer());
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

    async fn withdraw(&self) -> Result<(TendermintHash, Instant)> {
        let rpc = self.res.hub.rpc();

        let d = HyperlaneDomain::Known(KnownHyperlaneDomain::Osmosis);
        let l = ContractLocator::new(&d, H256::zero());
        let rtc = CosmosNativeRemoteTransfer::new(self.res.hub.clone(), l);
        let amount = self.value.to_string();
        let recipient = x::addr::hl_recipient(&self.res.args.escrow_address.clone().to_string());
        let req = MsgRemoteTransfer {
            sender: rpc.get_signer()?.address_string.clone(),
            token_id: self.res.args.token_hub.to_string(),
            destination_domain: self.res.args.domain_hub,
            recipient,
            amount,
            custom_hook_id: "".to_string(),
            gas_limit: "".to_string(),
            max_fee: Some(Coin {
                denom: "adym".to_string(),
                amount: "1000".to_string(),
            }),
            custom_hook_metadata: "".to_string(),
        };
        let a = Any {
            type_url: MsgRemoteTransfer::type_url(),
            value: req.encode_to_vec(),
        };
        let gas_limit = None;
        let response = rpc.send(vec![a], gas_limit).await?;
        let i = Instant::now();
        match response.tx_result.code {
            Code::Ok => {
                let tx_id = response.hash.clone();
                Ok((tx_id, i))
            }
            _ => Err(RoundTripError::WithdrawalTxFailed.into()),
        }
    }

    async fn await_kaspa_credit(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RoundTripError {
    #[error("hub balance mismatch: {balance} != {expected}")]
    HubBalanceMismatch { balance: U256, expected: U256 },
    #[error("withdrawal tx fail")]
    WithdrawalTxFailed,
}

#[cfg(test)]
mod tests {}
