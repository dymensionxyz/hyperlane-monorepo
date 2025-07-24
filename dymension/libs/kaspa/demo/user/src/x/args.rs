use super::deposit::DepositArgs;
use crate::sim::{Params, SimulateTrafficArgs, TaskArgs};
use clap::{Args, Parser, Subcommand};
use hyperlane_core::H256;
use kaspa_addresses::Address;
use kaspa_consensus_core::network::NetworkId;
use std::str::FromStr;
use std::str::FromStr;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "demo-user",
    author,
    version, // `version()` is automatically called by clap
    about = "Tools for users, validator operators, developers etc",
    subcommand_required = true,
    arg_required_else_help = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Convert kaspa address (like kaspatest:pzlq49sp...y4za866ne90v7e6pyrfr) to HL address (like 0x000000000..0000000)
    Recipient(RecipientCli),
    /// Get the escrow address for some secp256k1 pub keys (like kaspatest:pzlq49spp6...66ne90v7e6pyrfr)
    Escrow(EscrowCli),
    /// Generate all the info needed for a validator (without escrow address)
    Validator(ValidatorCli),
    /// Generate all the info needed for a validator with a 1 of 1 multisig escrow
    #[clap(name = "validator-with-escrow")]
    ValidatorAndEscrow,
    /// Make a user deposit (to escrow)
    Deposit(DepositCli),
    /// Create a relayer
    Relayer,
    /// Simulate traffic
    #[clap(name = "sim")]
    SimulateTraffic(SimulateTrafficCli),
}

#[derive(Args, Debug)]
pub struct ValidatorCli {
    /// Generate more than one validator at a time
    #[arg(required = false, index = 1, default_value = "1")]
    pub n: u32,
}

#[derive(Args, Debug)]
pub struct EscrowCli {
    /// Comma separated list of pub keys
    #[arg(required = true, index = 1)]
    pub pub_keys: String,
    /// Required signatures
    #[arg(required = true, index = 2)]
    pub required_signatures: u8,
}

#[derive(Args, Debug)]
pub struct RecipientCli {
    /// The address to be converted
    #[arg(required = true, index = 1)]
    pub address: String,
}

#[derive(Args, Debug)]
pub struct SimulateTrafficCli {
    #[arg(required = true, index = 1)]
    pub time_limit: u64,
    #[arg(required = true, index = 2)]
    pub budget: u64,
    #[arg(required = true, index = 3)]
    pub ops_per_minute: u64,

    #[arg(required = true, index = 4)]
    pub domain_kas: u32,
    #[arg(required = true, index = 5)]
    pub token_kas_placeholder: H256,
    #[arg(required = true, index = 6)]
    pub domain_hub: u32,
    #[arg(required = true, index = 7)]
    pub token_hub: H256,
    #[arg(required = true, index = 8)]
    pub escrow_address: String,
}

impl SimulateTrafficCli {
    pub fn to_sim_args(&self) -> SimulateTrafficArgs {
        let addr = Address::try_from(self.escrow_address.clone()).unwrap();
        SimulateTrafficArgs {
            params: Params {
                time_limit: Duration::from_secs(self.time_limit),
                budget: self.budget,
                ops_per_minute: self.ops_per_minute,
            },
            task_args: TaskArgs {
                domain_kas: self.domain_kas,
                token_kas_placeholder: self.token_kas_placeholder,
                domain_hub: self.domain_hub,
                token_hub: self.token_hub,
                escrow_address: addr,
            },
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct DepositCli {
    /// The escrow address (like kaspatest:pzlq49spp66vkjjex0w7z8708f6zteqwr6swy33fmy4za866ne90v7e6pyrfr)
    #[arg(long, required = true)]
    pub escrow_address: String,

    /// The amount to deposit in sompi (like 100000)
    #[arg(long, required = true)]
    pub amount: String,

    /// The payload to deposit (hex without 0x prefix)
    #[arg(long, required = false, default_value = "")]
    pub payload: String,

    /// The wRPC url (like localhost:17210)
    #[arg(long("wrpc-url"), required = true)]
    pub rpc_url: String,

    /// The kaspa network id (like testnet-10)
    #[arg(long("network-id"), required = true)]
    // If you have a NetworkId type that implements `FromStr`, you can use it directly:
    // pub network_id: kaspa_consensus_core::network::NetworkId,
    pub network_id: String,

    /// Local kaspa wallet keychain secret (not private key)
    #[arg(long("wallet-secret"), required = true)]
    pub wallet_secret: String,

    /// Local kaspa wallet directory
    #[arg(long("wallet-dir"), required = false)]
    pub wallet_dir: Option<String>,
}

impl DepositCli {
    pub fn to_deposit_args(&self) -> DepositArgs {
        DepositArgs {
            escrow_address: self.escrow_address.clone(),
            amount: self.amount.clone(),
            payload: self.payload.clone(),
            network_id: NetworkId::from_str(&self.network_id).unwrap(),
            rpc_url: self.rpc_url.clone(),
            wallet_secret: self.wallet_secret.clone(),
            wallet_dir: self.wallet_dir.clone(),
        }
    }
}
