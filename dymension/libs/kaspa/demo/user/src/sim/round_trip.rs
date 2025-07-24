use super::stats::{RoundTripStats, Stats};
use corelib::wallet::EasyKaspaWallet;
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct TaskResources {
    rpc_hub: CosmosGrpcClient,
    w: EasyKaspaWallet,
}

pub async fn do_round_trip(
    resources: Arc<TaskResources>,
    value: u64,
    tx: mpsc::Sender<RoundTripStats>,
) {
}
