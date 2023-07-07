use sudo_test::{Command, Env, TextFile};

use crate::{
    visudo::{CHMOD_EXEC, DEFAULT_EDITOR, EDITOR_TRUE, ETC_SUDOERS, TMP_SUDOERS},
    Result, USERNAME,
};

#[test]
#[ignore = "gh657"]
fn when_present_changes_perms_of_existing_file() -> Result<()> {
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, TextFile("").chmod("777"))
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    Command::new("visudo")
        .args(["--perms", "--file", file_path])
        .output(&env)?
        .assert_success()?;

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)?
        .stdout()?;

    assert!(ls_output.starts_with("-r--r----- "));

    Ok(())
}

#[test]
fn when_absent_perms_are_preserved() -> Result<()> {
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, TextFile("").chmod("777"))
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

    assert!(ls_output.starts_with("-rwxrwxrwx "));

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn etc_sudoers_perms_are_always_changed() -> Result<()> {
    let file_path = ETC_SUDOERS;
    let env = Env(TextFile("").chmod("777"))
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)?
        .stdout()?;

    assert!(ls_output.starts_with("-r--r----- "));

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
        .args(["--check", "--perms", "--file", file_path])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_eq!(
        format!("{file_path}: bad permissions, should be mode 0440"),
        output.stderr(),
    );

    Ok(())
}
