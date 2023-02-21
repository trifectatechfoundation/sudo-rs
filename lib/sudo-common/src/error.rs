use std::fmt;

#[derive(Debug)]
pub enum Error {
    InvalidCommand,
    UserNotFound,
    Exec,
    Authentication(String),
    Configuration(String),
    IoError(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidCommand => write!(f, "Invalid command provided"),
            Error::UserNotFound => write!(f, "User not found"),
            Error::Exec => write!(f, "Error executing the command"),
            Error::Authentication(e) => write!(f, "Authentication failed. {e}"),
            Error::Configuration(e) => write!(f, "Invalid configuration. {e}"),
            Error::IoError(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IoError(e)
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
