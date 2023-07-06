use crate::pam::PamError;
use std::{fmt, path::PathBuf};

#[derive(Debug)]
pub enum Error {
    CommandNotFound(PathBuf),
    InvalidCommand(PathBuf),
    ChDirNotAllowed { chdir: PathBuf, command: PathBuf },
    UserNotFound(String),
    GroupNotFound(String),
    Authentication(String),
    Configuration(String),
    Options(String),
    Pam(PamError),
    IoError(Option<PathBuf>, std::io::Error),
    MaxAuthAttempts(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::CommandNotFound(p) => write!(f, "'{}': command not found", p.display()),
            Error::InvalidCommand(p) => write!(f, "'{}': invalid command", p.display()),
            Error::UserNotFound(u) => write!(f, "user '{u}' not found"),
            Error::GroupNotFound(g) => write!(f, "group '{g}' not found"),
            Error::Authentication(e) => write!(f, "authentication failed: {e}"),
            Error::Configuration(e) => write!(f, "invalid configuration: {e}"),
            Error::Options(e) => write!(f, "{e}"),
            Error::Pam(e) => write!(f, "PAM error: {e}"),
            Error::IoError(location, e) => {
                if let Some(path) = location {
                    write!(f, "cannot execute '{}': {e}", path.display())
                } else {
                    write!(f, "IO error: {e}")
                }
            }
            Error::MaxAuthAttempts(num) => {
                write!(f, "Maximum {num} incorrect authentication attempts")
            }
            Error::ChDirNotAllowed { chdir, command } => write!(
                f,
                "you are not allowed to use '--chdir {}' with '{}'",
                chdir.display(),
                command.display()
            ),
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
        Error::IoError(None, err)
    }
}

impl Error {
    pub fn auth(message: &str) -> Self {
        Self::Authentication(message.to_string())
    }
}
