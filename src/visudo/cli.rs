#[derive(Debug, PartialEq)]
pub(crate) struct VisudoOptions {
    pub(crate) file: Option<String>,
    pub(crate) includes: bool,
    pub(crate) quiet: bool,
    pub(crate) strict: bool,
    pub(crate) action: VisudoAction,
}

impl Default for VisudoOptions {
    fn default() -> Self {
        Self {
            file: None,
            includes: true,
            quiet: false,
            strict: false,
            action: VisudoAction::Run,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum VisudoAction {
    Help,
    Version,
    Check,
    Run,
}

type OptionSetter = fn(&mut VisudoOptions, Option<String>) -> Result<(), String>;

struct VisudoOption {
    short: char,
    long: &'static str,
    takes_argument: bool,
    set: OptionSetter,
}

impl VisudoOptions {
    const VISUDO_OPTIONS: &[VisudoOption] = &[
        VisudoOption {
            short: 'c',
            long: "check",
            takes_argument: false,
            set: |options, _| {
                options.action = VisudoAction::Check;
                Ok(())
            },
        },
        VisudoOption {
            short: 'f',
            long: "file",
            takes_argument: true,
            set: |options, argument| {
                options.file = Some(argument.ok_or("option requires an argument -- 'f'")?);
                Ok(())
            },
        },
        VisudoOption {
            short: 'h',
            long: "help",
            takes_argument: false,
            set: |options, _| {
                options.action = VisudoAction::Help;
                Ok(())
            },
        },
        VisudoOption {
            short: 'I',
            long: "no-includes",
            takes_argument: false,
            set: |options, _| {
                options.includes = true;
                Ok(())
            },
        },
        VisudoOption {
            short: 'q',
            long: "quiet",
            takes_argument: false,
            set: |options, _| {
                options.quiet = true;
                Ok(())
            },
        },
        VisudoOption {
            short: 's',
            long: "strict",
            takes_argument: false,
            set: |options, _| {
                options.strict = true;
                Ok(())
            },
        },
        VisudoOption {
            short: 'V',
            long: "version",
            takes_argument: false,
            set: |options, _| {
                options.action = VisudoAction::Version;
                Ok(())
            },
        },
    ];

    pub(crate) fn from_env() -> Result<VisudoOptions, String> {
        let args = std::env::args().collect();

        Self::parse_arguments(args)
    }

    /// parse su arguments into VisudoOptions struct
    pub(crate) fn parse_arguments(arguments: Vec<String>) -> Result<VisudoOptions, String> {
        let mut options: VisudoOptions = VisudoOptions::default();
        let mut arg_iter = arguments.into_iter().skip(1);

        while let Some(arg) = arg_iter.next() {
            // if the argument starts with -- it must be a full length option name
            if arg.starts_with("--") {
                // parse assignments like '--file=/etc/sudoers'
                if arg.contains('=') {
                    // convert assignment to normal tokens
                    let (key, value) = arg.split_once('=').unwrap();
                    // lookup the option by name
                    if let Some(option) = Self::VISUDO_OPTIONS.iter().find(|o| o.long == &key[2..])
                    {
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
                } else if let Some(option) =
                    Self::VISUDO_OPTIONS.iter().find(|o| o.long == &arg[2..])
                {
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
                // flags can be grouped, so we loop over the characters
                for (n, char) in arg.trim_start_matches('-').chars().enumerate() {
                    // lookup the option
                    if let Some(option) = Self::VISUDO_OPTIONS.iter().find(|o| o.short == char) {
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
