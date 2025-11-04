use corelib::escrow::EscrowPublic;
use kaspa_addresses::Prefix;
use secp256k1::PublicKey;
use serde::Serialize;
use std::str::FromStr;

use super::validator;

pub fn get_escrow_address(pub_keys: Vec<&str>, required_signatures: u8) -> String {
    let pub_keys = pub_keys
        .iter()
        .map(|s| PublicKey::from_str(s).unwrap())
        .collect::<Vec<_>>();
    let e = EscrowPublic::from_pubs(pub_keys, Prefix::Testnet, required_signatures);
    e.addr.to_string()
}

#[derive(Debug, Serialize)]
pub struct ValidatorInfosWithEscrow {
    // HL style address to register on the Hub for the Kaspa multisig ISM
    pub validator_ism_addr: String,
    /// what validator will use to sign checkpoints for new deposits (and also progress indications)
    validator_ism_priv_key: String,
    /// secret key to sign kaspa inputs for withdrawals
    validator_escrow_secret: String,
    /// and pub key...
    validator_escrow_pub_key: String,
    /// the address the bridge end user should deposit to
    multisig_escrow_addr: String,
}

impl ValidatorInfosWithEscrow {
    pub fn to_string(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

pub fn create_validator_with_escrow() -> ValidatorInfosWithEscrow {
    let (v, pub_key) = validator::create_validator();

    let e = EscrowPublic::from_pubs(vec![pub_key], Prefix::Testnet, 1);

    ValidatorInfosWithEscrow {
        validator_ism_addr: v.validator_ism_addr,
        validator_ism_priv_key: v.validator_ism_priv_key,
        validator_escrow_secret: v.validator_escrow_secret,
        validator_escrow_pub_key: v.validator_escrow_pub_key,
        multisig_escrow_addr: e.addr.to_string(),
    }
}
