use sudo_test::{Command, Env, TextFile};

use crate::{
    visudo::{CHMOD_EXEC, DEFAULT_EDITOR, EDITOR_TRUE, ETC_SUDOERS, TMP_SUDOERS},
    Result, USERNAME,
};

#[test]
#[ignore = "gh657"]
fn when_present_changes_ownership_of_existing_file() -> Result<()> {
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, TextFile("").chown("root:users").chmod("777"))
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    Command::new("visudo")
        .args(["--owner", "--file", file_path])
        .output(&env)?
        .assert_success()?;

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)?
        .stdout()?;

    assert_contains!(ls_output, " root root ");

    Ok(())
}

#[test]
fn when_absent_ownership_is_preserved() -> Result<()> {
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, TextFile("").chown("root:users").chmod("777"))
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    Command::new("visudo")
        .args(["--file", file_path])
        .output(&env)?
        .assert_success()?;

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)?
        .stdout()?;

    assert_contains!(ls_output, " root users ");

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn etc_sudoers_ownership_is_always_changed() -> Result<()> {
    let file_path = ETC_SUDOERS;
    let env = Env(TextFile("").chown(format!("{USERNAME}:users")).chmod("777"))
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .user(USERNAME)
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)?
        .stdout()?;

    assert_contains!(ls_output, " root root ");

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn flag_check() -> Result<()> {
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(
            file_path,
            TextFile("").chown(format!("{USERNAME}:users")).chmod("777"),
        )
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .user(USERNAME)
        .build()?;

    let output = Command::new("visudo")
        .args(["--check", "--owner", "--file", file_path])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_eq!(
        format!("{file_path}: wrong owner (uid, gid) should be (0, 0)"),
        output.stderr(),
    );

    Ok(())
}
