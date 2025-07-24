use super::round_trip::do_round_trip;
use super::round_trip::TaskArgs;
use super::round_trip::TaskResources;
use super::stats::render_stats;
use super::util::as_kas;
use corelib::wallet::get_wallet;
use corelib::wallet::EasyKaspaWallet;
use eyre::Result;
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use kaspa_consensus_core::network::NetworkId;
use kaspa_wallet_keys::secret::Secret;
use rand_distr::{Distribution, Exp};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::x::args::{SimulateTrafficCli, WalletCli};
use tracing::info;

pub struct Params {
    pub time_limit: Duration, // total target simulation time
    pub budget: u64,          // in sompi
    pub ops_per_minute: u64,  // osmosis does 90 per minute
}

impl Params {
    /// Used to draw value of each op, in sompi
    pub fn distr_value(&self) -> Exp<f64> {
        Exp::new(1.0 / self.op_budget()).unwrap()
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
            },
            task_args: TaskArgs {
                domain_kas: cli.domain_kas,
                token_kas_placeholder: cli.token_kas_placeholder,
                domain_hub: cli.domain_hub,
                token_hub: cli.token_hub,
                escrow_address: addr,
            },
            wallet: cli.wallet,
        })
    }
}

pub struct TrafficSim {
    params: Params,
    resources: Arc<TaskResources>,
}

impl TrafficSim {
    pub async fn new(args: SimulateTrafficArgs) -> Result<Self> {
        let s = Secret::from(args.wallet.wallet_secret);

        let network_id = NetworkId::from_str(&args.wallet.network_id).unwrap();
        let w = get_wallet(&s, network_id, args.wallet.rpc_url, args.wallet.wallet_dir).await?;
        todo!()
    }

    pub async fn run(&self) -> Result<()> {
        let mut rng = rand::rng();
        let start_time = Instant::now();
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

        while start_time.elapsed() < self.params.time_limit {
            let nominal_value = self.params.distr_value().sample(&mut rng) as u64;
            let sleep_millis = self.params.distr_time().sample(&mut rng) as u64;
            tokio::time::sleep(Duration::from_millis(sleep_millis)).await;
            let tx_clone = stats_tx.clone();
            let r = self.resources.clone();
            let task_id = total_ops;
            tokio::spawn(async move {
                do_round_trip(r, nominal_value, tx_clone, task_id).await;
            });
            total_spend += nominal_value;
            total_ops += 1;
            info!(
                "elasped millis {}, interval {}, value {}",
                start_time.elapsed().as_millis(),
                sleep_millis,
                as_kas(nominal_value)
            );
        }

        drop(stats_tx);
        let final_stats = collector_handle.await?;
        render_stats(final_stats, total_spend, total_ops);

        Ok(())
    }
}
