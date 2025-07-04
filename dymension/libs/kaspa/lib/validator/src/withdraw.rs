// We call the signers 'validators'

use corelib::escrow::*;

use kaspa_core::info;
use kaspa_wallet_core::error::Error;

use kaspa_wallet_pskt::prelude::*;
use secp256k1::Keypair as SecpKeypair;

use corelib::payload::MessageIDs;
use corelib::withdraw::WithdrawFXG;
use eyre::Result;
use hyperlane_core::HyperlaneMessage;
use kaspa_consensus_core::hashing::sighash::{
    calc_schnorr_signature_hash, SigHashReusedValuesUnsync,
};

pub async fn validate_withdrawals(fxg: &WithdrawFXG) -> Result<bool> {
    Ok(true)
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

pub fn sign_withdrawal_fxg(fxg: &WithdrawFXG, keypair: &SecpKeypair) -> Result<Bundle> {
    let mut signed = Vec::new();
    // Iterate over (PSKT; associated HL messages) pairs
    for (pskt, hl_messages) in fxg.bundle.iter().zip(fxg.messages.clone().into_iter()) {
        let pskt = PSKT::<Signer>::from(pskt.clone());

        let payload = MessageIDs::from(hl_messages)
            .to_bytes()
            .map_err(|e| eyre::eyre!("Deserialize MessageIDs: {}", e))?;

        let signed_pskt = sign_pskt(keypair, pskt, payload)?;

        signed.push(signed_pskt);
    }
    info!("Validator: signed pskts");
    let bundle = Bundle::from(signed);
    Ok(bundle)
}

// TODO: use wallet instead of raw keypair
pub fn sign_pskt(
    keypair: &SecpKeypair,
    pskt: PSKT<Signer>,
    payload: Vec<u8>,
) -> Result<PSKT<Signer>, Error> {
    let reused_values = SigHashReusedValuesUnsync::new();

    pskt.pass_signature_sync(|tx, sighashes| {
        let mut with_payload = tx.clone();
        with_payload.tx.payload = payload;

        with_payload
            .tx
            .inputs
            .iter()
            .enumerate()
            .map(|(idx, _input)| {
                let hash = calc_schnorr_signature_hash(
                    &with_payload.as_verifiable(),
                    idx,
                    sighashes[idx], // TODO: don't forget need to verify it's what's expected
                    &reused_values,
                );
                let msg = secp256k1::Message::from_digest_slice(&hash.as_bytes())
                    .map_err(|e| e.to_string())?;
                Ok(SignInputOk {
                    signature: Signature::Schnorr(keypair.sign_schnorr(msg)),
                    pub_key: keypair.public_key(),
                    key_source: None,
                })
            })
            .collect()
    })
}
