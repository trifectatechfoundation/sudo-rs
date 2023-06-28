use sudo_test::{Command, Env};

use crate::{
    helpers::{self, PsAuxEntry},
    Result,
};

#[test]
fn no_new_tty_allocation_when_absent() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("su")
        .args(["-c", "ps aux"])
        .output(&env)?
        .stdout()?;

    let entries = helpers::parse_ps_aux(&stdout);

    dbg!(&entries);

    let su = exactly_one_match("su", &entries, |entry| entry.command == "su -c ps aux");

    assert!(!su.has_tty());
    assert!(su.is_session_leader());
    assert!(!su.is_in_the_foreground_process_group());

    let command = exactly_one_match("ps aux", &entries, |entry| entry.command == "ps aux");

    assert!(!command.has_tty());
    assert!(!command.is_session_leader());
    assert!(!command.is_in_the_foreground_process_group());

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

    let su = exactly_one_match("su", &entries, |entry| {
        entry.command.starts_with("su --pty -c")
    });

    assert!(!su.has_tty());
    assert!(su.is_session_leader());
    assert!(!su.is_in_the_foreground_process_group());

    let command = exactly_one_match("ps aux", &entries, |entry| entry.command == "ps aux");

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

    let su = exactly_one_match("su", &entries, |entry| entry.command == "su -c ps aux");

    assert!(su.has_tty());
    assert!(su.is_session_leader());
    assert!(su.is_in_the_foreground_process_group());

    let command = exactly_one_match("ps aux", &entries, |entry| entry.command == "ps aux");

    assert_eq!(su.tty, command.tty);
    assert!(!command.is_session_leader());
    assert!(command.is_in_the_foreground_process_group());

    Ok(())
}

#[test]
#[ignore = "gh592"]
fn when_present_a_new_tty_is_allocated_for_exclusive_use_of_the_child() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("su")
        .args(["--pty", "-c", "ps aux"])
        .tty(true)
        .output(&env)?
        .stdout()?;

    let entries = helpers::parse_ps_aux(&stdout);

    dbg!(&entries);

    let su = exactly_one_match("su", &entries, |entry| {
        entry.command.starts_with("su --pty -c")
    });

    assert!(su.has_tty());
    assert!(su.is_session_leader());
    assert!(su.is_in_the_foreground_process_group());

    let command = exactly_one_match("ps aux", &entries, |entry| entry.command == "ps aux");

    assert!(command.has_tty());
    assert_ne!(su.tty, command.tty);
    assert!(command.is_session_leader());
    assert!(command.is_in_the_foreground_process_group());

    Ok(())
}

fn exactly_one_match<'a>(
    process_name: &str,
    entries: &'a [PsAuxEntry],
    cond: impl FnMut(&&PsAuxEntry) -> bool,
) -> &'a PsAuxEntry {
    let matches = entries.iter().filter(cond).collect::<Vec<_>>();

    assert!(
        !matches.is_empty(),
        "process `{process_name}` was not found"
    );
    assert_eq!(
        1,
        matches.len(),
        "found more than one `{process_name}` process"
    );

    matches[0]
}
