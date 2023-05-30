#[derive(Default, Debug, PartialEq)]
pub struct SuOptions {
    user: Option<String>,
    command: Option<String>,
    group: Option<String>,
    supp_group: Option<String>,
    pty: bool,
    login: bool,
    shell: Option<String>,
    whitelist_environment: Vec<String>,
    help: bool,
    version: bool,
    arguments: Vec<String>,
}

struct SuOption {
    short: char,
    long: &'static str,
    takes_argument: bool,
    set: &'static dyn Fn(&mut SuOptions, Option<String>) -> Result<(), String>,
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
                if argument.is_some() {
                    sudo_options.shell = argument;
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
                    sudo_options.whitelist_environment = list.split(',').map(str::to_string).collect();
                } else {
                    Err("no enivronment whitelist provided")?
                }

                Ok(())
            },
        },
        SuOption {
            short: 'v',
            long: "version",
            takes_argument: false,
            set: &|sudo_options, _| {
                sudo_options.version = true;
                Ok(())
            },
        },
        SuOption {
            short: 'h',
            long: "help",
            takes_argument: false,
            set: &|sudo_options, _| {
                sudo_options.help = true;
                Ok(())
            },
        },
    ];

    pub fn from_env() -> Result<SuOptions, String> {
        let args = std::env::args().collect();

        Self::parse_arguments(args)
    }

    fn parse_arguments(arguments: Vec<String>) -> Result<SuOptions, String> {
        let mut options: SuOptions = SuOptions::default();
        let mut arg_iter = arguments.into_iter();

        while let Some(arg) = arg_iter.next() { 
            if arg == "-" {
                options.login = true;
            } if arg.starts_with("--") {
                if arg.contains('=') {
                    // convert assignment to normal tokens
                    let (key, value) = arg.split_once('=').unwrap();
                    if let Some(option) = Self::SU_OPTIONS.iter().find(|o| o.long == &key[2..]) {
                        if option.takes_argument {
                            (option.set)(&mut options, Some(value.to_string()))?;
                        } else {
                            Err(format!("'--{}' does not take any arguments", option.long))?;
                        }
                    } else {
                        Err(format!("unrecognized option '{}'", arg))?;
                    }
                } else if let Some(option) = Self::SU_OPTIONS.iter().find(|o| o.long == &arg[2..]) {
                    if option.takes_argument {
                        let next_arg = arg_iter.next();
                        (option.set)(&mut options, next_arg)?;
                    } else {
                        (option.set)(&mut options, None)?;
                    }
                } else {
                    Err(format!("unrecognized option '{}'", arg))?;
                }
            } else if arg.starts_with("-") {
                for (n, char) in arg.trim_start_matches('-').chars().enumerate() {
                    if let Some(option) = Self::SU_OPTIONS.iter().find(|o| o.short == char) {
                        if option.takes_argument {
                            let rest = arg[(n + 2)..].trim().to_string();
                            let next_arg = if rest.is_empty() {
                                 arg_iter.next()
                            } else {
                                Some(rest)
                            };
                            (option.set)(&mut options, next_arg)?;
                            break;
                        } else {
                            (option.set)(&mut options, None)?;
                        }
                    } else {
                        Err(format!("unrecognized option '{}'", char))?;
                    }
                }
            } else {
                options.user = Some(arg);
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

    use super::SuOptions;

    fn parse(args: &[&str]) -> SuOptions {
        SuOptions::parse_arguments(
            args.into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>(),
        )
        .unwrap()
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
    fn it_parses_login() {
        let result = parse(&["--login"]);
        assert_eq!(
            result,
            SuOptions {
                login: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn it_parses_shell_default() {
        let result = parse(&["--shell", "/bin/bash"]);
        assert_eq!(
            result,
            SuOptions {
                shell: Some("/bin/bash".to_string()),
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
            shell: Some("/bin/bash".to_string()),
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
        assert_eq!(SuOptions {
            user: Some("ferris".to_string()),
            pty: true,
            ..Default::default()
        }, parse(&["-P", "ferris"]));

        assert_eq!(SuOptions {
            user: Some("ferris".to_string()),
            arguments: vec!["-P".to_string()],
            ..Default::default()
        }, parse(&["ferris", "-P"]));
    }

    #[test]
    fn it_parses_arguments() {
        let expected = SuOptions {
            user: Some("ferris".to_string()),
            pty: true,
            arguments: vec!["script.sh".to_string()],
            ..Default::default()
        };

        assert_eq!(expected, parse(&["-P", "ferris", "script.sh"]));
    }
}
