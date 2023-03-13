use std::{
    os::unix::process::CommandExt,
    process::{Command, ExitStatus},
};

use crate::{context::ContextWithEnv, error::Error};

pub fn exec(setup: ContextWithEnv) -> Result<ExitStatus, Error> {
    Command::new(setup.context.command.command)
        .args(setup.context.command.arguments)
        .uid(setup.context.target_user.uid)
        .gid(setup.context.target_user.gid)
        .envs(setup.target_environment)
        .status()
        .map_err(|_| Error::Exec)
}
