use std::{env, ffi::OsString, path::PathBuf};

use crate::common::resolve::is_valid_executable;
use crate::common::{error::Error, Environment};
use crate::exec::RunOptions;
use crate::system::{Group, Process, User};

use super::cli::SuOptions;

const PATH_MAILDIR: &str = env!("PATH_MAILDIR");
const PATH_DEFAULT: &str = env!("SU_PATH_DEFAULT");
const PATH_DEFAULT_ROOT: &str = env!("SU_PATH_DEFAULT_ROOT");

#[derive(Debug)]
pub(crate) struct SuContext {
    command: PathBuf,
    arguments: Vec<String>,
    options: SuOptions,
    pub(crate) environment: Environment,
    user: User,
    group: Group,
    pub(crate) process: Process,
}

impl SuContext {
    pub(crate) fn from_env(options: SuOptions) -> Result<SuContext, Error> {
        let process = crate::system::Process::new();

        // resolve environment, reset if this is a login
        let mut environment = if options.login {
            Environment::default()
        } else {
            env::vars_os().collect::<Environment>()
        };

        // Don't reset the environment variables specified in the
        // comma-separated list when clearing the environment for
        // --login. The whitelist is ignored for the environment
        // variables HOME, SHELL, USER, LOGNAME, and PATH.
        if options.login {
            for name in options.whitelist_environment.iter() {
                if let Some(value) = env::var_os(name) {
                    environment.insert(name.into(), value);
                }
            }
        }

        // resolve target user
        let mut user = User::from_name(&options.user)?
            .ok_or_else(|| Error::UserNotFound(options.user.clone()))?;

        // check the current user is root
        let is_current_root = User::effective_uid() == 0;
        let is_target_root = options.user == "root";

        // resolve target group
        let group = match (&options.group, &options.supp_group) {
            (Some(group), _) | (_, Some(group)) => {
                if is_current_root {
                    Group::from_name(group)?.ok_or_else(|| Error::GroupNotFound(group.to_owned()))
                } else {
                    Err(Error::Options(
                        "setting a group is only allowed for root".to_owned(),
                    ))
                }
            }
            _ => {
                Group::from_gid(user.gid)?.ok_or_else(|| Error::GroupNotFound(user.gid.to_string()))
            }
        }?;

        // add additional group if current user is root
        if let Some(group_name) = &options.supp_group {
            if is_current_root {
                let supp_group = Group::from_name(group_name)?
                    .ok_or_else(|| Error::GroupNotFound(group_name.to_owned()))?;
                user.groups.push(supp_group.gid);
            }
        }

        // the shell specified with --shell
        // the shell specified in the environment variable SHELL, if the --preserve-environment option is used
        // the shell listed in the passwd entry of the target user
        let command = options
            .shell
            .as_ref()
            .cloned()
            .or_else(|| {
                if options.preserve_environment {
                    environment.get(&OsString::from("SHELL")).map(|v| v.into())
                } else {
                    None
                }
            })
            .unwrap_or(user.shell.clone());

        if !command.exists() {
            return Err(Error::CommandNotFound(command));
        }

        if !is_valid_executable(&command) {
            return Err(Error::InvalidCommand(command));
        }

        // pass command to shell
        let arguments = if let Some(command) = &options.command {
            vec!["-c".to_owned(), command.to_owned()]
        } else {
            options.arguments.clone()
        };

        if options.login {
            environment.insert(
                "PATH".into(),
                if is_target_root {
                    PATH_DEFAULT_ROOT
                } else {
                    PATH_DEFAULT
                }
                .into(),
            );
        }

        if !options.preserve_environment {
            // extend environment with fixed variables
            environment.insert("HOME".into(), user.home.clone().into_os_string());
            environment.insert("SHELL".into(), command.clone().into());
            environment.insert(
                "MAIL".into(),
                format!("{PATH_MAILDIR}/{}", user.name).into(),
            );

            if !is_target_root || options.login {
                environment.insert("USER".into(), options.user.clone().into());
                environment.insert("LOGNAME".into(), options.user.clone().into());
            }
        }

        Ok(SuContext {
            command,
            arguments,
            options,
            environment,
            user,
            group,
            process,
        })
    }
}

impl RunOptions for SuContext {
    fn command(&self) -> &PathBuf {
        &self.command
    }

    fn arguments(&self) -> &Vec<String> {
        &self.arguments
    }

    fn chdir(&self) -> Option<&std::path::PathBuf> {
        None
    }

    fn is_login(&self) -> bool {
        self.options.login
    }

    fn user(&self) -> &crate::system::User {
        &self.user
    }

    fn group(&self) -> &crate::system::Group {
        &self.group
    }

    fn pid(&self) -> i32 {
        self.process.pid
    }
}
