use std::os::unix::process::ExitStatusExt;
use std::thread;
use std::time::Duration;

use sudo_test::{Command, Env, TextFile, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
fn time_out() -> Result<()> {
    let timeout_seconds = 6;

    let script = include_str!("passwd_timeout.sh");
    let script_path = "/tmp/passwd_timeout.sh";

    let env = Env(format!(
        "{USERNAME} ALL=(ALL:ALL) ALL\nDefaults passwd_timeout={:04}",
        timeout_seconds as f64 / 60.0,
    ))
    .user(User(USERNAME).password(PASSWORD))
    .file(script_path, TextFile(script).chmod("777"))
    .build();

    let mut child = Command::new("sh")
        .arg(script_path)
        .as_user(USERNAME)
        .spawn(&env);

    thread::sleep(Duration::from_secs(timeout_seconds + 1));

    match child.try_wait()? {
        None => {
            child.kill()?;
            panic!("passwd_timeout did not force exit: {:?}", child.wait());
        }
        Some(status) => {
            if status.success() {
                panic!("succeeded without password: {:?}", child.wait());
            }
        }
    }

    let output = child.wait();
    output.assert_exit_code(1);
    let diagnostic = if sudo_test::is_original_sudo() {
        "timed out reading password"
    } else {
        "timed out"
    };
    assert_contains!(output.stderr(), diagnostic);
    Ok(())
}

#[test]
fn dont_time_out() -> Result<()> {
    let timeout_seconds = 5;

    let script = include_str!("passwd_timeout.sh");
    let script_path = "/tmp/passwd_timeout.sh";

    let env = Env(format!(
        "{USERNAME} ALL=(ALL:ALL) ALL\nDefaults passwd_timeout={:04}",
        timeout_seconds as f64 / 60.0,
    ))
    .user(User(USERNAME).password(PASSWORD))
    .file(script_path, TextFile(script).chmod("777"))
    .build();

    let mut child = Command::new("sh")
        .arg(script_path)
        .as_user(USERNAME)
        .spawn(&env);

    thread::sleep(Duration::from_secs(timeout_seconds - 2));

    child.kill()?;

    let output = child.wait();
    assert_eq!(output.status().signal(), Some(9 /* SIGKILL */));
    assert_not_contains!(output.stderr(), "timed out");
    Ok(())
}

#[test]
fn zero_time_out() -> Result<()> {
    let script = include_str!("passwd_timeout.sh");
    let script_path = "/tmp/passwd_timeout.sh";

    let env = Env(format!(
        "{USERNAME} ALL=(ALL:ALL) ALL\nDefaults passwd_timeout=0"
    ))
    .user(User(USERNAME).password(PASSWORD))
    .file(script_path, TextFile(script).chmod("777"))
    .build();

    let mut child = Command::new("sh")
        .arg(script_path)
        .as_user(USERNAME)
        .spawn(&env);

    thread::sleep(Duration::from_secs(5));

    match child.try_wait()? {
        None => {
            child.kill()?;
        }
        Some(_status) => {
            println!("exited with {:?}", child.wait());
            panic!();
        }
    }

    let output = child.wait();
    assert_eq!(output.status().signal(), Some(9 /* SIGKILL */));
    assert_not_contains!(output.stderr(), "timed out");
    Ok(())
}
