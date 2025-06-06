#![allow(unused)] // TODO: remove

mod x;
use x::args::Args;

use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::network::{NetworkId, NetworkType};
use kaspa_core::info;
use kaspa_grpc_client::GrpcClient;
use kaspa_notify::subscription::context::SubscriptionContext;
use kaspa_rpc_core::notify::mode::NotificationMode;
use kaspa_wallet_core::api::WalletApi;
use kaspa_wallet_core::error::Error;
use kaspa_wallet_core::wallet::Wallet;
use kaspa_wallet_keys::secret::Secret;

use std::sync::Arc;

use kaspa_wrpc_client::Resolver;
use secp256k1::{Keypair, rand::thread_rng};

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

async fn get_client(args: &Args) {
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
    let user = get_user(&args);
}

#[tokio::main]
async fn main() {
    lets_go().await;
}
