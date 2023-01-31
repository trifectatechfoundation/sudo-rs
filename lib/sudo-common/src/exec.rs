use std::{
    os::unix::process::CommandExt,
    process::{Command, ExitStatus},
};

use crate::{context::Context, error::Error};

pub fn exec(context: Context) -> Result<ExitStatus, Error> {
    Command::new(context.command.command)
        .args(context.command.arguments)
        .uid(context.target_user.uid)
        .gid(context.target_user.gid)
        .envs(context.target_environment)
        .status()
        .map_err(|_| Error::Exec)
}
