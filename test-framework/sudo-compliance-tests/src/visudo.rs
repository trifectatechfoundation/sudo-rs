use std::{thread, time::Duration};

use sudo_test::{Command, Env, TextFile};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD};

const ETC_SUDOERS: &str = "/etc/sudoers";

#[test]
#[ignore = "gh657"]
fn creates_sudoers_file_with_default_ownership_and_perms_if_it_doesnt_exist() -> Result<()> {
    let env = Env("").build()?;

    Command::new("rm")
        .args(["-f", ETC_SUDOERS])
        .output(&env)?
        .assert_success()?;

    Command::new("env")
        .args(["EDITOR=true", "visudo"])
        .output(&env)?
        .assert_success()?;

    let ls_output = Command::new("ls")
        .args(["-l", ETC_SUDOERS])
        .output(&env)?
        .stdout()?;

    assert!(ls_output.starts_with("-r--r----- 1 root root"));

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn errors_if_currently_being_edited() -> Result<()> {
    let editor_path = "/tmp/editor.sh";
    let env = Env("")
        .file(
            editor_path,
            TextFile(
                "#!/bin/sh
sleep 3",
            )
            .chmod("100"),
        )
        .build()?;

    let child = Command::new("env")
        .arg(format!("EDITOR={editor_path}"))
        .arg("visudo")
        .spawn(&env)?;

    // wait until `child` has been spawned
    thread::sleep(Duration::from_secs(1));

    let output = Command::new("env")
        .args(["EDITOR=true", "visudo"])
        .output(&env)?;

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
#[ignore = "gh657"]
fn passes_temporary_file_to_editor() -> Result<()> {
    let args_path = "/tmp/args.txt";
    let editor_path = "/tmp/editor.sh";
    let env = Env("")
        .file(
            editor_path,
            TextFile(format!(
                r#"#!/bin/sh
echo "$@" > {args_path}"#
            ))
            .chmod("100"),
        )
        .build()?;

    Command::new("env")
        .arg(format!("EDITOR={editor_path}"))
        .arg("visudo")
        .output(&env)?
        .assert_success()?;

    let args = Command::new("cat").arg(args_path).output(&env)?.stdout()?;

    assert_eq!("-- /etc/sudoers.tmp", args);

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn temporary_file_owner_and_perms() -> Result<()> {
    let args_path = "/tmp/args.txt";
    let editor_path = "/tmp/editor.sh";
    let env = Env("")
        .file(
            editor_path,
            TextFile(format!(
                r#"#!/bin/sh
ls -l /etc/sudoers.tmp > {args_path}"#
            ))
            .chmod("100"),
        )
        .build()?;

    Command::new("env")
        .arg(format!("EDITOR={editor_path}"))
        .arg("visudo")
        .output(&env)?
        .assert_success()?;

    let ls_output = Command::new("cat").arg(args_path).output(&env)?.stdout()?;

    assert!(ls_output.starts_with("-rwx------ 1 root root"));

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn saves_file_if_no_syntax_errors() -> Result<()> {
    let editor_path = "/tmp/editor.sh";
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env("")
        .file(
            editor_path,
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

    Command::new("env")
        .arg(format!("EDITOR={editor_path}"))
        .arg("visudo")
        .output(&env)?
        .assert_success()?;

    let sudoers = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, sudoers);

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn stderr_message_when_file_is_not_modified() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected).build()?;

    let output = Command::new("env")
        .args(["EDITOR=true", "visudo"])
        .output(&env)?;

    assert!(output.status().success());
    assert_eq!(output.stderr(), "visudo: /etc/sudoers.tmp unchanged");

    let actual = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}
