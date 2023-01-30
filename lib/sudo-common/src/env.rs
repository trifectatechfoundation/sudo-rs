use std::{
    collections::HashMap,
    env,
    ffi::{OsStr, OsString},
};
use sudo_system::User;

pub type Environment = HashMap<OsString, OsString>;

pub fn environment_from_list<K: Into<OsString>, V: Into<OsString>>(
    list: Vec<(K, V)>,
) -> Environment {
    list.into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect::<Environment>()
}

pub struct SudoArguments {
    command: String,
    args: Vec<String>,
    preserve_env: bool,
    preserve_env_list: Option<String>,
    set_home: bool,
    target_user: String,
}

/// Formats the command and arguments passed for the SUDO_COMMAND
/// environment variable. Limit the length of arguments to 4096 bytes to prevent
/// execve failure for very long argument vectors
fn format_command(args: &SudoArguments) -> String {
    let mut args_bytes = args.args.join(" ").as_bytes().to_owned();
    args_bytes.truncate(4096);

    format!("{} {}", args.command, String::from_utf8_lossy(&args_bytes))
}

/// Construct sudo-specific environment variables
fn get_extra_env(args: &SudoArguments) -> Environment {
    let user = User::real()
        // TODO: move fetching user and error handling to sudo-rs main create
        .expect("Could not determine real user")
        .expect("Current user not found");

    let mut extra_env = environment_from_list(vec![
        ("SUDO_COMMAND", format_command(args)),
        ("SUDO_UID", user.uid.to_string()),
        ("SUDO_GID", user.gid.to_string()),
        ("SUDO_USER", user.name),
    ]);

    if args.set_home {
        let target_user = User::from_name(&args.target_user)
            // TODO: move fetching user and error handling to sudo-rs main create
            .expect("Could not determine target user")
            .expect("Target user not found");

        extra_env.insert("HOME".into(), target_user.home.into());
    }

    extra_env
}

fn filter_env(preserve_env_list: &str, environment: Environment) -> Environment {
    let preserve_env_list = preserve_env_list
        .split(',')
        .map(OsStr::new)
        .collect::<Vec<&OsStr>>();

    let mut filtered_env = environment;
    filtered_env.retain(|k, _| preserve_env_list.contains(&k.as_os_str()));

    filtered_env
}

pub fn get_env_vars(args: &SudoArguments) -> Environment {
    let mut result = Environment::new();
    let current = env::vars_os().collect::<Environment>();

    if args.preserve_env {
        result.extend(current);
    } else if let Some(preserve_env_list) = &args.preserve_env_list {
        let filtered_current_env = filter_env(preserve_env_list, current);
        result.extend(filtered_current_env);
    }

    let sudo_env = get_extra_env(args);
    result.extend(sudo_env);

    result
}

#[cfg(test)]
mod tests {
    use crate::env::{environment_from_list, get_env_vars, SudoArguments};
    use std::env;
    use sudo_system::User;

    use super::Environment;

    fn setup(set_env: Environment) -> SudoArguments {
        env::vars().for_each(|(k, _)| env::remove_var(k));
        set_env.into_iter().for_each(|(k, v)| env::set_var(k, v));

        SudoArguments {
            command: "/usr/bin/echo".to_string(),
            args: vec!["hello".to_string()],
            preserve_env: false,
            preserve_env_list: None,
            set_home: false,
            target_user: "root".to_string(),
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn do_not_preserve_env() {
        let user = User::real().unwrap().unwrap();
        let args = setup(Environment::new());
        let result = get_env_vars(&args);

        assert_eq!(
            result,
            environment_from_list(vec![
                ("SUDO_GID", user.gid.to_string().as_str()),
                ("SUDO_COMMAND", "/usr/bin/echo hello"),
                ("SUDO_USER", &user.name),
                ("SUDO_UID", user.uid.to_string().as_str()),
            ])
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn set_home() {
        let user = User::real().unwrap().unwrap();
        let target_user = User::from_name("root").unwrap().unwrap();
        let mut args = setup(Environment::new());
        args.set_home = true;
        let result = get_env_vars(&args);

        assert_eq!(
            result,
            environment_from_list(vec![
                ("SUDO_GID", user.gid.to_string().as_str()),
                ("SUDO_COMMAND", "/usr/bin/echo hello"),
                ("SUDO_USER", &user.name),
                ("SUDO_UID", user.uid.to_string().as_str()),
                ("HOME", &target_user.home)
            ])
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn truncate_long_argument_list() {
        let user = User::real().unwrap().unwrap();
        let mut args = setup(Environment::new());
        args.args = (1..1000).map(|_| "hello".to_string()).collect();
        let result = get_env_vars(&args);

        let mut truncated = args.args.join(" ");
        truncated.truncate(4096);

        assert_eq!(
            result,
            environment_from_list(vec![
                ("SUDO_GID", user.gid.to_string().as_str()),
                ("SUDO_COMMAND", &format!("/usr/bin/echo {}", truncated)),
                ("SUDO_USER", &user.name),
                ("SUDO_UID", user.uid.to_string().as_str()),
            ])
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn preserve_env() {
        let user = User::real().unwrap().unwrap();
        let mut args = setup(environment_from_list(vec![("FOO", "BAR")]));
        args.preserve_env = true;
        let result = get_env_vars(&args);

        assert_eq!(
            result,
            environment_from_list(vec![
                ("SUDO_GID", user.gid.to_string().as_str()),
                ("SUDO_COMMAND", "/usr/bin/echo hello"),
                ("SUDO_USER", &user.name),
                ("SUDO_UID", user.uid.to_string().as_str()),
                ("FOO", "BAR"),
            ])
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn preserve_env_list() {
        let user = User::real().unwrap().unwrap();
        let mut args = setup(environment_from_list(vec![
            ("FOO", "BAR"),
            ("FERRIS", "CRAB"),
            ("NONO", "NONONO"),
        ]));
        args.preserve_env_list = Some("FOO,FERRIS".to_string());
        let result = get_env_vars(&args);

        assert_eq!(
            result,
            environment_from_list(vec![
                ("SUDO_GID", user.gid.to_string().as_str()),
                ("SUDO_COMMAND", "/usr/bin/echo hello"),
                ("SUDO_USER", &user.name),
                ("SUDO_UID", user.uid.to_string().as_str()),
                ("FOO", "BAR"),
                ("FERRIS", "CRAB"),
            ])
        );
    }
}
