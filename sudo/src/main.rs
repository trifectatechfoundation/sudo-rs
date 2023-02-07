use std::{
    fs::File,
    io::{self, BufRead},
};

use sudo_cli::SudoOptions;
use sudo_common::{
    context::{CommandAndArguments, Context},
    error::Error,
    pam::authenticate,
};
use sudo_system::{hostname, Group, User};
use sudoers::Tag;

/// retrieve user information and build context object
fn build_context(sudo_options: &SudoOptions) -> Result<Context, Error> {
    let command_args = sudo_options
        .external_args
        .iter()
        .map(|v| v.as_str())
        .collect::<Vec<&str>>();

    let command = CommandAndArguments::try_from(command_args)?;

    let hostname = hostname();

    let current_user = User::real()
        .map_err(|_| Error::UserNotFound)?
        .ok_or(Error::UserNotFound)?;

    let target_user = User::from_name(sudo_options.user.as_deref().unwrap_or("root"))
        .map_err(|_| Error::UserNotFound)?
        .ok_or(Error::UserNotFound)?;

    let target_group = Group::from_gid(target_user.gid)
        .map_err(|_| Error::UserNotFound)?
        .ok_or(Error::UserNotFound)?;

    let mut context = Context {
        hostname,
        command,
        current_user,
        target_user,
        target_group,
        target_environment: Default::default(),
        preserve_env: sudo_options.preserve_env,
        set_home: sudo_options.set_home,
        preserve_env_list: sudo_options.preserve_env_list.clone(),
    };

    context.target_environment = sudo_common::env::get_target_environment(&context);

    Ok(context)
}

/// parse suoers file and check permission to run the provided command given the context
fn check_sudoers(context: &Context, sudo_options: &SudoOptions) -> Result<Option<Vec<Tag>>, Error> {
    // TODO: move to global configuration
    let sudoers_path = "/etc/sudoers.test";

    let file = File::open(sudoers_path)
        .map_err(|e| Error::Configuration(format!("no sudoers file {e}")))?;

    let sudoers_lines = io::BufReader::new(file).lines().map(|x| x.unwrap());
    let parsed_file = sudoers_lines.filter_map(|text| match sudoers::parse_string(&text) {
        Ok(x) => Some(x),
        Err(error) => {
            eprintln!("Parse error: {error:?}");
            None
        }
    });

    let (input, aliases) = sudoers::analyze(parsed_file);

    Ok(sudoers::check_permission(
        &input,
        &aliases,
        &context.current_user.name,
        &sudoers::UserInfo {
            user: &context.target_user.name,
            group: &context.target_group.name,
        },
        &context.hostname,
        &sudo_options.external_args.join(" "),
    ))
}

fn main() -> Result<(), Error> {
    // parse cli options
    let sudo_options = SudoOptions::parse();

    // build context and environment
    let context = build_context(&sudo_options)?;

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
            eprintln!("{:?}", e);
            std::process::exit(1);
        }
    }
}
