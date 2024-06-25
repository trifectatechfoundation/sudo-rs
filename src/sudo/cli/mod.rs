#![forbid(unsafe_code)]

use std::{borrow::Cow, mem};

use crate::common::context::{ContextAction, OptionsForContext};
use crate::common::{SudoPath, SudoString};

pub mod help;

#[cfg(test)]
mod tests;

// remove dead_code when sudoedit has been implemented
#[allow(dead_code)]
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

    #[cfg(test)]
    #[must_use]
    pub fn is_edit(&self) -> bool {
        matches!(self, Self::Edit(..))
    }

    #[cfg(test)]
    #[must_use]
    pub fn is_help(&self) -> bool {
        matches!(self, Self::Help(..))
    }

    #[cfg(test)]
    #[must_use]
    pub fn is_remove_timestamp(&self) -> bool {
        matches!(self, Self::RemoveTimestamp(..))
    }

    #[cfg(test)]
    #[must_use]
    pub fn is_reset_timestamp(&self) -> bool {
        matches!(self, Self::ResetTimestamp(..))
    }

    #[cfg(test)]
    #[must_use]
    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(..))
    }

    #[cfg(test)]
    #[must_use]
    pub fn is_version(&self) -> bool {
        matches!(self, Self::Version(..))
    }

    #[cfg(test)]
    #[must_use]
    pub fn is_validate(&self) -> bool {
        matches!(self, Self::Validate(..))
    }

    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub fn try_into_run(self) -> Result<SudoRunOptions, Self> {
        if let Self::Run(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn is_run(&self) -> bool {
        matches!(self, Self::Run(..))
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

        reject_all("--help", opts)?;

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

        reject_all("--version", opts)?;

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
    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
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

        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let group = mem::take(&mut opts.group);
        let user = mem::take(&mut opts.user);

        reject_all("--validate", opts)?;

        Ok(Self {
            reset_timestamp,
            non_interactive,
            stdin,
            group,
            user,
        })
    }
}

// sudo -e [-ABkNnS] [-r role] [-t type] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R directory] [-T timeout] [-u user] file ...
#[allow(dead_code)]
pub struct SudoEditOptions {
    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
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

        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let chdir = mem::take(&mut opts.chdir);
        let group = mem::take(&mut opts.group);
        let user = mem::take(&mut opts.user);
        let positional_args = mem::take(&mut opts.positional_args);

        reject_all("--edit", opts)?;

        if positional_args.is_empty() {
            return Err("must specify at least one file path".into());
        }

        Ok(Self {
            reset_timestamp,
            non_interactive,
            stdin,
            chdir,
            group,
            user,
            positional_args,
        })
    }
}

// sudo -l [-ABkNnS] [-g group] [-h host] [-p prompt] [-U user] [-u user] [command [arg ...]]
pub struct SudoListOptions {
    // -l OR -l -l
    pub list: List,

    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
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
        let list = opts.list.take().unwrap();
        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let group = mem::take(&mut opts.group);
        let other_user = mem::take(&mut opts.other_user);
        let user = mem::take(&mut opts.user);
        let positional_args = mem::take(&mut opts.positional_args);

        // when present, `-u` must be accompanied by a command
        let has_command = !positional_args.is_empty();
        let valid_user_flag = user.is_none() || has_command;

        if !valid_user_flag {
            return Err("'--user' flag must be accompanied by a command".into());
        }

        reject_all("--list", opts)?;

        Ok(Self {
            list,
            reset_timestamp,
            non_interactive,
            stdin,
            group,
            other_user,
            user,
            positional_args,
        })
    }
}

// sudo [-ABbEHnPS] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R directory] [-T timeout] [-u user] [VAR=value] [-i | -s] [command [arg ...]]
pub struct SudoRunOptions {
    // -E
    pub preserve_env: PreserveEnv,
    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
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
        let preserve_env = mem::take(&mut opts.preserve_env);
        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let chdir = mem::take(&mut opts.chdir);
        let group = mem::take(&mut opts.group);
        let user = mem::take(&mut opts.user);
        let env_var_list = mem::take(&mut opts.env_var_list);
        let login = mem::take(&mut opts.login);
        let shell = mem::take(&mut opts.shell);
        let positional_args = mem::take(&mut opts.positional_args);

        let context = match (login, shell, positional_args.is_empty()) {
            (true, false, _) => "--login",
            (false, true, _) => "--shell",
            (false, false, false) => "command (positional argument)",

            (true, true, _) => return Err("--login conflicts with --shell".into()),
            (false, false, true) => {
                if cfg!(debug_assertions) {
                    // see `SudoOptions::validate`
                    panic!();
                } else {
                    return Err(
                        "expected one of: --login, --shell, a command as a positional argument"
                            .into(),
                    );
                }
            }
        };

        reject_all(context, opts)?;

        Ok(Self {
            preserve_env,
            reset_timestamp,
            non_interactive,
            stdin,
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
    preserve_env: PreserveEnv,
    // -s
    shell: bool,
    // -S
    stdin: bool,
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

    // arguments passed straight through, either seperated by -- or just trailing.
    positional_args: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum PreserveEnv {
    #[default]
    Nothing,
    Everything,
    Only(Vec<String>),
}

impl PreserveEnv {
    #[cfg(test)]
    pub fn try_into_only(self) -> Result<Vec<String>, Self> {
        if let Self::Only(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn is_nothing(&self) -> bool {
        matches!(self, Self::Nothing)
    }
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
    const TAKES_ARGUMENT_SHORT: &'static [char] = &['D', 'g', 'h', 'R', 'U', 'u'];
    const TAKES_ARGUMENT: &'static [&'static str] =
        &["chdir", "group", "host", "chroot", "other-user", "user"];

    /// argument assignments and shorthand options preprocessing
    fn normalize_arguments<I>(iter: I) -> Result<Vec<Self>, String>
    where
        I: IntoIterator<Item = String>,
    {
        // the first argument is the sudo command - so we can skip it
        let mut arg_iter = iter.into_iter().skip(1);
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
                        Err(format!("'{}' does not take any arguments", key))?;
                    }
                    processed.push(SudoArg::Argument("--".to_string() + key, value.to_string()));
                } else if Self::TAKES_ARGUMENT.contains(&unprefixed) {
                    if let Some(next) = arg_iter.next() {
                        processed.push(SudoArg::Argument(arg, next));
                    } else {
                        Err(format!("'{}' expects an argument", &arg))?;
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
                            Err("invalid option '='")?;
                        }
                        if next.is_some() {
                            processed.push(SudoArg::Argument(flag, rest.to_string()));
                        } else if let Some(next) = arg_iter.next() {
                            processed.push(SudoArg::Argument(flag, next));
                        } else if curr == 'h' {
                            // short version of --help has no arguments
                            processed.push(SudoArg::Flag(flag));
                        } else {
                            Err(format!("'-{}' expects an argument", curr))?;
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
                return Err("expected one of these actions: --help, --version, --remove-timestamp, --validate, --list, --edit, --login, --shell, a command as a positional argument, --reset-timestamp".into());
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
        let mut options = Self::default();
        let arg_iter = SudoArg::normalize_arguments(iter.into_iter().map(Into::into))?
            .into_iter()
            .peekable();

        for arg in arg_iter {
            match arg {
                SudoArg::Flag(flag) => match flag.as_str() {
                    "-E" | "--preserve-env" => {
                        options.preserve_env = PreserveEnv::Everything;
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
                        Err("invalid option provided")?;
                    }
                },
                SudoArg::Argument(option, value) => match option.as_str() {
                    "-D" | "--chdir" => {
                        options.chdir = Some(SudoPath::from_cli_string(value));
                    }
                    "-E" | "--preserve-env" => {
                        let split_value = || value.split(',').map(str::to_string);
                        match &mut options.preserve_env {
                            PreserveEnv::Nothing => {
                                options.preserve_env = PreserveEnv::Only(split_value().collect())
                            }
                            PreserveEnv::Everything => {}
                            PreserveEnv::Only(list) => list.extend(split_value()),
                        }
                        // options.preserve_env = value.split(',').map(str::to_string).collect()
                    }
                    "-g" | "--group" => {
                        options.group = Some(SudoString::from_cli_string(value));
                    }
                    "-U" | "--other-user" => {
                        options.other_user = Some(SudoString::from_cli_string(value));
                    }
                    "-u" | "--user" => {
                        options.user = Some(SudoString::from_cli_string(value));
                    }
                    _option => {
                        Err("invalid option provided")?;
                    }
                },
                SudoArg::Environment(key, value) => {
                    options.env_var_list.push((key, value));
                }
                SudoArg::Rest(rest) => {
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

impl IsAbsent for PreserveEnv {
    fn is_absent(&self) -> bool {
        self.is_nothing()
    }
}

fn ensure_is_absent(context: &str, thing: &dyn IsAbsent, name: &str) -> Result<(), String> {
    if thing.is_absent() {
        Ok(())
    } else {
        Err(format!("{context} conflicts with {name}"))
    }
}

fn reject_all(context: &str, opts: SudoOptions) -> Result<(), String> {
    macro_rules! tuple {
        ($expr:expr) => {
            (&$expr as &dyn IsAbsent, {
                let name = concat!("--", stringify!($expr));
                if name.contains('_') {
                    Cow::Owned(name.replace('_', "-"))
                } else {
                    Cow::Borrowed(name)
                }
            })
        };
    }

    let SudoOptions {
        chdir,
        group,
        login,
        non_interactive,
        other_user,
        preserve_env,
        shell,
        stdin,
        user,
        env_var_list,
        edit,
        help,
        list,
        remove_timestamp,
        reset_timestamp,
        validate,
        version,
        positional_args,
    } = opts;

    let flags = [
        tuple!(chdir),
        tuple!(edit),
        tuple!(group),
        tuple!(help),
        tuple!(list),
        tuple!(login),
        tuple!(non_interactive),
        tuple!(other_user),
        tuple!(preserve_env),
        tuple!(remove_timestamp),
        tuple!(reset_timestamp),
        tuple!(shell),
        tuple!(stdin),
        tuple!(user),
        tuple!(validate),
        tuple!(version),
    ];
    for (value, name) in flags {
        ensure_is_absent(context, value, &name)?;
    }

    ensure_is_absent(context, &env_var_list, "environment variable")?;
    ensure_is_absent(context, &positional_args, "positional argument")?;

    Ok(())
}

impl From<SudoListOptions> for OptionsForContext {
    fn from(opts: SudoListOptions) -> Self {
        let SudoListOptions {
            group,
            non_interactive,
            positional_args,
            reset_timestamp,
            stdin,
            user,

            list: _,
            other_user: _,
        } = opts;

        Self {
            action: ContextAction::List,

            group,
            non_interactive,
            positional_args,
            reset_timestamp,
            stdin,
            user,

            chdir: None,
            login: false,
            shell: false,
        }
    }
}

impl From<SudoValidateOptions> for OptionsForContext {
    fn from(opts: SudoValidateOptions) -> Self {
        let SudoValidateOptions {
            group,
            non_interactive,
            reset_timestamp,
            stdin,
            user,
        } = opts;

        Self {
            action: ContextAction::Validate,

            group,
            non_interactive,
            reset_timestamp,
            stdin,
            user,

            chdir: None,
            login: false,
            positional_args: vec![],
            shell: false,
        }
    }
}

impl From<SudoRunOptions> for OptionsForContext {
    fn from(opts: SudoRunOptions) -> Self {
        let SudoRunOptions {
            chdir,
            group,
            login,
            non_interactive,
            positional_args,
            reset_timestamp,
            shell,
            stdin,
            user,

            env_var_list: _,
            preserve_env: _,
        } = opts;

        Self {
            action: ContextAction::Run,

            chdir,
            group,
            login,
            non_interactive,
            positional_args,
            reset_timestamp,
            shell,
            stdin,
            user,
        }
    }
}
