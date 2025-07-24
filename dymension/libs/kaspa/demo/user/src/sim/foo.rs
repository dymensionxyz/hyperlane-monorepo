use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use corelib::wallet::EasyKaspaWallet;
use eyre::Result;

/*
Goals
    - Do deposits from n users, to n users on the hub
    - Withdraw from those users on hub back to kaspa
    - Vary nominal amounts by some distribution
    - Measure latency of each direction 
 */

pub struct TrafficSim {
    cosmos_rpc: CosmosGrpcClient,
    easy_wallet: EasyKaspaWallet,
}

impl TrafficSim {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(&self) -> Result<()> {
        Ok(())
    }
}