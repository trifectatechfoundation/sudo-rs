use std::io::{self, ErrorKind};
use std::path::PathBuf;

use crate::common::{context::LaunchType, Context};
use crate::system::{Group, User};

pub trait RunOptions {
    fn command(&self) -> io::Result<&PathBuf>;
    fn arguments(&self) -> &Vec<String>;
    fn chdir(&self) -> Option<&PathBuf>;
    fn is_login(&self) -> bool;
    fn user(&self) -> &User;
    fn requesting_user(&self) -> &User;
    fn group(&self) -> &Group;
    fn pid(&self) -> i32;
    fn use_pty(&self) -> bool;
}

impl RunOptions for Context {
    fn command(&self) -> io::Result<&PathBuf> {
        if self.command.resolved {
            Ok(&self.command.command)
        } else {
            Err(ErrorKind::NotFound.into())
        }
    }

    fn arguments(&self) -> &Vec<String> {
        &self.command.arguments
    }

    fn chdir(&self) -> Option<&PathBuf> {
        self.chdir.as_ref()
    }

    fn is_login(&self) -> bool {
        self.launch == LaunchType::Login
    }

    fn user(&self) -> &User {
        &self.target_user
    }

    fn requesting_user(&self) -> &User {
        &self.current_user
    }

    fn group(&self) -> &Group {
        &self.target_group
    }

    fn pid(&self) -> i32 {
        self.process.pid
    }

    fn use_pty(&self) -> bool {
        self.use_pty
    }
}
