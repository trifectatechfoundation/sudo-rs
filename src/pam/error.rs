use std::ffi::{c_int, NulError};
use std::fmt;
use std::str::Utf8Error;

use crate::cutils::string_from_ptr;

use super::sys::*;

pub type PamResult<T, E = PamError> = Result<T, E>;

// TODO: add missing doc-comments
#[derive(PartialEq, Eq, Debug)]
pub enum PamErrorType {
    /// There was no error running the PAM command
    Success,
    OpenError,
    SymbolError,
    ServiceError,
    SystemError,
    BufferError,
    ConversationError,
    PermissionDenied,
    /// The maximum number of authentication attempts was reached and no more
    /// attempts should be made.
    MaxTries,
    /// The user failed to authenticate correctly.
    AuthError,
    NewAuthTokenRequired,
    /// The application does not have enough credentials to authenticate the
    /// user. This can for example happen if we wanted to update the user
    /// password from a non-root process, which we cannot do.
    CredentialsInsufficient,
    /// PAM modules were unable to access the authentication information (for
    /// example due to a network error).
    AuthInfoUnavailable,
    /// The specified user is unknown to an authentication service.
    UserUnknown,
    /// Failed to retrieve the credentials (i.e. password) for a user.
    CredentialsUnavailable,
    /// The credentials (i.e. password) for this user were expired.
    CredentialsExpired,
    /// There was an error setting the user credentials.
    CredentialsError,
    /// The user account is expired and can no longer be used.
    AccountExpired,
    AuthTokenExpired,
    SessionError,
    AuthTokenError,
    AuthTokenRecoveryError,
    AuthTokenLockBusy,
    AuthTokenDisableAging,
    NoModuleData,
    Ignore,
    /// The application should exit immediately.
    Abort,
    TryAgain,
    ModuleUnknown,
    /// The application tried to set/delete an undefined or inaccessible item.
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
    pub(super) fn from_int(errno: c_int) -> PamErrorType {
        use PamErrorType::*;

        match errno as _ {
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

    pub fn as_int(&self) -> c_int {
        use PamErrorType::*;

        match self {
            Success => PAM_SUCCESS as c_int,
            OpenError => PAM_OPEN_ERR as c_int,
            SymbolError => PAM_SYMBOL_ERR as c_int,
            ServiceError => PAM_SERVICE_ERR as c_int,
            SystemError => PAM_SYSTEM_ERR as c_int,
            BufferError => PAM_BUF_ERR as c_int,
            ConversationError => PAM_CONV_ERR as c_int,
            PermissionDenied => PAM_PERM_DENIED as c_int,
            MaxTries => PAM_MAXTRIES as c_int,
            AuthError => PAM_AUTH_ERR as c_int,
            NewAuthTokenRequired => PAM_NEW_AUTHTOK_REQD as c_int,
            CredentialsInsufficient => PAM_CRED_INSUFFICIENT as c_int,
            AuthInfoUnavailable => PAM_AUTHINFO_UNAVAIL as c_int,
            UserUnknown => PAM_USER_UNKNOWN as c_int,
            CredentialsUnavailable => PAM_CRED_UNAVAIL as c_int,
            CredentialsExpired => PAM_CRED_EXPIRED as c_int,
            CredentialsError => PAM_CRED_ERR as c_int,
            AccountExpired => PAM_ACCT_EXPIRED as c_int,
            AuthTokenExpired => PAM_AUTHTOK_EXPIRED as c_int,
            SessionError => PAM_SESSION_ERR as c_int,
            AuthTokenError => PAM_AUTHTOK_ERR as c_int,
            AuthTokenRecoveryError => PAM_AUTHTOK_RECOVERY_ERR as c_int,
            AuthTokenLockBusy => PAM_AUTHTOK_LOCK_BUSY as c_int,
            AuthTokenDisableAging => PAM_AUTHTOK_DISABLE_AGING as c_int,
            NoModuleData => PAM_NO_MODULE_DATA as c_int,
            Ignore => PAM_IGNORE as c_int,
            Abort => PAM_ABORT as c_int,
            TryAgain => PAM_TRY_AGAIN as c_int,
            ModuleUnknown => PAM_MODULE_UNKNOWN as c_int,
            BadItem => PAM_BAD_ITEM as c_int,
            // DomainUnknown => PAM_DOMAIN_UNKNOWN as c_int,
            // BadHandle => PAM_BAD_HANDLE as c_int,
            // BadFeature => PAM_BAD_FEATURE as c_int,
            // BadConstant => PAM_BAD_CONSTANT as c_int,
            // ConverseAgain => PAM_CONV_AGAIN as c_int,
            // Incomplete => PAM_INCOMPLETE as c_int,
            UnknownErrorType(e) => *e,
        }
    }

    fn get_err_msg(&self) -> String {
        // SAFETY: pam_strerror technically takes a pam handle as the first argument,
        // but we do not know of any implementation that actually uses the pamh
        // argument. See also the netbsd man page for `pam_strerror`.
        let data = unsafe { pam_strerror(std::ptr::null_mut(), self.as_int()) };
        if data.is_null() {
            String::from("Error unresolved by PAM")
        } else {
            // SAFETY: pam_strerror returns a pointer to a null-terminated string
            unsafe { string_from_ptr(data) }
        }
    }
}

#[derive(Debug)]
pub enum PamError {
    UnexpectedNulByte(NulError),
    Utf8Error(Utf8Error),
    Pam(PamErrorType),
    IoError(std::io::Error),
    EnvListFailure,
    InteractionRequired,
    TimedOut,
    InvalidUser(String, String),
}

impl From<std::io::Error> for PamError {
    fn from(err: std::io::Error) -> Self {
        PamError::IoError(err)
    }
}

impl From<NulError> for PamError {
    fn from(err: NulError) -> Self {
        PamError::UnexpectedNulByte(err)
    }
}

impl From<Utf8Error> for PamError {
    fn from(err: Utf8Error) -> Self {
        PamError::Utf8Error(err)
    }
}

impl fmt::Display for PamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PamError::UnexpectedNulByte(_) => write!(f, "Unexpected nul byte in input"),
            PamError::Utf8Error(_) => write!(f, "Could not read input data as UTF-8 string"),
            PamError::Pam(PamErrorType::AuthError) => {
                write!(f, "Account validation failure, is your account locked?")
            }
            PamError::Pam(PamErrorType::NewAuthTokenRequired) => {
                write!(
                    f,
                    "Account or password is expired, reset your password and try again"
                )
            }
            PamError::Pam(PamErrorType::AuthTokenExpired) => {
                write!(f, "Password expired, contact your system administrator")
            }
            PamError::Pam(tp) => write!(f, "PAM error: {}", tp.get_err_msg()),
            PamError::IoError(e) => write!(f, "IO error: {e}"),
            PamError::EnvListFailure => {
                write!(
                    f,
                    "It was not possible to get a list of environment variables"
                )
            }
            PamError::InteractionRequired => write!(f, "Interaction is required"),
            PamError::TimedOut => write!(f, "timed out"),
            PamError::InvalidUser(username, other_user) => {
                write!(
                    f,
                    "Sorry, user {username} is not allowed to authenticate as {other_user}.",
                )
            }
        }
    }
}

impl PamError {
    /// Create a new PamError based on the error number from pam.
    pub(super) fn from_pam(errno: c_int) -> PamError {
        let tp = PamErrorType::from_int(errno);
        PamError::Pam(tp)
    }
}

/// Returns `Ok(())` if the error code is `PAM_SUCCESS` or a `PamError` in other cases
pub(super) fn pam_err(err: c_int) -> Result<(), PamError> {
    if err == PAM_SUCCESS as c_int {
        Ok(())
    } else {
        Err(PamError::from_pam(err))
    }
}

#[cfg(test)]
mod test {
    use super::PamErrorType;

    #[test]
    fn isomorphy() {
        for i in -100..100 {
            let pam = PamErrorType::from_int(i);
            assert_eq!(pam.as_int(), i);
            assert_eq!(PamErrorType::from_int(pam.as_int()), pam);
        }
    }
}
