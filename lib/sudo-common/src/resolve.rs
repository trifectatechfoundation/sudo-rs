use std::{
    env, fs,
    os::unix::prelude::MetadataExt,
    path::{Path, PathBuf},
    str::FromStr,
};
use sudo_cli::SudoOptions;
use sudo_system::{Group, User};

use crate::{context::LaunchType, Error};

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

type Shell = Option<PathBuf>;

pub(crate) fn resolve_launch_and_shell(
    sudo_options: &SudoOptions,
    current_user: &User,
    target_user: &User,
) -> (LaunchType, Shell) {
    if sudo_options.login {
        (LaunchType::Login, Some(target_user.shell.clone()))
    } else if sudo_options.shell {
        let shell = env::var("SHELL")
            .map(|s| s.into())
            .unwrap_or_else(|_| current_user.shell.clone());

        (LaunchType::Shell, Some(shell))
    } else {
        (LaunchType::Direct, None)
    }
}

pub(crate) fn resolve_target_user_and_group(
    target_user_name_or_id: &Option<String>,
    target_group_name_or_id: &Option<String>,
    current_user: &User,
) -> Result<(User, Group), Error> {
    // resolve user name or #<id> to a user
    let mut target_user =
        match NameOrId::parse(target_user_name_or_id.as_deref().unwrap_or_default()) {
            Some(NameOrId::Name(name)) => User::from_name(name)?,
            Some(NameOrId::Id(uid)) => User::from_uid(uid)?,
            _ => None,
        };

    // resolve group name or #<id> to a group
    let mut target_group =
        match NameOrId::parse(target_group_name_or_id.as_deref().unwrap_or_default()) {
            Some(NameOrId::Name(name)) => Group::from_name(name)?,
            Some(NameOrId::Id(gid)) => Group::from_gid(gid)?,
            _ => None,
        };

    match (&target_user_name_or_id, &target_group_name_or_id) {
        // when -g is specified, but -u is not specified default -u to the current user
        (None, Some(_)) => {
            target_user = Some(current_user.clone());
        }
        // when -u is specified but -g is not specified, default -g to the primary group of the specified user
        (Some(_), None) => {
            if let Some(user) = &target_user {
                target_group = Group::from_gid(user.gid)?;
            }
        }
        // when no -u or -g is specified, default to root:root
        (None, None) => {
            target_user = User::from_name("root")?;
            target_group = Group::from_name("root")?;
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

    use super::{resolve_current_user, resolve_target_user_and_group, NameOrId};
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
    fn test_resolve_target_user_and_group() {
        let current_user = resolve_current_user().unwrap();

        // fallback to root
        let (user, group) = resolve_target_user_and_group(&None, &None, &current_user).unwrap();
        assert_eq!(user.name, "root");
        assert_eq!(group.name, "root");

        // unknown user
        let result = resolve_target_user_and_group(
            &Some("non_existing_ghost".to_string()),
            &None,
            &current_user,
        );
        assert!(result.is_err());

        // unknown user
        let result = resolve_target_user_and_group(
            &None,
            &Some("non_existing_ghost".to_string()),
            &current_user,
        );
        assert!(result.is_err());

        // fallback to current user when different group specified
        let (user, group) =
            resolve_target_user_and_group(&None, &Some("root".to_string()), &current_user).unwrap();
        assert_eq!(user.name, current_user.name);
        assert_eq!(group.name, "root");

        // fallback to current users group when no group specified
        let (user, group) = resolve_target_user_and_group(
            &Some(current_user.name.to_string()),
            &None,
            &current_user,
        )
        .unwrap();
        assert_eq!(user.name, current_user.name);
        assert_eq!(group.gid, current_user.gid);
    }
}
