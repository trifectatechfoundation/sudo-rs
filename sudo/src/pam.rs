use sudo_common::{error::Error, Context};
use sudo_system::{
    time::Duration,
    timestamp::{RecordLimit, RecordMatch, SessionRecordFile},
    Process,
};

pub fn authenticate(context: &Context) -> Result<(), Error> {
    let authenticate_for = &context.current_user.name;
    let target_user = context.target_user.uid;

    // determine session limit
    let record_for = if let Some(tty_device) = Process::tty_device_id(None)? {
        Some(RecordLimit::TTY {
            tty_device,
            session_pid: context.process.session_id,
            init_time: context.process.session_starting_time()?,
        })
    } else if let Some(parent_pid) = context.process.parent_pid {
        Some(RecordLimit::PPID {
            group_pid: parent_pid,
            init_time: Process::starting_time(Some(parent_pid))?,
        })
    } else {
        None
    };

    // init pam
    let mut pam = sudo_pam::PamContext::builder_cli()
        .target_user(authenticate_for)
        .service_name("sudo")
        .build()?;
    pam.mark_silent(true);
    pam.mark_allow_null_auth_token(false);

    // only if there is an interactive terminal or parent process we can store session information
    let (must_authenticate, records_file) = if let Some(record_for) = record_for {
        let mut session_records =
            SessionRecordFile::open_for_user(authenticate_for, Duration::minutes(15))?;

        // try and find an entry and update it to the current timestamp
        let must_authenticate = match session_records.touch(record_for, target_user)? {
            RecordMatch::Found { .. } | RecordMatch::Updated { .. } => false,
            RecordMatch::NotFound | RecordMatch::Removed { .. } | RecordMatch::Outdated { .. } => {
                true
            }
        };
        (must_authenticate, Some(session_records))
    } else {
        (true, None)
    };

    if must_authenticate {
        pam.authenticate()?;
        if let (Some(mut session_records), Some(record_for)) = (records_file, record_for) {
            session_records.create_or_update(record_for, target_user)?;
        }
    }

    pam.validate_account()?;

    Ok(())
}
