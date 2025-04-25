use sudo_test::{Command, Env, TextFile};

use super::{CHMOD_EXEC, DEFAULT_EDITOR, EDITOR_DUMMY};

#[test]
#[ignore = "gh657"]
fn supresses_syntax_error_messages() {
    let env = Env("this is fine")
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    let output = Command::new("visudo").arg("-q").output(&env);

    output.assert_success();
    assert_not_contains!(output.stderr(), "syntax error");
}
