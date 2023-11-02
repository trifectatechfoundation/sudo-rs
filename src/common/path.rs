use std::{
    ffi::OsString,
    ops,
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    str,
};

use super::{Error, SudoString};

/// A `PathBuf` guaranteed to not contain null bytes and be UTF-8 encoded
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(test, derive(Eq))]
pub struct SudoPath {
    inner: String,
}

impl SudoPath {
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        let bytes = path.as_os_str().as_bytes();
        if bytes.contains(&0) {
            return Err(Error::PathValidation(path));
        }

        // check this through a reference so we can return `path` in the error case
        if str::from_utf8(bytes).is_err() {
            return Err(Error::PathValidation(path));
        }

        Ok(Self {
            // NOTE(unwrap): UTF-8 encoding is checked above
            inner: path.into_os_string().into_string().unwrap(),
        })
    }

    pub fn from_cli_string(cli_string: impl Into<String>) -> Self {
        Self::new(cli_string.into().into())
            .expect("strings that come in from CLI should not have interior null bytes")
    }

    /// Resolve the use of a '~' that occurs in this `SudoPathBuf`; based on the sudoers context
    pub fn expand_tilde_in_path(&self, default_username: &SudoString) -> Result<SudoPath, Error> {
        if let Some(prefix) = self.inner.strip_prefix('~') {
            let (username, relpath) = prefix.split_once('/').unwrap_or((prefix, ""));

            let username = if username.is_empty() {
                default_username.clone()
            } else {
                SudoString::new(username.to_string()).unwrap()
            };

            let home_dir = crate::system::User::from_name(username.as_cstr())
                .ok()
                .flatten()
                .ok_or(Error::UserNotFound(username.to_string()))?
                .home;
            let path = home_dir.join(relpath);

            Self::new(path)
        } else {
            Ok(self.clone())
        }
    }
}

impl From<SudoPath> for PathBuf {
    fn from(value: SudoPath) -> Self {
        value.inner.into()
    }
}

impl AsRef<Path> for SudoPath {
    fn as_ref(&self) -> &Path {
        self.inner.as_ref()
    }
}

impl ops::Deref for SudoPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl TryFrom<String> for SudoPath {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value.into())
    }
}

impl From<SudoPath> for OsString {
    fn from(value: SudoPath) -> Self {
        value.inner.into()
    }
}

#[cfg(test)]
impl From<&'_ str> for SudoPath {
    fn from(value: &'_ str) -> Self {
        Self::new(value.into()).unwrap()
    }
}
