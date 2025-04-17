use std::io;

use crate::common::{HARDENED_ENUM_VALUE_0, HARDENED_ENUM_VALUE_1, HARDENED_ENUM_VALUE_2};
use crate::exec::RunOptions;
use crate::sudo::{SudoListOptions, SudoRunOptions, SudoValidateOptions};
use crate::sudoers::Sudoers;
use crate::system::{Group, Hostname, Process, User};

use super::{
    command::CommandAndArguments,
    resolve::{resolve_shell, resolve_target_user_and_group, CurrentUser},
    Error, SudoPath,
};

#[derive(Debug)]
pub struct Context {
    // cli options
    pub launch: LaunchType,
    pub chdir: Option<SudoPath>,
    pub command: CommandAndArguments,
    pub target_user: User,
    pub target_group: Group,
    pub stdin: bool,
    pub bell: bool,
    pub prompt: Option<String>,
    pub non_interactive: bool,
    pub use_session_records: bool,
    // system
    pub hostname: Hostname,
    pub current_user: CurrentUser,
    pub process: Process,
    // policy
    pub use_pty: bool,
    pub password_feedback: bool,
    pub noexec: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum LaunchType {
    #[default]
    Direct = HARDENED_ENUM_VALUE_0,
    Shell = HARDENED_ENUM_VALUE_1,
    Login = HARDENED_ENUM_VALUE_2,
}

impl Context {
    pub fn from_run_opts(
        sudo_options: SudoRunOptions,
        policy: &mut Sudoers,
    ) -> Result<Context, Error> {
        let hostname = Hostname::resolve();
        let current_user = CurrentUser::resolve()?;

        let (target_user, target_group) =
            resolve_target_user_and_group(&sudo_options.user, &sudo_options.group, &current_user)?;

        let launch = if sudo_options.login {
            LaunchType::Login
        } else if sudo_options.shell {
            LaunchType::Shell
        } else {
            LaunchType::Direct
        };

        let shell = resolve_shell(launch, &current_user, &target_user);

        let override_path = policy.search_path(&hostname, &current_user, &target_user);

        let command = {
            let system_path;

            let path = if let Some(path) = override_path {
                path
            } else {
                system_path = std::env::var("PATH").unwrap_or_default();
                system_path.as_ref()
            };

            CommandAndArguments::build_from_args(shell, sudo_options.positional_args, path)
        };

        Ok(Context {
            hostname,
            command,
            current_user,
            target_user,
            target_group,
            use_session_records: !sudo_options.reset_timestamp,
            launch,
            chdir: sudo_options.chdir,
            stdin: sudo_options.stdin,
            bell: sudo_options.bell,
            prompt: sudo_options.prompt,
            non_interactive: sudo_options.non_interactive,
            process: Process::new(),
            use_pty: true,
            password_feedback: false,
            noexec: false,
        })
    }

    pub fn from_validate_opts(sudo_options: SudoValidateOptions) -> Result<Context, Error> {
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
            chdir: None,
            stdin: sudo_options.stdin,
            bell: sudo_options.bell,
            prompt: sudo_options.prompt,
            non_interactive: sudo_options.non_interactive,
            process: Process::new(),
            use_pty: true,
            password_feedback: false,
            noexec: false,
        })
    }

    pub fn from_list_opts(
        sudo_options: SudoListOptions,
        policy: &mut Sudoers,
    ) -> Result<Context, Error> {
        let hostname = Hostname::resolve();
        let current_user = CurrentUser::resolve()?;
        let (target_user, target_group) =
            resolve_target_user_and_group(&sudo_options.user, &sudo_options.group, &current_user)?;

        let override_path = policy.search_path(&hostname, &current_user, &target_user);

        let command = if sudo_options.positional_args.is_empty() {
            Default::default()
        } else {
            let system_path;

            let path = if let Some(path) = override_path {
                path
            } else {
                system_path = std::env::var("PATH").unwrap_or_default();
                system_path.as_ref()
            };

            CommandAndArguments::build_from_args(None, sudo_options.positional_args, path)
        };

        Ok(Context {
            hostname,
            command,
            current_user,
            target_user,
            target_group,
            use_session_records: !sudo_options.reset_timestamp,
            launch: Default::default(),
            chdir: None,
            stdin: sudo_options.stdin,
            bell: sudo_options.bell,
            prompt: sudo_options.prompt,
            non_interactive: sudo_options.non_interactive,
            process: Process::new(),
            use_pty: true,
            password_feedback: false,
            noexec: false,
        })
    }

    pub(crate) fn try_as_run_options(&self) -> io::Result<RunOptions<'_>> {
        Ok(RunOptions {
            command: if self.command.resolved {
                &self.command.command
            } else {
                return Err(io::ErrorKind::NotFound.into());
            },
            arguments: &self.command.arguments,
            arg0: self.command.arg0.as_deref(),
            chdir: self.chdir.as_deref(),
            is_login: self.launch == LaunchType::Login,
            user: &self.target_user,
            group: &self.target_group,

            use_pty: self.use_pty,
            noexec: self.noexec,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        sudo::SudoAction,
        system::{interface::UserId, Hostname},
    };

    use super::Context;

    #[test]
    fn test_build_run_context() {
        let options = SudoAction::try_parse_from(["sudo", "echo", "hello"])
            .unwrap()
            .try_into_run()
            .ok()
            .unwrap();

        let context = Context::from_run_opts(options, &mut Default::default()).unwrap();

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
