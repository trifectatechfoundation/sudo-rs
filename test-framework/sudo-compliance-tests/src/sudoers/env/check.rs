use std::iter;

use sudo_test::{Command, Env};

use crate::{helpers, Result, SUDOERS_ALL_ALL_NOPASSWD, SUDO_ENV_DEFAULT_TERM};

use super::BAD_TZ_VALUES;

const ENV_LIST: crate::EnvList = crate::EnvList::Check;

#[test]
fn equal_single() -> Result<()> {
    super::equal_single(ENV_LIST)
}

#[test]
fn equal_multiple() -> Result<()> {
    super::equal_multiple(ENV_LIST)
}

#[test]
fn equal_repeated() -> Result<()> {
    super::equal_repeated(ENV_LIST)
}

#[test]
fn equal_overrides() -> Result<()> {
    super::equal_overrides(ENV_LIST)
}

#[test]
fn plus_equal_on_empty_set() -> Result<()> {
    super::plus_equal_on_empty_set(ENV_LIST)
}

#[test]
fn plus_equal_appends() -> Result<()> {
    super::plus_equal_appends(ENV_LIST)
}

#[test]
fn plus_equal_repeated() -> Result<()> {
    super::plus_equal_repeated(ENV_LIST)
}

#[test]
fn vars_with_target_user_specific_values() -> Result<()> {
    super::vars_with_target_user_specific_values(ENV_LIST)
}

#[test]
fn sudo_env_vars() -> Result<()> {
    super::sudo_env_vars(ENV_LIST)
}

#[test]
fn user_set_to_preserved_logname_value() -> Result<()> {
    super::user_set_to_preserved_logname_value(ENV_LIST)
}

#[test]
fn logname_set_to_preserved_user_value() -> Result<()> {
    super::logname_set_to_preserved_user_value(ENV_LIST)
}

#[test]
fn if_value_starts_with_parentheses_variable_is_removed() -> Result<()> {
    super::if_value_starts_with_parentheses_variable_is_removed(ENV_LIST)
}

#[test]
#[ignore = "gh344"]
fn key_value_matches() -> Result<()> {
    super::key_value_matches(ENV_LIST)
}

#[test]
fn key_value_no_match() -> Result<()> {
    super::key_value_no_match(ENV_LIST)
}

#[test]
#[ignore = "gh345"]
fn key_value_syntax_needs_double_quotes() -> Result<()> {
    super::key_value_syntax_needs_double_quotes(ENV_LIST)
}

#[test]
#[ignore = "gh346"]
fn key_value_where_value_is_parentheses_glob() -> Result<()> {
    super::key_value_where_value_is_parentheses_glob(ENV_LIST)
}

#[test]
fn minus_equal_removes() -> Result<()> {
    super::minus_equal_removes(ENV_LIST)
}

#[test]
fn minus_equal_an_element_not_in_the_list_is_not_an_error() -> Result<()> {
    super::minus_equal_an_element_not_in_the_list_is_not_an_error(ENV_LIST)
}

#[test]
fn bang_clears_the_whole_list() -> Result<()> {
    super::bang_clears_the_whole_list(ENV_LIST)
}

#[test]
fn can_append_after_bang() -> Result<()> {
    super::can_append_after_bang(ENV_LIST)
}

#[test]
fn can_override_after_bang() -> Result<()> {
    super::can_override_after_bang(ENV_LIST)
}

#[test]
fn wildcard_works() -> Result<()> {
    super::wildcard_works(ENV_LIST)
}

#[test]
fn double_wildcard_is_ok() -> Result<()> {
    super::double_wildcard_is_ok(ENV_LIST)
}

#[test]
fn minus_equal_can_remove_wildcard() -> Result<()> {
    super::minus_equal_can_remove_wildcard(ENV_LIST)
}

#[test]
fn accepts_uncommon_var_names() -> Result<()> {
    super::accepts_uncommon_var_names(ENV_LIST)
}

#[test]
#[ignore = "gh384"]
fn skips_invalid_variable_names() -> Result<()> {
    super::skips_invalid_variable_names(ENV_LIST)
}

#[test]
fn equal_can_disable_preservation_of_vars_term_but_not_display_path() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults env_check = WHATEVER"]).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").exec(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").exec(&env)?.stdout()?;

    let display = "some-display";
    let path = "some-path";
    let stdout = Command::new("env")
        .arg(format!("PATH={path}"))
        .arg(format!("DISPLAY={display}"))
        .arg("TERM=some-term")
        .args([sudo_abs_path, env_abs_path])
        .exec(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    // can be disabled
    assert_eq!(Some(SUDO_ENV_DEFAULT_TERM), sudo_env.get("TERM").copied());

    // cannot be disabled
    assert_eq!(Some(display), sudo_env.get("DISPLAY").copied());
    assert_eq!(Some(path), sudo_env.get("PATH").copied());

    Ok(())
}

#[test]
fn minus_equal_can_disable_preservation_of_vars_term_but_not_display_path() -> Result<()> {
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        "Defaults env_check -= \"DISPLAY PATH TERM\"",
    ])
    .build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").exec(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").exec(&env)?.stdout()?;

    let display = "some-display";
    let path = "some-path";
    let stdout = Command::new("env")
        .arg(format!("PATH={path}"))
        .arg(format!("DISPLAY={display}"))
        .arg("TERM=some-term")
        .args([sudo_abs_path, env_abs_path])
        .exec(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    // can be disabled
    assert_eq!(Some(SUDO_ENV_DEFAULT_TERM), sudo_env.get("TERM").copied());

    // cannot be disabled
    assert_eq!(Some(display), sudo_env.get("DISPLAY").copied());
    assert_eq!(Some(path), sudo_env.get("PATH").copied());

    Ok(())
}

#[test]
fn bang_can_disable_preservation_of_vars_term_but_not_display_path() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults !env_check"]).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").exec(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").exec(&env)?.stdout()?;

    let display = "some-display";
    let path = "some-path";
    let stdout = Command::new("env")
        .arg(format!("PATH={path}"))
        .arg(format!("DISPLAY={display}"))
        .arg("TERM=some-term")
        .args([sudo_abs_path, env_abs_path])
        .exec(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    // can be disabled
    assert_eq!(Some(SUDO_ENV_DEFAULT_TERM), sudo_env.get("TERM").copied());

    // cannot be disabled
    assert_eq!(Some(display), sudo_env.get("DISPLAY").copied());
    assert_eq!(Some(path), sudo_env.get("PATH").copied());

    Ok(())
}

#[test]
fn vars_not_preserved_if_they_fail_checks() -> Result<()> {
    let env_name1 = "SHOULD_NOT_BE_PRESERVED";
    let env_name2 = "ALSO_SHOULD_NOT_BE_PRESERVED";
    let env_val1 = "4/2";
    let env_val2 = "2%4";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_check = \"{env_name1} {env_name2}\""),
    ])
    .build()?;

    let stdout = Command::new("env")
        .arg(format!("{env_name1}={env_val1}"))
        .arg(format!("{env_name2}={env_val2}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(None, sudo_env.get(env_name1).copied());
    assert_eq!(None, sudo_env.get(env_name2).copied());

    Ok(())
}

const TZ: &str = "TZ";

// the TZ is variable a different set of checks
// see the 'SUDOERS OPTIONS' section in `man sudoers`
#[test]
fn good_tz() -> Result<()> {
    let values = [
        // https://www.gnu.org/software/libc/manual/html_node/TZ-Variable.html
        // ^ documents valid TZ variable formats

        // first format
        "EST+5",
        "ABC+9",
        "XYZ-9",
        "ZER+0",
        "ZER-0",
        "ONE",
        // second format
        "EST+5EDT,M3.2.0/2,M11.1.0/2",
        // third format
        "/usr/share/zoneinfo/Europe/Berlin",
        ":/usr/share/zoneinfo/Europe/Berlin",
        "/usr/share/zoneinfo/",
        "/usr/share/zoneinfo/does/not/exist",
        "/usr/share/zoneinfo/%",
    ];

    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_check = {TZ}"),
    ])
    .build()?;

    for value in values {
        let stdout = Command::new("env")
            .arg(format!("{TZ}={value}"))
            .args(["sudo", "env"])
            .exec(&env)?
            .stdout()?;
        let sudo_env = helpers::parse_env_output(&stdout)?;

        assert_eq!(Some(value), sudo_env.get(TZ).copied());
    }

    Ok(())
}

#[test]
fn bad_tz() -> Result<()> {
    // according to "linux/include/uapi/linux/limits.h" as of Linux 6.3.4
    const PATH_MAX: usize = 4096;

    // "It is longer than the value of PATH_MAX."
    let long_path = "/usr/share/zoneinfo/"
        .chars()
        .chain(iter::repeat('a').take(PATH_MAX))
        .collect::<String>();

    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_check = {TZ}"),
    ])
    .build()?;

    for value in BAD_TZ_VALUES
        .iter()
        .copied()
        .chain(iter::once(long_path.as_str()))
    {
        let stdout = Command::new("env")
            .arg(format!("{TZ}={value}"))
            .args(["sudo", "env"])
            .exec(&env)?
            .stdout()?;
        let sudo_env = helpers::parse_env_output(&stdout)?;

        assert_eq!(None, sudo_env.get(TZ).copied());
    }

    Ok(())
}

#[test]
fn tz_is_in_default_list() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let value = "EST+5";
    let stdout = Command::new("env")
        .arg(format!("{TZ}={value}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(value), sudo_env.get(TZ).copied());

    let value = "../invalid";
    let stdout = Command::new("env")
        .arg(format!("{TZ}={value}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(None, sudo_env.get(TZ).copied());

    Ok(())
}
