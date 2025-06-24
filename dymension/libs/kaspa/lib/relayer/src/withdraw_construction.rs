use anyhow::Result;
use hyperlane_core::{Decode, H256, HyperlaneMessage};
use hyperlane_cosmos_native::CosmosNativeProvider;
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::{WithdrawalId, WithdrawalStatus};
use hyperlane_warp_route::TokenMessage;
use kaspa_consensus_core::hashing::sighash_type::{
    SIG_HASH_ALL, SIG_HASH_ANY_ONE_CAN_PAY, SigHashType,
};
use kaspa_consensus_core::mass::transaction_output_estimated_serialized_size;
use kaspa_consensus_core::network::NetworkId;
use kaspa_consensus_core::tx::TransactionOutpoint;
use kaspa_consensus_core::tx::{ScriptPublicKey, UtxoEntry};
use kaspa_hashes;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::{RpcUtxoEntry, RpcUtxosByAddressesEntry};
use kaspa_txscript;
use kaspa_txscript::standard::pay_to_address_script;
use kaspa_wallet_core::account::Account;
use kaspa_wallet_core::utxo::{NetworkParams, UtxoIterator};
use kaspa_wallet_pskt::prelude::*;
use kaspa_wallet_pskt::prelude::{PSKT, Signer};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
// Assuming EscrowPublic is correctly defined in the `core` crate
// and `core` is a dependency in this crate's Cargo.toml (e.g., `core = { path = "../core" }`)
use core::escrow::EscrowPublic;


struct WithdrawalConstructionArgs {
    messages: Vec<HyperlaneMessage>,
    kaspa_rpc: &impl RpcApi,
    escrow_public: &EscrowPublic,
    relayer_kaspa_account: &Arc<dyn Account>,
    current_hub_state: &TransactionOutpoint,
    network_id: NetworkId,
}

/// Updated signature matching the specification
async fn build_kaspa_withdrawal_pskts_pending(
    args: WithdrawalConstructionArgs,
) -> Result<Option<Vec<PSKT<Signer>>>> {

}