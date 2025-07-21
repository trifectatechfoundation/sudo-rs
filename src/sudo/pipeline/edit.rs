use std::process::exit;

use super::super::cli::SudoEditOptions;
use crate::common::{Context, Error};
use crate::exec::ExitReason;
use crate::sudoers::Authorization;

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

    for (path, arg) in context.files_to_edit.iter().zip(&context.command.arguments) {
        if path.is_none() {
            eprintln_ignore_io_error!("invalid path: {arg}")
        }
    }

    // run command and return corresponding exit code
    let command_exit_reason = {
        super::log_command_execution(&context);

        let editor = policy.preferred_editor();

        crate::sudo::edit::edit_files(
            &editor,
            &context
                .files_to_edit
                .iter()
                .flatten()
                .map(|path| &**path)
                .collect::<Vec<_>>(),
        )
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
