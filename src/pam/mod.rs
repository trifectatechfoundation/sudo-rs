use std::{
    ffi::{CStr, CString, OsStr, OsString},
    os::raw::c_char,
    os::unix::prelude::OsStrExt,
    ptr::NonNull,
};

use converse::ConverserData;
use error::pam_err;
pub use error::{PamError, PamErrorType, PamResult};
use sys::*;

mod converse;
mod error;
mod rpassword;
mod securemem;

#[cfg_attr(target_os = "linux", path = "sys_linuxpam.rs")]
#[cfg_attr(
    any(target_os = "freebsd", target_os = "macos"),
    path = "sys_openpam.rs"
)]
#[allow(nonstandard_style)]
pub mod sys;

#[cfg(any(target_os = "freebsd", target_os = "macos"))]
const PAM_DATA_SILENT: std::ffi::c_int = 0;

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
            // SAFETY: we are passing the required fields to `pam_start`; in particular, the value
            // of `pamh` set above is not used, but will be overwritten by `pam_start`.
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
            PAM_SILENT as _
        } else {
            0
        }
    }

    /// Get the PAM flag value for the disallow_null_authtok flag
    fn disallow_null_auth_token_flag(&self) -> i32 {
        if self.allow_null_auth_token {
            0
        } else {
            PAM_DISALLOW_NULL_AUTHTOK as _
        }
    }

    /// Run authentication for the account
    pub fn authenticate(&mut self) -> PamResult<()> {
        let mut flags = 0;
        flags |= self.silent_flag();
        flags |= self.disallow_null_auth_token_flag();

        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`)
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

        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`)
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
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`); furthermore,
        // `c_user.as_ptr()` will point to a correct null-terminated string.
        pam_err(unsafe {
            pam_set_item(
                self.pamh,
                PAM_USER as _,
                c_user.as_ptr() as *const libc::c_void,
            )
        })
    }

    /// Get the user that is currently active in the PAM handle
    pub fn get_user(&mut self) -> PamResult<String> {
        let mut data = std::ptr::null();
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`)
        pam_err(unsafe { pam_get_item(self.pamh, PAM_USER as _, &mut data) })?;

        // safety check to make sure that we were not passed a null pointer by PAM,
        // or that in fact PAM did not write to `data` at all.
        if data.is_null() {
            return Err(PamError::InvalidState);
        }

        // SAFETY: the contract for `pam_get_item` ensures that if `data` was touched by
        // `pam_get_item`, it will point to a valid null-terminated string.
        let cstr = unsafe { CStr::from_ptr(data as *const c_char) };

        Ok(cstr.to_str()?.to_owned())
    }

    /// Set the TTY path for the current TTY that this PAM session started from.
    pub fn set_tty<P: AsRef<OsStr>>(&mut self, tty_path: P) -> PamResult<()> {
        let data = CString::new(tty_path.as_ref().as_bytes())?;
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`); furthermore,
        // `data.as_ptr()` will point to a correct null-terminated string.
        pam_err(unsafe {
            pam_set_item(
                self.pamh,
                PAM_TTY as _,
                data.as_ptr() as *const libc::c_void,
            )
        })
    }

    // Set the user that requested the actions in this PAM instance.
    pub fn set_requesting_user(&mut self, user: &str) -> PamResult<()> {
        let data = CString::new(user.as_bytes())?;
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`); furthermore,
        // `data.as_ptr()` will point to a correct null-terminated string.
        pam_err(unsafe {
            pam_set_item(
                self.pamh,
                PAM_RUSER as _,
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

        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`).
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
            flags |= PAM_CHANGE_EXPIRED_AUTHTOK as libc::c_int;
        }
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`).
        pam_err(unsafe { pam_chauthtok(self.pamh, flags) })
    }

    /// Start a user session for the authenticated user.
    pub fn open_session(&mut self) -> PamResult<()> {
        if !self.session_started {
            // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`).
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
            // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`).
            pam_err(unsafe { pam_close_session(self.pamh, self.silent_flag()) })?;
            self.session_started = false;
            Ok(())
        } else {
            Err(PamError::SessionNotOpen)
        }
    }

    /// Get a full listing of the current PAM environment
    pub fn env(&mut self) -> PamResult<Vec<(OsString, OsString)>> {
        let mut res = Vec::new();
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`).
        // The man page for pam_getenvlist states that:
        //    The format of the memory is a malloc()'d array of char pointers, the last element
        //    of which is set to NULL. Each of the non-NULL entries in this array point to a
        //    NUL terminated and malloc()'d char string of the form: "name=value".
        //
        //    The pam_getenvlist function returns NULL on failure.
        let envs = unsafe { pam_getenvlist(self.pamh) };
        if envs.is_null() {
            return Err(PamError::EnvListFailure);
        }
        let mut curr_env = envs;
        // SAFETY: the loop invariant is as follows:
        // - `curr_env` itself is always a valid pointer to an array of valid (possibly NULL) pointers
        // - if `curr_env` points to a pointer that is not-null, that data is a c-string allocated by malloc()
        // - `curr_env` points to NULL if and only if it is the final element in the array
        while let Some(curr_str) = NonNull::new(unsafe { curr_env.read() }) {
            let data = {
                // SAFETY: `curr_str` points to a valid null-terminated string per the above
                let cstr = unsafe { CStr::from_ptr(curr_str.as_ptr()) };
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
                res.push((k, v));
            }

            // SAFETY: curr_str was obtained via libc::malloc() so we are responsible for freeing it.
            // At this point, curr_str is also the only remaining pointer/reference to that allocated data
            // (the data was copied above), so it can be deallocated without risk of use-after-free errors.
            unsafe { libc::free(curr_str.as_ptr().cast()) };
            // SAFETY: curr_env was not NULL, so it was not the last element in the list and so PAM
            // ensures that the next offset also is a valid pointer, and points to valid data.
            curr_env = unsafe { curr_env.offset(1) };
        }

        // SAFETY: `envs` itself was obtained by malloc(), so we are reponsible for freeing it.
        unsafe { libc::free(envs.cast()) };

        Ok(res)
    }

    /// Check if anything panicked since the last call.
    pub fn has_panicked(&self) -> bool {
        // SAFETY: self.data_ptr was created by Box::into_raw
        unsafe { (*self.data_ptr).panicked }
    }
}

impl PamContext<CLIConverser> {
    /// Create a builder that uses the CLI conversation function.
    pub fn builder_cli(
        name: &str,
        use_stdin: bool,
        no_interact: bool,
        password_feedback: bool,
    ) -> PamContextBuilder<CLIConverser> {
        PamContextBuilder::default().converser(CLIConverser {
            name: name.to_owned(),
            use_stdin,
            no_interact,
            password_feedback,
        })
    }
}

impl<C: Converser> Drop for PamContext<C> {
    fn drop(&mut self) {
        // data_ptr's pointee is de-allocated in this scope
        // SAFETY: self.data_ptr was created by Box::into_raw
        let _data = unsafe { Box::from_raw(self.data_ptr) };
        let _ = self.close_session();

        // It looks like PAM_DATA_SILENT is important to set for our sudo context, but
        // it is unclear what it really does and does not do, other than the vague
        // documentation description to 'not take the call to seriously'
        // Also see https://github.com/systemd/systemd/issues/22318
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`)
        unsafe {
            pam_end(
                self.pamh,
                self.last_pam_status.unwrap_or(PAM_SUCCESS as libc::c_int)
                    | PAM_DATA_SILENT as libc::c_int,
            )
        };
    }
}
