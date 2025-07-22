use cosmrs::crypto::{secp256k1::SigningKey, PublicKey};
use hyperlane_core::{AccountAddressType, ChainResult, H256};
use std::str::FromStr;

use crate::{CosmosAddress, HyperlaneCosmosError};

#[derive(Clone, Debug)]
/// Signer for cosmos chain
pub struct Signer {
    /// public key
    pub public_key: PublicKey,
    /// cosmos address
    pub address: CosmosAddress,
    /// precomputed address, because computing it is a fallible operation
    /// and we want to avoid returning `Result`
    pub address_string: String,
    /// address prefix
    pub prefix: String,
    /// private key
    private_key: Vec<u8>,
}

impl Signer {
    /// create new signer
    ///
    /// # Arguments
    /// * `private_key` - private key for signer
    /// * `prefix` - prefix for signer address
    /// * `account_address_type` - the type of account address used for signer
    pub fn new(
        private_key: Vec<u8>,
        prefix: String,
        account_address_type: &AccountAddressType,
    ) -> ChainResult<Self> {
        let address = CosmosAddress::from_privkey(&private_key, &prefix, account_address_type)?;
        let address_string = address.address();
        let signing_key = Self::build_signing_key(&private_key)?;
        let public_key = signing_key.public_key();
        Ok(Self {
            public_key,
            private_key,
            address,
            address_string,
            prefix,
        })
    }

    /// Build a SigningKey from a private key. This cannot be
    /// precompiled and stored in `Signer`, because `SigningKey` is not `Sync`.
    pub fn signing_key(&self) -> ChainResult<SigningKey> {
        Self::build_signing_key(&self.private_key)
    }

    fn build_signing_key(private_key: &Vec<u8>) -> ChainResult<SigningKey> {
        Ok(SigningKey::from_slice(private_key.as_slice())
            .map_err(Box::new)
            .map_err(Into::<HyperlaneCosmosError>::into)?)
    }

    /// gets digest of the cosmos account
    pub fn address_h256(&self) -> H256 {
        self.address.digest()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use hyperlane_core::AccountAddressType;

    #[test]
    fn test_create_new_signer() {
        let k_s = "0xe95baa20c85b39e4b67549b6ef0c4528681005f9b5b86b073a12dbaa712e5d39";
        let k = H256::from_str(k_s).unwrap();
        let prefix = "dym".to_string();
        let account_address_type = AccountAddressType::Ethereum;

        let signer = Signer::new(k.as_bytes().to_vec(), prefix.clone(), &account_address_type)
            .expect("should create signer");
        assert_eq!(signer.address_string, "dym1mh2cyxppuvn7c2z0dg84qyjy8w4kn9307ahgxt");
    }
}
