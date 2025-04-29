use sudo_test::{Command, Env, TextFile};

use crate::{
    visudo::{CHMOD_EXEC, DEFAULT_EDITOR, EDITOR_DUMMY, ETC_SUDOERS, TMP_SUDOERS},
    USERNAME,
};

#[test]
fn when_present_changes_perms_of_existing_file() {
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, TextFile("").chmod("777"))
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    Command::new("visudo")
        .args(["--perms", "--file", file_path])
        .output(&env)
        .assert_success();

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)
        .stdout();

    assert!(ls_output.starts_with("-r--r----- "));
}

#[test]
fn when_absent_perms_are_preserved() {
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, TextFile("").chmod("777"))
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    Command::new("visudo")
        .args(["--file", file_path])
        .output(&env)
        .assert_success();

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)
        .stdout();

    assert!(ls_output.starts_with("-rwxrwxrwx "));
}

#[test]
fn etc_sudoers_perms_are_always_changed() {
    let file_path = ETC_SUDOERS;
    let env = Env(TextFile("").chmod("777"))
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    Command::new("visudo").output(&env).assert_success();

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)
        .stdout();

    assert!(ls_output.starts_with("-r--r----- "));
}

#[test]
fn flag_check() {
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(
            file_path,
            TextFile("").chown(format!("{USERNAME}:users")).chmod("777"),
        )
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .user(USERNAME)
        .build();

    let output = Command::new("visudo")
        .args(["--check", "--perms", "--file", file_path])
        .output(&env);

    output.assert_exit_code(1);
    assert_contains!(
        output.stderr(),
        format!("{file_path}: bad permissions, should be mode 0440")
    );
}
