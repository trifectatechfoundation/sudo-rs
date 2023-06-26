#![forbid(unsafe_code)]

use std::path::PathBuf;

pub mod help;

#[cfg(test)]
mod tests;

#[derive(Debug, Default, PartialEq, Clone)]
pub enum SudoAction {
    #[default]
    Help,
    Version,
    Validate,
    RemoveTimestamp,
    ResetTimestamp,
    Run(Vec<String>),
    List(Vec<String>),
    Edit(Vec<PathBuf>),
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct SudoOptions {
    pub background: bool,
    pub chroot: Option<PathBuf>,
    pub directory: Option<PathBuf>,
    pub group: Option<String>,
    pub host: Option<String>,
    pub login: bool,
    pub non_interactive: bool,
    pub other_user: Option<String>,
    pub preserve_env: Vec<String>,
    pub preserve_groups: bool,
    pub shell: bool,
    pub stdin: bool,
    pub user: Option<String>,
    // additional environment
    pub env_var_list: Vec<(String, String)>,
    // resulting action enum
    pub action: SudoAction,
    // actions
    edit: bool,
    help: bool,
    list: bool,
    remove_timestamp: bool,
    pub reset_timestamp: bool,
    validate: bool,
    version: bool,
    // arguments passed straight through, either seperated by -- or just trailing.
    external_args: Vec<String>,
}

enum SudoArg {
    Flag(String),
    Argument(String, String),
    Environment(String, String),
    Rest(Vec<String>),
}

impl SudoOptions {
    const TAKES_ARGUMENT_SHORT: &[char] = &['D', 'E', 'g', 'h', 'R', 'U', 'u'];
    const TAKES_ARGUMENT: &[&'static str] = &[
        "chdir",
        "preserve-env",
        "group",
        "host",
        "chroot",
        "other-user",
        "user",
    ];

    /// argument assignments and shorthand options preprocessing
    fn normalize_arguments<I>(iter: I) -> Result<Vec<SudoArg>, String>
    where
        I: IntoIterator<Item = String>,
    {
        // the first argument is the sudo command - so we can skip it
        let mut arg_iter = iter.into_iter().skip(1);
        let mut processed: Vec<SudoArg> = vec![];

        while let Some(arg) = arg_iter.next() {
            match arg.as_str() {
                "--" => {
                    processed.push(SudoArg::Rest(arg_iter.collect()));
                    break;
                }
                long_arg if long_arg.starts_with("--") => {
                    if long_arg.contains('=') {
                        // convert assignment to normal tokens
                        let (key, value) = long_arg.split_once('=').unwrap();
                        // only accept arguments when one is expected
                        if !Self::TAKES_ARGUMENT.contains(&&key[2..]) {
                            Err(format!("'{}' does not take any arguments", key))?;
                        }
                        processed.push(SudoArg::Argument(key.to_string(), value.to_string()));
                    } else if Self::TAKES_ARGUMENT.contains(&&long_arg[2..]) {
                        if let Some(next) = arg_iter.next() {
                            processed.push(SudoArg::Argument(arg, next));
                        } else {
                            Err(format!("'{}' expects an argument", &long_arg))?;
                        }
                    } else {
                        processed.push(SudoArg::Flag(arg));
                    }
                }
                short_arg if short_arg.starts_with('-') => {
                    // split combined shorthand options
                    for (n, char) in short_arg.trim_start_matches('-').chars().enumerate() {
                        let flag = format!("-{char}");
                        // convert option argument to seperate segment
                        if Self::TAKES_ARGUMENT_SHORT.contains(&char) {
                            let rest = short_arg[(n + 2)..].trim().to_string();
                            // assignment syntax is not accepted for shorthand arguments
                            if rest.starts_with('=') {
                                Err("invalid option '='")?;
                            }
                            if !rest.is_empty() {
                                processed.push(SudoArg::Argument(flag, rest));
                            } else if let Some(next) = arg_iter.next() {
                                processed.push(SudoArg::Argument(flag, next));
                            } else if char == 'h' {
                                // short version of --help has no arguments
                                processed.push(SudoArg::Flag(flag));
                            } else {
                                Err(format!("'-{}' expects an argument", char))?;
                            }
                            break;
                        } else {
                            processed.push(SudoArg::Flag(flag));
                        }
                    }
                }
                env_var if SudoOptions::try_to_env_var(env_var).is_some() => {
                    let (key, value) = SudoOptions::try_to_env_var(env_var).unwrap();
                    processed.push(SudoArg::Environment(key, value));
                }
                _argument => {
                    let mut rest = vec![arg];
                    rest.extend(arg_iter);
                    processed.push(SudoArg::Rest(rest));
                    break;
                }
            }
        }

        Ok(processed)
    }

    /// try to parse and environment variable assignment
    fn try_to_env_var(arg: &str) -> Option<(String, String)> {
        if let Some((name, value)) = arg.split_once('=').and_then(|(name, value)| {
            name.chars()
                .all(|c| c.is_alphanumeric() || c == '_')
                .then_some((name, value))
        }) {
            Some((name.to_owned(), value.to_owned()))
        } else {
            None
        }
    }

    /// parse command line arguments from the environment and handle errors
    pub fn from_env() -> Result<SudoOptions, String> {
        Self::try_parse_from(std::env::args())
    }

    /// from the arguments resolve which action should be performed
    fn resolve_action(&mut self) {
        if self.help {
            self.action = SudoAction::Help;
        } else if self.version {
            self.action = SudoAction::Version;
        } else if self.remove_timestamp {
            self.action = SudoAction::RemoveTimestamp;
        } else if self.reset_timestamp && self.external_args.is_empty() {
            self.action = SudoAction::ResetTimestamp;
        } else if self.validate {
            self.action = SudoAction::Validate;
        } else if self.list {
            self.action = SudoAction::List(std::mem::take(self.external_args.as_mut()));
        } else if self.edit {
            let args: Vec<String> = std::mem::take(self.external_args.as_mut());
            let args = args.into_iter().map(PathBuf::from).collect();
            self.action = SudoAction::Edit(args);
        } else {
            self.action = SudoAction::Run(std::mem::take(self.external_args.as_mut()));
        }
    }

    /// verify that the passed arguments are valid given the action and there are no conflicts
    fn validate(&self) -> Result<(), String> {
        // conflicting arguments
        if self.remove_timestamp && self.reset_timestamp {
            Err("conflicting arguments '--remove-timestamp' and '--reset-timestamp'")?;
        }

        // check arguments for validate action
        if matches!(self.action, SudoAction::Validate)
            && (self.background
                || self.preserve_groups
                || self.login
                || self.shell
                || !self.preserve_env.is_empty()
                || self.other_user.is_some()
                || self.directory.is_some()
                || self.chroot.is_some())
        {
            Err("invalid argument found for '--validate'")?;
        }

        // check arguments for list action
        if matches!(self.action, SudoAction::List(_))
            && (self.background
                || self.preserve_groups
                || self.login
                || self.shell
                || !self.preserve_env.is_empty()
                || self.directory.is_some()
                || self.chroot.is_some())
        {
            Err("invalid argument found for '--list'")?;
        }

        // check arguments for edit action
        if matches!(self.action, SudoAction::Edit(_))
            && (self.background
                || self.preserve_groups
                || self.login
                || self.shell
                || self.other_user.is_some()
                || !self.preserve_env.is_empty())
        {
            Err("invalid argument found for '--edit'")?;
        }

        Ok(())
    }

    /// parse an iterator over command line arguments
    pub fn try_parse_from<I, T>(iter: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = T>,
        T: Into<String> + Clone,
    {
        let mut options: SudoOptions = SudoOptions::default();
        let arg_iter = Self::normalize_arguments(iter.into_iter().map(Into::into))?
            .into_iter()
            .peekable();

        for arg in arg_iter {
            match arg {
                SudoArg::Flag(flag) => match flag.as_str() {
                    "-b" | "--background" => {
                        options.background = true;
                    }
                    "-e" | "--edit" => {
                        options.edit = true;
                    }
                    "-H" | "--set-home" => {
                        // this option is ignored, since it is the default for sudo-rs; but accept
                        // it for backwards compatibility reasons
                    }
                    "-h" | "--help" => {
                        options.help = true;
                    }
                    "-i" | "--login" => {
                        options.login = true;
                    }
                    "-K" | "--remove-timestamp" => {
                        options.remove_timestamp = true;
                    }
                    "-k" | "--reset-timestamp" => {
                        options.reset_timestamp = true;
                    }
                    "-l" | "--list" => {
                        options.list = true;
                    }
                    "-n" | "--non-interactive" => {
                        options.non_interactive = true;
                    }
                    "-P" | "--preserve-groups" => {
                        options.preserve_groups = true;
                    }
                    "-S" | "--stdin" => {
                        options.stdin = true;
                    }
                    "-s" | "--shell" => {
                        options.shell = true;
                    }
                    "-V" | "--version" => {
                        options.version = true;
                    }
                    "-v" | "--validate" => {
                        options.validate = true;
                    }
                    _option => {
                        Err("invalid option provided")?;
                    }
                },
                SudoArg::Argument(option, value) => match option.as_str() {
                    "-D" | "--chdir" => {
                        options.directory = Some(PathBuf::from(value));
                    }
                    "-E" | "--preserve-env" => {
                        options.preserve_env = value.split(',').map(str::to_string).collect()
                    }
                    "-g" | "--group" => {
                        options.group = Some(value);
                    }
                    "-h" | "--host" => {
                        options.host = Some(value);
                    }
                    "-R" | "--chroot" => {
                        options.chroot = Some(PathBuf::from(value));
                    }
                    "-U" | "--other-user" => {
                        options.other_user = Some(value);
                    }
                    "-u" | "--user" => {
                        options.user = Some(value);
                    }
                    _option => {
                        Err("invalid option provided")?;
                    }
                },
                SudoArg::Environment(key, value) => {
                    options.env_var_list.push((key, value));
                }
                SudoArg::Rest(rest) => {
                    options.external_args = rest;
                }
            }
        }

        options.resolve_action();
        options.validate()?;

        Ok(options)
    }

    #[cfg(test)]
    pub fn args(self) -> Vec<String> {
        match self.action {
            SudoAction::Run(args) => args,
            SudoAction::List(args) => args,
            _ => vec![],
        }
    }
}
