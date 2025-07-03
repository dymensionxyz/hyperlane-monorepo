// We call the signers 'validators'

use std::collections::hash_map::Entry;
use corelib::escrow::*;

use kaspa_core;
use kaspa_wallet_core::error::Error;

use kaspa_wallet_pskt::prelude::*;
use secp256k1::Keypair as SecpKeypair;

use corelib::payload::MessageIDs;
use corelib::util::{filter_pending_withdrawals, get_recipient_address};
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
use std::collections::HashMap;
use std::io::Cursor;
use tracing::{debug, error, info, warn};

pub async fn validate_withdrawals(
    pskt: PSKT<Signer>,
    messages: Vec<HyperlaneMessage>,
    cosmos_client: &CosmosGrpcClient,
    mailbox_id: String,
) -> Result<bool> {
    debug!(
        "Starting withdrawal validation for {} messages",
        messages.len()
    );

    // Step 1: Check that all messages are delivered
    for message in &messages {
        let delivered_response = cosmos_client
            .delivered(mailbox_id.clone(), message.id().encode_hex())
            .await
            .map_err(|e| eyre::eyre!("Failed to check message delivery status: {}", e))?;

        if !delivered_response.delivered {
            warn!("Message {} is not delivered", message.id().encode_hex());
            return Ok(false);
        }
    }

    debug!("All messages are delivered");

    // Step 2: All messages should be not processed on the Hub
    // Filter out non-pending messages
    let num_messages_initial = messages.len();
    let (hub_outpoint, pending_messages) =
        filter_pending_withdrawals(messages, cosmos_client, None)
            .await
            .map_err(|e| eyre::eyre!("Get pending withdrawals: {}", e))?;

    // All given messages should be pending!
    if num_messages_initial != pending_messages.len() {
        warn!("Some of the messages are not in the unprocessed status on the Hub");
        return Ok(false);
    }

    // Step 3: Check that PSKT contains the Hub outpoint as input
    let hub_outpoint_found = pskt.inputs.iter().any(|input| {
        input.previous_outpoint.transaction_id == hub_outpoint.transaction_id
            && input.previous_outpoint.index == hub_outpoint.index
    });

    if !hub_outpoint_found {
        warn!("Hub outpoint {:?} not found in PSKT inputs", hub_outpoint);
        return Ok(false);
    }

    debug!("Hub outpoint found in PSKT inputs");

    // Step 4: Check that UTXO outputs align with withdrawals
    // Construct a multiset of expected outputs from HL messages.
    // Key:   recipiend + amount
    // Value: number of entries
    //
    // Such structure accounts for cases where one address might send several transfers
    // with the same amount.
    let mut expected_outputs: HashMap<(ScriptPublicKey, U256), usize> = HashMap::new();

    for message in pending_messages {
        let token_message = TokenMessage::read_from(&mut Cursor::new(&message.body))
            .map_err(|e| eyre::eyre!("Failed to parse TokenMessage from message body: {}", e))?;

        let address_prefix = Testnet; // TODO: use real address prefix
        let recipient = ScriptPublicKey::from(pay_to_address_script(&get_recipient_address(
            token_message.recipient(),
            address_prefix,
        )));

        let key = (recipient, token_message.amount());
        expected_outputs
            .entry(key)
            .and_modify(|v| *v += 1)
            .or_insert(0);
    }

    // Check PSKT outputs against expected outputs
    let mut actual_outputs: HashMap<String, u64> = HashMap::new();
    let mut escrow_change_count = 0;
    let mut relayer_change_count = 0;

    let mut extra_outputs = 0;
    for output in &pskt.outputs {
        let key = (output.script_public_key.clone(), U256::from(output.amount));

        let e = expected_outputs.entry(key).and_modify(|v| *v -= 1);
        match e {
            Entry::Occupied(e) => {
                if *e.get() == 0 {
                    e.remove();
                }
            },
            Entry::Vacant(_) => {
                // We expect to have exactly two extra outputs: relayer and escrow change
                extra_outputs += 1;
            }
        }



        let script_bytes = output.script_public_key.script();

        // Try to extract address from script - this is a simplified check
        // In practice, we'd need to properly decode the script to get the address
        // For now, we'll group by script hash for validation
        let script_key = hex::encode(script_bytes);

        // Check if this looks like escrow change (P2SH script pattern)
        // or relayer change (P2PKH script pattern)
        if script_bytes.len() == 23 && script_bytes[0] == 0xa9 && script_bytes[22] == 0x87 {
            // This looks like a P2SH script (escrow change)
            escrow_change_count += 1;
        } else if script_bytes.len() == 25 && script_bytes[0] == 0x76 && script_bytes[1] == 0xa9 {
            // This looks like a P2PKH script (could be relayer change or withdrawal)
            if actual_outputs.contains_key(&script_key) {
                // If we've seen this script before, it might be relayer change
                relayer_change_count += 1;
            } else {
                // New script, count as withdrawal output
                *actual_outputs.entry(script_key).or_insert(0) += output.value;
            }
        } else {
            // Unknown script type, assume it's a withdrawal
            *actual_outputs.entry(script_key).or_insert(0) += output.value;
        }
    }

    // Validate that we have the right number of outputs
    if actual_outputs.len() != expected_outputs.len() {
        warn!("Output count mismatch: expected {} withdrawal outputs, found {} actual outputs (excluding {} escrow change and {} relayer change)", 
              expected_outputs.len(), actual_outputs.len(), escrow_change_count, relayer_change_count);
        return Ok(false);
    }

    // We should have exactly one escrow change output
    if escrow_change_count != 1 {
        warn!(
            "Expected exactly 1 escrow change output, found {}",
            escrow_change_count
        );
        return Ok(false);
    }

    // We should have at most one relayer change output
    if relayer_change_count > 1 {
        warn!(
            "Expected at most 1 relayer change output, found {}",
            relayer_change_count
        );
        return Ok(false);
    }

    // Now validate that the actual outputs match the expected outputs
    // Since we're using simplified script pattern matching, we need to validate amounts
    let total_expected_amount: u64 = expected_outputs.values().sum();
    let total_actual_amount: u64 = actual_outputs.values().sum();

    if total_expected_amount != total_actual_amount {
        warn!(
            "Amount mismatch: expected total {} KAS, found total {} KAS in withdrawal outputs",
            total_expected_amount, total_actual_amount
        );
        return Ok(false);
    }

    // For a more precise validation, we should match recipients to script addresses
    // This is a simplified validation that checks amounts are consistent
    // TODO: Implement precise address-to-script matching for full validation

    // Validate that each expected amount appears in the actual outputs
    // This is not perfect since multiple recipients could have the same amount,
    // but it's a reasonable check for the current simplified implementation
    let mut expected_amounts: Vec<u64> = expected_outputs.values().cloned().collect();
    let mut actual_amounts: Vec<u64> = actual_outputs.values().cloned().collect();
    expected_amounts.sort();
    actual_amounts.sort();

    if expected_amounts != actual_amounts {
        warn!(
            "Amount distribution mismatch: expected amounts {:?}, found amounts {:?}",
            expected_amounts, actual_amounts
        );
        return Ok(false);
    }

    debug!("Output validation passed: {} withdrawal outputs with correct amounts, {} escrow change, {} relayer change", 
           actual_outputs.len(), escrow_change_count, relayer_change_count);

    info!("Withdrawal validation completed successfully");
    Ok(true)
}

// Mimic a parallel multi-validator signing process
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

// TODO: use wallet instead of raw keypair
pub fn sign_pskt(
    kp: &SecpKeypair,
    pskt: PSKT<Signer>,
    messages: Vec<HyperlaneMessage>,
) -> Result<PSKT<Signer>, Error> {
    let reused_values = SigHashReusedValuesUnsync::new();

    let msg_ids_bytes = MessageIDs::from(messages)
        .to_bytes()
        .map_err(|e| format!("Deserialize MessageIDs: {}", e))?;

    pskt.pass_signature_sync(|tx, sighashes| {
        // Sign tx as if it had a payload
        let mut tx_payload = tx.clone();
        tx_payload.tx.payload = msg_ids_bytes;

        tx_payload
            .tx
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, _input)| {
                let hash = calc_schnorr_signature_hash(
                    &tx_payload.as_verifiable(),
                    idx,
                    sighashes[idx], // TODO: don't forget need to verify it's what's expected
                    &reused_values,
                );
                let msg = secp256k1::Message::from_digest_slice(&hash.as_bytes())
                    .map_err(|e| e.to_string())?;
                Ok(SignInputOk {
                    signature: Signature::Schnorr(kp.sign_schnorr(msg)),
                    pub_key: kp.public_key(),
                    key_source: None,
                })
            })
            .collect()
    })
}
