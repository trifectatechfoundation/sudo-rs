use std::ffi::NulError;

use sudo_pam_sys::*;
use thiserror::Error;

pub type PamResult<T, E = PamError> = Result<T, E>;

#[derive(PartialEq, Eq, Debug)]
pub enum PamErrorType {
    Success,
    OpenError,
    SymbolError,
    ServiceError,
    SystemError,
    BufferError,
    ConversationError,
    PermissionDenied,
    MaxTries,
    AuthError,
    NewAuthTokenRequired,
    CredentialsInsufficient,
    AuthInfoUnavailable,
    UserUnknown,
    CredentialsUnavailable,
    CredentialsExpired,
    CredentialsError,
    AccountExpired,
    AuthTokenExpired,
    SessionError,
    AuthTokenError,
    AuthTokenRecoveryError,
    AuthTokenLockBusy,
    AuthTokenDisableAging,
    NoModuleData,
    Ignore,
    Abort,
    TryAgain,
    ModuleUnknown,
    BadItem, // Extension in OpenPAM and LinuxPAM
    // DomainUnknown, // OpenPAM only
    // BadHandle // OpenPAM only
    // BadFeature // OpenPAM only
    // BadConstant // OpenPAM only
    // ConverseAgain // LinuxPAM only
    // Incomplete // LinuxPAM only
    UnknownErrorType(i32),
}

impl PamErrorType {
    pub(crate) fn from_int(errno: libc::c_int) -> PamErrorType {
        use PamErrorType::*;

        match errno as libc::c_uint {
            PAM_SUCCESS => Success,
            PAM_OPEN_ERR => OpenError,
            PAM_SYMBOL_ERR => SymbolError,
            PAM_SERVICE_ERR => ServiceError,
            PAM_SYSTEM_ERR => SystemError,
            PAM_BUF_ERR => BufferError,
            PAM_CONV_ERR => ConversationError,
            PAM_PERM_DENIED => PermissionDenied,
            PAM_MAXTRIES => MaxTries,
            PAM_AUTH_ERR => AuthError,
            PAM_NEW_AUTHTOK_REQD => NewAuthTokenRequired,
            PAM_CRED_INSUFFICIENT => CredentialsInsufficient,
            PAM_AUTHINFO_UNAVAIL => AuthInfoUnavailable,
            PAM_USER_UNKNOWN => UserUnknown,
            PAM_CRED_UNAVAIL => CredentialsUnavailable,
            PAM_CRED_EXPIRED => CredentialsExpired,
            PAM_CRED_ERR => CredentialsError,
            PAM_ACCT_EXPIRED => AccountExpired,
            PAM_AUTHTOK_EXPIRED => AuthTokenExpired,
            PAM_SESSION_ERR => SessionError,
            PAM_AUTHTOK_ERR => AuthTokenError,
            PAM_AUTHTOK_RECOVERY_ERR => AuthTokenRecoveryError,
            PAM_AUTHTOK_LOCK_BUSY => AuthTokenLockBusy,
            PAM_AUTHTOK_DISABLE_AGING => AuthTokenDisableAging,
            PAM_NO_MODULE_DATA => NoModuleData,
            PAM_IGNORE => Ignore,
            PAM_ABORT => Abort,
            PAM_TRY_AGAIN => TryAgain,
            PAM_MODULE_UNKNOWN => ModuleUnknown,
            PAM_BAD_ITEM => BadItem,
            // PAM_DOMAIN_UNKNOWN => DomainUnknown,
            // PAM_BAD_HANDLE => BadHandle,
            // PAM_BAD_FEATURE => BadFeature,
            // PAM_BAD_CONSTANT => BadConstant,
            // PAM_CONV_AGAIN => ConverseAgain,
            // PAM_INCOMPLETE => Incomplete,
            _ => UnknownErrorType(errno),
        }
    }

    pub fn as_int(&self) -> libc::c_int {
        use PamErrorType::*;

        match self {
            Success => PAM_SUCCESS as libc::c_int,
            OpenError => PAM_OPEN_ERR as libc::c_int,
            SymbolError => PAM_SYMBOL_ERR as libc::c_int,
            ServiceError => PAM_SERVICE_ERR as libc::c_int,
            SystemError => PAM_SYSTEM_ERR as libc::c_int,
            BufferError => PAM_BUF_ERR as libc::c_int,
            ConversationError => PAM_CONV_ERR as libc::c_int,
            PermissionDenied => PAM_PERM_DENIED as libc::c_int,
            MaxTries => PAM_MAXTRIES as libc::c_int,
            AuthError => PAM_AUTH_ERR as libc::c_int,
            NewAuthTokenRequired => PAM_NEW_AUTHTOK_REQD as libc::c_int,
            CredentialsInsufficient => PAM_CRED_INSUFFICIENT as libc::c_int,
            AuthInfoUnavailable => PAM_AUTHINFO_UNAVAIL as libc::c_int,
            UserUnknown => PAM_USER_UNKNOWN as libc::c_int,
            CredentialsUnavailable => PAM_CRED_UNAVAIL as libc::c_int,
            CredentialsExpired => PAM_CRED_EXPIRED as libc::c_int,
            CredentialsError => PAM_CRED_ERR as libc::c_int,
            AccountExpired => PAM_ACCT_EXPIRED as libc::c_int,
            AuthTokenExpired => PAM_AUTHTOK_EXPIRED as libc::c_int,
            SessionError => PAM_SESSION_ERR as libc::c_int,
            AuthTokenError => PAM_AUTHTOK_ERR as libc::c_int,
            AuthTokenRecoveryError => PAM_AUTHTOK_RECOVERY_ERR as libc::c_int,
            AuthTokenLockBusy => PAM_AUTHTOK_LOCK_BUSY as libc::c_int,
            AuthTokenDisableAging => PAM_AUTHTOK_DISABLE_AGING as libc::c_int,
            NoModuleData => PAM_NO_MODULE_DATA as libc::c_int,
            Ignore => PAM_IGNORE as libc::c_int,
            Abort => PAM_ABORT as libc::c_int,
            TryAgain => PAM_TRY_AGAIN as libc::c_int,
            ModuleUnknown => PAM_MODULE_UNKNOWN as libc::c_int,
            BadItem => PAM_BAD_ITEM as libc::c_int,
            // DomainUnknown => PAM_DOMAIN_UNKNOWN as libc::c_int,
            // BadHandle => PAM_BAD_HANDLE as libc::c_int,
            // BadFeature => PAM_BAD_FEATURE as libc::c_int,
            // BadConstant => PAM_BAD_CONSTANT as libc::c_int,
            // ConverseAgain => PAM_CONV_AGAIN as libc::c_int,
            // Incomplete => PAM_INCOMPLETE as libc::c_int,
            UnknownErrorType(e) => *e,
        }
    }

    fn get_err_msg(&self, handle: *const pam_handle_t) -> String {
        // TODO: check if handle is fine being a const ptr and the cast here is for the typechecker only
        let data = unsafe { pam_strerror(handle as *mut _, self.as_int()) };
        if data.is_null() {
            String::from("Error unresolved by PAM")
        } else {
            unsafe { sudo_cutils::string_from_ptr(data) }
        }
    }
}

#[derive(Debug, Error)]
pub enum PamError {
    #[error("Unexpected nul byte in input")]
    UnexpectedNulByte(#[from] NulError),
    #[error("Could not initiate pam because the state is not complete")]
    InvalidState,
    #[error("PAM returned an error ({0:?}): {1}")]
    Pam(PamErrorType, String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Cannot open session while one is already open")]
    SessionAlreadyOpen,
    #[error("Cannot close session while none is open")]
    SessionNotOpen,
}

impl PamError {
    /// Create a new PamError based on the error number from pam and a handle to a pam session
    /// The handle to the pam session is allowed to be null
    pub(crate) fn from_pam(errno: libc::c_int, handle: *const pam_handle_t) -> PamError {
        let tp = PamErrorType::from_int(errno);
        let str = tp.get_err_msg(handle);
        PamError::Pam(tp, str)
    }
}

/// Returns `Ok(())` if the error code is `PAM_SUCCESS` or a `PamError` in other cases
pub(crate) fn pam_err(err: libc::c_int, handle: *const pam_handle_t) -> Result<(), PamError> {
    if err == PAM_SUCCESS as libc::c_int {
        Ok(())
    } else {
        Err(PamError::from_pam(err, handle))
    }
}
