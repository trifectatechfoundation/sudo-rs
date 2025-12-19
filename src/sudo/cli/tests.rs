use crate::common::SudoPath;

use super::{SudoAction, SudoOptions, SudoRunOptions};

impl SudoAction {
    #[must_use]
    pub fn is_edit(&self) -> bool {
        matches!(self, Self::Edit(..))
    }

    #[must_use]
    pub fn is_help(&self) -> bool {
        matches!(self, Self::Help(..))
    }

    #[must_use]
    pub fn is_remove_timestamp(&self) -> bool {
        matches!(self, Self::RemoveTimestamp(..))
    }

    #[must_use]
    pub fn is_reset_timestamp(&self) -> bool {
        matches!(self, Self::ResetTimestamp(..))
    }

    #[must_use]
    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(..))
    }

    #[must_use]
    pub fn is_version(&self) -> bool {
        matches!(self, Self::Version(..))
    }

    #[must_use]
    pub fn is_validate(&self) -> bool {
        matches!(self, Self::Validate(..))
    }

    #[allow(clippy::result_large_err)]
    pub fn try_into_run(self) -> Result<SudoRunOptions, Self> {
        if let Self::Run(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    #[must_use]
    pub fn is_run(&self) -> bool {
        matches!(self, Self::Run(..))
    }
}

/// Passing '-E' with a variable fails
#[test]
fn short_preserve_env_with_var_fails() {
    let argss = [["sudo", "-E=variable"], ["sudo", "-Evariable"]];

    for args in argss {
        let res = SudoOptions::try_parse_from(args);
        assert!(res.is_err())
    }
}

/// Passing '--preserve-env' with an argument fills 'preserve_env', 'short_preserve_env' stays 'false'
#[test]
fn preserve_env_with_var() {
    let cmd = SudoOptions::try_parse_from(["sudo", "--preserve-env=HOME"]).unwrap();
    assert_eq!(
        [("HOME".to_string(), std::env::var("HOME").unwrap())],
        cmd.env_var_list.as_slice()
    );
}

/// Passing '--preserve-env' with several arguments fills 'preserve_env', 'short_preserve_env' stays 'false'
#[test]
fn preserve_env_with_several_vars() {
    let cmd = SudoOptions::try_parse_from(["sudo", "--preserve-env=PATH,HOME"]).unwrap();
    assert_eq!(
        [
            ("PATH".to_string(), std::env::var("PATH").unwrap()),
            ("HOME".to_string(), std::env::var("HOME").unwrap()),
        ],
        cmd.env_var_list.as_slice()
    );
}

#[test]
fn preserve_env_boolean_and_list() {
    let argss = [
        ["sudo", "--preserve-env", "--preserve-env=HOME"],
        ["sudo", "--preserve-env=HOME", "--preserve-env"],
    ];

    for args in argss {
        let cmd = SudoOptions::try_parse_from(args).unwrap();
        assert_eq!(
            [("HOME".to_string(), std::env::var("HOME").unwrap())],
            cmd.env_var_list.as_slice()
        );
    }
}

#[test]
fn preserve_env_repeated() {
    let cmd = SudoOptions::try_parse_from(["sudo", "--preserve-env=PATH", "--preserve-env=HOME"])
        .unwrap();
    assert_eq!(
        ["PATH", "HOME"],
        cmd.env_var_list
            .into_iter()
            .map(|x| x.0)
            .collect::<Vec<_>>()
            .as_slice()
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
    assert!(cmd.positional_args.is_empty());
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
    assert!(cmd.positional_args.is_empty());
}

/// Mix env variables and trailing arguments that just pass through sudo
/// Divided by hyphens.
#[test]
fn mix_env_variables_with_trailing_args_divided_by_hyphens() {
    let cmd = SudoOptions::try_parse_from(["sudo", "env=var", "--", "external=args", "something"])
        .unwrap();
    assert_eq!(cmd.env_var_list, vec![("env".to_owned(), "var".to_owned())]);
    assert_eq!(cmd.positional_args, vec!["external=args", "something"]);
}

/// Mix env variables and trailing arguments that just pass through sudo
/// Divided by known flag.
#[test]
fn mix_env_variables_with_trailing_args_divided_by_known_flag() {
    let cmd = SudoOptions::try_parse_from(["sudo", "-i", "external=args", "something"]).unwrap();
    assert_eq!(
        cmd.env_var_list,
        vec![("external".to_owned(), "args".to_owned())]
    );
    assert!(cmd.login);
    assert_eq!(cmd.positional_args, vec!["something"]);
}

/// Catch trailing arguments that just pass through sudo
/// but look like a known flag.
#[test]
fn trailing_args_followed_by_known_flag() {
    let cmd =
        SudoOptions::try_parse_from(["sudo", "args", "followed_by", "known_flag", "-i"]).unwrap();
    assert!(!cmd.login);
    assert_eq!(
        cmd.positional_args,
        vec!["args", "followed_by", "known_flag", "-i"]
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
        "-i",
    ])
    .unwrap();
    assert!(!cmd.login);
    assert_eq!(
        cmd.positional_args,
        vec!["trailing", "args", "followed_by", "known_flag", "-i"]
    );
}

/// Check that the first environment variable declaration before any command is not treated as part
/// of the command.
#[test]
fn first_trailing_env_var_is_not_an_external_arg() {
    let cmd = SudoAction::try_parse_from(["sudo", "FOO=1", "command", "BAR=2"]).unwrap();
    let opts = if let SudoAction::Run(opts) = cmd {
        opts
    } else {
        panic!()
    };
    assert_eq!(opts.env_var_list, vec![("FOO".to_owned(), "1".to_owned()),]);
    assert_eq!(opts.positional_args, ["command", "BAR=2"],);
}

#[test]
fn trailing_env_vars_are_external_args() {
    let cmd = SudoOptions::try_parse_from([
        "sudo", "FOO=1", "-i", "BAR=2", "command", "BAZ=3", "arg", "FOOBAR=4", "command", "arg",
        "BARBAZ=5",
    ])
    .unwrap();
    assert!(cmd.login);
    assert_eq!(
        cmd.env_var_list,
        vec![
            ("FOO".to_owned(), "1".to_owned()),
            ("BAR".to_owned(), "2".to_owned())
        ]
    );
    assert_eq!(
        cmd.positional_args,
        ["command", "BAZ=3", "arg", "FOOBAR=4", "command", "arg", "BARBAZ=5"]
    );
}

#[test]
fn single_env_var_declaration() {
    let cmd = SudoOptions::try_parse_from(["sudo", "FOO=1", "command"]).unwrap();
    assert_eq!(cmd.env_var_list, vec![("FOO".to_owned(), "1".to_owned())]);
    assert_eq!(cmd.positional_args, ["command"]);
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
    assert_eq!(cmd.chdir, Some(SudoPath::from("/some/path")));

    let cmd = SudoOptions::try_parse_from(["sudo", "--chdir", "/some/path"]).unwrap();
    assert_eq!(cmd.chdir, Some(SudoPath::from("/some/path")));

    let cmd = SudoOptions::try_parse_from(["sudo", "--chdir=/some/path"]).unwrap();
    assert_eq!(cmd.chdir, Some(SudoPath::from("/some/path")));
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
    let cmd = SudoAction::try_parse_from(["sudo", "-e", "filepath"]).unwrap();
    assert!(cmd.is_edit());

    let cmd = SudoAction::try_parse_from(["sudo", "--edit", "filepath"]).unwrap();
    assert!(cmd.is_edit());

    let cmd = SudoAction::try_parse_from(["sudoedit", "filepath"]).unwrap();
    assert!(cmd.is_edit());

    let res = SudoAction::try_parse_from(["sudo", "--edit"]);
    assert!(res.is_err());

    let res = SudoAction::try_parse_from(["sudoedit", "--edit", "filepath"]);
    assert!(res.is_err());
}

#[test]
fn help() {
    let cmd = SudoAction::try_parse_from(["sudo", "-h"]).unwrap();
    assert!(cmd.is_help());

    let cmd = SudoAction::try_parse_from(["sudo", "-bh"]);
    assert!(cmd.is_err());

    let cmd = SudoAction::try_parse_from(["sudo", "--help"]).unwrap();
    assert!(cmd.is_help());
}

#[test]
fn conflicting_arguments() {
    let cmd = SudoAction::try_parse_from(["sudo", "-K", "-k"]);
    assert!(cmd.is_err());

    let cmd = SudoAction::try_parse_from(["sudo", "--remove-timestamp", "--reset-timestamp"]);
    assert!(cmd.is_err());

    let cmd = SudoAction::try_parse_from(["sudo", "-K"]).unwrap();
    assert!(cmd.is_remove_timestamp());

    let cmd = SudoAction::try_parse_from(["sudo", "-k"]).unwrap();
    assert!(cmd.is_reset_timestamp());
}

#[test]
fn list() {
    let valid: &[&[_]] = &[
        &["sudo", "--list"],
        &["sudo", "-l"],
        &["sudo", "-l", "true"],
        &["sudo", "-l", "-U", "ferris"],
        &["sudo", "-l", "-U", "ferris", "true"],
        &["sudo", "-l", "-u", "ferris", "true"],
        &["sudo", "-l", "-u", "ferris", "-U", "root", "true"],
    ];

    for args in valid {
        let cmd = SudoAction::try_parse_from(args.iter().copied()).unwrap();
        assert!(cmd.is_list());
    }

    let invalid: &[&[_]] = &[
        &["sudo", "-l", "-u", "ferris"],
        &["sudo", "-l", "-u", "ferris", "-U", "root"],
    ];

    for args in invalid {
        let res = SudoAction::try_parse_from(args.iter().copied());
        assert!(res.is_err())
    }
}

#[test]
fn validate() {
    let cmd = SudoAction::try_parse_from(["sudo", "-v"]).unwrap();
    assert!(cmd.is_validate());

    let cmd = SudoAction::try_parse_from(["sudo", "--validate"]).unwrap();
    assert!(cmd.is_validate());
}

#[test]
fn version() {
    let cmd = SudoAction::try_parse_from(["sudo", "-V"]).unwrap();
    assert!(cmd.is_version());

    let cmd = SudoAction::try_parse_from(["sudo", "--version"]).unwrap();
    assert!(cmd.is_version());
}

#[test]
fn run_reset_timestamp_command() {
    let action = SudoAction::try_parse_from(["sudo", "-k", "true"])
        .unwrap()
        .try_into_run()
        .ok()
        .unwrap();
    assert_eq!(["true"], action.positional_args.as_slice());
    assert!(action.reset_timestamp);
}

#[test]
fn run_reset_timestamp_login() {
    let action = SudoAction::try_parse_from(["sudo", "-k", "-i"])
        .unwrap()
        .try_into_run()
        .ok()
        .unwrap();
    assert!(action.positional_args.is_empty());
    assert!(action.reset_timestamp);
    assert!(action.login);
}

#[test]
fn run_reset_timestamp_shell() {
    let action = SudoAction::try_parse_from(["sudo", "-k", "-s"])
        .unwrap()
        .try_into_run()
        .ok()
        .unwrap();
    assert!(action.positional_args.is_empty());
    assert!(action.reset_timestamp);
    assert!(action.shell);
}

#[test]
fn run_no_command() {
    assert!(SudoAction::try_parse_from(["sudo", "-u", "root"]).is_err());
}

#[test]
fn run_login() {
    assert!(SudoAction::try_parse_from(["sudo", "-i"]).unwrap().is_run());
}

#[test]
fn run_shell() {
    assert!(SudoAction::try_parse_from(["sudo", "-s"]).unwrap().is_run());
}
