use sudo_test::{Command, Env, TextFile};

use crate::{
    visudo::{CHMOD_EXEC, DEFAULT_EDITOR, EDITOR_TRUE, LOGS_PATH},
    Result,
};

#[test]
fn when_disabled_env_vars_are_ignored() -> Result<()> {
    let var_names = ["SUDO_EDITOR", "VISUAL", "EDITOR"];

    let editor_path = "/tmp/editor";
    let env = Env("Defaults !env_editor")
        .file(
            editor_path,
            TextFile(format!(
                "#!/bin/sh
rm -f {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    for var_name in var_names {
        Command::new("touch")
            .arg(LOGS_PATH)
            .output(&env)?
            .assert_success()?;

        Command::new("env")
            .arg(format!("{var_name}={editor_path}"))
            .arg("visudo")
            .output(&env)?
            .assert_success()?;

        Command::new("sh")
            .arg("-c")
            .arg(format!("test -f {LOGS_PATH}"))
            .output(&env)?
            .assert_success()?;
    }

    Ok(())
}

struct Fixture {
    env: Env,
    bad_editor_path: &'static str,
    good_editor_path: &'static str,
    expected: &'static str,
}

impl Fixture {
    fn new() -> Result<Self> {
        let expected = "good-editor was called";
        let unexpected = "bad-editor was called";
        let good_editor_path = "/tmp/good-editor";
        let bad_editor_path = "/tmp/bad-editor";
        let env = Env("")
            .file(
                good_editor_path,
                TextFile(format!(
                    "#!/bin/sh
echo {expected} >> {LOGS_PATH}"
                ))
                .chmod(CHMOD_EXEC),
            )
            .file(
                bad_editor_path,
                TextFile(format!(
                    "#!/bin/sh
echo {unexpected} >> {LOGS_PATH}"
                ))
                .chmod(CHMOD_EXEC),
            )
            .file(DEFAULT_EDITOR, EDITOR_TRUE)
            .build()?;

        Ok(Fixture {
            env,
            bad_editor_path,
            good_editor_path,
            expected,
        })
    }
}

#[test]
#[ignore = "gh657"]
fn uses_editor() -> Result<()> {
    let Fixture {
        env,
        good_editor_path,
        bad_editor_path: _,
        expected,
    } = Fixture::new()?;

    Command::new("env")
        .arg(format!("EDITOR={good_editor_path}"))
        .arg("visudo")
        .output(&env)?
        .assert_success()?;

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn visual_has_precedence_over_editor() -> Result<()> {
    let Fixture {
        env,
        good_editor_path,
        bad_editor_path,
        expected,
    } = Fixture::new()?;

    Command::new("env")
        .arg(format!("VISUAL={good_editor_path}"))
        .arg(format!("EDITOR={bad_editor_path}"))
        .arg("visudo")
        .output(&env)?
        .assert_success()?;

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn sudo_editor_has_precedence_over_visual() -> Result<()> {
    let Fixture {
        env,
        good_editor_path,
        bad_editor_path,
        expected,
    } = Fixture::new()?;

    Command::new("env")
        .arg(format!("SUDO_EDITOR={good_editor_path}"))
        .arg(format!("VISUAL={bad_editor_path}"))
        .arg("visudo")
        .output(&env)?
        .assert_success()?;

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn falls_back_to_editor_list_when_env_editor_is_not_executable() -> Result<()> {
    let var_names = ["SUDO_EDITOR", "VISUAL", "EDITOR"];

    let expected = "default editor was called";
    let editor_path = "/tmp/editor";
    let env = Env("")
        .file(editor_path, EDITOR_TRUE)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh
echo '{expected}' > {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    for var_name in var_names {
        Command::new("rm")
            .args(["-f", LOGS_PATH])
            .output(&env)?
            .assert_success()?;

        let output = Command::new("env")
            .arg(format!("{var_name}={editor_path}"))
            .arg("visudo")
            .output(&env)?;

        output.assert_success()?;

        let actual = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

        assert_eq!(expected, actual);
    }

    Ok(())
}
