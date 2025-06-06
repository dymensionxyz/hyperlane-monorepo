use kaspa_core::kaspad_env::version;

use clap::{Arg, Command};



pub fn cli() -> Command {
    Command::new("rothschild")
        .about(format!(
            "{} (rothschild) v{}",
            env!("CARGO_PKG_DESCRIPTION"),
            version()
        ))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("private-key")
                .long("private-key")
                .short('k')
                .value_name("private-key")
                .help("Private key in hex format"),
        )
        .arg(
            Arg::new("rpcserver")
                .long("rpcserver")
                .short('s')
                .value_name("rpcserver")
                .default_value("localhost:16210")
                .help("RPC server"),
        )
}

pub struct Args {
    pub private_key: Option<String>,
    pub rpc_server: String,
}

impl Args {
    pub fn parse() -> Self {
        let m = cli().get_matches();
        Args {
            private_key: m.get_one::<String>("private-key").cloned(),
            rpc_server: m
                .get_one::<String>("rpcserver")
                .cloned()
                .unwrap_or("localhost:16210".to_owned()),
        }
    }
}
