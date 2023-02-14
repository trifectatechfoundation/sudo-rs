use crate::{
    context::{CommandAndArguments, Context},
    wildcard_match::wildcard_match,
};
use std::collections::HashMap;
use sudo_system::PATH_MAX;

include!(concat!(env!("OUT_DIR"), "/paths.rs"));

pub type Environment = HashMap<String, String>;

/// Remove if these environment variables if the value contains '/' or '%'
const CHECK_ENV_TABLE: &[&str] = &[
    "COLORTERM",
    "LANG",
    "LANGUAGE",
    "LC_*",
    "LINGUAS",
    "TERM",
    "TZ",
];

/// Keep these environment variables by default
const KEEP_ENV_TABLE: &[&str] = &[
    "COLORS",
    "DISPLAY",
    "HOSTNAME",
    "KRB5CCNAME",
    "LS_COLORS",
    "PATH",
    "PS1",
    "PS2",
    "XAUTHORITY",
    "XAUTHORIZATION",
    "XDG_CURRENT_DESKTOP",
];

/// Convert a list of Into<String> key value pars to an Environment
pub fn environment_from_list<K: Into<String>, V: Into<String>>(list: Vec<(K, V)>) -> Environment {
    list.into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect::<Environment>()
}

/// Formats the command and arguments passed for the SUDO_COMMAND
/// environment variable. Limit the length of arguments to 4096 bytes to prevent
/// execve failure for very long argument vectors
fn format_command(command_and_arguments: &CommandAndArguments) -> String {
    let mut args_bytes = command_and_arguments
        .arguments
        .join(" ")
        .as_bytes()
        .to_owned();

    args_bytes.truncate(4096);

    [
        command_and_arguments.command.to_string_lossy(),
        String::from_utf8_lossy(&args_bytes),
    ]
    .join(" ")
    .trim()
    .to_string()
}

/// Construct sudo-specific environment variables
fn get_extra_env(context: &Context) -> Environment {
    environment_from_list(vec![
        ("SUDO_COMMAND", format_command(&context.command)),
        ("SUDO_UID", context.current_user.uid.to_string()),
        ("SUDO_GID", context.current_user.gid.to_string()),
        ("SUDO_USER", context.current_user.name.clone()),
        // TODO: preserve exsisting when sudo -s
        ("SHELL", context.target_user.shell.clone()),
        // TODO: Set to the login name of the target user when the -i option is specified,
        // when the set_logname option is enabled in sudoers, or when the env_reset option
        // is enabled in sudoers (unless LOGNAME is present in the env_keep list).
        ("LOGNAME", context.target_user.name.clone()),
        ("USER", context.target_user.name.clone()),
        // TODO: check home dir config + options
        ("HOME", context.target_user.home.clone()),
        (
            "MAIL",
            format!("{PATH_MAILDIR}/{}", context.target_user.name),
        ),
    ])
}

/// Check a string only contains printable (non-space) characters
fn is_printable(input: &str) -> bool {
    input
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c.is_ascii_punctuation())
}

/// The TZ variable is considered unsafe if any of the following are true:
/// It consists of a fully-qualified path name, optionally prefixed with a colon (‘:’), that does not match the location of the zoneinfo directory.
/// It contains a .. path element.
/// It contains white space or non-printable characters.
/// It is longer than the value of PATH_MAX.
fn is_safe_tz(value: &str) -> bool {
    let check_value = value.trim_start_matches(':');

    if check_value.starts_with('/') {
        if let Some(path) = PATH_ZONEINFO {
            if !check_value.starts_with(path)
                || check_value.chars().nth(path.len() + 1) != Some('/')
            {
                return false;
            }
        } else {
            return false;
        }
    }

    !check_value.contains("..")
        && !is_printable(check_value)
        && check_value.len() < PATH_MAX as usize
}

/// Check whether the needle exists in a haystack, in which the haystack is a list of patterns, possibly containing wildcards
fn in_table(needle: &str, haystack: &[&str]) -> bool {
    haystack
        .iter()
        .any(|pattern| wildcard_match(needle, pattern))
}

/// Determine whether a specific environment variable should be kept
fn should_keep(key: &str, value: &str, check_env: &[&str], keep_env: &[&str]) -> bool {
    if value.starts_with("()") {
        return false;
    }

    if key == "TZ" && !is_safe_tz(value) {
        return false;
    }

    if in_table(key, check_env) && !value.contains(|c| c == '%' || c == '/') {
        return true;
    }

    in_table(key, keep_env)
}

/// Construct the final environment from the current one and a sudo context
/// see https://github.com/sudo-project/sudo/blob/main/plugins/sudoers/env.c for the original implementation
/// see https://www.sudo.ws/docs/man/sudoers.man/#Command_environment for the original documentation
///
/// The HOME, MAIL, SHELL, LOGNAME and USER environment variables are initialized based on the target user
/// and the SUDO_* variables are set based on the invoking user.
///
/// Additional variables, such as DISPLAY, PATH and TERM, are preserved from the invoking user's
/// environment if permitted by the env_check, or env_keep options
///
/// TODO: If the PATH and TERM variables are not preserved from the user's environment, they will be set to default value
///
/// Environment variables with a value beginning with ‘()’ are removed
pub fn get_target_environment(current_env: Environment, context: &Context) -> Environment {
    let mut result = Environment::new();

    for (key, value) in current_env.into_iter() {
        if should_keep(&key, &value, CHECK_ENV_TABLE, KEEP_ENV_TABLE) {
            result.insert(key, value);
        }
    }

    result.extend(get_extra_env(context));

    result
}
