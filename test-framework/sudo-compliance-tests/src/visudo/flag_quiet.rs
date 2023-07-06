use sudo_test::{Command, Env, TextFile};

use crate::Result;

use super::{CHMOD_EXEC, DEFAULT_EDITOR, EDITOR_TRUE};

#[test]
#[ignore = "gh657"]
fn supresses_syntax_error_messages() -> Result<()> {
    let env = Env("this is fine")
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    let output = Command::new("visudo").arg("-q").output(&env)?;

    assert!(output.status().success());
    assert_not_contains!(output.stderr(), "syntax error");

    Ok(())
}
