use sudo_test::{
    helpers::{self, PsAuxEntry, PRINT_PTY_OWNER},
    Command, Env,
};

use crate::{OTHER_USERNAME, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

#[derive(Debug)]
struct Processes {
    original: PsAuxEntry,
    monitor: PsAuxEntry,
    command: PsAuxEntry,
}

fn fixture() -> Processes {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"]).build();

    let child = Command::new("sudo")
        .args(["sh", "-c", "touch /tmp/barrier; sleep 3; true"])
        .tty(true)
        .spawn(&env);

    let ps_aux = Command::new("sh")
        .args([
            "-c",
            "until [ -f /tmp/barrier ]; do sleep 0.1; done; ps aux",
        ])
        .output(&env)
        .stdout();

    child.wait().assert_success();

    let entries = helpers::parse_ps_aux(&ps_aux);

    let mut sudo_related_processes = entries
        .into_iter()
        .filter(|entry| entry.command.contains("sh -c touch"))
        .collect::<Vec<_>>();

    sudo_related_processes.sort_by_key(|entry| entry.pid);

    let [original, monitor, command]: [PsAuxEntry; 3] = sudo_related_processes
        .try_into()
        .expect("expected 3 sudo-related processes");

    // sanity check
    let prefix = "sudo ";
    assert!(original.command.starts_with(prefix));
    assert!(monitor.command.starts_with(prefix));
    assert!(!command.command.starts_with(prefix));

    assert!(original.has_tty());
    assert!(monitor.has_tty());
    assert!(command.has_tty());

    Processes {
        original,
        monitor,
        command,
    }
}

#[test]
fn spawns_three_processes() {
    let _ = fixture();
}

#[test]
fn allocates_a_second_pty_which_is_assigned_to_the_command_process() {
    let Processes {
        original,
        monitor,
        command,
    } = fixture();

    assert_eq!(monitor.tty, command.tty);
    assert_ne!(original.tty, monitor.tty);
}

#[test]
fn process_state() {
    let Processes {
        original,
        monitor,
        command,
    } = fixture();

    assert!(original.is_in_the_foreground_process_group());
    assert!(command.is_in_the_foreground_process_group());

    assert!(original.is_session_leader());
    assert!(monitor.is_session_leader());
}

#[test]
fn terminal_is_restored() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"]).build();
    // Run `stty` before and after running sudo to check that the terminal configuration is
    // restored before sudo exits.
    let stdout = Command::new("sh")
        .args(["-c", "stty; sudo echo 'hello'; stty"])
        .tty(true)
        .output(&env)
        .stdout();

    assert_contains!(stdout, "hello");
    let (before, after) = stdout.split_once("hello").unwrap();
    assert_eq!(before.trim(), after.trim());
}

#[test]
fn pty_owner() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"])
        .user(USERNAME)
        .user(OTHER_USERNAME)
        .build();

    let stdout = Command::new("sudo")
        .as_user(USERNAME)
        .args(["-u", OTHER_USERNAME, "sh", "-c", PRINT_PTY_OWNER])
        .tty(true)
        .output(&env)
        .stdout();

    assert_eq!(stdout.trim(), format!("{OTHER_USERNAME} tty"));
}

#[test]
fn stdin_pipe() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"]).build();

    let stdout = Command::new("sh")
        .args(["-c", "echo 'hello world' | sudo grep -o hello"])
        .tty(true)
        .output(&env)
        .stdout();

    assert_eq!(stdout.trim(), "hello");
}

#[test]
fn stdout_pipe() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"]).build();

    let stdout = Command::new("sh")
        .args(["-c", "sudo echo 'hello world' | grep -o hello"])
        .tty(true)
        .output(&env)
        .stdout();

    assert_eq!(stdout.trim(), "hello");
}

#[test]
fn stderr_pipe() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"]).build();

    let output = Command::new("sh")
        .args([
            "-c",
            "2>/tmp/stderr.txt sudo sh -c '>&2 echo \"hello world\"'",
        ])
        .tty(true)
        .output(&env);

    assert!(output.stderr().is_empty());

    let stdout = Command::new("cat")
        .arg("/tmp/stderr.txt")
        .output(&env)
        .stdout();

    assert_eq!(stdout, "hello world");
}

#[test]
fn stdout_foreign_pty() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"]).build();

    // Everything is put in a single command with separators to keep the pts numbers predictable
    let output = Command::new("sh")
        .args([
            "-c",
            "socat - SYSTEM:'ls -l /proc/self/fd; echo @@@@; sudo ls -l /proc/self/fd',pty,setsid,ctty;
            echo ====;
            socat - SYSTEM:'ls -l /proc/self/fd; echo @@@@; sudo ls -l /proc/self/fd',pty",
        ])
        .tty(true)
        .output(&env);

    let stdout = output.stdout();
    let (own_term, foreign_term) = stdout.split_once("====").unwrap();

    let (own_term_in, own_term_sudo) = own_term.split_once("@@@@").unwrap();
    assert_contains!(own_term_in, " 0 -> /dev/pts/1");
    assert_contains!(own_term_in, " 1 -> /dev/pts/1");
    assert_contains!(own_term_in, " 2 -> /dev/pts/0");
    // pts/1 is our controlling tty, so it gets proxied.
    // pts/0 is a foreign pty, so it gets inherited
    assert_contains!(own_term_sudo, " 0 -> /dev/pts/2");
    assert_contains!(own_term_sudo, " 1 -> /dev/pts/2");
    assert_contains!(own_term_sudo, " 2 -> /dev/pts/0");

    let (foreign_term_in, foreign_term_sudo) = foreign_term.split_once("@@@@").unwrap();
    assert_contains!(foreign_term_in, " 0 -> /dev/pts/1");
    assert_contains!(foreign_term_in, " 1 -> /dev/pts/1");
    assert_contains!(foreign_term_in, " 2 -> /dev/pts/0");
    // pts/1 is not our controlling tty, so it gets inherited.
    // pts/0 is our controlling tty, so it gets proxied
    assert_contains!(foreign_term_sudo, " 0 -> /dev/pts/1");
    assert_contains!(foreign_term_sudo, " 1 -> /dev/pts/1");
    assert_contains!(foreign_term_sudo, " 2 -> /dev/pts/2");
}

#[test]
fn stdout_pipe_tty() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults use_pty"]).build();

    let output = Command::new("sh")
        .args([
            "-c",
            "echo -n 'hello world' | socat -d0 STDIO SYSTEM:'sudo cat /dev/tty | cat',pty",
        ])
        .tty(true)
        .output(&env);

    assert_eq!(output.stdout(), "hello world");
}
