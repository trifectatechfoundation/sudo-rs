use sudo_test::{Command, Env};

use crate::{helpers, Result, SUDOERS_ALL_ALL_NOPASSWD, SUDO_ENV_DEFAULT_PATH};

const ENV_LIST: crate::EnvList = crate::EnvList::Keep;

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
fn skips_invalid_variable_names() -> Result<()> {
    super::skips_invalid_variable_names(ENV_LIST)
}

// DISPLAY, PATH and TERM are env vars preserved by sudo by default
// they appear to be part of the default `env_keep` list
#[test]
fn equal_can_disable_preservation_of_vars_display_path_but_not_term() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults env_keep = WHATEVER"]).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").output(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").output(&env)?.stdout()?;

    let term = "some-term";
    let stdout = Command::new("env")
        .arg("PATH=some-path")
        .arg("DISPLAY=some-display")
        .arg(format!("TERM={term}"))
        .args([sudo_abs_path, env_abs_path])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    // can be disabled
    assert!(!sudo_env.contains_key("DISPLAY"));
    assert_eq!(Some(SUDO_ENV_DEFAULT_PATH), sudo_env.get("PATH").copied());

    // cannot be disabled
    assert_eq!(Some(term), sudo_env.get("TERM").copied());

    Ok(())
}

#[test]
fn minus_equal_can_disable_preservation_of_vars_display_path_but_not_term() -> Result<()> {
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        "Defaults env_keep -= \"DISPLAY PATH TERM\"",
    ])
    .build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").output(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").output(&env)?.stdout()?;

    let term = "some-term";
    let stdout = Command::new("env")
        .arg("PATH=some-path")
        .arg("DISPLAY=some-display")
        .arg(format!("TERM={term}"))
        .args([sudo_abs_path, env_abs_path])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    // can be disabled
    assert!(!sudo_env.contains_key("DISPLAY"));
    assert_eq!(Some(SUDO_ENV_DEFAULT_PATH), sudo_env.get("PATH").copied());

    // cannot be disabled
    assert_eq!(Some(term), sudo_env.get("TERM").copied());

    Ok(())
}

#[test]
fn bang_can_disable_preservation_of_vars_display_path_but_not_term() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults !env_keep"]).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").output(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").output(&env)?.stdout()?;

    let term = "some-term";
    let stdout = Command::new("env")
        .arg("PATH=some-path")
        .arg("DISPLAY=some-display")
        .arg(format!("TERM={term}"))
        .args([sudo_abs_path, env_abs_path])
        .output(&env)?
        .stdout()?;

    let sudo_env = helpers::parse_env_output(&stdout)?;

    // can be disabled
    assert!(!sudo_env.contains_key("DISPLAY"));
    assert_eq!(Some(SUDO_ENV_DEFAULT_PATH), sudo_env.get("PATH").copied());

    // cannot be disabled
    assert_eq!(Some(term), sudo_env.get("TERM").copied());

    Ok(())
}

#[test]
fn checks_not_applied() -> Result<()> {
    let name = "SHOULD_BE_PRESERVED";
    let value = "4%2";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
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
