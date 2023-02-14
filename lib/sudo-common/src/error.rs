use sudo_pam::PamError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid command")]
    InvalidCommand,
    #[error("User not found")]
    UserNotFound,
    #[error("Exec failed")]
    Exec,
    #[error("Authentication error: {0}")]
    Authentication(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("PAM error: {0}")]
    Pam(#[from] PamError),
}

impl Error {
    pub fn auth(message: &str) -> Self {
        Self::Authentication(message.to_string())
    }

    pub fn conf(message: &str) -> Self {
        Self::Configuration(message.to_string())
    }
}
