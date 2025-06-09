use super::escrow::*;

use std::sync::Arc;

use kaspa_addresses::Address;
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionOutpoint, UtxoEntry};
use kaspa_core::info;
use kaspa_wallet_core::error::Error;
use kaspa_wallet_core::utxo::UtxoIterator;

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
    let utxo_entry = UtxoEntry::from(utxo_entry);
    let outpoint = TransactionOutpoint::from(utxo_ref.outpoint);
    let input = InputBuilder::default()
        .utxo_entry(utxo_entry.clone())
        .previous_outpoint(outpoint)
        .sig_op_count(e.n() as u8) // Total possible signers
        .redeem_script(e.redeem_script.clone())
        .build()
        .map_err(|e| Error::Custom(format!("Error building PSKT input: {}", e)))?;

    let output_script = pay_to_address_script(&user_address);
    let output = OutputBuilder::default()
        .amount(utxo_entry.amount)
        .script_public_key(ScriptPublicKey::from(output_script))
        .build()
        .map_err(|e| Error::Custom(format!("Error building PSKT output: {}", e)))?;

    let pskt = PSKT::<Creator>::default()
        .constructor()
        .input(input)
        .output(output)
        // .no_more_inputs()
        // .no_more_outputs()
        .signer();

    Ok(pskt)
}

pub async fn deliver_withdrawal_tx<T: RpcApi + ?Sized>(
    rpc: &T,
    w_relayer: &Arc<Wallet>,
    pskt_validator_signed: PSKT<Combiner>, // Takes the result from the signing function
    e: &EscrowPublic,
) -> Result<TransactionId, Error> {
    let a_relayer = w_relayer.account()?;
    let a_ctx = a_relayer.utxo_context();

    pskt_validator_signed.updater().input(InputBuilder::default()

    let fee_utxo = UtxoIterator::new(a_ctx)
        .next() // For the demo, we just take the first available UTXO.
        .ok_or("Relayer account has no spendable UTXO to pay for fees")?;

    pskt_validator_signed.up

    let finalized_pskt = pskt_validator_signed
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
