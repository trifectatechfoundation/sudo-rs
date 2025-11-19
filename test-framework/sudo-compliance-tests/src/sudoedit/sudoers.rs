use sudo_test::{Command, Env, TextFile, User};

use crate::{DEFAULT_EDITOR, OTHER_USERNAME, USERNAME};

const CHMOD_EXEC: &str = "555";
const EDITOR_DUMMY: &str = "#!/bin/sh
echo \"#\" >> \"$1\"";

#[test]
fn cannot_edit_without_permission() {
    for sudoers in [
        "ALL ALL=(ALL:ALL) NOPASSWD: /bin/sh",
        "ALL ALL=(ALL:ALL) NOPASSWD: sudoedit /bar.txt",
    ] {
        let env = Env(sudoers)
            .user(USERNAME)
            .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
            .build();

        let file = "/foo.txt";

        let output = Command::new("sudoedit")
            .as_user(USERNAME)
            .arg(file)
            .output(&env);

        output.assert_exit_code(1);
        if sudo_test::is_original_sudo() {
            assert_contains!(output.stderr(), "a password is required");
        } else {
            assert_contains!(
                output.stderr(),
                "I'm sorry ferris. I'm afraid I can't do that"
            );
        }
    }
}

#[test]
fn can_edit_with_explicit_permission() {
    for sudoers in [
        "ALL ALL=NOPASSWD: ALL",
        "ALL ALL=NOPASSWD: sudoedit",
        "ALL ALL=NOPASSWD: sudoedit /foo.txt, sudoedit /etc/bar.txt",
    ] {
        let env = Env(sudoers)
            .user(USERNAME)
            .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
            .build();

        for file in ["/foo.txt", "/etc/bar.txt"] {
            Command::new("sudoedit")
                .as_user(USERNAME)
                .arg(file)
                .output(&env)
                .assert_success();
        }
    }
}

#[test]
fn respects_runas_user() {
    let file = "/foo.txt";
    let env = Env(format!("ALL ALL=({OTHER_USERNAME}) NOPASSWD: sudoedit"))
        .user(USERNAME)
        .user(OTHER_USERNAME)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .file(file, TextFile("").chmod("600").chown(OTHER_USERNAME))
        .build();

    Command::new("sudoedit")
        .as_user(USERNAME)
        .args(["-u", OTHER_USERNAME, file])
        .output(&env)
        .assert_success();

    let output = Command::new("sudoedit")
        .as_user(USERNAME)
        .args(["-u", USERNAME, file])
        .output(&env);

    output.assert_exit_code(1);
    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "user ferris is not allowed to execute 'sudoedit /foo.txt' as ferris"
        );
    } else {
        assert_contains!(
            output.stderr(),
            "I'm sorry ferris. I'm afraid I can't do that"
        );
    }
}

#[test]
fn respects_runas_group() {
    let file = "/foo.txt";
    let env = Env(format!(
        "ALL ALL=({OTHER_USERNAME}:{OTHER_USERNAME}) NOPASSWD: sudoedit"
    ))
    .user(USERNAME)
    .group(OTHER_USERNAME)
    .user(User(OTHER_USERNAME).secondary_group(OTHER_USERNAME))
    .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
    .file(
        file,
        TextFile("")
            .chmod("660")
            .chown(format!("{OTHER_USERNAME}:{OTHER_USERNAME}")),
    )
    .build();

    if !(sudo_test::ogsudo("1.9.13p3")..=sudo_test::ogsudo("1.9.17p2"))
        .contains(&sudo_test::sudo_version())
    {
        // FIXME: sudo 1.9.16p2 has a different interaction with a bare "-g" and the runas specifier
        Command::new("sudoedit")
            .as_user(USERNAME)
            .args(["-g", OTHER_USERNAME, file])
            .output(&env)
            .assert_success();
    }

    let output = Command::new("sudoedit")
        .as_user(USERNAME)
        .args(["-u", OTHER_USERNAME, "-g", "root", file])
        .output(&env);

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "a password is required");
    } else {
        assert_contains!(
            output.stderr(),
            "I'm sorry ferris. I'm afraid I can't do that"
        );
    }
}

#[test]
fn user_host_must_match() {
    let env = Env(format!("{USERNAME} container=NOPASSWD: sudoedit"))
        .hostname("container")
        .user(USERNAME)
        .user(OTHER_USERNAME)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    let other_env = Env(format!("{USERNAME} container=NOPASSWD: sudoedit"))
        .hostname("notcontainer")
        .user(USERNAME)
        .user(OTHER_USERNAME)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    let file = "/foo.txt";

    Command::new("sudoedit")
        .as_user(USERNAME)
        .arg(file)
        .output(&env)
        .assert_success();

    let output = Command::new("sudoedit")
        .as_user(USERNAME)
        .arg(file)
        .output(&other_env);

    output.assert_exit_code(1);
    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "a password is required");
    } else {
        assert_contains!(
            output.stderr(),
            format!("I'm sorry {USERNAME}. I'm afraid I can't do that")
        );
    }

    for env in [env, other_env] {
        let output = Command::new("sudoedit")
            .as_user(OTHER_USERNAME)
            .arg(file)
            .output(&env);

        output.assert_exit_code(1);
        if sudo_test::is_original_sudo() {
            assert_contains!(output.stderr(), "a password is required");
        } else {
            assert_contains!(
                output.stderr(),
                format!("I'm sorry {OTHER_USERNAME}. I'm afraid I can't do that")
            );
        }
    }
}

#[test]
fn passwd() {
    let env = Env("ALL ALL=NOPASSWD: sudoedit /unprot.txt, PASSWD: sudoedit /prot.txt")
        .user(USERNAME)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    Command::new("sudoedit")
        .as_user(USERNAME)
        .arg("/unprot.txt")
        .output(&env)
        .assert_success();

    let output = Command::new("sudoedit")
        .as_user(USERNAME)
        .arg("/prot.txt")
        .output(&env);

    output.assert_exit_code(1);
    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "a password is required");
    } else {
        assert_contains!(
            output.stderr(),
            "A terminal is required to read the password"
        );
    }
}
