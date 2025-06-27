use core::escrow::{generate_escrow_priv_key, Escrow};
use core::KaspaSecpKeypair;
use secp256k1::{rand::thread_rng, Keypair, PublicKey};
use serde::{Deserialize, Serialize};

fn main() {
    let kp = generate_escrow_priv_key();
    let s = serde_json::to_string(&kp).unwrap();
    println!("secret key: {}", s)
}
