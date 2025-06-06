#![allow(unused)] // TODO: remove

mod x;
use x::args::Args;

use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::{
    constants::{SOMPI_PER_KASPA, TX_VERSION},
    network::{NetworkId, NetworkType},
    subnets::SUBNETWORK_ID_NATIVE,
    tx::{MutableTransaction, Transaction, TransactionInput, TransactionOutpoint, TransactionOutput, UtxoEntry},
    sign::sign,
};
use kaspa_core::info;
use kaspa_grpc_client::GrpcClient;
use kaspa_notify::subscription::context::SubscriptionContext;
use kaspa_rpc_core::notify::mode::NotificationMode;
use kaspa_wallet_core::api::WalletApi;
use kaspa_wallet_core::error::Error;
use kaspa_wallet_core::wallet::Wallet;
use kaspa_wallet_keys::secret::Secret;
use kaspa_txscript::pay_to_address_script;

use kaspa_txscript::{
    extract_script_pub_key_address, multisig_redeem_script, opcodes::codes::OpData65,
    pay_to_script_hash_script, script_builder::ScriptBuilder,
};

use std::sync::Arc;

use kaspa_wrpc_client::Resolver;
use secp256k1::{Keypair, rand::thread_rng};

use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::model::GetBalanceByAddressRequest;

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

struct User {
    pub k: Keypair,
    pub addr: Address,
}

async fn get_client(args: &Args) -> GrpcClient {
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
    rpc_client
}

fn get_user(args: &Args) -> Result<User, Error> {
    if let Some(sk_hex) = &args.private_key {
        let mut sk_bz = [0u8; 32];
        faster_hex::hex_decode(sk_hex.as_bytes(), &mut sk_bz).unwrap();
        let k = Keypair::from_seckey_slice(secp256k1::SECP256K1, &sk_bz).unwrap();
        let kas_addr = Address::new(
            ADDRESS_PREFIX,
            ADDRESS_VERSION,
            &k.x_only_public_key().0.serialize(),
        );
        return Ok(User { k, addr: kas_addr });
    } else {
        let (sk, pk) = &secp256k1::generate_keypair(&mut thread_rng());
        let kas_addr = Address::new(
            ADDRESS_PREFIX,
            ADDRESS_VERSION,
            &pk.x_only_public_key().0.serialize(),
        );
        info!(
            "Generated private key {} and address {}. Send some funds to this address and rerun with `--private-key {}`",
            sk.display_secret(),
            String::from(&kas_addr),
            sk.display_secret()
        );
        return Err(Error::PoisonError("No private key provided".to_string()));
    };
}

struct Escrow {
    keys: Vec<Keypair>,
    redeem_script: Vec<u8>,
    addr: Address,
}

fn create_escrow() -> Escrow {
    let m = 2; // required
    let n = 2; // total
    let kps = (0..n)
        .map(|_| Keypair::new(secp256k1::SECP256K1, &mut thread_rng()))
        .collect::<Vec<_>>();
    let redeem_script =
        multisig_redeem_script(kps.iter().map(|pk| pk.x_only_public_key().0.serialize()), m)
            .unwrap();
    let p2hs = pay_to_script_hash_script(&redeem_script);
    let addr = extract_script_pub_key_address(&p2hs, ADDRESS_PREFIX).unwrap();
    Escrow {
        keys: kps.to_vec(),
        redeem_script,
        addr,
    }
}

async fn check_balance(client: &GrpcClient, addr: &Address) -> Result<u64, Error> {
    let balance = client.get_balance_by_address(addr.clone()).await?;
    Ok(balance)
}

async fn deposit(client: &GrpcClient, addr: &Address, amount: u64) -> Result<(), Error> {
    let user = get_user(&Args::parse())?;
    let utxos = client.get_utxos_by_addresses(vec![user.addr.clone()]).await?;
    let utxo_entries: Vec<(TransactionOutpoint, UtxoEntry)> = utxos
        .into_iter()
        .map(|entry| (TransactionOutpoint::from(entry.outpoint), UtxoEntry::from(entry.utxo_entry)))
        .collect();

    let script_public_key = pay_to_address_script(addr);
    let inputs = utxo_entries
        .iter()
        .map(|(op, _)| TransactionInput { 
            previous_outpoint: *op, 
            signature_script: vec![], 
            sequence: 0, 
            sig_op_count: 1 
        })
        .collect::<Vec<_>>();

    let outputs = vec![TransactionOutput { 
        value: amount, 
        script_public_key: script_public_key.clone() 
    }];

    let unsigned_tx = Transaction::new_non_finalized(
        TX_VERSION,
        inputs,
        outputs,
        0,
        SUBNETWORK_ID_NATIVE,
        0,
        vec![],
    );

    let signed_tx = sign(
        MutableTransaction::with_entries(
            unsigned_tx,
            utxo_entries.iter().map(|(_, entry)| entry.clone()).collect::<Vec<_>>()
        ),
        user.k,
    );

    client.submit_transaction(signed_tx.tx.as_ref().into(), false).await?;
    Ok(())
}

// demonstrates on testnet
// 1. create multisig escrow address
// 2. user deposits to escrow (1 kas)
// 3. user creates a multisig tx which requires sigs from the escrow key holders. User adds his own utxo to pay fees
// 4. user gathers sigs from the escrow key holders, mimick a parallel signing flow, to combine later
// 5. user combines the sigs and submits to the network for real, confirming he gets a 'refund' from his original deposit
// async fn run_demo() {
async fn lets_go() {
    kaspa_core::log::init_logger(None, "");
    let args = Args::parse();
    let rpc_client = get_client(&args).await;
    let user = get_user(&args).unwrap();
    let balance = check_balance(&rpc_client, &user.addr).await.unwrap();
    println!("Balance: {}", balance);
    let escrow = create_escrow();
    println!("Escrow address: {}", escrow.addr);
    let balance = check_balance(&rpc_client, &escrow.addr).await.unwrap();
    println!("Escrow balance: {}", balance);
}

#[tokio::main]
async fn main() {
    lets_go().await;
}
