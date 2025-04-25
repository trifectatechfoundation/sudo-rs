use sudo_test::{Command, Env, TextFile};

#[test]
fn it_works() {
    let env = Env("").build();

    Command::new("su")
        .args(["-c", "true"])
        .output(&env)
        .assert_success();

    let output = Command::new("su").args(["-c", "false"]).output(&env);

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
}

#[test]
fn pass_to_shell_via_c_flag() {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env("")
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build();

    let command = "command";
    let output = Command::new("su")
        .args(["-s", shell_path, "-c", command])
        .output(&env)
        .stdout();

    assert_eq!(format!("-c {command}"), output);
}

#[test]
fn when_specified_more_than_once_only_last_value_is_used() {
    let env = Env("").build();

    let output = Command::new("su")
        .args(["-c", "id"])
        .args(["-c", "true"])
        .output(&env);

    assert!(output.status().success());
    assert!(output.stderr().is_empty());
    assert!(output.stdout().is_empty());
}

#[test]
fn positional_arguments_are_not_passed_to_command() {
    let env = Env("").build();

    let argss = [["-c", "echo", "root", "a"], ["root", "-c", "echo", "a"]];

    for args in argss {
        let output = Command::new("su").args(args).output(&env);
        let stdout = output.stdout();

        assert!(stdout.trim().is_empty());
    }
}
