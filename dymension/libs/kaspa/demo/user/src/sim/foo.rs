use corelib::wallet::EasyKaspaWallet;
use eyre::Result;
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use rand_distr::{Distribution, Exp};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::info;


/*
Goals
    - Do deposits from n users, to n users on the hub
    - Withdraw from those users on hub back to kaspa
    - Vary nominal amounts by some distribution
    - Measure latency of each direction

Observations
    - Can just use one kaspa whale
    - Can use a new keypair on the hub for each user
 */



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

struct TaskResources {
    rpc_hub: CosmosGrpcClient,
    w: EasyKaspaWallet,
}

pub struct TrafficSim {
    params: Params,
    resources: Arc<TaskResources>,
}

impl TrafficSim {
    pub fn new() -> Self {
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
            tokio::spawn(async move {
                do_round_trip(r, nominal_value, tx_clone).await;
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
