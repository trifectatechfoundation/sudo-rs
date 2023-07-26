use std::{thread, time::Duration};

use sudo_test::{Command, Env, TextFile};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD};

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
const ETC_SUDOERS: &str = "/etc/sudoers";
const DEFAULT_EDITOR: &str = "/usr/bin/editor";
const LOGS_PATH: &str = "/tmp/logs.txt";
const CHMOD_EXEC: &str = "100";
const EDITOR_TRUE: &str = "#!/bin/sh
true";

#[test]
fn default_editor_is_usr_bin_editor() -> Result<()> {
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
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn creates_sudoers_file_with_default_ownership_and_perms_if_it_doesnt_exist() -> Result<()> {
    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    Command::new("rm")
        .args(["-f", ETC_SUDOERS])
        .output(&env)?
        .assert_success()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let ls_output = Command::new("ls")
        .args(["-l", ETC_SUDOERS])
        .output(&env)?
        .stdout()?;

    assert!(ls_output.starts_with("-r--r----- 1 root root"));

    Ok(())
}

#[test]
fn errors_if_currently_being_edited() -> Result<()> {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
sleep 3",
            )
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    let child = Command::new("visudo").spawn(&env)?;

    // wait until `child` has been spawned
    thread::sleep(Duration::from_secs(1));

    let output = Command::new("visudo").output(&env)?;

    child.wait()?.assert_success()?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(
        output.stderr(),
        "visudo: /etc/sudoers busy, try again later"
    );

    Ok(())
}

#[test]
fn passes_temporary_file_to_editor() -> Result<()> {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo "$@" > {LOGS_PATH}"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let args = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    if sudo_test::is_original_sudo() {
        assert_eq!("-- /etc/sudoers.tmp", args);
    } else {
        assert_snapshot!(args);
    }

    Ok(())
}

#[test]
fn temporary_file_owner_and_perms() -> Result<()> {
    let editor_script = if sudo_test::is_original_sudo() {
        format!(
            r#"#!/bin/sh
ls -l /etc/sudoers.tmp > {LOGS_PATH}"#
        )
    } else {
        format!(
            r#"#!/bin/sh
ls -l /tmp/sudoers-*/sudoers > {LOGS_PATH}"#
        )
    };

    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(editor_script).chmod(CHMOD_EXEC))
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let ls_output = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert!(ls_output.starts_with("-rwx------ 1 root root"));

    Ok(())
}

#[test]
fn saves_file_if_no_syntax_errors() -> Result<()> {
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
        .build()?;

    Command::new("rm")
        .args(["-f", ETC_SUDOERS])
        .output(&env)?
        .assert_success()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let sudoers = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, sudoers);

    Ok(())
}

#[test]
fn stderr_message_when_file_is_not_modified() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    let output = Command::new("visudo").output(&env)?;

    assert!(output.status().success());
    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_eq!(output.stderr(), "visudo: /etc/sudoers.tmp unchanged");
    } else {
        assert_snapshot!(stderr);
    }

    let actual = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn does_not_save_the_file_if_there_are_syntax_errors() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh

echo 'this is fine' > $2",
            )
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    let output = Command::new("visudo").output(&env)?;

    assert!(output.status().success());
    assert_contains!(output.stderr(), "syntax error");

    let actual = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn editor_exits_with_a_nonzero_code() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
exit 11",
            )
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    let output = Command::new("visudo").output(&env)?;

    assert!(output.status().success());

    let actual = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn temporary_file_is_deleted_during_edition() -> Result<()> {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
rm $2",
            )
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    let output = Command::new("visudo").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_contains!(
            stderr,
            "visudo: unable to re-open temporary file (/etc/sudoers.tmp), /etc/sudoers unchanged"
        );
    } else {
        assert_snapshot!(stderr);
    }

    Ok(())
}

#[test]
fn temp_file_initial_contents() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh
cp $2 {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn temporary_file_is_deleted_when_done() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let output = Command::new("find")
        .args(["/tmp", "-name", "sudoers-*"])
        .output(&env)?
        .stdout()?;

    assert!(output.is_empty());

    Ok(())
}

#[test]
fn temporary_file_is_deleted_when_terminated_by_signal() -> Result<()> {
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
        .build()?;

    let child = Command::new("visudo").spawn(&env)?;

    Command::new("sh")
        .args([kill_visudo, "-TERM"])
        .output(&env)?
        .assert_success()?;

    assert!(!child.wait()?.status().success());

    let output = Command::new("find")
        .args(["/tmp", "-name", "sudoers-*"])
        .output(&env)?
        .stdout()?;

    assert!(output.is_empty());

    Ok(())
}
