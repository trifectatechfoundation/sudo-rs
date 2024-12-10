use std::ffi::{OsStr, OsString};
use std::process::exit;

use super::cli::{PreserveEnv, SudoRunOptions, SudoValidateOptions};
use crate::common::context::OptionsForContext;
use crate::common::resolve::CurrentUser;
use crate::common::{Context, Error};
use crate::exec::{ExecOutput, ExitReason};
use crate::log::{auth_info, auth_warn};
use crate::sudo::env::environment;
use crate::sudo::Duration;
use crate::sudoers::{Authorization, AuthorizationAllowed, DirChange, Policy, PreJudgementPolicy};
use crate::system::interface::UserId;
use crate::system::term::current_tty_name;
use crate::system::timestamp::{RecordScope, SessionRecordFile, TouchResult};
use crate::system::{escape_os_str_lossy, Process};

mod list;

pub trait PolicyPlugin {
    type PreJudgementPolicy: PreJudgementPolicy;
    type Policy: Policy;

    fn init(&mut self) -> Result<Self::PreJudgementPolicy, Error>;
    fn judge(
        &mut self,
        pre: Self::PreJudgementPolicy,
        context: &Context,
    ) -> Result<Self::Policy, Error>;
}

pub trait AuthPlugin {
    fn init(&mut self, context: &Context) -> Result<(), Error>;
    fn authenticate(&mut self, non_interactive: bool, max_tries: u16) -> Result<(), Error>;
    fn pre_exec(&mut self, target_user: &str) -> Result<Vec<(OsString, OsString)>, Error>;
    fn cleanup(&mut self);
}

pub struct Pipeline<Policy: PolicyPlugin, Auth: AuthPlugin> {
    pub policy: Policy,
    pub authenticator: Auth,
}

impl<Policy: PolicyPlugin, Auth: AuthPlugin> Pipeline<Policy, Auth> {
    pub fn run(mut self, cmd_opts: SudoRunOptions) -> Result<(), Error> {
        let pre = self.policy.init()?;

        let (ctx_opts, pipe_opts) = cmd_opts.into();

        let mut context = build_context(ctx_opts, &pre)?;

        let policy = self.policy.judge(pre, &context)?;

        let Authorization::Allowed(auth) = policy.authorization() else {
            return Err(Error::Authorization(context.current_user.name.to_string()));
        };
        self.apply_policy_to_context(&mut context, &policy)?;
        self.auth_and_update_record_file(&context, &auth)?;

        // build environment
        let current_env = environment::system_environment();

        let pam_env = self.authenticator.pre_exec(&context.target_user.name)?;

        let additional_env = match pipe_opts.preserve_env {
            PreserveEnv::Nothing => pam_env.into_iter().collect(),
            PreserveEnv::Everything if auth.trust_environment => current_env.clone(),
            PreserveEnv::Only(kept) if auth.trust_environment => {
                let mut env = current_env.clone();
                env.retain(|key, _| kept.contains(key));

                env
            }
            _ => return Err(Error::EnvironmentPreserve),
        };

        let (checked_vars, trusted_vars) = if auth.trust_environment {
            (vec![], pipe_opts.user_requested_env_vars)
        } else {
            (pipe_opts.user_requested_env_vars, vec![])
        };

        let mut target_env = environment::get_target_environment(
            current_env,
            additional_env,
            checked_vars,
            &context,
            &policy,
        )?;

        environment::dangerous_extend(&mut target_env, trusted_vars);

        let pid = context.process.pid;

        // run command and return corresponding exit code
        let exec_result = if context.command.resolved {
            log_command_execution(&context);

            crate::exec::run_command(&context, target_env)
                .map_err(|io_error| Error::Io(Some(context.command.command), io_error))
        } else {
            Err(Error::CommandNotFound(context.command.command))
        };

        self.authenticator.cleanup();

        let ExecOutput {
            command_exit_reason,
            restore_signal_handlers,
        } = exec_result?;

        // Run any clean-up code before this line.
        restore_signal_handlers();

        match command_exit_reason {
            ExitReason::Code(code) => exit(code),
            ExitReason::Signal(signal) => {
                crate::system::kill(pid, signal)?;
            }
        }

        Ok(())
    }

    pub fn run_validate(mut self, cmd_opts: SudoValidateOptions) -> Result<(), Error> {
        let pre = self.policy.init()?;
        let context = build_context(cmd_opts.into(), &pre)?;

        match pre.validate_authorization() {
            Authorization::Forbidden => {
                return Err(Error::Authorization(context.current_user.name.to_string()));
            }
            Authorization::Allowed(auth) => {
                self.auth_and_update_record_file(&context, &auth)?;
            }
        }

        Ok(())
    }

    fn auth_and_update_record_file(
        &mut self,
        context: &Context,
        &AuthorizationAllowed {
            must_authenticate,
            prior_validity,
            allowed_attempts,
            ..
        }: &AuthorizationAllowed,
    ) -> Result<(), Error> {
        let scope = RecordScope::for_process(&Process::new());
        let mut auth_status = determine_auth_status(
            must_authenticate,
            context.use_session_records,
            scope,
            context.current_user.uid,
            &context.current_user,
            prior_validity,
        );
        self.authenticator.init(context)?;
        if auth_status.must_authenticate {
            self.authenticator
                .authenticate(context.non_interactive, allowed_attempts)?;
            if let (Some(record_file), Some(scope)) = (&mut auth_status.record_file, scope) {
                match record_file.create(scope, context.current_user.uid) {
                    Ok(_) => (),
                    Err(e) => {
                        auth_warn!("Could not update session record file with new record: {e}");
                    }
                }
            }
        }

        Ok(())
    }

    fn apply_policy_to_context(
        &mut self,
        context: &mut Context,
        policy: &<Policy as PolicyPlugin>::Policy,
    ) -> Result<(), crate::common::Error> {
        // see if the chdir flag is permitted
        match policy.chdir() {
            DirChange::Any => {}
            DirChange::Strict(optdir) => {
                if let Some(chdir) = &context.chdir {
                    return Err(Error::ChDirNotAllowed {
                        chdir: chdir.clone(),
                        command: context.command.command.clone(),
                    });
                } else {
                    context.chdir = optdir.cloned();
                }
            }
        }

        // expand tildes in the path with the users home directory
        if let Some(dir) = context.chdir.take() {
            context.chdir = Some(dir.expand_tilde_in_path(&context.target_user.name)?)
        }

        // override the default pty behaviour if indicated
        if !policy.use_pty() {
            context.use_pty = false
        }

        Ok(())
    }
}

fn build_context(
    cmd_opts: OptionsForContext,
    pre: &dyn PreJudgementPolicy,
) -> Result<Context, Error> {
    let secure_path: String = pre
        .secure_path()
        .unwrap_or_else(|| std::env::var("PATH").unwrap_or_default());
    Context::build_from_options(cmd_opts, secure_path)
}

/// This should determine what the authentication status for the given record
/// match limit and origin/target user from the context is.
fn determine_auth_status(
    must_policy_authenticate: bool,
    use_session_records: bool,
    record_for: Option<RecordScope>,
    auth_uid: UserId,
    current_user: &CurrentUser,
    prior_validity: Duration,
) -> AuthStatus {
    if !must_policy_authenticate {
        AuthStatus::new(false, None)
    } else if let (true, Some(record_for)) = (use_session_records, record_for) {
        match SessionRecordFile::open_for_user(current_user, prior_validity) {
            Ok(mut sr) => {
                match sr.touch(record_for, auth_uid) {
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
