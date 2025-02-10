use crate::common::{HARDENED_ENUM_VALUE_0, HARDENED_ENUM_VALUE_1, HARDENED_ENUM_VALUE_2};
use crate::system::{Group, Hostname, Process, User};

use super::resolve::CurrentUser;
use super::{
    command::CommandAndArguments,
    resolve::{resolve_launch_and_shell, resolve_target_user_and_group},
    Error, SudoPath, SudoString,
};

#[derive(Clone, Copy)]
pub enum ContextAction {
    List,
    Run,
    Validate,
}

// this is a bit of a hack to keep the existing `Context` API working
#[derive(Clone)]
pub struct OptionsForContext {
    pub chdir: Option<SudoPath>,
    pub group: Option<SudoString>,
    pub login: bool,
    pub non_interactive: bool,
    pub positional_args: Vec<String>,
    pub reset_timestamp: bool,
    pub shell: bool,
    pub stdin: bool,
    pub user: Option<SudoString>,
    pub action: ContextAction,
}

#[derive(Debug)]
pub struct Context {
    // cli options
    pub launch: LaunchType,
    pub chdir: Option<SudoPath>,
    pub command: CommandAndArguments,
    pub target_user: User,
    pub target_group: Group,
    pub stdin: bool,
    pub non_interactive: bool,
    pub use_session_records: bool,
    // system
    pub hostname: Hostname,
    pub current_user: CurrentUser,
    pub process: Process,
    // policy
    pub use_pty: bool,
    pub password_feedback: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum LaunchType {
    #[default]
    Direct = HARDENED_ENUM_VALUE_0,
    Shell = HARDENED_ENUM_VALUE_1,
    Login = HARDENED_ENUM_VALUE_2,
}

impl Context {
    pub fn build_from_options(sudo_options: OptionsForContext) -> Result<Context, Error> {
        let hostname = Hostname::resolve();
        let current_user = CurrentUser::resolve()?;
        let (target_user, target_group) =
            resolve_target_user_and_group(&sudo_options.user, &sudo_options.group, &current_user)?;

        Ok(Context {
            hostname,
            command: Default::default(),
            current_user,
            target_user,
            target_group,
            use_session_records: !sudo_options.reset_timestamp,
            launch: Default::default(),
            chdir: sudo_options.chdir,
            stdin: sudo_options.stdin,
            non_interactive: sudo_options.non_interactive,
            process: Process::new(),
            use_pty: true,
            password_feedback: false,
        })
    }

    pub fn supply_command(
        self,
        sudo_options: OptionsForContext,
        secure_path: Option<&str>,
    ) -> Result<Context, Error> {
        let (launch, shell) =
            resolve_launch_and_shell(&sudo_options, &self.current_user, &self.target_user);

        let command = match sudo_options.action {
            ContextAction::Run | ContextAction::List
                if !sudo_options.positional_args.is_empty() =>
            {
                let system_path;

                let path = if let Some(path) = secure_path {
                    path
                } else {
                    system_path = std::env::var("PATH").unwrap_or_default();
                    system_path.as_ref()
                };

                CommandAndArguments::build_from_args(shell, sudo_options.positional_args, path)
            }

            // FIXME `Default` is being used as `Option::None`
            _ => Default::default(),
        };

        Ok(Self {
            launch,
            command,
            ..self
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        sudo::SudoAction,
        system::{interface::UserId, Hostname},
    };
    use std::collections::HashMap;

    use super::Context;

    #[test]
    fn test_build_context() {
        let options = SudoAction::try_parse_from(["sudo", "echo", "hello"])
            .unwrap()
            .try_into_run()
            .ok()
            .unwrap();
        let path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
        let (ctx_opts, _pipe_opts) = options.into();
        let context = Context::build_from_options(ctx_opts, Some(path)).unwrap();

        let mut target_environment = HashMap::new();
        target_environment.insert("SUDO_USER".to_string(), context.current_user.name.clone());

        if cfg!(target_os = "linux") {
            // this assumes /bin is a symlink on /usr/bin, like it is on modern Debian/Ubuntu
            assert_eq!(context.command.command.to_str().unwrap(), "/usr/bin/echo");
        } else {
            assert_eq!(context.command.command.to_str().unwrap(), "/bin/echo");
        }
        assert_eq!(context.command.arguments, ["hello"]);
        assert_eq!(context.hostname, Hostname::resolve());
        assert_eq!(context.target_user.uid, UserId::ROOT);
    }
}
