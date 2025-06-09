#![allow(unused)] // TODO: remove

mod x;
use x::args::Args;
use x::consts::*;
use x::deposit::*;
use x::escrow::*;
use x::util::*;
use x::wallet::*;
use x::withdraw::*;

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

use kaspa_wallet_core::prelude::*;
use kaspa_wallet_pskt::prelude::*; // Import the prelude for easy access to traits/structs

use kaspa_txscript::{
    extract_script_pub_key_address, multisig_redeem_script, pay_to_address_script,
    pay_to_script_hash_script,
};

use secp256k1::{Keypair, rand::thread_rng};

use kaspa_rpc_core::api::rpc::RpcApi;
use workflow_core::abortable::Abortable;

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

    let amt = DEPOSIT_AMOUNT;
    let tx_id = deposit(&w, &s, &e, amt).await?;
    info!("Deposit transaction sent: {}", tx_id);

    workflow_core::task::sleep(std::time::Duration::from_secs(5)).await;

    check_wallet_balance(w.clone()).await?;
    let balance = check_escrow_balance(&w, &e).await?;
    info!("Escrow balance: {}", balance);

    let user_addr = w.account()?.receive_address()?;

    let pskt_unsigned = build_withdrawal_tx(&w, &e, user_addr).await?;

    let pskt_signed = sign_withdrawal_tx(pskt_unsigned, &e)?;

    let withdrawal_tx_id = deliver_withdrawal_tx(&w, pskt_signed, &e).await?;

    workflow_core::task::sleep(std::time::Duration::from_secs(5)).await;

    check_wallet_balance(w.clone()).await?;
    let balance = check_escrow_balance(&w, &e).await?;
    info!("Escrow balance: {}", balance);

    w.stop().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = demo().await {
        eprintln!("Error: {}", e);
    }
}
