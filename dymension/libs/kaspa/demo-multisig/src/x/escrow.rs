use super::consts::*;

use kaspa_addresses::Address;
use kaspa_consensus_core::tx::ScriptPublicKey;

use kaspa_wallet_core::prelude::*;

use kaspa_txscript::{
    extract_script_pub_key_address, multisig_redeem_script, pay_to_script_hash_script,
};

use secp256k1::{Keypair, PublicKey, rand::thread_rng};

pub struct Escrow {
    pub keys: Vec<Keypair>,
    pub required_signatures: u8,
}

pub struct EscrowPublic {
    pub n: u8,
    pub m: u8,
    pub redeem_script: Vec<u8>,
    pub p2sh: ScriptPublicKey,
    pub addr: Address,
    pub pubs: Vec<PublicKey>,
}

impl Escrow {
    pub fn new(n: u8) -> Self {
        let kps = (0..n)
            .map(|_| Keypair::new(secp256k1::SECP256K1, &mut thread_rng()))
            .collect::<Vec<_>>();

        Self {
            keys: kps,
            required_signatures: n,
        }
    }

    pub fn public(&self) -> EscrowPublic {
        let redeem_script = multisig_redeem_script(
            self.keys
                .iter()
                .map(|pk| pk.x_only_public_key().0.serialize()),
            self.required_signatures as usize,
        )
        .unwrap();

        let p2sh = pay_to_script_hash_script(&redeem_script);
        let addr = extract_script_pub_key_address(&p2sh, ADDRESS_PREFIX).unwrap();

        EscrowPublic {
            n: self.keys.len() as u8,
            m: self.required_signatures,
            redeem_script,
            p2sh,
            addr,
            pubs: self.keys.iter().map(|kp| kp.public_key()).collect(),
        }
    }
}
