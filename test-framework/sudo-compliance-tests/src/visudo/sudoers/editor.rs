use sudo_test::{Command, Env, TextFile};

use crate::{visudo::CHMOD_EXEC, Result};

use crate::visudo::{DEFAULT_EDITOR, LOGS_PATH};

#[test]
#[ignore = "gh657"]
fn it_works() -> Result<()> {
    let expected = "configured editor was called";
    let editor_path = "/usr/bin/my-editor";
    let env = Env(format!("Defaults editor={editor_path}"))
        .file(
            editor_path,
            TextFile(format!(
                "#!/bin/sh
echo '{expected}' >> {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn fallback() -> Result<()> {
    let expected = "configured editor was called";
    let editor_path = "/usr/bin/my-editor";

    let bad_editors = ["/does/not/exist", "/tmp", "/dev/null"];

    for bad_editor in bad_editors {
        let env = Env(format!("Defaults editor={bad_editor}:{editor_path}"))
            .file(
                editor_path,
                TextFile(format!(
                    "#!/bin/sh
echo '{expected}' >> {LOGS_PATH}"
                ))
                .chmod(CHMOD_EXEC),
            )
            .build()?;

        Command::new("rm")
            .args(["-f", LOGS_PATH])
            .output(&env)?
            .assert_success()?;

        Command::new("visudo").output(&env)?.assert_success()?;
        let actual = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

        assert_eq!(expected, actual);
    }

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn no_valid_editor_in_list() -> Result<()> {
    let env = Env("Defaults editor=/dev/null").build()?;

    let output = Command::new("visudo").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_eq!(
        "visudo: no editor found (editor path = /dev/null)",
        output.stderr()
    );

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn editors_must_be_specified_by_absolute_path() -> Result<()> {
    let env = Env("Defaults editor=true").build()?;

    let output = Command::new("visudo").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(
        output.stderr(),
        "values for \"editor\" must start with a '/'"
    );

    Ok(())
}

#[test]
fn on_invalid_editor_does_not_falls_back_to_configured_default_value() -> Result<()> {
    let env = Env("Defaults editor=true")
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
rm -f {LOGS_PATH}",
            )
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("touch")
        .arg(LOGS_PATH)
        .output(&env)?
        .assert_success()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!("test -f {LOGS_PATH}"))
        .output(&env)?
        .assert_success()?;

    Ok(())
}
