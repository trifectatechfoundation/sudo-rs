use std::collections::HashMap;

use pretty_assertions::assert_eq;
use sudo_test::{Command, Env, User};

use crate::{
    helpers, Result, SUDOERS_ROOT_ALL_NOPASSWD, SUDO_ENV_DEFAULT_PATH, SUDO_ENV_DEFAULT_TERM,
    USERNAME,
};

// NOTE if 'env_reset' is not in `/etc/sudoers` it is enabled by default

// see 'environment' section in`man sudo`
// see 'command environment' section in`man sudoers`
#[test]
fn some_vars_are_set() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let stdout = Command::new("env").output(&env)?.stdout()?;
    let normal_env = helpers::parse_env_output(&stdout)?;

    let sudo_abs_path = Command::new("which").arg("sudo").output(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").output(&env)?.stdout()?;

    // run sudo in an empty environment
    let stdout = Command::new("env")
        .args(["-i", &sudo_abs_path, &env_abs_path])
        .output(&env)?
        .stdout()?;
    let mut sudo_env = helpers::parse_env_output(&stdout)?;

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

    assert_eq!(SUDO_ENV_DEFAULT_PATH, sudo_path);

    assert_eq!(Some(SUDO_ENV_DEFAULT_TERM), sudo_env.remove("TERM"));

    let empty = HashMap::new();
    assert_eq!(empty, sudo_env);

    Ok(())
}

#[test]
fn most_vars_are_removed() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let varname = "SHOULD_BE_REMOVED";
    let set_env_var = format!("export {varname}=1");

    // sanity check that `set_env_var` makes `varname` visible to `env` program
    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!("{set_env_var}; env"))
        .output(&env)?
        .stdout()?;
    let env_vars = helpers::parse_env_output(&stdout)?;
    assert!(env_vars.contains_key(varname));

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!("{set_env_var}; sudo env"))
        .output(&env)?
        .stdout()?;
    let env_vars = helpers::parse_env_output(&stdout)?;
    assert!(!env_vars.contains_key(varname));

    Ok(())
}

// this complements the `some_vars_are_set` test where the target user is the
// invoking user
#[test]
fn user_dependent_vars() -> Result<()> {
    let shell_path = "/tmp/shell";
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD)
        .user(User(USERNAME).shell(shell_path))
        .build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").output(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").output(&env)?.stdout()?;

    // run sudo in an empty environment
    let stdout = Command::new("env")
        .args([
            "-i",
            &sudo_abs_path,
            "-u",
            USERNAME,
            &env_abs_path,
        ])
        .output(&env)?
        .stdout()?;
    let mut sudo_env = helpers::parse_env_output(&stdout)?;

    // "The HOME, MAIL, SHELL, LOGNAME and USER environment variables are initialized based on the target user"
    assert_eq!(
        Some(format!("/home/{USERNAME}")).as_deref(),
        sudo_env.remove("HOME")
    );
    assert_eq!(
        Some(format!("/var/mail/{USERNAME}")).as_deref(),
        sudo_env.remove("MAIL")
    );
    assert_eq!(Some(shell_path), sudo_env.remove("SHELL"));
    assert_eq!(Some(USERNAME), sudo_env.remove("LOGNAME"));
    assert_eq!(Some(USERNAME), sudo_env.remove("USER"));

    // "the SUDO_* variables are set based on the invoking user."
    assert_eq!(Some("/usr/bin/env"), sudo_env.remove("SUDO_COMMAND"));
    assert_eq!(Some("0"), sudo_env.remove("SUDO_GID"));
    assert_eq!(Some("0"), sudo_env.remove("SUDO_UID"));
    assert_eq!(Some("root"), sudo_env.remove("SUDO_USER"));

    assert_eq!(Some(SUDO_ENV_DEFAULT_PATH), sudo_env.remove("PATH"));
    assert_eq!(Some(SUDO_ENV_DEFAULT_TERM), sudo_env.remove("TERM"));

    assert_eq!(HashMap::new(), sudo_env);

    Ok(())
}

#[test]
fn some_vars_are_preserved() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").output(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").output(&env)?.stdout()?;

    let home = "some-home";
    let mail = "some-mail";
    let shell = "some-shell";
    let logname = "some-logname";
    let user = "some-user";
    let display = "some-display";
    let path = "some-path";
    let term = "some-term";
    let sudo_command = "some-sudo-command";
    let sudo_user = "some-sudo-user";
    let sudo_uid = "some-sudo-uid";
    let sudo_gid = "some-sudo-gid";
    let stdout = Command::new("env")
        .args([
            "-i",
            &format!("HOME={home}"),
            &format!("MAIL={mail}"),
            &format!("SHELL={shell}"),
            &format!("LOGNAME={logname}"),
            &format!("USER={user}"),
            &format!("DISPLAY={display}"),
            &format!("PATH={path}"),
            &format!("TERM={term}"),
            &format!("SUDO_COMMAND={sudo_command}"),
            &format!("SUDO_USER={sudo_user}"),
            &format!("SUDO_UID={sudo_uid}"),
            &format!("SUDO_GID={sudo_gid}"),
            &sudo_abs_path,
            &env_abs_path,
        ])
        .output(&env)?
        .stdout()?;
    let mut sudo_env = helpers::parse_env_output(&stdout)?;

    // not preserved
    assert_eq!(Some("/root"), sudo_env.remove("HOME"));
    assert_eq!(Some("/var/mail/root"), sudo_env.remove("MAIL"));
    assert_eq!(Some("/bin/bash"), sudo_env.remove("SHELL"));
    assert_eq!(Some("root"), sudo_env.remove("LOGNAME"));
    assert_eq!(Some("root"), sudo_env.remove("USER"));
    assert_eq!(
        Some(env_abs_path).as_deref(),
        sudo_env.remove("SUDO_COMMAND")
    );
    assert_eq!(Some("root"), sudo_env.remove("SUDO_USER"));
    assert_eq!(Some("0"), sudo_env.remove("SUDO_UID"));
    assert_eq!(Some("0"), sudo_env.remove("SUDO_GID"));

    // preserved
    assert_eq!(Some(display), sudo_env.remove("DISPLAY"));
    assert_eq!(Some(path), sudo_env.remove("PATH"));
    assert_eq!(Some(term), sudo_env.remove("TERM"));

    assert_eq!(HashMap::new(), sudo_env);

    Ok(())
}

// only relevant to preserved env vars
#[test]
fn vars_whose_values_start_with_parentheses_are_removed() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").output(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").output(&env)?.stdout()?;

    let stdout = Command::new("env")
        .args([
            "-i",
            "DISPLAY=() display",
            "PATH=() path",
            "TERM=() term",
            &sudo_abs_path,
            &env_abs_path,
        ])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(!sudo_env.contains_key("DISPLAY"));
    assert_eq!(Some(SUDO_ENV_DEFAULT_PATH), sudo_env.get("PATH").copied());
    assert_eq!(Some(SUDO_ENV_DEFAULT_TERM), sudo_env.get("TERM").copied());

    Ok(())
}
