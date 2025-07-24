use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use corelib::wallet::EasyKaspaWallet;

pub struct TrafficSim {
    cosmos_rpc: CosmosGrpcClient,
    easy_wallet: EasyKaspaWallet,
}

impl TrafficSim {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}