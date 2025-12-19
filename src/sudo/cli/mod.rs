#![forbid(unsafe_code)]

use std::os::unix::ffi::OsStrExt;
use std::{borrow::Cow, mem};

use crate::common::{SudoPath, SudoString};
use crate::log::user_warn;

pub mod help;
pub mod help_edit;

#[cfg(test)]
mod tests;

pub enum SudoAction {
    Edit(SudoEditOptions),
    Help(SudoHelpOptions),
    List(SudoListOptions),
    RemoveTimestamp(SudoRemoveTimestampOptions),
    ResetTimestamp(SudoResetTimestampOptions),
    Run(SudoRunOptions),
    Validate(SudoValidateOptions),
    Version(SudoVersionOptions),
}

pub(super) fn is_sudoedit(command_path: Option<String>) -> bool {
    std::path::Path::new(&command_path.unwrap_or_default())
        .file_name()
        .is_some_and(|name| name.as_bytes().starts_with(b"sudoedit"))
}

impl SudoAction {
    /// try to parse and environment variable assignment
    /// parse command line arguments from the environment and handle errors
    pub fn from_env() -> Result<Self, String> {
        Self::try_parse_from(std::env::args())
    }

    pub fn try_parse_from<I, T>(iter: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = T>,
        T: Into<String> + Clone,
    {
        let opts = SudoOptions::try_parse_from(iter)?;
        opts.validate()
    }
}

// sudo -h | -K | -k | -V
pub struct SudoHelpOptions {}

impl TryFrom<SudoOptions> for SudoHelpOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        let help = mem::take(&mut opts.help);
        debug_assert!(help);

        Ok(Self {})
    }
}

// sudo -h | -K | -k | -V
pub struct SudoVersionOptions {}

impl TryFrom<SudoOptions> for SudoVersionOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        let version = mem::take(&mut opts.version);
        debug_assert!(version);

        Ok(Self {})
    }
}

// sudo -h | -K | -k | -V
pub struct SudoRemoveTimestampOptions {}

impl TryFrom<SudoOptions> for SudoRemoveTimestampOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        let remove_timestamp = mem::take(&mut opts.remove_timestamp);
        debug_assert!(remove_timestamp);

        reject_all("--remove-timestamp", opts)?;

        Ok(Self {})
    }
}

// sudo -h | -K | -k | -V
pub struct SudoResetTimestampOptions {}

impl TryFrom<SudoOptions> for SudoResetTimestampOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        debug_assert!(reset_timestamp);

        reject_all("--reset-timestamp", opts)?;

        Ok(Self {})
    }
}

// sudo -v [-ABkNnS] [-g group] [-h host] [-p prompt] [-u user]
pub struct SudoValidateOptions {
    // -A
    pub askpass: bool,
    // -B
    pub bell: bool,
    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
    // -p
    pub prompt: Option<String>,
    // -g
    pub group: Option<SudoString>,
    // -u
    pub user: Option<SudoString>,
}

impl TryFrom<SudoOptions> for SudoValidateOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        let validate = mem::take(&mut opts.validate);
        debug_assert!(validate);

        let askpass = mem::take(&mut opts.askpass);
        let bell = mem::take(&mut opts.bell);
        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let prompt = mem::take(&mut opts.prompt);
        let group = mem::take(&mut opts.group);
        let user = mem::take(&mut opts.user);

        if bell && stdin {
            return Err(xlat!(
                "{context} cannot be used together with {option}",
                context = "--bell",
                option = "--stdin"
            ));
        }

        reject_all("--validate", opts)?;

        Ok(Self {
            askpass,
            bell,
            reset_timestamp,
            non_interactive,
            stdin,
            prompt,
            group,
            user,
        })
    }
}

// sudo -e [-ABkNnS] [-r role] [-t type] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R directory] [-T timeout] [-u user] file ...
pub struct SudoEditOptions {
    // -A
    pub askpass: bool,
    // -B
    pub bell: bool,
    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
    // -p
    pub prompt: Option<String>,
    // -D
    pub chdir: Option<SudoPath>,
    // -g
    pub group: Option<SudoString>,
    // -u
    pub user: Option<SudoString>,
    pub positional_args: Vec<String>,
}

impl TryFrom<SudoOptions> for SudoEditOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        let edit = mem::take(&mut opts.edit);
        debug_assert!(edit);

        let askpass = mem::take(&mut opts.askpass);
        let bell = mem::take(&mut opts.bell);
        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let prompt = mem::take(&mut opts.prompt);
        let chdir = mem::take(&mut opts.chdir);
        let group = mem::take(&mut opts.group);
        let user = mem::take(&mut opts.user);
        let positional_args = mem::take(&mut opts.positional_args);

        if bell && stdin {
            return Err(xlat!(
                "{context} cannot be used together with {option}",
                context = "--bell",
                option = "--stdin"
            ));
        }

        reject_all("--edit", opts)?;

        if positional_args.is_empty() {
            return Err(xlat!("must specify at least one file path").into());
        }

        Ok(Self {
            askpass,
            bell,
            reset_timestamp,
            non_interactive,
            stdin,
            prompt,
            chdir,
            group,
            user,
            positional_args,
        })
    }
}

// sudo -l [-ABkNnS] [-g group] [-h host] [-p prompt] [-U user] [-u user] [command [arg ...]]
pub struct SudoListOptions {
    // -A
    pub askpass: bool,
    // -B
    pub bell: bool,
    // -l OR -l -l
    pub list: List,

    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
    // -p
    pub prompt: Option<String>,
    // -g
    pub group: Option<SudoString>,
    // -U
    pub other_user: Option<SudoString>,
    // -u
    pub user: Option<SudoString>,

    pub positional_args: Vec<String>,
}

impl TryFrom<SudoOptions> for SudoListOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        let askpass = mem::take(&mut opts.askpass);
        let bell = mem::take(&mut opts.bell);
        let list = opts.list.take().unwrap();
        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let prompt = mem::take(&mut opts.prompt);
        let group = mem::take(&mut opts.group);
        let other_user = mem::take(&mut opts.other_user);
        let user = mem::take(&mut opts.user);
        let positional_args = mem::take(&mut opts.positional_args);

        if bell && stdin {
            return Err(xlat!(
                "{context} cannot be used together with {option}",
                context = "--bell",
                option = "--stdin"
            ));
        }

        // when present, `-u` must be accompanied by a command
        let has_command = !positional_args.is_empty();
        let valid_user_flag = user.is_none() || has_command;

        if !valid_user_flag {
            return Err(xlat!(
                "'{option}' flag must be accompanied by a command",
                option = "--user"
            ));
        }

        reject_all("--list", opts)?;

        Ok(Self {
            askpass,
            bell,
            list,
            reset_timestamp,
            non_interactive,
            stdin,
            prompt,
            group,
            other_user,
            user,
            positional_args,
        })
    }
}

// sudo [-ABbEHnPS] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R directory] [-T timeout] [-u user] [VAR=value] [-i | -s] [command [arg ...]]
pub struct SudoRunOptions {
    // -A
    pub askpass: bool,
    // -B
    pub bell: bool,
    // -E
    /* ignored, part of env_var_list */
    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
    // -p
    pub prompt: Option<String>,
    // -D
    pub chdir: Option<SudoPath>,
    // -g
    pub group: Option<SudoString>,
    // -u
    pub user: Option<SudoString>,
    // VAR=value
    pub env_var_list: Vec<(String, String)>,
    // -i
    pub login: bool,
    // -s
    pub shell: bool,
    pub positional_args: Vec<String>,
}

impl TryFrom<SudoOptions> for SudoRunOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        let askpass = mem::take(&mut opts.askpass);
        let bell = mem::take(&mut opts.bell);
        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let prompt = mem::take(&mut opts.prompt);
        let chdir = mem::take(&mut opts.chdir);
        let group = mem::take(&mut opts.group);
        let user = mem::take(&mut opts.user);
        let env_var_list = mem::take(&mut opts.env_var_list);
        let login = mem::take(&mut opts.login);
        let shell = mem::take(&mut opts.shell);
        let positional_args = mem::take(&mut opts.positional_args);

        if bell && stdin {
            return Err(xlat!(
                "{context} cannot be used together with {option}",
                context = "--bell",
                option = "--stdin"
            ));
        }

        let context = match (login, shell, positional_args.is_empty()) {
            (true, false, _) => "--login",
            (false, true, _) => "--shell",
            (false, false, false) => xlat!("command (positional argument)"),

            (true, true, _) => {
                return Err(xlat!(
                    "{context} cannot be used together with {option}",
                    context = "--login",
                    option = "--shell"
                ))
            }
            (false, false, true) => {
                if cfg!(debug_assertions) {
                    // see `SudoOptions::validate`
                    panic!();
                } else {
                    return Err(xlat!(
                        "expected one of: --login, --shell, a command as a positional argument"
                    )
                    .into());
                }
            }
        };

        reject_all(context, opts)?;

        Ok(Self {
            askpass,
            bell,
            reset_timestamp,
            non_interactive,
            stdin,
            prompt,
            chdir,
            group,
            user,
            env_var_list,
            login,
            shell,
            positional_args,
        })
    }
}

#[derive(Default)]
struct SudoOptions {
    // -A
    askpass: bool,
    // -B
    bell: bool,
    // -D
    chdir: Option<SudoPath>,
    // -g
    group: Option<SudoString>,
    // -i
    login: bool,
    // -n
    non_interactive: bool,
    // -U
    other_user: Option<SudoString>,
    // -E
    /* ignored, part of env_var_list */
    // -s
    shell: bool,
    // -S
    stdin: bool,
    // -p
    prompt: Option<String>,
    // -u
    user: Option<SudoString>,

    // additional environment
    env_var_list: Vec<(String, String)>,

    /* actions */
    // -e
    edit: bool,
    // -h
    help: bool,
    // -l
    list: Option<List>,
    // -K
    remove_timestamp: bool,
    // -k
    reset_timestamp: bool,
    // -v
    validate: bool,
    // -V
    version: bool,

    // arguments passed straight through, either separated by -- or just trailing.
    positional_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum List {
    Once,
    Verbose,
}

impl List {
    #[must_use]
    pub fn is_verbose(&self) -> bool {
        matches!(self, Self::Verbose)
    }
}

enum SudoArg {
    Flag(String),
    Argument(String, String),
    Environment(String, String),
    Rest(Vec<String>),
}

impl SudoArg {
    const TAKES_ARGUMENT_SHORT: &'static [char] = &['D', 'g', 'h', 'p', 'R', 'U', 'u'];
    const TAKES_ARGUMENT: &'static [&'static str] = &[
        "chdir",
        "group",
        "host",
        "chroot",
        "other-user",
        "user",
        "prompt",
    ];

    /// argument assignments and shorthand options preprocessing
    /// the iterator should only iterate over the actual arguments
    fn normalize_arguments<I>(mut arg_iter: I) -> Result<Vec<Self>, String>
    where
        I: Iterator<Item = String>,
    {
        let mut processed = vec![];

        while let Some(arg) = arg_iter.next() {
            if arg == "--" {
                processed.push(SudoArg::Rest(arg_iter.collect()));
                break;
            } else if let Some(unprefixed) = arg.strip_prefix("--") {
                if let Some((key, value)) = unprefixed.split_once('=') {
                    // convert assignment to normal tokens

                    // only accept arguments when one is expected
                    // `--preserve-env` is special as it only takes an argument using this `key=value` syntax
                    if !Self::TAKES_ARGUMENT.contains(&key) && key != "preserve-env" {
                        Err(xlat!(
                            "'{option}' does not take any arguments",
                            option = key
                        ))?;
                    }
                    processed.push(SudoArg::Argument("--".to_string() + key, value.to_string()));
                } else if Self::TAKES_ARGUMENT.contains(&unprefixed) {
                    if let Some(next) = arg_iter.next() {
                        processed.push(SudoArg::Argument(arg, next));
                    } else {
                        Err(xlat!("'{option}' expects an argument", option = arg))?;
                    }
                } else {
                    processed.push(SudoArg::Flag(arg));
                }
            } else if let Some(unprefixed) = arg.strip_prefix('-') {
                // split combined shorthand options
                let mut chars = unprefixed.chars();

                while let Some(curr) = chars.next() {
                    let flag = format!("-{curr}");
                    // convert option argument to separate segment
                    if Self::TAKES_ARGUMENT_SHORT.contains(&curr) {
                        let rest = chars.as_str();
                        let next = chars.next();

                        // assignment syntax is not accepted for shorthand arguments
                        if next == Some('=') {
                            Err(xlat!("invalid option '='"))?;
                        }
                        if next.is_some() {
                            processed.push(SudoArg::Argument(flag, rest.to_string()));
                        } else if let Some(next) = arg_iter.next() {
                            processed.push(SudoArg::Argument(flag, next));
                        } else if curr == 'h' {
                            // short version of --help has no arguments
                            processed.push(SudoArg::Flag(flag));
                        } else {
                            Err(xlat!("'{option}' expects an argument", option = flag))?;
                        }
                        break;
                    } else {
                        processed.push(SudoArg::Flag(flag));
                    }
                }
            } else if let Some((key, value)) = try_to_env_var(&arg) {
                processed.push(SudoArg::Environment(key, value));
            } else {
                let mut rest = vec![arg];
                rest.extend(arg_iter);
                processed.push(SudoArg::Rest(rest));
                break;
            }
        }

        Ok(processed)
    }
}

impl SudoOptions {
    fn validate(self) -> Result<SudoAction, String> {
        let action = if self.help {
            SudoAction::Help(self.try_into()?)
        } else if self.version {
            SudoAction::Version(self.try_into()?)
        } else if self.remove_timestamp {
            SudoAction::RemoveTimestamp(self.try_into()?)
        } else if self.validate {
            SudoAction::Validate(self.try_into()?)
        } else if self.list.is_some() {
            SudoAction::List(self.try_into()?)
        } else if self.edit {
            SudoAction::Edit(self.try_into()?)
        } else {
            let is_run = self.login | self.shell | !self.positional_args.is_empty();

            if is_run {
                SudoAction::Run(self.try_into()?)
            } else if self.reset_timestamp {
                SudoAction::ResetTimestamp(self.try_into()?)
            } else {
                return Err(xlat!("expected one of these actions: --help, --version, --remove-timestamp, --validate, --list, --edit, --login, --shell, a command as a positional argument, --reset-timestamp").into());
            }
        };

        Ok(action)
    }

    /// parse an iterator over command line arguments
    fn try_parse_from<I, T>(iter: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = T>,
        T: Into<String> + Clone,
    {
        let mut arg_iter = iter.into_iter().map(Into::into);

        let invoked_as_sudoedit = is_sudoedit(arg_iter.next());

        let mut options = Self {
            edit: invoked_as_sudoedit,
            ..Self::default()
        };

        let arg_iter = SudoArg::normalize_arguments(arg_iter)?
            .into_iter()
            .peekable();

        for arg in arg_iter {
            match arg {
                SudoArg::Flag(flag) => match flag.as_str() {
                    "-A" | "--askpass" => {
                        options.askpass = true;
                    }
                    "-B" | "--bell" => {
                        options.bell = true;
                    }
                    "-E" | "--preserve-env" => {
                        user_warn!(
                            "preserving the entire environment is not supported, '{flag}' is ignored",
                            flag = flag
                        )
                    }
                    "-e" | "--edit" if !invoked_as_sudoedit => {
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
                    "-l" | "--list" => match options.list {
                        None => options.list = Some(List::Once),
                        Some(List::Once) => options.list = Some(List::Verbose),
                        Some(List::Verbose) => {}
                    },
                    "-n" | "--non-interactive" => {
                        options.non_interactive = true;
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
                        Err(xlat!("invalid option provided"))?;
                    }
                },
                SudoArg::Argument(option, value) => match option.as_str() {
                    "-D" | "--chdir" => {
                        options.chdir = Some(SudoPath::from_cli_string(value));
                    }
                    "-E" | "--preserve-env" => {
                        options
                            .env_var_list
                            .extend(value.split(',').filter_map(|var| {
                                std::env::var(var)
                                    .ok()
                                    .map(|value| (var.to_string(), value))
                            }));
                    }
                    "-g" | "--group" => {
                        options.group = Some(SudoString::from_cli_string(value));
                    }
                    "-p" | "--prompt" => {
                        options.prompt = Some(value);
                    }
                    "-U" | "--other-user" => {
                        options.other_user = Some(SudoString::from_cli_string(value));
                    }
                    "-u" | "--user" => {
                        options.user = Some(SudoString::from_cli_string(value));
                    }
                    _option => {
                        Err(xlat!("invalid option provided"))?;
                    }
                },
                SudoArg::Environment(key, value) => {
                    options.env_var_list.push((key, value));
                }
                SudoArg::Rest(mut rest) => {
                    if let Some(cmd) = rest.first() {
                        let cmd = std::path::Path::new(cmd);
                        // This checks if the last character in the path is a /. This
                        // works because the OS directly splits at b'/' without regards
                        // for if it is part of another character (which it can't be
                        // with UTF-8 anyways).
                        let is_dir = cmd.as_os_str().as_bytes().ends_with(b"/");
                        if !options.edit
                            && !is_dir
                            && (cmd.ends_with("sudoedit") || cmd.ends_with("sudoedit-rs"))
                        {
                            user_warn!("sudoedit doesn't need to be run via sudo");
                            options.edit = true;
                            rest.remove(0);
                        }
                    }

                    options.positional_args = rest;
                }
            }
        }

        Ok(options)
    }
}

fn try_to_env_var(arg: &str) -> Option<(String, String)> {
    let (name, value) = arg.split_once('=')?;

    if name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Some((name.to_owned(), value.to_owned()))
    } else {
        None
    }
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

fn ensure_is_absent(context: &str, thing: &dyn IsAbsent, name: &str) -> Result<(), String> {
    if thing.is_absent() {
        Ok(())
    } else {
        Err(xlat!(
            "{context} cannot be used together with {option}",
            context = context,
            option = name
        ))
    }
}

fn reject_all(context: &str, opts: SudoOptions) -> Result<(), String> {
    macro_rules! check_options {
        ($($field:ident $(= $name:expr)?,)*) => {{
            let SudoOptions { $($field),* } = opts;

            $(
                let name = check_options!(@name $field $($name)?);
                ensure_is_absent(context, &$field, &name)?;
            )*

            Ok(())
        }};
        (@name $field:ident) => {{
            let name = concat!("--", stringify!($field));
            if name.contains('_') {
                Cow::Owned(name.replace('_', "-"))
            } else {
                Cow::Borrowed(name)
            }
        }};
        (@name $field:ident $name:expr) => {
            $name
        };
    }

    check_options!(
        askpass,
        bell,
        chdir,
        edit,
        group,
        help,
        list,
        login,
        non_interactive,
        other_user,
        remove_timestamp,
        reset_timestamp,
        shell,
        stdin,
        prompt,
        user,
        validate,
        version,
        positional_args = xlat!("command"),
        env_var_list = xlat!("environment variable"),
    )
}
