use std::ffi::CString;

use converse::{Converser, ConverserData};
use error::pam_err;
pub use error::{PamError, PamErrorType};
use sudo_pam_sys::*;

mod converse;
mod error;

pub use converse::CLIConverser;

pub enum PamItemType {
    Service = PAM_SERVICE as isize,
    User = PAM_USER as isize,
    TTY = PAM_TTY as isize,
    UserPrompt = PAM_USER_PROMPT as isize,
    RequestingUser = PAM_RUSER as isize,
    RequestingHostname = PAM_RHOST as isize,
    AuthToken = PAM_AUTHTOK as isize,
    OldAuthToken = PAM_OLDAUTHTOK as isize,
    Conv = PAM_CONV as isize,
    FailDelay = PAM_FAIL_DELAY as isize,
    XDisplay = PAM_XDISPLAY as isize,
    XAuthData = PAM_XAUTHDATA as isize,
    AuthTokenType = PAM_AUTHTOK_TYPE as isize,
}

pub struct PamContext<C> {
    data: ConverserData<C>,
    pam_conv: Option<pam_conv>,
    pamh: *mut pam_handle_t,
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
            let c_user = self
                .target_user
                .map(|user| CString::new(user))
                .transpose()?;
            let c_user_ptr = c_user
                .map(|cu| cu.as_ptr())
                .unwrap_or_else(|| std::ptr::null());

            let data = ConverserData {
                converser,
                panicked: false,
            };
            let mut context = PamContext {
                data,
                pam_conv: None,
                pamh: std::ptr::null_mut(),
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

    pub fn no_target_user(mut self) -> PamContextBuilder<C> {
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

impl<C> PamContext<C> {
    pub fn new() -> PamContextBuilder<C> {
        PamContextBuilder::default()
    }

    fn pam_err(&self, err: libc::c_int) -> Result<(), PamError> {
        pam_err(err, self.pamh)
    }
}

impl<C: Converser> PamContext<C> {
    pub fn authenticate(
        &mut self,
        silent: bool,
        allow_null_auth_token: bool,
    ) -> Result<(), PamError> {
        let mut flags = 0;
        if silent {
            flags |= PAM_SILENT as i32;
        }

        if !allow_null_auth_token {
            flags |= PAM_DISALLOW_NULL_AUTHTOK as i32;
        }
        self.pam_err(unsafe { pam_authenticate(self.pamh, flags) })?;
        Ok(())
    }

    pub fn validate_account(
        &mut self,
        silent: bool,
        allow_null_auth_token: bool,
    ) -> Result<(), PamError> {
        let mut flags = 0;
        if silent {
            flags |= PAM_SILENT as i32;
        }

        if !allow_null_auth_token {
            flags |= PAM_DISALLOW_NULL_AUTHTOK as i32;
        }

        self.pam_err(unsafe { pam_acct_mgmt(self.pamh, flags) })?;
        Ok(())
    }
}

impl PamContext<CLIConverser> {
    pub fn new_cli() -> PamContextBuilder<CLIConverser> {
        PamContextBuilder::default().converser(CLIConverser)
    }
}
