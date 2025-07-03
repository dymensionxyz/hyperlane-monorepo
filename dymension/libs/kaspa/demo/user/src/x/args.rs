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
                    Arg::new("escrow-address")
                        .help("The escrow address (like kaspatest:pzlq49spp66vkjjex0w7z8708f6zteqwr6swy33fmy4za866ne90v7e6pyrfr)")
                        .required(true)
                        .long("escrow-address")
                )
                .arg(
                    Arg::new("amount")
                        .help("The amount to deposit in sompi (like 100000)")
                        .required(true)
                        .long("amount")
                )
                .arg(
                    Arg::new("payload")
                        .help("The payload to deposit (hex without 0x prefix) (like 03000...00003e8)")
                        .required(true)
                        .long("payload")
                )
                .arg(
                    Arg::new("wrpc-url")
                        .help("The rpc url (like localhost:16210)")
                        .required(true)
                        .long("wrpc-url")
                )
                .arg(
                    Arg::new("network-id")
                        .help("The kaspa network id (like testnet-10)")
                        .required(true)
                        .long("network-id")
                )
                .arg(
                    Arg::new("wallet-secret")
                        .help("Local kaspa wallet keychain secret (not private key)")
                        .required(true)
                        .long("wallet-secret")
                ),
        )
}

impl DepositArgs {
    pub fn parse() -> Self {
        let m = cli().get_matches();
        let network_id = m.get_one::<String>("network-id").unwrap().clone();
        let network_id = NetworkId::from_str(&network_id).unwrap();
        DepositArgs {
            wallet_secret: m.get_one::<String>("wallet-secret").unwrap().clone(),
            amount: m.get_one::<String>("amount").unwrap().clone(),
            payload: m.get_one::<String>("payload").unwrap().clone(),
            escrow_address: m.get_one::<String>("escrow-address").unwrap().clone(),
            network_id: network_id,
            rpc_url: m.get_one::<String>("wrpc-url").unwrap().clone(),
        }
    }
}
