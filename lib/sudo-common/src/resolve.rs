use std::{
    env, fs,
    os::unix::prelude::MetadataExt,
    path::{Path, PathBuf},
    str::FromStr,
};
use sudo_system::{Group, User};

use crate::Error;

#[derive(PartialEq, Debug)]
enum NameOrId<'a, T: FromStr> {
    Name(&'a str),
    Id(T),
}

impl<'a, T: FromStr> NameOrId<'a, T> {
    pub fn parse(input: &'a str) -> Option<Self> {
        if input.is_empty() {
            None
        } else if let Some(stripped) = input.strip_prefix('#') {
            stripped.parse::<T>().ok().map(|id| Self::Id(id))
        } else {
            Some(Self::Name(input))
        }
    }
}

pub(crate) fn resolve_current_user() -> Result<User, Error> {
    User::real()?.ok_or(Error::UserNotFound("current user".to_string()))
}

pub(crate) fn resolve_target_user(target_name_or_id: &Option<String>) -> Result<User, Error> {
    let is_default = target_name_or_id.is_none();
    let target_name_or_id = target_name_or_id.as_deref().unwrap_or("root");

    let mut user = match NameOrId::parse(target_name_or_id) {
        Some(NameOrId::Name(name)) => User::from_name(name)?,
        Some(NameOrId::Id(uid)) => User::from_uid(uid)?,
        _ => None,
    }
    .ok_or_else(|| Error::UserNotFound(target_name_or_id.to_string()))?;

    user.is_default = is_default;

    Ok(user)
}

pub(crate) fn resolve_target_group(
    target_name_or_id: &Option<String>,
    target_user: &User,
) -> Result<Group, Error> {
    match target_name_or_id.as_deref() {
        Some(name_or_id) => match NameOrId::parse(name_or_id) {
            Some(NameOrId::Name(name)) => Group::from_name(name)?,
            Some(NameOrId::Id(gid)) => Group::from_gid(gid)?,
            _ => None,
        },
        None => Group::from_gid(target_user.gid)?,
    }
    .ok_or(Error::GroupNotFound(
        target_name_or_id
            .clone()
            .unwrap_or_else(|| target_user.gid.to_string()),
    ))
}

/// Check whether a path points to a regular file and any executable flag is set
fn is_valid_executable(path: &PathBuf) -> bool {
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
    // To prevent command spoofing, sudo checks "." and "" (both denoting current directory)
    // last when searching for a command in the user's PATH (if one or both are in the PATH).
    // Depending on the security policy, the user's PATH environment variable may be modified,
    // replaced, or passed unchanged to the program that sudo executes.
    let mut resolve_current_path = false;

    path.split(':')
        // register whether to look in the current directory, but first check the other PATH segments
        .filter(|&path| {
            if path.is_empty() || path == "." {
                resolve_current_path = true;

                false
            } else {
                true
            }
        })
        // construct a possible executable absolute path candidate
        .map(|path| PathBuf::from(path).join(command))
        // check whether the candidate is a regular file and any executable flag is set
        .find(is_valid_executable)
        // if no no executable could be resolved try the current directory
        // if it was present in the PATH
        .or_else(|| {
            if resolve_current_path {
                env::current_dir()
                    .ok()
                    .map(|dir| dir.join(command))
                    .and_then(|path| {
                        if is_valid_executable(&path) {
                            Some(path)
                        } else {
                            None
                        }
                    })
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use sudo_system::User;

    use super::{resolve_target_group, resolve_target_user, NameOrId};
    use crate::resolve::resolve_path;

    // this test is platform specific -> should be changed when targetting different platforms
    #[test]
    fn test_resolve_path() {
        let path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
        assert_eq!(
            resolve_path(&PathBuf::from("sh"), path),
            Some(PathBuf::from("/usr/bin/sh"))
        );
        assert_eq!(
            resolve_path(&PathBuf::from("sudo"), path),
            Some(PathBuf::from("/usr/bin/sudo"))
        );
        assert_eq!(
            resolve_path(&PathBuf::from("env"), path),
            Some(PathBuf::from("/usr/bin/env"))
        );
        assert_eq!(
            resolve_path(&PathBuf::from("thisisnotonyourfs"), path),
            None
        );
        assert_eq!(resolve_path(&PathBuf::from("thisisnotonyourfs"), "."), None);
    }

    #[test]
    fn test_name_or_id() {
        assert_eq!(NameOrId::<u32>::parse(""), None);
        assert_eq!(NameOrId::<u32>::parse("mies"), Some(NameOrId::Name("mies")));
        assert_eq!(NameOrId::<u32>::parse("1337"), Some(NameOrId::Name("1337")));
        assert_eq!(NameOrId::<u32>::parse("#1337"), Some(NameOrId::Id(1337)));
        assert_eq!(NameOrId::<u32>::parse("#-1"), None);
    }

    #[test]
    fn test_resolve_target_user() {
        assert_eq!(
            resolve_target_user(&Some("mies".to_string())).is_err(),
            true
        );
        assert_eq!(resolve_target_user(&Some("root".to_string())).is_ok(), true);
        assert_eq!(resolve_target_user(&Some("#1".to_string())).is_ok(), true);
        assert_eq!(resolve_target_user(&Some("#-1".to_string())).is_err(), true);
        assert_eq!(
            resolve_target_user(&Some("#1337".to_string())).is_err(),
            true
        );
    }

    #[test]
    fn test_resolve_target_group() {
        let current_user = User {
            uid: 1000,
            gid: 1000,
            name: "test".to_string(),
            gecos: String::new(),
            home: "/home/test".to_string(),
            shell: "/bin/sh".to_string(),
            passwd: String::new(),
            groups: None,
            is_default: false,
        };

        assert_eq!(
            resolve_target_group(&Some("root".to_string()), &current_user).is_ok(),
            true
        );
        assert_eq!(
            resolve_target_group(&Some("#1".to_string()), &current_user).is_ok(),
            true
        );
        assert_eq!(
            resolve_target_group(&Some("#-1".to_string()), &current_user).is_err(),
            true
        );
    }
}
