use std::fs::File;
use std::process::exit;

use crate::cli::SudoOptions;
use crate::common::{Context, Environment, Error};
use crate::env::environment;
use crate::exec::ExitReason;
use crate::log::auth_warn;
use crate::sudo::Duration;
use crate::sudoers::{Authorization, DirChange, Policy, PreJudgementPolicy};
use crate::system::interface::UserId;
use crate::system::timestamp::{RecordScope, SessionRecordFile, TouchResult};
use crate::system::Process;

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
    fn pre_exec(&mut self, target_user: &str) -> Result<Environment, Error>;
    fn cleanup(&mut self);
}

pub struct Pipeline<Policy: PolicyPlugin, Auth: AuthPlugin> {
    pub policy: Policy,
    pub authenticator: Auth,
}

impl<Policy: PolicyPlugin, Auth: AuthPlugin> Pipeline<Policy, Auth> {
    pub fn run(mut self, cmd_opts: SudoOptions) -> Result<(), Error> {
        let pre = self.policy.init()?;
        let mut context = build_context(cmd_opts, &pre)?;

        let policy = self.policy.judge(pre, &context)?;
        let authorization = policy.authorization();
        let scope = RecordScope::for_process(&Process::new());

        match authorization {
            Authorization::Forbidden => {
                return Err(Error::auth(&format!(
                    "I'm sorry {}. I'm afraid I can't do that",
                    context.current_user.name
                )));
            }
            Authorization::Allowed {
                must_authenticate,
                prior_validity,
                allowed_attempts,
            } => {
                self.apply_policy_to_context(&mut context, &policy)?;

                let mut auth_status = determine_auth_status(
                    must_authenticate,
                    context.use_session_records,
                    scope,
                    context.current_user.uid,
                    &context.current_user.name,
                    prior_validity,
                );

                self.authenticator.init(&context)?;
                if auth_status.must_authenticate {
                    self.authenticator
                        .authenticate(context.non_interactive, allowed_attempts)?;
                    if let (Some(record_file), Some(scope)) = (&mut auth_status.record_file, scope)
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

        let additional_env = self.authenticator.pre_exec(&context.target_user.name)?;

        // build environment
        let current_env = std::env::vars_os().collect();
        let target_env =
            environment::get_target_environment(current_env, additional_env, &context, &policy);

        let pid = context.process.pid;

        // run command and return corresponding exit code
        let exec_result = if context.command.resolved {
            crate::exec::run_command(&context, target_env)
                .map_err(|io_error| Error::IoError(Some(context.command.command), io_error))
        } else {
            Err(Error::CommandNotFound(context.command.command))
        };

        self.authenticator.cleanup();

        let (reason, emulate_default_handler) = exec_result?;

        // Run any clean-up code before this line.
        emulate_default_handler();

        match reason {
            ExitReason::Code(code) => exit(code),
            ExitReason::Signal(signal) => {
                crate::system::kill(pid, signal)?;
            }
        }

        Ok(())
    }

    pub fn run_validate(mut self, cmd_opts: SudoOptions) -> Result<(), Error> {
        let scope = RecordScope::for_process(&Process::new());
        let pre = self.policy.init()?;
        let context = build_context(cmd_opts, &pre)?;

        match pre.validate_authorization() {
            Authorization::Forbidden => {
                return Err(Error::auth(&format!(
                    "I'm sorry {}. I'm afraid I can't do that",
                    context.current_user.name
                )));
            }
            Authorization::Allowed {
                must_authenticate,
                allowed_attempts,
                prior_validity,
            } => {
                let mut auth_status = determine_auth_status(
                    must_authenticate,
                    context.use_session_records,
                    scope,
                    context.current_user.uid,
                    &context.current_user.name,
                    prior_validity,
                );

                self.authenticator.init(&context)?;
                if auth_status.must_authenticate {
                    self.authenticator
                        .authenticate(context.non_interactive, allowed_attempts)?;
                    if let (Some(record_file), Some(scope)) = (&mut auth_status.record_file, scope)
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
                if context.chdir.is_some() {
                    return Err(Error::auth("no permission")); // TODO better user error messages
                } else {
                    context.chdir = optdir.map(std::path::PathBuf::from)
                }
            }
        }
        // override the default pty behaviour if indicated
        if !policy.use_pty() {
            context.use_pty = false
        }

        Ok(())
    }
}

fn build_context(cmd_opts: SudoOptions, pre: &dyn PreJudgementPolicy) -> Result<Context, Error> {
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
    current_user: &str,
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

struct AuthStatus<'a> {
    must_authenticate: bool,
    record_file: Option<SessionRecordFile<'a, File>>,
}

impl<'a> AuthStatus<'a> {
    fn new(
        must_authenticate: bool,
        record_file: Option<SessionRecordFile<'a, File>>,
    ) -> AuthStatus<'a> {
        AuthStatus {
            must_authenticate,
            record_file,
        }
    }
}
