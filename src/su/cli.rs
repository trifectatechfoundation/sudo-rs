use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub struct SuOptions {
    pub user: String,
    pub command: Option<String>,
    pub group: Option<String>,
    pub supp_group: Option<String>,
    pub pty: bool,
    pub login: bool,
    pub shell: Option<PathBuf>,
    pub whitelist_environment: Vec<String>,
    pub arguments: Vec<String>,
    pub action: SuAction,
}

impl Default for SuOptions {
    fn default() -> Self {
        Self {
            user: "root".to_owned(),
            command: None,
            group: None,
            supp_group: None,
            pty: false,
            login: false,
            shell: None,
            whitelist_environment: vec![],
            arguments: vec![],
            action: SuAction::Run,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum SuAction {
    Help,
    Version,
    Run,
}

type OptionSetter = &'static dyn Fn(&mut SuOptions, Option<String>) -> Result<(), String>;

struct SuOption {
    short: char,
    long: &'static str,
    takes_argument: bool,
    set: OptionSetter,
}

impl SuOptions {
    const SU_OPTIONS: &[SuOption] = &[
        SuOption {
            short: 'c',
            long: "command",
            takes_argument: true,
            set: &|sudo_options, argument| {
                if argument.is_some() {
                    sudo_options.command = argument;
                } else {
                    Err("no command provided")?
                }

                Ok(())
            },
        },
        SuOption {
            short: 'g',
            long: "group",
            takes_argument: true,
            set: &|sudo_options, argument| {
                if argument.is_some() {
                    sudo_options.group = argument;
                } else {
                    Err("no group provided")?
                }

                Ok(())
            },
        },
        SuOption {
            short: 'G',
            long: "supp-group",
            takes_argument: true,
            set: &|sudo_options, argument| {
                if argument.is_some() {
                    sudo_options.supp_group = argument;
                } else {
                    Err("no supplementary group provided")?
                }

                Ok(())
            },
        },
        SuOption {
            short: 'l',
            long: "login",
            takes_argument: false,
            set: &|sudo_options, _| {
                sudo_options.login = true;
                Ok(())
            },
        },
        SuOption {
            short: 'P',
            long: "pty",
            takes_argument: false,
            set: &|sudo_options, _| {
                sudo_options.pty = true;
                Ok(())
            },
        },
        SuOption {
            short: 's',
            long: "shell",
            takes_argument: true,
            set: &|sudo_options, argument| {
                if let Some(path) = argument {
                    sudo_options.shell = Some(PathBuf::from(path));
                } else {
                    Err("no shell provided")?
                }

                Ok(())
            },
        },
        SuOption {
            short: 'w',
            long: "whitelist-environment",
            takes_argument: true,
            set: &|sudo_options, argument| {
                if let Some(list) = argument {
                    sudo_options.whitelist_environment =
                        list.split(',').map(str::to_string).collect();
                } else {
                    Err("no enivronment whitelist provided")?
                }

                Ok(())
            },
        },
        SuOption {
            short: 'V',
            long: "version",
            takes_argument: false,
            set: &|sudo_options, _| {
                sudo_options.action = SuAction::Version;
                Ok(())
            },
        },
        SuOption {
            short: 'h',
            long: "help",
            takes_argument: false,
            set: &|sudo_options, _| {
                sudo_options.action = SuAction::Help;
                Ok(())
            },
        },
    ];

    pub fn from_env() -> Result<SuOptions, String> {
        let args = std::env::args().collect();

        Self::parse_arguments(args)
    }

    /// parse su arguments into SuOptions struct
    fn parse_arguments(arguments: Vec<String>) -> Result<SuOptions, String> {
        let mut options: SuOptions = SuOptions::default();
        let mut arg_iter = arguments.into_iter().skip(1);

        while let Some(arg) = arg_iter.next() {
            // - or -l or --login indicates a login shell should be started
            if arg == "-" {
                options.login = true;
            // if the argument starts with -- it must be a full length option name
            } else if arg.starts_with("--") {
                // parse assignments like '--group=ferris'
                if arg.contains('=') {
                    // convert assignment to normal tokens
                    let (key, value) = arg.split_once('=').unwrap();
                    // lookup the option by name
                    if let Some(option) = Self::SU_OPTIONS.iter().find(|o| o.long == &key[2..]) {
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
                } else if let Some(option) = Self::SU_OPTIONS.iter().find(|o| o.long == &arg[2..]) {
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
            } else if arg.starts_with('-') {
                // flags can be grouped, so we loop over the the characters
                for (n, char) in arg.trim_start_matches('-').chars().enumerate() {
                    // lookup the option
                    if let Some(option) = Self::SU_OPTIONS.iter().find(|o| o.short == char) {
                        // try to parse an argument when one is necessary, either the rest of the current flag group or the next argument
                        if option.takes_argument {
                            let rest = arg[(n + 2)..].trim().to_string();
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
                // when no option is provided (styarting with - or --) the next argument is interpreted as an username
                options.user = arg;
                // the rest of the arguments are passed to the shell
                options.arguments = arg_iter.collect();
                break;
            }
        }

        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::{SuAction, SuOptions};

    fn parse(args: &[&str]) -> SuOptions {
        let mut args = args.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        args.insert(0, "/bin/su".to_string());
        SuOptions::parse_arguments(args).unwrap()
    }

    #[test]

    fn it_parses_group() {
        let expected = SuOptions {
            group: Some("ferris".to_string()),
            ..Default::default()
        };
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
            SuOptions {
                shell: Some("/bin/bash".into()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn it_parses_whitelist() {
        let result = parse(&["-w", "FOO,BAR"]);
        assert_eq!(
            result,
            SuOptions {
                whitelist_environment: vec!["FOO".to_string(), "BAR".to_string()],
                ..Default::default()
            }
        );
    }

    #[test]
    fn it_parses_combined_options() {
        let expected = SuOptions {
            login: true,
            pty: true,
            ..Default::default()
        };

        assert_eq!(expected, parse(&["-Pl"]));
        assert_eq!(expected, parse(&["-lP"]));
    }

    #[test]
    fn it_parses_combined_options_and_arguments() {
        let expected = SuOptions {
            login: true,
            pty: true,
            shell: Some("/bin/bash".into()),
            ..Default::default()
        };

        assert_eq!(expected, parse(&["-Pls/bin/bash"]));
        assert_eq!(expected, parse(&["-Pls", "/bin/bash"]));
        assert_eq!(expected, parse(&["-Pl", "-s/bin/bash"]));
        assert_eq!(expected, parse(&["-lP", "-s", "/bin/bash"]));
        assert_eq!(expected, parse(&["-lP", "--shell=/bin/bash"]));
        assert_eq!(expected, parse(&["-lP", "--shell", "/bin/bash"]));
    }

    #[test]
    fn it_parses_an_user() {
        assert_eq!(
            SuOptions {
                user: "ferris".to_string(),
                pty: true,
                ..Default::default()
            },
            parse(&["-P", "ferris"])
        );

        assert_eq!(
            SuOptions {
                user: "ferris".to_string(),
                arguments: vec!["-P".to_string()],
                ..Default::default()
            },
            parse(&["ferris", "-P"])
        );
    }

    #[test]
    fn it_parses_arguments() {
        let expected = SuOptions {
            user: "ferris".to_string(),
            pty: true,
            arguments: vec!["script.sh".to_string()],
            ..Default::default()
        };

        assert_eq!(expected, parse(&["-P", "ferris", "script.sh"]));
    }

    #[test]
    fn it_parses_command() {
        let expected = SuOptions {
            command: Some("'echo hi'".to_string()),
            ..Default::default()
        };
        assert_eq!(expected, parse(&["-c", "'echo hi'"]));
        assert_eq!(expected, parse(&["-c'echo hi'"]));
        assert_eq!(expected, parse(&["--command", "'echo hi'"]));
        assert_eq!(expected, parse(&["--command='echo hi'"]));

        let expected = SuOptions {
            command: Some("env".to_string()),
            ..Default::default()
        };
        assert_eq!(expected, parse(&["-c", "env"]));
        assert_eq!(expected, parse(&["-cenv"]));
        assert_eq!(expected, parse(&["--command", "env"]));
        assert_eq!(expected, parse(&["--command=env"]));
    }

    #[test]
    fn it_parses_supplementary_group() {
        let expected = SuOptions {
            supp_group: Some("ferris".to_string()),
            ..Default::default()
        };
        assert_eq!(expected, parse(&["-G", "ferris"]));
        assert_eq!(expected, parse(&["-Gferris"]));
        assert_eq!(expected, parse(&["--supp-group", "ferris"]));
        assert_eq!(expected, parse(&["--supp-group=ferris"]));
    }

    #[test]
    fn it_parses_login() {
        let expected = SuOptions {
            login: true,
            ..Default::default()
        };
        assert_eq!(expected, parse(&["-"]));
        assert_eq!(expected, parse(&["-l"]));
        assert_eq!(expected, parse(&["--login"]));
    }

    #[test]
    fn it_parses_pty() {
        let expected = SuOptions {
            pty: true,
            ..Default::default()
        };
        assert_eq!(expected, parse(&["-P"]));
        assert_eq!(expected, parse(&["--pty"]));
    }

    #[test]
    fn it_parses_shell() {
        let expected = SuOptions {
            shell: Some("some-shell".into()),
            ..Default::default()
        };
        assert_eq!(expected, parse(&["-s", "some-shell"]));
        assert_eq!(expected, parse(&["-ssome-shell"]));
        assert_eq!(expected, parse(&["--shell", "some-shell"]));
        assert_eq!(expected, parse(&["--shell=some-shell"]));
    }

    #[test]
    fn it_parses_whitelist_environment() {
        let expected = SuOptions {
            whitelist_environment: vec!["FOO".to_string(), "BAR".to_string()],
            ..Default::default()
        };
        assert_eq!(expected, parse(&["-w", "FOO,BAR"]));
        assert_eq!(expected, parse(&["-wFOO,BAR"]));
        assert_eq!(expected, parse(&["--whitelist-environment", "FOO,BAR"]));
        assert_eq!(expected, parse(&["--whitelist-environment=FOO,BAR"]));
    }

    #[test]
    fn it_parses_help() {
        let expected = SuOptions {
            action: SuAction::Help,
            ..Default::default()
        };
        assert_eq!(expected, parse(&["-h"]));
        assert_eq!(expected, parse(&["--help"]));
    }

    #[test]
    fn it_parses_version() {
        let expected = SuOptions {
            action: SuAction::Version,
            ..Default::default()
        };
        assert_eq!(expected, parse(&["-V"]));
        assert_eq!(expected, parse(&["--version"]));
    }
}
