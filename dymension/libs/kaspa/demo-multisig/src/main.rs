#![allow(unused)] // TODO: remove

mod x;
use x::args::Args;
use x::consts::*;
use x::wallet::*;
use x::withdraw::*;
use x::escrow::*;
use x::deposit::*;
use x::util::*;

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

    let dst = PaymentDestination::from(PaymentOutput::new(e.addr.clone(), amt));
    let fees = Fees::from(0i64);
    let payload = None;
    let payment_secret = None;
    let abortable = Abortable::new();

    // use account.send, because wallet.accounts_send(AccountsSendRequest{..}) is buggy
    let (summary, _) = a
        .send(
            dst,
            fees,
            payload,
            secret.clone(),
            payment_secret,
            &abortable,
            None,
        )
        .await?;

    summary.final_transaction_id().ok_or_else(|| {
        Error::Custom("Deposit transaction failed to generate a transaction ID".to_string())
    })
}

async fn check_escrow_balance(w: &Arc<Wallet>, e: &Escrow) -> Result<u64, Error> {
    w.rpc_api()
        .get_balance_by_address(e.addr.clone())
        .await
        .map_err(|e| Error::Custom(format!("Error getting balance for escrow address: {}", e)))
}

async fn build_withdrawal_tx(
    w: &Arc<Wallet>,
    e: &Escrow,
    user_address: Address,
) -> Result<PSKT<Updater>, Error> {
    info!("Building withdrawal transaction...");
    let rpc = w.rpc_api();

    let utxos = rpc.get_utxos_by_addresses(vec![e.addr.clone()]).await?;
    let utxo_ref = utxos
        .into_iter()
        .next()
        .ok_or("No UTXO found at escrow address")?;
    let utxo_entry = utxo_ref.utxo_entry;
    info!("Found UTXO with amount {}", utxo_entry.amount);

    let fee = 10000; // A reasonable network fee (0.0001 KAS)
    let output_amount = utxo_entry
        .amount
        .checked_sub(fee)
        .ok_or("UTXO amount is less than the fee")?;
    // TODO: here it's like the withdrawer is paying fees directly from escrow, but actually we want it to be more expliclit (from relayer)

    let utxo_entry = UtxoEntry::from(utxo_entry);
    let outpoint = TransactionOutpoint::from(utxo_ref.outpoint);
    let input = InputBuilder::default()
        .utxo_entry(utxo_entry)
        .previous_outpoint(outpoint)
        .sig_op_count(e.keys.len() as u8) // Total possible signers
        .redeem_script(e.redeem_script.clone())
        .build()
        .map_err(|e| Error::Custom(format!("Error building PSKT input: {}", e)))?;

    let output_script = pay_to_address_script(&user_address);
    let output = OutputBuilder::default()
        .amount(output_amount)
        .script_public_key(ScriptPublicKey::from(output_script))
        .build()
        .map_err(|e| Error::Custom(format!("Error building PSKT output: {}", e)))?;

    let pskt = PSKT::<Creator>::default()
        .constructor()
        .input(input)
        .output(output)
        .updater();

    info!("PSKT built successfully. Ready for signing.");
    Ok(pskt)
}

async fn sign_withdrawal_tx(e: &Escrow, amt: u64) -> Result<(), Error> {}

async fn deliver_withdrawal_tx(w: &Arc<Wallet>, e: &Escrow, amt: u64) -> Result<(), Error> {}

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

    let balance = check_escrow_balance(&w, &e).await?;
    info!("Escrow balance: {}", balance);

    // --- Step 3: Relayer Builds the Withdrawal Transaction ---
    let user_address = w.account()?.receive_address()?;
    let pskt_to_sign = build_withdrawal_tx(&w, &e, user_address).await?;

    // let tx_signed = sign_withdrawal_tx(&e, amt).await?;

    // deliver_withdrawal_tx(&w, &e, amt).await?;

    w.stop().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = demo().await {
        eprintln!("Error: {}", e);
    }
}
