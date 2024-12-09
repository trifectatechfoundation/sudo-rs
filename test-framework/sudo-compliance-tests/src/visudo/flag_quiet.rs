use sudo_test::{Command, TextFile};

use crate::visudo::visudo_env;
use crate::Result;

use super::{CHMOD_EXEC, EDITOR_DUMMY};

#[test]
#[ignore = "gh657"]
fn supresses_syntax_error_messages() -> Result<()> {
    let env = visudo_env("this is fine", TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC)).build()?;

    let output = Command::new("visudo").arg("-q").output(&env)?;

    assert!(output.status().success());
    assert_not_contains!(output.stderr(), "syntax error");

    Ok(())
}
