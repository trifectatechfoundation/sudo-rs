use sudo_test::{Command, Directory, Env, TextFile, ROOT_GROUP};

use crate::{DEFAULT_EDITOR, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

const CHMOD_EXEC: &str = "555";
const EDITOR_DUMMY: &str = "#!/bin/sh
echo \"#\" >> \"$1\"";

#[test]
fn cannot_edit_writable_paths() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .directory(Directory("/tmp/bar").chmod("755"))
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    for file in ["/tmp/foo.sh", "/tmp/bar/foo.sh", "/var/tmp/foo.sh"] {
        let output = Command::new("sudoedit")
            .as_user(USERNAME)
            .arg(file)
            .output(&env);

        if sudo_test::is_original_sudo() {
            if file != "/tmp/bar/foo.sh" {
                assert_contains!(
                    output.stderr(),
                    "editing files in a writable directory is not permitted"
                );
            } else {
                // I don't know why ogsudo gives this error -- probably because opening failed
                assert_contains!(output.stderr(), "No such file or directory");
            }
        } else {
            assert_contains!(
                output.stderr(),
                "cannot open a file in a path writable by the user"
            );
        }
        output.assert_exit_code(1);
    }
}

#[test]
fn cannot_edit_symlinks() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    let file = "/usr/bin/sudoedit";

    let output = Command::new("sudoedit")
        .as_user(USERNAME)
        .arg(file)
        .output(&env);

    assert_contains!(output.stderr(), "editing symbolic links is not permitted");

    output.assert_exit_code(1);
}
