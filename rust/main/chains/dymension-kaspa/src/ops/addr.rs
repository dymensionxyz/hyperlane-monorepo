use hyperlane_core::H256;
use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::ScriptPublicKey;
use kaspa_txscript::pay_to_address_script;

/// Converts a Kaspa address to an H256 hash.
///
/// Kaspa PubKey addresses have a 32-byte payload that maps directly to H256.
///
/// # Panics
/// Panics if the address payload is not exactly 32 bytes.
pub fn kaspa_address_to_h256(address: &Address) -> H256 {
    let bytes_32: [u8; 32] = address.payload.as_slice().try_into().unwrap();
    H256::from_slice(&bytes_32)
}

/// Converts a Kaspa address string to a hex recipient string for Hyperlane.
///
/// The output is prefixed with "0x" for use in Hyperlane transfer recipient fields.
///
/// # Panics
/// Panics if the address string is invalid or the payload is not 32 bytes.
pub fn kaspa_address_to_hex_recipient(kaspa_addr: &str) -> String {
    let addr = Address::try_from(kaspa_addr).unwrap();
    let h256 = kaspa_address_to_h256(&addr);
    format!("0x{}", hex::encode(h256.as_bytes()))
}

/// Converts an H256 hash to a Kaspa address with the specified network prefix.
///
/// Always creates a PubKey version address.
pub fn h256_to_kaspa_address(recipient: H256, prefix: Prefix) -> Address {
    Address::new(prefix, Version::PubKey, recipient.as_bytes())
}

/// Converts an H256 hash to a Kaspa ScriptPublicKey.
///
/// Creates the pay-to-address script for the corresponding Kaspa address.
pub fn h256_to_script_pubkey(recipient: H256, prefix: Prefix) -> ScriptPublicKey {
    pay_to_address_script(&h256_to_kaspa_address(recipient, prefix))
}

/// Converts a Kaspa address to its ScriptPublicKey.
pub fn address_to_script_pubkey(address: &Address) -> ScriptPublicKey {
    pay_to_address_script(address)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_kaspa_address_h256_roundtrip() {
        let kaspa_str = "kaspatest:qq053k5up93kj5a3l08zens447s62ndstyrnuusserehq4laun7es8q29fwd4";
        let kaspa = Address::try_from(kaspa_str).unwrap();
        let h256 = kaspa_address_to_h256(&kaspa);
        let kaspa_actual = h256_to_kaspa_address(h256, Prefix::Testnet);

        assert_eq!(kaspa, kaspa_actual)
    }

    #[test]
    fn test_h256_to_kaspa_address_creates_valid_script() {
        let h256 =
            H256::from_str("0xbcff7587f574e249b549329291239682d6d3481ccbc5997c79770a607ab3ec98")
                .unwrap();
        let address = h256_to_kaspa_address(h256, Prefix::Testnet);
        let script_pubkey = h256_to_script_pubkey(h256, Prefix::Testnet);
        assert!(!script_pubkey.script().is_empty());
        assert_eq!(address.prefix, Prefix::Testnet);
    }

    #[test]
    fn test_hex_recipient_encoding_roundtrip() {
        let original = "kaspatest:qzlq49spp66vkjjex0w7z8708f6zteqwr6swy33fmy4za866ne90vhy54uh3j";
        let hex_recipient = kaspa_address_to_hex_recipient(original);

        let decoded = H256::from_slice(&hex::decode(&hex_recipient[2..]).unwrap());
        let recovered = h256_to_kaspa_address(decoded, Prefix::Testnet);

        assert_eq!(recovered.to_string(), original);
    }
}
