use std::{
    collections::HashMap,
    ffi::{CStr, CString, OsStr, OsString},
    os::unix::prelude::OsStrExt,
};

use converse::ConverserData;
use error::pam_err;
pub use error::{PamError, PamErrorType, PamResult};
use sys::*;

mod converse;
mod error;
mod rpassword;
mod securemem;

#[allow(nonstandard_style)]
#[allow(unused)]
pub mod sys;

pub use converse::{CLIConverser, Converser};

pub struct PamContext<C: Converser> {
    data_ptr: *mut ConverserData<C>,
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
    /// Build the PamContext based on the current configuration.
    ///
    /// This function will error when the required settings have not yet been
    /// set, or when initialization of the PAM session somehow failed.
    pub fn build(self) -> PamResult<PamContext<C>> {
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

            pam_err(res)?;

            if pamh.is_null() {
                Err(PamError::InvalidState)
            } else {
                Ok(PamContext {
                    data_ptr,
                    pamh,
                    silent: false,
                    allow_null_auth_token: true,
                    last_pam_status: None,
                    session_started: false,
                })
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

        pam_err(unsafe { pam_authenticate(self.pamh, flags) })?;

        if self.has_panicked() {
            panic!("Panic during pam authentication");
        }
        Ok(())
    }

    /// Check that the account is valid
    pub fn validate_account(&mut self) -> PamResult<()> {
        let mut flags = 0;
        flags |= self.silent_flag();
        flags |= self.disallow_null_auth_token_flag();

        pam_err(unsafe { pam_acct_mgmt(self.pamh, flags) })
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

    /// Set the user that will be authenticated.
    pub fn set_user(&mut self, user: &str) -> PamResult<()> {
        let c_user = CString::new(user)?;
        pam_err(unsafe {
            pam_set_item(
                self.pamh,
                PAM_USER as i32,
                c_user.as_ptr() as *const libc::c_void,
            )
        })
    }

    /// Set the TTY path for the current TTY that this PAM session started from.
    pub fn set_tty<P: AsRef<OsStr>>(&mut self, tty_path: P) -> PamResult<()> {
        let data = CString::new(tty_path.as_ref().as_bytes())?;
        pam_err(unsafe {
            pam_set_item(
                self.pamh,
                PAM_TTY as i32,
                data.as_ptr() as *const libc::c_void,
            )
        })
    }

    // Set the user that requested the actions in this PAM instance.
    pub fn set_requesting_user(&mut self, user: &str) -> PamResult<()> {
        let data = CString::new(user.as_bytes())?;
        pam_err(unsafe {
            pam_set_item(
                self.pamh,
                PAM_RUSER as i32,
                data.as_ptr() as *const libc::c_void,
            )
        })
    }

    /// Re-initialize the credentials stored in PAM
    pub fn credentials_reinitialize(&mut self) -> PamResult<()> {
        self.credentials(PAM_REINITIALIZE_CRED as libc::c_int)
    }

    /// Updates to the credentials stored in PAM
    fn credentials(&mut self, action: libc::c_int) -> PamResult<()> {
        let mut flags = action;
        flags |= self.silent_flag();

        pam_err(unsafe { pam_setcred(self.pamh, flags) })
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
        pam_err(unsafe { pam_chauthtok(self.pamh, flags) })
    }

    /// Start a user session for the authenticated user.
    pub fn open_session(&mut self) -> PamResult<()> {
        if !self.session_started {
            pam_err(unsafe { pam_open_session(self.pamh, self.silent_flag()) })?;
            self.session_started = true;
            Ok(())
        } else {
            Err(PamError::SessionAlreadyOpen)
        }
    }

    /// End the user session.
    pub fn close_session(&mut self) -> PamResult<()> {
        if self.session_started {
            pam_err(unsafe { pam_close_session(self.pamh, self.silent_flag()) })?;
            self.session_started = false;
            Ok(())
        } else {
            Err(PamError::SessionNotOpen)
        }
    }

    /// Get a full listing of the current PAM environment
    pub fn env(&mut self) -> PamResult<HashMap<OsString, OsString>> {
        let mut res = HashMap::new();
        let envs = unsafe { pam_getenvlist(self.pamh) };
        if envs.is_null() {
            return Err(PamError::EnvListFailure);
        }
        let mut curr_env = envs;
        while unsafe { !(*curr_env).is_null() } {
            let curr_str = unsafe { *curr_env };
            let data = {
                let cstr = unsafe { CStr::from_ptr(curr_str) };
                let bytes = cstr.to_bytes();
                if let Some(pos) = bytes.iter().position(|b| *b == b'=') {
                    let key = OsStr::from_bytes(&bytes[..pos]).to_owned();
                    let value = OsStr::from_bytes(&bytes[pos + 1..]).to_owned();
                    Some((key, value))
                } else {
                    None
                }
            };
            if let Some((k, v)) = data {
                res.insert(k, v);
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

impl PamContext<CLIConverser> {
    /// Create a builder that uses the CLI conversation function.
    pub fn builder_cli(
        name: &str,
        use_stdin: bool,
        no_interact: bool,
    ) -> PamContextBuilder<CLIConverser> {
        PamContextBuilder::default().converser(CLIConverser {
            name: name.to_owned(),
            use_stdin,
            no_interact,
        })
    }
}

impl<C: Converser> Drop for PamContext<C> {
    fn drop(&mut self) {
        // data_ptr's pointee is de-allocated in this scope
        let _data = unsafe { Box::from_raw(self.data_ptr) };
        let _ = self.close_session();

        // It looks like PAM_DATA_SILENT is important to set for our sudo context, but
        // it is unclear what it really does and does not do, other than the vague
        // documentation description to 'not take the call to seriously'
        // Also see https://github.com/systemd/systemd/issues/22318
        unsafe {
            pam_end(
                self.pamh,
                self.last_pam_status.unwrap_or(PAM_SUCCESS as libc::c_int) | PAM_DATA_SILENT as i32,
            )
        };
    }
}
