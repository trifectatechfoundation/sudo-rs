use sudo_test::{Command, Env, TextFile};

use crate::{helpers, Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/path_search",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn can_find_command_not_visible_to_regular_user() -> Result<()> {
    let path = "/root/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(path, TextFile("#!/bin/sh").chmod("100"))
        .build()?;

    Command::new("sh")
        .args(["-c", "export PATH=/root; cd /; /usr/bin/sudo my-script"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn when_path_is_unset_does_not_search_in_default_path_set_for_command_execution() -> Result<()> {
    let path = "/usr/bin/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh").chmod("777"))
        .build()?;

    let default_path = Command::new("sh")
        .args(["-c", "unset PATH; /usr/bin/sudo /usr/bin/printenv PATH"])
        .output(&env)?
        .stdout()?;

    // sanity check that `/usr/bin` is in sudo's default PATH
    let default_path = helpers::parse_path(&default_path);
    assert!(default_path.contains("/usr/bin"));

    let output = Command::new("sh")
        .args(["-c", "unset PATH; /usr/bin/sudo my-script"])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "'my-script': command not found");
    }

    Ok(())
}

#[test]
fn ignores_path_for_qualified_commands() -> Result<()> {
    let path = "/root/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh").chmod("100"))
        .build()?;

    for param in ["/root/my-script", "./my-script"] {
        Command::new("sh")
            .args(["-c", &format!("cd /root; sudo {param}")])
            .as_user("root")
            .output(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn paths_are_matched_using_realpath_in_sudoers() -> Result<()> {
    let env = Env(["ALL ALL = /bin/true"]).build()?;

    // this test assumes /bin is a symbolic link for /usr/bin, which is the
    // case on Debian bookworm; if it fails for original sudo, either change the
    // dockerfile or explicitly create a symbolic link

    Command::new("sudo")
        .arg("/usr/bin/true")
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn paths_are_matched_using_realpath_in_arguments() -> Result<()> {
    let env = Env(["ALL ALL = /usr/bin/true"]).build()?;

    // this test assumes /bin is a symbolic link for /usr/bin, which is the
    // case on Debian bookworm; if it fails for original sudo, either change the
    // dockerfile or explicitly create a symbolic link

    Command::new("sudo")
        .arg("/bin/true")
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn arg0_native_is_passed_from_commandline() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let output = Command::new("sh")
        .args([
            "-c",
            "ln -s /bin/ls /bin/foo; sudo /bin/foo --invalid-flag; true",
        ])
        .output(&env)?;

    let stderr = output.stderr();
    assert_starts_with!(stderr, "/bin/foo: unrecognized option");

    Ok(())
}

#[test]
fn arg0_native_is_resolved_from_commandline() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let output = Command::new("sh")
        .args([
            "-c",
            "ln -s /bin/ls /bin/foo; sudo foo --invalid-flag; true",
        ])
        .output(&env)?;

    let stderr = output.stderr();
    assert_starts_with!(stderr, "foo: unrecognized option");

    Ok(())
}

#[test]
#[ignore = "gh735"]
fn arg0_script_is_passed_from_commandline() -> Result<()> {
    let path = "/bin/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh\necho $0").chmod("777"))
        .build()?;

    let output = Command::new("sh")
        .args(["-c", &format!("ln -s {path} /bin/foo; sudo /bin/foo")])
        .output(&env)?;

    let stdout = output.stdout()?;
    assert_eq!(stdout, "/bin/foo");

    Ok(())
}

#[test]
fn arg0_script_is_resolved_from_commandline() -> Result<()> {
    let path = "/bin/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh\necho $0").chmod("777"))
        .build()?;

    let output = Command::new("sh")
        .args(["-c", &format!("ln -s {path} /bin/foo; sudo foo")])
        .output(&env)?;

    let stdout = output.stdout()?;
    assert_eq!(stdout, "/usr/bin/foo");

    Ok(())
}
