use std::env;

use crate::common::{Error, HARDENED_ENUM_VALUE_0, HARDENED_ENUM_VALUE_1, HARDENED_ENUM_VALUE_2};
use crate::exec::RunOptions;
use crate::sudo::{SudoEditOptions, SudoListOptions, SudoRunOptions, SudoValidateOptions};
use crate::sudoers::Sudoers;
use crate::sudoers::{DirChange, Restrictions};
use crate::system::{audit::sudo_call, Group, Hostname, User};

use super::{
    command::CommandAndArguments,
    resolve::{resolve_shell, resolve_target_user_and_group, CurrentUser},
    SudoPath,
};

#[derive(Debug)]
pub struct Context {
    // cli options
    pub launch: LaunchType,
    pub chdir: Option<SudoPath>,
    pub command: CommandAndArguments,
    pub target_user: User,
    pub target_group: Group,
    pub askpass: bool,
    pub stdin: bool,
    pub bell: bool,
    pub prompt: Option<String>,
    pub non_interactive: bool,
    pub use_session_records: bool,
    // system
    pub hostname: Hostname,
    pub current_user: CurrentUser,
    // sudoedit
    pub files_to_edit: Vec<Option<SudoPath>>,
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
                system_path = env::var("PATH").unwrap_or_default();
                system_path.as_ref()
            };

            sudo_call(&target_user, &target_group, || {
                CommandAndArguments::build_from_args(shell, sudo_options.positional_args, path)
            })?
        };

        let prompt = sudo_options.prompt.or_else(|| env::var("SUDO_PROMPT").ok());

        Ok(Context {
            hostname,
            command,
            current_user,
            target_user,
            target_group,
            use_session_records: !sudo_options.reset_timestamp,
            launch,
            chdir: sudo_options.chdir,
            askpass: sudo_options.askpass,
            stdin: sudo_options.stdin,
            bell: sudo_options.bell,
            prompt,
            non_interactive: sudo_options.non_interactive,
            files_to_edit: vec![],
        })
    }

    pub fn from_edit_opts(sudo_options: SudoEditOptions) -> Result<Context, Error> {
        use std::path::Path;
        let hostname = Hostname::resolve();
        let current_user = CurrentUser::resolve()?;

        let (target_user, target_group) =
            resolve_target_user_and_group(&sudo_options.user, &sudo_options.group, &current_user)?;

        // resolve file arguments; if something can't be resolved, don't add it to the "edit" list
        let resolved_args = sudo_call(&target_user, &target_group, || {
            sudo_options.positional_args.iter().map(|arg| {
                let path = Path::new(arg);
                let absolute_path;
                crate::common::resolve::canonicalize_newfile(if path.is_absolute() {
                    path
                } else {
                    absolute_path = Path::new(".").join(path);
                    &absolute_path
                })
                .map_err(|_| arg)
                .and_then(|path| path.into_os_string().into_string().map_err(|_| arg))
            })
        })?;

        let files_to_edit = resolved_args
            .clone()
            .map(|path| path.ok().map(SudoPath::from_cli_string))
            .collect();

        // if a path resolved to something that isn't in UTF-8, it means it isn't in the sudoers file
        // as well and so we treat it "as is" wrt. the policy lookup and fail if the user is allowed
        // by the policy to edit that file. this is to prevent leaking information.
        let arguments = resolved_args
            .map(|arg| match arg {
                Ok(arg) => arg,
                Err(arg) => arg.to_owned(),
            })
            .collect();

        // TODO: the more Rust way of doing things would be to create an alternative for sudoedit instead;
        // but a stringly typed interface feels the most decent thing to do (if we can pull it off)
        // since "sudoedit" really is like a builtin command to sudo. We may want to be a bit 'better' than
        // ogsudo in the future.
        let command = CommandAndArguments {
            command: std::path::PathBuf::from("sudoedit"),
            arguments,
            ..Default::default()
        };

        Ok(Context {
            hostname,
            command,
            current_user,
            target_user,
            target_group,
            use_session_records: !sudo_options.reset_timestamp,
            launch: Default::default(),
            chdir: sudo_options.chdir,
            askpass: sudo_options.askpass,
            stdin: sudo_options.stdin,
            bell: sudo_options.bell,
            prompt: sudo_options.prompt,
            non_interactive: sudo_options.non_interactive,
            files_to_edit,
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
            askpass: sudo_options.askpass,
            stdin: sudo_options.stdin,
            bell: sudo_options.bell,
            prompt: sudo_options.prompt,
            non_interactive: sudo_options.non_interactive,
            files_to_edit: vec![],
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
                system_path = env::var("PATH").unwrap_or_default();
                system_path.as_ref()
            };

            sudo_call(&target_user, &target_group, || {
                CommandAndArguments::build_from_args(None, sudo_options.positional_args, path)
            })?
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
            askpass: sudo_options.askpass,
            stdin: sudo_options.stdin,
            bell: sudo_options.bell,
            prompt: sudo_options.prompt,
            non_interactive: sudo_options.non_interactive,
            files_to_edit: vec![],
        })
    }

    pub(crate) fn try_as_run_options(
        &self,
        controls: &Restrictions,
    ) -> Result<RunOptions<'_>, Error> {
        // see if the chdir flag is permitted
        let mut chdir = match controls.chdir {
            DirChange::Any => self.chdir.clone(),
            DirChange::Strict(optdir) => {
                if let Some(chdir) = &self.chdir {
                    return Err(Error::ChDirNotAllowed {
                        chdir: chdir.clone(),
                        command: self.command.command.clone(),
                    });
                } else {
                    optdir.cloned()
                }
            }
        };

        // expand tildes in the path with the users home directory
        if let Some(dir) = chdir.take() {
            chdir = Some(dir.expand_tilde_in_path(&self.target_user.name)?);
        }

        Ok(RunOptions {
            command: if self.command.resolved {
                &self.command.command
            } else {
                return Err(Error::CommandNotFound(self.command.command.clone()));
            },
            arguments: &self.command.arguments,
            arg0: self.command.arg0.as_deref(),
            chdir: chdir.as_deref().map(ToOwned::to_owned),
            is_login: self.launch == LaunchType::Login,
            user: &self.target_user,
            group: &self.target_group,
            umask: controls.umask,

            use_pty: controls.use_pty,
            noexec: controls.noexec,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{common::resolve::CurrentUser, sudo::SudoAction, system::Hostname};

    use super::Context;

    #[test]
    fn test_build_run_context() {
        let mut options = SudoAction::try_parse_from(["sudo", "echo", "hello"])
            .unwrap()
            .try_into_run()
            .ok()
            .unwrap();

        let current_user = CurrentUser::resolve().unwrap();
        options.user = Some(current_user.name.clone());

        let context = Context::from_run_opts(options, &mut Default::default()).unwrap();

        if cfg!(target_os = "linux") {
            // this assumes /bin is a symlink on /usr/bin, like it is on modern Debian/Ubuntu
            assert_eq!(context.command.command.to_str().unwrap(), "/usr/bin/echo");
        } else {
            assert_eq!(context.command.command.to_str().unwrap(), "/bin/echo");
        }
        assert_eq!(context.command.arguments, ["hello"]);
        assert_eq!(context.hostname, Hostname::resolve());
        assert_eq!(context.target_user.uid, current_user.uid);
    }
}
