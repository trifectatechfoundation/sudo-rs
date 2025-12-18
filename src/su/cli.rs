use std::{mem, path::PathBuf};

use crate::common::SudoString;

use super::DEFAULT_USER;

#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum SuAction {
    Help(SuHelpOptions),
    Version(SuVersionOptions),
    Run(SuRunOptions),
}

impl SuAction {
    pub fn from_env() -> Result<Self, String> {
        SuOptions::parse_arguments(std::env::args())?.validate()
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct SuHelpOptions {}

impl TryFrom<SuOptions> for SuHelpOptions {
    type Error = String;

    fn try_from(mut opts: SuOptions) -> Result<Self, Self::Error> {
        let help = mem::take(&mut opts.help);
        debug_assert!(help);
        reject_all("--help", opts)?;
        Ok(Self {})
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct SuVersionOptions {}

impl TryFrom<SuOptions> for SuVersionOptions {
    type Error = String;

    fn try_from(mut opts: SuOptions) -> Result<Self, Self::Error> {
        let version = mem::take(&mut opts.version);
        debug_assert!(version);
        reject_all("--version", opts)?;
        Ok(Self {})
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct SuRunOptions {
    // -c
    pub command: Option<String>,
    // -g
    pub group: Vec<SudoString>,
    // -l
    pub login: bool,
    // -p
    pub preserve_environment: bool,
    // -s
    pub shell: Option<PathBuf>,
    // -G
    pub supp_group: Vec<SudoString>,
    // -w
    pub whitelist_environment: Vec<String>,

    pub user: SudoString,
    pub arguments: Vec<String>,
}

#[cfg(test)]
impl Default for SuRunOptions {
    fn default() -> Self {
        Self {
            command: None,
            group: vec![],
            login: false,
            preserve_environment: false,
            shell: None,
            supp_group: vec![],
            whitelist_environment: vec![],
            user: DEFAULT_USER.into(),
            arguments: vec![],
        }
    }
}

impl TryFrom<SuOptions> for SuRunOptions {
    type Error = String;

    fn try_from(mut opts: SuOptions) -> Result<Self, Self::Error> {
        let command = mem::take(&mut opts.command);
        let group = mem::take(&mut opts.group);
        let login = mem::take(&mut opts.login);
        let preserve_environment = mem::take(&mut opts.preserve_environment);
        // always `true`; cannot be disabled via the CLI
        let _pty = mem::take(&mut opts.pty);
        let shell = mem::take(&mut opts.shell);
        let supp_group = mem::take(&mut opts.supp_group);
        let whitelist_environment = mem::take(&mut opts.whitelist_environment);
        let mut positional_args = mem::take(&mut opts.positional_args);

        reject_all("run mode", opts)?;

        let user = if positional_args.is_empty() {
            DEFAULT_USER.to_string()
        } else {
            positional_args.remove(0)
        };
        let arguments = positional_args;

        Ok(Self {
            command,
            group,
            login,
            preserve_environment,
            shell,
            supp_group,
            whitelist_environment,
            user: SudoString::try_from(user).map_err(|err| err.to_string())?,
            arguments,
        })
    }
}

fn reject_all(context: &str, opts: SuOptions) -> Result<(), String> {
    macro_rules! ensure_options_absent {
        ($($opt:ident,)*) => {
            let SuOptions {
                $($opt),*
            } = opts;

            $(if !$opt.is_absent() {
                let name = concat!("--", stringify!($opt)).replace('_', "-");
                return Err(format!("{context} conflicts with {name}"));
            })*
        };
    }

    ensure_options_absent! {
        command,
        group,
        help,
        login,
        preserve_environment,
        pty,
        shell,
        supp_group,
        version,
        whitelist_environment,
        positional_args,
    };

    if !positional_args.is_absent() {
        return Err(format!("{context} conflicts with positional argument"));
    }

    Ok(())
}

trait IsAbsent {
    fn is_absent(&self) -> bool;
}

impl IsAbsent for bool {
    fn is_absent(&self) -> bool {
        !*self
    }
}

impl<T> IsAbsent for Option<T> {
    fn is_absent(&self) -> bool {
        self.is_none()
    }
}

impl<T> IsAbsent for Vec<T> {
    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct SuOptions {
    // -c
    command: Option<String>,
    // -g
    group: Vec<SudoString>,
    // -h
    help: bool,
    // -l
    login: bool,
    // -p
    preserve_environment: bool,
    // -P
    pty: bool,
    // -s
    shell: Option<PathBuf>,
    // -G
    supp_group: Vec<SudoString>,
    // -V
    version: bool,
    // -w
    whitelist_environment: Vec<String>,

    positional_args: Vec<String>,
}

type OptionSetter = fn(&mut SuOptions, Option<String>) -> Result<(), String>;

struct SuOption {
    short: char,
    long: &'static str,
    takes_argument: bool,
    set: OptionSetter,
}

impl SuOptions {
    const SU_OPTIONS: &'static [SuOption] = &[
        SuOption {
            short: 'c',
            long: "command",
            takes_argument: true,
            set: |sudo_options, argument| {
                if argument.is_some() {
                    sudo_options.command = argument;
                    Ok(())
                } else {
                    Err("no command provided".into())
                }
            },
        },
        SuOption {
            short: 'g',
            long: "group",
            takes_argument: true,
            set: |sudo_options, argument| {
                if let Some(value) = argument {
                    sudo_options.group.push(SudoString::from_cli_string(value));
                    Ok(())
                } else {
                    Err("no group provided".into())
                }
            },
        },
        SuOption {
            short: 'G',
            long: "supp-group",
            takes_argument: true,
            set: |sudo_options, argument| {
                if let Some(value) = argument {
                    sudo_options
                        .supp_group
                        .push(SudoString::from_cli_string(value));
                    Ok(())
                } else {
                    Err("no supplementary group provided".into())
                }
            },
        },
        SuOption {
            short: 'l',
            long: "login",
            takes_argument: false,
            set: |sudo_options, _| {
                if sudo_options.login {
                    Err(more_than_once("--login"))
                } else {
                    sudo_options.login = true;
                    Ok(())
                }
            },
        },
        SuOption {
            short: 'p',
            long: "preserve-environment",
            takes_argument: false,
            set: |sudo_options, _| {
                if sudo_options.preserve_environment {
                    Err(more_than_once("--preserve-environment"))
                } else {
                    sudo_options.preserve_environment = true;
                    Ok(())
                }
            },
        },
        SuOption {
            short: 'm',
            long: "preserve-environment",
            takes_argument: false,
            set: |sudo_options, _| {
                if sudo_options.preserve_environment {
                    Err(more_than_once("--preserve-environment"))
                } else {
                    sudo_options.preserve_environment = true;
                    Ok(())
                }
            },
        },
        SuOption {
            short: 'P',
            long: "pty",
            takes_argument: false,
            set: |sudo_options, _| {
                if sudo_options.pty {
                    Err(more_than_once("--pty"))
                } else {
                    sudo_options.pty = true;
                    Ok(())
                }
            },
        },
        SuOption {
            short: 's',
            long: "shell",
            takes_argument: true,
            set: |sudo_options, argument| {
                if let Some(path) = argument {
                    sudo_options.shell = Some(PathBuf::from(path));
                    Ok(())
                } else {
                    Err("no shell provided".into())
                }
            },
        },
        SuOption {
            short: 'w',
            long: "whitelist-environment",
            takes_argument: true,
            set: |sudo_options, argument| {
                if let Some(list) = argument {
                    let values: Vec<String> = list.split(',').map(str::to_string).collect();
                    sudo_options.whitelist_environment.extend(values);
                    Ok(())
                } else {
                    Err("no environment whitelist provided".into())
                }
            },
        },
        SuOption {
            short: 'V',
            long: "version",
            takes_argument: false,
            set: |sudo_options, _| {
                if sudo_options.version {
                    Err(more_than_once("--version"))
                } else {
                    sudo_options.version = true;
                    Ok(())
                }
            },
        },
        SuOption {
            short: 'h',
            long: "help",
            takes_argument: false,
            set: |sudo_options, _| {
                if sudo_options.help {
                    Err(more_than_once("--help"))
                } else {
                    sudo_options.help = true;
                    Ok(())
                }
            },
        },
    ];

    /// parse su arguments into SuOptions struct
    pub(super) fn parse_arguments(
        arguments: impl IntoIterator<Item = String>,
    ) -> Result<SuOptions, String> {
        let mut options: SuOptions = SuOptions::default();
        let mut arg_iter = arguments.into_iter().skip(1);

        while let Some(arg) = arg_iter.next() {
            // - or -l or --login indicates a login shell should be started
            if arg == "-" {
                if options.login {
                    return Err(more_than_once("--login"));
                } else {
                    options.login = true;
                }
            } else if arg == "--" {
                // only positional arguments after this point
                options.positional_args.extend(arg_iter);

                break;

                // if the argument starts with -- it must be a full length option name
            } else if let Some(unprefixed) = arg.strip_prefix("--") {
                // parse assignments like '--group=ferris'
                if let Some((key, value)) = unprefixed.split_once('=') {
                    // lookup the option by name
                    if let Some(option) = Self::SU_OPTIONS.iter().find(|o| o.long == key) {
                        // the value is already present, when the option does not take any arguments this results in an error
                        if option.takes_argument {
                            (option.set)(&mut options, Some(value.to_string()))?;
                        } else {
                            Err(format!("'--{}' does not take any arguments", option.long))?;
                        }
                    } else {
                        Err(format!("unrecognized option '{arg}'"))?;
                    }
                // lookup the option
                } else if let Some(option) = Self::SU_OPTIONS.iter().find(|o| o.long == unprefixed)
                {
                    // try to parse an argument when the option needs an argument
                    if option.takes_argument {
                        let next_arg = arg_iter.next();
                        (option.set)(&mut options, next_arg)?;
                    } else {
                        (option.set)(&mut options, None)?;
                    }
                } else {
                    Err(format!("unrecognized option '{arg}'"))?;
                }
            } else if let Some(unprefixed) = arg.strip_prefix('-') {
                // flags can be grouped, so we loop over the the characters
                let mut chars = unprefixed.chars();
                while let Some(curr) = chars.next() {
                    // lookup the option
                    if let Some(option) = Self::SU_OPTIONS.iter().find(|o| o.short == curr) {
                        // try to parse an argument when one is necessary, either the rest of the current flag group or the next argument
                        let rest = chars.as_str();

                        if option.takes_argument {
                            let next_arg = if rest.is_empty() {
                                arg_iter.next()
                            } else {
                                Some(rest.to_string())
                            };
                            (option.set)(&mut options, next_arg)?;
                            // stop looping over flags if the current flag takes an argument
                            break;
                        } else {
                            // parse flag without argument
                            (option.set)(&mut options, None)?;
                        }
                    } else {
                        Err(format!("unrecognized option '{curr}'"))?;
                    }
                }
            } else {
                options.positional_args.push(arg);
            }
        }

        Ok(options)
    }

    pub(super) fn validate(self) -> Result<SuAction, String> {
        let action = if self.help {
            SuAction::Help(self.try_into()?)
        } else if self.version {
            SuAction::Version(self.try_into()?)
        } else {
            SuAction::Run(self.try_into()?)
        };
        Ok(action)
    }
}

fn more_than_once(flag: &str) -> String {
    format!("argument '{flag}' was provided more than once, but cannot be used multiple times")
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::{SuAction, SuHelpOptions, SuOptions, SuRunOptions, SuVersionOptions};

    fn parse(args: &[&str]) -> SuAction {
        let mut args = args.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        args.insert(0, "/bin/su".to_string());
        SuOptions::parse_arguments(args)
            .unwrap()
            .validate()
            .unwrap()
    }

    #[test]
    fn it_parses_group() {
        let expected = SuAction::Run(SuRunOptions {
            group: vec!["ferris".into()],
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-g", "ferris"]));
        assert_eq!(expected, parse(&["-gferris"]));
        assert_eq!(expected, parse(&["--group", "ferris"]));
        assert_eq!(expected, parse(&["--group=ferris"]));
    }

    #[test]
    fn it_parses_shell_default() {
        let result = parse(&["--shell", "/bin/bash"]);
        assert_eq!(
            result,
            SuAction::Run(SuRunOptions {
                shell: Some("/bin/bash".into()),
                ..<_>::default()
            })
        );
    }

    #[test]
    fn it_parses_whitelist() {
        let result = parse(&["-w", "FOO,BAR"]);
        assert_eq!(
            result,
            SuAction::Run(SuRunOptions {
                whitelist_environment: vec!["FOO".to_string(), "BAR".to_string()],
                ..<_>::default()
            })
        );
    }

    #[test]
    fn it_parses_combined_options() {
        let expected = SuAction::Run(SuRunOptions {
            login: true,
            ..<_>::default()
        });

        assert_eq!(expected, parse(&["-Pl"]));
        assert_eq!(expected, parse(&["-lP"]));
    }

    #[test]
    fn it_parses_combined_options_and_arguments() {
        let expected = SuAction::Run(SuRunOptions {
            login: true,
            shell: Some("/bin/bash".into()),
            ..<_>::default()
        });

        assert_eq!(expected, parse(&["-Pls/bin/bash"]));
        assert_eq!(expected, parse(&["-Pls", "/bin/bash"]));
        assert_eq!(expected, parse(&["-Pl", "-s/bin/bash"]));
        assert_eq!(expected, parse(&["-lP", "-s", "/bin/bash"]));
        assert_eq!(expected, parse(&["-lP", "--shell=/bin/bash"]));
        assert_eq!(expected, parse(&["-lP", "--shell", "/bin/bash"]));
    }

    #[test]
    fn it_parses_an_user() {
        let expected = SuAction::Run(SuRunOptions {
            user: "ferris".into(),
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-P", "ferris"]));
        assert_eq!(expected, parse(&["ferris", "-P"]));
    }

    #[test]
    fn it_parses_arguments() {
        let expected = SuAction::Run(SuRunOptions {
            user: "ferris".into(),
            arguments: vec!["script.sh".to_string()],
            ..<_>::default()
        });

        assert_eq!(expected, parse(&["-P", "ferris", "script.sh"]));
    }

    #[test]
    fn it_parses_command() {
        let expected = SuAction::Run(SuRunOptions {
            command: Some("'echo hi'".to_string()),
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-c", "'echo hi'"]));
        assert_eq!(expected, parse(&["-c'echo hi'"]));
        assert_eq!(expected, parse(&["--command", "'echo hi'"]));
        assert_eq!(expected, parse(&["--command='echo hi'"]));

        let expected = SuAction::Run(SuRunOptions {
            command: Some("env".to_string()),
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-c", "env"]));
        assert_eq!(expected, parse(&["-cenv"]));
        assert_eq!(expected, parse(&["--command", "env"]));
        assert_eq!(expected, parse(&["--command=env"]));
    }

    #[test]
    fn it_parses_supplementary_group() {
        let expected = SuAction::Run(SuRunOptions {
            supp_group: vec!["ferris".into()],
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-G", "ferris"]));
        assert_eq!(expected, parse(&["-Gferris"]));
        assert_eq!(expected, parse(&["--supp-group", "ferris"]));
        assert_eq!(expected, parse(&["--supp-group=ferris"]));
    }

    #[test]
    fn it_parses_multiple_supplementary_groups() {
        let expected = SuAction::Run(SuRunOptions {
            supp_group: vec!["ferris".into(), "krabbetje".into(), "krabbe".into()],
            ..<_>::default()
        });
        assert_eq!(
            expected,
            parse(&["-G", "ferris", "-G", "krabbetje", "--supp-group", "krabbe"])
        );
    }

    #[test]
    fn it_parses_login() {
        let expected = SuAction::Run(SuRunOptions {
            login: true,
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-"]));
        assert_eq!(expected, parse(&["-l"]));
        assert_eq!(expected, parse(&["--login"]));
    }

    #[test]
    fn it_parses_pty() {
        let expected = SuAction::Run(<_>::default());
        assert_eq!(expected, parse(&["-P"]));
        assert_eq!(expected, parse(&["--pty"]));
    }

    #[test]
    fn it_parses_shell() {
        let expected = SuAction::Run(SuRunOptions {
            shell: Some("some-shell".into()),
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-s", "some-shell"]));
        assert_eq!(expected, parse(&["-ssome-shell"]));
        assert_eq!(expected, parse(&["--shell", "some-shell"]));
        assert_eq!(expected, parse(&["--shell=some-shell"]));
    }

    #[test]
    fn it_parses_whitelist_environment() {
        let expected = SuAction::Run(SuRunOptions {
            whitelist_environment: vec!["FOO".to_string(), "BAR".to_string()],
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-w", "FOO,BAR"]));
        assert_eq!(expected, parse(&["-wFOO,BAR"]));
        assert_eq!(expected, parse(&["--whitelist-environment", "FOO,BAR"]));
        assert_eq!(expected, parse(&["--whitelist-environment=FOO,BAR"]));
    }

    #[test]
    fn it_parses_help() {
        let expected = SuAction::Help(SuHelpOptions {});
        assert_eq!(expected, parse(&["-h"]));
        assert_eq!(expected, parse(&["--help"]));
    }

    #[test]
    fn it_parses_version() {
        let expected = SuAction::Version(SuVersionOptions {});
        assert_eq!(expected, parse(&["-V"]));
        assert_eq!(expected, parse(&["--version"]));
    }

    #[test]
    fn short_flag_whitespace() {
        let expected = SuAction::Run(SuRunOptions {
            group: vec![" ".into()],
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-g "]));
    }

    #[test]
    fn short_flag_whitespace_positional_argument() {
        let expected = SuAction::Run(SuRunOptions {
            group: vec![" ".into()],
            user: "ghost".into(),
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-g ", "ghost"]));
    }

    #[test]
    fn long_flag_equal_whitespace() {
        let expected = SuAction::Run(SuRunOptions {
            group: vec![" ".into()],
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["--group= "]));
    }

    #[test]
    fn flag_after_positional_argument() {
        let expected = SuAction::Run(SuRunOptions {
            arguments: vec![],
            login: true,
            user: "ferris".into(),
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["ferris", "-l"]));
    }

    #[test]
    fn flags_after_dash() {
        let expected = SuAction::Run(SuRunOptions {
            command: Some("echo".to_string()),
            login: true,
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["-", "-c", "echo"]));
    }

    #[test]
    fn only_positional_args_after_dashdash() {
        let expected = SuAction::Run(SuRunOptions {
            user: "ferris".into(),
            arguments: vec!["-c".to_string(), "echo".to_string()],
            ..<_>::default()
        });
        assert_eq!(expected, parse(&["--", "ferris", "-c", "echo"]));
    }

    #[test]
    fn repeated_boolean_flag() {
        let f = |s: &str| s.to_string();

        assert!(SuOptions::parse_arguments(["su", "-l", "-l"].map(f)).is_err());
        assert!(SuOptions::parse_arguments(["su", "-", "-l"].map(f)).is_err());
        assert!(SuOptions::parse_arguments(["su", "--login", "-l"].map(f)).is_err());

        assert!(SuOptions::parse_arguments(["su", "-p", "-p"].map(f)).is_err());
        assert!(SuOptions::parse_arguments(["su", "-p", "--preserve-environment"].map(f)).is_err());
    }
}
