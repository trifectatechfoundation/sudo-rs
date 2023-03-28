use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    str::FromStr,
};
use sudo_cli::SudoOptions;
use sudo_system::{hostname, Group, User};
use sudoers::Settings;

use crate::error::Error;

pub type Environment = HashMap<String, String>;

#[derive(Debug)]
pub struct CommandAndArguments<'a> {
    pub command: PathBuf,
    pub arguments: Vec<&'a str>,
}

impl<'a> TryFrom<&'a [String]> for CommandAndArguments<'a> {
    type Error = Error;

    fn try_from(external_args: &'a [String]) -> Result<Self, Self::Error> {
        let mut iter = external_args.iter();

        let command = iter.next().ok_or(Error::InvalidCommand)?.to_string();

        // resolve the binary if the path is not absolute
        let command = if command.starts_with("/") {
            PathBuf::from(command)
        } else {
            // TODO: we resolve in the context of the current user using the 'which' crate - we want to reconsider this in the future
            // TODO: use value of secure_path setting to possible override current path
            // FIXME: propagating the error is a possible security problem since it leaks information before any permission check is done
            which::which(command).map_err(|_| Error::InvalidCommand)?
        };

        Ok(CommandAndArguments {
            command,
            arguments: iter.map(|v| v.as_str()).collect(),
        })
    }
}

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

/// A Context is based off of global information and is 'non-judgmental'
#[derive(Debug)]
pub struct Context<'a> {
    // cli options
    pub preserve_env_list: Vec<String>,
    pub set_home: bool,
    pub login: bool,
    pub shell: bool,
    pub chdir: Option<PathBuf>,
    pub command: CommandAndArguments<'a>,
    pub target_user: User,
    pub target_group: Group,
    // system
    pub hostname: String,
    pub current_user: User,
    pub pid: i32,
}

pub trait Configuration {
    fn env_keep(&self) -> &HashSet<String>;
    fn env_check(&self) -> &HashSet<String>;
}

impl Configuration for Settings {
    fn env_keep(&self) -> &HashSet<String> {
        self.list
            .get("env_keep")
            .expect("env_keep missing from settings")
    }

    fn env_check(&self) -> &HashSet<String> {
        self.list
            .get("env_check")
            .expect("env_check missing from settings")
    }
}

fn resolve_current_user() -> Result<User, Error> {
    User::real()?.ok_or(Error::UserNotFound("current user".to_string()))
}

fn resolve_target_user(target_name_or_id: &Option<String>) -> Result<User, Error> {
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

fn resolve_target_group(
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

impl<'a> Context<'a> {
    pub fn build_from_options(sudo_options: &'a SudoOptions) -> Result<Context<'a>, Error> {
        let command = CommandAndArguments::try_from(sudo_options.external_args.as_slice())?;
        let hostname = hostname();
        let current_user = resolve_current_user()?.with_groups();
        let target_user = resolve_target_user(&sudo_options.user)?.with_groups();
        let target_group = resolve_target_group(&sudo_options.group, &target_user)?;

        Ok(Context {
            hostname,
            command,
            current_user,
            target_user,
            target_group,
            set_home: sudo_options.set_home,
            preserve_env_list: sudo_options.preserve_env_list.clone(),
            login: sudo_options.login,
            shell: sudo_options.shell,
            chdir: sudo_options.directory.clone(),
            pid: sudo_system::Process::process_id(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use sudo_cli::SudoOptions;
    use sudo_system::User;

    use super::{resolve_target_group, resolve_target_user, Context, NameOrId};

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

    #[test]
    fn test_build_context() {
        let options = SudoOptions::try_parse_from(["sudo", "echo", "hello"]).unwrap();

        let context = Context::build_from_options(&options).unwrap();

        let mut target_environment = HashMap::new();
        target_environment.insert("SUDO_USER".to_string(), context.current_user.name.clone());

        assert_eq!(context.command.command.to_str().unwrap(), "/usr/bin/echo");
        assert_eq!(context.command.arguments, ["hello"]);
        assert_eq!(context.hostname, sudo_system::hostname());
        assert_eq!(context.target_user.uid, 0);
    }
}
