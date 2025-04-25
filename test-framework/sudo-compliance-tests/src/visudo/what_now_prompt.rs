use sudo_test::{is_original_sudo, Command, Env, EnvNoImplicit, TextFile};

use crate::{
    visudo::{CHMOD_EXEC, DEFAULT_EDITOR, ETC_SUDOERS, LOGS_PATH},
    SUDOERS_ALL_ALL_NOPASSWD,
};

const BAD_SUDOERS: &str = "this is fine";

fn editor() -> String {
    format!(
        r#"#!/bin/sh
echo "$@" >> {LOGS_PATH}
echo '{BAD_SUDOERS}' > $2"#
    )
}

#[test]
fn prompt_is_printed_to_stdout() {
    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(editor()).chmod(CHMOD_EXEC))
        .build();

    let output = Command::new("visudo").output(&env);

    assert!(output.status().success());
    assert!(output.stdout_unchecked().starts_with("What now?"));
}

#[test]
fn on_e_re_edits() {
    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(editor()).chmod(CHMOD_EXEC))
        .build();

    Command::new("visudo")
        .stdin("e")
        .output(&env)
        .assert_success();

    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let lines = logs.lines().collect::<Vec<_>>();

    let num_times_called = lines.len();
    assert_eq!(2, num_times_called);
    if is_original_sudo() && cfg!(target_os = "freebsd") {
        // On FreeBSD we have to name our editor vi, which seems to trigger a special case in sudo.
        assert_eq!(lines[0], "-- /usr/local/etc/sudoers.tmp");
        assert_eq!(lines[1], "+1 -- /usr/local/etc/sudoers.tmp");
    } else {
        assert_eq!(lines[0], lines[1]);
    }
}

#[test]
fn on_x_closes_without_saving_changes() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = EnvNoImplicit(expected)
        .file(DEFAULT_EDITOR, TextFile(editor()).chmod(CHMOD_EXEC))
        .build();

    Command::new("visudo")
        .stdin("x")
        .output(&env)
        .assert_success();

    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let lines = logs.lines().collect::<Vec<_>>();

    let num_times_called = lines.len();
    assert_eq!(1, num_times_called);

    let actual = Command::new("cat").arg(ETC_SUDOERS).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
#[ignore = "gh657"]
fn on_uppercase_q_closes_while_saving_changes() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .file(DEFAULT_EDITOR, TextFile(editor()).chmod(CHMOD_EXEC))
        .build();

    Command::new("visudo")
        .stdin("Q")
        .output(&env)
        .assert_success();

    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let lines = logs.lines().collect::<Vec<_>>();

    let num_times_called = lines.len();
    assert_eq!(1, num_times_called);

    let actual = Command::new("cat").arg(ETC_SUDOERS).output(&env).stdout();

    assert_eq!(BAD_SUDOERS, actual);
}

#[test]
#[ignore = "gh657"]
fn on_invalid_option_prompts_again() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .file(DEFAULT_EDITOR, TextFile(editor()).chmod(CHMOD_EXEC))
        .build();

    let cases = ["?", "abc", "\n", "\r\n", "a\nb", "\n\r", "a\rb"];
    for input in cases {
        dbg!(input);

        let output = Command::new("visudo").stdin(input).output(&env);

        let num_prompts = output
            .stdout()
            .lines()
            .filter(|line| line.starts_with("What now?"))
            .count();

        assert!(num_prompts >= 2);
    }
}
