use std::{thread, time::Duration};

use sudo_test::{
    helpers::assert_ls_output, Command, Env, EnvNoImplicit, TextFile, ETC_DIR, ETC_SUDOERS,
    ROOT_GROUP,
};

use crate::{Result, DEFAULT_EDITOR, PANIC_EXIT_CODE, SUDOERS_ALL_ALL_NOPASSWD};

mod flag_check;
mod flag_file;
mod flag_help;
mod flag_no_includes;
mod flag_owner;
mod flag_perms;
mod flag_quiet;
mod flag_strict;
mod flag_version;
mod include;
mod sudoers;
mod what_now_prompt;

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![(r"sudoers-[a-zA-Z0-9]{6}", "[mkdtemp]")],
            prepend_module_to_snapshot => false,
            snapshot_path => "snapshots/visudo",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

const TMP_SUDOERS: &str = "/tmp/sudoers";
const LOGS_PATH: &str = "/tmp/logs.txt";
const CHMOD_EXEC: &str = "100";
const EDITOR_DUMMY: &str = "#!/bin/sh
echo \"#\" >> \"$2\"";

#[test]
fn default_editor_is_usr_bin_editor() {
    let expected = "default editor was called";
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh

echo '{expected}' > {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("visudo").output(&env).assert_success();

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn creates_sudoers_file_with_default_ownership_and_perms_if_it_doesnt_exist() {
    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    Command::new("rm")
        .args(["-f", ETC_SUDOERS])
        .output(&env)
        .assert_success();

    Command::new("visudo").output(&env).assert_success();

    let ls_output = Command::new("ls")
        .args(["-l", ETC_SUDOERS])
        .output(&env)
        .stdout();

    assert_ls_output(&ls_output, "-r--r-----", "root", ROOT_GROUP);
}

#[test]
fn errors_if_currently_being_edited() {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
sleep 3",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let child = Command::new("visudo").spawn(&env);

    // wait until `child` has been spawned
    thread::sleep(Duration::from_secs(1));

    let output = Command::new("visudo").output(&env);

    child.wait().assert_success();

    output.assert_exit_code(1);
    assert_contains!(
        output.stderr(),
        format!("visudo: {ETC_DIR}/sudoers busy, try again later")
    );
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

    Command::new("visudo").output(&env).assert_success();

    let args = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    if sudo_test::is_original_sudo() {
        assert_eq!(format!("-- {ETC_DIR}/sudoers.tmp"), args);
    } else {
        assert_snapshot!(args);
    }
}

#[test]
fn temporary_file_owner_and_perms() {
    let editor_script = if sudo_test::is_original_sudo() {
        format!(
            r#"#!/bin/sh
ls -l {ETC_DIR}/sudoers.tmp > {LOGS_PATH}"#
        )
    } else {
        format!(
            r#"#!/bin/sh
ls -l /tmp/sudoers-*/sudoers > {LOGS_PATH}"#
        )
    };

    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(editor_script).chmod(CHMOD_EXEC))
        .build();

    Command::new("visudo").output(&env).assert_success();

    let ls_output = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    if sudo_test::is_original_sudo() {
        //TODO: this is incorrect, will be fixed in a future sudo; the
        //point of the test anyway is that the file is not accessible
        //by any other than the owner.
        assert_ls_output(&ls_output, "-rwx------", "root", ROOT_GROUP);
    } else {
        assert_ls_output(&ls_output, "-rw-------", "root", ROOT_GROUP);
    }
}

#[test]
fn saves_file_if_no_syntax_errors() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' >> $2"#
            ))
            .chmod("100"),
        )
        .build();

    Command::new("rm")
        .args(["-f", ETC_SUDOERS])
        .output(&env)
        .assert_success();

    Command::new("visudo").output(&env).assert_success();

    let sudoers = Command::new("cat").arg(ETC_SUDOERS).output(&env).stdout();

    assert_eq!(expected, sudoers);
}

#[test]
fn stderr_message_when_file_is_not_modified() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = EnvNoImplicit(expected)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
                 touch \"$2\"",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("visudo").output(&env);

    output.assert_success();
    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_eq!(
            output.stderr(),
            format!("visudo: {ETC_DIR}/sudoers.tmp unchanged")
        );
    } else {
        assert_snapshot!(stderr);
    }

    let actual = Command::new("cat").arg(ETC_SUDOERS).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn does_not_save_the_file_if_there_are_syntax_errors() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = EnvNoImplicit(expected)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh

echo 'this is fine' > $2",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("visudo").output(&env);

    output.assert_success();
    assert_contains!(output.stderr(), "syntax error");

    let actual = Command::new("cat").arg(ETC_SUDOERS).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn editor_exits_with_a_nonzero_code() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = EnvNoImplicit(SUDOERS_ALL_ALL_NOPASSWD)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
exit 11",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("visudo").output(&env);

    output.assert_success();

    let actual = Command::new("cat").arg(ETC_SUDOERS).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn temporary_file_is_deleted_during_editing() {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
rm $2",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("visudo").output(&env);

    output.assert_exit_code(1);
    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_contains!(
            stderr,
            format!("visudo: unable to re-open temporary file ({ETC_DIR}/sudoers.tmp), {ETC_DIR}/sudoers unchanged")
        );
    } else {
        assert_snapshot!(stderr.replace(ETC_DIR, "<ETC_DIR>"));
    }
}

#[test]
fn temp_file_initial_contents() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = EnvNoImplicit(expected)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh
cp $2 {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("visudo").output(&env).assert_success();

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn temporary_file_is_deleted_when_done() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    Command::new("visudo").output(&env).assert_success();

    let output = Command::new("find")
        .args(["/tmp", "-name", "sudoers-*"])
        .output(&env)
        .stdout();

    assert!(output.is_empty());
}

#[test]
fn temporary_file_is_deleted_when_terminated_by_signal() {
    let kill_visudo = "/root/kill-visudo.sh";
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
touch /tmp/barrier
sleep 2",
            )
            .chmod(CHMOD_EXEC),
        )
        .file(kill_visudo, include_str!("visudo/kill-visudo.sh"))
        .build();

    let child = Command::new("visudo").spawn(&env);

    Command::new("sh")
        .args([kill_visudo, "-TERM"])
        .output(&env)
        .assert_success();

    assert!(!child.wait().status().success());

    let output = Command::new("find")
        .args(["/tmp", "-name", "sudoers-*"])
        .output(&env)
        .stdout();

    assert!(output.is_empty());
}

#[test]
fn does_not_panic_on_io_errors_parse_ok() -> Result<()> {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh

echo ' ' >> $2",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("bash")
        .args(["-c", "visudo | true; echo \"${PIPESTATUS[0]}\""])
        .output(&env);

    let stderr = output.stderr();
    assert!(stderr.is_empty());

    let exit_code = output.stdout().parse()?;
    assert_ne!(PANIC_EXIT_CODE, exit_code);
    assert_eq!(0, exit_code);

    Ok(())
}

#[test]
fn does_not_panic_on_io_errors_parse_error() -> Result<()> {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh

echo 'bad syntax' >> $2",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("bash")
        .args(["-c", "visudo | true; echo \"${PIPESTATUS[0]}\""])
        .output(&env);

    let stderr = output.stderr();
    assert_not_contains!(stderr, "panicked");
    assert_not_contains!(stderr, "IO error");

    let exit_code = output.stdout().parse()?;
    assert_ne!(PANIC_EXIT_CODE, exit_code);
    // ogvisudo exits with 141 = SIGPIPE; visudo-rs exits with code 1 but the difference is not
    // relevant to this test
    // assert_eq!(141, exit_code);

    Ok(())
}
