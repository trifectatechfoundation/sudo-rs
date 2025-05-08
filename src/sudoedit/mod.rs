use std::{fs::File, io, path::PathBuf};

use libc::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};

use self::cli::{long_help_message, SudoEditAction, SudoEditOptions, USAGE_MSG};
use crate::system::{
    file::{create_temporary_dir, Chown as _, FileLock},
    interface::{GroupId, UserId},
    signal::{register_handlers, SignalStream},
    User,
};

mod cli;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn main() {
    if User::effective_uid() != User::real_uid() || User::effective_gid() != User::real_gid() {
        println_ignore_io_error!(
            "sudoedit must not be installed as setuid binary.
Please notify your packager about this misconfiguration.
To prevent privilege escalation visudo will now abort."
        );
        std::process::exit(1);
    }

    let options = match SudoEditOptions::from_env() {
        Ok(options) => options,
        Err(error) => {
            println_ignore_io_error!("visudo: {error}\n{USAGE_MSG}");
            std::process::exit(1);
        }
    };

    let cmd = match options.action {
        SudoEditAction::Help => {
            println_ignore_io_error!("{}", long_help_message());
            std::process::exit(0);
        }
        SudoEditAction::Version => {
            println_ignore_io_error!("visudo version {}", VERSION);
            std::process::exit(0);
        }
        SudoEditAction::Run => run,
    };

    match cmd(options.file.as_deref()) {
        Ok(()) => {}
        Err(error) => {
            eprintln_ignore_io_error!("sudoedit: {error}");
            std::process::exit(1);
        }
    }
}
