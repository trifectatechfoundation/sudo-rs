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
    SelfCheckSetuid,
    SelfCheckNoNewPrivs,
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
    MaxAuthAttempts(u16),
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
                    xlat_write!(
                        f,
                        "Sorry, user {user} is not allowed to execute '{command}' as {other_user} on {hostname}.",
                        user = username,
                        command = command,
                        other_user = other_user,
                        hostname = hostname,
                    )
                } else {
                    xlat_write!(
                        f,
                        "Sorry, user {user} may not run {command} on {hostname}.",
                        user = username,
                        command = command,
                        hostname = hostname,
                    )
                }
            }
            Error::SelfCheckSetuid => {
                xlat_write!(f, "sudo must be owned by uid 0 and have the setuid bit set")
            }
            Error::SelfCheckNoNewPrivs => {
                xlat_write!(
                    f,
                    "The \"no new privileges\" flag is set, which prevents sudo from running as root.\n\
                    If sudo is running in a container, you may need to adjust the container \
                    configuration to disable the flag."
                )
            }
            Error::CommandNotFound(p) => {
                xlat_write!(f, "'{path}': command not found", path = p.display())
            }
            Error::InvalidCommand(p) => {
                xlat_write!(f, "'{path}': invalid command", path = p.display())
            }
            Error::UserNotFound(u) => xlat_write!(f, "user '{user}' not found", user = u),
            Error::GroupNotFound(g) => xlat_write!(f, "group '{group}' not found", group = g),
            Error::Authorization(u) => {
                // TRANSLATORS: This is a well-known quote, try to preserve it in translation.
                xlat_write!(f, "I'm sorry {user}. I'm afraid I can't do that", user = u)
            }
            Error::InteractionRequired => xlat_write!(f, "interactive authentication is required"),
            Error::EnvironmentVar(vs) => {
                xlat_write!(
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
            Error::Configuration(e) => write!(f, "{e}"),
            Error::Options(e) => write!(f, "{e}"),
            Error::Pam(e) => write!(f, "{e}"),
            Error::Io(location, e) => {
                if let Some(path) = location {
                    xlat_write!(
                        f,
                        "cannot execute '{path}': {error}",
                        path = path.display(),
                        error = e
                    )
                } else {
                    xlat_write!(f, "IO error: {error}", error = e)
                }
            }
            Error::MaxAuthAttempts(num) => {
                xlat_write!(
                    f,
                    "maximum {num} incorrect authentication attempts",
                    num = num
                )
            }
            Error::ChDirNotAllowed { chdir, command } => xlat_write!(
                f,
                "you are not allowed to use '--chdir {path}' with '{command}'",
                path = chdir.display(),
                command = command.display()
            ),
            Error::StringValidation(string) => {
                write!(
                    f,
                    "{}: {string:?}",
                    xlat!("Unexpected null character in input")
                )
            }
            Error::PathValidation(path) => {
                write!(
                    f,
                    "{}: {path:?}",
                    xlat!("Unexpected null character in input")
                )
            }
            #[cfg(feature = "apparmor")]
            Error::AppArmor(profile, e) => {
                xlat_write!(
                    f,
                    "unable to change AppArmor profile to {profile}: {error}",
                    profile = profile,
                    error = e
                )
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
