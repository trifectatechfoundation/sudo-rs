use sudo_test::{Command, Env};

use crate::{helpers, Result, SUDOERS_ALL_ALL_NOPASSWD};

#[test]
fn equal_single() -> Result<()> {
    let env_name = "SHOULD_BE_PRESERVED";
    let env_val = "42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &*format!("Defaults env_keep = {env_name}"),
    ])
    .build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").exec(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").exec(&env)?.stdout()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args([&sudo_abs_path, &env_abs_path])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val), sudo_env.get(env_name).copied());

    Ok(())
}

#[test]
fn equal_multiple() -> Result<()> {
    let env_name1 = "SHOULD_BE_PRESERVED";
    let env_name2 = "ALSO_SHOULD_BE_PRESERVED";
    let env_val1 = "42";
    let env_val2 = "24";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &*format!("Defaults env_keep = \"{env_name1} {env_name2}\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val1), sudo_env.get(env_name1).copied());
    assert_eq!(Some(env_val2), sudo_env.get(env_name2).copied());

    Ok(())
}

#[test]
fn equal_repeated() -> Result<()> {
    let env_name = "SHOULD_BE_PRESERVED";
    let env_val = "42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &*format!("Defaults env_keep = \"{env_name} {env_name}\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val), sudo_env.get(env_name).copied());

    Ok(())
}

#[test]
fn equal_overrides() -> Result<()> {
    let env_name1 = "SHOULD_BE_PRESERVED";
    let env_val1 = "42";
    let env_name2 = "SHOULD_BE_REMOVED";
    let env_val2 = "24";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &*format!("Defaults env_keep = \"{env_name1} {env_name2}\""),
        &*format!("Defaults env_keep = {env_name1}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(sudo_env.get(env_name2).is_none());

    Ok(())
}

#[test]
fn plus_equal_on_empty_set() -> Result<()> {
    let env_name = "SHOULD_BE_PRESERVED";
    let env_value = "42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &*format!("Defaults env_keep += {env_name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_value}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_value), sudo_env.get(env_name).copied());

    Ok(())
}

#[test]
fn plus_equal_appends() -> Result<()> {
    let env_name1 = "SHOULD_BE_PRESERVED";
    let env_name2 = "ALSO_SHOULD_BE_PRESERVED";
    let env_val1 = "42";
    let env_val2 = "24";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &*format!("Defaults env_keep = {env_name1}"),
        &*format!("Defaults env_keep += {env_name2}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val1), sudo_env.get(env_name1).copied());
    assert_eq!(Some(env_val2), sudo_env.get(env_name2).copied());

    Ok(())
}

#[test]
fn plus_equal_repeated() -> Result<()> {
    let env_name = "SHOULD_BE_PRESERVED";
    let env_val = "42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &*format!("Defaults env_keep = {env_name}"),
        &*format!("Defaults env_keep += {env_name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val), sudo_env.get(env_name).copied());

    Ok(())
}

// see 'environment' section in `man sudo`
// the variables HOME, LOGNAME, MAIL and USER are set by sudo with a value that depends on the
// target user *unless* they appear in the env_keep list
#[test]
#[ignore]
fn vars_with_target_user_specific_values() -> Result<()> {
    let home = "my-home";
    let logname = "my-logname";
    let mail = "my-mail";
    let user = "my-user";

    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        "Defaults env_keep = \"HOME LOGNAME MAIL USER\"",
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("HOME={home}"))
        .arg(format!("LOGNAME={logname}"))
        .arg(format!("MAIL={mail}"))
        .arg(format!("USER={user}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(home), sudo_env.get("HOME").copied());
    assert_eq!(Some(logname), sudo_env.get("LOGNAME").copied());
    assert_eq!(Some(mail), sudo_env.get("MAIL").copied());
    assert_eq!(Some(user), sudo_env.get("USER").copied());

    Ok(())
}

// these variables cannot be preserved as they'll be set by sudo
#[test]
fn sudo_env_vars() -> Result<()> {
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        "Defaults env_keep = \"SUDO_COMMAND SUDO_GID SUDO_UID SUDO_USER\"",
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg("SUDO_COMMAND=command")
        .arg("SUDO_GID=gid")
        .arg("SUDO_UID=uid")
        .arg("SUDO_USER=user")
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some("/usr/bin/env"), sudo_env.get("SUDO_COMMAND").copied());
    assert_eq!(Some("0"), sudo_env.get("SUDO_GID").copied());
    assert_eq!(Some("0"), sudo_env.get("SUDO_UID").copied());
    assert_eq!(Some("root"), sudo_env.get("SUDO_USER").copied());

    Ok(())
}

#[test]
#[ignore]
fn user_set_to_preserved_logname_value() -> Result<()> {
    let value = "ghost";
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults env_keep = \"LOGNAME\""]).build()?;

    let stdout = Command::new("env")
        .arg(format!("LOGNAME={value}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(value), sudo_env.get("LOGNAME").copied());
    assert_eq!(Some(value), sudo_env.get("USER").copied());

    Ok(())
}

#[test]
#[ignore]
fn logname_set_to_preserved_user_value() -> Result<()> {
    let value = "ghost";
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults env_keep = \"USER\""]).build()?;

    let stdout = Command::new("env")
        .arg(format!("USER={value}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(value), sudo_env.get("LOGNAME").copied());
    assert_eq!(Some(value), sudo_env.get("USER").copied());

    Ok(())
}

#[test]
fn if_value_starts_with_parentheses_variable_is_removed() -> Result<()> {
    let env_name = "SHOULD_BE_REMOVED";
    let env_val = "() 42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &*format!("Defaults env_keep = {env_name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(sudo_env.get(env_name).is_none());

    Ok(())
}
