use ethers::signers::{LocalWallet, Signer};
use ethers::utils::hex;
use secp256k1::{rand::thread_rng, Keypair, PublicKey};
use serde::Serialize;

pub struct EthereumStyleSigner {
    pub address: String,
    pub private_key: String,
}

pub fn get_ethereum_style_signer() -> Result<EthereumStyleSigner, eyre::Error> {
    let wallet = LocalWallet::new(&mut thread_rng());

    let private_key_bytes = wallet.signer().to_bytes();
    let private_key_hex = format!("0x{}", hex::encode(private_key_bytes));

    let address = wallet.address();

    let address_str = serde_json::to_string(&address).unwrap();

    Ok(EthereumStyleSigner {
        address: address_str,
        private_key: private_key_hex,
    })
}
