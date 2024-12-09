use sudo_test::{Command, TextFile, ETC_DIR};

use crate::visudo::visudo_env;
use crate::{
    visudo::{CHMOD_EXEC, LOGS_PATH},
    Result,
};

#[test]
fn does_not_edit_at_include_files_that_dont_contain_syntax_errors() -> Result<()> {
    let env = visudo_env(
        "# 1
@include sudoers2",
        TextFile(format!(
            "#!/bin/sh
cat $2 >> {LOGS_PATH}"
        ))
        .chmod(CHMOD_EXEC),
    )
    .file(format!("{ETC_DIR}/sudoers2"), "# 2")
    .build()?;

    Command::new("visudo")
        .arg("--no-includes")
        .output(&env)?
        .assert_success()?;
    let logs = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    let comments = logs
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect::<Vec<_>>();

    assert_eq!(["# 1"], &*comments);

    Ok(())
}

#[test]
fn does_edit_at_include_files_that_contain_syntax_errors() -> Result<()> {
    let env = visudo_env(
        "# 1
@include sudoers2",
        TextFile(format!(
            "#!/bin/sh
cat $2 >> {LOGS_PATH}"
        ))
        .chmod(CHMOD_EXEC),
    )
    .file(
        format!("{ETC_DIR}/sudoers2"),
        "# 2
this is fine",
    )
    .build()?;

    Command::new("visudo")
        .arg("--no-includes")
        .output(&env)?
        .assert_success()?;
    let logs = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    let comments = logs
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect::<Vec<_>>();

    assert_eq!(["# 1"], &*comments);

    Ok(())
}

#[test]
fn does_not_edit_deep_at_include_files_that_contain_syntax_errors() -> Result<()> {
    let env = visudo_env(
        "# 1
@include sudoers2",
        TextFile(format!(
            "#!/bin/sh
cat $2 >> {LOGS_PATH}"
        ))
        .chmod(CHMOD_EXEC),
    )
    .file(
        format!("{ETC_DIR}/sudoers2"),
        "# 2
@include sudoers3",
    )
    .file(
        format!("{ETC_DIR}/sudoers3"),
        "# 3
this is fine",
    )
    .build()?;

    Command::new("visudo")
        .arg("--no-includes")
        .output(&env)?
        .assert_success()?;
    let logs = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    let comments = logs
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect::<Vec<_>>();

    assert_eq!(["# 1"], &*comments);

    Ok(())
}
