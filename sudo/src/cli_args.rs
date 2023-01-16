use std::{path::PathBuf, error::Error};

use clap:: {
    Parser,
};

// warning: derive helper attribute is used before it is introduced
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
    #[arg(long, short, action)] 
    pub flag: bool,
    /// change the working directory before running command
    #[arg(long = "chdir=directory", short = 'D', action, conflicts_with("list"))] // exclusion
    pub directory: bool,
    #[arg(long, short, action)] 
    pub list: bool,
    /// Hand-written parser for key-value pairs, e.g. envs. But needs flag.
    // can we strip hyphens?? -
    #[arg(long = "[VAR=value]", short = 'x', value_parser = parse_key_val::<String, String>)]
    defines: Vec<(String, String)>,
}

// It doesn't matter in which order args are given.


/// Parse a key-value pair
fn parse_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}
