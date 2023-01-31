use std::{
    collections::HashMap,
    env,
    ffi::{OsStr, OsString},
};
use sudo_system::User;

use crate::context::{CommandAndArguments, Context};

pub type Environment = HashMap<OsString, OsString>;

pub fn environment_from_list<K: Into<OsString>, V: Into<OsString>>(
    list: Vec<(K, V)>,
) -> Environment {
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

    format!(
        "{} {}",
        command_and_arguments.command,
        String::from_utf8_lossy(&args_bytes)
    )
}

/// Construct sudo-specific environment variables
fn get_extra_env(context: &Context) -> Environment {
    let user = User::real()
        // TODO: move fetching user and error handling to sudo-rs main create
        .expect("Could not determine real user")
        .expect("Current user not found");

    let mut extra_env = environment_from_list(vec![
        ("SUDO_COMMAND", format_command(&context.command)),
        ("SUDO_UID", user.uid.to_string()),
        ("SUDO_GID", user.gid.to_string()),
        ("SUDO_USER", user.name),
    ]);

    if context.set_home {
        let home: &OsStr = OsStr::new(&context.target_user.home);
        extra_env.insert("HOME".into(), home.into());
    }

    extra_env
}

fn filter_env(preserve_env_list: Vec<&str>, environment: Environment) -> Environment {
    let mut filtered_env = environment;
    filtered_env.retain(|k, _| {
        if let Some(name) = k.to_str() {
            preserve_env_list.contains(&name)
        } else {
            false
        }
    });

    filtered_env
}

pub fn get_target_environment(context: &Context) -> Environment {
    let mut result = Environment::new();
    let current = env::vars_os().collect::<Environment>();

    if context.preserve_env {
        result.extend(current);
    } else if context.preserve_env_list.is_empty() {
        let preserve_env_list = context
            .preserve_env_list
            .iter()
            .map(|s| s.as_ref())
            .collect();

        let filtered_current_env = filter_env(preserve_env_list, current);
        result.extend(filtered_current_env);
    }

    let sudo_env = get_extra_env(context);
    result.extend(sudo_env);

    result
}
