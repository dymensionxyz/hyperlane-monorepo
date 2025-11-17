use super::round_trip::do_round_trip;
use super::round_trip::TaskArgs;
use super::round_trip::TaskResources;
use super::stats::write_metadata;
use super::stats::StatsWriter;
use super::worker::Worker;
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

use crate::x::args::SimulateTrafficCli;
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

pub const FIXED_TRANSFER_AMOUNT_SOMPI: u64 = 4100000000;

async fn cosmos_provider(
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
    let signer = None;
    let metrics = PrometheusClientMetrics::default();
    let chain = None;
    CosmosProvider::<ModuleQueryClient>::new(&conf, &locator, signer, metrics, chain)
        .map_err(eyre::Report::from)
}

pub struct Params {
    pub time_limit: Duration,
    pub ops_per_minute: u64,
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

    async fn load_workers(&self) -> Result<Vec<Worker>> {
        use std::path::Path;

        let workers_path = Path::new(&self.workers_dir);

        let entries: Vec<_> = std::fs::read_dir(workers_path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        let num_workers = entries.len();

        let mut workers = Vec::new();
        for i in 0..num_workers {
            let worker = Worker::load_existing(
                i,
                self.wrpc_url.clone(),
                Network::KaspaTest10,
                &self.workers_dir,
            )
            .await?;

            workers.push(worker);
        }

        info!(
            "Loaded workers: N: {}, dir: {}",
            num_workers, self.workers_dir
        );

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
        workers: Vec<Worker>,
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
            let cancel_token_clone = cancel.clone();

            tokio::spawn(async move {
                do_round_trip(
                    r,
                    worker,
                    FIXED_TRANSFER_AMOUNT_SOMPI,
                    &tx_clone,
                    task_id,
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

        info!("Loaded workers: N: {}", workers.len());

        let cancel = CancellationToken::new();
        let (total_spend, total_ops) = self
            .execute_simulation_loop(workers, stats_tx.clone(), cancel.clone())
            .await?;

        info!("Waiting for tasks to finish");
        drop(stats_tx);
        tokio::time::sleep(self.params.max_wait_for_cancel).await;
        cancel.cancel();

        info!("Waiting for stats collector to finish writing all stats");
        let total_stats = collector_handle.await?;
        info!("Stats collector finished, wrote {} stats total", total_stats);

        write_metadata(&metadata_file_path, total_spend, total_ops)?;

        info!("Simulation complete");
        info!("Stats file: {}", stats_file_path);
        info!("Metadata file: {}", metadata_file_path);

        Ok(())
    }
}
