use sudo_test::{Command, Env};

use crate::{helpers, Result, SUDOERS_ROOT_ALL_NOPASSWD, SUDO_RS_IS_UNSTABLE};

// see 'environment' section in `man sudo`
// "SUDO_PS1: If set, PS1 will be set to its value for the program being run."
#[test]
fn ps1_env_var_is_set_when_sudo_ps1_is_set() -> Result<()> {
    let ps1 = "abc";
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").exec(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").exec(&env)?.stdout()?;

    // run sudo in an empty environment
    let stdout = Command::new("env")
        .args(["-i", SUDO_RS_IS_UNSTABLE])
        .arg(format!("SUDO_PS1={ps1}"))
        .args([&sudo_abs_path, &env_abs_path])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(ps1), sudo_env.get("PS1").copied());
    assert!(sudo_env.get("SUDO_PS1").is_none());

    Ok(())
}

#[test]
fn ps1_env_var_is_not_set_when_sudo_ps1_is_set_and_flag_login_is_used() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").exec(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").exec(&env)?.stdout()?;

    // run sudo in an empty environment
    let stdout = Command::new("env")
        .args(["-i", SUDO_RS_IS_UNSTABLE])
        .arg("SUDO_PS1=abc")
        .args([&sudo_abs_path, "-i", &env_abs_path])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(sudo_env.get("PS1").is_none());
    assert!(sudo_env.get("SUDO_PS1").is_none());

    Ok(())
}

// sudo removes env vars whose values start with `()` but that does not affect the SUDO_PS1 feature
#[test]
fn can_start_with_parentheses() -> Result<()> {
    let ps1 = "() abc";
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").exec(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").exec(&env)?.stdout()?;

    // run sudo in an empty environment
    let stdout = Command::new("env")
        .args(["-i", SUDO_RS_IS_UNSTABLE])
        .arg(format!("SUDO_PS1={ps1}"))
        .args([&sudo_abs_path, &env_abs_path])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(ps1), sudo_env.get("PS1").copied());
    assert!(sudo_env.get("SUDO_PS1").is_none());

    Ok(())
}

#[test]
fn preserved_when_in_env_keep_list() -> Result<()> {
    let ps1 = "abc";
    let env = Env([SUDOERS_ROOT_ALL_NOPASSWD, "Defaults env_keep = SUDO_PS1"]).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").exec(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").exec(&env)?.stdout()?;

    // run sudo in an empty environment
    let stdout = Command::new("env")
        .args(["-i", SUDO_RS_IS_UNSTABLE])
        .arg(format!("SUDO_PS1={ps1}"))
        .args([&sudo_abs_path, &env_abs_path])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(ps1), sudo_env.get("PS1").copied());
    assert_eq!(Some(ps1), sudo_env.get("SUDO_PS1").copied());

    Ok(())
}
