use std::path::PathBuf;

use super::{SudoAction, SudoOptions};
use pretty_assertions::assert_eq;

/// Passing '-E' with a variable fails
#[test]
fn short_preserve_env_with_var_fails() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-E=variable"]);
    assert!(cmd.is_err())
}

/// Passing '--preserve-env' with an argument fills 'preserve_env', 'short_preserve_env' stays 'false'
#[test]
fn preserve_env_with_var() {
    let cmd = SudoOptions::try_parse_from(["sudo", "--preserve-env=some_argument"]).unwrap();
    assert_eq!(cmd.preserve_env, vec!["some_argument"]);
}

/// Passing '--preserve-env' with several arguments fills 'preserve_env', 'short_preserve_env' stays 'false'
#[test]
fn preserve_env_with_several_vars() {
    let cmd = SudoOptions::try_parse_from([
        "sudo",
        "--preserve-env=some_argument,another_argument,a_third_one",
    ])
    .unwrap();
    assert_eq!(
        cmd.preserve_env,
        vec!["some_argument", "another_argument", "a_third_one"]
    );
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
    assert!(cmd.args().is_empty());
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
    assert!(cmd.args().is_empty());
}

/// Mix env variables and trailing arguments that just pass through sudo
/// Divided by hyphens.
#[test]
fn mix_env_variables_with_trailing_args_divided_by_hyphens() {
    let cmd = SudoOptions::try_parse_from(["sudo", "env=var", "--", "external=args", "something"])
        .unwrap();
    assert_eq!(cmd.env_var_list, vec![("env".to_owned(), "var".to_owned())]);
    assert_eq!(cmd.args(), vec!["external=args", "something"]);
}

/// Mix env variables and trailing arguments that just pass through sudo
/// Divided by known flag.
#[test]
fn mix_env_variables_with_trailing_args_divided_by_known_flag() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-b", "external=args", "something"]).unwrap();
    assert_eq!(
        cmd.env_var_list,
        vec![("external".to_owned(), "args".to_owned())]
    );
    assert!(cmd.background);
    assert_eq!(cmd.args(), vec!["something"]);
}

/// Catch trailing arguments that just pass through sudo
/// but look like a known flag.
#[test]
fn trailing_args_followed_by_known_flag() {
    let cmd =
        SudoOptions::try_parse_from(["sudo", "args", "followed_by", "known_flag", "-b"]).unwrap();
    assert!(!cmd.background);
    assert_eq!(cmd.args(), vec!["args", "followed_by", "known_flag", "-b"]);
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
        cmd.args(),
        vec!["trailing", "args", "followed_by", "known_flag", "-b"]
    );
}

/// Check that the first environment variable declaration before any command is not treated as part
/// of the command.
#[test]
fn first_trailing_env_var_is_not_an_external_arg() {
    let cmd = SudoOptions::try_parse_from(["sudo", "FOO=1", "command", "BAR=2"]).unwrap();
    assert_eq!(cmd.env_var_list, vec![("FOO".to_owned(), "1".to_owned()),]);
    assert_eq!(
        cmd.action,
        SudoAction::Run(["command", "BAR=2"].map(String::from).to_vec())
    );
}

#[test]
fn trailing_env_vars_are_external_args() {
    let cmd = SudoOptions::try_parse_from([
        "sudo", "FOO=1", "-b", "BAR=2", "command", "BAZ=3", "arg", "FOOBAR=4", "command", "arg",
        "BARBAZ=5",
    ])
    .unwrap();
    assert!(cmd.background);
    assert_eq!(
        cmd.env_var_list,
        vec![
            ("FOO".to_owned(), "1".to_owned()),
            ("BAR".to_owned(), "2".to_owned())
        ]
    );
    assert_eq!(
        cmd.args(),
        vec!["command", "BAZ=3", "arg", "FOOBAR=4", "command", "arg", "BARBAZ=5"]
    );
}

#[test]
fn single_env_var_declaration() {
    let cmd = SudoOptions::try_parse_from(["sudo", "FOO=1", "command"]).unwrap();
    assert_eq!(cmd.env_var_list, vec![("FOO".to_owned(), "1".to_owned())]);
    assert_eq!(cmd.args(), vec!["command"]);
}

#[test]
fn shorthand_with_argument() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-u", "ferris"]).unwrap();
    assert_eq!(cmd.user.as_deref(), Some("ferris"));
}

#[test]
fn shorthand_with_direct_argument() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-uferris"]).unwrap();
    assert_eq!(cmd.user.as_deref(), Some("ferris"));
}

#[test]
fn shorthand_without_argument() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-u"]);
    assert!(cmd.is_err())
}

#[test]
fn non_interactive() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-n"]).unwrap();
    assert!(cmd.non_interactive);

    let cmd = SudoOptions::try_parse_from(["sudo", "--non-interactive"]).unwrap();
    assert!(cmd.non_interactive);
}

#[test]
fn preserve_groups() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-P"]).unwrap();
    assert!(cmd.preserve_groups);

    let cmd = SudoOptions::try_parse_from(["sudo", "--preserve-groups"]).unwrap();
    assert!(cmd.preserve_groups);
}

#[test]
fn stdin() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-S"]).unwrap();
    assert!(cmd.stdin);

    let cmd = SudoOptions::try_parse_from(["sudo", "--stdin"]).unwrap();
    assert!(cmd.stdin);
}

#[test]
fn shell() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-s"]).unwrap();
    assert!(cmd.shell);

    let cmd = SudoOptions::try_parse_from(["sudo", "--shell"]).unwrap();
    assert!(cmd.shell);
}

#[test]
fn directory() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-D/some/path"]).unwrap();
    assert_eq!(cmd.directory, Some(PathBuf::from("/some/path")));

    let cmd = SudoOptions::try_parse_from(["sudo", "--chdir", "/some/path"]).unwrap();
    assert_eq!(cmd.directory, Some(PathBuf::from("/some/path")));

    let cmd = SudoOptions::try_parse_from(["sudo", "--chdir=/some/path"]).unwrap();
    assert_eq!(cmd.directory, Some(PathBuf::from("/some/path")));
}

#[test]
fn group() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-grustaceans"]).unwrap();
    assert_eq!(cmd.group.as_deref(), Some("rustaceans"));

    let cmd = SudoOptions::try_parse_from(["sudo", "--group", "rustaceans"]).unwrap();
    assert_eq!(cmd.group.as_deref(), Some("rustaceans"));

    let cmd = SudoOptions::try_parse_from(["sudo", "--group=rustaceans"]).unwrap();
    assert_eq!(cmd.group.as_deref(), Some("rustaceans"));
}

#[test]
fn host() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-hlilo"]).unwrap();
    assert_eq!(cmd.host.as_deref(), Some("lilo"));

    let cmd = SudoOptions::try_parse_from(["sudo", "--host", "lilo"]).unwrap();
    assert_eq!(cmd.host.as_deref(), Some("lilo"));

    let cmd = SudoOptions::try_parse_from(["sudo", "--host=lilo"]).unwrap();
    assert_eq!(cmd.host.as_deref(), Some("lilo"));
}

#[test]
fn chroot() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-R/some/path"]).unwrap();
    assert_eq!(cmd.chroot, Some(PathBuf::from("/some/path")));

    let cmd = SudoOptions::try_parse_from(["sudo", "--chroot", "/some/path"]).unwrap();
    assert_eq!(cmd.chroot, Some(PathBuf::from("/some/path")));

    let cmd = SudoOptions::try_parse_from(["sudo", "--chroot=/some/path"]).unwrap();
    assert_eq!(cmd.chroot, Some(PathBuf::from("/some/path")));
}

#[test]
fn other_user() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-Uferris"]).unwrap();
    assert_eq!(cmd.other_user.as_deref(), Some("ferris"));

    let cmd = SudoOptions::try_parse_from(["sudo", "--other-user", "ferris"]).unwrap();
    assert_eq!(cmd.other_user.as_deref(), Some("ferris"));

    let cmd = SudoOptions::try_parse_from(["sudo", "--other-user=ferris"]).unwrap();
    assert_eq!(cmd.other_user.as_deref(), Some("ferris"));
}

#[test]
fn invalid_option() {
    let cmd = SudoOptions::try_parse_from(["sudo", "--wololo"]);
    assert!(cmd.is_err())
}

#[test]
fn invalid_option_with_argument() {
    let cmd = SudoOptions::try_parse_from(["sudo", "--background=yes"]);
    assert!(cmd.is_err())
}

#[test]
fn no_argument_provided() {
    let cmd = SudoOptions::try_parse_from(["sudo", "--user"]);
    assert!(cmd.is_err())
}

#[test]
fn login() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-i"]).unwrap();
    assert!(cmd.login);

    let cmd = SudoOptions::try_parse_from(["sudo", "--login"]).unwrap();
    assert!(cmd.login);
}

#[test]
fn edit() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-e"]).unwrap();
    assert_eq!(cmd.action, SudoAction::Edit(vec![]));

    let cmd = SudoOptions::try_parse_from(["sudo", "--edit"]).unwrap();
    assert_eq!(cmd.action, SudoAction::Edit(vec![]));
}

#[test]
fn help() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-h"]).unwrap();
    assert_eq!(cmd.action, SudoAction::Help);

    let cmd = SudoOptions::try_parse_from(["sudo", "-bh"]).unwrap();
    assert_eq!(cmd.action, SudoAction::Help);

    let cmd = SudoOptions::try_parse_from(["sudo", "--help"]).unwrap();
    assert_eq!(cmd.action, SudoAction::Help);
}

#[test]
fn conflicting_arguments() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-K", "-k"]);
    assert!(cmd.is_err());

    let cmd = SudoOptions::try_parse_from(["sudo", "--remove-timestamp", "--reset-timestamp"]);
    assert!(cmd.is_err());

    let cmd = SudoOptions::try_parse_from(["sudo", "-K"]).unwrap();
    assert_eq!(cmd.action, SudoAction::RemoveTimestamp);

    let cmd = SudoOptions::try_parse_from(["sudo", "-k"]).unwrap();
    assert_eq!(cmd.action, SudoAction::ResetTimestamp);
}

#[test]
fn list() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-l"]).unwrap();
    assert_eq!(cmd.action, SudoAction::List(vec![]));

    let cmd = SudoOptions::try_parse_from(["sudo", "--list"]).unwrap();
    assert_eq!(cmd.action, SudoAction::List(vec![]));
}

#[test]
fn validate() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-v"]).unwrap();
    assert_eq!(cmd.action, SudoAction::Validate);

    let cmd = SudoOptions::try_parse_from(["sudo", "--validate"]).unwrap();
    assert_eq!(cmd.action, SudoAction::Validate);
}

#[test]
fn version() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-V"]).unwrap();
    assert_eq!(cmd.action, SudoAction::Version);

    let cmd = SudoOptions::try_parse_from(["sudo", "--version"]).unwrap();
    assert_eq!(cmd.action, SudoAction::Version);
}
