use crate::Result;

const ENV_LIST: super::EnvList = super::EnvList::Check;

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
#[ignore]
fn key_value_matches() -> Result<()> {
    super::key_value_matches(ENV_LIST)
}

#[test]
fn key_value_no_match() -> Result<()> {
    super::key_value_no_match(ENV_LIST)
}

#[test]
#[ignore]
fn key_value_syntax_needs_double_quotes() -> Result<()> {
    super::key_value_syntax_needs_double_quotes(ENV_LIST)
}

#[test]
#[ignore]
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
