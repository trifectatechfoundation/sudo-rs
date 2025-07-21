use std::process::exit;

use super::super::cli::SudoEditOptions;
use crate::common::{Context, Error};
use crate::exec::ExitReason;
use crate::sudoers::Authorization;
use crate::system::audit;

pub fn run_edit(edit_opts: SudoEditOptions) -> Result<(), Error> {
    super::super::unstable_warning();

    let policy = super::read_sudoers()?;

    let mut context = Context::from_edit_opts(edit_opts)?;

    let policy = super::judge(policy, &context)?;

    let Authorization::Allowed(auth, controls) = policy.authorization() else {
        return Err(Error::Authorization(context.current_user.name.to_string()));
    };

    super::apply_policy_to_context(&mut context, &controls)?;
    let mut pam_context = super::auth_and_update_record_file(&context, auth)?;

    let pid = context.process.pid;

    let mut opened_files = Vec::with_capacity(context.files_to_edit.len());
    for (path, arg) in context.files_to_edit.iter().zip(&context.command.arguments) {
        if let Some(path) = path {
            match audit::secure_open_for_sudoedit(path, &context.target_user) {
                Ok(file) => opened_files.push((path, file)),
                Err(error) => eprintln_ignore_io_error!("error opening {arg}: {error}"),
            }
        } else {
            eprintln_ignore_io_error!("invalid path: {arg}");
        }
    }

    if opened_files.len() != context.files_to_edit.len() {
        eprintln_ignore_io_error!("please address the problems and try again");
        return Ok(());
    }

    // run command and return corresponding exit code
    let command_exit_reason = {
        super::log_command_execution(&context);

        let editor = policy.preferred_editor();

        eprintln_ignore_io_error!(
            "this would launch sudoedit as requested, to edit the files: {:?} using editor {}",
            opened_files.into_iter().map(|x| x.1).collect::<Vec<_>>(),
            editor.display(),
        );

        Ok::<_, std::io::Error>(ExitReason::Code(42))
    };

    pam_context.close_session();

    match command_exit_reason? {
        ExitReason::Code(code) => exit(code),
        ExitReason::Signal(signal) => {
            crate::system::kill(pid, signal)?;
        }
    }

    Ok(())
}
