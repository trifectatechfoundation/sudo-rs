use sudo_test::{Command, Env, TextFile};

use crate::{
    visudo::{CHMOD_EXEC, DEFAULT_EDITOR, EDITOR_TRUE},
    Result,
};

#[test]
#[ignore = "gh657"]
fn undefined_alias() -> Result<()> {
    let env = Env(["# User_Alias ADMINS = root", "ADMINS ALL=(ALL:ALL) ALL"])
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    let output = Command::new("visudo").arg("--strict").output(&env)?;

    let diagnostic = r#"User_Alias "ADMINS" referenced but not defined"#;
    let prompt = "What now?";

    assert!(output.status().success());
    assert_contains!(output.stderr(), diagnostic);
    // we only get this prompt in `--strict` mode
    assert_contains!(output.stdout()?, prompt);

    let output = Command::new("visudo").output(&env)?;

    assert!(output.status().success());
    assert_contains!(output.stderr(), diagnostic);
    assert_not_contains!(output.stdout()?, prompt);

    Ok(())
}

#[test]
fn alias_cycle() -> Result<()> {
    let env = Env(["User_Alias FOO = FOO", "FOO ALL=(ALL:ALL) ALL"])
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    let output = Command::new("visudo").arg("--strict").output(&env)?;

    let diagnostic = if sudo_test::is_original_sudo() {
        r#"cycle in User_Alias "FOO""#
    } else {
        "syntax error: recursive alias: 'FOO'"
    };
    let prompt = "What now?";

    assert!(output.status().success());
    assert_contains!(output.stderr(), diagnostic);
    // we only get this prompt in `--strict` mode
    assert_contains!(output.stdout()?, prompt);

    let output = Command::new("visudo").output(&env)?;

    assert!(output.status().success());
    assert_contains!(output.stderr(), diagnostic);
    if sudo_test::is_original_sudo() {
        assert_not_contains!(output.stdout()?, prompt);
    } else {
        // visudo-rs is always strict
        assert_contains!(output.stdout()?, prompt);
    }

    Ok(())
}
