use sudo_test::{Command, Env};

use crate::{
    helpers::{self, PsAuxEntry},
    Result, SUDOERS_ALL_ALL_NOPASSWD,
};

fn fixture() -> Result<Vec<PsAuxEntry>> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"]).build()?;

    let child = Command::new("sudo")
        .args(["sh", "-c", "touch /tmp/barrier; sleep 3"])
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

    let mut sudo_related_processes = entries
        .into_iter()
        .filter(|entry| entry.command.contains("sh -c touch"))
        .collect::<Vec<_>>();

    sudo_related_processes.sort_by_key(|entry| entry.pid);

    Ok(sudo_related_processes)
}

#[test]
fn spawns_three_processes() -> Result<()> {
    let sudo_related_processes = fixture()?;

    assert_eq!(3, sudo_related_processes.len());

    Ok(())
}

#[test]
fn allocates_a_second_pty_which_is_assigned_to_the_command_process() -> Result<()> {
    let sudo_related_processes = fixture()?;

    let original = &sudo_related_processes[0];
    let monitor = &sudo_related_processes[1];
    let command = &sudo_related_processes[2];

    dbg!(original);
    dbg!(monitor);
    dbg!(command);

    // sanity checks
    assert!(original.command.starts_with("sudo "));
    assert!(monitor.command.starts_with("sudo "));
    assert!(!command.command.starts_with("sudo "));
    assert_ne!("?", original.tty);
    assert_ne!("?", monitor.tty);
    assert_ne!("?", command.tty);

    assert_eq!(monitor.tty, command.tty);
    assert_ne!(original.tty, monitor.tty);

    Ok(())
}

#[test]
fn process_state() -> Result<()> {
    const IS_A_SESSION_LEADER: char = 's';
    const IS_IN_FOREGROUND_PROCESS_GROUP: char = '+';

    let sudo_related_processes = fixture()?;

    let original = &sudo_related_processes[0];
    let monitor = &sudo_related_processes[1];
    let command = &sudo_related_processes[2];

    dbg!(original);
    dbg!(monitor);
    dbg!(command);

    // sanity checks
    assert!(original.command.starts_with("sudo "));
    assert!(monitor.command.starts_with("sudo "));
    assert!(!command.command.starts_with("sudo "));

    assert!(original
        .process_state
        .contains(IS_IN_FOREGROUND_PROCESS_GROUP));
    assert!(command
        .process_state
        .contains(IS_IN_FOREGROUND_PROCESS_GROUP));

    assert!(original.process_state.contains(IS_A_SESSION_LEADER));
    assert!(monitor.process_state.contains(IS_A_SESSION_LEADER));

    Ok(())
}

#[test]
fn terminal_is_restored() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"]).build()?;
    // Run `stty` before and after running sudo to check that the terminal configuration is
    // restored before sudo exits.
    let stdout = Command::new("sh")
        .args(["-c", "stty; sudo echo 'hello'; stty"])
        .tty(true)
        .output(&env)?
        .stdout()?;

    assert_contains!(stdout, "hello");
    let (before, after) = stdout.split_once("hello").unwrap();
    assert_eq!(before.trim(), after.trim());

    Ok(())
}

#[test]
fn pty_owner() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"]).build()?;

    let stdout = Command::new("sudo")
        .args(["sh", "-c", "stat $(tty) --format '%U %G'"])
        .tty(true)
        .output(&env)?
        .stdout()?;

    assert_eq!(stdout.trim(), "root tty");

    Ok(())
}
