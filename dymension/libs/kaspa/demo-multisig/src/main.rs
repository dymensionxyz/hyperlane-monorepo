#![allow(unused)] // TODO: remove

mod x;
use x::args::Args;
use x::consts::*;
use x::wallet::*;

use std::sync::Arc;

use kaspa_addresses::Address;
use kaspa_consensus_core::{
    constants::TX_VERSION,
    sign::sign,
    subnets::SUBNETWORK_ID_NATIVE,
    tx::{
        MutableTransaction, ScriptPublicKey, Transaction, TransactionInput, TransactionOutpoint,
        TransactionOutput, UtxoEntry,
    },
};
use kaspa_core::info;
use kaspa_grpc_client::GrpcClient;
use kaspa_wallet_core::api::{AccountsSendRequest, WalletApi};
use kaspa_wallet_core::error::Error;
use kaspa_wallet_core::tx::Fees;

use kaspa_wallet_core::prelude::*; // Import the prelude for easy access to traits/structs

use kaspa_txscript::{
    extract_script_pub_key_address, multisig_redeem_script, pay_to_script_hash_script,
};

use secp256k1::{Keypair, rand::thread_rng};

use kaspa_rpc_core::api::rpc::RpcApi;

struct Escrow {
    keys: Vec<Keypair>,
    redeem_script: Vec<u8>,
    p2sh: ScriptPublicKey,
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
    let p2sh = pay_to_script_hash_script(&redeem_script);
    let addr = extract_script_pub_key_address(&p2sh, ADDRESS_PREFIX).unwrap();
    Escrow {
        keys: kps.to_vec(),
        redeem_script,
        p2sh,
        addr,
    }
}

async fn deposit(
    w: &Arc<Wallet>,
    secret: &Secret,
    e: &Escrow,
    amt: u64,
) -> Result<TransactionId, Error> {
    let a = w.account()?;

    let r = w
        .clone()
        // https://github.com/kaspanet/rusty-kaspa/blob/eb71df4d284593fccd1342094c37edc8c000da85/cli/src/modules/send.rs#L28-L38
        .accounts_send(AccountsSendRequest {
            account_id: a.id().clone(),
            wallet_secret: secret.clone(),
            payment_secret: None,
            destination: PaymentDestination::from(PaymentOutput::new(e.addr.clone(), amt)),
            priority_fee_sompi: Fees::from(0i64),
            payload: None,
        })
        .await;

    info!("r: {:?}", r);

    r?.final_transaction_id().ok_or_else(|| {
        Error::Custom("Deposit transaction failed to generate a transaction ID".to_string())
    })
}

async fn check_escrow_balance(w: &Arc<Wallet>, e: &Escrow) -> Result<u64, Error> {
    w.rpc_api()
        .get_balance_by_address(e.addr.clone())
        .await
        .map_err(|e| Error::Custom(format!("Error getting balance for escrow address: {}", e)))
}

/*
Demo:
The purpose is to test out using a multisig for securing an escrow address.
There are three roles, signer 1 and 2, and a relayer.
The relayer is responsible for building and orchestrating the multisig TXs, including paying any fees.
The signers are just responsible for signing.

The test involves a 'user', which corresponds to the local wallet account.

Steps are:

1. Create an escrow address.
2. User deposits some funds to the escrow address.
3. The relayer builds a multisig TX to send the funds back to the user from the escrow address.
4. The signers sign the TX.
5. The relayer sends the TX to the network.

Always, we want to get confirmation that everything has worked, been accepted by the network etc.

We will test against testnet 10. The wallet has 200'000 KAS available.
 */
async fn demo() -> Result<(), Error> {
    kaspa_core::log::init_logger(None, "");
    let args = Args::parse();

    let s = Secret::from(args.wallet_secret.unwrap_or("".to_string()));
    let w = get_wallet(&s).await?;

    check_wallet_balance(w.clone()).await?;

    let e = create_escrow();
    info!("Escrow address: {}", e.addr);

    info!("Doing the deposit");
    let tx_id = deposit(&w, &s, &e, 1).await?;
    info!("Deposit transaction sent: {}", tx_id);

    let balance = check_escrow_balance(&w, &e).await?;
    info!("Escrow balance: {}", balance);
    check_wallet_balance(w.clone()).await?;

    w.stop().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = demo().await {
        eprintln!("Error: {}", e);
    }
}
