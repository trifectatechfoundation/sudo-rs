use std::{borrow::Cow, ffi::OsStr, ops::ControlFlow, path::Path};

use crate::{
    common::{Context, Error},
    sudo::cli::SudoListOptions,
    sudoers::{Authorization, ListRequest, Request, Sudoers},
    system::User,
};

use super::auth_and_update_record_file;

pub(in crate::sudo) fn run_list(cmd_opts: SudoListOptions) -> Result<(), Error> {
    let verbose_list_mode = cmd_opts.list.is_verbose();
    let other_user = cmd_opts
        .other_user
        .as_ref()
        .map(|username| {
            User::from_name(username.as_cstr())?
                .ok_or_else(|| Error::UserNotFound(username.clone().into()))
        })
        .transpose()?;

    let original_command = cmd_opts.positional_args.first().cloned();

    let mut sudoers = super::read_sudoers()?;

    let context = Context::from_list_opts(cmd_opts, &mut sudoers)?;

    if auth_invoking_user(&context, &mut sudoers, &original_command, &other_user)?.is_break() {
        return Ok(());
    }

    if let Some(original_command) = original_command {
        check_sudo_command_perms(&original_command, context, &other_user, &mut sudoers)?;
    } else {
        let inspected_user = other_user.as_ref().unwrap_or(&context.current_user);
        let mut matching_entries = sudoers
            .matching_entries(inspected_user, &context.hostname)
            .peekable();

        if matching_entries.peek().is_some() {
            xlat_println!(
                "User {user} may run the following commands on {hostname}:",
                user = inspected_user.name,
                hostname = context.hostname
            );

            for entry in matching_entries {
                if verbose_list_mode {
                    let entry = entry.verbose();
                    println_ignore_io_error!("{entry}");
                } else {
                    println_ignore_io_error!("{entry}");
                }
            }
        } else {
            xlat_println!(
                "User {user} is not allowed to run sudo on {hostname}.",
                user = inspected_user.name,
                hostname = context.hostname,
            );
        }
    }

    Ok(())
}

fn auth_invoking_user(
    context: &Context,
    sudoers: &mut Sudoers,
    original_command: &Option<String>,
    other_user: &Option<User>,
) -> Result<ControlFlow<(), ()>, Error> {
    let inspected_user = other_user.as_ref().unwrap_or(&context.current_user);

    let list_request = ListRequest {
        inspected_user,
        target_user: &context.target_user,
        target_group: &context.target_group,
    };
    match sudoers.check_list_permission(&*context.current_user, &context.hostname, list_request) {
        Authorization::Allowed(auth, ()) => {
            auth_and_update_record_file(context, auth)?;
            Ok(ControlFlow::Continue(()))
        }

        Authorization::Forbidden => {
            let command = if other_user.is_none() {
                "sudo".into()
            } else {
                format_list_command(original_command)
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

fn check_sudo_command_perms(
    original_command: &str,
    context: Context,
    other_user: &Option<User>,
    sudoers: &mut Sudoers,
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
        if !context.command.resolved {
            return Err(Error::CommandNotFound(context.command.command));
        }
        let command_is_relative_path =
            original_command.contains('/') && !Path::new(&original_command).is_absolute();
        let command: Cow<_> = if command_is_relative_path {
            original_command.into()
        } else {
            let resolved_command = &context.command.command;
            resolved_command.display().to_string().into()
        };

        if context.command.arguments.is_empty() {
            println_ignore_io_error!("{command}");
        } else {
            println_ignore_io_error!(
                "{command} {}",
                context.command.arguments.join(OsStr::new(" ")).display(),
            );
        }
    }

    Ok(())
}

fn format_list_command(original_command: &Option<String>) -> Cow<'static, str> {
    if let Some(original_command) = original_command {
        format!("list {original_command}").into()
    } else {
        "list".into()
    }
}
