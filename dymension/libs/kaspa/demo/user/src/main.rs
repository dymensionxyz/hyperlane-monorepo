use clap::ArgMatches;
mod x;
use x::args::cli;

fn run(matches: ArgMatches) {
    let is_verbose = matches.get_flag("verbose");
    if is_verbose {
        println!("Verbose mode is enabled");
    }

    match matches.subcommand() {
        Some(("recipient", sub_matches)) => {
            let address = sub_matches
                .get_one::<String>("ADDRESS")
                .expect("required argument");
            let converted = x::addr::hl_recipient(address);
            println!("The recipient address is: {}", converted);
            if sub_matches.get_flag("verbose") {
                println!("Verbose output for recipient.");
            }
        }
        Some(("foo", sub_matches)) => {
            println!("The 'foo' subcommand was used.");
            if sub_matches.get_flag("verbose") {
                println!("Verbose output for foo.");
            }
            if let Some(bar_value) = sub_matches.get_one::<String>("bar") {
                println!("The value of --bar is: {}", bar_value);
            }
        }
        _ => {
            unreachable!();
        }
    }
}

fn main() {
    let matches = cli().get_matches();
    run(matches);
}
