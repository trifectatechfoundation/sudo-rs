use std::{
    collections::{hash_map::Entry, HashSet},
    ffi::{OsStr, OsString},
    os::unix::prelude::OsStrExt,
};

use crate::common::{CommandAndArguments, Context, Environment};
use crate::sudoers::Policy;
use crate::system::PATH_MAX;

use super::wildcard_match::wildcard_match;

const PATH_MAILDIR: &str = env!("PATH_MAILDIR");
const PATH_ZONEINFO: &str = env!("PATH_ZONEINFO");
const PATH_DEFAULT: &str = env!("SUDO_PATH_DEFAULT");

/// check byte slice contains with given byte slice
fn contains_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Formats the command and arguments passed for the SUDO_COMMAND
/// environment variable. Limit the length to 4096 bytes to prevent
/// execve failure for very long argument vectors
fn format_command(command_and_arguments: &CommandAndArguments) -> OsString {
    let mut formatted: OsString = command_and_arguments.command.clone().into();

    for arg in &command_and_arguments.arguments {
        if formatted.len() + arg.len() < 4096 {
            formatted.push(" ");
            formatted.push(arg);
        }
    }

    formatted
}

/// Construct sudo-specific environment variables
fn add_extra_env(
    context: &Context,
    cfg: &impl Policy,
    sudo_ps1: Option<OsString>,
    environment: &mut Environment,
) {
    // current user
    environment.insert("SUDO_COMMAND".into(), format_command(&context.command));
    environment.insert(
        "SUDO_UID".into(),
        context.current_user.uid.to_string().into(),
    );
    environment.insert(
        "SUDO_GID".into(),
        context.current_user.gid.to_string().into(),
    );
    environment.insert("SUDO_USER".into(), context.current_user.name.clone().into());
    // target user
    if let Entry::Vacant(entry) = environment.entry("MAIL".into()) {
        entry.insert(format!("{PATH_MAILDIR}/{}", context.target_user.name).into());
    }
    // The current SHELL variable should determine the shell to run when -s is passed, if none set use passwd entry
    environment.insert("SHELL".into(), context.target_user.shell.clone().into());
    // HOME' Set to the home directory of the target user if -i or -H are specified, env_reset or always_set_home are
    // set in sudoers, or when the -s option is specified and set_home is set in sudoers.
    // Since we always want to do env_reset -> always set HOME
    if let Entry::Vacant(entry) = environment.entry("HOME".into()) {
        entry.insert(context.target_user.home.clone().into());
    }

    match (
        environment.get(OsStr::new("LOGNAME")),
        environment.get(OsStr::new("USER")),
    ) {
        // Set to the login name of the target user when the -i option is specified,
        // when the set_logname option is enabled in sudoers, or when the env_reset option
        // is enabled in sudoers (unless LOGNAME is present in the env_keep list).
        // Since we always want to do env_reset -> always set these except when present in env
        (None, None) => {
            environment.insert("LOGNAME".into(), context.target_user.name.clone().into());
            environment.insert("USER".into(), context.target_user.name.clone().into());
        }
        // LOGNAME should be set to the same value as USER if the latter is preserved.
        (None, Some(user)) => {
            environment.insert("LOGNAME".into(), user.clone());
        }
        // USER should be set to the same value as LOGNAME if the latter is preserved.
        (Some(logname), None) => {
            environment.insert("USER".into(), logname.clone());
        }
        (Some(_), Some(_)) => {}
    }

    // Overwrite PATH when secure_path is set
    if let Some(secure_path) = cfg.secure_path() {
        // assign path by env path or secure_path configuration
        environment.insert("PATH".into(), secure_path.into());
    }
    // If the PATH and TERM variables are not preserved from the user's environment, they will be set to default value
    if !environment.contains_key(OsStr::new("PATH")) {
        // If the PATH variable is not set, it will be set to default value
        environment.insert("PATH".into(), PATH_DEFAULT.into());
    }
    // If the TERM variable is not preserved from the user's environment, it will be set to default value
    if !environment.contains_key(OsStr::new("TERM")) {
        environment.insert("TERM".into(), "unknown".into());
    }
    // The SUDO_PS1 variable requires special treatment as the PS1 variable must be set in the
    // target environment to the same value of SUDO_PS1 if the latter is set.
    if let Some(sudo_ps1_value) = sudo_ps1 {
        // set PS1 to the SUDO_PS1 value in the target environment
        environment.insert("PS1".into(), sudo_ps1_value);
    }
}

/// Check a string only contains printable (non-space) characters
fn is_printable(input: &[u8]) -> bool {
    input
        .iter()
        .all(|c| c.is_ascii_alphanumeric() || c.is_ascii_punctuation())
}

/// The TZ variable is considered unsafe if any of the following are true:
/// It consists of a fully-qualified path name, optionally prefixed with a colon (‘:’), that does not match the location of the zoneinfo directory.
/// It contains a .. path element.
/// It contains white space or non-printable characters.
/// It is longer than the value of PATH_MAX.
fn is_safe_tz(value: &[u8]) -> bool {
    let check_value = if value.starts_with(&[b':']) {
        &value[1..]
    } else {
        value
    };

    if check_value.starts_with(&[b'/']) {
        // clippy 1.79 wants to us to optimise this check away; but we don't know what this will always
        // be possible; and the compiler is clever enough to do that for us anyway if it can be.
        #[allow(clippy::const_is_empty)]
        if !PATH_ZONEINFO.is_empty() {
            if !check_value.starts_with(PATH_ZONEINFO.as_bytes())
                || check_value.get(PATH_ZONEINFO.len()) != Some(&b'/')
            {
                return false;
            }
        } else {
            return false;
        }
    }

    !contains_subsequence(check_value, "..".as_bytes())
        && is_printable(check_value)
        && check_value.len() < PATH_MAX as usize
}

/// Check whether the needle exists in a haystack, in which the haystack is a list of patterns, possibly containing wildcards
fn in_table(needle: &OsStr, haystack: &HashSet<String>) -> bool {
    haystack
        .iter()
        .any(|pattern| wildcard_match(needle.as_bytes(), pattern.as_bytes()))
}

/// Determine whether a specific environment variable should be kept
fn should_keep(key: &OsStr, value: &OsStr, cfg: &impl Policy) -> bool {
    if value.as_bytes().starts_with("()".as_bytes()) {
        return false;
    }

    if key == "TZ" {
        return in_table(key, cfg.env_keep())
            || (in_table(key, cfg.env_check()) && is_safe_tz(value.as_bytes()));
    }

    if in_table(key, cfg.env_check()) {
        return !value.as_bytes().iter().any(|c| *c == b'%' || *c == b'/');
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
    additional_env: Environment,
    context: &Context,
    settings: &impl Policy,
) -> Environment {
    let mut environment = Environment::default();

    // retrieve SUDO_PS1 value to set a PS1 value as additional environment
    let sudo_ps1 = current_env.get(OsStr::new("SUDO_PS1")).cloned();

    // variables preserved from the invoking user's environment by the
    // env_keep list take precedence over those in the PAM environment
    environment.extend(additional_env);

    environment.extend(
        current_env
            .into_iter()
            .filter(|(key, value)| should_keep(key, value, settings)),
    );

    add_extra_env(context, settings, sudo_ps1, &mut environment);

    environment
}

#[cfg(test)]
mod tests {
    use super::{is_safe_tz, should_keep, PATH_ZONEINFO};
    use crate::sudoers::Policy;
    use std::{collections::HashSet, ffi::OsStr};

    struct TestConfiguration {
        keep: HashSet<String>,
        check: HashSet<String>,
    }

    impl Policy for TestConfiguration {
        fn env_keep(&self) -> &HashSet<String> {
            &self.keep
        }

        fn env_check(&self) -> &HashSet<String> {
            &self.check
        }

        fn secure_path(&self) -> Option<String> {
            None
        }

        fn use_pty(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_filtering() {
        let config = TestConfiguration {
            keep: HashSet::from(["AAP".to_string(), "NOOT".to_string()]),
            check: HashSet::from(["MIES".to_string(), "TZ".to_string()]),
        };

        let check_should_keep = |key: &str, value: &str, expected: bool| {
            assert_eq!(
                should_keep(OsStr::new(key), OsStr::new(value), &config),
                expected,
                "{} should {}",
                key,
                if expected { "be kept" } else { "not be kept" }
            );
        };

        check_should_keep("AAP", "FOO", true);
        check_should_keep("MIES", "BAR", true);
        check_should_keep("AAP", "()=foo", false);
        check_should_keep("TZ", "Europe/Amsterdam", true);
        check_should_keep("TZ", "../Europe/Berlin", false);
        check_should_keep("MIES", "FOO/BAR", false);
        check_should_keep("MIES", "FOO%", false);
    }

    #[allow(clippy::useless_format)]
    #[allow(clippy::bool_assert_comparison)]
    #[test]
    fn test_tzinfo() {
        assert_eq!(is_safe_tz("Europe/Amsterdam".as_bytes()), true);
        assert_eq!(
            is_safe_tz(format!("{PATH_ZONEINFO}/Europe/London").as_bytes()),
            true
        );
        assert_eq!(
            is_safe_tz(format!(":{PATH_ZONEINFO}/Europe/Amsterdam").as_bytes()),
            true
        );
        assert_eq!(
            is_safe_tz(format!("/schaap/Europe/Amsterdam").as_bytes()),
            false
        );
        assert_eq!(
            is_safe_tz(format!("{PATH_ZONEINFO}/../Europe/London").as_bytes()),
            false
        );
        assert_eq!(
            is_safe_tz(format!("{PATH_ZONEINFO}/../Europe/London").as_bytes()),
            false
        );
    }
}
