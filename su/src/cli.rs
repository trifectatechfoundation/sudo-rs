#[derive(Default, Debug, PartialEq)]
pub struct SuOptions {
    command: Option<String>,
    group: Option<String>,
    supp_group: Option<String>,
    pty: bool,
    login: bool,
    shell: Option<String>,
    whitelist_environment: Vec<String>,
    help: bool,
    version: bool,
}

impl SuOptions {
    const TAKES_ARGUMENT: &[char] = &['c', 'g', 'G', 'w', 's'];

    pub fn from_env() -> Result<SuOptions, &'static str> {
        let args = std::env::args().collect();

        Self::parse_arguments(args)
    }

    fn parse_arguments(arguments: Vec<String>) -> Result<SuOptions, &'static str> {
        let mut options: SuOptions = SuOptions::default();
        let mut arg_iter = arguments
            .into_iter()
            .flat_map(|segment| {
                if segment.starts_with("--") && segment.contains('=') {
                    // convert argument assignment to seperate segment
                    segment.splitn(2, '=').map(str::to_string).collect()
                } else if segment.starts_with('-')
                    && !segment.starts_with("--")
                    && segment.len() > 2
                {
                    // split combined shorthand options
                    let mut flags: Vec<String> = vec![];

                    for (n, char) in segment.trim_start_matches('-').chars().enumerate() {
                        flags.push(format!("-{char}"));

                        // convert option argument to seperate segment
                        if Self::TAKES_ARGUMENT.contains(&char) {
                            flags.push(segment[(n + 2)..].to_string());
                            break;
                        }
                    }

                    flags
                } else {
                    vec![segment]
                }
            })
            .peekable();

        while let Some(arg) = arg_iter.next() {
            match arg.as_str() {
                "-c" | "--command" => {
                    options.command =
                        Some(arg_iter.next().ok_or("no command provided")?.to_string());
                }
                "-g" | "--group" => {
                    options.group = Some(arg_iter.next().ok_or("no group provided")?.to_string());
                }
                "-G" | "--supp-group" => {
                    options.supp_group = Some(
                        arg_iter
                            .next()
                            .to_owned()
                            .ok_or("no additional group provided")?
                            .to_string(),
                    );
                }
                "-" | "-l" | "--login" => {
                    options.login = true;
                }
                "-p" | "--pty" => {
                    options.pty = true;
                }
                "-s" | "--shell" => {
                    options.shell = Some(
                        arg_iter
                            .next()
                            .to_owned()
                            .ok_or("no shell provided")?
                            .to_string(),
                    );
                }
                "-w" | "--whitelist-environemnt" => {
                    options.whitelist_environment = arg_iter
                        .next()
                        .ok_or("no environment list provided")?
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
                _ => {}
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
        let result = SuOptions::parse_arguments(["--login"].map(str::to_string).to_vec()).unwrap();
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
        let result =
            SuOptions::parse_arguments(["--shell", "/bin/bash"].map(str::to_string).to_vec())
                .unwrap();
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
        let result =
            SuOptions::parse_arguments(["-w", "FOO,BAR"].map(str::to_string).to_vec()).unwrap();
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
        let result = SuOptions::parse_arguments(["-pl"].map(str::to_string).to_vec()).unwrap();
        assert_eq!(
            result,
            SuOptions {
                login: true,
                pty: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn it_parses_combined_options_and_arguments() {
        let result =
            SuOptions::parse_arguments(["-pls/bin/bash"].map(str::to_string).to_vec()).unwrap();
        assert_eq!(
            result,
            SuOptions {
                login: true,
                pty: true,
                shell: Some("/bin/bash".to_string()),
                ..Default::default()
            }
        );
    }
}
