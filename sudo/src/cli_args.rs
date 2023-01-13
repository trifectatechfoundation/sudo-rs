use std::path::PathBuf;

use clap:: {
    Parser,
};


// doesn't work when derive and builder patterns are combined.
#[command(name = "sudo-rs")]
#[command(about = "sudo - execute a command as another user", long_about = None)]
#[command(version)] // Read from `Cargo.toml`

#[derive(Debug, Parser)]
pub struct Cli {
    /// This comment will be printed in help!
    // when `Option<>` isn't used, the arg is required
    pattern: Option<String>,
    /// The path to the file to read
    #[arg(short, help = "This overrides the comment!")]
    pub path: Option<PathBuf>,
    /// A flag
    // `action` sets ArgAction::SetTrue, means we don't need to set a value when we have a bool.
    #[clap(long, short, action)] 
    pub flag: bool,
    /// Give me an easy exclusion please
    #[clap(long, short, action, conflicts_with("flag"))]
    bla: bool,
    #[clap(long, short, action)] 
    pub list: bool,
    #[clap(long, short, action)] 
    pub directory: bool,
}

// It doesn't matter in which order args are given.
