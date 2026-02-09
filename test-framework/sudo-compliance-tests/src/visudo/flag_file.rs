use sudo_test::{Command, Env, ROOT_GROUP, TextFile, helpers::assert_ls_output};
use sudo_test::{EnvNoImplicit, is_original_sudo};

use crate::{
    SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_ROOT_ALL, USERNAME,
    visudo::{CHMOD_EXEC, DEFAULT_EDITOR, EDITOR_DUMMY, ETC_SUDOERS, LOGS_PATH, TMP_SUDOERS},
};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![(r"sudoers-[a-zA-Z0-9]{6}", "[mkdtemp]")],
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/visudo/flag_file",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn creates_sudoers_file_with_default_ownership_and_perms_if_it_doesnt_exist() {
    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    let file_path = TMP_SUDOERS;
    Command::new("visudo")
        .args(["-f", file_path])
        .output(&env)
        .assert_success();

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)
        .stdout();

    assert_ls_output(
        &ls_output,
        if cfg!(target_os = "freebsd") && is_original_sudo() {
            "-rw-------"
        } else {
            "-rw-r-----"
        },
        "root",
        ROOT_GROUP,
    );
}

#[test]
fn saves_file_if_no_syntax_errors() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let unexpected = SUDOERS_ROOT_ALL;
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, unexpected)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' > $2"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("visudo")
        .args(["-f", file_path])
        .output(&env)
        .assert_success();

    let actual = Command::new("cat").arg(file_path).output(&env).stdout();
    assert_eq!(expected, actual);
}

#[test]
fn positional_argument() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let unexpected = SUDOERS_ROOT_ALL;
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, unexpected)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' > $2"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("visudo")
        .arg(file_path)
        .output(&env)
        .assert_success();

    let actual = Command::new("cat").arg(file_path).output(&env).stdout();
    assert_eq!(expected, actual);
}

#[test]
fn flag_has_precedence_over_positional_argument() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let original = SUDOERS_ROOT_ALL;
    let file_path = "/tmp/sudoers";
    let file_path2 = "/tmp/sudoers2";
    let env = Env("")
        .file(file_path, original)
        .file(file_path2, original)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' > $2"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("visudo")
        .args(["-f", file_path])
        .arg(file_path2)
        .output(&env)
        .assert_success();

    let changed = Command::new("cat").arg(file_path).output(&env).stdout();
    assert_eq!(expected, changed);

    let unchanged = Command::new("cat").arg(file_path2).output(&env).stdout();
    assert_eq!(original, unchanged);
}

#[test]
fn etc_sudoers_is_not_modified() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let unexpected = SUDOERS_ROOT_ALL;
    let env = EnvNoImplicit(expected)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh
echo '{unexpected}' > $2"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("visudo")
        .args(["--file", TMP_SUDOERS])
        .output(&env)
        .assert_success();

    let actual = Command::new("cat").arg(ETC_SUDOERS).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn passes_temporary_file_to_editor() {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo "$@" > {LOGS_PATH}"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    let file_path = TMP_SUDOERS;
    Command::new("visudo")
        .args(["--file", file_path])
        .output(&env)
        .assert_success();

    let args = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    if sudo_test::is_original_sudo() {
        assert_eq!(format!("-- {file_path}.tmp"), args);
    } else {
        assert_snapshot!(args);
    }
}

#[test]
fn regular_user_can_create_file() {
    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod("755"))
        .user(USERNAME)
        .build();

    let file_path = TMP_SUDOERS;
    Command::new("visudo")
        .args(["-f", file_path])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let ls_output = Command::new("ls")
        .args(["-l", file_path])
        .output(&env)
        .stdout();

    assert_ls_output(
        &ls_output,
        if cfg!(target_os = "freebsd") && is_original_sudo() {
            "-rw-------"
        } else {
            "-rw-r-----"
        },
        USERNAME,
        if cfg!(target_os = "freebsd") {
            "wheel"
        } else {
            "users"
        },
    );
}

#[test]
fn regular_user_can_update_a_file_they_own() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let unexpected = SUDOERS_ROOT_ALL;
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, TextFile(unexpected).chown(USERNAME).chmod("666"))
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' > $2"#
            ))
            .chmod("777"),
        )
        .user(USERNAME)
        .build();

    Command::new("visudo")
        .args(["-f", file_path])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let sudoers = Command::new("cat").arg(file_path).output(&env).stdout();

    assert_eq!(expected, sudoers);
}

#[test]
#[ignore = "gh657"]
fn regular_user_cannot_update_a_file_they_dont_own() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let unexpected = SUDOERS_ROOT_ALL;
    let file_path = TMP_SUDOERS;
    let env = Env("")
        .file(file_path, TextFile(unexpected).chmod("666"))
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' > $2"#
            ))
            .chmod("777"),
        )
        .user(USERNAME)
        .build();

    let output = Command::new("visudo")
        .args(["-f", file_path])
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);
    assert_contains!(
        output.stderr(),
        "visudo: unable to set (uid, gid) of /tmp/sudoers.tmp"
    );
}
