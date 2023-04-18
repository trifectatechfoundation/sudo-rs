use std::{
    ffi::{CStr, CString},
    time::Duration,
};

use converse::{Converser, ConverserData};
use error::pam_err;
pub use error::{PamError, PamErrorType, PamResult};
use sudo_cutils::string_from_ptr;
use sudo_pam_sys::*;

mod converse;
mod error;
mod rpassword;
mod securemem;

pub use converse::CLIConverser;

pub struct PamContext<'a, C: Converser> {
    data_ptr: *mut ConverserData<C>,
    pamh: Option<&'a mut pam_handle_t>,
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
    /// Build the PamContext based on the current configuration.
    ///
    /// This function will error when the required settings have not yet been
    /// set, or when initialization of the PAM session somehow failed.
    pub fn build<'a>(self) -> PamResult<PamContext<'a, C>> {
        if let (Some(converser), Some(service_name)) = (self.converser, self.service_name) {
            let c_service_name = CString::new(service_name)?;
            let c_user = self.target_user.map(CString::new).transpose()?;
            let c_user_ptr = match c_user {
                Some(ref c) => c.as_ptr(),
                None => std::ptr::null(),
            };

            // this will be de-allocated explicitly in this type's drop method
            let data_ptr = Box::into_raw(Box::new(ConverserData {
                converser,
                panicked: false,
            }));

            let mut context = PamContext {
                data_ptr,
                pamh: None,
                silent: false,
                allow_null_auth_token: true,
                last_pam_status: None,
                session_started: false,
            };

            let mut pamh = std::ptr::null_mut();
            let res = unsafe {
                pam_start(
                    c_service_name.as_ptr(),
                    c_user_ptr,
                    &pam_conv {
                        conv: Some(converse::converse::<C>),
                        appdata_ptr: data_ptr as *mut libc::c_void,
                    },
                    &mut pamh,
                )
            };

            pam_err(res, pamh)?;

            if pamh.is_null() {
                Err(PamError::InvalidState)
            } else {
                context.pamh = Some(unsafe { &mut *pamh });
                Ok(context)
            }
        } else {
            Err(PamError::InvalidState)
        }
    }

    /// Set a converser implementation that will be used for the PAM conversation.
    pub fn converser(mut self, converser: C) -> PamContextBuilder<C> {
        self.converser = Some(converser);
        self
    }

    /// Set the service name for the PAM session.
    ///
    /// Note that the service name should be based on a static string and not
    /// based on the name of the binary.
    pub fn service_name<T: Into<String>>(mut self, name: T) -> PamContextBuilder<C> {
        self.service_name = Some(name.into());
        self
    }

    /// Set a target user that should be inserted into the pam context.
    ///
    /// The target user is optional and may also be set after the context was
    /// constructed or not set at all in which case PAM will ask for a
    /// username.
    pub fn target_user<T: Into<String>>(mut self, user: T) -> PamContextBuilder<C> {
        self.target_user = Some(user.into());
        self
    }

    /// Remove the target user if one was previously set.
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

impl<'a, C: Converser> PamContext<'a, C> {
    /// Get the pam handle or an error if there is no valid pam handle
    /// in our current context.
    fn handle(&mut self) -> PamResult<&mut pam_handle_t> {
        match &mut self.pamh {
            Some(h) => Ok(*h),
            None => Err(PamError::InvalidState),
        }
    }

    /// Create a new builder that can be used to create a new context.
    pub fn builder() -> PamContextBuilder<C> {
        PamContextBuilder::default()
    }

    /// Error handling function that also stores the last error in the struct
    /// for correct handling of the shutdown function.
    fn pam_err(&mut self, err: libc::c_int) -> PamResult<()> {
        self.last_pam_status = Some(err);
        let ptr = match &mut self.pamh {
            Some(h) => *h,
            None => std::ptr::null_mut(),
        };
        pam_err(err, ptr)
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
    pub fn authenticate(&mut self) -> PamResult<()> {
        let mut flags = 0;
        flags |= self.silent_flag();
        flags |= self.disallow_null_auth_token_flag();

        let res = unsafe { pam_authenticate(self.handle()?, flags) };
        self.pam_err(res)?;
        Ok(())
    }

    /// Check that the account is valid
    pub fn validate_account(&mut self) -> PamResult<()> {
        let mut flags = 0;
        flags |= self.silent_flag();
        flags |= self.disallow_null_auth_token_flag();

        let res = unsafe { pam_acct_mgmt(self.handle()?, flags) };
        self.pam_err(res)?;
        Ok(())
    }

    /// Attempt to validate the account, if that fails because the authentication
    /// token is outdated, then an update of the authentication token is requested.
    pub fn validate_account_or_change_auth_token(&mut self) -> PamResult<()> {
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
    pub fn request_failure_delay(&mut self, delay: Duration) -> PamResult<()> {
        let delay = delay.as_micros();
        let delay = delay.min(libc::c_uint::MAX as u128) as libc::c_uint;
        let res = unsafe { pam_fail_delay(self.handle()?, delay) };
        self.pam_err(res)?;
        Ok(())
    }

    /// Set the user that is requesting the authentication.
    pub fn set_requesting_user(&mut self, user: &str) -> PamResult<()> {
        let c_user = CString::new(user)?;
        let res = unsafe {
            pam_set_item(
                self.handle()?,
                PAM_RUSER as i32,
                c_user.as_ptr() as *const libc::c_void,
            )
        };
        self.pam_err(res)?;
        Ok(())
    }

    /// Clear the user that is requesting the authentication.
    pub fn clear_requesting_user(&mut self) -> PamResult<()> {
        let res = unsafe { pam_set_item(self.handle()?, PAM_RUSER as i32, std::ptr::null()) };
        self.pam_err(res)?;
        Ok(())
    }

    /// Set the user that will be authenticated.
    pub fn set_user(&mut self, user: &str) -> PamResult<()> {
        let c_user = CString::new(user)?;
        let res = unsafe {
            pam_set_item(
                self.handle()?,
                PAM_USER as i32,
                c_user.as_ptr() as *const libc::c_void,
            )
        };
        self.pam_err(res)?;
        Ok(())
    }

    /// Clear the user that will be authenticated
    pub fn clear_user(&mut self) -> PamResult<()> {
        let res = unsafe { pam_set_item(self.handle()?, PAM_USER as i32, std::ptr::null()) };
        self.pam_err(res)?;
        Ok(())
    }

    /// Get the user that will be/was authenticated.
    ///
    /// Note that PAM modules might change the authenticated user, so you should
    /// read this after authentication was completed to make sure what the
    /// authenticated user is.
    pub fn get_user(&mut self) -> PamResult<String> {
        let mut ptr = std::ptr::null();
        let res = unsafe { pam_get_item(self.handle()?, PAM_USER as i32, &mut ptr) };
        self.pam_err(res)?;
        Ok(unsafe { string_from_ptr(ptr as *const libc::c_char) })
    }

    /// Set the host that is requesting authentication
    pub fn set_requesting_host(&mut self, host: &str) -> PamResult<()> {
        let c_host = CString::new(host)?;
        let res = unsafe {
            pam_set_item(
                self.handle()?,
                PAM_RHOST as i32,
                c_host.as_ptr() as *const libc::c_void,
            )
        };
        self.pam_err(res)?;
        Ok(())
    }

    /// Clear the host that is requesting authentication
    pub fn clear_requesting_host(&mut self) -> PamResult<()> {
        let res = unsafe { pam_set_item(self.handle()?, PAM_RHOST as i32, std::ptr::null()) };
        self.pam_err(res)?;
        Ok(())
    }

    /// Establish credentials to be stored in PAM
    pub fn credentials_establish(&mut self) -> PamResult<()> {
        self.credentials(CredentialsAction::Establish)
    }

    /// Delete the credentials stored in PAM
    pub fn credentials_delete(&mut self) -> PamResult<()> {
        self.credentials(CredentialsAction::Delete)
    }

    /// Re-initialize the credentials stored in PAM
    pub fn credentials_reinitialize(&mut self) -> PamResult<()> {
        self.credentials(CredentialsAction::Reinitialize)
    }

    /// Refresh the credentials stored in PAM
    pub fn credentials_refresh(&mut self) -> PamResult<()> {
        self.credentials(CredentialsAction::Refresh)
    }

    /// Updates to the credentials stored in PAM
    pub fn credentials(&mut self, action: CredentialsAction) -> PamResult<()> {
        let mut flags = action.as_int();
        flags |= self.silent_flag();

        let res = unsafe { pam_setcred(self.handle()?, flags) };
        self.pam_err(res)?;

        Ok(())
    }

    /// Ask the user to change the authentication token (password).
    ///
    /// If `expired_only` is set to true, only expired authentication tokens
    /// will be asked to be replaced, otherwise a replacement will always be
    /// requested.
    pub fn change_auth_token(&mut self, expired_only: bool) -> PamResult<()> {
        let mut flags = 0;
        flags |= self.silent_flag();
        if expired_only {
            flags |= PAM_CHANGE_EXPIRED_AUTHTOK as i32;
        }
        let res = unsafe { pam_chauthtok(self.handle()?, flags) };
        self.pam_err(res)?;
        Ok(())
    }

    /// Start a user session for the authenticated user.
    pub fn open_session(&mut self) -> PamResult<()> {
        if !self.session_started {
            let res = unsafe { pam_open_session(self.handle()?, self.silent_flag()) };
            self.pam_err(res)?;
            self.session_started = true;
            Ok(())
        } else {
            Err(PamError::SessionAlreadyOpen)
        }
    }

    /// End the user session.
    pub fn close_session(&mut self) -> PamResult<()> {
        if self.session_started {
            let res = unsafe { pam_close_session(self.handle()?, self.silent_flag()) };
            self.pam_err(res)?;
            self.session_started = true;
            Ok(())
        } else {
            Err(PamError::SessionNotOpen)
        }
    }

    /// Set an environment variable in the PAM environment
    pub fn set_env(&mut self, variable: &str, value: &str) -> PamResult<()> {
        let env = format!("{variable}={value}");
        let c_env = CString::new(env)?;
        let res = unsafe { pam_putenv(self.handle()?, c_env.as_ptr()) };
        self.pam_err(res)?;
        Ok(())
    }

    /// Remove an environment variable in the PAM environment
    pub fn unset_env(&mut self, variable: &str) -> PamResult<()> {
        let c_env = CString::new(variable)?;
        let res = unsafe { pam_putenv(self.handle()?, c_env.as_ptr()) };
        self.pam_err(res)?;

        Ok(())
    }

    /// Get an environment variable from the PAM environment
    pub fn get_env(&mut self, variable: &str) -> PamResult<Option<String>> {
        let c_env = CString::new(variable).expect("String contained nul bytes");
        let c_res = unsafe { pam_getenv(self.handle()?, c_env.as_ptr()) };
        if c_res.is_null() {
            Ok(None)
        } else {
            Ok(Some(unsafe { string_from_ptr(c_res) }))
        }
    }

    /// Get a full listing of the current PAM environment
    pub fn env(&mut self) -> PamResult<Vec<(String, String)>> {
        let mut res = vec![];
        let envs = unsafe { pam_getenvlist(self.handle()?) };
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

    /// Check if anything panicked since the last call.
    pub fn has_panicked(&self) -> bool {
        unsafe { (*self.data_ptr).panicked }
    }
}

impl<'a> PamContext<'a, CLIConverser> {
    /// Create a builder that uses the CLI conversation function.
    pub fn builder_cli(use_stdin: bool) -> PamContextBuilder<CLIConverser> {
        PamContextBuilder::default().converser(CLIConverser { use_stdin })
    }
}

impl<'a, C: Converser> Drop for PamContext<'a, C> {
    fn drop(&mut self) {
        // data_ptr's pointee is de-allocated in this scope
        let _data = unsafe { Box::from_raw(self.data_ptr) };
        if self.pamh.is_some() {
            if !self.session_started {
                let _ = self.close_session();
            }

            // It looks like PAM_DATA_SILENT is important to set for our sudo context, but
            // it is unclear what it really does and does not do, other than the vague
            // documentation description to 'not take the call to seriously'
            // Also see https://github.com/systemd/systemd/issues/22318
            unsafe {
                pam_end(
                    self.handle().unwrap(),
                    self.last_pam_status.unwrap_or(PAM_SUCCESS as libc::c_int),
                ) | PAM_DATA_SILENT as i32
            };
        }
    }
}
