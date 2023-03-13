use std::{collections::HashSet, path::PathBuf, str::FromStr};
use sudo_cli::SudoOptions;
use sudo_system::{hostname, Group, User};
use sudoers::Settings;

use crate::{env::Environment, error::Error};

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

        // TODO: we resolve in the context of the current user using the 'which' crate - we want to reconsider this in the future
        let command = which::which(command).map_err(|_| Error::InvalidCommand)?;

        Ok(CommandAndArguments {
            command,
            arguments: iter.map(|v| v.as_str()).collect(),
        })
    }
}

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
    // configuration
    pub env_delete: &'a HashSet<String>,
    pub env_keep: &'a HashSet<String>,
    pub env_check: &'a HashSet<String>,
    pub always_set_home: bool,
    pub use_pty: bool,
    // system
    pub hostname: String,
    pub current_user: User,
    // computed
    pub target_environment: Environment,
}

pub trait Configuration {
    fn env_delete(&self) -> &HashSet<String>;
    fn env_keep(&self) -> &HashSet<String>;
    fn env_check(&self) -> &HashSet<String>;
    fn always_set_home(&self) -> bool;
    fn use_pty(&self) -> bool;
}

impl Configuration for Settings {
    fn env_delete(&self) -> &HashSet<String> {
        self.list
            .get("env_delete")
            .expect("env_delete missing from settings")
    }

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

    fn always_set_home(&self) -> bool {
        self.flags.contains("always_set_home")
    }

    fn use_pty(&self) -> bool {
        self.flags.contains("use_pty")
    }
}

fn resolve_current_user() -> Result<User, Error> {
    User::real()?.ok_or(Error::UserNotFound("current user".to_string()))
}

fn resolve_target_user(target_name_or_id: &Option<String>) -> Result<User, Error> {
    let target_name_or_id = target_name_or_id.as_deref().unwrap_or("root");

    match NameOrId::parse(target_name_or_id) {
        Some(NameOrId::Name(name)) => User::from_name(name)?,
        Some(NameOrId::Id(uid)) => User::from_uid(uid)?,
        _ => None,
    }
    .ok_or_else(|| Error::UserNotFound(target_name_or_id.to_string()))
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
    pub fn build_from_options(
        sudo_options: &'a SudoOptions,
        settings: &'a Settings,
    ) -> Result<Context<'a>, Error> {
        let command = CommandAndArguments::try_from(sudo_options.external_args.as_slice())?;
        let hostname = hostname();
        let current_user = resolve_current_user()?;
        let target_user = resolve_target_user(&sudo_options.user)?;
        let target_group = resolve_target_group(&sudo_options.group, &target_user)?;

        Ok(Context {
            hostname,
            command,
            current_user,
            target_user,
            target_group,
            target_environment: Default::default(),
            set_home: sudo_options.set_home,
            preserve_env_list: sudo_options.preserve_env_list.clone(),
            login: sudo_options.login,
            shell: sudo_options.shell,
            chdir: sudo_options.directory.clone(),
            env_delete: settings.env_delete(),
            env_keep: settings.env_keep(),
            env_check: settings.env_check(),
            always_set_home: settings.always_set_home(),
            use_pty: settings.use_pty(),
        })
    }

    pub fn with_filtered_env(mut self, current_env: Environment) -> Context<'a> {
        self.target_environment = crate::env::get_target_environment(current_env, &self);

        self
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Context;
    use sudo_cli::SudoOptions;
    use sudoers::Settings;

    #[test]
    fn test_build_context() {
        let options = SudoOptions::try_parse_from(["sudo", "echo", "hello"]).unwrap();

        let mut current_env = HashMap::new();
        current_env.insert("FOO".to_string(), "BAR".to_string());

        let settings = Settings::default();
        let context = Context::build_from_options(&options, &settings)
            .unwrap()
            .with_filtered_env(current_env);

        assert_eq!(context.command.command.to_str().unwrap(), "/usr/bin/echo");
        assert_eq!(context.command.arguments, ["hello"]);
        assert_eq!(context.hostname, sudo_system::hostname());
        assert_eq!(context.target_user.uid, 0);
        assert_eq!(
            context.target_environment["SUDO_USER"],
            context.current_user.name
        );
    }
}
