use std::collections::HashMap;

use pretty_assertions::assert_eq;
use sudo_test::{As, EnvBuilder};

use crate::{Result, SUDOERS_ROOT_ALL_NOPASSWD};

// NOTE if 'env_reset' is not in `/etc/sudoers` it is enabled by default

// see 'environment' section in`man sudo`
// see 'command environment' section in`man sudoers`
#[test]
fn vars_set_by_sudo_in_env_reset_mode() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers(SUDOERS_ROOT_ALL_NOPASSWD)
        .build()?;

    let stdout = env.stdout(&["env"], As::Root, None)?;
    let normal_env = parse_env_output(&stdout)?;

    let sudo_abs_path = env.stdout(&["which", "sudo"], As::Root, None)?;
    let env_abs_path = env.stdout(&["which", "env"], As::Root, None)?;

    // run sudo in an empty environment
    let stdout = env.stdout(
        &["env", "-i", &sudo_abs_path, &env_abs_path],
        As::Root,
        None,
    )?;
    let mut sudo_env = parse_env_output(&stdout)?;

    // # man sudo
    // "Set to the mail spool of the target user"
    assert_eq!(Some("/var/mail/root"), sudo_env.remove("MAIL"));

    // "Set to the home directory of the target user"
    assert_eq!(Some("/root"), sudo_env.remove("HOME"));

    // "Set to the login name of the target user"
    assert_eq!(Some("root"), sudo_env.remove("LOGNAME"));

    // "Set to the command run by sudo, including any args"
    assert_eq!(Some("/usr/bin/env"), sudo_env.remove("SUDO_COMMAND"));

    // "Set to the group-ID of the user who invoked sudo"
    assert_eq!(Some("0"), sudo_env.remove("SUDO_GID"));

    // "Set to the user-ID of the user who invoked sudo"
    assert_eq!(Some("0"), sudo_env.remove("SUDO_UID"));

    // "Set to the login name of the user who invoked sudo"
    assert_eq!(Some("root"), sudo_env.remove("SUDO_USER"));

    // "Set to the same value as LOGNAME"
    assert_eq!(Some("root"), sudo_env.remove("USER"));

    // # man sudoers
    // "The HOME, MAIL, SHELL, LOGNAME and USER environment variables are initialized based on the target user"
    assert_eq!(Some("/bin/bash"), sudo_env.remove("SHELL"));

    // "If the PATH and TERM variables are not preserved from the user's environment, they will be set to default values."
    let sudo_path = sudo_env.remove("PATH").expect("PATH not set");

    let normal_path = normal_env["PATH"];
    assert_ne!(normal_path, sudo_path);

    let default_path = "/usr/bin:/bin:/usr/sbin:/sbin";
    assert_eq!(default_path, sudo_path);

    let default_term = "unknown";
    assert_eq!(Some(default_term), sudo_env.remove("TERM"));

    let empty = HashMap::new();
    assert_eq!(empty, sudo_env);

    Ok(())
}

#[test]
fn env_reset_mode_clears_env_vars() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers(SUDOERS_ROOT_ALL_NOPASSWD)
        .build()?;

    let varname = "SHOULD_BE_REMOVED";
    let set_env_var = format!("export {varname}=1");

    // sanity check that `set_env_var` makes `varname` visible to `env` program
    let stdout = env.stdout(
        &["sh", "-c", &format!("{set_env_var}; env")],
        As::Root,
        None,
    )?;
    let env_vars = parse_env_output(&stdout)?;
    assert!(env_vars.contains_key(varname));

    let stdout = env.stdout(
        &["sh", "-c", &format!("{set_env_var}; sudo env")],
        As::Root,
        None,
    )?;
    let env_vars = parse_env_output(&stdout)?;
    assert!(!env_vars.contains_key(varname));

    Ok(())
}

fn parse_env_output(env_output: &str) -> Result<HashMap<&str, &str>> {
    let mut env = HashMap::new();
    for line in env_output.lines() {
        if let Some((key, value)) = line.split_once('=') {
            env.insert(key, value);
        }
    }

    Ok(env)
}
