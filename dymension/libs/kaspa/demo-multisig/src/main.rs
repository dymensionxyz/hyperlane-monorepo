#![allow(unused)] // TODO: remove

mod x;
use x::args::Args;

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

const URL: &str = "https://api-tn10.kaspa.org";
const NETWORK: NetworkType = NetworkType::Testnet;
const NETWORK_ID: NetworkId = NetworkId::with_suffix(NETWORK, 10);
const ADDRESS_PREFIX: Prefix = Prefix::Testnet;
const ADDRESS_VERSION: Version = Version::PubKey;

fn get_wallet() -> Result<Arc<Wallet>, Error> {
    Ok(Arc::new(Wallet::try_new(
        Wallet::local_store()?,
        Some(Resolver::default()),
        Some(NETWORK_ID),
    )?))
}

async fn roth() {
    kaspa_core::log::init_logger(None, "");
    let args = Args::parse();
    let subscription_context = SubscriptionContext::new();

    let rpc_client = GrpcClient::connect_with_args(
        NotificationMode::Direct,
        format!("grpc://{}", args.rpc_server),
        Some(subscription_context.clone()),
        true,
        None,
        false,
        Some(500_000),
        Default::default(),
    )
    .await
    .expect("Critical error: failed to connect to the RPC server.");

    info!("Connected to RPC");

    let schnorr_key = if let Some(private_key_hex) = args.private_key {
        let mut private_key_bytes = [0u8; 32];
        faster_hex::hex_decode(private_key_hex.as_bytes(), &mut private_key_bytes).unwrap();
        Keypair::from_seckey_slice(secp256k1::SECP256K1, &private_key_bytes).unwrap()
    } else {
        let (sk, pk) = &secp256k1::generate_keypair(&mut thread_rng());
        let kaspa_addr = Address::new(
            ADDRESS_PREFIX,
            ADDRESS_VERSION,
            &pk.x_only_public_key().0.serialize(),
        );
        info!(
            "Generated private key {} and address {}. Send some funds to this address and rerun rothschild with `--private-key {}`",
            sk.display_secret(),
            String::from(&kaspa_addr),
            sk.display_secret()
        );
        return;
    };
}

// demonstrates on testnet
// 1. create multisig escrow address
// 2. user deposits to escrow (1 kas)
// 3. user creates a multisig tx which requires sigs from the escrow key holders. User adds his own utxo to pay fees
// 4. user gathers sigs from the escrow key holders, mimick a parallel signing flow, to combine later
// 5. user combines the sigs and submits to the network for real, confirming he gets a 'refund' from his original deposit
// async fn run_demo() {
async fn run_demo() -> Result<(), Error> {
    let wallet = get_wallet()?;
    let open = wallet.is_open();
    let secret = Secret::from("lkjsdf");
    wallet.wallet_open(secret, None, true, false).await?;
    println!("Open: {:?}", open);
    // wallet.account()
    // let acc = wallet.accounts(filter, guard)
    // wallet.open(wallet_secret, filename, args, guard)
    // let res = wallet.ping(None).await;
    // let accounts = wallet.accounts(None, &guard).await;
    // println!("Ping response: {:?}", res);
    // println!("Accounts: {:?}", accounts);
    // println!("Ping response: {:?}", res);
    Ok(())

    // Create escrow info
    // let escrow_info = create_escrow_addr();
    // println!("Escrow address: {}", escrow_info.escrow_address);

    // let client = get_testnet_client();
    // let signer = get_signer();
    // let amt = 1_000_000_000; // 1 KAS

    // deposit_funds(
    //     &client,
    //     &escrow_info.escrow_address,
    //     amt,
    //     &signer,
    // ).await.unwrap();

    // let pskt = create_multisig_tx(
    //     &escrow_info,
    //     "kaspa:qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq".to_string(),
    //     amt,
    // );

    // let signed_pskts = get_sigs(pskt, &escrow_info);
    // submit_tx(signed_pskts).unwrap();
}

#[tokio::main]
async fn main() {
    // tokio::runtime::Runtime::new().unwrap().block_on(run_demo()).unwrap();
    // run_demo().await;
    roth().await;
}
