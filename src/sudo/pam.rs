use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::File;

use crate::common::context::LaunchType;
use crate::common::{error::Error, Context};
use crate::log::{auth_warn, dev_info, user_warn};
use crate::pam::{CLIConverser, Converser, PamContext, PamError, PamErrorType, PamResult};
use crate::system::term::current_tty_name;
use crate::system::{
    time::Duration,
    timestamp::{RecordScope, SessionRecordFile, TouchResult},
    Process, WithProcess,
};

use super::pipeline::AuthPlugin;

/// Tries to determine a record match scope for the current context.
/// This should never produce an error since any actual error should just be
/// ignored and no session record file should be used in that case.
pub fn determine_record_scope(process: &Process) -> Option<RecordScope> {
    let tty = Process::tty_device_id(WithProcess::Current);
    if let Ok(Some(tty_device)) = tty {
        if let Ok(init_time) = Process::starting_time(WithProcess::Other(process.session_id)) {
            Some(RecordScope::Tty {
                tty_device,
                session_pid: process.session_id,
                init_time,
            })
        } else {
            auth_warn!("Could not get terminal foreground process starting time");
            None
        }
    } else if let Some(parent_pid) = process.parent_pid {
        if let Ok(init_time) = Process::starting_time(WithProcess::Other(parent_pid)) {
            Some(RecordScope::Ppid {
                group_pid: parent_pid,
                init_time,
            })
        } else {
            auth_warn!("Could not get parent process starting time");
            None
        }
    } else {
        None
    }
}

/// This should determine what the authentication status for the given record
/// match limit and origin/target user from the context is.
fn determine_auth_status(
    record_for: Option<RecordScope>,
    context: &Context,
    prior_validity: Duration,
) -> (bool, Option<SessionRecordFile<File>>) {
    if let (true, Some(record_for)) = (context.use_session_records, record_for) {
        match SessionRecordFile::open_for_user(&context.current_user.name, prior_validity) {
            Ok(mut sr) => {
                match sr.touch(record_for, context.current_user.uid) {
                    // if a record was found and updated within the timeout, we do not need to authenticate
                    Ok(TouchResult::Updated { .. }) => (false, Some(sr)),
                    Ok(TouchResult::NotFound | TouchResult::Outdated { .. }) => (true, Some(sr)),
                    Err(e) => {
                        auth_warn!("Unexpected error while reading session information: {e}");
                        (true, None)
                    }
                }
            }
            // if we cannot open the session record file we just assume there is none and continue as normal
            Err(e) => {
                auth_warn!("Could not use session information: {e}");
                (true, None)
            }
        }
    } else {
        (true, None)
    }
}

type PamBuilder<C> = dyn Fn(&Context) -> PamResult<PamContext<C>>;

pub struct PamAuthenticator<C: Converser> {
    builder: Box<PamBuilder<C>>,
    pam: Option<PamContext<C>>,
}

impl<C: Converser> PamAuthenticator<C> {
    fn new(
        initializer: impl Fn(&Context) -> PamResult<PamContext<C>> + 'static,
    ) -> PamAuthenticator<C> {
        PamAuthenticator {
            builder: Box::new(initializer),
            pam: None,
        }
    }
}

impl PamAuthenticator<CLIConverser> {
    pub fn new_cli() -> PamAuthenticator<CLIConverser> {
        PamAuthenticator::new(|context| {
            let service_name = if matches!(context.launch, LaunchType::Login) {
                "sudo-i"
            } else {
                "sudo"
            };
            let mut pam = PamContext::builder_cli("sudo", context.stdin, context.non_interactive)
                .target_user(&context.current_user.name)
                .service_name(service_name)
                .build()?;
            pam.mark_silent(true);
            pam.mark_allow_null_auth_token(false);
            Ok(pam)
        })
    }
}

impl<C: Converser> AuthPlugin for PamAuthenticator<C> {
    fn init(&mut self, context: &Context) -> Result<(), Error> {
        self.pam = Some((self.builder)(context)?);
        Ok(())
    }

    fn authenticate(
        &mut self,
        context: &Context,
        prior_validity: Duration,
        mut max_tries: u16,
    ) -> Result<(), Error> {
        let pam = self
            .pam
            .as_mut()
            .expect("Pam must be initialized before authenticate");
        pam.set_user(&context.current_user.name)?;
        pam.set_requesting_user(&context.current_user.name)?;

        // attempt to set the TTY this session is communicating on
        if let Ok(pam_tty) = current_tty_name() {
            pam.set_tty(&pam_tty)?;
        }

        // determine session limit
        let scope = determine_record_scope(&context.process);

        // only if there is an interactive terminal or parent process we can store session information
        let (must_authenticate, records_file) =
            determine_auth_status(scope, context, prior_validity);

        if must_authenticate {
            let mut current_try = 0;
            loop {
                current_try += 1;
                match pam.authenticate() {
                    // there was no error, so authentication succeeded
                    Ok(_) => break,

                    // maxtries was reached, pam does not allow any more tries
                    Err(PamError::Pam(PamErrorType::MaxTries, _)) => {
                        return Err(Error::MaxAuthAttempts(current_try));
                    }

                    // there was an authentication error, we can retry
                    Err(PamError::Pam(PamErrorType::AuthError, _)) => {
                        max_tries -= 1;
                        if max_tries == 0 {
                            return Err(Error::MaxAuthAttempts(current_try));
                        } else if context.non_interactive {
                            return Err(Error::Authentication("interaction required".to_string()));
                        } else {
                            user_warn!("Authentication failed, try again.");
                        }
                    }

                    // there was another pam error, return the error
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }
            if let (Some(mut session_records), Some(scope)) = (records_file, scope) {
                match session_records.create(scope, context.current_user.uid) {
                    Ok(_) => (),
                    Err(e) => {
                        auth_warn!("Could not update session record file with new record: {e}");
                    }
                }
            }
        }

        Ok(())
    }

    fn pre_exec(&mut self, context: &Context) -> Result<HashMap<OsString, OsString>, Error> {
        let pam = self
            .pam
            .as_mut()
            .expect("Pam must be initialized before pre_exec");

        // make sure that the user that needed to authenticate has a valid token
        pam.validate_account_or_change_auth_token()?;

        // switch pam over to the target user
        pam.set_user(&context.target_user.name)?;

        // make sure that credentials are loaded for the target user
        // errors are ignored because not all modules support this functionality
        if let Err(e) = pam.credentials_reinitialize() {
            dev_info!(
                "PAM gave an error while trying to re-initialize credentials: {:?}",
                e
            );
        }

        pam.open_session()?;

        let env_vars = pam.env()?;

        Ok(env_vars)
    }

    fn cleanup(&mut self) {
        let pam = self
            .pam
            .as_mut()
            .expect("Pam must be initialized before cleanup");

        // closing the pam session is best effort, if any error occurs we cannot
        // do anything with it
        let _ = pam.close_session();
    }
}
