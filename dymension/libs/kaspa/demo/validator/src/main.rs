use core::escrow::{generate_escrow_priv_key, Escrow};
use core::KaspaSecpKeypair;
use secp256k1::{rand::thread_rng, Keypair, PublicKey};

fn main() {
    let kp = generate_escrow_priv_key();
    println!("kp: {:?}", kp);
}
