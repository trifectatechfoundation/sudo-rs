use sudo_test::{Command, Env, TextFile};

use crate::{Result, USERNAME};

const SIGTERM: u8 = 15;
const SIGNAL_OFFSET: u8 = 128;

// TODO `--pty` variants
#[test]
#[ignore = "gh602"]
fn sigterms_child_on_sigterm_when_no_tty() -> Result<()> {
    sigterms_child_on_sigterm(false, false)
}

#[test]
#[ignore = "gh602"]
fn sigterms_child_on_sigterm_when_preexisting_tty() -> Result<()> {
    sigterms_child_on_sigterm(true, false)
}

fn sigterms_child_on_sigterm(tty: bool, flag_pty: bool) -> Result<()> {
    let script_path = "/tmp/script.sh";
    let env = Env("")
        .file(
            script_path,
            TextFile(include_str!("exit-on-sigterm.sh")).chmod("777"),
        )
        .user(USERNAME)
        .build()?;

    let mut su = Command::new("su");
    if flag_pty {
        su.arg("--pty");
    }
    let child = su.args([USERNAME, script_path]).tty(tty).spawn(&env)?;

    Command::new("sh")
        .arg("-c")
        .arg(format!("sleep 1; kill -{SIGTERM} $(pidof su)"))
        .output(&env)?
        .assert_success()?;

    let output = child.wait()?;
    let stdout = output.unchecked_stdout();
    eprintln!("\n--- STDOUT ---\n{stdout}\n--- STDOUT ---\n");

    assert!(!output.status().success());
    assert_eq!(
        Some(i32::from(SIGNAL_OFFSET + SIGTERM)),
        output.status().code()
    );

    assert_contains!(stdout, "received SIGTERM");

    if sudo_test::is_original_sudo() {
        let stream = if tty { stdout } else { output.stderr() };
        assert_contains!(stream, "Session terminated, killing shell...");
        assert_contains!(stream, "...killed");
    }

    Ok(())
}

#[test]
#[ignore = "gh603"]
fn escalates_to_sigkill_when_sigterm_is_ignored_when_no_tty() -> Result<()> {
    escalates_to_sigkill_when_sigterm_is_ignored(false, false)
}

#[test]
#[ignore = "gh603"]
fn escalates_to_sigkill_when_sigterm_is_ignored_when_preexisting_tty() -> Result<()> {
    escalates_to_sigkill_when_sigterm_is_ignored(true, false)
}
fn escalates_to_sigkill_when_sigterm_is_ignored(tty: bool, flag_pty: bool) -> Result<()> {
    let script_path = "/tmp/script.sh";
    let env = Env("")
        .file(
            script_path,
            TextFile(include_str!("ignore-sigterm.sh")).chmod("777"),
        )
        .user(USERNAME)
        .build()?;

    let mut su = Command::new("su");
    if flag_pty {
        su.arg("--pty");
    }
    let child = su.args([USERNAME, script_path]).tty(tty).spawn(&env)?;

    Command::new("sh")
        .arg("-c")
        .arg(format!("sleep 1; kill -{SIGTERM} $(pidof su)"))
        .output(&env)?
        .assert_success()?;

    let output = child.wait()?;
    let stdout = output.unchecked_stdout();
    eprintln!("\n--- STDOUT ---\n{stdout}\n--- STDOUT ---\n");

    assert!(!output.status().success(), "{stdout}");
    assert_eq!(
        Some(i32::from(SIGNAL_OFFSET + SIGTERM)),
        output.status().code()
    );

    let received_sigterm = "received SIGTERM";
    assert_contains!(stdout, received_sigterm);
    assert_not_contains!(stdout, "timeout");

    // it's not possible to `trap` SIGKILL so as a way to sanity check that the shell continued
    // executing after SIGTERM,  we check that there is at least one number printed by the for loop
    // after 'received SIGTERM'
    let (_, after_sigterm) = stdout.split_once(received_sigterm).unwrap();
    let numbers = after_sigterm
        .trim()
        .lines()
        .filter_map(|line| line.parse::<u8>().ok())
        .collect::<Vec<_>>();
    dbg!(&numbers);
    assert!(!numbers.is_empty());

    // SIGKILL is to be sent 2 seconds after SIGTERM so the loop can run one or twice before SIGKILL
    // arrives. look at the printed numbers to confirm that; allow up to 3 iterations (tolerance)
    assert!((1..=3).contains(&numbers.len()));

    if sudo_test::is_original_sudo() {
        let stream = if tty { stdout } else { output.stderr() };
        assert_contains!(stream, "Session terminated, killing shell...");
        assert_contains!(stream, "...killed");
    }

    Ok(())
}
