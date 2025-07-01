use std::process::exit;

use super::super::cli::SudoEditOptions;
use crate::common::{Context, Error};
use crate::exec::ExitReason;
use crate::sudoers::Authorization;

pub fn run_edit(edit_opts: SudoEditOptions) -> Result<(), Error> {
    let policy = super::read_sudoers()?;

    let mut context = Context::from_edit_opts(edit_opts)?;

    let policy = super::judge(policy, &context)?;

    let Authorization::Allowed(auth, controls) = policy.authorization() else {
        return Err(Error::Authorization(context.current_user.name.to_string()));
    };

    super::apply_policy_to_context(&mut context, &controls)?;
    let mut pam_context = super::auth_and_update_record_file(&context, auth)?;

    let pid = context.process.pid;

    // run command and return corresponding exit code
    let command_exit_reason = {
        super::log_command_execution(&context);

        eprintln_ignore_io_error!(
            "this would launch sudoedit as requested, to edit the files: {:?}",
            context.command.arguments.as_slice()
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
