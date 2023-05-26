use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD};

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
        .exec(&env)?
        .stdout()?;

    child.wait()?.assert_success()?;

    let entries = parse_ps_aux(&ps_aux);

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

fn parse_ps_aux(ps_aux: &str) -> Vec<PsAuxEntry> {
    let mut entries = vec![];
    for line in ps_aux.lines().skip(1 /* header */) {
        let columns = line.split_ascii_whitespace().collect::<Vec<_>>();

        let entry = PsAuxEntry {
            command: columns[10..].join(" "),
            pid: columns[1].parse().expect("invalid PID"),
            process_state: columns[7].to_owned(),
            tty: columns[6].to_owned(),
        };

        entries.push(entry);
    }

    entries
}

#[derive(Debug)]
struct PsAuxEntry {
    command: String,
    pid: u32,
    process_state: String,
    tty: String,
}
