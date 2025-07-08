use eyre::Result;
use kaspa_addresses::Address;
use kaspa_core::info;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wallet_core::error::Error;

pub async fn check_balance<T: RpcApi + ?Sized>(
    source: &str,
    rpc: &T,
    addr: &Address,
) -> Result<u64, Error> {
    let balance = rpc
        .get_balance_by_address(addr.clone())
        .await
        .map_err(|e| Error::Custom(format!("Getting balance for address: {}", e)))?;

    info!("{} balance: {}", source, balance);
    Ok(balance)
}
