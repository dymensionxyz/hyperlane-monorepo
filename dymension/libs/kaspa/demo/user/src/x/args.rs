use corelib::env::version;
use std::str::FromStr;

use super::deposit::DepositArgs;
use clap::{Arg, Command};
use kaspa_consensus_core::network::NetworkId;

pub fn common_args(cmd: Command) -> Command {
    cmd
}

pub fn cli() -> Command {
    Command::new("user")
        .about(format!(
            "Tools for users, validator operators, developers etc (version: {})",
            version()
        ))
        .version(version())
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            common_args(Command::new("recipient").about("Convert address")).arg(
                Arg::new("ADDRESS")
                    .help("The address to be converted")
                    .required(true)
                    .index(1),
            ),
        )
        .subcommand(common_args(Command::new("validator").about(
            "Generate all the info needed for a validator with a 1 of 1 multisig escrow",
        )))
        .subcommand(
            common_args(Command::new("deposit").about("Make a user deposit"))
                /*
                need args for:
                wallet secret, network id, rpc url, payload string, escrow addr, amt
                 */
                .arg(
                    Arg::new(DEPOSIT_FLAG_ESCROW_ADDRESS)
                        .help("The escrow address (like kaspatest:pzlq49spp66vkjjex0w7z8708f6zteqwr6swy33fmy4za866ne90v7e6pyrfr)")
                        .required(true)
                        .long(DEPOSIT_FLAG_ESCROW_ADDRESS)
                )
                .arg(
                    Arg::new(DEPOSIT_FLAG_AMOUNT)
                        .help("The amount to deposit in sompi (like 100000)")
                        .required(true)
                        .long(DEPOSIT_FLAG_AMOUNT)
                )
                .arg(
                    Arg::new(DEPOSIT_FLAG_PAYLOAD)
                        .help("The payload to deposit (hex without 0x prefix) (like 03000...00003e8)")
                        .required(true)
                        .long(DEPOSIT_FLAG_PAYLOAD)
                )
                .arg(
                    Arg::new(DEPOSIT_FLAG_WRPC_URL)
                        .help("The rpc url (like localhost:16210)")
                        .required(true)
                        .long(DEPOSIT_FLAG_WRPC_URL)
                )
                .arg(
                    Arg::new(DEPOSIT_FLAG_NETWORK_ID)
                        .help("The kaspa network id (like testnet-10)")
                        .required(true)
                        .long(DEPOSIT_FLAG_NETWORK_ID)
                )
                .arg(
                    Arg::new(DEPOSIT_FLAG_WALLET_SECRET)
                        .help("Local kaspa wallet keychain secret (not private key)")
                        .required(true)
                        .long(DEPOSIT_FLAG_WALLET_SECRET)
                ),
        )
}

const DEPOSIT_FLAG_ESCROW_ADDRESS: &str = "escrow-address";
const DEPOSIT_FLAG_AMOUNT: &str = "amount";
const DEPOSIT_FLAG_PAYLOAD: &str = "payload";
const DEPOSIT_FLAG_WRPC_URL: &str = "wrpc-url";
const DEPOSIT_FLAG_NETWORK_ID: &str = "network-id";
const DEPOSIT_FLAG_WALLET_SECRET: &str = "wallet-secret";

impl DepositArgs {
    pub fn parse() -> Self {
        let m = cli().get_matches();
        let network_id = m.get_one::<String>(DEPOSIT_FLAG_NETWORK_ID).unwrap().clone();
        let network_id = NetworkId::from_str(&network_id).unwrap();
        DepositArgs {
            wallet_secret: m.get_one::<String>(DEPOSIT_FLAG_WALLET_SECRET).unwrap().clone(),
            amount: m.get_one::<String>(DEPOSIT_FLAG_AMOUNT).unwrap().clone(),
            payload: m.get_one::<String>(DEPOSIT_FLAG_PAYLOAD).unwrap().clone(),
            escrow_address: m.get_one::<String>(DEPOSIT_FLAG_ESCROW_ADDRESS).unwrap().clone(),
            network_id: network_id,
            rpc_url: m.get_one::<String>(DEPOSIT_FLAG_WRPC_URL).unwrap().clone(),
        }
    }
}
