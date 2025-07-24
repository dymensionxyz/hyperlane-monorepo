use corelib::wallet::EasyKaspaWallet;
use eyre::Result;
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use probability::distribution::Exponential;
use rand::Rng;
use std::time::{Duration, Instant};

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

const SOMPI_PER_KAS: u64 = 100_000_000;

pub struct Params {
    pub time_limit: Duration, // total target simulation time
    pub budget: u64,          // in sompi
    pub ops_per_minute: u64,  // osmosis does 90 per minute
}

impl Params {
    /// Used to draw value of each op, in sompi
    fn distr_value(&self) -> Exponential {
        Exponential::new(1.0 / self.op_budget())
    }
    /// Used to draw time between ops, in milliseconds
    fn distr_time(&self) -> Exponential {
        Exponential::new(1000.0 / self.ops_per_second())
    }
    fn num_ops(&self) -> f64 {
        self.time_limit.as_secs_f64() * self.ops_per_second()
    }
    fn op_budget(&self) -> f64 {
        self.budget as f64 / self.num_ops()
    }
    fn ops_per_second(&self) -> f64 {
        self.ops_per_minute as f64 / 60.0
    }
}

pub struct TrafficSim {
    params: Params,
    rpc_hub: CosmosGrpcClient,
    w: EasyKaspaWallet,
}

impl TrafficSim {
    pub fn new() -> Self {
        todo!()
    }

    pub async fn run(&self) -> Result<()> {
        Ok(())
    }

    async fn round_trip(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_params_parameterization() {
        let params = Params {
            time_limit: Duration::from_secs(60),
            budget: 200000 * SOMPI_PER_KAS,
            ops_per_minute: 90,
        };
        let mut r = rand::thread_rng();
        let elapsed = 0;
        let total_spend = 0;
        let total_ops =0;
        while elapsed < params.time_limit {
            let value = params.distr_value().sample(&mut r) as u64;
            let time = params.distr_time().sample(&mut r) as u64;
            total_spend += value;
            total_ops += 1;
        }
        println!("total_spend: {}, total_ops: {}", total_spend, total_ops);
    }
}
