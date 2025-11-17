use super::key_kaspa::get_kaspa_keypair;
use super::stats::RoundTripStats;
use super::worker::Worker;
use crate::x;
use cometbft::Hash as HubHash;
use cometbft_rpc::endpoint::broadcast::tx_commit::Response as HubResponse;
use corelib::api::client::HttpClient;
use corelib::user::payload::make_deposit_payload_easy;
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
use cosmrs::Any;
use eyre::Result;
use hyperlane_core::H256;
use hyperlane_core::U256;
use hyperlane_cosmos::{native::ModuleQueryClient, CosmosProvider};
use hyperlane_cosmos_rs::hyperlane::warp::v1::MsgRemoteTransfer;
use hyperlane_cosmos_rs::prost::{Message, Name};
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionId;
use std::time::Duration;
use std::{collections::HashMap, hash::Hash};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::info;
// use ethers::utils::hex::ToHex;
use hex::ToHex;

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

#[derive(Debug, Clone)]
pub struct TaskResources {
    pub hub: CosmosProvider<ModuleQueryClient>,
    pub args: TaskArgs,
    pub kas_rest: HttpClient,
}

#[derive(Debug, Clone)]
pub struct TaskArgs {
    pub domain_kas: u32,
    pub token_kas_placeholder: H256,
    pub domain_hub: u32,
    pub token_hub: H256,
    pub escrow_address: Address,
}

impl TaskArgs {
    pub fn token_hub_str(&self) -> String {
        format!("0x{}", hex::encode(self.token_hub.as_bytes()))
    }
    pub fn hub_denom(&self) -> String {
        format!("hyperlane/{}", self.token_hub_str())
    }
}

/*
Stages
    1. Deposit using worker kaspa wallet to worker hub account
    2. Poll for hub user balance to be credited
    3. Withdraw from hub user to a kaspa user
    4. Poll for kaspa user balance to be credited

    Measure the time gaps, and record failures
 */
pub async fn do_round_trip(
    res: TaskResources,
    worker: Worker,
    value: u64,
    tx: &mpsc::Sender<RoundTripStats>,
    task_id: u64,
    cancel_token: CancellationToken,
) {
    let mut rt = RoundTrip::new(res, worker, value, task_id, cancel_token, tx);
    do_round_trip_inner(&mut rt).await;
}

async fn do_round_trip_inner(rt: &mut RoundTrip<'_>) {
    let hub_addr = rt.worker.hub_key.signer().address_string.clone();
    rt.stats.deposit_addr_hub = Some(hub_addr.clone());

    debug!(
        "round trip started: task_id={} worker_id={} hub_addr={} value={}",
        rt.task_id, rt.worker.worker_id, hub_addr, rt.value
    );

    match rt.deposit().await {
        Ok((tx_id, deposit_time_millis)) => {
            rt.stats.kaspa_deposit_tx_id = Some(tx_id);
            rt.stats.kaspa_deposit_tx_time_millis = Some(deposit_time_millis);
            info!(
                "deposit completed: task_id={} worker_id={} hub_addr={} kas_receive_addr={} kas_change_addr={} tx_id={:?}",
                rt.task_id,
                rt.worker.worker_id,
                hub_addr,
                rt.worker.receive_address().unwrap(),
                rt.worker.change_address().unwrap(),
                tx_id
            );
            rt.send_stats().await;
        }
        Err(e) => {
            error!(
                "deposit error: task_id={} worker_id={} error={:?}",
                rt.task_id, rt.worker.worker_id, e
            );
            rt.send_stats().await;
            return;
        }
    };
    match rt.await_hub_credit().await {
        Ok(()) => {
            rt.stats.deposit_credit_time_millis = Some(now_millis());
            info!(
                "hub credit received: task_id={} worker_id={} hub_addr={}",
                rt.task_id,
                rt.worker.worker_id,
                rt.worker.hub_key.signer().address_string
            );
            rt.send_stats().await;
        }
        Err(e) => {
            rt.stats.deposit_credit_error = Some(e.to_string());
            error!(
                "hub credit error: task_id={} worker_id={} error={}",
                rt.task_id, rt.worker.worker_id, e
            );
            rt.send_stats().await;
            return;
        }
    };

    let withdraw_res = rt.withdraw().await;
    if !withdraw_res.is_ok() {
        let e = withdraw_res.err().unwrap();
        error!(
            "withdrawal error: task_id={} worker_id={} error={:?}",
            rt.task_id, rt.worker.worker_id, e
        );
        rt.send_stats().await;
        return;
    }
    let (kaspa_addr, tx_id, withdrawal_time_millis) = withdraw_res.unwrap();
    rt.stats.hub_withdraw_tx_id = Some(tx_id.clone());
    rt.stats.hub_withdraw_tx_time_millis = Some(withdrawal_time_millis);
    rt.stats.withdraw_addr_kaspa = Some(kaspa_addr.clone());
    rt.send_stats().await;

    match rt.await_kaspa_credit(kaspa_addr.clone()).await {
        Ok(()) => {
            rt.stats.withdraw_credit_time_millis = Some(now_millis());
            info!(
                "kaspa credit received: task_id={} worker_id={} hub_addr={} kaspa_addr={}",
                rt.task_id,
                rt.worker.worker_id,
                rt.worker.hub_key.signer().address_string,
                kaspa_addr
            );
            rt.send_stats().await;
        }
        Err(e) => {
            rt.stats.withdraw_credit_error = Some(e.to_string());
            error!(
                "kaspa credit error: task_id={} worker_id={} error={}",
                rt.task_id, rt.worker.worker_id, e
            );
            rt.send_stats().await;
            return;
        }
    };
}

struct RoundTrip<'a> {
    res: TaskResources,
    worker: Worker,
    value: u64,
    task_id: u64,
    stats: RoundTripStats,
    cancel: CancellationToken,
    tx: &'a mpsc::Sender<RoundTripStats>,
}

impl<'a> RoundTrip<'a> {
    pub fn new(
        res: TaskResources,
        worker: Worker,
        value: u64,
        task_id: u64,
        cancel_token: CancellationToken,
        tx: &'a mpsc::Sender<RoundTripStats>,
    ) -> Self {
        let mut res = res.clone();
        res.hub.rpc = res.hub.rpc().with_signer(worker.hub_key.signer());
        Self {
            res,
            worker,
            value,
            stats: RoundTripStats::new(task_id, value),
            task_id,
            cancel: cancel_token,
            tx,
        }
    }

    async fn send_stats(&self) {
        if let Err(e) = self.tx.send(self.stats.clone()).await {
            error!(
                "stat send error: task_id={} worker_id={} error={:?}",
                self.task_id, self.worker.worker_id, e
            );
        }
    }

    async fn deposit(&self) -> Result<(TransactionId, u128)> {
        let a = self.res.args.escrow_address.clone();
        let amt = self.value;
        debug!(
            "deposit starting: task_id={} worker_id={} escrow_addr={} amount={}",
            self.task_id, self.worker.worker_id, a, amt
        );
        let payload = make_deposit_payload_easy(
            self.res.args.domain_kas,
            self.res.args.token_kas_placeholder,
            self.res.args.domain_hub,
            self.res.args.token_hub,
            amt,
            &self.worker.hub_key.signer(),
        );
        let tx_id = self.worker.deposit_with_payload(a, amt, payload).await?;
        Ok((tx_id, now_millis()))
    }

    async fn await_hub_credit(&self) -> Result<()> {
        let a = self.worker.hub_key.signer().address_string;
        debug!(
            "await hub credit starting: task_id={} worker_id={} hub_addr={} expected_value={}",
            self.task_id, self.worker.worker_id, a, self.value
        );
        loop {
            let balance = self
                .res
                .hub
                .rpc()
                .get_balance_denom(a.clone(), "adym".to_string())
                .await?;
            if balance == U256::from(0) {
                if self.cancel.is_cancelled() {
                    return Err(RoundTripError::Cancelled.into());
                }
                tokio::time::sleep(Duration::from_millis(1000)).await;
                continue;
            }
            break;
        }
        loop {
            let balance = self
                .res
                .hub
                .rpc()
                .get_balance_denom(a.clone(), self.res.args.hub_denom())
                .await?;
            if balance == U256::from(0) {
                if self.cancel.is_cancelled() {
                    return Err(RoundTripError::Cancelled.into());
                }
                tokio::time::sleep(Duration::from_millis(1000)).await;
                continue;
            }
            if balance != U256::from(self.value) {
                let e = RoundTripError::HubBalanceMismatch {
                    balance: balance.as_u64() as i64,
                    expected: self.value as i64,
                };
                return Err(e.into());
            }
            break;
        }

        Ok(())
    }

    async fn withdraw(&self) -> Result<(Address, String, u128)> {
        let kaspa_recipient = get_kaspa_keypair();
        debug!(
            "withdraw starting: task_id={} worker_id={} kaspa_recipient_addr={} amount={}",
            self.task_id, self.worker.worker_id, kaspa_recipient.address, self.value
        );
        let rpc = self.res.hub.rpc();

        let amount = self.value.to_string();
        let recipient = x::addr::hl_recipient(&kaspa_recipient.address.to_string());
        let token_id = self.res.args.token_hub_str();

        let req = MsgRemoteTransfer {
            sender: rpc.get_signer()?.address_string.clone(),
            token_id,
            destination_domain: self.res.args.domain_kas,
            recipient,
            amount,
            custom_hook_id: "".to_string(),
            gas_limit: "0".to_string(),
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
        let response = rpc.send(vec![a], gas_limit).await;
        match response {
            Ok(response) => {
                if response.tx_result.code.is_ok() & response.check_tx.code.is_ok() {
                    Ok((
                        kaspa_recipient.address,
                        hub_tx_query_id(&response),
                        now_millis(),
                    ))
                } else {
                    Err(RoundTripError::WithdrawalTxFailed { response }.into())
                }
            }
            Err(e) => Err(eyre::eyre!("Failed to withdraw: {:?}", e)),
        }
    }

    async fn await_kaspa_credit(&self, kaspa_addr: Address) -> Result<()> {
        debug!(
            "await kaspa credit starting: task_id={} worker_id={} kaspa_addr={} expected_value={}",
            self.task_id, self.worker.worker_id, kaspa_addr, self.value
        );
        loop {
            let balance = self
                .res
                .kas_rest
                .get_balance_by_address(&kaspa_addr.to_string())
                .await?;
            if balance == 0 {
                if self.cancel.is_cancelled() {
                    return Err(RoundTripError::Cancelled.into());
                }
                tokio::time::sleep(Duration::from_millis(1000)).await;
                continue;
            }
            if balance != self.value as i64 {
                let e = RoundTripError::KaspaBalanceMismatch {
                    balance,
                    expected: self.value as i64,
                };
                return Err(e.into());
            }
            break;
        }

        Ok(())
    }
}

fn hub_tx_query_id(response: &HubResponse) -> String {
    let asH256 = H256::from_slice(response.hash.as_bytes()).into();
    let tx_hash = hyperlane_cosmos::native::h512_to_h256(asH256).encode_hex_upper::<String>();
    tx_hash
}

#[derive(Debug, thiserror::Error)]
pub enum RoundTripError {
    #[error("hub balance mismatch: {balance} != {expected}")]
    HubBalanceMismatch { balance: i64, expected: i64 },
    #[error("kaspa balance mismatch: {balance} != {expected}")]
    KaspaBalanceMismatch { balance: i64, expected: i64 },
    #[error("withdrawal tx failed: {response:?}")]
    WithdrawalTxFailed { response: HubResponse },
    #[error("cancelled")]
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::TaskArgs;
    use hyperlane_core::H256;
    use kaspa_addresses::Address;
    use std::str::FromStr;

    #[test]
    fn test_hub_denom() {
        let token_hub =
            H256::from_str("0x726f757465725f61707000000000000000000000000000020000000000000000")
                .unwrap();
        let args = TaskArgs {
            domain_kas: 0,
            token_kas_placeholder: H256::zero(),
            domain_hub: 0,
            token_hub,
            escrow_address: Address::try_from(
                "kaspatest:pzlq49spp66vkjjex0w7z8708f6zteqwr6swy33fmy4za866ne90v7e6pyrfr",
            )
            .unwrap(),
        };
        let denom = args.hub_denom();
        assert_eq!(
            denom,
            "hyperlane/0x726f757465725f61707000000000000000000000000000020000000000000000"
        );
    }
}
