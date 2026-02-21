use sudo_test::{BIN_SUDO, Command, Env, User, helpers::assert_ls_output, is_original_sudo};

use crate::{PANIC_EXIT_CODE, Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/misc",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn user_not_in_passwd_database_cannot_use_sudo() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build();

    let output = Command::new("sudo")
        .arg("true")
        .as_user_id(1000)
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "user 'current user' not found");
    }
}

fn closes_open_file_descriptors(tty: bool) {
    let script_path = "/tmp/script.bash";
    let defaults = if tty {
        "Defaults use_pty"
    } else {
        "Defaults !use_pty"
    };
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, defaults])
        .file(
            script_path,
            include_str!("misc/read-parents-open-file-descriptor.bash"),
        )
        .build();

    let output = Command::new("bash").arg(script_path).tty(tty).output(&env);

    output.assert_exit_code(1);

    assert_contains!(
        if tty {
            // Docker merges stderr into stdout with "--tty". See gh622
            output.stdout_unchecked()
        } else {
            output.stderr()
        },
        "42: Bad file descriptor"
    );
}

#[test]
fn closes_open_file_descriptors_with_tty() {
    closes_open_file_descriptors(true)
}

#[test]
fn closes_open_file_descriptors_without_tty() {
    closes_open_file_descriptors(false)
}

#[test]
fn sudo_binary_lacks_setuid_flag() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build();

    Command::new("chmod")
        .args(["0755", BIN_SUDO])
        .output(&env)
        .assert_success();

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    assert_contains!(
        output.stderr(),
        "sudo must be owned by uid 0 and have the setuid bit set"
    );
}

#[test]
fn sudo_binary_is_not_owned_by_root() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build();

    Command::new("chown")
        .args([USERNAME, BIN_SUDO])
        .output(&env)
        .assert_success();

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    assert_contains!(
        output.stderr(),
        "sudo must be owned by uid 0 and have the setuid bit set"
    );
}

#[test]
fn sudo_binary_is_not_owned_by_root_and_ran_as_root() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build();

    Command::new("chmod")
        .args(["0755", BIN_SUDO])
        .output(&env)
        .assert_success();

    Command::new("chown")
        .args([USERNAME, BIN_SUDO])
        .output(&env)
        .assert_success();

    Command::new("sudo")
        .arg("true")
        .as_user("root")
        .output(&env)
        .assert_success();
}

#[test]
fn works_when_invoked_through_a_symlink() {
    let symlink_path = "/tmp/sudo";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build();

    Command::new("ln")
        .args(["-s", BIN_SUDO, symlink_path])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    // symlink is not owned by root
    let ls_output = Command::new("ls")
        .args(["-ahl", symlink_path])
        .output(&env)
        .stdout();

    if cfg!(target_os = "freebsd") {
        assert_ls_output(&ls_output, "lrwx------", "ferris", "wheel");
    } else {
        assert_ls_output(&ls_output, "lrwxrwxrwx", "ferris", "users");
    }

    // still, we expect sudo to work because the executable behind the symlink has the right
    // ownership and permissions
    Command::new(symlink_path)
        .arg("true")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn does_not_panic_on_io_errors_no_command() -> Result<()> {
    let env = Env("").build();

    let output = Command::new("bash")
        .args(["-c", "sudo 2>&1 | true; echo \"${PIPESTATUS[0]}\""])
        .output(&env);

    let exit_code = output.stdout().parse()?;
    assert_ne!(PANIC_EXIT_CODE, exit_code);
    assert_eq!(1, exit_code);

    Ok(())
}

#[test]
fn does_not_panic_on_io_errors_cli_error() -> Result<()> {
    let env = Env("").build();

    let output = Command::new("bash")
        .args([
            "-c",
            "sudo --bad-flag 2>&1 | true; echo \"${PIPESTATUS[0]}\"",
        ])
        .output(&env);

    let exit_code = output.stdout().parse()?;
    assert_ne!(PANIC_EXIT_CODE, exit_code);
    assert_eq!(1, exit_code);

    Ok(())
}

#[test]
fn does_not_panic_on_invalid_executable() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build();

    let output = Command::new("bash")
        .args(["-c", "sudo /tmp; a=$?; sleep .1; exit $a"])
        .tty(true) // Necessary to reproduce the panic
        .output(&env);
    output.assert_exit_code(1);

    assert!(!output.stderr().contains("panic"), "{output:?}");
    assert!(!output.stdout_unchecked().contains("panic"), "{output:?}");
    if is_original_sudo() {
        assert_contains!(output.stdout_unchecked(), "command not found");
    } else {
        assert_contains!(output.stdout_unchecked(), "Permission denied");
    }
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "FreeBSD uses a binary database as canonical source of users"
)]
fn long_username() {
    // `useradd` limits usernames to 32 characters
    // directly write to `/etc/passwd` to work around this limitation
    let username = "a".repeat(33);
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build();

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {username}:x:1000:1000::/tmp:/bin/sh >> /etc/passwd && echo {username}:x:1000: >> /etc/group"
        ))
        .output(&env)
        .assert_success();

    Command::new("sudo")
        .arg("-u")
        .arg(username)
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "FreeBSD uses a binary database as canonical source of users"
)]
fn missing_primary_group() {
    let username = "user";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build();

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {username}:x:1000:1000::/tmp:/bin/sh >> /etc/passwd"
        ))
        .output(&env)
        .assert_success();

    Command::new("sudo")
        .arg("-u")
        .arg(username)
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn rootpw_option_works() {
    const PASSWORD: &str = "passw0rd";
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env(format!(
        "Defaults rootpw\nDefaults passwd_tries=1\n{USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user_password("root", ROOT_PASSWORD)
    .user(User(USERNAME).password(PASSWORD))
    .build();

    // User password is not accepted when rootpw is enabled
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env);
    assert!(!output.status().success());

    // Root password is accepted when rootpw is enabled
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env);
    output.assert_success();
}

#[test]
fn rootpw_option_doesnt_affect_authorization() {
    const PASSWORD: &str = "passw0rd";
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env("Defaults rootpw\nroot ALL=(ALL:ALL) ALL")
        .user_password("root", ROOT_PASSWORD)
        .user(User(USERNAME).password(PASSWORD))
        .build();

    // Even though we accept the root password when rootpw is enabled, we still check that the
    // actual invoking user is authorized to run the command.
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env);
    assert!(!output.status().success());
}

#[test]
fn targetpw_option_works() {
    const PASSWORD: &str = "passw0rd";
    const PASSWORD2: &str = "notr00t";

    let env = Env(format!(
        "Defaults targetpw\nDefaults passwd_tries=1\n{USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user(User(USERNAME).password(PASSWORD))
    .user(User("user2").password(PASSWORD2))
    .build();

    // User password is not accepted when targetpw is enabled
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S -u user2 true"))
        .as_user(USERNAME)
        .output(&env);
    assert!(!output.status().success());

    // Target user password is accepted when targetpw is enabled
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD2} | sudo -S -u user2 true"))
        .as_user(USERNAME)
        .output(&env);
    output.assert_success();
}

#[test]
fn targetpw_option_doesnt_affect_authorization() {
    const PASSWORD: &str = "passw0rd";
    const PASSWORD2: &str = "notr00t";

    let env = Env("Defaults targetpw\nuser2 ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(PASSWORD))
        .user(User("user2").password(PASSWORD2))
        .build();

    // Even though we accept the target user password when targetpw is enabled,
    // we still check that the actual invoking user is authorized to run the command.
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD2} | sudo -S -u user2 true"))
        .as_user(USERNAME)
        .output(&env);
    assert!(!output.status().success());
}

#[test]
fn rootpw_takes_priority_over_targetpw() {
    const PASSWORD: &str = "passw0rd";
    const PASSWORD2: &str = "notr00t";
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env(format!(
        "Defaults rootpw, targetpw\nDefaults passwd_tries=1\n{USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user_password("root", ROOT_PASSWORD)
    .user(User(USERNAME).password(PASSWORD))
    .user(User("user2").password(PASSWORD2))
    .build();

    // Root password is accepted when targetpw is enabled
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S -u user2 true"))
        .as_user(USERNAME)
        .output(&env);
    output.assert_success();

    // Target user password is not accepted when targetpw is enabled
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD2} | sudo -S -u user2 true"))
        .as_user(USERNAME)
        .output(&env);
    assert!(!output.status().success());
}

#[test]
fn signal_handlers_should_be_restored_before_execve() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD]).build();

    let stdout = Command::new("/bin/sh")
        .args(["-c", "(false | timeout 1 sudo head /dev/tty); true"])
        .tty(true)
        .output(&env)
        .stdout();

    assert_eq!(stdout, "");
}
