use pretty_assertions::assert_eq;
use sudo_cli::SudoOptions;

/// --preserve-env
/// Passing '-E' sets 'short_preserve_env' to true, 'preserve_env_list' stays empty
#[test]
fn short_preserve_env() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-E"]).unwrap();
    assert!(cmd.preserve_env);
    assert!(cmd.preserve_env_list.is_empty());
}

/// Passing '--preserve-env' sets 'short_preserve_env' to true, 'preserve_env_list' stays empty
#[test]
fn preserve_env_witout_var() {
    let cmd = SudoOptions::try_parse_from(["sudo", "--preserve-env"]).unwrap();
    assert!(cmd.preserve_env);
    assert!(cmd.preserve_env_list.is_empty());
}

/// Passing '-E' with a variable fails
#[test]
#[should_panic]
fn short_preserve_env_with_var_fails() {
    SudoOptions::try_parse_from(["sudo", "-E=variable"]).unwrap();
}

/// Passing '--preserve-env' with an argument fills 'preserve_env_list', 'short_preserve_env' stays 'false'
#[test]
fn preserve_env_with_var() {
    let cmd = SudoOptions::try_parse_from(["sudo", "--preserve-env=some_argument"]).unwrap();
    assert_eq!(cmd.preserve_env_list, vec!["some_argument"]);
    assert!(!cmd.preserve_env);
}

/// Passing '--preserve-env' with several arguments fills 'preserve_env_list', 'short_preserve_env' stays 'false'
#[test]
fn preserve_env_with_several_vars() {
    let cmd = SudoOptions::try_parse_from([
        "sudo",
        "--preserve-env=some_argument,another_argument,a_third_one",
    ])
    .unwrap();
    assert_eq!(
        cmd.preserve_env_list,
        vec!["some_argument", "another_argument", "a_third_one"]
    );
    assert!(!cmd.preserve_env);
}

/// Catch env variable that is given without hyphens in 'VAR=value' form in env_var_list.
/// external_args stay empty.
#[test]
fn env_variable() {
    let cmd = SudoOptions::try_parse_from(["sudo", "ENV=with_a_value"]).unwrap();
    assert_eq!(
        cmd.env_var_list,
        vec![("ENV".to_owned(), "with_a_value".to_owned())]
    );
    assert!(cmd.external_args.is_empty());
}

/// Catch several env variablse that are given without hyphens in 'VAR=value' form in env_var_list.
/// external_args stay empty.
#[test]
fn several_env_variables() {
    let cmd = SudoOptions::try_parse_from([
        "sudo",
        "ENV=with_a_value",
        "another_var=otherval",
        "more=this_is_a_val",
    ])
    .unwrap();
    assert_eq!(
        cmd.env_var_list,
        vec![
            ("ENV".to_owned(), "with_a_value".to_owned()),
            ("another_var".to_owned(), "otherval".to_owned()),
            ("more".to_owned(), "this_is_a_val".to_owned())
        ]
    );
    assert!(cmd.external_args.is_empty());
}

/// Mix env variables and trailing arguments that just pass through sudo
/// Divided by hyphens.
#[test]
fn mix_env_variables_with_trailing_args_divided_by_hyphens() {
    let cmd = SudoOptions::try_parse_from(["sudo", "env=var", "--", "external=args", "something"])
        .unwrap();
    assert_eq!(cmd.env_var_list, vec![("env".to_owned(), "var".to_owned())]);
    assert_eq!(cmd.external_args, vec!["external=args", "something"]);
}

/// Mix env variables and trailing arguments that just pass through sudo
/// Divided by known flag.
// Currently panics.
#[test]
fn mix_env_variables_with_trailing_args_divided_by_known_flag() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-b", "external=args", "something"]).unwrap();
    assert_eq!(
        cmd.env_var_list,
        vec![("external".to_owned(), "args".to_owned())]
    );
    assert_eq!(cmd.external_args, vec!["something"]);
    assert!(cmd.background);
}

/// Catch trailing arguments that just pass through sudo
/// but look like a known flag.
#[test]
fn trailing_args_followed_by_known_flag() {
    let cmd =
        SudoOptions::try_parse_from(["sudo", "args", "followed_by", "known_flag", "-b"]).unwrap();
    assert!(!cmd.background);
    assert_eq!(
        cmd.external_args,
        vec!["args", "followed_by", "known_flag", "-b"]
    );
}

/// Catch trailing arguments that just pass through sudo
/// but look like a known flag, divided by hyphens.
#[test]
fn trailing_args_hyphens_known_flag() {
    let cmd = SudoOptions::try_parse_from([
        "sudo",
        "--",
        "trailing",
        "args",
        "followed_by",
        "known_flag",
        "-b",
    ])
    .unwrap();
    assert!(!cmd.background);
    assert_eq!(
        cmd.external_args,
        vec!["trailing", "args", "followed_by", "known_flag", "-b"]
    );
}

/// Flags that exclude each other
#[test]
#[should_panic]
fn remove_and_reset_timestamp_exclusion() {
    SudoOptions::try_parse_from(["sudo", "--reset-timestamp", "--reboot-timestamp"]).unwrap();
}
