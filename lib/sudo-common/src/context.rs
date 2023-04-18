use std::path::PathBuf;
use sudo_cli::SudoOptions;
use sudo_system::{hostname, Group, Process, User};

use crate::{
    command::CommandAndArguments,
    resolve::{resolve_current_user, resolve_launch_and_shell, resolve_target_user_and_group},
    Error,
};

#[derive(Debug)]
pub struct Context {
    // cli options
    pub preserve_env_list: Vec<String>,
    pub set_home: bool,
    pub launch: LaunchType,
    pub chdir: Option<PathBuf>,
    pub command: CommandAndArguments,
    pub target_user: User,
    pub target_group: Group,
    pub stdin: bool,
    // system
    pub hostname: String,
    pub path: String,
    pub current_user: User,
    pub process: Process,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LaunchType {
    Direct,
    Shell,
    Login,
}

impl Context {
    pub fn build_from_options(sudo_options: SudoOptions, path: String) -> Result<Context, Error> {
        let hostname = hostname();
        let current_user = resolve_current_user()?;
        let (target_user, target_group) =
            resolve_target_user_and_group(&sudo_options.user, &sudo_options.group, &current_user)?;
        let (launch, shell) = resolve_launch_and_shell(&sudo_options, &current_user, &target_user);
        let command = CommandAndArguments::try_from_args(shell, sudo_options.external_args, &path)?;

        Ok(Context {
            hostname,
            path,
            command,
            current_user,
            target_user,
            target_group,
            set_home: sudo_options.set_home,
            preserve_env_list: sudo_options.preserve_env_list,
            launch,
            chdir: sudo_options.directory,
            stdin: sudo_options.stdin,
            process: sudo_system::Process::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use sudo_cli::SudoOptions;

    use super::Context;

    #[test]
    fn test_build_context() {
        let options = SudoOptions::try_parse_from(["sudo", "echo", "hello"]).unwrap();
        let path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
        let context = Context::build_from_options(options, path.to_string()).unwrap();

        let mut target_environment = HashMap::new();
        target_environment.insert("SUDO_USER".to_string(), context.current_user.name.clone());

        assert_eq!(context.command.command.to_str().unwrap(), "/usr/bin/echo");
        assert_eq!(context.command.arguments, ["hello"]);
        assert_eq!(context.hostname, sudo_system::hostname());
        assert_eq!(context.target_user.uid, 0);
    }
}
