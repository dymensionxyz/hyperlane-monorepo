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
        .sighash_type(SIG_HASH_ANY_ONE_CAN_PAY)
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
    w_relayer_secret: &Secret,
    pskt_validator_signed: PSKT<Combiner>, // Takes the result from the signing function
    e: &EscrowPublic,
) -> Result<TransactionId, Error> {
    let pskt_fee = get_fee_pskt(w_relayer, w_relayer_secret)?;

    let pskt_done = (pskt_validator_signed + pskt_fee).unwrap();

    let finalized_pskt = pskt_done
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

fn get_fee_pskt(w_relayer: &Arc<Wallet>, w_relayer_secret: &Secret) -> Result<PSKT<Combiner>, Error> {
    let a_relayer = w_relayer.account()?;
    let a_ctx = a_relayer.utxo_context();

    let utxo = UtxoIterator::new(a_ctx)
        .next() // For the demo, we just take the first available UTXO.
        .ok_or("Relayer account has no spendable UTXO to pay for fees")?
        .utxo
        .as_ref()
        .clone();

    let entry = UtxoEntry::from(&utxo);
    let outpoint = TransactionOutpoint::from(utxo.outpoint);

    let input = InputBuilder::default()
        .utxo_entry(entry)
        .previous_outpoint(outpoint)
        .sig_op_count(1)
        .sighash_type(SIG_HASH_ALL)
        .build()
        .map_err(|e| Error::Custom(format!("Error building PSKT input: {}", e)))?;

    let change_amount = utxo.amount - RELAYER_NETWORK_FEE; // TODO: negative check

    let addr = a_relayer.change_address()?;
    let change_script = pay_to_address_script(&addr);
    let change_output = OutputBuilder::default()
        .amount(change_amount)
        .script_public_key(change_script)
        .build()
        .map_err(|e| Error::Custom(format!("Error building PSKT output: {}", e)))?;

    let pskt = PSKT::<Creator>::default()
        .constructor()
        .input(input)
        .output(change_output)
        .signer();

    let relayer_private_key =
        get_private_key_for_input(&a_relayer, w_relayer_secret, 0, &pskt).await?;
    let relayer_keypair = SecpKeypair::from_secret_key(&relayer_private_key);

    sign_pskt(pskt, &relayer_keypair, 0)
}

async fn get_private_key_for_input(
    account: &Arc<dyn Account>,
    secret: &Secret,
    input_index: usize,
    pskt: &PSKT<Signer>,
) -> Result<secp256k1::SecretKey, Error> {
    let keydata = account.prv_key_data(secret.clone()).await?;
    let derivation_capable = account.as_derivation_capable()?;
    
    let input = pskt.inputs.get(input_index).ok_or("Input index out of bounds")?;
    let utxo_entry = input.utxo_entry.as_ref().ok_or("Input is missing UTXO entry")?;
    let utxo_address = kaspa_txscript::extract_script_pub_key_address(
        &utxo_entry.script_public_key,
        account.wallet().address_prefix()?
    )?;

    // --- THIS IS THE FIX ---
    // We interact with the public AddressManager APIs provided by the trait.

    // 1. Get the address managers.
    let receive_manager = derivation_capable.receive_address_manager();
    let change_manager = derivation_capable.change_address_manager();

    // 2. Check if the address belongs to the receive set, then the change set.
    let (is_change, index) = if let Some(index) = receive_manager.inner().address_to_index_map.get(&utxo_address) {
        (false, *index)
    } else if let Some(index) = change_manager.inner().address_to_index_map.get(&utxo_address) {
        (true, *index)
    } else {
        // The address was not found in either manager. This can happen if the UTXO
        // belongs to an address outside the current derivation window.
        // A full application might need to scan further, but for this demo, it's an error.
        return Err(format!("Could not find derivation index for relayer's fee UTXO address: {}. Please ensure the account is fully synced.", utxo_address).into());
    };
    // ----------------------

    let xprv = keydata.get_xprv(None)?;
    let derived_keys = derivation_capable.get_range_with_keys(is_change, index..index + 1, false, &xprv).await?;
    Ok(derived_keys.first().ok_or("Failed to derive private key for relayer")?.1)
}
