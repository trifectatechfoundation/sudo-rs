use crate::{pam::PamError, system::Hostname};
use std::{borrow::Cow, fmt, path::PathBuf};

use super::{SudoPath, SudoString};

#[derive(Debug)]
pub enum Error {
    Silent,
    NotAllowed {
        username: SudoString,
        command: Cow<'static, str>,
        hostname: Hostname,
        other_user: Option<SudoString>,
    },
    SelfCheck,
    CommandNotFound(PathBuf),
    InvalidCommand(PathBuf),
    ChDirNotAllowed {
        chdir: SudoPath,
        command: PathBuf,
    },
    UserNotFound(String),
    GroupNotFound(String),
    Authorization(String),
    InteractionRequired,
    EnvironmentVar(Vec<String>),
    Configuration(String),
    Options(String),
    Pam(PamError),
    Io(Option<PathBuf>, std::io::Error),
    MaxAuthAttempts(usize),
    PathValidation(PathBuf),
    StringValidation(String),
    #[cfg(feature = "apparmor")]
    AppArmor(String, std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Silent => Ok(()),
            Error::NotAllowed {
                username,
                command,
                hostname,
                other_user,
            } => {
                if let Some(other_user) = other_user {
                    write!(
                        f,
                        "Sorry, user {username} is not allowed to execute '{command}' as {other_user} on {hostname}.",
                    )
                } else {
                    write!(
                        f,
                        "Sorry, user {username} may not run {command} on {hostname}.",
                    )
                }
            }
            Error::SelfCheck => {
                f.write_str("sudo must be owned by uid 0 and have the setuid bit set")
            }
            Error::CommandNotFound(p) => write!(f, "'{}': command not found", p.display()),
            Error::InvalidCommand(p) => write!(f, "'{}': invalid command", p.display()),
            Error::UserNotFound(u) => write!(f, "user '{u}' not found"),
            Error::GroupNotFound(g) => write!(f, "group '{g}' not found"),
            Error::Authorization(u) => write!(f, "I'm sorry {u}. I'm afraid I can't do that"),
            Error::InteractionRequired => write!(f, "interactive authentication is required"),
            Error::EnvironmentVar(vs) => {
                write!(
                    f,
                    "you are not allowed to set the following environment variables:"
                )?;
                let mut sep = "";
                for v in vs {
                    write!(f, "{sep} {v}")?;
                    sep = ",";
                }
                Ok(())
            }
            Error::Configuration(e) => write!(f, "invalid configuration: {e}"),
            Error::Options(e) => write!(f, "{e}"),
            Error::Pam(e) => write!(f, "{e}"),
            Error::Io(location, e) => {
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
            Error::StringValidation(string) => {
                write!(f, "invalid string: {string:?}")
            }
            Error::PathValidation(path) => {
                write!(f, "invalid path: {path:?}")
            }
            #[cfg(feature = "apparmor")]
            Error::AppArmor(profile, e) => {
                write!(f, "unable to change AppArmor profile to {profile}: {e}")
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
        Error::Io(None, err)
    }
}

impl Error {
    /// Returns `true` if the error is [`Silent`].
    ///
    /// [`Silent`]: Error::Silent
    #[must_use]
    pub fn is_silent(&self) -> bool {
        matches!(self, Self::Silent)
    }
}
