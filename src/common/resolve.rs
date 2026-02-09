use crate::system::interface::UserId;
use crate::system::{Group, User};
use core::fmt;
use std::{
    env,
    ffi::CStr,
    fs, io, ops,
    os::unix::prelude::MetadataExt,
    path::{Path, PathBuf},
    str::FromStr,
};

use super::SudoString;
use super::{context::LaunchType, Error};

#[derive(PartialEq, Debug)]
enum NameOrId<'a, T: FromStr> {
    Name(&'a SudoString),
    Id(T),
}

impl<'a, T: FromStr> NameOrId<'a, T> {
    pub fn parse(input: &'a SudoString) -> Option<Self> {
        if input.is_empty() {
            None
        } else if let Some(stripped) = input.strip_prefix('#') {
            stripped.parse::<T>().ok().map(|id| Self::Id(id))
        } else {
            Some(Self::Name(input))
        }
    }
}

#[derive(Clone)]
pub struct CurrentUser {
    inner: User,
}

impl From<CurrentUser> for User {
    fn from(value: CurrentUser) -> Self {
        value.inner
    }
}

impl fmt::Debug for CurrentUser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CurrentUser").field(&self.inner).finish()
    }
}

impl ops::Deref for CurrentUser {
    type Target = User;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl CurrentUser {
    #[cfg(test)]
    pub fn fake(user: User) -> Self {
        Self { inner: user }
    }

    pub fn resolve() -> Result<Self, Error> {
        Ok(Self {
            inner: User::real()?.ok_or(Error::UserNotFound("current user".to_string()))?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct AuthUser(User);

impl AuthUser {
    pub fn from_current_user(user: CurrentUser) -> Self {
        Self(user.inner)
    }

    pub fn resolve_root_for_rootpw() -> Result<Self, Error> {
        Ok(Self(
            User::from_uid(UserId::ROOT)?.ok_or(Error::UserNotFound("root".to_string()))?,
        ))
    }

    pub fn from_user_for_targetpw(user: User) -> Self {
        Self(user)
    }
}

impl ops::Deref for AuthUser {
    type Target = User;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

type Shell = Option<PathBuf>;

pub(super) fn resolve_shell(
    launch_type: LaunchType,
    current_user: &User,
    target_user: &User,
) -> Shell {
    match launch_type {
        LaunchType::Login => Some(target_user.shell.clone()),

        LaunchType::Shell => Some(
            env::var("SHELL")
                .map(|s| s.into())
                .unwrap_or_else(|_| current_user.shell.clone()),
        ),

        LaunchType::Direct => None,
    }
}

pub(crate) fn resolve_target_user_and_group(
    target_user_name_or_id: &Option<SudoString>,
    target_group_name_or_id: &Option<SudoString>,
    current_user: &CurrentUser,
) -> Result<(User, Group), Error> {
    // resolve user name or #<id> to a user
    let mut target_user =
        resolve_from_name_or_id(target_user_name_or_id, User::from_name, User::from_uid)?;

    // resolve group name or #<id> to a group
    let mut target_group =
        resolve_from_name_or_id(target_group_name_or_id, Group::from_name, Group::from_gid)?;

    match (&target_user_name_or_id, &target_group_name_or_id) {
        // when -g is specified, but -u is not specified default -u to the current user
        (None, Some(_)) => {
            target_user = Some(current_user.clone().into());
        }
        // when -u is specified but -g is not specified, default -g to the primary group of the specified user
        (Some(_), None) => {
            if let Some(user) = &target_user {
                target_group = Some(user.primary_group()?);
            }
        }
        // when no -u or -g is specified, default to root:root
        (None, None) => {
            target_user = User::from_name(c"root")?;
            target_group = Group::from_name(if cfg!(target_os = "linux") {
                c"root"
            } else {
                c"wheel"
            })?;
        }
        _ => {}
    }

    match (target_user, target_group) {
        (Some(user), Some(group)) => {
            // resolve success
            Ok((user, group))
        }
        // group name or id not found
        (Some(_), None) => Err(Error::GroupNotFound(
            target_group_name_or_id
                .as_deref()
                .unwrap_or_default()
                .to_string(),
        )),
        // user (and maybe group) name or id not found
        _ => Err(Error::UserNotFound(
            target_user_name_or_id
                .as_deref()
                .unwrap_or_default()
                .to_string(),
        )),
    }
}

fn resolve_from_name_or_id<T, I, E>(
    input: &Option<SudoString>,
    from_name: impl FnOnce(&CStr) -> Result<Option<T>, E>,
    from_id: impl FnOnce(I) -> Result<Option<T>, E>,
) -> Result<Option<T>, E>
where
    I: FromStr,
{
    match input.as_ref().and_then(NameOrId::parse) {
        Some(NameOrId::Name(name)) => from_name(name.as_cstr()),
        Some(NameOrId::Id(id)) => from_id(id),
        None => Ok(None),
    }
}

/// Check whether a path points to a regular file and any executable flag is set
pub(crate) fn is_valid_executable(path: &PathBuf) -> bool {
    if path.is_file() {
        match fs::metadata(path) {
            Ok(meta) => meta.mode() & 0o111 != 0,
            _ => false,
        }
    } else {
        false
    }
}

/// Resolve a executable name based in the PATH environment variable
/// When resolving a path, this code checks whether the target file is
/// a regular file and has any executable bits set. It does not specifically
/// check for user, group, or others' executable bit.
pub(crate) fn resolve_path(command: &Path, path: &str) -> Option<PathBuf> {
    // Depending on the security policy, the user's PATH environment variable may be modified,
    // replaced, or passed unchanged to the program that sudo executes.

    path.split(':')
        .map(Path::new)
        // ignore all relative paths ("", "." or "./")
        .filter(|path| path.is_absolute())
        // construct a possible executable absolute path candidate
        .map(|path| path.join(command))
        // check whether the candidate is a regular file and any executable flag is set
        .find(is_valid_executable)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::common::resolve::CurrentUser;
    use crate::system::ROOT_GROUP_NAME;

    use super::{is_valid_executable, resolve_path, resolve_target_user_and_group, NameOrId};

    #[test]
    fn test_resolve_path() {
        // Assume any linux distro has utilities in this PATH
        let path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

        assert!(is_valid_executable(
            &resolve_path(&PathBuf::from("yes"), path).unwrap()
        ));

        assert!(is_valid_executable(
            &resolve_path(&PathBuf::from("whoami"), path).unwrap()
        ));

        assert!(is_valid_executable(
            &resolve_path(&PathBuf::from("env"), path).unwrap()
        ));
        assert_eq!(
            resolve_path(&PathBuf::from("thisisnotonyourfs"), path),
            None
        );
        assert_eq!(resolve_path(&PathBuf::from("thisisnotonyourfs"), "."), None);
    }

    #[test]
    fn test_cwd_resolve_path() {
        // We modify the path to contain ".", which is supposed to be ignored
        let path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:.";

        let cwd = std::env::current_dir().unwrap();

        // we filter for executable files, so it is most likely going to pick one of the shell
        // scripts in the project's root
        let some_file = cwd
            .read_dir()
            .unwrap()
            .filter_map(|entry| entry.ok())
            .find_map(|entry| {
                let pathbuf = PathBuf::from(entry.file_name());
                is_valid_executable(&pathbuf).then_some(pathbuf)
            })
            .unwrap();

        assert_eq!(resolve_path(&some_file, path), None);
    }

    #[test]
    fn test_name_or_id() {
        assert_eq!(NameOrId::<u32>::parse(&"".into()), None);
        assert_eq!(
            NameOrId::<u32>::parse(&"mies".into()),
            Some(NameOrId::Name(&"mies".into()))
        );
        assert_eq!(
            NameOrId::<u32>::parse(&"1337".into()),
            Some(NameOrId::Name(&"1337".into()))
        );
        assert_eq!(
            NameOrId::<u32>::parse(&"#1337".into()),
            Some(NameOrId::Id(1337))
        );
        assert_eq!(NameOrId::<u32>::parse(&"#-1".into()), None);
    }

    #[test]
    fn test_resolve_target_user_and_group() {
        let current_user = CurrentUser::resolve().unwrap();

        // fallback to root
        let (user, group) = resolve_target_user_and_group(&None, &None, &current_user).unwrap();
        assert_eq!(user.name, "root");
        assert_eq!(group.name.unwrap(), ROOT_GROUP_NAME);

        // unknown user
        let result =
            resolve_target_user_and_group(&Some("non_existing_ghost".into()), &None, &current_user);
        assert!(result.is_err());

        // unknown user
        let result =
            resolve_target_user_and_group(&None, &Some("non_existing_ghost".into()), &current_user);
        assert!(result.is_err());

        // fallback to current user when different group specified
        let (user, group) =
            resolve_target_user_and_group(&None, &Some(ROOT_GROUP_NAME.into()), &current_user)
                .unwrap();
        assert_eq!(user.name, current_user.name);
        assert_eq!(group.name.unwrap(), ROOT_GROUP_NAME);

        // fallback to current users group when no group specified
        let (user, group) =
            resolve_target_user_and_group(&Some(current_user.name.clone()), &None, &current_user)
                .unwrap();
        assert_eq!(user.name, current_user.name);
        assert_eq!(group.gid, current_user.gid);
    }
}

/// Resolve symlinks in all the directories leading up to a file, but
/// not the file itself; this allows sudo to specify a precise policy with
/// tools like busybox or pgrep (which is a symlink to pgrep on systems)
/// This function will check for existence.
pub fn canonicalize<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let reconstructed_path = canonicalize_newfile(path)?;

    // access the object to generate the regular error if it does not exist
    let _ = fs::metadata(&reconstructed_path)?;

    Ok(reconstructed_path)
}

/// Resolve symlinks in all the directories leading up to a file, but
/// not the file itself; this allows us to keep symlinks as is, and will
/// also work on non-existing files.
pub fn canonicalize_newfile<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let path = path.as_ref();
    let Some(parent) = path.parent() else {
        // path is "/" or a prefix
        return Ok(path.to_path_buf());
    };

    let canon_path = fs::canonicalize(parent)?;

    let reconstructed_path = if let Some(file_name) = path.file_name() {
        canon_path.join(file_name)
    } else {
        canon_path
    };

    Ok(reconstructed_path)
}

#[cfg(test)]
mod test {
    use super::canonicalize;
    use std::path::Path;

    #[test]
    fn canonicalization() {
        assert_eq!(canonicalize("/").unwrap(), Path::new("/"));
        assert!(canonicalize("").is_err());
        if cfg!(any(target_os = "linux")) {
            // this test REQUIRES /usr/bin/unxz to be a symlink for /usr/bin/xz or /usr/bin/busybox
            assert!(Path::new("/bin").is_symlink());
            assert!(Path::new("/usr/bin/unxz").is_symlink());
            assert_eq!(
                canonicalize("/usr/bin/unxz").unwrap(),
                Path::new("/usr/bin/unxz")
            );
            // this assumes /bin is a symlink on /usr/bin, like it is on modern Debian/Ubuntu
            assert_eq!(
                canonicalize("/bin/unxz").unwrap(),
                Path::new("/usr/bin/unxz")
            );
        } else if cfg!(target_os = "freebsd") {
            // this test REQUIRES /usr/bin/pkill to be a symlink
            assert!(Path::new("/usr/bin/pkill").is_symlink());
            assert_eq!(
                canonicalize("/usr/bin/pkill").unwrap(),
                Path::new("/usr/bin/pkill")
            );
            assert_eq!(canonicalize("/bin/pkill").unwrap(), Path::new("/bin/pkill"));
        } else {
            panic!(
                "canonicalization test not yet adapted for {}",
                std::env::consts::OS
            );
        }
    }
}
