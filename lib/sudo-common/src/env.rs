use crate::{
    context::{CommandAndArguments, Configuration, Context},
    wildcard_match::wildcard_match,
};
use std::collections::{HashMap, HashSet};
use sudo_system::PATH_MAX;

pub type Environment = HashMap<String, String>;

const PATH_MAILDIR: &str = env!("PATH_MAILDIR");
const PATH_ZONEINFO: &str = env!("PATH_ZONEINFO");

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
fn get_extra_env(
    context: &Context,
    settings: &impl Configuration,
) -> impl Iterator<Item = (String, String)> {
    let mut extra = vec![
        ("SUDO_COMMAND", format_command(&context.command)),
        ("SUDO_UID", context.current_user.uid.to_string()),
        ("SUDO_GID", context.current_user.gid.to_string()),
        ("SUDO_USER", context.current_user.name.clone()),
        (
            "MAIL",
            format!("{PATH_MAILDIR}/{}", context.target_user.name),
        ),
    ];

    // TODO: preserve existing when sudo -s
    if !context.shell {
        extra.push(("SHELL", context.target_user.shell.clone()));
    }

    // TODO: Set to the login name of the target user when the -i option is specified,
    // when the set_logname option is enabled in sudoers, or when the env_reset option
    // is enabled in sudoers (unless LOGNAME is present in the env_keep list).
    if context.login {
        extra.push(("LOGNAME", context.target_user.name.clone()));
        extra.push(("USER", context.target_user.name.clone()));
    }

    if settings.always_set_home() || context.set_home {
        extra.push(("HOME", context.target_user.home.clone()));
    }

    extra
        .into_iter()
        .map(|(name, value)| (name.to_string(), value))
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
        if !PATH_ZONEINFO.is_empty() {
            if !check_value.starts_with(PATH_ZONEINFO)
                || check_value.chars().nth(PATH_ZONEINFO.len()) != Some('/')
            {
                return false;
            }
        } else {
            return false;
        }
    }

    !check_value.contains("..")
        && is_printable(check_value)
        && check_value.len() < PATH_MAX as usize
}

/// Check whether the needle exists in a haystack, in which the haystack is a list of patterns, possibly containing wildcards
fn in_table(needle: &str, haystack: &HashSet<String>) -> bool {
    haystack
        .iter()
        .any(|pattern| wildcard_match(needle, pattern))
}

/// Determine whether a specific environment variable should be kept
fn should_keep(key: &str, value: &str, cfg: &impl Configuration) -> bool {
    if value.starts_with("()") {
        return false;
    }

    if key == "TZ" && !is_safe_tz(value) {
        return false;
    }

    if in_table(key, cfg.env_check()) && !value.contains(|c| c == '%' || c == '/') {
        return true;
    }

    in_table(key, cfg.env_keep())
}

/// Construct the final environment from the current one and a sudo context
/// see <https://github.com/sudo-project/sudo/blob/main/plugins/sudoers/env.c> for the original implementation
/// see <https://www.sudo.ws/docs/man/sudoers.man/#Command_environment> for the original documentation
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
pub fn get_target_environment(
    current_env: Environment,
    context: &Context,
    settings: &impl Configuration,
) -> Environment {
    current_env
        .into_iter()
        .filter(|(key, value)| should_keep(key, value, settings))
        .chain(get_extra_env(context, settings))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::env::{is_safe_tz, PATH_ZONEINFO};

    #[test]
    fn test_tzinfo() {
        assert_eq!(is_safe_tz("Europe/Amsterdam"), true);
        assert_eq!(
            is_safe_tz(format!("{PATH_ZONEINFO}/Europe/London").as_str()),
            true
        );
        assert_eq!(
            is_safe_tz(format!(":{PATH_ZONEINFO}/Europe/Amsterdam").as_str()),
            true
        );
        assert_eq!(
            is_safe_tz(format!("/schaap/Europe/Amsterdam").as_str()),
            false
        );
        assert_eq!(
            is_safe_tz(format!("{PATH_ZONEINFO}/../Europe/London").as_str()),
            false
        );
    }
}
