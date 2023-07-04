use sudo_test::{
    helpers::{self, PsAuxEntry},
    Command, Env,
};

use crate::Result;

#[derive(Debug)]
struct Processes {
    original: PsAuxEntry,
    monitor: PsAuxEntry,
    command: PsAuxEntry,
}

fn fixture() -> Result<Processes> {
    let env = Env("").build()?;

    let child = Command::new("su")
        .args(["--pty", "-c"])
        .arg("sh -c 'touch /tmp/barrier; sleep 3'")
        .tty(true)
        .spawn(&env)?;

    let ps_aux = Command::new("sh")
        .args([
            "-c",
            "until [ -f /tmp/barrier ]; do sleep 0.1; done; ps aux",
        ])
        .output(&env)?
        .stdout()?;

    child.wait()?.assert_success()?;

    let entries = helpers::parse_ps_aux(&ps_aux);

    let mut su_related_processes = entries
        .into_iter()
        .filter(|entry| {
            entry.command.contains("sh -c 'touch") | entry.command.starts_with("sh -c touch")
        })
        .collect::<Vec<_>>();

    su_related_processes.sort_by_key(|entry| entry.pid);

    let [original, monitor, command]: [PsAuxEntry; 3] = su_related_processes
        .try_into()
        .map_err(|_| "expected 3 su-related processes")?;

    // sanity check
    let prefix = "su ";
    assert!(original.command.starts_with(prefix));
    assert!(monitor.command.starts_with(prefix));
    assert!(!command.command.starts_with(prefix));

    assert!(original.has_tty());
    assert!(monitor.has_tty());
    assert!(command.has_tty());

    Ok(Processes {
        original,
        monitor,
        command,
    })
}

#[test]
fn spawns_three_processes() -> Result<()> {
    fixture().map(drop)
}

#[test]
fn allocates_a_second_pty_which_is_assigned_to_the_command_process() -> Result<()> {
    let Processes {
        original,
        monitor,
        command,
    } = fixture()?;

    assert_eq!(monitor.tty, command.tty);
    assert_ne!(original.tty, monitor.tty);

    Ok(())
}

#[test]
fn process_state() -> Result<()> {
    let Processes {
        original,
        monitor,
        command,
    } = fixture()?;

    assert!(original.is_in_the_foreground_process_group());
    assert!(command.is_in_the_foreground_process_group());

    assert!(original.is_session_leader());
    assert!(monitor.is_session_leader());

    Ok(())
}

#[test]
fn terminal_is_restored() -> Result<()> {
    let env = Env("").build()?;
    // Run `stty` before and after running sudo to check that the terminal configuration is
    // restored before sudo exits.
    let stdout = Command::new("sh")
        .args(["-c", "stty; su --pty -c 'echo hello'; stty"])
        .tty(true)
        .output(&env)?
        .stdout()?;

    let (before, after) = stdout.split_once("hello").unwrap();
    assert_eq!(before.trim(), after.trim());

    Ok(())
}

#[test]
fn pty_owner() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("su")
        .args(["--pty", "-c"])
        .arg("stat $(tty) --format '%U %G'")
        .tty(true)
        .output(&env)?
        .stdout()?;
    assert_eq!(stdout.trim(), "root tty");

    Ok(())
}

#[test]
fn stdin_pipe() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("sh")
        .args(["-c", "echo 'hello world' | su --pty -c 'grep -o hello'"])
        .tty(true)
        .output(&env)?
        .stdout()?;

    assert_eq!(stdout.trim(), "hello");

    Ok(())
}

#[test]
fn stdout_pipe() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("sh")
        .args(["-c", "su --pty -c 'echo hello world' | grep -o hello"])
        .tty(true)
        .output(&env)?
        .stdout()?;

    assert_eq!(stdout.trim(), "hello");

    Ok(())
}

#[test]
fn stderr_pipe() -> Result<()> {
    let env = Env("").build()?;

    let output = Command::new("sh")
        .args([
            "-c",
            "2>/tmp/stderr.txt su --pty -c '>&2 echo \"hello world\"'",
        ])
        .tty(true)
        .output(&env)?;

    assert!(output.stderr().is_empty());

    let stdout = Command::new("cat")
        .arg("/tmp/stderr.txt")
        .output(&env)?
        .stdout()?;

    assert_eq!(stdout, "hello world");

    Ok(())
}
