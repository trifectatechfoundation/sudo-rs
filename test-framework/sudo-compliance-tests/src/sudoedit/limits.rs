use sudo_test::{Command, Directory, Env, TextFile, ROOT_GROUP};

use crate::{DEFAULT_EDITOR, OTHER_USERNAME, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

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
fn can_edit_writable_paths_as_root() {
    // note: we already have tests that sudoedit "works" so we are skipping
    // the content check here---the point here is that sudoedit does not stop
    // the user.

    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .directory(Directory("/tmp/bar").chmod("755"))
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    let file = "/tmp/foo.sh";
    Command::new("sudoedit")
        .arg(file)
        .output(&env)
        .assert_success();
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

#[test]
fn cannot_edit_files_target_user_cannot_access() {
    let file = "/test.txt";

    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .user(OTHER_USERNAME)
        .group(USERNAME)
        .group(OTHER_USERNAME)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .file(
            file,
            TextFile("")
                .chown(format!("{USERNAME}:{ROOT_GROUP}"))
                .chmod("460"),
        )
        .build();

    let test_cases = [
        // incorrect user
        &["-u", OTHER_USERNAME][..],
        // correct user, but does not have write permits
        &["-u", USERNAME][..],
        // incorrect group
        &["-u", OTHER_USERNAME, "-g", USERNAME][..],
        // group permission doesn't override matching user permissions
        &["-u", USERNAME, "-g", ROOT_GROUP][..],
        &["-g", ROOT_GROUP][..],
    ];

    for args in test_cases {
        let output = Command::new("sudoedit")
            .args(args)
            .arg(file)
            .as_user(USERNAME)
            .output(&env);

        assert_contains!(output.stderr(), "Permission denied");
        output.assert_exit_code(1);
    }
}

#[test]
fn can_edit_files_target_user_or_group_can_access() {
    // note: we already have tests that sudoedit "works" so we are skipping
    // the content check here---the point here is that sudoedit does not stop
    // the user.

    let file = "/test.txt";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .user(OTHER_USERNAME)
        .group(OTHER_USERNAME)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .file(
            file,
            TextFile("")
                .chown(format!("{OTHER_USERNAME}:{OTHER_USERNAME}"))
                .chmod("660"),
        )
        .build();

    for user in ["root", OTHER_USERNAME] {
        Command::new("sudoedit")
            .args(["-u", user, file])
            .as_user(USERNAME)
            .output(&env)
            .assert_success();
    }

    Command::new("sudoedit")
        .args(["-g", OTHER_USERNAME, file])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    Command::new("sudoedit")
        .args(["-u", USERNAME, "-g", OTHER_USERNAME, file])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}
