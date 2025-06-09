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
use kaspa_wallet_core::error::Error;

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
    wallet: &Arc<Wallet>,
    escrow: &Escrow,
    amount: u64,
) -> Result<(), Error> {
    let utxos = client
        .get_utxos_by_addresses(vec![user.addr.clone()])
        .await?;
    let utxo_entries: Vec<(TransactionOutpoint, UtxoEntry)> = utxos
        .into_iter()
        .map(|entry| {
            (
                TransactionOutpoint::from(entry.outpoint),
                UtxoEntry::from(entry.utxo_entry),
            )
        })
        .collect();

    let inputs = utxo_entries
        .iter()
        .map(|(op, _)| TransactionInput {
            previous_outpoint: *op,
            signature_script: vec![],
            sequence: 0,
            sig_op_count: 1,
        })
        .collect::<Vec<_>>();

    let outputs = vec![TransactionOutput {
        value: amount,
        script_public_key: escrow.p2sh.clone(),
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
            utxo_entries
                .iter()
                .map(|(_, entry)| entry.clone())
                .collect::<Vec<_>>(),
        ),
        user.k,
    );

    client
        .submit_transaction(signed_tx.tx.as_ref().into(), false)
        .await?;
    Ok(())
}

async fn demo() -> Result<(), Error> {
    kaspa_core::log::init_logger(None, "");
    let args = Args::parse();

    let w = get_wallet(args.wallet_secret.unwrap_or("".to_string())).await?;

    debug_balance(w.clone()).await?;

    let e = create_escrow();
    info!("Escrow address: {}", e.addr);

    w.stop().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = demo().await {
        eprintln!("Error: {}", e);
    }
}
