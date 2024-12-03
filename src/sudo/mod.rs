#![forbid(unsafe_code)]

use crate::common::resolve::CurrentUser;
use crate::common::{Context, Error};
use crate::log::dev_info;
use crate::system::interface::UserId;
use crate::system::kernel::kernel_check;
use crate::system::timestamp::RecordScope;
use crate::system::User;
use crate::system::{time::Duration, timestamp::SessionRecordFile, Process};
use cli::help;
#[cfg(test)]
pub use cli::SudoAction;
#[cfg(not(test))]
use cli::SudoAction;
use pam::PamAuthenticator;
use pipeline::{Pipeline, PolicyPlugin};
use std::path::Path;

mod cli;
pub mod diagnostic;
mod env;
mod pam;
mod pipeline;

/// show warning message when SUDO_RS_IS_UNSTABLE is not set to the appropriate value
fn unstable_warning() {
    if cfg!(target_os = "linux") {
        return;
    }

    let check_var = std::env::var("SUDO_RS_IS_UNSTABLE").unwrap_or_else(|_| "".to_string());

    if check_var != "I accept that my system may break unexpectedly" {
        eprintln_ignore_io_error!(
            "WARNING!
Sudo-rs is in the early stages of supporting OSes other than Linux and could potentially
break your system. We recommend that you do not run this on any production environment.
To turn off this warning and start using sudo-rs set the environment variable
SUDO_RS_IS_UNSTABLE to the value `I accept that my system may break unexpectedly`. If
you are unsure how to do this then this software is not suited for you at this time."
        );

        std::process::exit(1);
    }
}

const VERSION: &str = if let Some(version_override) = std::option_env!("SUDO_RS_VERSION") {
    version_override
} else {
    std::env!("CARGO_PKG_VERSION")
};

pub(crate) fn candidate_sudoers_file() -> &'static Path {
    let pb_rs = Path::new("/etc/sudoers-rs");
    let file = if pb_rs.exists() {
        pb_rs
    } else if cfg!(target_os = "freebsd") {
        // FIXME maybe make this configurable by the packager?
        Path::new("/usr/local/etc/sudoers")
    } else {
        Path::new("/etc/sudoers")
    };
    dev_info!("Running with {} file", file.display());
    file
}

#[derive(Default)]
pub(crate) struct SudoersPolicy {}

impl PolicyPlugin for SudoersPolicy {
    type PreJudgementPolicy = crate::sudoers::Sudoers;
    type Policy = crate::sudoers::Judgement;

    fn init(&mut self) -> Result<Self::PreJudgementPolicy, Error> {
        let sudoers_path = candidate_sudoers_file();

        let (sudoers, syntax_errors) = crate::sudoers::Sudoers::open(sudoers_path)
            .map_err(|e| Error::Configuration(format!("{e}")))?;

        for crate::sudoers::Error {
            source,
            location,
            message,
        } in syntax_errors
        {
            let path = source.as_deref().unwrap_or(sudoers_path);
            diagnostic::diagnostic!("{message}", path @ location);
        }

        Ok(sudoers)
    }

    fn judge(
        &mut self,
        pre: Self::PreJudgementPolicy,
        context: &Context,
    ) -> Result<Self::Policy, Error> {
        Ok(pre.check(
            &*context.current_user,
            &context.hostname,
            crate::sudoers::Request {
                user: &context.target_user,
                group: context.target_group.as_ref(),
                command: &context.command.command,
                arguments: &context.command.arguments,
            },
        ))
    }
}

fn sudo_process() -> Result<(), Error> {
    crate::log::SudoLogger::new("sudo: ").into_global_logger();

    dev_info!("development logs are enabled");

    self_check()?;
    kernel_check()?;

    let pipeline = Pipeline {
        policy: SudoersPolicy::default(),
        authenticator: PamAuthenticator::new_cli(),
    };

    // parse cli options
    match SudoAction::from_env() {
        Ok(action) => match action {
            SudoAction::Help(_) => {
                eprintln_ignore_io_error!("{}", help::long_help_message());
                std::process::exit(0);
            }
            SudoAction::Version(_) => {
                eprintln_ignore_io_error!("sudo-rs {VERSION}");
                std::process::exit(0);
            }
            SudoAction::RemoveTimestamp(_) => {
                let user = CurrentUser::resolve()?;
                let mut record_file =
                    SessionRecordFile::open_for_user(&user, Duration::seconds(0))?;
                record_file.reset()?;
                Ok(())
            }
            SudoAction::ResetTimestamp(_) => {
                if let Some(scope) = RecordScope::for_process(&Process::new()) {
                    let user = CurrentUser::resolve()?;
                    let mut record_file =
                        SessionRecordFile::open_for_user(&user, Duration::seconds(0))?;
                    record_file.disable(scope, None)?;
                }
                Ok(())
            }
            SudoAction::Validate(options) => pipeline.run_validate(options),
            SudoAction::Run(options) => {
                // special case for when no command is given
                if options.positional_args.is_empty() && !options.shell && !options.login {
                    eprintln_ignore_io_error!("{}", help::USAGE_MSG);
                    std::process::exit(1);
                } else {
                    unstable_warning();

                    pipeline.run(options)
                }
            }
            SudoAction::List(options) => pipeline.run_list(options),
            SudoAction::Edit(_) => {
                eprintln_ignore_io_error!("error: `--edit` flag has not yet been implemented");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln_ignore_io_error!("{e}\n{}", help::USAGE_MSG);
            std::process::exit(1);
        }
    }
}

fn self_check() -> Result<(), Error> {
    let euid = User::effective_uid();
    if euid == UserId::ROOT {
        Ok(())
    } else {
        Err(Error::SelfCheck)
    }
}

pub fn main() {
    match sudo_process() {
        Ok(()) => (),
        Err(error) => {
            if !error.is_silent() {
                diagnostic::diagnostic!("{error}");
            }
            std::process::exit(1);
        }
    }
}
