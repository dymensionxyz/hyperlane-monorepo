#![allow(unused)] // TODO: remove

use kaspa_addresses::Address;
mod x;
use x::args::Args;
use x::deposit::{demo, DemoArgs};
use hardcode::e2e::*;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let demo_args = DemoArgs::default();

    demo_args.payload = args.payload;
    demo_args.only_deposit = args.only_deposit;
    let amt = args.amount.unwrap_or(DEPOSIT_AMOUNT);
    let escrow_address = if let Some(e) = args.escrow_address {
        Address::try_from(e)?
    } else {
        Address::try_from(ESCROW_ADDRESS)?
    };

    demo_args.amt = amt;
    demo_args.escrow_address = escrow_address;


    if let Err(e) = demo(demo_args).await {
        eprintln!("Error: {}", e);
    }
}
