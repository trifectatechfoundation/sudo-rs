use std::{
    ffi::{CStr, CString, OsStr, OsString, c_int, c_void},
    io,
    os::raw::c_char,
    os::unix::prelude::OsStrExt,
    ptr::NonNull,
    time::Duration,
};

use crate::system::signal::{self, SignalSet};

use converse::ConverserData;
use error::pam_err;
pub use error::{PamError, PamErrorType, PamResult};
use sys::*;

mod askpass;
mod converse;
mod error;
mod rpassword;
mod securemem;

#[cfg_attr(target_os = "linux", path = "sys_linuxpam.rs")]
#[cfg_attr(target_os = "freebsd", path = "sys_openpam.rs")]
#[allow(nonstandard_style)]
pub mod sys;

#[link(name = "pam")]
unsafe extern "C" {}

#[cfg(target_os = "freebsd")]
const PAM_DATA_SILENT: std::ffi::c_int = 0;

pub use converse::CLIConverser;

pub struct PamContext {
    data_ptr: *mut ConverserData<CLIConverser>,
    pamh: *mut pam_handle_t,
    silent: bool,
    allow_null_auth_token: bool,
    last_pam_status: Option<c_int>,
    session_started: bool,
}

impl PamContext {
    /// Build the PamContext with the CLI conversation function.
    ///
    /// The target user is optional and may also be set after the context was
    /// constructed or not set at all in which case PAM will ask for a
    /// username.
    ///
    /// This function will error when initialization of the PAM session somehow failed.
    #[allow(clippy::too_many_arguments)]
    pub fn new_cli(
        converser_name: &str,
        service_name: &str,
        use_askpass: bool,
        use_stdin: bool,
        bell: bool,
        no_interact: bool,
        password_feedback: bool,
        password_timeout: Option<Duration>,
        target_user: Option<&str>,
    ) -> PamResult<PamContext> {
        let converser = CLIConverser {
            bell: bell.into(),
            name: converser_name.to_owned(),
            use_askpass,
            use_stdin,
            password_feedback,
            password_timeout,
        };

        let c_service_name = CString::new(service_name)?;
        let c_user = target_user.map(CString::new).transpose()?;
        let c_user_ptr = match c_user {
            Some(ref c) => c.as_ptr(),
            None => std::ptr::null(),
        };

        // this will be de-allocated explicitly in this type's drop method
        let data_ptr = Box::into_raw(Box::new(ConverserData {
            converser,
            converser_name: converser_name.to_owned(),
            no_interact,
            auth_prompt: Some(xlat!("authenticate").to_owned()),
            error: None,
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
                    conv: Some(converse::converse::<CLIConverser>),
                    appdata_ptr: data_ptr as *mut c_void,
                },
                &mut pamh,
            )
        };

        pam_err(res)?;

        assert!(!pamh.is_null());

        Ok(PamContext {
            data_ptr,
            pamh,
            silent: false,
            allow_null_auth_token: true,
            last_pam_status: None,
            session_started: false,
        })
    }

    pub fn set_auth_prompt(&mut self, prompt: Option<String>) {
        // SAFETY: self.data_ptr was created by Box::into_raw
        unsafe {
            (*self.data_ptr).auth_prompt = prompt;
        }
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
        if self.silent { PAM_SILENT as _ } else { 0 }
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
    pub fn authenticate(&mut self, for_user: &str) -> PamResult<()> {
        let mut flags = 0;
        flags |= self.silent_flag();
        flags |= self.disallow_null_auth_token_flag();

        // Temporarily mask SIGINT and SIGQUIT.
        let cur_signals = SignalSet::empty().and_then(|mut set| {
            set.add(signal::consts::SIGINT)?;
            set.add(signal::consts::SIGQUIT)?;
            set.block()
        });

        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`)
        let auth_res = pam_err(unsafe { pam_authenticate(self.pamh, flags) });

        // Restore signals
        if let Ok(set) = cur_signals {
            set.set_mask().map_err(PamError::IoError)?;
        }

        if self.has_panicked() {
            panic!("Panic during pam authentication");
        }

        // SAFETY: self.data_ptr was created by Box::into_raw
        if let Some(error) = unsafe { (*self.data_ptr).error.take() } {
            return Err(error);
        }

        #[allow(clippy::question_mark)]
        if let Err(err) = auth_res {
            return Err(err);
        }

        // Check that no PAM module changed the user.
        match self.get_user() {
            Ok(pam_user) => {
                if pam_user != for_user {
                    return Err(PamError::InvalidUser(pam_user, for_user.to_string()));
                }
            }
            Err(e) => {
                return Err(e);
            }
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
            Err(PamError::Pam(PamErrorType::NewAuthTokenRequired)) => {
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
        pam_err(unsafe { pam_set_item(self.pamh, PAM_USER as _, c_user.as_ptr() as *const c_void) })
    }

    /// Get the user that is currently active in the PAM handle
    pub fn get_user(&mut self) -> PamResult<String> {
        let mut data = std::ptr::null();
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`)
        pam_err(unsafe { pam_get_item(self.pamh, PAM_USER as _, &mut data) })?;

        // safety check to make sure that we were not passed a null pointer by PAM,
        // or that in fact PAM did not write to `data` at all.
        if data.is_null() {
            return Err(PamError::IoError(io::Error::new(
                io::ErrorKind::InvalidData,
                "PAM didn't return username",
            )));
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
        pam_err(unsafe { pam_set_item(self.pamh, PAM_TTY as _, data.as_ptr() as *const c_void) })
    }

    // Set the user that requested the actions in this PAM instance.
    pub fn set_requesting_user(&mut self, user: &str) -> PamResult<()> {
        let data = CString::new(user.as_bytes())?;
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`); furthermore,
        // `data.as_ptr()` will point to a correct null-terminated string.
        pam_err(unsafe { pam_set_item(self.pamh, PAM_RUSER as _, data.as_ptr() as *const c_void) })
    }

    /// Re-initialize the credentials stored in PAM
    pub fn credentials_reinitialize(&mut self) -> PamResult<()> {
        self.credentials(PAM_REINITIALIZE_CRED as c_int)
    }

    /// Updates to the credentials stored in PAM
    fn credentials(&mut self, action: c_int) -> PamResult<()> {
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
            flags |= PAM_CHANGE_EXPIRED_AUTHTOK as c_int;
        }
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`).
        pam_err(unsafe { pam_chauthtok(self.pamh, flags) })
    }

    /// Start a user session for the authenticated user.
    pub fn open_session(&mut self) -> PamResult<()> {
        assert!(!self.session_started);

        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`).
        pam_err(unsafe { pam_open_session(self.pamh, self.silent_flag()) })?;
        self.session_started = true;
        Ok(())
    }

    /// End the user session.
    pub fn close_session(&mut self) {
        // closing the pam session is best effort, if any error occurs we cannot
        // do anything with it
        if self.session_started {
            // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`).
            let _ = pam_err(unsafe { pam_close_session(self.pamh, self.silent_flag()) });
            self.session_started = false;
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

        // SAFETY: `envs` itself was obtained by malloc(), so we are responsible for freeing it.
        unsafe { libc::free(envs.cast()) };

        Ok(res)
    }

    /// Check if anything panicked since the last call.
    pub fn has_panicked(&self) -> bool {
        // SAFETY: self.data_ptr was created by Box::into_raw
        unsafe { (*self.data_ptr).panicked }
    }
}

impl Drop for PamContext {
    fn drop(&mut self) {
        // data_ptr's pointee is de-allocated in this scope
        // SAFETY: self.data_ptr was created by Box::into_raw
        let _data = unsafe { Box::from_raw(self.data_ptr) };
        self.close_session();

        // It looks like PAM_DATA_SILENT is important to set for our sudo context, but
        // it is unclear what it really does and does not do, other than the vague
        // documentation description to 'not take the call to seriously'
        // Also see https://github.com/systemd/systemd/issues/22318
        // SAFETY: `self.pamh` contains a correct handle (obtained from `pam_start`)
        unsafe {
            pam_end(
                self.pamh,
                self.last_pam_status.unwrap_or(PAM_SUCCESS as c_int) | PAM_DATA_SILENT as c_int,
            )
        };
    }
}
