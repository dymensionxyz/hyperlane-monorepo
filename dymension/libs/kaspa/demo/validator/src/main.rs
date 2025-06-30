use core::escrow::{generate_escrow_priv_key, Escrow};
use core::KaspaSecpKeypair;
use serde::{Deserialize, Serialize};
use validator::signer::get_ethereum_style_signer;
use kaspa_addresses::Prefix;
use core::escrow::EscrowPublic;

#[derive(Debug, Serialize)]
struct Validator {
    validator_ism_addr: String,
    validator_ism_priv_key: String,
    validator_escrow_secret: String,
    multisig_escrow_addr: String,
}

fn main() {
    let kp = generate_escrow_priv_key();
    let s = serde_json::to_string(&kp).unwrap();

    let signer = get_ethereum_style_signer().unwrap();

    let e = EscrowPublic::from_pubs(vec![kp.public_key()], Prefix::Testnet, 1);

    let v = Validator {
        validator_escrow_secret: s,
        validator_ism_addr: signer.address,
        validator_ism_priv_key: signer.private_key,
        multisig_escrow_addr: e.addr.to_string(),
    };

    println!("{}", serde_json::to_string_pretty(&v).unwrap());
}
