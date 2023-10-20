#![forbid(unsafe_code)]

use std::{borrow::Cow, mem, path::PathBuf};

use crate::common::context::{ContextAction, OptionsForContext};

pub mod help;

#[cfg(test)]
mod tests;

pub enum SudoAction {
    Edit(EditOptions),
    Help(HelpOptions),
    List(ListOptions),
    RemoveTimestamp(RemoveTimestampOptions),
    ResetTimestamp(ResetTimestampOptions),
    Run(RunOptions),
    Validate(ValidateOptions),
    Version(VersionOptions),
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
    pub fn try_into_run(self) -> Result<RunOptions, Self> {
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
pub struct HelpOptions {}

impl TryFrom<SudoOptions> for HelpOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        debug_assert!(mem::take(&mut opts.help));

        reject_all("--help", opts)?;

        Ok(Self {})
    }
}

// sudo -h | -K | -k | -V
pub struct VersionOptions {}

impl TryFrom<SudoOptions> for VersionOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        debug_assert!(mem::take(&mut opts.version));

        reject_all("--version", opts)?;

        Ok(Self {})
    }
}

// sudo -h | -K | -k | -V
pub struct RemoveTimestampOptions {}

impl TryFrom<SudoOptions> for RemoveTimestampOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        debug_assert!(mem::take(&mut opts.remove_timestamp));

        reject_all("--remove-timestamp", opts)?;

        Ok(Self {})
    }
}

// sudo -h | -K | -k | -V
pub struct ResetTimestampOptions {}

impl TryFrom<SudoOptions> for ResetTimestampOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        debug_assert!(mem::take(&mut opts.reset_timestamp));

        reject_all("--reset-timestamp", opts)?;

        Ok(Self {})
    }
}

// sudo -v [-ABkNnS] [-g group] [-h host] [-p prompt] [-u user]
pub struct ValidateOptions {
    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
    // -g
    pub group: Option<String>,
    // -h
    pub host: Option<String>,
    // -u
    pub user: Option<String>,
}

impl TryFrom<SudoOptions> for ValidateOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        debug_assert!(mem::take(&mut opts.validate));

        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let group = mem::take(&mut opts.group);
        let host = mem::take(&mut opts.host);
        let user = mem::take(&mut opts.user);

        reject_all("--validate", opts)?;

        Ok(Self {
            reset_timestamp,
            non_interactive,
            stdin,
            group,
            host,
            user,
        })
    }
}

// sudo -e [-ABkNnS] [-r role] [-t type] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R directory] [-T timeout] [-u user] file ...
pub struct EditOptions {
    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
    // -D
    pub chdir: Option<PathBuf>,
    // -g
    pub group: Option<String>,
    // -h
    pub host: Option<String>,
    // -R
    pub chroot: Option<PathBuf>,
    // -u
    pub user: Option<String>,
    pub positional_args: Vec<String>,
}

impl TryFrom<SudoOptions> for EditOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        // see `SudoOptions::validate`
        debug_assert!(mem::take(&mut opts.edit));

        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let chdir = mem::take(&mut opts.chdir);
        let group = mem::take(&mut opts.group);
        let host = mem::take(&mut opts.host);
        let chroot = mem::take(&mut opts.chroot);
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
            host,
            chroot,
            user,
            positional_args,
        })
    }
}

// sudo -l [-ABkNnS] [-g group] [-h host] [-p prompt] [-U user] [-u user] [command [arg ...]]
pub struct ListOptions {
    // -l OR -l -l
    pub list: List,

    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -S
    pub stdin: bool,
    // -g
    pub group: Option<String>,
    // -h
    pub host: Option<String>,
    // -U
    pub other_user: Option<String>,
    // -u
    pub user: Option<String>,

    pub positional_args: Vec<String>,
}

impl TryFrom<SudoOptions> for ListOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        let list = opts.list.take().unwrap();
        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let stdin = mem::take(&mut opts.stdin);
        let group = mem::take(&mut opts.group);
        let host = mem::take(&mut opts.host);
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
            host,
            other_user,
            user,
            positional_args,
        })
    }
}

// sudo [-ABbEHnPS] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R directory] [-T timeout] [-u user] [VAR=value] [-i | -s] [command [arg ...]]
pub struct RunOptions {
    // -b
    pub background: bool,
    // -E
    pub preserve_env: Vec<String>,
    // -k
    pub reset_timestamp: bool,
    // -n
    pub non_interactive: bool,
    // -P
    pub preserve_groups: bool,
    // -S
    pub stdin: bool,
    // -D
    pub chdir: Option<PathBuf>,
    // -g
    pub group: Option<String>,
    // -h
    pub host: Option<String>,
    // -R
    pub chroot: Option<PathBuf>,
    // -u
    pub user: Option<String>,
    // VAR=value
    pub env_var_list: Vec<(String, String)>,
    // -i
    pub login: bool,
    // -s
    pub shell: bool,
    pub positional_args: Vec<String>,
}

impl TryFrom<SudoOptions> for RunOptions {
    type Error = String;

    fn try_from(mut opts: SudoOptions) -> Result<Self, Self::Error> {
        let background = mem::take(&mut opts.background);
        let preserve_env = mem::take(&mut opts.preserve_env);
        let reset_timestamp = mem::take(&mut opts.reset_timestamp);
        let non_interactive = mem::take(&mut opts.non_interactive);
        let preserve_groups = mem::take(&mut opts.preserve_groups);
        let stdin = mem::take(&mut opts.stdin);
        let chdir = mem::take(&mut opts.chdir);
        let group = mem::take(&mut opts.group);
        let host = mem::take(&mut opts.host);
        let chroot = mem::take(&mut opts.chroot);
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
            background,
            preserve_env,
            reset_timestamp,
            non_interactive,
            preserve_groups,
            stdin,
            chdir,
            group,
            host,
            chroot,
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
    // -b
    background: bool,
    // -R
    chroot: Option<PathBuf>,
    // -D
    chdir: Option<PathBuf>,
    // -g
    group: Option<String>,
    // -h
    host: Option<String>,
    // -i
    login: bool,
    // -n
    non_interactive: bool,
    // -U
    other_user: Option<String>,
    // -E
    preserve_env: Vec<String>,
    // -P
    preserve_groups: bool,
    // -s
    shell: bool,
    // -S
    stdin: bool,
    // -u
    user: Option<String>,

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
    const TAKES_ARGUMENT_SHORT: &'static [char] = &['D', 'E', 'g', 'h', 'R', 'U', 'u'];
    const TAKES_ARGUMENT: &'static [&'static str] = &[
        "chdir",
        "preserve-env",
        "group",
        "host",
        "chroot",
        "other-user",
        "user",
    ];

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
                    if !Self::TAKES_ARGUMENT.contains(&key) {
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
                    "-b" | "--background" => {
                        options.background = true;
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
                        options.chdir = Some(PathBuf::from(value));
                    }
                    "-E" | "--preserve-env" => {
                        options.preserve_env = value.split(',').map(str::to_string).collect()
                    }
                    "-g" | "--group" => {
                        options.group = Some(value);
                    }
                    "-h" | "--host" => {
                        options.host = Some(value);
                    }
                    "-R" | "--chroot" => {
                        options.chroot = Some(PathBuf::from(value));
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
        background,
        chroot,
        chdir,
        group,
        host,
        login,
        non_interactive,
        other_user,
        preserve_env,
        preserve_groups,
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
        tuple!(background),
        tuple!(chdir),
        tuple!(chroot),
        tuple!(edit),
        tuple!(group),
        tuple!(help),
        tuple!(host),
        tuple!(list),
        tuple!(login),
        tuple!(non_interactive),
        tuple!(other_user),
        tuple!(preserve_env),
        tuple!(preserve_groups),
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

impl From<ListOptions> for OptionsForContext {
    fn from(opts: ListOptions) -> Self {
        let ListOptions {
            group,
            non_interactive,
            positional_args,
            reset_timestamp,
            stdin,
            user,

            list: _,
            host: _,
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

impl From<ValidateOptions> for OptionsForContext {
    fn from(opts: ValidateOptions) -> Self {
        let ValidateOptions {
            group,
            non_interactive,
            reset_timestamp,
            stdin,
            user,

            host: _,
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

impl From<RunOptions> for OptionsForContext {
    fn from(opts: RunOptions) -> Self {
        let RunOptions {
            chdir,
            group,
            login,
            non_interactive,
            positional_args,
            reset_timestamp,
            shell,
            stdin,
            user,

            background: _,
            chroot: _,
            env_var_list: _,
            host: _,
            preserve_env: _,
            preserve_groups: _,
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
