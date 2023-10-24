use crate::common::error::Error;
use crate::exec::{ExecOutput, ExitReason, RunOptions};
use crate::log::user_warn;
use crate::pam::{CLIConverser, PamContext, PamError, PamErrorType};
use crate::system::term::current_tty_name;

use std::{env, process};

use cli::SuAction;
use context::SuContext;
use help::{long_help_message, USAGE_MSG};

use self::cli::SuRunOptions;

mod cli;
mod context;
mod help;

const DEFAULT_USER: &str = "root";
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn authenticate(
    requesting_user: &str,
    user: &str,
    login: bool,
) -> Result<PamContext<CLIConverser>, Error> {
    let context = if login { "su-l" } else { "su" };
    let use_stdin = true;
    let mut pam = PamContext::builder_cli("su", use_stdin, Default::default())
        .target_user(user)
        .service_name(context)
        .build()?;
    pam.set_requesting_user(requesting_user)?;

    // attempt to set the TTY this session is communicating on
    if let Ok(pam_tty) = current_tty_name() {
        pam.set_tty(&pam_tty)?;
    }

    pam.mark_silent(true);
    pam.mark_allow_null_auth_token(false);

    pam.set_user(user)?;

    let mut max_tries = 3;
    let mut current_try = 0;

    loop {
        current_try += 1;
        match pam.authenticate() {
            // there was no error, so authentication succeeded
            Ok(_) => break,

            // maxtries was reached, pam does not allow any more tries
            Err(PamError::Pam(PamErrorType::MaxTries, _)) => {
                return Err(Error::MaxAuthAttempts(current_try));
            }

            // there was an authentication error, we can retry
            Err(PamError::Pam(PamErrorType::AuthError, _)) => {
                max_tries -= 1;
                if max_tries == 0 {
                    return Err(Error::MaxAuthAttempts(current_try));
                } else {
                    user_warn!("Authentication failed, try again.");
                }
            }

            // there was another pam error, return the error
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    pam.validate_account_or_change_auth_token()?;
    pam.open_session()?;

    Ok(pam)
}

fn run(options: SuRunOptions) -> Result<(), Error> {
    // lookup user and build context object
    let context = SuContext::from_env(options)?;

    // authenticate the target user
    let mut pam: PamContext<CLIConverser> = authenticate(
        &context.requesting_user().name,
        &context.user().name,
        context.is_login(),
    )?;

    // su in all cases uses PAM (pam_getenvlist(3)) to do the
    // final environment modification. Command-line options such as
    // --login and --preserve-environment affect the environment before
    // it is modified by PAM.
    let mut environment = context.environment.clone();
    environment.extend(pam.env()?);

    let pid = context.process.pid;

    // run command and return corresponding exit code
    let ExecOutput {
        command_exit_reason,
        restore_signal_handlers,
    } = crate::exec::run_command(&context, environment)?;

    // closing the pam session is best effort, if any error occurs we cannot
    // do anything with it
    let _ = pam.close_session();

    // Run any clean-up code before this line.
    restore_signal_handlers();

    match command_exit_reason {
        ExitReason::Code(code) => process::exit(code),
        ExitReason::Signal(signal) => {
            crate::system::kill(pid, signal)?;
        }
    }

    Ok(())
}

pub fn main() {
    crate::log::SudoLogger::new("su: ").into_global_logger();

    let action = match SuAction::from_env() {
        Ok(action) => action,
        Err(error) => {
            println_ignore_io_error!("su: {error}\n{USAGE_MSG}");
            std::process::exit(1);
        }
    };

    match action {
        SuAction::Help(_) => {
            println_ignore_io_error!("{}", long_help_message());
            std::process::exit(0);
        }
        SuAction::Version(_) => {
            eprintln_ignore_io_error!("su-rs {VERSION}");
            std::process::exit(0);
        }
        SuAction::Run(options) => match run(options) {
            Err(Error::CommandNotFound(c)) => {
                eprintln_ignore_io_error!("su: {}", Error::CommandNotFound(c));
                std::process::exit(127);
            }
            Err(Error::InvalidCommand(c)) => {
                eprintln_ignore_io_error!("su: {}", Error::InvalidCommand(c));
                std::process::exit(126);
            }
            Err(error) => {
                eprintln_ignore_io_error!("su: {error}");
                std::process::exit(1);
            }
            _ => {}
        },
    };
}
