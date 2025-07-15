/// Refactored copy
/// https://github.com/kaspanet/rusty-kaspa/blob/v1.0.0/wallet/core/src/storage/transaction/record.rs
use eyre::Result;
use kaspa_consensus_core::network::NetworkId;
use kaspa_wallet_core::prelude::DynRpcApi;
use kaspa_wallet_core::utxo::NetworkParams;
use std::sync::Arc;

pub async fn validate_maturity(
    client: &Arc<DynRpcApi>,
    block_daa_score: u64,
    network_id: NetworkId,
) -> Result<bool> {
    let dag_info = client
        .get_block_dag_info()
        .await
        .map_err(|e| eyre::eyre!("Get block DAG info: {}", e))?;

    Ok(is_mature(
        block_daa_score,
        dag_info.virtual_daa_score,
        network_id,
    ))
}

pub fn is_mature(block_daa_score: u64, current_daa_score: u64, network_id: NetworkId) -> bool {
    let params = NetworkParams::from(network_id);
    let maturity = params.user_transaction_maturity_period_daa();

    current_daa_score >= block_daa_score + maturity
}
