use hex::{FromHex, ToHex};
use hyperlane_core::H256;
use kaspa_addresses::{Address, Prefix};

pub fn convert_addr(addr_s: &str) -> String {
    let addr = Address::try_from(addr_s).unwrap();
    println!("{}", addr.to_string());
    let bz = addr.payload.as_slice();
    let bz_hex = hex::encode(bz);
    let s = format!("0x{}", bz_hex);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use relayer::withdraw_construction::get_recipient_address;

    #[test]
    fn test_convert_addr_roundtrip() {
        let test_addr = "kaspatest:pzlq49spp66vkjjex0w7z8708f6zteqwr6swy33fmy4za866ne90v7e6pyrfr";
        let result = convert_addr(test_addr);

        let unprefixed = s.chars().skip(2).collect::<String>();
        let unhexed = hex::decode(unprefixed).unwrap();
        let decoded = H256::from_slice(&unhexed);

        let prefix = Prefix::Testnet;
        let recipient_addr = get_recipient_address(decoded, prefix);
        if recipient_addr.to_string() != test_addr {
            println!("{}", "something wrong");
        }

        assert_eq!(result, test_addr);
    }
}
