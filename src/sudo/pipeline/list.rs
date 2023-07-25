use std::{borrow::Cow, ops::ControlFlow, path::Path};

use crate::{
    cli::{SudoAction, SudoOptions},
    common::{Context, Error},
    log::auth_warn,
    pam::CLIConverser,
    sudo::{pam::PamAuthenticator, SudoersPolicy},
    sudoers::{Authorization, ListRequest, Policy, Request, Sudoers},
    system::{timestamp::RecordScope, Process, User},
};

use super::{AuthPlugin, Pipeline, PolicyPlugin};

impl Pipeline<SudoersPolicy, PamAuthenticator<CLIConverser>> {
    pub(in crate::sudo) fn run_list(mut self, cmd_opts: SudoOptions) -> Result<(), Error> {
        let other_user = cmd_opts
            .other_user
            .as_ref()
            .map(|username| {
                User::from_name(username)?.ok_or_else(|| Error::UserNotFound(username.clone()))
            })
            .transpose()?;

        let original_command = if let SudoAction::List(args) = &cmd_opts.action {
            args.first().cloned()
        } else {
            panic!("called `Pipeline::run_list` with a SudoAction other than `List`")
        };

        let sudoers = self.policy.init()?;
        let context = super::build_context(cmd_opts, &sudoers)?;

        if original_command.is_some() && !context.command.resolved {
            return Err(Error::CommandNotFound(context.command.command));
        }

        if self
            .auth_invoking_user(&context, &sudoers, &original_command, &other_user)?
            .is_break()
        {
            return Ok(());
        }

        if let Some(other_user) = &other_user {
            check_other_users_list_perms(other_user, &context, &sudoers, &original_command)?;
        }

        if let Some(original_command) = original_command {
            check_sudo_command_perms(&original_command, &context, &other_user, &sudoers)?;
        } else {
            println!(
                "User {} may run the following commands on {}:",
                other_user.as_ref().unwrap_or(&context.current_user).name,
                context.hostname
            );

            // TODO print sudoers policies
        }

        Ok(())
    }

    fn auth_invoking_user(
        &mut self,
        context: &Context,
        sudoers: &Sudoers,
        original_command: &Option<String>,
        other_user: &Option<User>,
    ) -> Result<ControlFlow<(), ()>, Error> {
        let list_request = ListRequest {
            target_user: &context.target_user,
            target_group: &context.target_group,
        };
        let judgement =
            sudoers.check_list_permission(&context.current_user, &context.hostname, list_request);
        match judgement.authorization() {
            Authorization::Allowed {
                must_authenticate,
                allowed_attempts,
                prior_validity,
            } => {
                if must_authenticate {
                    let scope = RecordScope::for_process(&Process::new());
                    let mut auth_status = super::determine_auth_status(
                        must_authenticate,
                        context.use_session_records,
                        scope,
                        context.current_user.uid,
                        &context.current_user.name,
                        prior_validity,
                    );

                    self.authenticator.init(context)?;
                    if auth_status.must_authenticate() {
                        self.authenticator
                            .authenticate(context.non_interactive, allowed_attempts)?;
                        if let (Some(record_file), Some(scope)) =
                            (&mut auth_status.record_file, scope)
                        {
                            match record_file.create(scope, context.current_user.uid) {
                                Ok(_) => (),
                                Err(e) => {
                                    auth_warn!(
                                        "Could not update session record file with new record: {e}"
                                    );
                                }
                            }
                        }
                    }
                }

                Ok(ControlFlow::Continue(()))
            }

            Authorization::Forbidden => {
                if context.current_user.uid == 0 {
                    if original_command.is_some() {
                        return Err(Error::Silent);
                    }

                    println!(
                        "User {} is not allowed to run sudo on {}.",
                        other_user.as_ref().unwrap_or(&context.current_user).name,
                        context.hostname
                    );

                    // this branch does not result in exit code 1 but no further information should
                    // be printed in this case
                    Ok(ControlFlow::Break(()))
                } else {
                    let command = if other_user.is_none() {
                        "sudo".into()
                    } else if original_command.is_none() {
                        "list".into()
                    } else {
                        format!("list{}", context.command.command.display()).into()
                    };

                    Err(Error::NotAllowed {
                        username: context.current_user.name.clone(),
                        command,
                        hostname: context.hostname.clone(),
                        other_user: other_user.as_ref().map(|user| &user.name).cloned(),
                    })
                }
            }
        }
    }
}

fn check_other_users_list_perms(
    other_user: &User,
    context: &Context,
    sudoers: &Sudoers,
    original_command: &Option<String>,
) -> Result<(), Error> {
    let list_request = ListRequest {
        target_user: &context.target_user,
        target_group: &context.target_group,
    };
    let judgement = sudoers.check_list_permission(other_user, &context.hostname, list_request);

    if let Authorization::Forbidden = judgement.authorization() {
        let command = if original_command.is_none() {
            "list".into()
        } else {
            format!("list{}", context.command.command.display()).into()
        };

        return Err(Error::NotAllowed {
            username: context.current_user.name.clone(),
            command,
            hostname: context.hostname.clone(),
            other_user: Some(other_user.name.clone()),
        });
    }

    Ok(())
}

fn check_sudo_command_perms(
    original_command: &str,
    context: &Context,
    other_user: &Option<User>,
    sudoers: &Sudoers,
) -> Result<(), Error> {
    let user = other_user.as_ref().unwrap_or(&context.current_user);

    let request = Request {
        user: &context.target_user,
        group: &context.target_group,
        command: &context.command.command,
        arguments: &context.command.arguments,
    };

    let judgement = sudoers.check(user, &context.hostname, request);

    if let Authorization::Forbidden = judgement.authorization() {
        return Err(Error::Silent);
    } else {
        let command_is_relative_path =
            original_command.contains('/') && !Path::new(&original_command).is_absolute();
        let command: Cow<_> = if command_is_relative_path {
            original_command.into()
        } else {
            let resolved_command = &context.command.command;
            resolved_command.display().to_string().into()
        };

        if context.command.arguments.is_empty() {
            println!("{command}")
        } else {
            println!("{command} {}", context.command.arguments.join(" "))
        }
    }

    Ok(())
}
