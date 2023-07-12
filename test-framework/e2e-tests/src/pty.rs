use core::fmt;

use sudo_test::{
    helpers::{self, PsAuxEntry},
    Command, Env,
};

use crate::Result;

enum Binary {
    Sudo,
    Su,
}

enum ExecMode {
    ExecPty,
    ExecNoPty,
}

impl fmt::Display for Binary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Binary::Sudo => "sudo",
            Binary::Su => "su",
        };
        f.write_str(s)
    }
}

fn do_test(binary: Binary, not_use_pty: bool, user_tty: bool, expected: ExecMode) -> Result<()> {
    let env = Env([
        "ALL ALL=(ALL:ALL) ALL",
        if not_use_pty { "Defaults !use_pty" } else { "" },
    ])
    .build()?;

    let mut cmd = match binary {
        Binary::Su => {
            let mut cmd = Command::new("su");
            cmd.args(["-c", "sh -c 'touch /tmp/barrier; sleep 3'"]);
            cmd
        }

        Binary::Sudo => {
            let mut cmd = Command::new("sudo");
            cmd.args(["sh", "-c", "touch /tmp/barrier; sleep 3"]);
            cmd
        }
    };

    let child = cmd.tty(user_tty).spawn(&env)?;

    let ps_aux = Command::new("sh")
        .args([
            "-c",
            "until [ -f /tmp/barrier ]; do sleep 0.1; done; ps aux",
        ])
        .output(&env)?
        .stdout()?;

    child.wait()?.assert_success()?;

    let entries = helpers::parse_ps_aux(&ps_aux);

    let mut binary_related_processes = entries
        .into_iter()
        .filter(|entry| entry.command.contains("touch"))
        .collect::<Vec<_>>();

    binary_related_processes.sort_by_key(|entry| entry.pid);

    let prefix = format!("{binary} ");
    match expected {
        ExecMode::ExecPty => {
            let [original, monitor, command]: [PsAuxEntry; 3] = binary_related_processes
                .try_into()
                .map_err(|_| format!("expected 3 {binary}-related processes"))?;

            dbg!(&original, &monitor, &command);

            // sanity checks
            assert!(original.command.starts_with(&prefix));
            assert!(monitor.command.starts_with(&prefix));
            assert!(!command.command.starts_with(&prefix));

            assert!(original.has_tty());
            assert!(monitor.has_tty());
            assert!(command.has_tty());

            // actual checks
            assert_eq!(monitor.tty, command.tty);
            assert_ne!(original.tty, monitor.tty);

            assert!(original.is_in_the_foreground_process_group());
            assert!(command.is_in_the_foreground_process_group());

            assert!(original.is_session_leader());
            assert!(monitor.is_session_leader());
        }

        ExecMode::ExecNoPty => {
            let [original, command]: [PsAuxEntry; 2] = binary_related_processes
                .try_into()
                .map_err(|_| format!("expected 2 {binary}-related processes"))?;

            dbg!(&original, &command);

            // sanity checks
            assert!(original.command.starts_with(&prefix));
            assert!(!command.command.starts_with(&prefix));

            // actual checks
            assert_eq!(user_tty, original.has_tty());
            assert_eq!(user_tty, command.has_tty());
        }
    }

    Ok(())
}

#[test]
fn su_uses_exec_pty_by_default_when_user_tty_exists() -> Result<()> {
    do_test(Binary::Su, false, true, ExecMode::ExecPty)
}

#[test]
fn sudo_uses_exec_pty_by_default_when_user_tty_exists() -> Result<()> {
    do_test(Binary::Sudo, false, true, ExecMode::ExecPty)
}

#[test]
fn su_uses_exec_no_pty_when_user_tty_does_not_exist() -> Result<()> {
    do_test(Binary::Su, false, false, ExecMode::ExecNoPty)
}

#[test]
fn sudo_uses_exec_no_pty_when_user_tty_does_not_exist() -> Result<()> {
    do_test(Binary::Sudo, false, false, ExecMode::ExecNoPty)
}

// no `su` test here because there's no way to disable `--pty`
#[test]
fn sudo_uses_exec_no_pty_when_user_tty_exists_and_sudoers_says_so() -> Result<()> {
    do_test(Binary::Sudo, true, true, ExecMode::ExecNoPty)
}
