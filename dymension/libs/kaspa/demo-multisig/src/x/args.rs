
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::{
    hashing::sighash::{SigHashReusedValuesUnsync, calc_schnorr_signature_hash},
    network::{NetworkId, NetworkType},
    tx::{TransactionId, TransactionOutpoint, UtxoEntry},
};
use kaspa_txscript::{
    extract_script_pub_key_address, multisig_redeem_script, opcodes::codes::OpData65,
    pay_to_script_hash_script, script_builder::ScriptBuilder,
};
use kaspa_wallet_pskt::prelude::{
    Combiner, Creator, Extractor, Finalizer, Inner, InputBuilder, OutputBuilder, PSKT, SignInputOk,
    Signature, Signer, Updater,
};
// use kaspa_wallet::Wallet;
use kaspa_core::{info, kaspad_env::version, time::unix_now, warn};
use kaspa_grpc_client::{ClientPool, GrpcClient};
use kaspa_notify::subscription::context::SubscriptionContext;
use kaspa_rpc_core::{RpcUtxoEntry, api::rpc::RpcApi, notify::mode::NotificationMode};
use kaspa_txscript::pay_to_address_script;
use kaspa_wallet_core::error::Error;
use kaspa_wallet_core::rpc::DynRpcApi;
use kaspa_wallet_core::storage::{IdT, PrvKeyDataInfo};
use kaspa_wallet_core::wallet::Wallet;
// use kaspa_wallet;
use kaspa_wallet_core::{account::Account, api::PingRequest, api::WalletApi};
use kaspa_wallet_keys::secret::Secret;
// use parking_lot::Mutex;
// use rand::RngCore;
// use rayon::prelude::*;
// use bip39::{Language, Mnemonic, Seed};
use kaspa_bip32::{DerivationPath, ExtendedPublicKey};

use clap::{Arg, Command};

use std::sync::Arc;

use kaspa_wrpc_client::{KaspaRpcClient, Resolver, WrpcEncoding};
use secp256k1::{Keypair, rand::thread_rng};
use std::{iter, str::FromStr};

pub fn cli() -> Command {
    Command::new("rothschild")
        .about(format!(
            "{} (rothschild) v{}",
            env!("CARGO_PKG_DESCRIPTION"),
            version()
        ))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("private-key")
                .long("private-key")
                .short('k')
                .value_name("private-key")
                .help("Private key in hex format"),
        )
        .arg(
            Arg::new("rpcserver")
                .long("rpcserver")
                .short('s')
                .value_name("rpcserver")
                .default_value("localhost:16210")
                .help("RPC server"),
        )
}

pub struct Args {
    pub private_key: Option<String>,
    pub rpc_server: String,
}

impl Args {
    pub fn parse() -> Self {
        let m = cli().get_matches();
        Args {
            private_key: m.get_one::<String>("private-key").cloned(),
            rpc_server: m
                .get_one::<String>("rpcserver")
                .cloned()
                .unwrap_or("localhost:16210".to_owned()),
        }
    }
}