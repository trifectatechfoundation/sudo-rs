mod cli_args;
use crate::cli_args::{Cli, SudoOptions};

#[derive(Debug)]
struct CustomError(String);

fn main() -> Result<(), CustomError> {
    let args = SudoOptions::parse();
    println!("args: {:?}", args);
    Ok(())
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}
