use std::{
    ffi::{CStr, CString},
    time::Duration,
};

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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CredentialsAction {
    Establish,
    Delete,
    Reinitialize,
    Refresh,
}

impl CredentialsAction {
    pub fn as_int(&self) -> libc::c_int {
        use CredentialsAction::*;

        match self {
            Establish => PAM_ESTABLISH_CRED as libc::c_int,
            Delete => PAM_DELETE_CRED as libc::c_int,
            Reinitialize => PAM_REINITIALIZE_CRED as libc::c_int,
            Refresh => PAM_REFRESH_CRED as libc::c_int,
        }
    }
}

impl<C: Converser> PamContextBuilder<C> {
    pub fn build(self) -> Result<PamContext<C>, PamError> {
        if let (Some(converser), Some(service_name)) = (self.converser, self.service_name) {
            let c_service_name = CString::new(service_name)?;
            let c_user = self.target_user.map(CString::new).transpose()?;
            let c_user_ptr = match c_user {
                Some(ref c) => c.as_ptr(),
                None => std::ptr::null(),
            };

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
        let c_user = CString::new(user)?;
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
        let c_user = CString::new(user)?;
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
        let c_host = CString::new(host)?;
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

    /// Establish credentials to be stored in PAM
    pub fn credentials_establish(&mut self) -> Result<(), PamError> {
        self.credentials(CredentialsAction::Establish)
    }

    /// Delete the credentials stored in PAM
    pub fn credentials_delete(&mut self) -> Result<(), PamError> {
        self.credentials(CredentialsAction::Delete)
    }

    /// Re-initialize the credentials stored in PAM
    pub fn credentials_reinitialize(&mut self) -> Result<(), PamError> {
        self.credentials(CredentialsAction::Reinitialize)
    }

    /// Refresh the credentials stored in PAM
    pub fn credentials_refresh(&mut self) -> Result<(), PamError> {
        self.credentials(CredentialsAction::Refresh)
    }

    /// Updates to the credentials stored in PAM
    pub fn credentials(&mut self, action: CredentialsAction) -> Result<(), PamError> {
        let mut flags = action.as_int();
        flags |= self.silent_flag();

        self.pam_err(unsafe { pam_setcred(self.pamh, flags) })?;

        Ok(())
    }

    /// Ask the user to change the authentication token (password).
    ///
    /// If `expired_only` is set to true, only expired authentication tokens
    /// will be asked to be replaced, otherwise a replacement will always be
    /// requested.
    pub fn change_auth_token(&mut self, expired_only: bool) -> Result<(), PamError> {
        let mut flags = 0;
        flags |= self.silent_flag();
        if expired_only {
            flags |= PAM_CHANGE_EXPIRED_AUTHTOK as i32;
        }
        self.pam_err(unsafe { pam_chauthtok(self.pamh, flags) })?;
        Ok(())
    }

    /// Start a user session for the authenticated user.
    pub fn open_session(&mut self) -> Result<(), PamError> {
        if !self.session_started {
            self.pam_err(unsafe { pam_open_session(self.pamh, self.silent_flag()) })?;
            self.session_started = true;
            Ok(())
        } else {
            Err(PamError::SessionAlreadyOpen)
        }
    }

    /// End the user session.
    pub fn close_session(&mut self) -> Result<(), PamError> {
        if self.session_started {
            self.pam_err(unsafe { pam_close_session(self.pamh, self.silent_flag()) })?;
            self.session_started = true;
            Ok(())
        } else {
            Err(PamError::SessionNotOpen)
        }
    }

    /// Set an environment variable in the PAM environment
    pub fn set_env(&mut self, variable: &str, value: &str) -> Result<(), PamError> {
        let env = format!("{variable}={value}");
        let c_env = CString::new(env)?;
        self.pam_err(unsafe { pam_putenv(self.pamh, c_env.as_ptr()) })?;
        Ok(())
    }

    /// Remove an environment variable in the PAM environment
    pub fn unset_env(&mut self, variable: &str) -> Result<(), PamError> {
        let c_env = CString::new(variable)?;
        self.pam_err(unsafe { pam_putenv(self.pamh, c_env.as_ptr()) })?;

        Ok(())
    }

    /// Get an environment variable from the PAM environment
    pub fn get_env(&mut self, variable: &str) -> Option<String> {
        let c_env = CString::new(variable).expect("String contained nul bytes");
        let c_res = unsafe { pam_getenv(self.pamh, c_env.as_ptr()) };
        if c_res.is_null() {
            None
        } else {
            Some(unsafe { string_from_ptr(c_res) })
        }
    }

    /// Get a full listing of the current PAM environment
    pub fn env(&mut self) -> Result<Vec<(String, String)>, PamError> {
        let mut res = vec![];
        let envs = unsafe { pam_getenvlist(self.pamh) };
        let mut curr_env = envs;
        while unsafe { !(*curr_env).is_null() } {
            let curr_str = unsafe { *curr_env };
            let data = {
                let cstr = unsafe { CStr::from_ptr(curr_str) };
                if let Some((key, value)) = cstr.to_string_lossy().split_once('=') {
                    Some((String::from(key), String::from(value)))
                } else {
                    None
                }
            };
            if let Some(kv) = data {
                res.push(kv);
            }

            // free the current string and move to the next one
            unsafe { libc::free(curr_str as *mut libc::c_void) };
            curr_env = unsafe { curr_env.offset(1) };
        }

        // free the entire array
        unsafe { libc::free(envs as *mut libc::c_void) };

        Ok(res)
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
