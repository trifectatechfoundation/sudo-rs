use std::collections::HashSet;

use sudo_common::context::{CommandAndArguments, Configuration, Context, Environment};
use sudo_system::PATH_MAX;

use crate::wildcard_match::wildcard_match;

const PATH_MAILDIR: &str = env!("PATH_MAILDIR");
const PATH_ZONEINFO: &str = env!("PATH_ZONEINFO");
const PATH_DEFAULT: &str = env!("PATH_DEFAULT");

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
fn add_extra_env(context: &Context, environment: &mut Environment) {
    // current user
    environment.insert("SUDO_COMMAND".to_string(), format_command(&context.command));
    environment.insert("SUDO_UID".to_string(), context.current_user.uid.to_string());
    environment.insert("SUDO_GID".to_string(), context.current_user.gid.to_string());
    environment.insert("SUDO_USER".to_string(), context.current_user.name.clone());
    // target user
    environment.insert(
        "MAIL".to_string(),
        format!("{PATH_MAILDIR}/{}", context.target_user.name),
    );
    // The current SHELL variable should determine the shell to run when -s is passed, if none set use passwd entry
    environment.insert("SHELL".to_string(), context.target_user.shell.clone());
    // HOME' Set to the home directory of the target user if -i or -H are specified, env_reset or always_set_home are
    // set in sudoers, or when the -s option is specified and set_home is set in sudoers.
    // Since we always want to do env_reset -> always set HOME
    environment.insert("HOME".to_string(), context.target_user.home.clone());
    // Set to the login name of the target user when the -i option is specified,
    // when the set_logname option is enabled in sudoers, or when the env_reset option
    // is enabled in sudoers (unless LOGNAME is present in the env_keep list).
    // Since we always want to do env_reset -> always set these except when present in env
    if !environment.contains_key("LOGNAME") && !environment.contains_key("USER") {
        environment.insert("LOGNAME".to_string(), context.target_user.name.clone());
        environment.insert("USER".to_string(), context.target_user.name.clone());
    }
    // If the PATH and TERM variables are not preserved from the user's environment, they will be set to default value
    if !environment.contains_key("PATH") {
        environment.insert("PATH".to_string(), PATH_DEFAULT.to_string());
    }
    if !environment.contains_key("TERM") {
        environment.insert("TERM".to_string(), "unknown".to_string());
    }
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
/// If the PATH and TERM variables are not preserved from the user's environment, they will be set to default value
///
/// Environment variables with a value beginning with ‘()’ are removed
pub fn get_target_environment(
    current_env: Environment,
    context: &Context,
    settings: &impl Configuration,
) -> Environment {
    let mut environment = current_env
        .into_iter()
        .filter(|(key, value)| should_keep(key, value, settings))
        .collect();

    add_extra_env(context, &mut environment);

    environment
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use sudo_common::context::Configuration;

    use crate::environment::{is_safe_tz, should_keep, PATH_ZONEINFO};

    struct TestConfiguration {
        keep: HashSet<String>,
        check: HashSet<String>,
    }

    impl Configuration for TestConfiguration {
        fn env_keep(&self) -> &HashSet<String> {
            &self.keep
        }

        fn env_check(&self) -> &HashSet<String> {
            &self.check
        }
    }

    #[test]
    fn test_filtering() {
        let config = TestConfiguration {
            keep: HashSet::from(["AAP".to_string(), "NOOT".to_string(), "TZ".to_string()]),
            check: HashSet::from(["MIES".to_string()]),
        };

        assert_eq!(should_keep("AAP", "FOO", &config), true);
        assert_eq!(should_keep("MIES", "BAR", &config), true);
        assert_eq!(should_keep("AAP", "()=foo", &config), false);
        assert_eq!(should_keep("TZ", "Europe/Amsterdam", &config), true);
        assert_eq!(should_keep("TZ", "../Europe/Berlin", &config), false);
        assert_eq!(should_keep("MIES", "FOO/BAR", &config), false);
        assert_eq!(should_keep("MIES", "FOO%", &config), false);
    }

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
        assert_eq!(
            is_safe_tz(format!("{PATH_ZONEINFO}/../Europe/London").as_str()),
            false
        );
    }
}
