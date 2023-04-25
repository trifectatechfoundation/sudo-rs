use std::fs::File;

use sudo_common::{error::Error, Context};
use sudo_log::auth_warn;
use sudo_system::{
    time::Duration,
    timestamp::{RecordScope, SessionRecordFile, TouchResult},
    Process, WithProcess,
};

use crate::pipeline::AuthPlugin;

/// Tries to determine a record match scope for the current context.
/// This should never produce an error since any actual error should just be
/// ignored and no session record file should be used in that case.
fn determine_record_scope(context: &Context) -> Option<RecordScope> {
    let tty = Process::tty_device_id(WithProcess::Current);
    if let Ok(Some(tty_device)) = tty {
        if let Ok(init_time) =
            Process::starting_time(WithProcess::Other(context.process.session_id))
        {
            Some(RecordScope::TTY {
                tty_device,
                session_pid: context.process.session_id,
                init_time,
            })
        } else {
            auth_warn!("Could not get terminal foreground process starting time");
            None
        }
    } else if let Some(parent_pid) = context.process.parent_pid {
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
    if let Some(record_for) = record_for {
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

pub fn authenticate(context: &Context) -> Result<(), Error> {
    let authenticate_for = &context.current_user.name;
    let target_user = context.target_user.uid;

    // init pam
    let mut pam = sudo_pam::PamContext::builder_cli(context.stdin)
        .target_user(authenticate_for)
        .service_name("sudo")
        .build()?;
    pam.mark_silent(true);
    pam.mark_allow_null_auth_token(false);

    // determine session limit
    let scope = determine_record_scope(context);

    // only if there is an interactive terminal or parent process we can store session information
    let (must_authenticate, records_file) = determine_auth_status(scope, context);

    if must_authenticate {
        pam.authenticate()?;
        if let (Some(mut session_records), Some(scope)) = (records_file, scope) {
            match session_records.create(scope, target_user) {
                Ok(_) => (),
                Err(e) => {
                    auth_warn!("Could not update session record file with new record: {e}");
                }
            }
        }
    }

    pam.validate_account()?;

    Ok(())
}

#[derive(Default)]
pub struct PamAuthenticator;

impl AuthPlugin for PamAuthenticator {
    fn init(&mut self, _context: &Context) -> Result<(), Error> {
        // TODO todo!()
        Ok(())
    }

    fn authenticate(&mut self, context: &Context) -> Result<(), Error> {
        authenticate(context)
    }

    fn pre_exec(&mut self, _context: &Context) -> Result<(), Error> {
        // TODO todo!()
        Ok(())
    }

    fn cleanup(&mut self) {
        // TODO
    }
}
