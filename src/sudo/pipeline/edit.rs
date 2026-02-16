use super::super::cli::SudoEditOptions;
use crate::common::{Context, DisplayOsStr, Error};
use crate::log::{user_error, user_info};
use crate::sudoers::Authorization;
use crate::system::audit;

pub fn run_edit(edit_opts: SudoEditOptions) -> Result<(), Error> {
    let policy = super::read_sudoers()?;

    let context = Context::from_edit_opts(edit_opts)?;

    let policy = super::judge(policy, &context)?;

    let Authorization::Allowed(auth, _controls) = policy.authorization() else {
        return Err(Error::Authorization(context.current_user.name.to_string()));
    };

    let mut pam_context = super::auth_and_update_record_file(&context, auth)?;

    let mut opened_files = Vec::with_capacity(context.files_to_edit.len());
    for (path, arg) in context.files_to_edit.iter().zip(&context.command.arguments) {
        if let Some(path) = path {
            match audit::secure_open_for_sudoedit(
                path,
                &context.current_user,
                &context.target_user,
                &context.target_group,
            ) {
                Ok(file) => opened_files.push((path, file)),
                // ErrorKind::FilesystemLoop was only stabilized in 1.83
                Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
                    user_error!(
                        "{path}: editing symbolic links is not permitted",
                        path = DisplayOsStr(arg),
                    )
                }
                Err(error) => {
                    user_error!(
                        "error opening {path}: {error}",
                        path = DisplayOsStr(arg),
                        error = error,
                    )
                }
            }
        } else {
            user_error!("invalid path: {path}", path = DisplayOsStr(arg));
        }
    }

    if opened_files.len() != context.files_to_edit.len() {
        user_info!("please address the problems and try again");
        return Err(Error::Silent);
    }

    // run command and return corresponding exit code
    let command_exit_reason = {
        super::log_command_execution(&context);

        let editor = policy.preferred_editor();

        crate::sudo::edit::edit_files(&editor, opened_files)
    };

    pam_context.close_session();

    match command_exit_reason?.exit_process()? {}
}
