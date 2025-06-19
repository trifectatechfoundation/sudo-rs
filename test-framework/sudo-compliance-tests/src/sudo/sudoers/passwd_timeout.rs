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
        "{USERNAME} ALL=(ALL:ALL) ALL
Defaults passwd_timeout={}",
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

    match child.try_wait() {
        Ok(None) => {
            child.kill()?;
            panic!("passwd_timeout did not force exit");
        }
        Ok(Some(ret)) => {
            if ret.success() {
                panic!("succeeded without password");
            }
        }
        _ => {}
    }

    let output = child.wait();
    let diagnostic = if sudo_test::is_original_sudo() {
        "timed out reading password"
    } else {
        "timed out"
    };
    assert_contains!(output.stderr(), diagnostic);
    Ok(())
}
