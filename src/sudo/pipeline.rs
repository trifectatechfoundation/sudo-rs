use std::ffi::OsStr;
use std::time::Duration;

use super::cli::{SudoRunOptions, SudoValidateOptions};
use super::diagnostic;
use crate::common::resolve::{AuthUser, CurrentUser};
use crate::common::{Context, Error};
use crate::log::{auth_info, auth_warn};
use crate::pam::PamContext;
use crate::sudo::env::environment;
use crate::sudo::pam::{InitPamArgs, attempt_authenticate, init_pam, pre_exec};
use crate::sudoers::{AuthenticatingUser, Authentication, Authorization, Judgement, Sudoers};
use crate::system::term::current_tty_name;
use crate::system::timestamp::{RecordScope, SessionRecordFile, TouchResult};
use crate::system::{Process, escape_os_str_lossy};

mod list;
pub(super) use list::run_list;

mod edit;
pub(super) use edit::run_edit;

fn read_sudoers() -> Result<Sudoers, Error> {
    let sudoers_path = &super::candidate_sudoers_file();

    let (sudoers, syntax_errors) = Sudoers::open(sudoers_path).map_err(|e| {
        // Provide a more helpful error message when the sudoers file is missing
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::Configuration(xlat!(
                "sudoers file not found: {path}\n\
                 \n\
                 The sudoers file is required for sudo-rs to function. Please ensure:\n\
                 - The file exists at the expected location\n\
                 - You have the necessary permissions to read it\n\
                 - If setting up sudo-rs for the first time, create a sudoers file with appropriate permissions\n\
                 \n\
                 For more information, see the sudo-rs documentation.",
                path = sudoers_path.display()
            ))
        } else {
            Error::Configuration(xlat!("invalid configuration: {error}", error = e))
        }
    })?;

    for crate::sudoers::Error {
        source,
        location,
        message,
    } in syntax_errors
    {
        let path = source.as_deref().unwrap_or(sudoers_path);
        diagnostic::diagnostic!("{message}", path @ location);
    }

    Ok(sudoers)
}

fn judge(mut policy: Sudoers, context: &Context) -> Result<Judgement, Error> {
    Ok(policy.check(
        &*context.current_user,
        &context.hostname,
        crate::sudoers::Request {
            user: &context.target_user,
            group: &context.target_group,
            command: &context.command.command,
            arguments: &context.command.arguments,
        },
    ))
}

pub fn run(mut cmd_opts: SudoRunOptions) -> Result<(), Error> {
    let mut policy = read_sudoers()?;

    let user_requested_env_vars = std::mem::take(&mut cmd_opts.env_var_list);

    let context = Context::from_run_opts(cmd_opts, &mut policy)?;

    let policy = judge(policy, &context)?;

    let Authorization::Allowed(auth, controls) = policy.authorization() else {
        return Err(Error::Authorization(context.current_user.name.to_string()));
    };

    let mut pam_context = auth_and_update_record_file(&context, auth)?;

    // build environment
    let additional_env = pre_exec(&mut pam_context, &context.target_user.name)?;

    let current_env = environment::system_environment();
    let (checked_vars, trusted_vars) = if controls.trust_environment {
        (vec![], user_requested_env_vars)
    } else {
        (user_requested_env_vars, vec![])
    };

    let mut target_env = environment::get_target_environment(
        current_env,
        additional_env,
        checked_vars,
        &context,
        &controls,
    )?;

    environment::dangerous_extend(&mut target_env, trusted_vars);

    // prepare switch of apparmor profile
    #[cfg(feature = "apparmor")]
    if let Some(profile) = &controls.apparmor_profile {
        crate::apparmor::set_profile_for_next_exec(profile)
            .map_err(|err| Error::AppArmor(profile.clone(), err))?;
    }

    let options = context.try_as_run_options(&controls)?;

    // Log after try_as_run_options to avoid logging if the command is not resolved
    log_command_execution(&context);

    // run command and return corresponding exit code
    let command_exit_reason = crate::exec::run_command(options, target_env)
        .map_err(|io_error| Error::Io(Some(context.command.command), io_error));

    pam_context.close_session();

    match command_exit_reason?.exit_process()? {}
}

pub fn run_validate(cmd_opts: SudoValidateOptions) -> Result<(), Error> {
    let mut policy = read_sudoers()?;

    let context = Context::from_validate_opts(cmd_opts)?;

    match policy.check_validate_permission(&*context.current_user, &context.hostname) {
        Authorization::Forbidden => {
            return Err(Error::Authorization(context.current_user.name.to_string()));
        }
        Authorization::Allowed(auth, ()) => {
            auth_and_update_record_file(&context, auth)?;
        }
    }

    Ok(())
}

fn auth_and_update_record_file(
    context: &Context,
    Authentication {
        must_authenticate,
        prior_validity,
        allowed_attempts,
        password_timeout,
        ref credential,
        pwfeedback,
        noninteractive_auth,
    }: Authentication,
) -> Result<PamContext, Error> {
    let auth_user = match credential {
        AuthenticatingUser::InvokingUser => {
            AuthUser::from_current_user(context.current_user.clone())
        }
        AuthenticatingUser::Root => AuthUser::resolve_root_for_rootpw()?,
        AuthenticatingUser::TargetUser => {
            AuthUser::from_user_for_targetpw(context.target_user.clone())
        }
    };

    let scope = RecordScope::for_process(&Process::new());
    let mut auth_status = determine_auth_status(
        must_authenticate,
        context.use_session_records,
        scope,
        &context.current_user,
        &auth_user,
        prior_validity,
    );

    let mut pam_context = init_pam(InitPamArgs {
        launch: context.launch,
        use_askpass: context.askpass,
        use_stdin: context.stdin,
        bell: context.bell,
        non_interactive: context.non_interactive,
        password_feedback: pwfeedback,
        password_timeout,
        auth_prompt: context.prompt.clone(),
        auth_user: &auth_user.name,
        requesting_user: &context.current_user.name,
        target_user: &context.target_user.name,
        hostname: &context.hostname,
    })?;
    if auth_status.must_authenticate {
        if context.non_interactive && !noninteractive_auth {
            return Err(Error::InteractionRequired);
        }

        attempt_authenticate(
            &mut pam_context,
            &auth_user.name,
            context.non_interactive,
            allowed_attempts,
        )?;
        if let (Some(record_file), Some(scope)) = (&mut auth_status.record_file, scope) {
            match record_file.create(scope, &auth_user) {
                Ok(_) => (),
                Err(e) => {
                    auth_warn!("Could not update session record file with new record: {e}");
                }
            }
        }
    }

    Ok(pam_context)
}

/// This should determine what the authentication status for the given record
/// match limit and origin/target user from the context is.
fn determine_auth_status(
    must_policy_authenticate: bool,
    use_session_records: bool,
    record_for: Option<RecordScope>,
    current_user: &CurrentUser,
    auth_user: &AuthUser,
    prior_validity: Duration,
) -> AuthStatus {
    if !must_policy_authenticate {
        AuthStatus::new(false, None)
    } else if let (true, Some(record_for)) = (use_session_records, record_for) {
        match SessionRecordFile::open_for_user(current_user, prior_validity) {
            Ok(mut sr) => {
                match sr.touch(record_for, auth_user) {
                    // if a record was found and updated within the timeout, we do not need to authenticate
                    Ok(TouchResult::Updated { .. }) => AuthStatus::new(false, Some(sr)),
                    Ok(TouchResult::NotFound | TouchResult::Outdated { .. }) => {
                        AuthStatus::new(true, Some(sr))
                    }
                    Err(e) => {
                        auth_warn!("Unexpected error while reading session information: {e}");
                        AuthStatus::new(true, None)
                    }
                }
            }
            // if we cannot open the session record file we just assume there is none and continue as normal
            Err(e) => {
                auth_warn!("Could not use session information: {e}");
                AuthStatus::new(true, None)
            }
        }
    } else {
        AuthStatus::new(true, None)
    }
}

struct AuthStatus {
    must_authenticate: bool,
    record_file: Option<SessionRecordFile>,
}

impl AuthStatus {
    fn new(must_authenticate: bool, record_file: Option<SessionRecordFile>) -> AuthStatus {
        AuthStatus {
            must_authenticate,
            record_file,
        }
    }
}

fn log_command_execution(context: &Context) {
    let tty_info = if let Ok(tty_name) = current_tty_name() {
        format!("TTY={} ;", escape_os_str_lossy(&tty_name))
    } else {
        String::from("")
    };
    let pwd = escape_os_str_lossy(
        std::env::current_dir()
            .as_ref()
            .map(|s| s.as_os_str())
            .unwrap_or_else(|_| OsStr::new("unknown")),
    );
    let user = context.target_user.name.escape_debug().collect::<String>();
    auth_info!(
        "{} : {} PWD={} ; USER={} ; COMMAND={}",
        &context.current_user.name,
        tty_info,
        pwd,
        user,
        &context.command
    );
}
