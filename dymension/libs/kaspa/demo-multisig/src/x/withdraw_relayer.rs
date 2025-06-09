use super::escrow::*;

use std::sync::Arc;

use kaspa_addresses::Address;
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionOutpoint, UtxoEntry};
use kaspa_core::info;
use kaspa_wallet_core::error::Error;

use kaspa_wallet_core::prelude::*;
use kaspa_wallet_keys::prelude::*;
use kaspa_wallet_pskt::prelude::*;
use secp256k1::{Keypair as SecpKeypair, Secp256k1};

use kaspa_txscript::{
    opcodes::codes::OpData65, pay_to_address_script, script_builder::ScriptBuilder,
};

use kaspa_rpc_core::api::rpc::RpcApi;

use kaspa_consensus_core::hashing::sighash::{
    SigHashReusedValuesUnsync, calc_schnorr_signature_hash,
};

use std::iter;

pub async fn build_withdrawal_tx<T: RpcApi + ?Sized>(
    rpc: &T,
    e: &EscrowPublic,
    user_address: Address,
) -> Result<PSKT<Signer>, Error> {
    let utxos = rpc.get_utxos_by_addresses(vec![e.addr.clone()]).await?;
    let utxo_ref = utxos
        .into_iter()
        .next()
        .ok_or("No UTXO found at escrow address")?;
    let utxo_entry = utxo_ref.utxo_entry;

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
        .sig_op_count(e.n) // Total possible signers
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

pub async fn deliver_withdrawal_tx<T: RpcApi + ?Sized>(
    rpc: &T,
    signed_pskt: PSKT<Combiner>, // Takes the result from the signing function
    e: &EscrowPublic,
) -> Result<TransactionId, Error> {
    let finalized_pskt = signed_pskt
        .finalizer()
        .finalize_sync(|inner: &Inner| -> Result<Vec<Vec<u8>>, String> {
            Ok(inner
                .inputs
                .iter()
                .map(|input| -> Vec<u8> {
                    // todo actually required count can be retrieved from redeem_script, sigs can be taken from partial sigs according to required count
                    // considering xpubs sorted order

                    let signatures: Vec<_> = e
                        .pubs
                        .iter()
                        .flat_map(|kp| {
                            let sig = input.partial_sigs.get(&kp).unwrap().into_bytes();
                            iter::once(OpData65)
                                .chain(sig)
                                .chain([input.sighash_type.to_u8()])
                        })
                        .collect();
                    signatures
                        .into_iter()
                        .chain(
                            ScriptBuilder::new()
                                .add_data(input.redeem_script.as_ref().unwrap().as_slice())
                                .unwrap()
                                .drain()
                                .iter()
                                .cloned(),
                        )
                        .collect()
                })
                .collect())
        })
        .unwrap();

    let (tx, _) = finalized_pskt.extractor().unwrap().extract_tx().unwrap()(10000); // TODO: wtf is this number?

    let rpc_tx = (&tx).into();
    let tx_id = rpc.submit_transaction(rpc_tx, false).await?;

    Ok(tx_id)
}
