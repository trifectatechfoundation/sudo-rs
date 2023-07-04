#![forbid(unsafe_code)]

use crate::cli::{help, SudoAction, SudoOptions};
use crate::common::{resolve::resolve_current_user, Context, Error};
use crate::log::dev_info;
use crate::system::timestamp::RecordScope;
use crate::system::{time::Duration, timestamp::SessionRecordFile, Process};
use pam::PamAuthenticator;
use pipeline::{Pipeline, PolicyPlugin};
use std::env;
use std::path::Path;

mod diagnostic;
use diagnostic::diagnostic;
mod pam;
mod pipeline;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn candidate_sudoers_file() -> &'static Path {
    let pb_rs: &'static Path = Path::new("/etc/sudoers-rs");
    if pb_rs.exists() {
        dev_info!("Running with /etc/sudoers-rs file");
        pb_rs
    } else {
        dev_info!("Running with /etc/sudoers file");
        Path::new("/etc/sudoers")
    }
}

#[derive(Default)]
pub(crate) struct SudoersPolicy {}

impl PolicyPlugin for SudoersPolicy {
    type PreJudgementPolicy = crate::sudoers::Sudoers;
    type Policy = crate::sudoers::Judgement;

    fn init(&mut self) -> Result<Self::PreJudgementPolicy, Error> {
        let sudoers_path = candidate_sudoers_file();

        let (sudoers, syntax_errors) = crate::sudoers::Sudoers::new(sudoers_path)
            .map_err(|e| Error::Configuration(format!("{e}")))?;

        for crate::sudoers::Error(pos, error) in syntax_errors {
            diagnostic!("{error}", sudoers_path @ pos);
        }

        Ok(sudoers)
    }

    fn judge(
        &mut self,
        pre: Self::PreJudgementPolicy,
        context: &Context,
    ) -> Result<Self::Policy, Error> {
        Ok(pre.check(
            &context.current_user,
            &context.hostname,
            crate::sudoers::Request {
                user: &context.target_user,
                group: &context.target_group,
                command: &context.command.command,
                arguments: &context.command.arguments,
            },
        ))
    }
}

fn sudo_process() -> Result<(), Error> {
    crate::log::SudoLogger::new("sudo: ").into_global_logger();

    dev_info!("development logs are enabled");

    let pipeline = Pipeline {
        policy: SudoersPolicy::default(),
        authenticator: PamAuthenticator::new_cli(),
    };

    // parse cli options
    match SudoOptions::from_env() {
        Ok(options) => match options.action {
            SudoAction::Help => {
                eprintln!("{}", help::long_help_message());
                std::process::exit(0);
            }
            SudoAction::Version => {
                eprintln!("sudo-rs {VERSION}");
                std::process::exit(0);
            }
            SudoAction::RemoveTimestamp => {
                let user = resolve_current_user()?;
                let mut record_file =
                    SessionRecordFile::open_for_user(&user.name, Duration::seconds(0))?;
                record_file.reset()?;
                Ok(())
            }
            SudoAction::ResetTimestamp => {
                if let Some(scope) = RecordScope::for_process(&Process::new()) {
                    let user = resolve_current_user()?;
                    let mut record_file =
                        SessionRecordFile::open_for_user(&user.name, Duration::seconds(0))?;
                    record_file.disable(scope, None)?;
                }
                Ok(())
            }
            SudoAction::Validate => pipeline.run_validate(options),
            SudoAction::Run(ref cmd) => {
                // special case for when no command is given
                if cmd.is_empty() && !options.shell && !options.login {
                    eprintln!("{}", help::USAGE_MSG);
                    std::process::exit(1);
                } else {
                    pipeline.run(options)
                }
            }
            SudoAction::List(_) => {
                unimplemented!();
            }
            SudoAction::Edit(_) => {
                unimplemented!();
            }
        },
        Err(e) => {
            eprintln!("{e}\n{}", help::USAGE_MSG);
            std::process::exit(1);
        }
    }
}

pub fn main() {
    match sudo_process() {
        Ok(()) => (),
        Err(error) => {
            diagnostic!("{error}");
            std::process::exit(1);
        }
    }
}
