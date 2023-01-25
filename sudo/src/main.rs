mod cli_args;
use clap::{Parser, CommandFactory};
use cli_args::Cli;

use crate::cli_args::SudoOptions;


#[derive(Debug)]
struct CustomError(String);

fn main() -> Result<(), CustomError> {
    let mut args = Cli::parse();
    let mut bla = SudoOptions::from(args.clone());
    args.preserve_env.clear();
    args.preserve_env.append(& mut bla.preserve_env_list);
    args.short_preserve_env.clone_from(&bla.preserve_env);

    println!("args: {:?}", args);
    Ok(())
}

#[test]
fn verify_cli() {
    Cli::command().debug_assert()
}