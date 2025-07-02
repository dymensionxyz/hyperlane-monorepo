use corelib::user::deposit::deposit_impl;
use eyre::Result;
use kaspa_consensus_core::network::NetworkId;

pub struct DepositArgs {
    pub wallet_secret: String,
    pub amount: String,
    pub payload: String,
    pub escrow_address: String,
    pub network_id: NetworkId,
    pub rpc_url: String,
}

pub fn do_deposit(args: DepositArgs) -> Result<()> {
    let w = get_wallet(&args.wallet_secret, &args.network_id, &args.rpc_url);
    let s = Secret::from(args.wallet_secret);
    let a = Address::from(args.escrow_address);
    let amt = args.amount.parse::<u64>().unwrap();
    let payload = args.payload.as_bytes().to_vec();

    let res = deposit_impl(&w, &s, a, amt, args.payload.as_bytes().to_vec());

    Ok(())
}
