mod cli_args;

#[derive(Debug)]
struct CustomError(String);

fn main() -> Result<(), CustomError> {
    let captured = cli_args::SudoOptions::parse();
    println!("captured: {captured:?}");
    Ok(())
}
