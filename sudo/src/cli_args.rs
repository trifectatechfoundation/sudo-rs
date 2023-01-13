use std::path::PathBuf;

use clap:: {
    Parser,
    Arg,
    Command,
    Args,
    error::Error,
    ArgMatches,
    FromArgMatches,
    ArgAction
};


// // doesn't work when derive and builder patterns are combined.
// #[command(name = "sudo-rs")]
// #[command(about = "sudo - execute a command as another user", long_about = None)]
// #[command(version)] // Read from `Cargo.toml`

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
    /// builder with derive api
    #[command(flatten)]
    more_args: CliArgs,
}

#[derive(Debug)]
struct CliArgs {
    either_this: bool,
    or_that: bool,
}

impl FromArgMatches for CliArgs {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, Error> {
        let mut matches = matches.clone();
        Self::from_arg_matches_mut(&mut matches)
    }
    fn from_arg_matches_mut(matches: &mut ArgMatches) -> Result<Self, Error> {
        Ok(Self {
            either_this: matches.get_flag("either_this"),
            or_that: matches.get_flag("or_that"),
        })
    }
    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), Error> {
        let mut matches = matches.clone();
        self.update_from_arg_matches_mut(&mut matches)
    }

}

impl Args for CliArgs {
    fn augment_args(cmd: Command) -> Command {
        cmd.arg(
            Arg::new("either_this")
                .short('e')
                .long("either_this")
                .action(ArgAction::SetTrue)
                .conflicts_with("or_that"), // this is what we need, can conflict with any arg, e.g. "flag"
        )
        .arg(
            Arg::new("or_that")
                .short('o')
                .long("or_that")
                .action(ArgAction::SetTrue),
        )
    }
    // what is this needed for?
    fn augment_args_for_update(cmd: Command) -> Command {
        cmd.arg(
            Arg::new("either_this")
                .short('e')
                .long("either_this")
                .action(ArgAction::SetTrue)
                .conflicts_with("or_that"), // this is what we need,
        )
        .arg(
            Arg::new("or_that")
                .short('o')
                .long("or_that")
                .action(ArgAction::SetTrue),
        )
    }
}


// It doesn't matter in which order args are given.
