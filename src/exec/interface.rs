use std::io::{self, ErrorKind};
use std::path::Path;

use crate::system::interface::ProcessId;
use crate::{
    common::{context::LaunchType, Context},
    system::{Group, User},
};

pub trait RunOptions {
    fn command(&self) -> io::Result<&Path>;
    fn arguments(&self) -> &[String];
    fn arg0(&self) -> Option<&Path>;
    fn chdir(&self) -> Option<&Path>;
    fn is_login(&self) -> bool;
    fn user(&self) -> &User;
    fn group(&self) -> &Group;
    fn pid(&self) -> ProcessId;

    fn use_pty(&self) -> bool;
}

impl RunOptions for Context {
    fn command(&self) -> io::Result<&Path> {
        if self.command.resolved {
            Ok(&self.command.command)
        } else {
            Err(ErrorKind::NotFound.into())
        }
    }

    fn arguments(&self) -> &[String] {
        &self.command.arguments
    }

    fn arg0(&self) -> Option<&Path> {
        self.command.arg0.as_ref().map(|arg0| &**arg0)
    }

    fn chdir(&self) -> Option<&Path> {
        self.chdir.as_ref().map(|chdir| &**chdir)
    }

    fn is_login(&self) -> bool {
        self.launch == LaunchType::Login
    }

    fn user(&self) -> &User {
        &self.target_user
    }

    fn group(&self) -> &Group {
        &self.target_group
    }

    fn pid(&self) -> ProcessId {
        self.process.pid
    }

    fn use_pty(&self) -> bool {
        self.use_pty
    }
}
