use crate::cli::{SudoAction, SudoOptions};
use crate::system::{hostname, Group, Process, User};
use std::path::PathBuf;

use super::{
    command::CommandAndArguments,
    resolve::{resolve_current_user, resolve_launch_and_shell, resolve_target_user_and_group},
    Error,
};

#[derive(Debug)]
pub struct Context {
    // cli options
    pub launch: LaunchType,
    pub chdir: Option<PathBuf>,
    pub command: CommandAndArguments,
    pub target_user: User,
    pub target_group: Group,
    pub stdin: bool,
    pub non_interactive: bool,
    pub use_session_records: bool,
    // system
    pub hostname: String,
    pub current_user: User,
    pub process: Process,
    // policy
    pub use_pty: bool,
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
        let command = match sudo_options.action {
            SudoAction::Run(args) | SudoAction::List(args) => {
                CommandAndArguments::build_from_args(shell, args, &path)
            }
            _ => Default::default(),
        };

        Ok(Context {
            hostname,
            command,
            current_user,
            target_user,
            target_group,
            use_session_records: !sudo_options.reset_timestamp,
            launch,
            chdir: sudo_options.directory,
            stdin: sudo_options.stdin,
            non_interactive: sudo_options.non_interactive,
            process: Process::new(),
            use_pty: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{cli::SudoOptions, system::hostname};
    use std::collections::HashMap;

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
        assert_eq!(context.hostname, hostname());
        assert_eq!(context.target_user.uid, 0);
    }
}
