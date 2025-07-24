use super::stats::RoundTripStats;
use corelib::user::deposit::deposit_with_payload;
use corelib::user::payload::make_deposit_payload_easy;
use corelib::wallet::EasyKaspaWallet;
use cosmrs::crypto::secp256k1::SigningKey;
use eyre::Result;
use hyperlane_core::AccountAddressType;
use hyperlane_core::H256;
use hyperlane_cosmos_native::signers::Signer;
use hyperlane_cosmos_native::GrpcProvider as CosmosGrpcClient;
use k256::ecdsa::SigningKey as K256SigningKey;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionId;
use rand_core::OsRng;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;


pub struct EasyHubKey {
    k: K256SigningKey,
}

impl EasyHubKey {
    pub fn new() -> Self {
        let hub_k = K256SigningKey::random(&mut OsRng);
        Self { k: hub_k }
    }
    pub fn signer(&self) -> Signer {
        let priv_k = self.k.to_bytes().to_vec();
        Signer::new(priv_k, "dym".to_string(), &AccountAddressType::Ethereum).unwrap()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hub_key() {
        let hub_key = EasyHubKey::new();
        let signer = hub_key.signer();
        let addr = signer.address_string;
        let priv_k = hub_key.k.to_bytes().to_vec();
        let priv_k_hex = hex::encode(priv_k);
        println!("priv_k_hex: {}", priv_k_hex);
        println!("addr: {}", addr);
    }
}
