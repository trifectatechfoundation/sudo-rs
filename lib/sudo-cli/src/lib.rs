#![forbid(unsafe_code)]

use std::path::PathBuf;

const HELP_MSG: &str = "sudo - execute a command as another user

usage: sudo -h | -K | -k | -V
usage: sudo -v [-ABkNnS] [-g group] [-h host] [-p prompt] [-u user]
usage: sudo -l [-ABkNnS] [-g group] [-h host] [-p prompt] [-U user] [-u user] [command]
usage: sudo [-ABbEHkNnPS] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R
            directory] [-T timeout] [-u user] [VAR=value] [-i|-s] [<command>]
usage: sudo -e [-ABkNnS] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R
            directory] [-T timeout] [-u user] file ...

Options:
  -A, --askpass                 use a helper program for password prompting
  -b, --background              run command in the background
  -B, --bell                    ring bell when prompting
  -C, --close-from=num          close all file descriptors >= num
  -D, --chdir=directory         change the working directory before running command
  -E, --preserve-env            preserve user environment when running command
      --preserve-env=list       preserve specific environment variables
  -e, --edit                    edit files instead of running a command
  -g, --group=group             run command as the specified group name or ID
  -H, --set-home                set HOME variable to target user's home dir
  -h, --help                    display help message and exit
  -h, --host=host               run command on host (if supported by plugin)
  -i, --login                   run login shell as the target user; a command may also be
                                specified
  -K, --remove-timestamp        remove timestamp file completely
  -k, --reset-timestamp         invalidate timestamp file
  -l, --list                    list user's privileges or check a specific command; use twice
                                for longer format
  -n, --non-interactive         non-interactive mode, no prompts are used
  -P, --preserve-groups         preserve group vector instead of setting to target's
  -p, --prompt=prompt           use the specified password prompt
  -R, --chroot=directory        change the root directory before running command
  -S, --stdin                   read password from standard input
  -s, --shell                   run shell as the target user; a command may also be specified
  -T, --command-timeout=timeout terminate command after the specified time limit
  -U, --other-user=user         in list mode, display privileges for user
  -u, --user=user               run command (or edit file) as specified user name or ID
  -V, --version                 display version information and exit
  -v, --validate                update user's timestamp without running a command
  --                            stop processing command line arguments";

const USAGE_MSG: &str = "usage: sudo -h | -K | -k | -V
usage: sudo -v [-AknS] [-g group] [-h host] [-p prompt] [-u user]
usage: sudo -l [-AknS] [-g group] [-h host] [-p prompt] [-U user] [-u user] [command]
usage: sudo [-AbEHknPS] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R directory] [-T timeout] [-u user] [VAR=value] [-i|-s] [<command>]
usage: sudo -e [-AknS] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R directory] [-T timeout] [-u user] file ...";

#[derive(Debug, Default, PartialEq)]
pub struct SudoOptions {
    pub askpass: bool,
    pub background: bool,
    pub bell: bool,
    pub num: Option<i16>,
    pub directory: Option<PathBuf>,
    // This is what OGsudo calls `--preserve-env=list`
    pub preserve_env_list: Vec<String>,
    // This is what OGsudo calls `-E, --preserve-env`
    pub preserve_env: bool,
    pub edit: bool,
    pub group: Option<String>,
    pub set_home: bool,
    pub help: bool,
    pub login: bool,
    pub remove_timestamp: bool,
    pub reset_timestamp: bool,
    pub list: bool,
    pub non_interactive: bool,
    pub preserve_groups: bool,
    pub prompt: Option<String>,
    pub chroot: Option<PathBuf>,
    pub stdin: bool,
    pub shell: bool,
    pub command_timeout: Option<u64>,
    pub other_user: Option<String>,
    pub user: Option<String>,
    pub version: bool,
    pub validate: bool,
    pub host: Option<String>,
    // Arguments passed straight through, either seperated by -- or just trailing.
    pub external_args: Vec<String>,
    pub env_var_list: Vec<(String, String)>,
}

enum SudoArg {
    Flag(String),
    Argument(String, String),
    Environment(String, String),
    Rest(Vec<String>),
}

impl SudoOptions {
    const TAKES_ARGUMENT_SHORT: &[char] = &['C', 'D', 'E', 'g', 'h', 'p', 'R', 'T', 'U', 'u'];
    const TAKES_ARGUMENT: &[&'static str] = &[
        "close-from",
        "chdir",
        "preserve-env",
        "group",
        "host",
        "chroot",
        "command-timeout",
        "other-user",
        "user",
    ];

    /// argument assignments and shorthand options preprocessing
    fn normalize_arguments<I>(iter: I) -> Result<Vec<SudoArg>, &'static str>
    where
        I: IntoIterator<Item = String>,
    {
        // the first argument is the sudo command - so we can sklip it
        let mut arg_iter = iter.into_iter().skip(1).peekable();
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
                        processed.push(SudoArg::Argument(key.to_string(), value.to_string()));
                    } else if Self::TAKES_ARGUMENT.contains(&&long_arg[2..]) {
                        if let Some(next) = arg_iter.next() {
                            processed.push(SudoArg::Argument(arg, next));
                        } else if long_arg == "--preserve-env" {
                            processed.push(SudoArg::Flag(arg));
                        } else {
                            Err("invalid argument provided ")?;
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

                            if rest.starts_with('=') {
                                Err("invalid option '='")?;
                            }

                            if !rest.is_empty() {
                                processed.push(SudoArg::Argument(flag, rest));
                            } else if let Some(next) = arg_iter.next() {
                                processed.push(SudoArg::Argument(flag, next));
                            } else if char == 'E' || char == 'h' {
                                // preserve env and the short version of --help have optional arguments
                                processed.push(SudoArg::Flag(flag));
                            } else {
                                Err("invalid argument provided")?;
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
    pub fn parse() -> SudoOptions {
        match Self::try_parse_from(std::env::args()) {
            Ok(options) => {
                if options.help {
                    eprintln!("{HELP_MSG}");
                    std::process::exit(1);
                }

                options
            }
            Err(e) => {
                eprintln!("{e}\n{USAGE_MSG}");
                std::process::exit(1);
            }
        }
    }

    /// parse an iterator over command line arguments
    pub fn try_parse_from<I, T>(iter: I) -> Result<Self, &'static str>
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
                    "-A" | "--askpass" => {
                        options.askpass = true;
                    }
                    "-b" | "--background" => {
                        options.background = true;
                    }
                    "-B" | "--bell" => {
                        options.bell = true;
                    }
                    "-C" | "--close-from" => {
                        // pass
                    }
                    "-e" | "--edit" => {
                        options.edit = true;
                    }
                    "-E" | "--preserve-env" => {
                        options.preserve_env = true;
                    }
                    "-H" | "--set-home" => {
                        options.set_home = true;
                    }
                    "-h" | "--help" => {
                        options.help = true;
                    }
                    "-i" | "--login" => {
                        options.login = true;
                    }
                    "-K" | "--remove-timestamp" => {
                        if options.reset_timestamp {
                            Err("conflicting arguments")?;
                        }
                        options.remove_timestamp = true;
                    }
                    "-k" | "--reset-timestamp" => {
                        if options.remove_timestamp {
                            Err("conflicting arguments")?;
                        }
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
                        options.preserve_env_list = value.split(',').map(str::to_string).collect()
                    }
                    "-g" | "--group" => {
                        options.group = Some(value);
                    }
                    "-h" | "--host=host" => {
                        options.host = Some(value);
                    }
                    "-p" | "--prompt" => {
                        options.prompt = Some(value);
                    }
                    "-R" | "--chroot" => {
                        options.chroot = Some(PathBuf::from(value));
                    }
                    "-T" | "--command-timeout" => {
                        options.command_timeout = Some(
                            value
                                .parse()
                                .map_err(|_| "invalid command timeout provided")?,
                        )
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

        Ok(options)
    }
}
