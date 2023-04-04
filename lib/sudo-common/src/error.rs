use sudo_pam::PamError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("command not found")]
    InvalidCommand,
    #[error("user '{0}' not found")]
    UserNotFound(String),
    #[error("group '{0}' not found")]
    GroupNotFound(String),
    #[error("could not spawn child process")]
    Exec,
    #[error("authentication error: {0}")]
    Authentication(String),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("PAM error: {0}")]
    Pam(#[from] PamError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl Error {
    pub fn auth(message: &str) -> Self {
        Self::Authentication(message.to_string())
    }

    pub fn conf(message: &str) -> Self {
        Self::Configuration(message.to_string())
    }
}
