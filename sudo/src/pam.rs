use sudo_common::{error::Error, Context};
use sudo_system::{
    time::Duration,
    timestamp::{RecordLimit, RecordMatch, SessionRecordFile},
    Process,
};

pub fn authenticate(context: &Context) -> Result<(), Error> {
    let authenticate_for = &context.current_user.name;
    let mut session_records =
        SessionRecordFile::open_for_user(authenticate_for, Duration::minutes(15))?;
    let record_for = if let Some(tty_device) = Process::tty_device_id(None)? {
        RecordLimit::TTY {
            tty_device,
            session_pid: context.process.session_id,
            init_time: context.process.session_starting_time()?,
        }
    } else {
        RecordLimit::PPID {
            group_pid: context.process.parent_pid,
            init_time: context.process.parent_process_starting_time()?,
        }
    };
    let target_user = context.target_user.uid;

    // try and find an entry and update it to the current timestamp
    let must_authenticate = match session_records.touch(record_for, target_user)? {
        RecordMatch::Found { .. } | RecordMatch::Updated { .. } => false,
        RecordMatch::NotFound | RecordMatch::Removed { .. } | RecordMatch::Outdated { .. } => true,
    };

    let mut pam = sudo_pam::PamContext::builder_cli()
        .target_user(authenticate_for)
        .service_name("sudo")
        .build()?;
    pam.mark_silent(true);
    pam.mark_allow_null_auth_token(false);

    if must_authenticate {
        pam.authenticate()?;
        session_records.create_or_update(record_for, target_user)?;
    }

    pam.validate_account()?;

    Ok(())
}
