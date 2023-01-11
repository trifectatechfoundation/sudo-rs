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

fn sudoers_parse(lines: impl Iterator<Item = String>) -> impl Iterator<Item = sudoers::Sudo> {
    lines.filter_map(|text| match sudoers::parse_string(&text) {
        Ok(x) => Some(x),
        Err(error) => {
            eprintln!("Parse error: {error:?}");
            None
        }
    })
}

fn main() -> Result<(), Error> {
    let sudo_options = SudoOptions::parse();

    let sudoers_path = "/etc/sudoers.test";

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
        preserve_env_list: sudo_options.preserve_env_list,
    };

    context.target_environment = sudo_common::env::get_target_environment(&context);
    let context = context;

    let file = match File::open(sudoers_path) {
        Ok(f) => f,
        Err(e) => panic!("no sudoers file {e}"),
    };

    let sudoers = io::BufReader::new(file).lines().map(|x| x.unwrap());
    let (input, aliases) = sudoers::analyze(sudoers_parse(sudoers));

    let check_results = sudoers::check_permission(
        &input,
        &aliases,
        &context.current_user.name,
        &sudoers::UserInfo {
            user: &context.target_user.name,
            group: &context.target_group.name,
        },
        &context.hostname,
        &sudo_options.external_args.join(" "),
    );

    match check_results {
        Some(tags) => {
            if !tags.contains(&Tag::NoPasswd) {
                authenticate(&context.current_user.name)?;
            }
        }
        None => panic!("no permission"),
    }

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
