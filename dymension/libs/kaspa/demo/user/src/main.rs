use std::env;
use kaspa_addresses::{Address, Prefix};
use std::str::FromStr;

use hyperlane_core::H256;
use relayer::withdraw_construction::get_recipient_address;

fn main() {
    let args: Vec<String> = env::args().collect();
    let addr_s = args.get(1).unwrap();
    let addr = Address::try_from(addr_s.as_str()).unwrap();
    println!("{}", addr.to_string());

    let recipient = H256::random();

    let prefix = Prefix::Testnet;
    let recipient_addr = get_recipient_address(recipient, prefix);
    println!("{}", recipient_addr.to_string());
}
