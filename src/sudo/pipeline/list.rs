use std::path::Path;

use crate::{
    cli::{SudoAction, SudoOptions},
    common::Error,
    log::auth_warn,
    pam::CLIConverser,
    sudo::{pam::PamAuthenticator, SudoersPolicy},
    sudoers::{Authorization, ListRequest, Policy, Request},
    system::{timestamp::RecordScope, Process, User},
};

use super::{AuthPlugin, Pipeline, PolicyPlugin};

impl Pipeline<SudoersPolicy, PamAuthenticator<CLIConverser>> {
    pub(in crate::sudo) fn run_list(mut self, cmd_opts: SudoOptions) -> Result<(), Error> {
        let other_user = if let Some(other_user) = &cmd_opts.other_user {
            Some(
                User::from_name(other_user)?
                    .ok_or_else(|| Error::UserNotFound(other_user.to_string()))?,
            )
        } else {
            None
        };

        let pre = self.policy.init()?;
        let original_command = match &cmd_opts.action {
            SudoAction::List(args) => args.first().cloned(),
            _ => panic!("called `Pipeline::run_list` with a SudoAction other than `List`"),
        };

        let context = super::build_context(cmd_opts, &pre)?;

        if original_command.is_some() && !context.command.resolved {
            return Err(Error::CommandNotFound(context.command.command));
        }

        let list_request = ListRequest {
            target_user: &context.target_user,
            target_group: &context.target_group,
        };
        let judgement =
            pre.check_list_permission(&context.current_user, &context.hostname, list_request);
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

                    self.authenticator.init(&context)?;
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
                    return Ok(());
                } else {
                    let command = if other_user.is_none() {
                        "sudo".to_string()
                    } else if original_command.is_none() {
                        "list".to_string()
                    } else {
                        format!("list{}", context.command.command.display())
                    };

                    return Err(Error::NotAllowed {
                        username: context.current_user.name,
                        command,
                        hostname: context.hostname,
                        other_user: other_user.map(|user| user.name),
                    });
                }
            }
        }

        if let Some(other_user) = &other_user {
            let list_request = ListRequest {
                target_user: &context.target_user,
                target_group: &context.target_group,
            };
            let judgement = pre.check_list_permission(other_user, &context.hostname, list_request);

            if let Authorization::Forbidden = judgement.authorization() {
                let command = if original_command.is_none() {
                    "list".to_string()
                } else {
                    format!("list{}", context.command.command.display())
                };

                return Err(Error::NotAllowed {
                    username: context.current_user.name,
                    command,
                    hostname: context.hostname,
                    other_user: Some(other_user.name.clone()),
                });
            }
        }

        if let Some(original_command) = original_command {
            let user = other_user.as_ref().unwrap_or(&context.current_user);

            let request = Request {
                user: &context.target_user,
                group: &context.target_group,
                command: &context.command.command,
                arguments: &context.command.arguments,
            };

            let judgement = pre.check(user, &context.hostname, request);

            if let Authorization::Forbidden = judgement.authorization() {
                return Err(Error::Silent);
            } else {
                let command = if original_command.contains('/')
                    && !Path::new(&original_command).is_absolute()
                {
                    original_command
                } else {
                    context.command.command.display().to_string()
                };

                if context.command.arguments.is_empty() {
                    println!("{command}")
                } else {
                    println!("{command} {}", context.command.arguments.join(" "))
                }
            }
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
}
