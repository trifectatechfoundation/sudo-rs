use std::path::PathBuf;

use sudo_system::{Group, User};

use crate::{env::Environment, error::Error};

pub struct CommandAndArguments {
    pub command: PathBuf,
    pub arguments: Vec<String>,
}

impl TryFrom<Vec<&str>> for CommandAndArguments {
    type Error = Error;

    fn try_from(external_args: Vec<&str>) -> Result<Self, Self::Error> {
        let mut iter = external_args.into_iter();

        let command = iter.next().ok_or(Error::InvalidCommand)?.to_string();
        // TODO: we resolve in the context of the current user using the 'which' crate - we want to reconsider this in the future
        let command = which::which(command).map_err(|_| Error::InvalidCommand)?;

        Ok(CommandAndArguments {
            command,
            arguments: iter.map(|v| v.to_string()).collect(),
        })
    }
}

pub struct Context {
    pub preserve_env: bool,
    pub preserve_env_list: Vec<String>,
    pub set_home: bool,
    pub command: CommandAndArguments,
    pub hostname: String,
    pub current_user: User,
    pub target_user: User,
    pub target_group: Group,
    pub target_environment: Environment,
}
