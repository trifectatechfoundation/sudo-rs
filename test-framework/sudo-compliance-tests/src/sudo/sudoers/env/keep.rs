use sudo_test::{Command, Env};

use crate::{SUDO_ENV_DEFAULT_PATH, SUDOERS_ALL_ALL_NOPASSWD, helpers};

const ENV_LIST: crate::EnvList = crate::EnvList::Keep;

#[test]
fn equal_single() {
    super::equal_single(ENV_LIST);
}

#[test]
fn equal_multiple() {
    super::equal_multiple(ENV_LIST);
}

#[test]
fn equal_repeated() {
    super::equal_repeated(ENV_LIST);
}

#[test]
fn equal_overrides() {
    super::equal_overrides(ENV_LIST);
}

#[test]
fn plus_equal_on_empty_set() {
    super::plus_equal_on_empty_set(ENV_LIST);
}

#[test]
fn plus_equal_appends() {
    super::plus_equal_appends(ENV_LIST);
}

#[test]
fn plus_equal_repeated() {
    super::plus_equal_repeated(ENV_LIST);
}

#[test]
fn vars_with_target_user_specific_values() {
    super::vars_with_target_user_specific_values(ENV_LIST);
}

#[test]
fn sudo_env_vars() {
    super::sudo_env_vars(ENV_LIST);
}

#[test]
fn user_set_to_preserved_logname_value() {
    super::user_set_to_preserved_logname_value(ENV_LIST);
}

#[test]
fn logname_set_to_preserved_user_value() {
    super::logname_set_to_preserved_user_value(ENV_LIST);
}

#[test]
fn if_value_starts_with_parentheses_variable_is_removed() {
    super::if_value_starts_with_parentheses_variable_is_removed(ENV_LIST);
}

#[test]
fn key_value_matches() {
    super::key_value_matches(ENV_LIST);
}

#[test]
fn key_value_no_match() {
    super::key_value_no_match(ENV_LIST);
}

#[test]
fn key_value_syntax_needs_double_quotes() {
    super::key_value_syntax_needs_double_quotes(ENV_LIST);
}

#[test]
#[ignore = "gh346"]
fn key_value_where_value_is_parentheses_glob() {
    super::key_value_where_value_is_parentheses_glob(ENV_LIST);
}

#[test]
fn minus_equal_removes() {
    super::minus_equal_removes(ENV_LIST);
}

#[test]
fn minus_equal_an_element_not_in_the_list_is_not_an_error() {
    super::minus_equal_an_element_not_in_the_list_is_not_an_error(ENV_LIST);
}

#[test]
fn bang_clears_the_whole_list() {
    super::bang_clears_the_whole_list(ENV_LIST);
}

#[test]
fn can_append_after_bang() {
    super::can_append_after_bang(ENV_LIST);
}

#[test]
fn can_override_after_bang() {
    super::can_override_after_bang(ENV_LIST);
}

#[test]
fn wildcard_works() {
    super::wildcard_works(ENV_LIST);
}

#[test]
fn double_wildcard_is_ok() {
    super::double_wildcard_is_ok(ENV_LIST);
}

#[test]
fn minus_equal_can_remove_wildcard() {
    super::minus_equal_can_remove_wildcard(ENV_LIST);
}

#[test]
fn accepts_uncommon_var_names() {
    super::accepts_uncommon_var_names(ENV_LIST);
}

#[test]
fn skips_invalid_variable_names() {
    super::skips_invalid_variable_names(ENV_LIST);
}

// DISPLAY, PATH and TERM are env vars preserved by sudo by default
// they appear to be part of the default `env_keep` list
#[test]
fn equal_can_disable_preservation_of_vars_display_path_but_not_term() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults env_keep = WHATEVER"]).build();

    let sudo_abs_path = Command::new("which").arg("sudo").output(&env).stdout();
    let env_abs_path = Command::new("which").arg("env").output(&env).stdout();

    let term = "some-term";
    let stdout = Command::new("env")
        .arg("PATH=some-path")
        .arg("DISPLAY=some-display")
        .arg(format!("TERM={term}"))
        .args([sudo_abs_path, env_abs_path])
        .output(&env)
        .stdout();

    let sudo_env = helpers::parse_env_output(&stdout);

    // can be disabled
    assert!(!sudo_env.contains_key("DISPLAY"));
    assert_eq!(Some(SUDO_ENV_DEFAULT_PATH), sudo_env.get("PATH").copied());

    // cannot be disabled
    assert_eq!(Some(term), sudo_env.get("TERM").copied());
}

#[test]
fn minus_equal_can_disable_preservation_of_vars_display_path_but_not_term() {
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        "Defaults env_keep -= \"DISPLAY PATH TERM\"",
    ])
    .build();

    let sudo_abs_path = Command::new("which").arg("sudo").output(&env).stdout();
    let env_abs_path = Command::new("which").arg("env").output(&env).stdout();

    let term = "some-term";
    let stdout = Command::new("env")
        .arg("PATH=some-path")
        .arg("DISPLAY=some-display")
        .arg(format!("TERM={term}"))
        .args([sudo_abs_path, env_abs_path])
        .output(&env)
        .stdout();

    let sudo_env = helpers::parse_env_output(&stdout);

    // can be disabled
    assert!(!sudo_env.contains_key("DISPLAY"));
    assert_eq!(Some(SUDO_ENV_DEFAULT_PATH), sudo_env.get("PATH").copied());

    // cannot be disabled
    assert_eq!(Some(term), sudo_env.get("TERM").copied());
}

#[test]
fn bang_can_disable_preservation_of_vars_display_path_but_not_term() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults !env_keep"]).build();

    let sudo_abs_path = Command::new("which").arg("sudo").output(&env).stdout();
    let env_abs_path = Command::new("which").arg("env").output(&env).stdout();

    let term = "some-term";
    let stdout = Command::new("env")
        .arg("PATH=some-path")
        .arg("DISPLAY=some-display")
        .arg(format!("TERM={term}"))
        .args([sudo_abs_path, env_abs_path])
        .output(&env)
        .stdout();

    let sudo_env = helpers::parse_env_output(&stdout);

    // can be disabled
    assert!(!sudo_env.contains_key("DISPLAY"));
    assert_eq!(Some(SUDO_ENV_DEFAULT_PATH), sudo_env.get("PATH").copied());

    // cannot be disabled
    assert_eq!(Some(term), sudo_env.get("TERM").copied());
}

#[test]
fn checks_not_applied() {
    let name = "SHOULD_BE_PRESERVED";
    let value = "4%2";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_keep = {name}"),
    ])
    .build();

    let stdout = Command::new("env")
        .arg(format!("{name}={value}"))
        .args(["sudo", "env"])
        .output(&env)
        .stdout();
    let sudo_env = helpers::parse_env_output(&stdout);

    assert_eq!(Some(value), sudo_env.get(name).copied());
}

#[test]
fn can_set_from_commandline() {
    let name = "CAN_BE_SET";
    let value = "4%2";
    for sudoers in [
        [
            "ALL ALL=(ALL:ALL) NOPASSWD: /usr/bin/env",
            &format!("Defaults env_keep = {name}"),
        ],
        [
            // SETENV overrides checks
            "ALL ALL=(ALL:ALL) NOPASSWD: SETENV: /usr/bin/env",
            &format!("Defaults env_delete = {name}"),
        ],
        [
            // ALL has an implicit SETENV
            "ALL ALL=(ALL:ALL) NOPASSWD: ALL",
            &format!("Defaults env_delete = {name}"),
        ],
        [
            // SETENV is sticky
            "ALL ALL=(ALL:ALL) NOPASSWD: SETENV: /bin/ls, (ALL:ALL) /usr/bin/env",
            &format!("Defaults env_delete = {name}"),
        ],
        [
            // ordering can be important (see below)
            "ALL ALL=(ALL:ALL) NOPASSWD: /usr/bin/env, ALL",
            "",
        ],
    ] {
        let env = Env(sudoers).build();

        let stdout = Command::new("sudo")
            .args([format!("{name}={value}"), "env".to_string()])
            .output(&env)
            .stdout();
        let sudo_env = helpers::parse_env_output(&stdout);

        assert_eq!(Some(value), sudo_env.get(name).copied());
    }
}

#[test]
fn cannot_set_from_commandline() {
    let name = "CANNOT_BE_SET";
    let value = "42";

    for sudoers in [
        ["ALL ALL=(ALL:ALL) NOPASSWD: /usr/bin/env"],
        ["ALL ALL=(ALL:ALL) NOPASSWD: NOSETENV: /usr/bin/env"],
        ["ALL ALL=(ALL:ALL) NOPASSWD: NOSETENV: ALL"],
        ["ALL ALL=(ALL:ALL) NOPASSWD: NOSETENV: ALL, (ALL:ALL) /usr/bin/env"],
        ["ALL ALL=(ALL:ALL) NOPASSWD: ALL, /usr/bin/env"],
    ] {
        let env = Env(sudoers).build();

        let output = Command::new("sudo")
            .args([format!("{name}={value}"), "env".to_string()])
            .output(&env);

        output.assert_exit_code(1);
        assert_contains!(
            output.stderr(),
            format!("you are not allowed to set the following environment variables: {name}")
        );
    }
}

#[test]
#[ignore = "gh760"]
fn can_surprisingly_be_set_from_commandline() {
    let name = "CAN_BE_SET";
    let value = "42";

    let env = Env(["ALL ALL=(ALL:ALL) NOPASSWD: NOSETENV: /usr/bin/env, ALL"]).build();

    let stdout = Command::new("sudo")
        .args([format!("{name}={value}"), "env".to_string()])
        .output(&env)
        .stdout();
    let sudo_env = helpers::parse_env_output(&stdout);

    assert_eq!(Some(value), sudo_env.get(name).copied());
}
