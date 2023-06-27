use sudo_test::{Command, Env};

use crate::{helpers, Result};

#[test]
fn no_tty_allocation_when_absent() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("su")
        .args(["-c", "ps aux"])
        .output(&env)?
        .stdout()?;

    let entries = helpers::parse_ps_aux(&stdout);

    dbg!(&entries);

    let su_entry = entries
        .iter()
        .find(|entry| entry.command == "su -c ps aux")
        .expect("`su` process not found");

    let no_tty = "?";
    assert_eq!(no_tty, su_entry.tty);

    let su_is_session_leader = su_entry.process_state.contains('s');
    assert!(su_is_session_leader);

    let command_entry = entries
        .iter()
        .find(|entry| entry.command == "ps aux")
        .expect("`ps aux` process not found");

    assert_eq!(no_tty, command_entry.tty);

    let command_is_session_leader = command_entry.process_state.contains('s');
    assert!(!command_is_session_leader);

    Ok(())
}

#[test]
fn when_present_tty_is_allocated() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("su")
        .args(["--pty", "-c", "ps aux"])
        .output(&env)?
        .stdout()?;

    let entries = helpers::parse_ps_aux(&stdout);

    dbg!(&entries);

    let su_entry = entries
        .iter()
        .find(|entry| entry.command.starts_with("su --pty -c"))
        .expect("`su` process not found");

    let no_tty = "?";
    assert_eq!(no_tty, su_entry.tty);

    let su_is_session_leader = su_entry.process_state.contains('s');
    assert!(su_is_session_leader);

    let command_entry = entries
        .iter()
        .find(|entry| entry.command == "ps aux")
        .expect("`ps aux` process not found");

    let command_has_tty = command_entry.tty.starts_with("pts/");
    assert!(command_has_tty);

    let command_is_session_leader = command_entry.process_state.contains('s');
    assert!(command_is_session_leader);

    let command_is_in_the_foreground_process_group = command_entry.process_state.contains('+');
    assert!(command_is_in_the_foreground_process_group);

    Ok(())
}
