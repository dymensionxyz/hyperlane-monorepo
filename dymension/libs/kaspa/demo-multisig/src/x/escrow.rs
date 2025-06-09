use super::consts::*;


use kaspa_addresses::Address;
use kaspa_consensus_core::
    tx::
        ScriptPublicKey
    
;

use kaspa_wallet_core::prelude::*;
 // Import the prelude for easy access to traits/structs

use kaspa_txscript::{
    extract_script_pub_key_address, multisig_redeem_script, pay_to_script_hash_script,
};

use secp256k1::{Keypair, rand::thread_rng};


pub struct Escrow {
    pub keys: Vec<Keypair>,
    pub redeem_script: Vec<u8>,
    pub p2sh: ScriptPublicKey,
    pub addr: Address,
}

pub fn create_escrow() -> Escrow {
    let m = 2; // required
    let n = 2; // total
    let kps = (0..n)
        .map(|_| Keypair::new(secp256k1::SECP256K1, &mut thread_rng()))
        .collect::<Vec<_>>();
    let redeem_script =
        multisig_redeem_script(kps.iter().map(|pk| pk.x_only_public_key().0.serialize()), m)
            .unwrap();
    let p2sh = pay_to_script_hash_script(&redeem_script);
    let addr = extract_script_pub_key_address(&p2sh, ADDRESS_PREFIX).unwrap();
    Escrow {
        keys: kps.to_vec(),
        redeem_script,
        p2sh,
        addr,
    }
}
