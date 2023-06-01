use std::fs::File;

use sudo_common::{error::Error, Context};
use sudo_log::{auth_warn, user_warn};
use sudo_pam::{CLIConverser, Converser, PamContext, PamError, PamErrorType, PamResult};
use sudo_system::{
    time::Duration,
    timestamp::{RecordScope, SessionRecordFile, TouchResult},
    Process, WithProcess,
};

use crate::pipeline::AuthPlugin;

/// Tries to determine a record match scope for the current context.
/// This should never produce an error since any actual error should just be
/// ignored and no session record file should be used in that case.
pub fn determine_record_scope(process: &Process) -> Option<RecordScope> {
    let tty = Process::tty_device_id(WithProcess::Current);
    if let Ok(Some(tty_device)) = tty {
        if let Ok(init_time) = Process::starting_time(WithProcess::Other(process.session_id)) {
            Some(RecordScope::TTY {
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
            Some(RecordScope::PPID {
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
) -> (bool, Option<SessionRecordFile<File>>) {
    if let (true, Some(record_for)) = (context.use_session_records, record_for) {
        match SessionRecordFile::open_for_user(&context.current_user.name, Duration::minutes(15)) {
            Ok(mut sr) => {
                match sr.touch(record_for, context.target_user.uid) {
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
            let mut pam = PamContext::builder_cli(context.stdin)
                .target_user(&context.current_user.name)
                .service_name("sudo")
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

    fn authenticate(&mut self, context: &Context) -> Result<(), Error> {
        let pam = self
            .pam
            .as_mut()
            .expect("Pam must be initialized before authenticate");
        pam.set_user(&context.current_user.name)?;

        // determine session limit
        let scope = determine_record_scope(&context.process);

        // only if there is an interactive terminal or parent process we can store session information
        let (must_authenticate, records_file) = determine_auth_status(scope, context);

        if must_authenticate {
            let mut max_tries = 3;
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
                match session_records.create(scope, context.target_user.uid) {
                    Ok(_) => (),
                    Err(e) => {
                        auth_warn!("Could not update session record file with new record: {e}");
                    }
                }
            }
        }

        Ok(())
    }

    fn pre_exec(&mut self, _context: &Context) -> Result<(), Error> {
        let pam = self
            .pam
            .as_mut()
            .expect("Pam must be initialized before pre_exec");

        pam.validate_account()?;
        pam.open_session()?;
        Ok(())
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
