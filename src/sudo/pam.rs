use std::collections::HashMap;
use std::ffi::OsString;

use crate::common::context::LaunchType;
use crate::common::{error::Error, Context};
use crate::log::{dev_info, user_warn};
use crate::pam::{CLIConverser, Converser, PamContext, PamError, PamErrorType, PamResult};
use crate::system::term::current_tty_name;

use super::pipeline::AuthPlugin;

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
            init_pam(
                matches!(context.launch, LaunchType::Login),
                matches!(context.launch, LaunchType::Shell),
                context.stdin,
                context.non_interactive,
                &context.current_user.name,
                &context.current_user.name,
            )
        })
    }
}

impl<C: Converser> AuthPlugin for PamAuthenticator<C> {
    fn init(&mut self, context: &Context) -> Result<(), Error> {
        self.pam = Some((self.builder)(context)?);
        Ok(())
    }

    fn authenticate(&mut self, non_interactive: bool, max_tries: u16) -> Result<(), Error> {
        let pam = self
            .pam
            .as_mut()
            .expect("Pam must be initialized before authenticate");

        attempt_authenticate(pam, non_interactive, max_tries)?;

        Ok(())
    }

    fn pre_exec(&mut self, target_user: &str) -> Result<HashMap<OsString, OsString>, Error> {
        let pam = self
            .pam
            .as_mut()
            .expect("Pam must be initialized before pre_exec");

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

pub fn init_pam(
    is_login_shell: bool,
    is_shell: bool,
    use_stdin: bool,
    non_interactive: bool,
    auth_user: &str,
    requesting_user: &str,
) -> PamResult<PamContext<CLIConverser>> {
    let service_name = if is_login_shell { "sudo-i" } else { "sudo" };
    let mut pam = PamContext::builder_cli("sudo", use_stdin, non_interactive)
        .service_name(service_name)
        .build()?;
    pam.mark_silent(!is_shell && !is_login_shell);
    pam.mark_allow_null_auth_token(false);
    pam.set_requesting_user(requesting_user)?;
    pam.set_user(auth_user)?;

    // attempt to set the TTY this session is communicating on
    if let Ok(pam_tty) = current_tty_name() {
        pam.set_tty(&pam_tty)?;
    }

    Ok(pam)
}

pub fn attempt_authenticate<C: Converser>(
    pam: &mut PamContext<C>,
    non_interactive: bool,
    mut max_tries: u16,
) -> Result<(), Error> {
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
                } else if non_interactive {
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

    Ok(())
}
