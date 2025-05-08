use std::path::PathBuf;

pub(crate) const USAGE_MSG: &str = "\
usage: sudoedit -h | -V
usage: sudoedit [-ABkNnS] [-C num] [-D directory]
                [-g group] [-h host] [-p prompt] [-R directory] [-T timeout]
                [-u user] file ...";

const DESCRIPTOR: &str = "sudoedit - edit files as another user";

const HELP_MSG: &str = "Options:
  -A, --askpass                 use a helper program for password prompting
  -B, --bell                    ring bell when prompting
  -C, --close-from=num          close all file descriptors >= num
  -D, --chdir=directory         change the working directory before running
                                command
  -g, --group=group             run command as the specified group name or ID
  -h, --help                    display help message and exit
  -h, --host=host               run command on host (if supported by plugin)
  -k, --reset-timestamp         invalidate timestamp file
  -n, --non-interactive         non-interactive mode, no prompts are used
  -p, --prompt=prompt           use the specified password prompt
  -R, --chroot=directory        change the root directory before running command
  -S, --stdin                   read password from standard input
  -T, --command-timeout=timeout terminate command after the specified time limit
  -u, --user=user               run command (or edit file) as specified user
                                name or ID
  -V, --version                 display version information and exit
  --                            stop processing command line arguments";

pub(crate) fn long_help_message() -> String {
    format!("{USAGE_MSG}\n\n{DESCRIPTOR}\n\n{HELP_MSG}")
}

#[derive(Debug, PartialEq)]
pub(crate) struct SudoEditOptions {
    pub(crate) file: Option<String>,

    pub(crate) askpass: bool,

    pub(crate) bell: bool,
    pub(crate) close_from: Option<u32>,
    pub(crate) chdir: Option<PathBuf>,
    pub(crate) group: Option<String>,
    pub(crate) host: Option<String>,

    pub(crate) reset_timestamp: bool,
    pub(crate) non_interactive: bool,
    pub(crate) prompt: Option<String>,
    pub(crate) chroot: Option<PathBuf>,
    pub(crate) stdin: bool,
    pub(crate) command_timeout: Option<String>,
    pub(crate) user: Option<String>,

    pub(crate) action: SudoEditAction,
}

impl Default for SudoEditOptions {
    fn default() -> Self {
        Self {
            file: None,
            askpass: false,
            bell: false,
            close_from: None,
            chdir: None,
            group: None,
            host: None,
            reset_timestamp: false,
            non_interactive: false,
            prompt: None,
            chroot: None,
            stdin: false,
            command_timeout: None,
            user: None,

            action: SudoEditAction::Run,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum SudoEditAction {
    Help,
    Version,
    Run,
}

type OptionSetter = fn(&mut SudoEditOptions, Option<String>) -> Result<(), String>;

struct SudoEditOption {
    short: char,
    long: &'static str,
    takes_argument: bool,
    set: OptionSetter,
}

impl SudoEditOptions {
    const VISUDO_OPTIONS: &'static [SudoEditOption] = &[
        SudoEditOption {
            short: 'A',
            long: "askpass",
            takes_argument: false,
            set: |options, _| {
                options.askpass = true;
                Ok(())
            },
        },
        SudoEditOption {
            short: 'B',
            long: "bell",
            takes_argument: false,
            set: |options, _| {
                options.bell = true;
                Ok(())
            },
        },
        SudoEditOption {
            short: 'C',
            long: "close-from",
            takes_argument: true,
            set: |options, arg| {
                let num = arg.ok_or("option requires an argument -- 'C/close-from'")?;
                let num = num.parse().map_err(|err| "option  for close-from")?;
                options.close_from = Some(num);
                Ok(())
            },
        },
        SudoEditOption {
            short: 'D',
            long: "chdir",
            takes_argument: true,
            set: |options, arg| {
                let path = arg.ok_or("option requires an argument -- 'D/chdir'")?;
                options.chdir = Some(path.into());
                Ok(())
            },
        },
        SudoEditOption {
            short: 'g',
            long: "group",
            takes_argument: true,
            set: |options, arg| {
                options.group = Some(arg.ok_or("option requires an argument -- 'g/group'")?);
                Ok(())
            },
        },
        // SudoEditOption {
        //     short: 'h',
        //     long: "host",
        //     takes_argument: true,
        //     set: |options, arg| {
        //         options.host = Some(arg.ok_or("option requires an argument -- 'h/host")?);
        //         Ok(())
        //     },
        // },

        // TODO: from sudo.ws, help and host have the same short letter, which
        // is not possible with the current parsing
        SudoEditOption {
            short: 'h',
            long: "help",
            takes_argument: false,
            set: |options, _| {
                options.action = SudoEditAction::Help;
                Ok(())
            },
        },
        SudoEditOption {
            short: 'k',
            long: "reset-timestamp",
            takes_argument: false,
            set: |options, _| {
                options.reset_timestamp = true;
                Ok(())
            },
        },
        SudoEditOption {
            short: 'n',
            long: "non-interactive",
            takes_argument: false,
            set: |options, _| {
                options.non_interactive = true;
                Ok(())
            },
        },
        SudoEditOption {
            short: 'p',
            long: "prompt",
            takes_argument: true,
            set: |options, arg| {
                options.prompt = Some(arg.ok_or("option requires an argument -- 'p/prompt'")?);
                Ok(())
            },
        },
        SudoEditOption {
            short: 'R',
            long: "chroot",
            takes_argument: true,
            set: |options, arg| {
                let path = arg.ok_or("option requires an argument -- 'R/chroot'")?;
                options.chroot = Some(path.into());
                Ok(())
            },
        },
        SudoEditOption {
            short: 'S',
            long: "stdin",
            takes_argument: false,
            set: |options, _| {
                options.stdin = true;
                Ok(())
            },
        },
        SudoEditOption {
            short: 'T',
            long: "command-timeout",
            takes_argument: true,
            set: |options, arg| {
                options.command_timeout =
                    Some(arg.ok_or("option requires an argument -- 'T/command-timeout'")?);
                Ok(())
            },
        },
        SudoEditOption {
            short: 'u',
            long: "user",
            takes_argument: true,
            set: |options, arg| {
                options.user = Some(arg.ok_or("option requires an argument -- 'u/user'")?);
                Ok(())
            },
        },
        SudoEditOption {
            short: 'V',
            long: "version",
            takes_argument: false,
            set: |options, _| {
                options.action = SudoEditAction::Version;
                Ok(())
            },
        },
    ];

    pub(crate) fn from_env() -> Result<SudoEditOptions, String> {
        let args = std::env::args().collect();

        Self::parse_arguments(args)
    }

    // TODO: code taken from src/visudo/cli.rs, dedup? change arg parsing method?

    /// parse su arguments into VisudoOptions struct
    pub(crate) fn parse_arguments(arguments: Vec<String>) -> Result<SudoEditOptions, String> {
        let mut options: SudoEditOptions = SudoEditOptions::default();
        let mut arg_iter = arguments.into_iter().skip(1);

        while let Some(arg) = arg_iter.next() {
            // if the argument starts with -- it must be a full length option name
            if let Some(arg) = arg.strip_prefix("--") {
                // parse assignments like '--file=/etc/sudoers'
                if let Some((key, value)) = arg.split_once('=') {
                    // lookup the option by name
                    if let Some(option) = Self::VISUDO_OPTIONS.iter().find(|o| o.long == key) {
                        // the value is already present, when the option does not take any arguments this results in an error
                        if option.takes_argument {
                            (option.set)(&mut options, Some(value.to_string()))?;
                        } else {
                            Err(format!("'--{}' does not take any arguments", option.long))?;
                        }
                    } else {
                        Err(format!("unrecognized option '{}'", arg))?;
                    }
                // lookup the option
                } else if let Some(option) = Self::VISUDO_OPTIONS.iter().find(|o| o.long == arg) {
                    // try to parse an argument when the option needs an argument
                    if option.takes_argument {
                        let next_arg = arg_iter.next();
                        (option.set)(&mut options, next_arg)?;
                    } else {
                        (option.set)(&mut options, None)?;
                    }
                } else {
                    Err(format!("unrecognized option '{}'", arg))?;
                }
            } else if let Some(arg) = arg.strip_prefix('-') {
                // flags can be grouped, so we loop over the characters
                for (n, char) in arg.chars().enumerate() {
                    // lookup the option
                    if let Some(option) = Self::VISUDO_OPTIONS.iter().find(|o| o.short == char) {
                        // try to parse an argument when one is necessary, either the rest of the current flag group or the next argument
                        if option.takes_argument {
                            // skip the single char
                            let rest = arg[(n + 1)..].trim().to_string();
                            let next_arg = if rest.is_empty() {
                                arg_iter.next()
                            } else {
                                Some(rest)
                            };
                            (option.set)(&mut options, next_arg)?;
                            // stop looping over flags if the current flag takes an argument
                            break;
                        } else {
                            // parse flag without argument
                            (option.set)(&mut options, None)?;
                        }
                    } else {
                        Err(format!("unrecognized option '{}'", char))?;
                    }
                }
            } else {
                // If the arg doesn't start with a `-` it must be a file argument. However `-f`
                // must take precedence
                if options.file.is_none() {
                    options.file = Some(arg);
                }
            }
        }

        Ok(options)
    }
}
