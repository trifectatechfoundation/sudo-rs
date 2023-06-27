use sudo_test::{Command, Env};

use crate::{helpers, Result};

#[test]
fn no_new_tty_allocation_when_absent() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("su")
        .args(["-c", "ps aux"])
        .output(&env)?
        .stdout()?;

    let entries = helpers::parse_ps_aux(&stdout);

    dbg!(&entries);

    let su = entries
        .iter()
        .find(|entry| entry.command == "su -c ps aux")
        .expect("`su` process not found");

    assert!(!su.has_tty());

    assert!(su.is_session_leader());

    let command = entries
        .iter()
        .find(|entry| entry.command == "ps aux")
        .expect("`ps aux` process not found");

    assert!(!command.has_tty());

    assert!(!command.is_session_leader());

    Ok(())
}

#[test]
#[ignore = "gh587"]
fn when_present_tty_is_allocated() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("su")
        .args(["--pty", "-c", "ps aux"])
        .output(&env)?
        .stdout()?;

    let entries = helpers::parse_ps_aux(&stdout);

    dbg!(&entries);

    let su = entries
        .iter()
        .find(|entry| entry.command.starts_with("su --pty -c"))
        .expect("`su` process not found");

    assert!(!su.has_tty());
    assert!(su.is_session_leader());

    let command = entries
        .iter()
        .find(|entry| entry.command == "ps aux")
        .expect("`ps aux` process not found");

    assert!(command.has_tty());
    assert!(command.is_session_leader());
    assert!(command.is_in_the_foreground_process_group());

    Ok(())
}

#[test]
fn existing_tty_is_shared_when_absent() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("su")
        .args(["-c", "ps aux"])
        .tty(true)
        .output(&env)?
        .stdout()?;

    let entries = helpers::parse_ps_aux(&stdout);

    dbg!(&entries);

    let su = entries
        .iter()
        .find(|entry| entry.command == "su -c ps aux")
        .expect("`su` process not found");

    assert!(su.has_tty());
    assert!(su.is_session_leader());

    let command = entries
        .iter()
        .find(|entry| entry.command == "ps aux")
        .expect("`ps aux` process not found");

    assert_eq!(su.tty, command.tty);
    assert!(!command.is_session_leader());

    Ok(())
}
