mod check;
mod keep;

use sudo_test::{Command, Env};

use crate::{helpers, EnvList, Result, SUDOERS_ALL_ALL_NOPASSWD};

const BAD_TZ_VALUES: &[&str] = &[
    // "It consists of a fully-qualified path name, optionally prefixed with a colon (‘:’), that
    // does not match the location of the zoneinfo directory."
    ":/usr/share/zoneinfo",
    "/usr/share/zoneinfo",
    "/etc/localtime",
    "/does/not/exist",
    // "It contains a .. path element."
    "../localtime",
    "/usr/share/zoneinfo/..",
    "/usr/../share/zoneinfo/Europe/Berlin",
    // "It contains white space or non-printable characters."
    "/usr/share/zoneinfo/ ",
    "/usr/share/zoneinfo/\u{7}",
    "/usr/share/zoneinfo/\t",
];

#[test]
fn var_in_both_lists_is_preserved() -> Result<()> {
    let name = "SHOULD_BE_PRESERVED";
    let value = "42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_keep = {name}"),
        &format!("Defaults env_check = {name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{name}={value}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(value), sudo_env.get(name).copied());

    drop(env);

    // test sudoers statements in reverse order
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_check = {name}"),
        &format!("Defaults env_keep = {name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{name}={value}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(value), sudo_env.get(name).copied());

    Ok(())
}

#[test]
fn checks_applied_if_in_both_lists() -> Result<()> {
    let name = "SHOULD_BE_REMOVED";
    let value = "4%2";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_keep = {name}"),
        &format!("Defaults env_check = {name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{name}={value}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(None, sudo_env.get(name).copied());

    drop(env);

    // test sudoers statements in reverse order
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_check = {name}"),
        &format!("Defaults env_keep = {name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{name}={value}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(None, sudo_env.get(name).copied());

    Ok(())
}

// adding TZ to env_keep is insufficient to avoid checks (see previous test)
// it's necessary to remove TZ from env_check first
// this applies to all env vars that are in the default env_check list
#[test]
fn unchecked_tz() -> Result<()> {
    const TZ: &str = "TZ";

    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_check -= {TZ}"),
        &format!("Defaults env_keep = {TZ}"),
    ])
    .build()?;

    for &value in BAD_TZ_VALUES {
        let stdout = Command::new("env")
            .arg(format!("{TZ}={value}"))
            .args(["sudo", "env"])
            .output(&env)?
            .stdout()?;
        let sudo_env = helpers::parse_env_output(&stdout)?;

        assert_eq!(Some(value), sudo_env.get(TZ).copied());
    }
    Ok(())
}

fn equal_single(env_list: EnvList) -> Result<()> {
    let env_name = "SHOULD_BE_PRESERVED";
    let env_val = "42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = {env_name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val), sudo_env.get(env_name).copied());

    Ok(())
}

fn equal_multiple(env_list: EnvList) -> Result<()> {
    let env_name1 = "SHOULD_BE_PRESERVED";
    let env_name2 = "ALSO_SHOULD_BE_PRESERVED";
    let env_val1 = "42";
    let env_val2 = "24";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"{env_name1} {env_name2}\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val1), sudo_env.get(env_name1).copied());
    assert_eq!(Some(env_val2), sudo_env.get(env_name2).copied());

    Ok(())
}

fn equal_repeated(env_list: EnvList) -> Result<()> {
    let env_name = "SHOULD_BE_PRESERVED";
    let env_val = "42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"{env_name} {env_name}\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val), sudo_env.get(env_name).copied());

    Ok(())
}

fn equal_overrides(env_list: EnvList) -> Result<()> {
    let env_name1 = "SHOULD_BE_PRESERVED";
    let env_val1 = "42";
    let env_name2 = "SHOULD_BE_REMOVED";
    let env_val2 = "24";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"{env_name1} {env_name2}\""),
        &format!("Defaults {env_list} = {env_name1}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(!sudo_env.contains_key(env_name2));
    assert_eq!(Some(env_val1), sudo_env.get(env_name1).copied());

    Ok(())
}

fn plus_equal_on_empty_set(env_list: EnvList) -> Result<()> {
    let env_name = "SHOULD_BE_PRESERVED";
    let env_value = "42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} += {env_name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_value}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_value), sudo_env.get(env_name).copied());

    Ok(())
}

fn plus_equal_appends(env_list: EnvList) -> Result<()> {
    let env_name1 = "SHOULD_BE_PRESERVED";
    let env_name2 = "ALSO_SHOULD_BE_PRESERVED";
    let env_val1 = "42";
    let env_val2 = "24";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = {env_name1}"),
        &format!("Defaults {env_list} += {env_name2}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val1), sudo_env.get(env_name1).copied());
    assert_eq!(Some(env_val2), sudo_env.get(env_name2).copied());

    Ok(())
}

fn plus_equal_repeated(env_list: EnvList) -> Result<()> {
    let env_name = "SHOULD_BE_PRESERVED";
    let env_val = "42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = {env_name}"),
        &format!("Defaults {env_list} += {env_name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val), sudo_env.get(env_name).copied());

    Ok(())
}

// see 'environment' section in `man sudo`
// the variables HOME, LOGNAME, MAIL and USER are set by sudo with a value that depends on the
// target user *unless* they appear in the env_keep list
fn vars_with_target_user_specific_values(env_list: EnvList) -> Result<()> {
    let home = "my-home";
    let logname = "my-logname";
    let mail = "my-mail";
    let user = "my-user";

    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"HOME LOGNAME MAIL USER\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("HOME={home}"))
        .arg(format!("LOGNAME={logname}"))
        .arg(format!("MAIL={mail}"))
        .arg(format!("USER={user}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(home), sudo_env.get("HOME").copied());
    assert_eq!(Some(logname), sudo_env.get("LOGNAME").copied());
    assert_eq!(Some(mail), sudo_env.get("MAIL").copied());
    assert_eq!(Some(user), sudo_env.get("USER").copied());

    Ok(())
}

// these variables cannot be preserved as they'll be set by sudo
fn sudo_env_vars(env_list: EnvList) -> Result<()> {
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"SUDO_COMMAND SUDO_GID SUDO_UID SUDO_USER\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg("SUDO_COMMAND=command")
        .arg("SUDO_GID=gid")
        .arg("SUDO_UID=uid")
        .arg("SUDO_USER=user")
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some("/usr/bin/env"), sudo_env.get("SUDO_COMMAND").copied());
    assert_eq!(Some("0"), sudo_env.get("SUDO_GID").copied());
    assert_eq!(Some("0"), sudo_env.get("SUDO_UID").copied());
    assert_eq!(Some("root"), sudo_env.get("SUDO_USER").copied());

    Ok(())
}

fn user_set_to_preserved_logname_value(env_list: EnvList) -> Result<()> {
    let value = "ghost";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"LOGNAME\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("LOGNAME={value}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(value), sudo_env.get("LOGNAME").copied());
    assert_eq!(Some(value), sudo_env.get("USER").copied());

    Ok(())
}

fn logname_set_to_preserved_user_value(env_list: EnvList) -> Result<()> {
    let value = "ghost";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"USER\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("USER={value}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(value), sudo_env.get("LOGNAME").copied());
    assert_eq!(Some(value), sudo_env.get("USER").copied());

    Ok(())
}

fn if_value_starts_with_parentheses_variable_is_removed(env_list: EnvList) -> Result<()> {
    let env_name = "SHOULD_BE_REMOVED";
    let env_val = "() 42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = {env_name}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(!sudo_env.contains_key(env_name));

    Ok(())
}

fn key_value_matches(env_list: EnvList) -> Result<()> {
    let env_name = "KEY";
    let env_val = "value";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"{env_name}={env_val}\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val), sudo_env.get(env_name).copied());

    Ok(())
}

fn key_value_no_match(env_list: EnvList) -> Result<()> {
    let env_name = "KEY";
    let env_val = "value";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"{env_name}={env_val}\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}=different-value"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(None, sudo_env.get(env_name));

    Ok(())
}

// without the double quotes the RHS is not interpreted as a key value pair
// also see the `key_value_matches` test
fn key_value_syntax_needs_double_quotes(env_list: EnvList) -> Result<()> {
    let env_name = "KEY";
    let env_val = "value";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = {env_name}={env_val}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(None, sudo_env.get(env_name));

    Ok(())
}

// also see the `if_value_starts_with_parentheses_variable_is_removed` test
fn key_value_where_value_is_parentheses_glob(env_list: EnvList) -> Result<()> {
    let env_name = "SHOULD_BE_PRESERVED";
    let env_val = "() 42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"{env_name}=()*\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val), sudo_env.get(env_name).copied());

    Ok(())
}

fn minus_equal_removes(env_list: EnvList) -> Result<()> {
    let env_name1 = "SHOULD_BE_PRESERVED";
    let env_val1 = "42";
    let env_name2 = "SHOULD_BE_REMOVED";
    let env_val2 = "24";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"{env_name1} {env_name2}\""),
        &format!("Defaults {env_list} -= {env_name2}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(env_val1), sudo_env.get(env_name1).copied());
    assert!(!sudo_env.contains_key(env_name2));

    Ok(())
}

fn minus_equal_an_element_not_in_the_list_is_not_an_error(env_list: EnvList) -> Result<()> {
    let env_name = "SHOULD_BE_REMOVED";
    let env_val = "42";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} -= {env_name}"),
    ])
    .build()?;

    let output = Command::new("env")
        .arg(format!("{env_name}={env_val}"))
        .args(["sudo", "env"])
        .output(&env)?;

    // no diagnostics in this case
    assert!(output.stderr().is_empty());

    let stdout = output.stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(!sudo_env.contains_key(env_name));

    Ok(())
}

fn bang_clears_the_whole_list(env_list: EnvList) -> Result<()> {
    let env_name1 = "SHOULD_BE_REMOVED";
    let env_name2 = "ALSO_SHOULD_BE_REMOVED";
    let env_val1 = "42";
    let env_val2 = "24";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"{env_name1} {env_name2}\""),
        &format!("Defaults !{env_list}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(!sudo_env.contains_key(env_name1));
    assert!(!sudo_env.contains_key(env_name1));

    Ok(())
}

fn can_append_after_bang(env_list: EnvList) -> Result<()> {
    let env_name1 = "SHOULD_BE_REMOVED";
    let env_name2 = "SHOULD_BE_PRESERVED";
    let env_val1 = "42";
    let env_val2 = "24";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = {env_name1}"),
        &format!("Defaults !{env_list}"),
        &format!("Defaults {env_list} += {env_name2}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(!sudo_env.contains_key(env_name1));
    assert_eq!(Some(env_val2), sudo_env.get(env_name2).copied());

    Ok(())
}

fn can_override_after_bang(env_list: EnvList) -> Result<()> {
    let env_name1 = "SHOULD_BE_REMOVED";
    let env_name2 = "SHOULD_BE_PRESERVED";
    let env_val1 = "42";
    let env_val2 = "24";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = {env_name1}"),
        &format!("Defaults !{env_list}"),
        &format!("Defaults {env_list} = {env_name2}"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(!sudo_env.contains_key(env_name1));
    assert_eq!(Some(env_val2), sudo_env.get(env_name2).copied());

    Ok(())
}

fn wildcard_works(env_list: EnvList) -> Result<()> {
    let kept_name1 = "FERRIS";
    let kept_value1 = "ferris";
    let kept_name2 = "FS";
    let kept_value2 = "fs";
    let discarded_name = "SF";
    let discarded_value = "sf";

    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = F*S"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{kept_name1}={kept_value1}"))
        .arg(format!("{kept_name2}={kept_value2}"))
        .arg(format!("{discarded_name}={discarded_value}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(kept_value1), sudo_env.get(kept_name1).copied());
    assert_eq!(Some(kept_value2), sudo_env.get(kept_name2).copied());
    assert_eq!(None, sudo_env.get(discarded_value).copied());

    Ok(())
}

fn double_wildcard_is_ok(env_list: EnvList) -> Result<()> {
    let kept_name1 = "FERRIS";
    let kept_value1 = "ferris";
    let kept_name2 = "FS";
    let kept_value2 = "fs";
    let discarded_name = "SF";
    let discarded_value = "sf";

    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = F**"),
        &format!("Defaults {env_list} += **S"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{kept_name1}={kept_value1}"))
        .arg(format!("{kept_name2}={kept_value2}"))
        .arg(format!("{discarded_name}={discarded_value}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(kept_value1), sudo_env.get(kept_name1).copied());
    assert_eq!(Some(kept_value2), sudo_env.get(kept_name2).copied());
    assert_eq!(None, sudo_env.get(discarded_value).copied());

    Ok(())
}

fn minus_equal_can_remove_wildcard(env_list: EnvList) -> Result<()> {
    let name1 = "FERRIS";
    let value1 = "ferris";
    let name2 = "F";
    let value2 = "f";

    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = F*"),
        &format!("Defaults {env_list} -= F*"),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{name1}={value1}"))
        .arg(format!("{name2}={value2}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(None, sudo_env.get(name1).copied());
    assert_eq!(None, sudo_env.get(name2).copied());

    Ok(())
}

fn accepts_uncommon_var_names(env_list: EnvList) -> Result<()> {
    let name1 = "00";
    let value1 = "42";
    let name2 = "0A";
    let value2 = "42";
    let name3 = "ferris";
    let value3 = "FERRIS";

    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"{name1} {name2} {name3}\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{name1}={value1}"))
        .arg(format!("{name2}={value2}"))
        .arg(format!("{name3}={value3}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(value1), sudo_env.get(name1).copied());
    assert_eq!(Some(value2), sudo_env.get(name2).copied());
    assert_eq!(Some(value3), sudo_env.get(name3).copied());

    Ok(())
}

fn skips_invalid_variable_names(env_list: EnvList) -> Result<()> {
    let kept_name = "FERRIS";
    let kept_value = "ferris";
    let discarded_name1 = "GHOST";
    let discarded_value1 = "ghost";
    let discarded_name2 = "G";
    let discarded_value2 = "g";

    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"G.* {kept_name}\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{kept_name}={kept_value}"))
        .arg(format!("{discarded_name1}={discarded_value1}"))
        .arg(format!("{discarded_name2}={discarded_value2}"))
        .args(["sudo", "env"])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(kept_value), sudo_env.get(kept_name).copied());
    assert_eq!(None, sudo_env.get(discarded_name1).copied());
    assert_eq!(None, sudo_env.get(discarded_name2).copied());

    Ok(())
}
