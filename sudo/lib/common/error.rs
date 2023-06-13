use crate::pam::PamError;
use std::{fmt, path::PathBuf};

#[derive(Debug)]
pub enum Error {
    InvalidCommand(PathBuf),
    UserNotFound(String),
    GroupNotFound(String),
    Exec,
    Authentication(String),
    Configuration(String),
    Options(String),
    Pam(PamError),
    IoError(std::io::Error),
    MaxAuthAttempts(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidCommand(p) => write!(f, "`{p:?}': command not found"),
            Error::UserNotFound(u) => write!(f, "user `{u}' not found"),
            Error::GroupNotFound(g) => write!(f, "group `{g}' not found"),
            Error::Exec => write!(f, "could not spawn child process"),
            Error::Authentication(e) => write!(f, "authentication failed: {e}"),
            Error::Configuration(e) => write!(f, "invalid configuration: {e}"),
            Error::Options(e) => write!(f, "invalid options: {e}"),
            Error::Pam(e) => write!(f, "PAM error: {e}"),
            Error::IoError(e) => write!(f, "IO error: {e}"),
            Error::MaxAuthAttempts(num) => {
                write!(f, "Maximum {num} incorrect authentication attempts")
            }
        }
    }
}

impl From<PamError> for Error {
    fn from(err: PamError) -> Self {
        Error::Pam(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError(err)
    }
}

impl Error {
    pub fn auth(message: &str) -> Self {
        Self::Authentication(message.to_string())
    }

    pub fn conf(message: &str) -> Self {
        Self::Configuration(message.to_string())
    }
}
