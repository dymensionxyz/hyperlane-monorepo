#![allow(unused)] // TODO: remove

use kaspa_consensus_core::{
    hashing::sighash::{calc_schnorr_signature_hash, SigHashReusedValuesUnsync},
    tx::{TransactionId, TransactionOutpoint, UtxoEntry},
    network::{NetworkType, NetworkId},
};
use kaspa_txscript::{multisig_redeem_script, opcodes::codes::OpData65, pay_to_script_hash_script, script_builder::ScriptBuilder, extract_script_pub_key_address};
use kaspa_wallet_pskt::prelude::{
    Combiner, Creator, Extractor, Finalizer, Inner, InputBuilder, OutputBuilder, SignInputOk, Signature, Signer, Updater, PSKT,
};
// use kaspa_wallet::Wallet;
use kaspa_wallet_core::{account::Account, api::WalletApi, api::PingRequest};
use kaspa_wallet_core::rpc::DynRpcApi;
use kaspa_wallet_core::storage::{IdT, PrvKeyDataInfo};
use kaspa_wallet_core::wallet::Wallet;
use kaspa_wallet_core::error::Error;


use std::sync::Arc;

use secp256k1::{rand::thread_rng, Keypair};
use std::{iter, str::FromStr};
use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding, Resolver};

const URL: &str = "https://api-tn10.kaspa.org";
const NETWORK: NetworkType = NetworkType::Testnet;
const NETWORK_ID: NetworkId = NetworkId::with_suffix(NETWORK, 10);


fn get_wallet() -> Result<Arc<Wallet>, Error> {
    // Wallet::try_with_rpc(rpc, store, network_id);
    Ok(Arc::new(Wallet::try_new(Wallet::local_store()?,Some(Resolver::default()), Some(NETWORK_ID))?))
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

fn main() {
    tokio::runtime::Runtime::new().unwrap().block_on(run_demo()).unwrap();
    // run_demo().unwrap();
}