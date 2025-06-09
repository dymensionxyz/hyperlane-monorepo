use super::escrow::*;

use std::sync::Arc;

use kaspa_addresses::Address;
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionOutpoint, UtxoEntry};
use kaspa_core::info;
use kaspa_wallet_core::error::Error;

use kaspa_wallet_core::prelude::*;
use kaspa_wallet_keys::prelude::*;
use kaspa_wallet_pskt::prelude::*;
use secp256k1::{ Secp256k1, Keypair as SecpKeypair};

use kaspa_txscript::pay_to_address_script;

use kaspa_rpc_core::api::rpc::RpcApi;

use kaspa_consensus_core::hashing::sighash::{calc_schnorr_signature_hash, SigHashReusedValuesUnsync};

pub async fn build_withdrawal_tx(
    w: &Arc<Wallet>,
    e: &Escrow,
    user_address: Address,
) -> Result<PSKT<Signer>, Error> {
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
        .no_more_inputs()
        .no_more_outputs()
        .signer();

    Ok(pskt)
}

pub fn sign_withdrawal_tx(pskt: PSKT<Signer>, e: &Escrow) -> Result<PSKT<Combiner>, Error> {
    let signed_pskts: Vec<PSKT<Signer>> = e
        .keys
        .iter()
        .enumerate()
        .map(|(i, keypair)| {
            info!("-> Signer {} is signing their copy...", i + 1);
            sign_pskt_with_single_key(pskt.clone(), keypair)
        })
        .collect::<Result<Vec<PSKT<Signer>>, Error>>()?;

    
    let mut combined_pskt = signed_pskts
        .first()
        .ok_or("No signatures provided to combine")?
        .clone()
        .combiner();

    for signed_pskt in signed_pskts.iter().skip(1) {
        combined_pskt = (combined_pskt + signed_pskt.clone()).unwrap();
    }

    Ok(combined_pskt)
}

fn sign_pskt_with_single_key(
    pskt: PSKT<Signer>,
    kp: &SecpKeypair,
) -> Result<PSKT<Signer>, Error> {
    let reused_values = SigHashReusedValuesUnsync::new();

    pskt.pass_signature_sync(|tx, sighashes| {
        // let tx = dbg!(tx);
        tx.tx
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, _input)| {
                let hash = calc_schnorr_signature_hash(&tx.as_verifiable(), idx, sighashes[idx], &reused_values);
                let msg =
                    secp256k1::Message::from_digest_slice(&hash.as_bytes()).map_err(|e| e.to_string())?;
                Ok(SignInputOk {
                    signature: Signature::Schnorr(kp.sign_schnorr(msg)),
                    pub_key: kp.public_key(),
                    key_source: None,
                })
            })
            .collect()
    })
}

pub async fn deliver_withdrawal_tx(w: &Arc<Wallet>, e: &Escrow, amt: u64) -> Result<(), Error> {
    Ok(())
}
