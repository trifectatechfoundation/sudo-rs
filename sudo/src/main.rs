use clap::Parser;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    /// The pattern to look for
    pattern: String,
    /// The path to the file to read
    path: std::path::PathBuf,
}

#[derive(Debug)]
struct CustomError(String);

// fn main() {
fn main() -> Result<(), CustomError> {

    let args = Cli::parse();
    let content = std::fs::read_to_string(&args.path)
    .map_err(|err| CustomError(format!("Error reading `{}`: {}", &args.path.display(), err)))?;
    println!("file content: {}", content);
    Ok(())
}
