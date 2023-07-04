use sudo_test::{Command, Env, TextFile};

use crate::{Result, USERNAME};

#[test]
fn signal_sent_by_child_process_is_ignored() -> Result<()> {
    let script = include_str!("kill-su-parent.sh");

    let script_path = "/tmp/script.sh";
    let env = Env("")
        .user(USERNAME)
        .file(script_path, TextFile(script).chmod("777"))
        .build()?;

    let output = Command::new("su")
        .arg("-c")
        .arg(format!("sh {script_path}"))
        .arg("root")
        .output(&env)?;

    assert!(output.status().success());
    assert!(output.stderr().is_empty());

    Ok(())
}

#[test]
fn signal_is_forwarded_to_child() -> Result<()> {
    let expected = "got signal";
    let signal = "TERM";
    let expects_signal = "/root/expects-signal.sh";
    let kill_su = "/root/kill-su.sh";
    let env = Env("")
        .file(expects_signal, include_str!("expects-signal.sh"))
        .file(kill_su, include_str!("kill-su.sh"))
        .build()?;

    let child = Command::new("su")
        .arg("-c")
        .arg(format!("sh {expects_signal} {signal}"))
        .spawn(&env)?;

    Command::new("sh")
        .arg(kill_su)
        .arg(format!("-{signal}"))
        .output(&env)?
        .assert_success()?;

    let actual = child.wait()?.stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn child_terminated_by_signal() -> Result<()> {
    let env = Env("").build()?;

    // child process sends SIGTERM to itself
    let output = Command::new("su")
        .arg("-c")
        .arg("sh -c 'kill $$'")
        .output(&env)?;

    assert_eq!(Some(143), output.status().code());
    assert!(output.stderr().is_empty());

    Ok(())
}

#[test]
fn sigstp_works() -> Result<()> {
    const STOP_DELAY: u64 = 5;
    const NUM_ITERATIONS: usize = 5;

    let script_path = "/tmp/script.sh";
    let env = Env("")
        .file(script_path, include_str!("sigtstp.bash"))
        .build()?;

    let output = Command::new("bash")
        .arg(script_path)
        .output(&env)?
        .stdout()?;

    let timestamps = output
        .lines()
        .filter_map(|line| line.parse::<u64>().ok())
        .collect::<Vec<_>>();

    dbg!(&timestamps);

    assert_eq!(NUM_ITERATIONS, timestamps.len());

    let suspended_iterations = timestamps
        .windows(2)
        .filter(|window| {
            let prev_timestamp = window[0];
            let curr_timestamp = window[1];
            let delta = curr_timestamp - prev_timestamp;

            delta >= STOP_DELAY
        })
        .count();
    let did_suspend = suspended_iterations == 1;

    assert!(did_suspend);

    Ok(())
}

#[test]
fn sigalrm_terminates_command() -> Result<()> {
    let expected = "got signal";
    let expects_signal = "/root/expects-signal.sh";
    let kill_su = "/root/kill-su.sh";
    let env = Env("")
        .file(expects_signal, include_str!("expects-signal.sh"))
        .file(kill_su, include_str!("kill-su.sh"))
        .build()?;

    let child = Command::new("su")
        .arg("-c")
        .arg(format!("sh {expects_signal} HUP TERM"))
        .spawn(&env)?;

    Command::new("sh")
        .args([kill_su, "-ALRM"])
        .output(&env)?
        .assert_success()?;

    let actual = child.wait()?.stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}
