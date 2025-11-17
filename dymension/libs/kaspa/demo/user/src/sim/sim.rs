use super::key_cosmos::EasyHubKey;
use super::round_trip::do_round_trip;
use super::round_trip::TaskArgs;
use super::round_trip::TaskResources;
use super::stats::write_metadata;
use super::stats::StatsWriter;
use super::worker::WorkerWallet;
use chrono::{DateTime, Utc};
use corelib::api::base::RateLimitConfig;
use corelib::api::client::HttpClient;
use corelib::wallet::Network;
use eyre::Result;
use hyperlane_cosmos::ConnectionConf as CosmosConnectionConf;
use hyperlane_cosmos::{native::ModuleQueryClient, CosmosProvider};
use rand_distr::{Distribution, Exp};
use std::time::SystemTime;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::x::args::SimulateTrafficCli;
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
use hyperlane_cosmos::RawCosmosAmount;
use hyperlane_metric::prometheus_metric::PrometheusClientMetrics;
use tracing::info;
use url::Url;

pub const FIXED_TRANSFER_AMOUNT_SOMPI: u64 = 5_000_000_000;

async fn cosmos_provider(
    signer_key_hex: &str,
    rpc_url: &str,
    grpc_url: &str,
    chain_id: &str,
    prefix: &str,
    denom: &str,
    decimals: u32,
) -> Result<CosmosProvider<ModuleQueryClient>> {
    let conf = CosmosConnectionConf::new(
        vec![Url::parse(grpc_url).map_err(|e| eyre::eyre!("Invalid gRPC URL: {}", e))?],
        vec![Url::parse(rpc_url).map_err(|e| eyre::eyre!("Invalid RPC URL: {}", e))?],
        chain_id.to_string(),
        prefix.to_string(),
        denom.to_string(),
        RawCosmosAmount {
            amount: "100000000000.0".to_string(),
            denom: denom.to_string(),
        },
        32,
        OpSubmissionConfig::default(),
        NativeToken {
            decimals,
            denom: denom.to_string(),
        },
        1.0,
        None,
    )
    .map_err(|e| eyre::eyre!(e))?;
    let d = HyperlaneDomain::Known(KnownHyperlaneDomain::Osmosis);
    let locator = ContractLocator::new(&d, H256::zero());
    let hub_key = EasyHubKey::from_hex(signer_key_hex);
    let signer = Some(hub_key.signer());
    debug!("signer: {:?}", signer);
    let metrics = PrometheusClientMetrics::default();
    let chain = None;
    CosmosProvider::<ModuleQueryClient>::new(&conf, &locator, signer, metrics, chain)
        .map_err(eyre::Report::from)
}

pub struct Params {
    pub time_limit: Duration,
    pub ops_per_minute: u64,
    pub hub_fund_amount: u64,
    pub max_wait_for_cancel: Duration,
    pub simple_mode: bool,
}

impl Params {
    pub fn ops_per_second(&self) -> f64 {
        self.ops_per_minute as f64 / 60.0
    }

    pub fn distr_time(&self) -> Exp<f64> {
        Exp::new(self.ops_per_second() / 1000.0).unwrap()
    }
}

pub struct SimulateTrafficArgs {
    pub params: Params,
    pub task_args: TaskArgs,
    pub workers_dir: String,
    pub kaspa_wrpc_url: String,
    pub hub_whale_priv_key: String,
    pub output_dir: String,
    pub hub_rpc_url: String,
    pub hub_grpc_url: String,
    pub hub_chain_id: String,
    pub hub_prefix: String,
    pub hub_denom: String,
    pub hub_decimals: u32,
    pub kaspa_rest_url: String,
}

impl TryFrom<SimulateTrafficCli> for SimulateTrafficArgs {
    type Error = eyre::Error;

    fn try_from(cli: SimulateTrafficCli) -> Result<Self, Self::Error> {
        let addr = kaspa_addresses::Address::try_from(cli.escrow_address.clone())?;
        let params = Params {
            time_limit: std::time::Duration::from_secs(cli.time_limit),
            ops_per_minute: cli.ops_per_minute,
            simple_mode: cli.simple,
            hub_fund_amount: cli.hub_fund_amount,
            max_wait_for_cancel: std::time::Duration::from_secs(cli.cancel_wait),
        };

        Ok(SimulateTrafficArgs {
            params,
            task_args: TaskArgs {
                domain_kas: cli.domain_kas,
                token_kas_placeholder: cli.token_kas_placeholder,
                domain_hub: cli.domain_hub,
                token_hub: cli.token_hub,
                escrow_address: addr,
            },
            workers_dir: cli.workers_dir,
            kaspa_wrpc_url: cli.kaspa_wrpc_url,
            hub_whale_priv_key: cli.hub_whale_priv_key,
            output_dir: cli.output_dir,
            hub_rpc_url: cli.hub_rpc_url,
            hub_grpc_url: cli.hub_grpc_url,
            hub_chain_id: cli.hub_chain_id,
            hub_prefix: cli.hub_prefix,
            hub_denom: cli.hub_denom,
            hub_decimals: cli.hub_decimals,
            kaspa_rest_url: cli.kaspa_rest_url,
        })
    }
}

pub struct TrafficSim {
    params: Params,
    resources: TaskResources,
    workers_dir: String,
    wrpc_url: String,
    output_dir: String,
}

impl TrafficSim {
    pub async fn new(args: SimulateTrafficArgs) -> Result<Self> {
        let resources = TaskResources {
            args: args.task_args,
            hub: cosmos_provider(
                &args.hub_whale_priv_key,
                &args.hub_rpc_url,
                &args.hub_grpc_url,
                &args.hub_chain_id,
                &args.hub_prefix,
                &args.hub_denom,
                args.hub_decimals,
            )
            .await?,
            kas_rest: HttpClient::new(args.kaspa_rest_url.clone(), RateLimitConfig::default()),
        };
        Ok(TrafficSim {
            params: args.params,
            resources,
            workers_dir: args.workers_dir,
            wrpc_url: args.kaspa_wrpc_url,
            output_dir: args.output_dir,
        })
    }

    async fn load_workers(&self) -> Result<Vec<WorkerWallet>> {
        use std::path::Path;

        let workers_path = Path::new(&self.workers_dir);
        if !workers_path.exists() {
            return Err(eyre::eyre!(
                "Workers directory does not exist: {}",
                self.workers_dir
            ));
        }

        let entries: Vec<_> = std::fs::read_dir(workers_path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        let num_workers = entries.len();
        info!(
            "Loading {} worker wallets from {}",
            num_workers, self.workers_dir
        );

        let mut workers = Vec::new();
        for i in 0..num_workers {
            let worker = WorkerWallet::load_existing(
                i,
                self.wrpc_url.clone(),
                Network::KaspaTest10,
                &self.workers_dir,
            )
            .await?;

            workers.push(worker);

            if i > 0 && i % 10 == 0 {
                info!("Loaded {}/{} workers", i + 1, num_workers);
            }
        }

        info!("All workers loaded, starting simulation");
        Ok(workers)
    }

    fn setup_output_files(&self) -> Result<(String, String, StatsWriter)> {
        let random_filename = H256::random();
        let now = SystemTime::now();
        let datetime: DateTime<Utc> = now.into();
        let stats_file_path = format!(
            "{}/stats_{}_{}.jsonl",
            self.output_dir,
            random_filename,
            datetime.format("%Y-%m-%d_%H-%M-%S")
        );
        let metadata_file_path = format!(
            "{}/metadata_{}_{}.json",
            self.output_dir,
            random_filename,
            datetime.format("%Y-%m-%d_%H-%M-%S")
        );

        let stats_writer = StatsWriter::new(stats_file_path.clone())?;
        info!("Writing stats to {}", stats_file_path);

        Ok((stats_file_path, metadata_file_path, stats_writer))
    }

    fn spawn_stats_collector(
        stats_writer: StatsWriter,
    ) -> (
        mpsc::Sender<crate::sim::stats::RoundTripStats>,
        tokio::task::JoinHandle<u64>,
    ) {
        let (stats_tx, mut stats_rx) = mpsc::channel(100);

        let collector_handle = tokio::spawn(async move {
            let mut count = 0u64;
            while let Some(stats) = stats_rx.recv().await {
                stats_writer.log_stat(&stats);
                if let Err(e) = stats_writer.write_stat(&stats) {
                    tracing::error!("Failed to write stat: {:?}", e);
                }
                count += 1;
                if count % 10 == 0 {
                    info!("Wrote {} stats to file", count);
                }
            }
            info!("Total stats written: {}", count);
            count
        });

        (stats_tx, collector_handle)
    }

    async fn execute_simulation_loop(
        &self,
        workers: Vec<WorkerWallet>,
        stats_tx: mpsc::Sender<crate::sim::stats::RoundTripStats>,
        cancel: CancellationToken,
    ) -> Result<(u64, u64)> {
        let mut rng = rand::rng();
        let start_time = Instant::now();
        let mut total_ops = 0;
        let mut total_spend = 0;

        let mut worker_iter = workers.into_iter();

        while start_time.elapsed() < self.params.time_limit {
            let worker = match worker_iter.next() {
                Some(w) => w,
                None => {
                    info!("Ran out of pre-funded workers at {} ops", total_ops);
                    break;
                }
            };

            let tx_clone = stats_tx.clone();
            let r = self.resources.clone();
            let task_id = total_ops;
            let hub_key = EasyHubKey::new();
            fund_hub_addr(&hub_key, &r.hub, self.params.hub_fund_amount).await?;
            let cancel_token_clone = cancel.clone();

            tokio::spawn(async move {
                do_round_trip(
                    r,
                    worker,
                    FIXED_TRANSFER_AMOUNT_SOMPI,
                    &tx_clone,
                    task_id,
                    hub_key,
                    cancel_token_clone,
                )
                .await;
                drop(tx_clone);
            });

            total_spend += FIXED_TRANSFER_AMOUNT_SOMPI;
            total_ops += 1;

            let sleep_millis = self.params.distr_time().sample(&mut rng) as u64;
            tokio::time::sleep(Duration::from_millis(sleep_millis)).await;

            if total_ops % 10 == 0 {
                info!(
                    "Started {} ops, elapsed: {}s",
                    total_ops,
                    start_time.elapsed().as_secs()
                );
            }

            if self.params.simple_mode {
                break;
            }
        }

        Ok((total_spend, total_ops))
    }

    pub async fn run(&self) -> Result<()> {
        let workers = self.load_workers().await?;
        let (stats_file_path, metadata_file_path, stats_writer) = self.setup_output_files()?;
        let (stats_tx, collector_handle) = Self::spawn_stats_collector(stats_writer);

        let cancel = CancellationToken::new();
        let (total_spend, total_ops) = self
            .execute_simulation_loop(workers, stats_tx.clone(), cancel.clone())
            .await?;

        info!("Waiting for tasks to finish");
        drop(stats_tx);
        tokio::time::sleep(self.params.max_wait_for_cancel).await;
        cancel.cancel();

        let stats_count = collector_handle.await?;
        info!("Total stats collected: {}", stats_count);

        info!("Writing metadata to {}", metadata_file_path);
        write_metadata(&metadata_file_path, total_spend, total_ops)?;

        info!("Simulation complete");
        info!("Stats file: {}", stats_file_path);
        info!("Metadata file: {}", metadata_file_path);

        Ok(())
    }
}

async fn fund_hub_addr(
    hub_key: &EasyHubKey,
    hub: &CosmosProvider<ModuleQueryClient>,
    amount: u64,
) -> Result<()> {
    let hub_addr = hub_key.signer().address_string.clone();
    debug!("funding hub address: {}", hub_addr);
    let rpc = hub.rpc();
    let msg = MsgSend {
        from_address: rpc.get_signer()?.address_string.clone(),
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
            if response.check_tx.code.is_err() {
                return Err(eyre::eyre!(
                    "Transaction failed during CheckTx with code {:?}: {}",
                    response.check_tx.code,
                    response.check_tx.log
                ));
            }
            if response.tx_result.code.is_err() {
                return Err(eyre::eyre!(
                    "Transaction failed during DeliverTx with code {:?}: {}",
                    response.tx_result.code,
                    response.tx_result.log
                ));
            }
            info!("Funded hub address: {}", hub_addr);
            Ok(())
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
    #[ignore = "requires playground to be up and populated"]
    async fn test_fund_hub_addr() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
        let recipient = EasyHubKey::new();
        println!("recipient: {:?}", recipient.signer().address_string);
        let k = "7c3ea937a1578534cbe33bc22486d837436d99d0fb66cf1e5f9c9aa120e05964";
        let hub = cosmos_provider(
            &k,
            "https://rpc-dymension-playground35.mzonder.com:443",
            "https://grpc-dymension-playground35.mzonder.com:443",
            "dymension_3405-1",
            "dym",
            "adym",
            18,
        )
        .await
        .unwrap();
        fund_hub_addr(&recipient, &hub, 100).await.unwrap();
    }
}
