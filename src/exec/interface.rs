use std::io::{self, ErrorKind};
use std::path::Path;

use crate::{
    common::{context::LaunchType, Context},
    system::{Group, User},
};

pub struct RunOptions<'a> {
    pub command: &'a Path,
    pub arguments: &'a [String],
    pub arg0: Option<&'a Path>,
    pub chdir: Option<&'a Path>,
    pub is_login: bool,
    pub user: &'a User,
    pub group: &'a Group,

    pub use_pty: bool,
}

impl Context {
    pub(crate) fn try_as_run_options(&self) -> io::Result<RunOptions<'_>> {
        Ok(RunOptions {
            command: if self.command.resolved {
                &self.command.command
            } else {
                return Err(ErrorKind::NotFound.into());
            },
            arguments: &self.command.arguments,
            arg0: self.command.arg0.as_deref(),
            chdir: self.chdir.as_deref(),
            is_login: self.launch == LaunchType::Login,
            user: &self.target_user,
            group: &self.target_group,

            use_pty: self.use_pty,
        })
    }
}
