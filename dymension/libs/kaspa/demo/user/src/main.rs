use std::env;
use kaspa_addresses::Address;
use std::str::FromStr;

fn main() {
    let args: Vec<String> = env::args().collect();
    let addr_s = args[0];
    let addr = kaspa_addresses::Address::from_str(&addr_s).unwrap();
    println!("{}", addr.to_string());
}
