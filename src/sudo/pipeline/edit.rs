use std::process::exit;

use super::super::cli::SudoRunOptions;
use crate::common::{Context, Error};
use crate::exec::ExitReason;
use crate::sudo::env::environment;
use crate::sudo::pam::pre_exec;
use crate::sudoers::Authorization;

pub fn run_edit(mut cmd_opts: SudoRunOptions) -> Result<(), Error> {
    let mut policy = super::read_sudoers()?;

    let user_requested_env_vars = std::mem::take(&mut cmd_opts.env_var_list);

    let mut context = Context::from_run_opts(cmd_opts, &mut policy)?;

    let policy = super::judge(policy, &context)?;

    let Authorization::Allowed(auth, controls) = policy.authorization() else {
        return Err(Error::Authorization(context.current_user.name.to_string()));
    };

    super::apply_policy_to_context(&mut context, &controls)?;
    let mut pam_context = super::auth_and_update_record_file(&context, auth)?;

    // build environment
    let additional_env = pre_exec(&mut pam_context, &context.target_user.name)?;

    let current_env = environment::system_environment();
    let (checked_vars, trusted_vars) = if controls.trust_environment {
        (vec![], user_requested_env_vars)
    } else {
        (user_requested_env_vars, vec![])
    };

    let mut target_env = environment::get_target_environment(
        current_env,
        additional_env,
        checked_vars,
        &context,
        &controls,
    )?;

    environment::dangerous_extend(&mut target_env, trusted_vars);

    let pid = context.process.pid;

    // prepare switch of apparmor profile
    #[cfg(feature = "apparmor")]
    if let Some(profile) = controls.apparmor_profile {
        crate::apparmor::set_profile_for_next_exec(&profile)
            .map_err(|err| Error::AppArmor(profile, err))?;
    }

    // run command and return corresponding exit code
    let command_exit_reason = if context.command.resolved {
        super::log_command_execution(&context);

        crate::exec::run_command(
            context
                .try_as_run_options()
                .map_err(|io_error| Error::Io(Some(context.command.command.clone()), io_error))?,
            target_env,
        )
        .map_err(|io_error| Error::Io(Some(context.command.command), io_error))
    } else {
        Err(Error::CommandNotFound(context.command.command))
    };

    pam_context.close_session();

    match command_exit_reason? {
        ExitReason::Code(code) => exit(code),
        ExitReason::Signal(signal) => {
            crate::system::kill(pid, signal)?;
        }
    }

    Ok(())
}
