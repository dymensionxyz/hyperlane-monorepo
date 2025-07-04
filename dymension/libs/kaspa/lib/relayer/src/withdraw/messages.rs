use eyre::Result;

use kaspa_consensus_core::hashing::sighash::{
    calc_schnorr_signature_hash, SigHashReusedValuesUnsync,
};
use kaspa_wallet_core::derivation::build_derivate_paths;

use corelib::consts::KEY_MESSAGE_IDS;
use corelib::escrow::EscrowPublic;
use corelib::payload::{MessageID, MessageIDs};
use hex::ToHex;
use hyperlane_core::{Decode, HyperlaneMessage, H256};
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::{WithdrawalId, WithdrawalStatus};
use hyperlane_warp_route::TokenMessage;
use kaspa_consensus_core::config::params::Params;
use kaspa_consensus_core::constants::TX_VERSION;
use kaspa_consensus_core::hashing::sighash_type::{
    SigHashType, SIG_HASH_ALL, SIG_HASH_ANY_ONE_CAN_PAY,
};
use kaspa_consensus_core::mass;
use kaspa_consensus_core::network::NetworkId;
use kaspa_consensus_core::subnets::SUBNETWORK_ID_NATIVE;
use kaspa_consensus_core::tx::{PopulatedTransaction, ScriptPublicKey, UtxoEntry};
use kaspa_consensus_core::tx::{
    Transaction, TransactionInput, TransactionOutpoint, TransactionOutput,
};
use kaspa_hashes;
use kaspa_rpc_core::{RpcTransaction, RpcUtxoEntry, RpcUtxosByAddressesEntry};
use kaspa_txscript::standard::pay_to_address_script;
use kaspa_txscript::{opcodes::codes::OpData65, script_builder::ScriptBuilder};
use kaspa_wallet_core::account::Account;
use kaspa_wallet_core::prelude::DynRpcApi;
use kaspa_wallet_core::prelude::*;
use kaspa_wallet_core::utxo::NetworkParams;
use kaspa_wallet_pskt::prelude::*;
use kaspa_wallet_pskt::prelude::*;
use kaspa_wallet_pskt::prelude::{Signer, PSKT};
use secp256k1::PublicKey;
use std::io::Cursor;
use std::sync::Arc;

use super::hub_to_kaspa::build_withdrawal_pskt;
use corelib::wallet::EasyKaspaWallet;
use corelib::withdraw::{WithdrawFXG, filter_pending_withdrawals};
use kaspa_addresses::Prefix;
use kaspa_wallet_pskt::prelude::Bundle;
use tracing::info;

pub fn get_recipient_address(recipient: H256, prefix: Prefix) -> kaspa_addresses::Address {
    let addr = kaspa_addresses::Address::new(
        prefix,
        kaspa_addresses::Version::PubKey, // should always be PubKey
        recipient.as_bytes(),
    );
    addr
}

/// Processes given messages and returns WithdrawFXG and the very first outpoint
/// (the one preceding all the given transfers; it should be used during process indication).
pub async fn on_new_withdrawals(
    messages: Vec<HyperlaneMessage>,
    relayer: EasyKaspaWallet,
    cosmos: CosmosGrpcClient,
    escrow_public: EscrowPublic,
    hub_height: Option<u32>,
) -> Result<Option<(WithdrawFXG, TransactionOutpoint)>> {
    info!("Kaspa relayer, getting pending withdrawals");
    let (outpoint, pending_messages) = filter_pending_withdrawals(messages, &cosmos, hub_height)
        .await
        .map_err(|e| eyre::eyre!("Get pending withdrawals: {}", e))?;
    info!("Kaspa relayer, got pending withdrawals");

    let withdrawal_details: Vec<_> = pending_messages
        .iter()
        .filter_map(|m| {
            match TokenMessage::read_from(&mut Cursor::new(&m.body)) {
                Ok(msg) => {
                    let kaspa_recipient =
                        get_recipient_address(m.recipient, relayer.network_info.address_prefix);

                    Some(WithdrawalDetails {
                        message_id: m.id(),
                        recipient: kaspa_recipient,
                        amount_sompi: msg.amount().as_u64(),
                    })
                }
                Err(e) => None, // TODO: log?
            }
        })
        .collect();

    if withdrawal_details.is_empty() {
        info!("Kaspa relayer, no pending withdrawals, all in batch are already processed and confirmed on hub");
        return Ok(None); // nothing to process
    }
    info!(
        "Kaspa relayer, got pending withdrawals, building PSKT, len: {}",
        withdrawal_details.len()
    );

    let pskt = build_withdrawal_pskt(
        withdrawal_details,
        &relayer.api(),
        &escrow_public,
        &relayer.account(),
        &outpoint,
        relayer.network_info.network_id,
    )
    .await
    .map_err(|e| eyre::eyre!("Build withdrawal PSKT: {}", e))?;

    // We have a bundle with one PSKT which covers all the HL messages.
    Ok(Some((
        WithdrawFXG::new(Bundle::from(pskt), vec![pending_messages]),
        outpoint,
    )))
}

/// Details of a withdrawal extracted from HyperlaneMessage
#[derive(Debug, Clone)]
pub struct WithdrawalDetails {
    pub message_id: H256,
    pub recipient: kaspa_addresses::Address,
    pub amount_sompi: u64,
}
