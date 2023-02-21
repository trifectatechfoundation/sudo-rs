use std::path::PathBuf;
use sudo_cli::SudoOptions;
use sudo_system::{hostname, Group, User};

use crate::{env::Environment, error::Error};

#[derive(Debug)]
pub struct CommandAndArguments<'a> {
    pub command: PathBuf,
    pub arguments: Vec<&'a str>,
}

impl<'a> TryFrom<&'a [String]> for CommandAndArguments<'a> {
    type Error = Error;

    fn try_from(external_args: &'a [String]) -> Result<Self, Self::Error> {
        let mut iter = external_args.into_iter();

        let command = iter.next().ok_or(Error::InvalidCommand)?.to_string();

        // TODO: we resolve in the context of the current user using the 'which' crate - we want to reconsider this in the future
        let command = which::which(command).map_err(|_| Error::InvalidCommand)?;

        Ok(CommandAndArguments {
            command,
            arguments: iter.map(|v| v.as_str()).collect(),
        })
    }
}

#[derive(Debug)]
pub struct Context<'a> {
    pub preserve_env: bool,
    pub preserve_env_list: Vec<String>,
    pub set_home: bool,
    pub command: CommandAndArguments<'a>,
    pub hostname: String,
    pub current_user: User,
    pub target_user: User,
    pub target_group: Group,
    pub target_environment: Environment,
}

impl<'a> Context<'a> {
    pub fn build_from_options(sudo_options: &SudoOptions) -> Result<Context, Error> {
        let command = CommandAndArguments::try_from(sudo_options.external_args.as_slice())?;
        let hostname = hostname();
        let current_user = User::real()?.ok_or(Error::UserNotFound)?;
        let target_username = sudo_options.user.as_deref().unwrap_or("root");
        let target_user = User::from_name(target_username)?
            .ok_or(Error::UserNotFound)?
            .with_groups();

        let target_group = Group::from_gid(target_user.gid)?.ok_or(Error::UserNotFound)?;

        Ok(Context {
            hostname,
            command,
            current_user,
            target_user,
            target_group,
            target_environment: Default::default(),
            preserve_env: sudo_options.preserve_env,
            set_home: sudo_options.set_home,
            preserve_env_list: sudo_options.preserve_env_list.clone(),
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

    #[test]
    fn test_build_context() {
        let options = SudoOptions::try_parse_from(["sudo", "echo", "hello"]).unwrap();

        let mut current_env = HashMap::new();
        current_env.insert("FOO".to_string(), "BAR".to_string());

        let context = Context::build_from_options(&options)
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
