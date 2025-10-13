use std::{ffi::OsString, path::PathBuf};

use crate::common::context::LaunchType;
use crate::common::error::Error;
use crate::log::{dev_info, user_warn};
use crate::pam::{PamContext, PamError, PamErrorType};
use crate::system::{term::current_tty_name, time::Duration};

#[cfg(target_os = "freebsd")]
const PAM_SERVICE_DIRS: &[&str] = &["/usr/local/etc/pam.d", "/etc/pam.d"];

#[cfg(not(target_os = "freebsd"))]
const PAM_SERVICE_DIRS: &[&str] = &["/etc/pam.d"];

fn pam_service_candidates(service_name: &str) -> Vec<PathBuf> {
    PAM_SERVICE_DIRS
        .iter()
        .map(|dir| PathBuf::from(dir).join(service_name))
        .collect()
}

fn ensure_pam_service_config(service_name: &str) -> Result<(), Error> {
    let candidates = pam_service_candidates(service_name);

    if candidates.iter().any(|path| path.exists()) {
        return Ok(());
    }

    let message = if candidates.len() == 1 {
        format!(
            "PAM configuration for service '{service_name}' not found at {}; This file is needed to run sudo-rs. Uninstalling sudo after installing sudo-rs might cause this problem on some distributions like arch linux due to a packaging issue. See https://github.com/trifectatechfoundation/sudo-rs/issues/1182",
            candidates[0].display()
        )
    } else {
        let searched = candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "PAM configuration for service '{service_name}' not found in any of: {searched}; This file is needed to run sudo-rs. Uninstalling sudo after installing sudo-rs might cause this problem on some distributions like arch linux due to a packaging issue. See https://github.com/trifectatechfoundation/sudo-rs/issues/1182"
        )
    };

    Err(Error::Configuration {
        message,
        path: candidates.first().cloned(),
    })
}

pub(super) struct InitPamArgs<'a> {
    pub(super) launch: LaunchType,
    pub(super) use_stdin: bool,
    pub(super) bell: bool,
    pub(super) non_interactive: bool,
    pub(super) password_feedback: bool,
    pub(super) password_timeout: Option<Duration>,
    pub(super) auth_prompt: Option<String>,
    pub(super) auth_user: &'a str,
    pub(super) requesting_user: &'a str,
    pub(super) target_user: &'a str,
    pub(super) hostname: &'a str,
}

pub(super) fn init_pam(
    InitPamArgs {
        launch,
        use_stdin,
        bell,
        non_interactive,
        password_feedback,
        password_timeout,
        auth_prompt,
        auth_user,
        requesting_user,
        target_user,
        hostname,
    }: InitPamArgs,
) -> Result<PamContext, Error> {
    let service_name = match launch {
        LaunchType::Login if cfg!(feature = "pam-login") => "sudo-i",
        LaunchType::Login | LaunchType::Shell | LaunchType::Direct => "sudo",
    };

    ensure_pam_service_config(service_name)?;
    let mut pam = PamContext::new_cli(
        "sudo",
        service_name,
        use_stdin,
        bell,
        non_interactive,
        password_feedback,
        password_timeout,
        Some(auth_user),
    )
    .map_err(Error::from)?;
    pam.mark_silent(matches!(launch, LaunchType::Direct));
    pam.mark_allow_null_auth_token(false);
    pam.set_requesting_user(requesting_user)?;

    match auth_prompt.as_deref() {
        None => {}
        Some("") => pam.set_auth_prompt(None),
        Some(auth_prompt) => {
            let mut final_prompt = String::new();
            let mut chars = auth_prompt.chars();
            while let Some(c) = chars.next() {
                if c != '%' {
                    final_prompt.push(c);
                    continue;
                }
                match chars.next() {
                    Some('H') => final_prompt.push_str(hostname),
                    Some('h') => final_prompt
                        .push_str(hostname.split_once('.').map(|x| x.0).unwrap_or(hostname)),
                    Some('p') => final_prompt.push_str(auth_user),
                    Some('U') => final_prompt.push_str(target_user),
                    Some('u') => final_prompt.push_str(requesting_user),
                    Some('%') | None => final_prompt.push('%'),
                    Some(c) => {
                        final_prompt.push('%');
                        final_prompt.push(c);
                    }
                }
            }
            pam.set_auth_prompt(Some(final_prompt));
        }
    }

    // attempt to set the TTY this session is communicating on
    if let Ok(pam_tty) = current_tty_name() {
        pam.set_tty(&pam_tty)?;
    }

    Ok(pam)
}

pub(super) fn attempt_authenticate(
    pam: &mut PamContext,
    auth_user: &str,
    non_interactive: bool,
    mut max_tries: u16,
) -> Result<(), Error> {
    let mut current_try = 0;
    loop {
        current_try += 1;
        match pam.authenticate(auth_user) {
            // there was no error, so authentication succeeded
            Ok(_) => break,

            // maxtries was reached, pam does not allow any more tries
            Err(PamError::Pam(PamErrorType::MaxTries)) => {
                return Err(Error::MaxAuthAttempts(current_try));
            }

            // there was an authentication error, we can retry
            Err(PamError::Pam(PamErrorType::AuthError | PamErrorType::ConversationError)) => {
                max_tries -= 1;
                if max_tries == 0 {
                    return Err(Error::MaxAuthAttempts(current_try));
                } else if non_interactive {
                    return Err(Error::InteractionRequired);
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

    Ok(())
}

pub(super) fn pre_exec(
    pam: &mut PamContext,
    target_user: &str,
) -> Result<Vec<(OsString, OsString)>, Error> {
    // make sure that the user that needed to authenticate has a valid token
    pam.validate_account_or_change_auth_token()?;

    // check what the current user in PAM is
    let user = pam.get_user()?;
    if user != target_user {
        // switch pam over to the target user
        pam.set_user(target_user)?;

        // make sure that credentials are loaded for the target user
        // errors are ignored because not all modules support this functionality
        if let Err(e) = pam.credentials_reinitialize() {
            dev_info!(
                "PAM gave an error while trying to re-initialize credentials: {:?}",
                e
            );
        }
    }

    pam.open_session()?;

    let env_vars = pam.env()?;

    Ok(env_vars)
}
