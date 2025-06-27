use core::escrow::{generate_escrow_priv_key, Escrow};
use core::KaspaSecpKeypair;
use serde::{Deserialize, Serialize};
use validator::signer::get_ethereum_style_signer;

#[derive(Debug)]
struct Validator {
    kaspa_secret: String,
    ism_address: String,
    ism_private_key: String,
}

fn main() {
    let kp = generate_escrow_priv_key();
    let s = serde_json::to_string(&kp).unwrap();

    let signer = get_ethereum_style_signer().unwrap();
    let v = Validator {
        kaspa_secret: s,
        ism_address: signer.address,
        ism_private_key: signer.private_key,
    };

    println!("validator: {:?}", v);
}
