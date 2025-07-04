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
use kaspa_wallet_pskt::prelude::{Signer, PSKT};
use secp256k1::PublicKey;
use std::io::Cursor;
use std::sync::Arc;

use super::messages::WithdrawalDetails;
use corelib::wallet::EasyKaspaWallet;
use corelib::withdraw::WithdrawFXG;
use eyre::eyre;
use kaspa_addresses::Prefix;
use kaspa_rpc_core::model::RpcTransactionId;
use kaspa_wallet_pskt::prelude::Bundle;
use tracing::info;

pub fn sign_pskt(
    pskt: PSKT<Signer>,
    key_pair: secp256k1::Keypair,
    payload: Vec<u8>,
    source: Option<KeySource>,
) -> Result<PSKT<Signer>> {
    // reused_values is something copied from the `pskb_signer_for_address` funciton
    let reused_values = SigHashReusedValuesUnsync::new();
    pskt.pass_signature_sync(|tx, sighash| {
        let mut with_payload = tx.clone();
        with_payload.tx.payload = payload;

        with_payload
            .tx
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, _input)| {
                let hash = calc_schnorr_signature_hash(
                    &with_payload.as_verifiable(),
                    idx,
                    sighash[idx],
                    &reused_values,
                );
                let msg = secp256k1::Message::from_digest_slice(&hash.as_bytes())
                    .map_err(|e| eyre::eyre!("Failed to convert hash to message: {}", e))?;
                Ok(SignInputOk {
                    signature: Signature::Schnorr(key_pair.sign_schnorr(msg)),
                    pub_key: key_pair.public_key(),
                    key_source: source.clone(),
                })
            })
            .collect()
    })
}
