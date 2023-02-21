use std::{ffi::CString, time::Duration};

use converse::{Converser, ConverserData};
use error::pam_err;
pub use error::{PamError, PamErrorType};
use sudo_cutils::string_from_ptr;
use sudo_pam_sys::*;

mod converse;
mod error;

pub use converse::CLIConverser;

pub struct PamContext<C: Converser> {
    data: ConverserData<C>,
    pam_conv: Option<pam_conv>,
    pamh: *mut pam_handle_t,
    silent: bool,
    allow_null_auth_token: bool,
    last_pam_status: Option<libc::c_int>,
    session_started: bool,
}

pub struct PamContextBuilder<C> {
    converser: Option<C>,
    service_name: Option<String>,
    target_user: Option<String>,
}

impl<C: Converser> PamContextBuilder<C> {
    pub fn build(self) -> Result<PamContext<C>, PamError> {
        if let (Some(converser), Some(service_name)) = (self.converser, self.service_name) {
            let c_service_name = CString::new(service_name)?;
            let c_user = self.target_user.map(CString::new).transpose()?;
            let c_user_ptr = c_user.map(|cu| cu.as_ptr()).unwrap_or_else(std::ptr::null);

            let data = ConverserData {
                converser,
                panicked: false,
            };
            let mut context = PamContext {
                data,
                pam_conv: None,
                pamh: std::ptr::null_mut(),
                silent: false,
                allow_null_auth_token: true,
                last_pam_status: None,
                session_started: false,
            };
            context.pam_conv = Some(unsafe { context.data.create_pam_conv() });

            pam_err(
                unsafe {
                    pam_start(
                        c_service_name.as_ptr(),
                        c_user_ptr,
                        &context.pam_conv.unwrap(),
                        &mut context.pamh,
                    )
                },
                context.pamh,
            )?;

            Ok(context)
        } else {
            Err(PamError::InvalidState)
        }
    }

    pub fn converser(mut self, converser: C) -> PamContextBuilder<C> {
        self.converser = Some(converser);
        self
    }

    pub fn service_name<T: Into<String>>(mut self, name: T) -> PamContextBuilder<C> {
        self.service_name = Some(name.into());
        self
    }

    pub fn target_user<T: Into<String>>(mut self, user: T) -> PamContextBuilder<C> {
        self.target_user = Some(user.into());
        self
    }

    pub fn clear_target_user(mut self) -> PamContextBuilder<C> {
        self.target_user = None;
        self
    }
}

impl<C> Default for PamContextBuilder<C> {
    fn default() -> Self {
        Self {
            converser: None,
            service_name: None,
            target_user: None,
        }
    }
}

impl<C: Converser> PamContext<C> {
    /// Create a new builder that can be used to create a new context.
    pub fn builder() -> PamContextBuilder<C> {
        PamContextBuilder::default()
    }

    /// Error handling function that also stores the last error in the struct
    /// for correct handling of the shutdown function.
    fn pam_err(&mut self, err: libc::c_int) -> Result<(), PamError> {
        self.last_pam_status = Some(err);
        pam_err(err, self.pamh)
    }

    /// Set whether output of pam calls should be silent or not, by default
    /// PAM calls are not silent.
    pub fn mark_silent(&mut self, silent: bool) {
        self.silent = silent;
    }

    /// Set whether or not to allow empty authentication tokens, by default such
    /// tokens are allowed.
    pub fn mark_allow_null_auth_token(&mut self, allow: bool) {
        self.allow_null_auth_token = allow;
    }

    /// Get the PAM flag value for the silent flag
    fn silent_flag(&self) -> i32 {
        if self.silent {
            PAM_SILENT as i32
        } else {
            0
        }
    }

    /// Get the PAM flag value for the disallow_null_authtok flag
    fn disallow_null_auth_token_flag(&self) -> i32 {
        if self.allow_null_auth_token {
            0
        } else {
            PAM_DISALLOW_NULL_AUTHTOK as i32
        }
    }

    /// Run authentication for the account
    pub fn authenticate(&mut self) -> Result<(), PamError> {
        let mut flags = 0;
        flags |= self.silent_flag();
        flags |= self.disallow_null_auth_token_flag();

        self.pam_err(unsafe { pam_authenticate(self.pamh, flags) })?;
        Ok(())
    }

    /// Check that the account is valid
    pub fn validate_account(&mut self) -> Result<(), PamError> {
        let mut flags = 0;
        flags |= self.silent_flag();
        flags |= self.disallow_null_auth_token_flag();

        self.pam_err(unsafe { pam_acct_mgmt(self.pamh, flags) })?;
        Ok(())
    }

    /// Attempt to validate the account, if that fails because the authentication
    /// token is outdated, then an update of the authentication token is requested.
    pub fn validate_account_or_change_auth_token(&mut self) -> Result<(), PamError> {
        let check_val = self.validate_account();
        match check_val {
            Ok(()) => Ok(()),
            Err(PamError::Pam(PamErrorType::NewAuthTokenRequired, _)) => {
                self.change_auth_token(true)?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Request a delay when an authentication failure occured in the PAM stack.
    pub fn request_failure_delay(&mut self, delay: Duration) -> Result<(), PamError> {
        let delay = delay.as_micros();
        let delay = delay.min(libc::c_uint::MAX as u128) as libc::c_uint;
        self.pam_err(unsafe { pam_fail_delay(self.pamh, delay) })?;
        Ok(())
    }

    /// Set the user that is requesting the authentication.
    pub fn set_requesting_user(&mut self, user: &str) -> Result<(), PamError> {
        let c_user = CString::new(user).expect("String contained null bytes");
        self.pam_err(unsafe {
            pam_set_item(
                self.pamh,
                PAM_RUSER as i32,
                c_user.as_ptr() as *const libc::c_void,
            )
        })?;
        Ok(())
    }

    /// Clear the user that is requesting the authentication.
    pub fn clear_requesting_user(&mut self) -> Result<(), PamError> {
        self.pam_err(unsafe { pam_set_item(self.pamh, PAM_RUSER as i32, std::ptr::null()) })?;
        Ok(())
    }

    /// Set the user that will be authenticated.
    pub fn set_user(&mut self, user: &str) -> Result<(), PamError> {
        let c_user = CString::new(user).expect("String contained null bytes");
        self.pam_err(unsafe {
            pam_set_item(
                self.pamh,
                PAM_USER as i32,
                c_user.as_ptr() as *const libc::c_void,
            )
        })?;
        Ok(())
    }

    /// Clear the user that will be authenticated
    pub fn clear_user(&mut self) -> Result<(), PamError> {
        self.pam_err(unsafe { pam_set_item(self.pamh, PAM_USER as i32, std::ptr::null()) })?;
        Ok(())
    }

    /// Get the user that will be/was authenticated.
    ///
    /// Note that PAM modules might change the authenticated user, so you should
    /// read this after authentication was completed to make sure what the
    /// authenticated user is.
    pub fn get_user(&mut self) -> Result<String, PamError> {
        let mut ptr = std::ptr::null();
        self.pam_err(unsafe { pam_get_item(self.pamh, PAM_USER as i32, &mut ptr) })?;
        Ok(unsafe { string_from_ptr(ptr as *const libc::c_char) })
    }

    /// Set the host that is requesting authentication
    pub fn set_requesting_host(&mut self, host: &str) -> Result<(), PamError> {
        let c_host = CString::new(host).expect("String contained null bytes");
        self.pam_err(unsafe {
            pam_set_item(
                self.pamh,
                PAM_RHOST as i32,
                c_host.as_ptr() as *const libc::c_void,
            )
        })?;
        Ok(())
    }

    /// Clear the host that is requesting authentication
    pub fn clear_requesting_host(&mut self) -> Result<(), PamError> {
        self.pam_err(unsafe { pam_set_item(self.pamh, PAM_RHOST as i32, std::ptr::null()) })?;
        Ok(())
    }

    // TODO: figure out what pam_setcred is supposed to do, and how the flags to it are used
    // TODO: implement pam_setcred in a way that makes sense in our Rust context

    /// Ask the user to change the authentication token (password).
    ///
    /// If `expired_only` is set to true, only expired authentication tokens
    /// will be asked to be replaced, otherwise a replacement will always be
    /// requested.
    pub fn change_auth_token(&mut self, expired_only: bool) -> Result<(), PamError> {
        todo!("pam_chauthtok")
    }

    /// Start a user session for the authenticated user.
    pub fn open_session(&mut self) -> Result<(), PamError> {
        self.session_started = true;
        todo!("pam_open_session")
    }

    /// End the user session.
    pub fn close_session(&mut self) -> Result<(), PamError> {
        self.session_started = false;
        todo!("pam_close_session")
    }

    /// Set an environment variable in the PAM environment
    pub fn set_env(&mut self, variable: &str, value: &str) -> Result<(), PamError> {
        todo!("pam_putenv add")
    }

    /// Remove an environment variable in the PAM environment
    pub fn unset_env(&mut self, variable: &str) -> Result<(), PamError> {
        todo!("pam_putenv delete")
    }

    /// Get an environment variable from the PAM environment
    pub fn get_env(&mut self, variable: &str) -> Result<Option<String>, PamError> {
        todo!("pam_getenv")
    }

    /// Get a full listing of the current PAM environment
    pub fn env(&mut self) -> Result<Vec<(String, String)>, PamError> {
        todo!("pam_getenvlist")
    }
}

impl PamContext<CLIConverser> {
    pub fn builder_cli() -> PamContextBuilder<CLIConverser> {
        PamContextBuilder::default().converser(CLIConverser)
    }
}

impl<C: Converser> Drop for PamContext<C> {
    fn drop(&mut self) {
        if !self.pamh.is_null() {
            if !self.session_started {
                let _ = self.close_session();
            }

            // It looks like PAM_DATA_SILENT is important to set for our sudo context, but
            // it is unclear what it really does and does not do, other than the vague
            // documentation description to 'not take the call to seriously'
            // Also see https://github.com/systemd/systemd/issues/22318
            unsafe {
                pam_end(
                    self.pamh,
                    self.last_pam_status.unwrap_or(PAM_SUCCESS as libc::c_int),
                ) | PAM_DATA_SILENT as i32
            };
        }
        self.pam_conv = None;
    }
}
