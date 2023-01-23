mod cli_args;
use clap::Parser;
use cli_args::Cli;


#[derive(Debug)]
struct CustomError(String);

fn main() -> Result<(), CustomError> {
    let args = Cli::parse();
    println!("args: {:?}", args);
    Ok(())
}
