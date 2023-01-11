mod cli_args;
use clap::{CommandFactory, Parser};
use cli_args::Cli;

use crate::cli_args::SudoOptions;

#[derive(Debug)]
struct CustomError(String);

fn main() -> Result<(), CustomError> {
    let args = Cli::parse();
    let captured = SudoOptions::from(args.clone());
    println!("captured: {:?}", captured);
    Ok(())
}

#[test]
fn verify_cli() {
    Cli::command().debug_assert()
}
