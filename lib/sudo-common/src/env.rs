use crate::context::{CommandAndArguments, Context};
use std::collections::HashMap;

pub type Environment = HashMap<String, String>;

const MAIL_DIR: &str = "/var/mail/";

const CHECK_ENV_TABLE: &[&str] = &[
    "COLORTERM",
    "LANG",
    "LANGUAGE",
    "LC_*",
    "LINGUAS",
    "TERM",
    "TZ",
];

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
    let mut extra_env = environment_from_list(vec![
        ("SUDO_COMMAND", format_command(&context.command)),
        ("SUDO_UID", context.current_user.uid.to_string()),
        ("SUDO_GID", context.current_user.gid.to_string()),
        ("SUDO_USER", context.current_user.name.clone()),
        ("SHELL", context.target_user.shell.clone()),
        // TODO: Set to the login name of the target user when the -i option is specified,
        // when the set_logname option is enabled in sudoers, or when the env_reset option
        // is enabled in sudoers (unless LOGNAME is present in the env_keep list).
        ("LOGNAME", context.target_user.name.clone()),
        ("USER", context.target_user.name.clone()),
    ]);

    // TODO: check home dir config + options
    if !context.preserve_env {
        extra_env.insert("HOME".to_string(), context.target_user.home.clone());
        extra_env.insert(
            "MAIL".to_string(),
            format!("{MAIL_DIR}{}", context.target_user.name),
        );
    }

    extra_env
}

fn filter_env(preserve_env_list: Vec<&str>, environment: &Environment) -> Environment {
    let mut filtered_env = Environment::new();

    for name_pattern in preserve_env_list {
        if let Ok(pattern) = glob::Pattern::new(name_pattern) {
            for (name, value) in environment {
                if pattern.matches(name) {
                    filtered_env.insert(name.to_string(), value.to_string());
                }
            }
        }
    }

    filtered_env
}

/// Construct the final environment from the current one and a sudo context
/// see https://github.com/sudo-project/sudo/blob/main/plugins/sudoers/env.c for the original implementation
pub fn get_target_environment(current_env: Environment, context: &Context) -> Environment {
    let mut result = Environment::new();

    if !context.preserve_env_list.is_empty() {
        let preserve_env_list = context
            .preserve_env_list
            .iter()
            .map(|s| s.as_ref())
            .collect();

        result.extend(filter_env(preserve_env_list, &current_env));
        result.extend(filter_env(CHECK_ENV_TABLE.to_vec(), &current_env));
        result.extend(filter_env(KEEP_ENV_TABLE.to_vec(), &current_env));
    } else if context.preserve_env {
        result.extend(current_env);
    } else {
        // TODO filter CHECK_ENV_TABLE list
        result.extend(filter_env(CHECK_ENV_TABLE.to_vec(), &current_env));
        result.extend(filter_env(KEEP_ENV_TABLE.to_vec(), &current_env));
    }

    let sudo_env = get_extra_env(context);
    result.extend(sudo_env);

    result
}
