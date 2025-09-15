use super::key_cosmos::EasyHubKey;
use super::round_trip::{do_deposit_phase, await_hub_credit_phase, do_withdrawal_phase, DepositData};
use super::round_trip::TaskArgs;
use super::round_trip::TaskResources;
use super::stats::render_stats;
use super::stats::write_stats;
use super::util::som_to_kas;
use chrono::{DateTime, Utc};
use corelib::api::base::RateLimitConfig;
use corelib::api::client::HttpClient;
use corelib::wallet::EasyKaspaWallet;
use corelib::wallet::{EasyKaspaWalletArgs, Network};
use eyre::Result;
use hyperlane_cosmos_native::ConnectionConf as CosmosConnectionConf;
use hyperlane_cosmos_native::CosmosNativeProvider;
use rand_distr::{Distribution, Exp};
use std::time::SystemTime;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::warn;

use crate::x::args::{SimulateTrafficCli, WalletCli};
use cosmos_sdk_proto::cosmos::bank::v1beta1::MsgSend;
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
use cosmos_sdk_proto::traits::Message;
use cosmrs::Any;
use hyperlane_core::config::OpSubmissionConfig;
use hyperlane_core::ContractLocator;
use hyperlane_core::HyperlaneDomain;
use hyperlane_core::KnownHyperlaneDomain;
use hyperlane_core::NativeToken;
use hyperlane_core::H256;
use hyperlane_cosmos_native::RawCosmosAmount;
use hyperlane_metric::prometheus_metric::PrometheusClientMetrics;
use tracing::info;
use url::Url;

const DEFAULT_RPC_URL: &str = "https://rpc-dymension-playground35.mzonder.com:443";
const DEFAULT_GRPC_URL: &str = "https://grpc-dymension-playground35.mzonder.com:443";
const DEFAULT_CHAIN_ID: &str = "dymension_3405-1";
const DEFAULT_PREFIX: &str = "dym";
const DEFAULT_DENOM: &str = "adym";
const DEFAULT_DECIMALS: u32 = 18;
const DEFAULT_WRPC_URL: &str = "api-kaspa.mzonder.com:17210";
const DEFAULT_REST_URL: &str = "https://kaspa-testnet-rest.mzonder.com/";

async fn cosmos_provider(signer_key_hex: &str) -> Result<CosmosNativeProvider> {
    let conf = CosmosConnectionConf::new(
        vec![Url::parse(DEFAULT_RPC_URL).unwrap()],
        vec![Url::parse(DEFAULT_GRPC_URL).unwrap()],
        DEFAULT_CHAIN_ID.to_string(),
        DEFAULT_PREFIX.to_string(),
        DEFAULT_DENOM.to_string(),
        RawCosmosAmount {
            amount: "100000000000.0".to_string(),
            denom: DEFAULT_DENOM.to_string(),
        },
        1.0,
        32,
        OpSubmissionConfig::default(),
        NativeToken {
            decimals: DEFAULT_DECIMALS,
            denom: DEFAULT_DENOM.to_string(),
        },
    );
    let d = HyperlaneDomain::Known(KnownHyperlaneDomain::Osmosis);
    let locator = ContractLocator::new(&d, H256::zero());
    let hub_key = EasyHubKey::from_hex(signer_key_hex);
    let signer = Some(hub_key.signer());
    debug!("signer: {:?}", signer);
    let metrics = PrometheusClientMetrics::default();
    let chain = None;
    CosmosNativeProvider::new(&conf, &locator, signer, metrics, chain).map_err(eyre::Report::from)
}

pub struct Params {
    pub time_limit: Duration,          // total target simulation time
    pub budget: u64,                   // in sompi
    pub ops_per_minute: u64,           // osmosis does 90 per minute
    pub min_value: u64,                // in sompi
    pub hub_fund_amount: u64,          // in adym
    pub max_wait_for_cancel: Duration, // max time to wait for cancel
    pub simple_mode: bool,
}

impl Params {
    /// Used to draw value of each op, in sompi
    pub fn distr_value(&self) -> Exp<f64> {
        // TODO: need to use some clamping/minimum
        Exp::new(1.0 / self.op_budget()).unwrap()
    }
    /// Sample deposit value
    pub fn sample_value(&self) -> u64 {
        if self.simple_mode {
            return self.min_value;
        }
        // TODO: use proper clamping, or this will blow the budget
        let v = self.distr_value().sample(&mut rand::rng()) as u64;
        if v < self.min_value {
            self.min_value
        } else {
            v
        }
    }
    /// Used to draw time between ops, in milliseconds
    pub fn distr_time(&self) -> Exp<f64> {
        Exp::new(self.ops_per_second() / 1000.0).unwrap()
    }
    pub fn num_ops(&self) -> f64 {
        self.time_limit.as_secs_f64() * self.ops_per_second()
    }
    pub fn op_budget(&self) -> f64 {
        self.budget as f64 / self.num_ops()
    }
    pub fn ops_per_second(&self) -> f64 {
        self.ops_per_minute as f64 / 60.0
    }
}

pub struct SimulateTrafficArgs {
    pub params: Params,
    pub task_args: TaskArgs,
    pub wallet: WalletCli,
    pub hub_whale_priv_key: String, // in hex
    pub output_dir: String,
}

impl TryFrom<SimulateTrafficCli> for SimulateTrafficArgs {
    type Error = eyre::Error;

    fn try_from(cli: SimulateTrafficCli) -> Result<Self, Self::Error> {
        let addr = kaspa_addresses::Address::try_from(cli.escrow_address.clone())?;
        Ok(SimulateTrafficArgs {
            params: Params {
                time_limit: std::time::Duration::from_secs(cli.time_limit),
                budget: cli.budget,
                ops_per_minute: cli.ops_per_minute,
                simple_mode: cli.simple,
                min_value: cli.min_deposit_sompi,
                hub_fund_amount: cli.hub_fund_amount,
                max_wait_for_cancel: std::time::Duration::from_secs(cli.cancel_wait),
            },
            task_args: TaskArgs {
                domain_kas: cli.domain_kas,
                token_kas_placeholder: cli.token_kas_placeholder,
                domain_hub: cli.domain_hub,
                token_hub: cli.token_hub,
                escrow_address: addr,
            },
            wallet: cli.wallet,
            hub_whale_priv_key: cli.hub_whale_priv_key,
            output_dir: cli.output_dir,
        })
    }
}

pub struct TrafficSim {
    params: Params,
    resources: TaskResources,
    output_dir: String,
}

impl TrafficSim {
    pub async fn new(args: SimulateTrafficArgs) -> Result<Self> {
        let w = EasyKaspaWallet::try_new(EasyKaspaWalletArgs {
            wallet_secret: args.wallet.wallet_secret,
            wrpc_url: args.wallet.rpc_url.to_string(),
            net: Network::KaspaTest10,
            storage_folder: None,
        })
        .await?;
        let resources = TaskResources {
            w: w.clone(),
            args: args.task_args,
            hub: cosmos_provider(&args.hub_whale_priv_key).await?,
            kas_rest: HttpClient::new(DEFAULT_REST_URL.to_string(), RateLimitConfig::default()),
        };
        Ok(TrafficSim {
            params: args.params,
            resources,
            output_dir: args.output_dir,
        })
    }

    pub async fn run(&self) -> Result<()> {
        let mut rng = rand::rng();
        let mut total_ops = 0;
        let mut total_spend = 0;

        let (stats_tx, mut stats_rx) = mpsc::channel(100);

        let collector_handle = tokio::spawn(async move {
            let mut collected_stats = Vec::new();
            while let Some(stats) = stats_rx.recv().await {
                collected_stats.push(stats);
            }
            collected_stats
        });

        // Pre-generate and fund hub keys before starting the simulation timer
        // Add small buffer (10%) to account for variance in exponential distribution timing
        let base_estimated_ops = self.params.num_ops() as usize;
        let estimated_ops = (base_estimated_ops as f64 * 1.1).ceil() as usize;
        info!("Pre-funding {} hub addresses for {} base estimated operations (10% buffer for timing variance)", 
              estimated_ops, base_estimated_ops);
        
        let mut pre_funded_keys = Vec::new();
        
        for i in 0..estimated_ops {
            let hub_key = EasyHubKey::new();
            let hub = self.resources.hub.clone();
            let hub_fund_amount = self.params.hub_fund_amount;
            let key_clone = hub_key.clone();
            pre_funded_keys.push(hub_key);
            
            if let Err(e) = fund_hub_addr(&key_clone, &hub, hub_fund_amount).await {
                error!("Failed to pre-fund hub address {}: {}", i, e);
                return Err(e);
            }
        }
        
        info!("Pre-funding complete, starting simulation");
        info!("Total pre-funded keys available: {}", pre_funded_keys.len());
        
        let mut key_iter = pre_funded_keys.into_iter();
        
        // Now start the actual simulation timer
        let start_time = Instant::now();
        info!("Starting deposit phase (parallel, no waiting for credit)");
        let cancel = CancellationToken::new();
        
        // Collect all task parameters first
        let mut task_params = Vec::new();
        
        while start_time.elapsed() < self.params.time_limit {
            // Check if we still have pre-funded keys available
            let hub_key = match key_iter.next() {
                Some(key) => key,
                None => {
                    warn!("Reached pre-funded address limit ({} addresses). Stopping operation generation early at {} ops.", 
                          estimated_ops, total_ops);
                    break; // Stop generating new operations when we run out of pre-funded keys
                }
            };
            
            let nominal_value = self.params.sample_value();
            let r = self.resources.clone();
            let task_id = total_ops;
            
            let hub_address = hub_key.signer().address_string.clone();
            info!("Task {} assigned hub address: {}", task_id, hub_address);
            
            task_params.push((r, nominal_value, task_id, hub_key));
            
            total_spend += nominal_value;
            total_ops += 1;
            let sleep_millis = self.params.distr_time().sample(&mut rng) as u64;
            tokio::time::sleep(Duration::from_millis(sleep_millis)).await;
            info!(
                "elapsed millis {}, interval {}, value {}",
                start_time.elapsed().as_millis(),
                sleep_millis,
                som_to_kas(nominal_value)
            );
            if self.params.simple_mode {
                break;
            }
        }
        
        // Phase 1: Execute deposits sequentially with delays (since they come from same wallet)
        let task_params_len = task_params.len();
        info!("Phase 1: Executing {} deposits sequentially with delays", task_params_len);
        let mut credit_tasks = Vec::new();
        for (i, (resources, value, task_id, hub_key)) in task_params.into_iter().enumerate() {
            let cancel_token_clone = cancel.clone();
            
            // Retry logic for deposits
            let mut retry_count = 0;
            const MAX_RETRIES: u32 = 3;
            let mut deposit_result = None;
            
            while retry_count < MAX_RETRIES {
                match do_deposit_phase(resources.clone(), value, task_id, &hub_key, cancel_token_clone.clone()).await {
                    Ok(deposit_data) => {
                        info!("Deposit {} successful (attempt {}), tx_id: {:?}", task_id, retry_count + 1, deposit_data.kaspa_deposit_tx_id);
                        deposit_result = Some(deposit_data);
                        break;
                    }
                    Err(e) => {
                        retry_count += 1;
                        if retry_count < MAX_RETRIES {
                            error!("Deposit failed for task {} (attempt {}): {:?}, retrying...", task_id, retry_count, e);
                            // Wait before retry to let wallet state settle
                            tokio::time::sleep(Duration::from_millis(1500)).await;
                        } else {
                            error!("Deposit failed for task {} after {} attempts: {:?}", task_id, MAX_RETRIES, e);
                        }
                    }
                }
            }
            
            if let Some(deposit_data) = deposit_result {
                credit_tasks.push((resources, value, task_id, hub_key, Some(deposit_data)));
            } else {
                credit_tasks.push((resources, value, task_id, hub_key, None));
            }
            
            // Add delay between deposits to let wallet UTXO set update
            // Skip delay after last deposit
            if i < task_params_len - 1 {
                info!("Waiting 1.5 seconds before next deposit to let wallet state update...");
                tokio::time::sleep(Duration::from_millis(1500)).await;
            }
        }
        
        // Phase 2: Wait for all hub credits sequentially to avoid RPC subscription limit
        let credit_tasks_len = credit_tasks.len();
        info!("Phase 2: Waiting for {} hub credits sequentially to avoid RPC limits", credit_tasks_len);
        let mut withdrawal_tasks = Vec::new();
        
        // Process credits completely sequentially to avoid any RPC connection issues
        for (i, (resources, value, task_id, hub_key, deposit_data)) in credit_tasks.into_iter().enumerate() {
            if let Some(deposit_data) = deposit_data {
                let cancel_token_clone = cancel.clone();
                match await_hub_credit_phase(resources.clone(), value, task_id, &hub_key, deposit_data, cancel_token_clone).await {
                    Ok(withdrawal_data) => {
                        info!("Hub credit confirmed for task {} ({}/{})", task_id, i + 1, credit_tasks_len);
                        withdrawal_tasks.push((resources, value, task_id, hub_key, Some(withdrawal_data)));
                    }
                    Err(e) => {
                        error!("Hub credit wait failed for task {}: {:?}", task_id, e);
                        withdrawal_tasks.push((resources, value, task_id, hub_key, None));
                    }
                }
                
                // Small delay between credit checks to be gentle on RPC
                tokio::time::sleep(Duration::from_millis(200)).await;
            } else {
                // Deposit failed earlier, skip credit check
                withdrawal_tasks.push((resources, value, task_id, hub_key, None));
            }
        }
        
        // Phase 3: Execute withdrawals with staggered startup (independent execution)
        info!("Phase 3: Executing {} withdrawals with staggered startup", withdrawal_tasks.len());

        // Start all withdrawals independently with a small stagger to avoid overwhelming RPC
        let mut withdrawal_handles = Vec::new();

        for (i, (resources, value, task_id, hub_key, withdrawal_data)) in withdrawal_tasks.into_iter().enumerate() {
            let tx_clone = stats_tx.clone();
            let cancel_token_clone = cancel.clone();

            let handle = tokio::spawn(async move {
                // Small staggered startup delay to avoid RPC burst
                if i > 0 {
                    let delay_ms = (i * 200) as u64; // 200ms stagger between each withdrawal start
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }

                do_withdrawal_phase(
                    resources,
                    value,
                    &tx_clone,
                    task_id,
                    hub_key,
                    withdrawal_data,
                    cancel_token_clone,
                )
                .await;
                drop(tx_clone);
            });
            withdrawal_handles.push(handle);
        }

        // Wait for all withdrawals to complete independently
        info!("All {} withdrawals started, waiting for completion...", withdrawal_handles.len());
        for (i, handle) in withdrawal_handles.into_iter().enumerate() {
            if let Err(e) = handle.await {
                error!("Withdrawal task {} failed: {:?}", i, e);
            }
        }
        
        info!("All tasks completed");

        drop(stats_tx);
        //tokio::time::sleep(self.params.max_wait_for_cancel).await; // TODO: should only wait if need to
        cancel.cancel();

        let final_stats = collector_handle.await?;
        render_stats(final_stats.clone(), total_spend, total_ops);

        let random_filename = H256::random();
        let now = SystemTime::now();
        let datetime: DateTime<Utc> = now.into();
        let file_path = format!(
            "{}/stats_{}_{}.json",
            self.output_dir,
            random_filename,
            datetime.format("%Y-%m-%d_%H-%M-%S")
        );
        info!("Writing stats to {}", file_path);
        write_stats(&file_path, final_stats, total_spend, total_ops);

        Ok(())
    }
}

async fn fund_hub_addr(
    hub_key: &EasyHubKey,
    hub: &CosmosNativeProvider,
    amount: u64,
) -> Result<()> {
    let hub_addr = hub_key.signer().address_string.clone();

    let rpc = hub.rpc();

    let from_address = rpc.get_signer()?.address_string.clone();
    //let from_address = "dym1f79cr4r2v34arp9kfafw8ala8qhkpmdtx2zghc".to_string(); // hardcode whale
    info!("funding hub address: {} from {}", hub_addr,from_address);
    let msg = MsgSend {
        from_address: from_address,
        to_address: hub_addr.clone(),
        amount: vec![Coin {
            amount: amount.to_string(),
            denom: "adym".to_string(),
        }],
    };
    let a = Any {
        type_url: "/cosmos.bank.v1beta1.MsgSend".to_string(),
        value: msg.encode_to_vec(),
    };
    let gas_limit = None;
    let response = rpc.send(vec![a], gas_limit).await;
    match response {
        Ok(response) => {
            // Check check_tx for errors first (mempool validation)
            if response.check_tx.code.is_err() {
                return Err(eyre::eyre!(
                    "Transaction failed during CheckTx with code {:?}: {}",
                    response.check_tx.code,
                    response.check_tx.log
                ));
            }
            // Then check tx_result for execution errors
            if response.tx_result.code.is_err() {
                return Err(eyre::eyre!(
                    "Transaction failed during DeliverTx with code {:?}: {}",
                    response.tx_result.code,
                    response.tx_result.log
                ));
            }
            info!("Funded hub address: {}", hub_addr);
            Ok(())
            /*if response.tx_result.code.is_ok() {
                info!("Funded hub address: {}", hub_addr);
                Ok(())
            } else {
                Err(eyre::eyre!(
                    "Failed to fund hub address, non success code: {:?}",
                    response.tx_result.code
                ))
            }*/
        }
        Err(e) => Err(eyre::eyre!("Failed to fund hub address: {:?}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperlane_core::H256;

    #[test]
    fn test_h256_random_stringify() {
        let h = H256::random();
        let s = format!("{:?}", h);
        println!("s: {}", s);
    }

    #[tokio::test]
    async fn test_fund_hub_addr() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
        let recipient = EasyHubKey::new();
        println!("recipient: {:?}", recipient.signer().address_string);
        let k = "7c3ea937a1578534cbe33bc22486d837436d99d0fb66cf1e5f9c9aa120e05964";
        let hub = cosmos_provider(&k).await.unwrap();
        fund_hub_addr(&recipient, &hub, 100).await.unwrap();
    }
}
