use clap::{Arg, Command};

pub fn common_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("verbose")
            .short('v')
            .long("verbose")
            .help("Enable verbose output")
            .action(clap::ArgAction::SetTrue),
    )
}

pub fn cli() -> Command {
    Command::new("user")
        .about("User tools")
        .version("1.0")
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
        .subcommand(common_args(
            Command::new("validator").about("Validator tools"),
        ))
}
