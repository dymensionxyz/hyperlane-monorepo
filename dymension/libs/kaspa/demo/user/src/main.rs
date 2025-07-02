use clap::ArgMatches;
use std::process;
mod x;
use x::args::cli;

// The run function dispatches logic based on the matched subcommand
fn run(matches: ArgMatches) {
    // You can check for global arguments like 'verbose' on the top-level matches
    let is_verbose = matches.get_flag("verbose");
    if is_verbose {
        println!("Verbose mode is enabled");
    }

    // Match on the subcommand name and handle its specific logic
    match matches.subcommand() {
        Some(("recipient", sub_matches)) => {
            // Safely unwrap the required "ADDRESS" argument
            let address = sub_matches
                .get_one::<String>("ADDRESS")
                .expect("required argument");
            println!("The recipient address is: {}", address);
            // You can also check for arguments common to this subcommand
            if sub_matches.get_flag("verbose") {
                println!("Verbose output for recipient.");
            }
            // TODO: Add your address conversion logic here
        }
        Some(("foo", sub_matches)) => {
            println!("The 'foo' subcommand was used.");
            if sub_matches.get_flag("verbose") {
                println!("Verbose output for foo.");
            }
            // Handle the optional '--bar' argument
            if let Some(bar_value) = sub_matches.get_one::<String>("bar") {
                println!("The value of --bar is: {}", bar_value);
            }
            // TODO: Add your "foo" logic here
        }
        _ => {
            // Since `subcommand_required` is true, this branch is unreachable.
            // Clap will have already exited with an error or help message.
            unreachable!();
        }
    }
}

fn main() {
    let matches = cli().get_matches();
    run(matches);
}
