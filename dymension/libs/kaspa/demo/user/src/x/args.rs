use clap::{Arg, Command};

pub fn common_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("verbose")
            .short('v')
            .long("verbose")
            .help("Enable verbose output")
            .action(clap::ArgAction::SetTrue), // This makes it a flag
    )
}

pub fn cli() -> Command {
    Command::new("user")
        .about("User tools")
        .version("1.0")
        .subcommand_required(true) // Ensures one of the subcommands must be used
        .arg_required_else_help(true) // Shows help if no subcommand is given
        .subcommand(
            common_args(Command::new("recipient").about("Convert address")).arg(
                Arg::new("ADDRESS")
                    .help("The address to be converted")

                    .required(true)
                    .index(1), // Positional argument
            ),
        )
        .subcommand(common_args(Command::new("foo").about("Foo logic placeholder")).arg(
            Arg::new("bar")
                .long("bar")
                .help("A bar value for foo")
                .required(false),
        ))
}