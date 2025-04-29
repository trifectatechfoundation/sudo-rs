use pretty_assertions::assert_eq;
use sudo_test::{Command, Env};

use crate::SUDOERS_ROOT_ALL_NOPASSWD;

mod signal_handling;

#[test]
fn sudo_forwards_childs_exit_code() {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build();

    let expected = 42;
    let output = Command::new("sudo")
        .args(["sh", "-c"])
        .arg(format!("exit {expected}"))
        .output(&env);
    output.assert_exit_code(expected);
}

#[test]
fn sudo_forwards_childs_stdout() {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build();

    let expected = "hello";
    let output = Command::new("sudo").args(["echo", expected]).output(&env);
    assert!(output.stderr().is_empty());
    assert_eq!(expected, output.stdout());
}

#[test]
fn sudo_forwards_childs_stderr() {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build();

    let expected = "hello";
    let output = Command::new("sudo")
        .args(["sh", "-c"])
        .arg(format!(">&2 echo {expected}"))
        .output(&env);
    assert_eq!(expected, output.stderr());
    assert!(output.stdout().is_empty());
}

#[test]
fn sudo_forwards_stdin_to_child() {
    let expected = "hello";
    let path = "/root/file";
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build();

    Command::new("sudo")
        .args(["tee", path])
        .stdin(expected)
        .output(&env)
        .assert_success();

    let actual = Command::new("cat").arg(path).output(&env).stdout();

    assert_eq!(expected, actual);
}
