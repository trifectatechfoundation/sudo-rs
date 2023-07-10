use sudo_test::{Command, Env, TextFile};

use crate::{
    visudo::{CHMOD_EXEC, DEFAULT_EDITOR, EDITOR_TRUE, ETC_SUDOERS, LOGS_PATH, TMP_SUDOERS},
    Result, SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_ROOT_ALL, USERNAME,
};

#[test]
fn creates_sudoers_file_with_default_ownership_and_perms_if_it_doesnt_exist() -> Result<()> {
    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    let file_path = TMP_SUDOERS;
    Command::new("visudo")
        .args(["-f", file_path])
        .output(&env)?
        .assert_success()?;

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)?
        .stdout()?;

    assert!(ls_output.starts_with("-rw-r----- 1 root root"));

    Ok(())
}

#[test]
fn saves_file_if_no_syntax_errors() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let unexpected = SUDOERS_ROOT_ALL;
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, unexpected)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' > $2"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("visudo")
        .args(["-f", file_path])
        .output(&env)?
        .assert_success()?;

    let actual = Command::new("cat").arg(file_path).output(&env)?.stdout()?;
    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn positional_argument() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let unexpected = SUDOERS_ROOT_ALL;
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, unexpected)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' > $2"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("visudo")
        .arg(file_path)
        .output(&env)?
        .assert_success()?;

    let actual = Command::new("cat").arg(file_path).output(&env)?.stdout()?;
    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn flag_has_precedence_over_positional_argument() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let original = SUDOERS_ROOT_ALL;
    let file_path = "/tmp/sudoers";
    let file_path2 = "/tmp/sudoers2";
    let env = Env("")
        .file(file_path, original)
        .file(file_path2, original)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' > $2"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("visudo")
        .args(["-f", file_path])
        .arg(file_path2)
        .output(&env)?
        .assert_success()?;

    let changed = Command::new("cat").arg(file_path).output(&env)?.stdout()?;
    assert_eq!(expected, changed);

    let unchanged = Command::new("cat").arg(file_path2).output(&env)?.stdout()?;
    assert_eq!(original, unchanged);

    Ok(())
}

#[test]
fn etc_sudoers_is_not_modified() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let unexpected = SUDOERS_ROOT_ALL;
    let env = Env(expected)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh
echo '{unexpected}' > $2"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("visudo")
        .args(["--file", TMP_SUDOERS])
        .output(&env)?
        .assert_success()?;

    let actual = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn passes_temporary_file_to_editor() -> Result<()> {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo "$@" > {LOGS_PATH}"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    let file_path = TMP_SUDOERS;
    Command::new("visudo")
        .args(["--file", file_path])
        .output(&env)?
        .assert_success()?;

    let args = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert_eq!(format!("-- {file_path}.tmp"), args);

    Ok(())
}

#[test]
fn regular_user_can_create_file() -> Result<()> {
    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod("111"))
        .user(USERNAME)
        .build()?;

    let file_path = TMP_SUDOERS;
    Command::new("visudo")
        .args(["-f", file_path])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)?
        .stdout()?;

    assert!(ls_output.starts_with(&format!("-rw-r----- 1 {USERNAME} users")));

    Ok(())
}

#[test]
fn regular_user_can_update_a_file_they_own() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let unexpected = SUDOERS_ROOT_ALL;
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, TextFile(unexpected).chown(USERNAME).chmod("666"))
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' > $2"#
            ))
            .chmod("777"),
        )
        .user(USERNAME)
        .build()?;

    Command::new("visudo")
        .args(["-f", file_path])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    let sudoers = Command::new("cat").arg(file_path).output(&env)?.stdout()?;

    assert_eq!(expected, sudoers);

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn regular_user_cannot_update_a_file_they_dont_own() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let unexpected = SUDOERS_ROOT_ALL;
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, TextFile(unexpected).chmod("666"))
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' > $2"#
            ))
            .chmod("777"),
        )
        .user(USERNAME)
        .build()?;

    let output = Command::new("visudo")
        .args(["-f", file_path])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(
        output.stderr(),
        "visudo: unable to set (uid, gid) of /tmp/sudoers.tmp"
    );

    Ok(())
}
