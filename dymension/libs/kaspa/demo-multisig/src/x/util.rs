use super::escrow::*;

use std::sync::Arc;

use kaspa_wallet_core::error::Error;

use kaspa_wallet_core::prelude::*;
 // Import the prelude for easy access to traits/structs



use kaspa_rpc_core::api::rpc::RpcApi;


pub async fn check_escrow_balance(w: &Arc<Wallet>, e: &Escrow) -> Result<u64, Error> {
    w.rpc_api()
        .get_balance_by_address(e.addr.clone())
        .await
        .map_err(|e| Error::Custom(format!("Error getting balance for escrow address: {}", e)))
}