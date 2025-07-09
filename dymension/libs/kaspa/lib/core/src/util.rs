use hyperlane_core::H256;
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::ScriptPublicKey;
use kaspa_txscript::pay_to_address_script;

pub fn get_recipient_address(recipient: H256, prefix: Prefix) -> Address {
    Address::new(
        prefix,
        Version::PubKey, // should always be PubKey
        recipient.as_bytes(),
    )
}

pub fn get_recipient_script_pubkey(recipient: H256, prefix: Prefix) -> ScriptPublicKey {
    ScriptPublicKey::from(pay_to_address_script(&get_recipient_address(
        recipient, prefix,
    )))
}

pub fn get_recipient_script_pubkey_address(address: &Address) -> ScriptPublicKey {
    ScriptPublicKey::from(pay_to_address_script(address))
}

/// Refactored copy
/// https://github.com/kaspanet/rusty-kaspa/blob/v1.0.0/wallet/core/src/storage/transaction/record.rs
pub mod maturity {
    use kaspa_consensus_core::network::NetworkId;
    use kaspa_wallet_core::prelude::DynRpcApi;
    use kaspa_wallet_core::utxo::NetworkParams;
    use std::sync::Arc;

    pub async fn validate_maturity(
        client: &Arc<DynRpcApi>,
        block_daa_score: u64,
        network_id: NetworkId,
    ) -> eyre::Result<bool> {
        validate_maturity_params(client, block_daa_score, &NetworkParams::from(network_id))
    }

    pub async fn validate_maturity_params(
        client: &Arc<DynRpcApi>,
        block_daa_score: u64,
        params: &NetworkParams,
    ) -> eyre::Result<bool> {
        let dag_info = client
            .get_block_dag_info()
            .await
            .map_err(|e| eyre::eyre!("Get block DAG info: {}", e))?;

        Ok(is_mature_params(
            block_daa_score,
            dag_info.virtual_daa_score,
            params,
        ))
    }

    pub fn is_mature(block_daa_score: u64, current_daa_score: u64, network_id: NetworkId) -> bool {
        is_mature_params(
            block_daa_score,
            current_daa_score,
            NetworkParams::from(network_id),
        )
    }

    pub fn is_mature_params(
        block_daa_score: u64,
        current_daa_score: u64,
        params: &NetworkParams,
    ) -> bool {
        let maturity = params.user_transaction_maturity_period_daa();

        current_daa_score >= block_daa_score + maturity
    }
}
