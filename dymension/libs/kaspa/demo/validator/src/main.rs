use core::escrow::Escrow;
use core::KaspaSecpKeypair;
use secp256k1::{rand::thread_rng, Keypair, PublicKey};

fn main() {
    let kp = KaspaSecpKeypair::new(secp256k1::SECP256K1, &mut thread_rng());
    println!("kp: {:?}", kp);
}
