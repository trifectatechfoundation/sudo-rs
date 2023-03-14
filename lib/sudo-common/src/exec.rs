use std::{
    os::unix::process::CommandExt,
    process::{Command, ExitStatus},
};

use crate::{
    context::{Context, Environment},
    error::Error,
};

pub fn exec(context: Context, env: Environment) -> Result<ExitStatus, Error> {
    Command::new(context.command.command)
        .args(context.command.arguments)
        .uid(context.target_user.uid)
        .gid(context.target_user.gid)
        .env_clear()
        .envs(env)
        .status()
        .map_err(|_| Error::Exec)
}
