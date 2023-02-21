use sudo_cli::SudoOptions;
use sudo_common::{context::Context, env::Environment, error::Error, pam::authenticate};
use sudoers::Tag;

/// parse suoers file and check permission to run the provided command given the context
fn check_sudoers(context: &Context, sudo_options: &SudoOptions) -> Result<Option<Vec<Tag>>, Error> {
    // TODO: move to global configuration
    let sudoers_path = "/etc/sudoers.test";

    let (sudoers, syntax_errors) = sudoers::compile(sudoers_path)
        .map_err(|e| Error::Configuration(format!("no sudoers file {e}")))?;

    for error in syntax_errors {
        eprintln!("Parse error: {error:?}");
    }

    Ok(sudoers::check_permission(
        &sudoers,
        &context.current_user,
        sudoers::Request {
            user: &context.target_user,
            group: &context.target_group,
        },
        &context.hostname,
        &sudo_options.external_args.join(" "),
    ))
}

fn main() -> Result<(), Error> {
    // parse cli options
    let sudo_options = SudoOptions::parse();

    // build context and environment
    let current_env = std::env::vars().collect::<Environment>();
    let context = Context::build_from_options(&sudo_options)?.with_filtered_env(current_env);

    // check sudoers file for permission
    match check_sudoers(&context, &sudo_options)? {
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
