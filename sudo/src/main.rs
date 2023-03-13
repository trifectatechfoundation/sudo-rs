#![forbid(unsafe_code)]

use sudo_cli::SudoOptions;
use sudo_common::{context::Context, env::Environment, error::Error, pam::authenticate};
use sudoers::{Sudoers, Tag};

fn parse_sudoers() -> Result<Sudoers, Error> {
    // TODO: move to global configuration
    let sudoers_path = "/etc/sudoers.test";

    let (sudoers, syntax_errors) = sudoers::compile(sudoers_path)
        .map_err(|e| Error::Configuration(format!("no sudoers file {e}")))?;

    for sudoers::Error(_pos, error) in syntax_errors {
        eprintln!("Parse error: {error}");
    }

    Ok(sudoers)
}

/// parse suoers file and check permission to run the provided command given the context
fn check_sudoers(sudoers: &Sudoers, context: &Context) -> Result<Option<Vec<Tag>>, Error> {
    Ok(sudoers::check_permission(
        sudoers,
        &context.current_user,
        &context.hostname,
        sudoers::Request {
            user: &context.target_user,
            group: &context.target_group,
            command: &context.command.command,
            arguments: &context.command.arguments.join(" "),
        },
    ))
}

fn main() -> Result<(), Error> {
    // parse cli options
    let sudo_options = SudoOptions::parse();

    // parse sudoers file
    let sudoers = parse_sudoers()?;

    // build context and environment
    let current_env = std::env::vars().collect::<Environment>();
    let context = Context::build_from_options(&sudo_options, &sudoers.settings)?
        .with_filtered_env(current_env);

    // check sudoers file for permission
    match check_sudoers(&sudoers, &context)? {
        Some(tags) => {
            if !tags.contains(&Tag::NoPasswd) {
                // authenticate user using pam
                authenticate(&context.current_user.name)?;
            }
        }
        None => {
            return Err(Error::auth("no permission"));
        }
    };

    // run command and return corresponding exit code
    match sudo_common::exec::exec(context) {
        Ok(status) => {
            if let Some(code) = status.code() {
                std::process::exit(code);
            } else {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{e:?}");
            std::process::exit(1);
        }
    }
}
