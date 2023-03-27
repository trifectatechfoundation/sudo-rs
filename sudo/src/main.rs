#![forbid(unsafe_code)]

use pam::authenticate;
use sudo_cli::SudoOptions;
use sudo_common::{
    context::{Context, Environment},
    error::Error,
};
use sudo_env::environment;
use sudoers::{Authorization, Sudoers};

mod diagnostic;
mod pam;

fn parse_sudoers() -> Result<Sudoers, Error> {
    // TODO: move to global configuration
    let sudoers_path = "/etc/sudoers.test";

    let (sudoers, syntax_errors) = Sudoers::new(sudoers_path)
        .map_err(|e| Error::Configuration(format!("no valid sudoers file: {e}")))?;

    for sudoers::Error(pos, error) in syntax_errors {
        diagnostic::diagnostic!("{error}", sudoers_path @ pos);
    }

    Ok(sudoers)
}

/// parse suoers file and check permission to run the provided command given the context
fn check_sudoers(sudoers: &Sudoers, context: &Context) -> sudoers::Judgement {
    sudoers.check(
        &context.current_user,
        &context.hostname,
        sudoers::Request {
            user: &context.target_user,
            group: &context.target_group,
            command: &context.command.command,
            arguments: &context.command.arguments.join(" "),
        },
    )
}

fn main() -> Result<(), Error> {
    // parse cli options
    let sudo_options = SudoOptions::parse();

    let check_var = std::env::var("SUDO_RS_IS_UNSTABLE").unwrap_or_else(|_| "".to_string());

    if check_var != "I accept that my system may break unexpectedly" {
        eprintln!("WARNING!");
        eprintln!("Sudo-rs is in the early stages of development and could potentially break your system.");
        eprintln!(
            "We recommend that you do not run this on any production environment. To turn off this"
        );
        eprintln!(
            "warning and start using sudo-rs set the environment variable SUDO_RS_IS_UNSTABLE to"
        );
        eprintln!(
            "the value `I accept that my system may break unexpectedly`. If you are unsure how to"
        );
        eprintln!("do this then this software is not suited for you at this time.");
        std::process::exit(1);
    }

    // parse sudoers file
    let sudoers = parse_sudoers()?;

    // build context and environment
    let current_env = std::env::vars().collect::<Environment>();
    let context = Context::build_from_options(&sudo_options)?;

    // check sudoers file for permission
    let policy = check_sudoers(&sudoers, &context);
    match policy.authorization() {
        Authorization::Required => {
            // authenticate user using pam
            authenticate(&context.current_user.name)?;
        }
        Authorization::Passed => {}
        Authorization::Forbidden => {
            return Err(Error::auth("no permission"));
        }
    };

    let target_env = environment::get_target_environment(current_env, &context, &policy);

    // run command and return corresponding exit code
    match sudo_exec::run_command(context, target_env) {
        Ok(status) => {
            if let Some(code) = status.code() {
                std::process::exit(code);
            } else {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
