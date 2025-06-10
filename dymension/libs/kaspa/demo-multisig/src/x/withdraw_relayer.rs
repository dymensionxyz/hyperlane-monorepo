use super::consts::RELAYER_NETWORK_FEE;
use super::escrow::*;
use super::util::sign_pskt;

use std::{ops::Deref, sync::Arc};

use kaspa_addresses::Address;
use kaspa_consensus_core::hashing::sighash_type::{
    SIG_HASH_ALL, SIG_HASH_ANY_ONE_CAN_PAY, SigHashType,
};
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
    a_relayer: &Arc<dyn Account>,
    amt: u64,
) -> Result<PSKT<Signer>, Error> {
    let utxos_e = rpc.get_utxos_by_addresses(vec![e.addr.clone()]).await?;
    let utxo_e_first = utxos_e
        .into_iter()
        .next()
        .ok_or("No UTXO found at escrow address")?;
    let utxo_e_entry = UtxoEntry::from(utxo_e_first.utxo_entry);
    let utxo_e_out = TransactionOutpoint::from(utxo_e_first.outpoint);

    let utxo_r = UtxoIterator::new(a_relayer.utxo_context())
        .next()
        .ok_or("Relayer has no UTXOs")?;
    let utxo_r_entry: UtxoEntry = (utxo_r.utxo.as_ref()).into();
    let utxo_r_out = TransactionOutpoint::from(utxo_r.outpoint());

    let input_e = InputBuilder::default()
        .utxo_entry(utxo_e_entry.clone())
        .previous_outpoint(utxo_e_out)
        .sig_op_count(e.n() as u8) // Total possible signers
        .redeem_script(e.redeem_script.clone())
        .build()
        .map_err(|e| Error::Custom(format!("pskt input e: {}", e)))?;

    let input_r = InputBuilder::default()
        .utxo_entry(utxo_r_entry.clone())
        .previous_outpoint(utxo_r_out)
        .sig_op_count(1)
        .build()
        .map_err(|e| Error::Custom(format!("pskt input r: {}", e)))?;

    let output_e_to_user = OutputBuilder::default()
        .amount(amt)
        .script_public_key(ScriptPublicKey::from(pay_to_address_script(&user_address)))
        .build()
        .map_err(|e| Error::Custom(format!("pskt output e_to_user: {}", e)))?;

    let output_e_change = OutputBuilder::default()
        .amount(utxo_e_entry.amount - amt)
        .script_public_key(e.p2sh.clone())
        .build()
        .map_err(|e| Error::Custom(format!("pskt output e_change: {}", e)))?;

    let output_r_change = OutputBuilder::default()
        .amount(utxo_r_entry.amount - RELAYER_NETWORK_FEE)
        .script_public_key(ScriptPublicKey::from(pay_to_address_script(
            &a_relayer.change_address()?,
        )))
        .build()
        .map_err(|e| Error::Custom(format!("pskt output r_change: {}", e)))?;

    let pskt = PSKT::<Creator>::default()
        .constructor()
        .input(input_e)
        .input(input_r)
        .output(output_e_to_user)
        .output(output_e_change)
        .output(output_r_change)
        .no_more_inputs()
        .no_more_outputs()
        .signer();

    Ok(pskt)
}

pub async fn sponsor_and_send_tx<T: RpcApi + ?Sized>(
    rpc: &T,
    pskt_signed_vals: PSKT<Combiner>,
    pskt_unsigned: PSKT<Signer>,
    e: &EscrowPublic,
    w_relayer: &Arc<Wallet>,
    s_relayer: &Secret,
) -> Result<TransactionId, Error> {
    let pskt_signed_relayer = sign_network_fee(rpc, pskt_unsigned.clone(), w_relayer, s_relayer).await?;
    let pskt_signed = (pskt_signed_relayer + pskt_signed_vals).unwrap();

    let finalized_pskt = pskt_signed
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

async fn sign_network_fee<T: RpcApi + ?Sized>(
    rpc: &T,
    pskt_unsigned: PSKT<Signer>,
    w: &Arc<Wallet>,
    s: &Secret,
) -> Result<PSKT<Combiner>, Error> {
    Ok(pskt_done)
}
