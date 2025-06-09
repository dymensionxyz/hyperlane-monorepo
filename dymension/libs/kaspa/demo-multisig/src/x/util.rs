use super::escrow::*;

use std::sync::Arc;

use kaspa_wallet_core::error::Error;

use kaspa_wallet_core::prelude::*;

use kaspa_rpc_core::api::rpc::RpcApi;

pub async fn check_balance<T: RpcApi + ?Sized>(rpc: &T, addr: &Address) -> Result<u64, Error> {
    rpc
        .get_balance_by_address(addr.clone())
        .await
        .map_err(|e| Error::Custom(format!("Getting balance for escrow address: {}", e)))
}
