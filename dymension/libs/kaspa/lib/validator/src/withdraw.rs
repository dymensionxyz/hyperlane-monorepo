// We call the signers 'validators'

use corelib::escrow::*;
use std::collections::hash_map::Entry;

use kaspa_core;
use kaspa_wallet_core::error::Error;

use kaspa_wallet_pskt::prelude::*;
use secp256k1::Keypair as SecpKeypair;

use crate::error::ValidationError;
use corelib::payload::MessageIDs;
use corelib::util::get_recipient_address;
use corelib::wallet::{EasyKaspaWallet, NetworkInfo};
use corelib::withdraw::{filter_pending_withdrawals, WithdrawFXG};
use eyre::Result;
use hex::ToHex;
use hyperlane_core::{Decode, HyperlaneMessage, H256, U256};
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::{WithdrawalId, WithdrawalStatus};
use hyperlane_warp_route::TokenMessage;
use kaspa_addresses::Address as KaspaAddress;
use kaspa_addresses::Prefix::Testnet;
use kaspa_consensus_core::hashing::sighash::{
    calc_schnorr_signature_hash, SigHashReusedValuesUnsync,
};
use kaspa_consensus_core::tx::{ScriptPublicKey, TransactionOutpoint};
use kaspa_hashes;
use kaspa_txscript::pay_to_address_script;
use kaspa_wallet_core::utxo::NetworkParams;
use std::collections::HashMap;
use std::io::Cursor;
use tracing::{debug, error, info, warn};

pub async fn validate_withdrawal_batch(
    fxg: &WithdrawFXG,
    cosmos_client: &CosmosGrpcClient,
    mailbox_id: String,
    network: &NetworkInfo,
    escrow_public: EscrowPublic,
) -> Result<(), ValidationError> {
    for (pskt, messages) in fxg.bundle.iter().zip(fxg.messages.clone().into_iter()) {
        validate_withdrawals(
            PSKT::<Signer>::from(pskt.clone()),
            messages,
            cosmos_client,
            mailbox_id.clone(),
            network,
            escrow_public.clone(),
        )
        .await?;
    }
    Ok(())
}

pub async fn validate_withdrawals(
    pskt: PSKT<Signer>,
    messages: Vec<HyperlaneMessage>,
    cosmos_client: &CosmosGrpcClient,
    mailbox_id: String,
    network: &NetworkInfo,
    escrow_public: EscrowPublic,
) -> Result<(), ValidationError> {
    debug!(
        "Starting withdrawal validation for {} messages",
        messages.len()
    );

    let num_msg = messages.len();

    // Step 1: Check that all messages are delivered
    for message in &messages {
        let delivered_response = cosmos_client
            .delivered(mailbox_id.clone(), message.id().encode_hex())
            .await
            .map_err(|e| eyre::eyre!("Failed to check message delivery status: {}", e))?;

        if !delivered_response.delivered {
            let message_id = message.id().encode_hex();
            return Err(ValidationError::MessageNotDelivered { message_id });
        }
    }

    debug!("All messages are delivered");

    // Step 2: All messages should be not processed on the Hub
    // Filter out non-pending messages
    let (hub_outpoint, pending_messages) =
        filter_pending_withdrawals(messages.clone(), cosmos_client, None)
            .await
            .map_err(|e| eyre::eyre!("Get pending withdrawals: {}", e))?;

    // All given messages should be pending!
    if num_msg != pending_messages.len() {
        return Err(ValidationError::MessagesNotUnprocessed);
    }

    // Step 3: Check that PSKT contains the Hub outpoint as input
    let hub_outpoint_found = pskt.inputs.iter().any(|input| {
        input.previous_outpoint.transaction_id == hub_outpoint.transaction_id
            && input.previous_outpoint.index == hub_outpoint.index
    });

    if !hub_outpoint_found {
        return Err(ValidationError::HubOutpointNotFound {
            outpoint: hub_outpoint,
        });
    }

    // Find escrow input amount
    let escrow_input_amount = pskt.inputs.iter().fold(0, |acc, i| {
        // redeem_script is None for relayer input
        let rs = i.redeem_script.clone().unwrap_or_default();
        return if rs == escrow_public.redeem_script {
            acc + i.utxo_entry.as_ref().unwrap().amount
        } else {
            acc
        };
    });

    debug!("Hub outpoint found in PSKT inputs");

    // Step 4: Check that UTXO outputs align with withdrawals
    // Construct a multiset of expected outputs from HL messages.
    // Key:   recipiend + amount
    // Value: number of entries
    //
    // Such structure accounts for cases where one address might send several transfers
    // with the same amount.
    let mut expected_outputs: HashMap<(ScriptPublicKey, U256), i32> = HashMap::new();

    for message in pending_messages {
        let token_message = TokenMessage::read_from(&mut Cursor::new(&message.body))
            .map_err(|e| eyre::eyre!("Failed to parse TokenMessage from message body: {}", e))?;

        let recipient = ScriptPublicKey::from(pay_to_address_script(&get_recipient_address(
            token_message.recipient(),
            network.address_prefix,
        )));

        let key = (recipient, token_message.amount());
        *expected_outputs.entry(key).or_default() += 1;
    }

    // Ensure that all HL messages have outputs.
    // Also, calculate the total output amount of withdrawals + escrow change,
    // it should match the input escrow amount.
    let mut escrow_output_amount = 0;
    for output in &pskt.outputs {
        let key = (output.script_public_key.clone(), U256::from(output.amount));

        let e = expected_outputs.entry(key).and_modify(|v| *v -= 1);
        if let Entry::Occupied(e) = e {
            escrow_output_amount += output.amount;
            if *e.get() == 0 {
                e.remove();
            }
            continue;
        }

        if output.script_public_key == escrow_public.p2sh {
            escrow_output_amount += output.amount;
        }
    }

    if !expected_outputs.is_empty() {
        return Err(ValidationError::MissingOutputs);
    }

    if escrow_input_amount != escrow_output_amount {
        return Err(ValidationError::EscrowAmountMismatch {
            input_amount: escrow_input_amount,
            output_amount: escrow_output_amount,
        });
    }

    info!(
        "Withdrawal validation completed successfully for {} withdrawals",
        num_msg
    );

    Ok(())
}

// Mimic a parallel multi-validator signing process
// used by multisig demo only
pub fn sign_escrow_spend(e: &Escrow, pskt_unsigned: PSKT<Signer>) -> Result<PSKT<Combiner>, Error> {
    let signed: Vec<PSKT<Signer>> = e
        .keys
        .iter()
        .enumerate()
        .map(|(i, keypair)| {
            info!("-> Signer {} is signing their copy...", i + 1);
            sign_pskt(keypair, pskt_unsigned.clone(), vec![])
        })
        .collect::<Result<Vec<PSKT<Signer>>, Error>>()?;

    let mut combined = signed
        .first()
        .ok_or("No signatures provided to combine")?
        .clone()
        .combiner();

    for s in signed.iter().skip(1) {
        combined = (combined + s.clone()).unwrap();
    }

    Ok(combined)
}

pub fn sign_withdrawal_fxg(fxg: &WithdrawFXG, kp: &SecpKeypair) -> Result<Bundle> {
    let mut signed = Vec::new();
    // Iterate over (PSKT; associated HL messages) pairs
    for (pskt, messages) in fxg.bundle.iter().zip(fxg.messages.clone().into_iter()) {
        let pskt = PSKT::<Signer>::from(pskt.clone());

        let payload_msg_ids = MessageIDs::from(messages)
            .to_bytes()
            .map_err(|e| eyre::eyre!("Deserialize MessageIDs: {}", e))?;

        let signed_pskt = sign_pskt(kp, pskt, payload_msg_ids)?;

        signed.push(signed_pskt);
    }
    info!("Validator: signed pskts");
    let bundle = Bundle::from(signed);
    Ok(bundle)
}

// TODO: use wallet instead of raw keypair
pub fn sign_pskt(
    kp: &SecpKeypair,
    pskt: PSKT<Signer>,
    payload: Vec<u8>,
) -> Result<PSKT<Signer>, Error> {
    let reused_values = SigHashReusedValuesUnsync::new();
    let pk = kp.public_key();

    pskt.pass_signature_sync(|tx, sighashes| {
        // Sign tx as if it had a payload
        let mut tx_payload = tx.clone();
        tx_payload.tx.payload = payload;
        let tx_verifiable = tx_payload.as_verifiable();

        tx_payload
            .tx
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, _input)| {
                let hash = calc_schnorr_signature_hash(
                    &tx_verifiable,
                    idx,
                    sighashes[idx], // TODO: don't forget need to verify it's what's expected
                    &reused_values,
                );
                let msg = secp256k1::Message::from_digest_slice(&hash.as_bytes())
                    .map_err(|e| e.to_string())?;
                Ok(SignInputOk {
                    signature: Signature::Schnorr(kp.sign_schnorr(msg)),
                    pub_key: pk,
                    key_source: None,
                })
            })
            .collect()
    })
}
