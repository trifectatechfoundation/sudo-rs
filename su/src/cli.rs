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

impl SuOptions {
    const TAKES_ARGUMENT: &[char] = &['c', 'g', 'G', 'w', 's'];

    pub fn from_env() -> Result<SuOptions, String> {
        let args = std::env::args().collect();

        Self::parse_arguments(args)
    }

    fn normalize_arguments(arguments: Vec<String>) -> impl Iterator<Item = String> {
        arguments.into_iter().flat_map(|segment| {
            if segment.starts_with("--") && segment.contains('=') {
                // convert argument assignment to seperate segment
                segment.splitn(2, '=').map(str::to_string).collect()
            } else if segment.starts_with('-') && !segment.starts_with("--") && segment.len() > 2 {
                // split combined shorthand options
                let mut flags: Vec<String> = vec![];

                for (n, char) in segment.trim_start_matches('-').chars().enumerate() {
                    flags.push(format!("-{char}"));

                    // convert option argument to seperate segment
                    if Self::TAKES_ARGUMENT.contains(&char) {
                        let rest = segment[(n + 2)..].trim().to_string();
                        if !rest.is_empty() {
                            flags.push(rest);
                        }
                        break;
                    }
                }

                flags
            } else {
                vec![segment]
            }
        })
    }

    fn parse_arguments(arguments: Vec<String>) -> Result<SuOptions, String> {
        let mut options: SuOptions = SuOptions::default();
        let mut arg_iter = Self::normalize_arguments(arguments);

        while let Some(arg) = arg_iter.next() {
            match arg.as_str() {
                "-c" | "--command" => {
                    options.command =
                        Some(arg_iter.next().ok_or("no command provided".to_string())?);
                }
                "-g" | "--group" => {
                    options.group = Some(arg_iter.next().ok_or("no group provided".to_string())?);
                }
                "-G" | "--supp-group" => {
                    options.supp_group = Some(
                        arg_iter
                            .next()
                            .ok_or("no additional group provided".to_string())?,
                    );
                }
                "-" | "-l" | "--login" => {
                    options.login = true;
                }
                "-p" | "--pty" => {
                    options.pty = true;
                }
                "-s" | "--shell" => {
                    options.shell = Some(arg_iter.next().ok_or("no shell provided".to_string())?);
                }
                "-w" | "--whitelist-environemnt" => {
                    options.whitelist_environment = arg_iter
                        .next()
                        .ok_or("no environment list provided".to_string())?
                        .split(',')
                        .map(str::to_string)
                        .collect()
                }
                "-h" | "--help" => {
                    options.help = true;
                }
                "-v" | "--version" => {
                    options.help = true;
                }
                option if option.starts_with('-') => {
                    Err(format!("invalid option {option}"))?;
                }
                _user if options.user.is_none() => {
                    options.user = Some(arg);
                }
                _argument => {
                    options.arguments.push(arg);
                }
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

        assert_eq!(expected, parse(&["-pl"]));
        assert_eq!(expected, parse(&["-lp"]));
    }

    #[test]
    fn it_parses_combined_options_and_arguments() {
        let expected = SuOptions {
            login: true,
            pty: true,
            shell: Some("/bin/bash".to_string()),
            ..Default::default()
        };

        assert_eq!(expected, parse(&["-pls/bin/bash"]));
        assert_eq!(expected, parse(&["-pls", "/bin/bash"]));
        assert_eq!(expected, parse(&["-pl", "-s/bin/bash"]));
        assert_eq!(expected, parse(&["-lp", "-s", "/bin/bash"]));
        assert_eq!(expected, parse(&["-lp", "--shell=/bin/bash"]));
        assert_eq!(expected, parse(&["-lp", "--shell", "/bin/bash"]));
    }

    #[test]
    fn it_parses_an_user() {
        let expected = SuOptions {
            user: Some("ferris".to_string()),
            pty: true,
            ..Default::default()
        };

        assert_eq!(expected, parse(&["-p", "ferris"]));
        assert_eq!(expected, parse(&["ferris", "-p"]));
    }

    #[test]
    fn it_parses_arguments() {
        let expected = SuOptions {
            user: Some("ferris".to_string()),
            pty: true,
            arguments: vec!["script.sh".to_string()],
            ..Default::default()
        };

        assert_eq!(expected, parse(&["-p", "ferris", "script.sh"]));
        assert_eq!(expected, parse(&["ferris", "-p", "script.sh"]));
    }
}
