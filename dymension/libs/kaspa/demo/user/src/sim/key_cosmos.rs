use hyperlane_core::AccountAddressType;
use hyperlane_cosmos_native::signers::Signer;
use k256::ecdsa::SigningKey as K256SigningKey;
use rand_core::OsRng;

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
