use sudo_test::{Command, Env, TextFile};

use crate::visudo::CHMOD_EXEC;

use crate::visudo::{DEFAULT_EDITOR, LOGS_PATH};

#[test]
fn it_works() {
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
        .build();

    Command::new("visudo").output(&env).assert_success();

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn fallback() {
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
            .build();

        Command::new("rm")
            .args(["-f", LOGS_PATH])
            .output(&env)
            .assert_success();

        Command::new("visudo").output(&env).assert_success();
        let actual = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

        assert_eq!(expected, actual);
    }
}

#[test]
fn no_valid_editor_in_list() {
    let env = Env("Defaults editor=/dev/null").build();

    let output = Command::new("visudo").output(&env);

    output.assert_exit_code(1);
    if sudo_test::is_original_sudo() {
        assert_eq!(
            "visudo: no editor found (editor path = /dev/null)",
            output.stderr()
        );
    } else {
        // this output shows that visudo has rejected /dev/null as an editor
        assert_eq!("visudo: no usable editor could be found", output.stderr());
    }
}

#[test]
fn editors_must_be_specified_by_absolute_path() {
    let env = Env("Defaults editor=true").build();

    let output = Command::new("visudo").output(&env);

    output.assert_exit_code(1);
    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "values for \"editor\" must start with a '/'"
        );
    } else {
        // this output shows that visudo has rejected /dev/null as an editor
        assert_eq!("visudo: no usable editor could be found", output.stderr());
    }
}

#[test]
fn on_invalid_editor_does_not_falls_back_to_configured_default_value() {
    let env = Env("Defaults editor=true")
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
rm -f {LOGS_PATH}",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("touch")
        .arg(LOGS_PATH)
        .output(&env)
        .assert_success();

    Command::new("visudo").output(&env).assert_success();

    Command::new("sh")
        .arg("-c")
        .arg(format!("test -f {LOGS_PATH}"))
        .output(&env)
        .assert_success();
}
