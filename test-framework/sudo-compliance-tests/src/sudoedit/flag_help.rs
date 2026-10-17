use sudo_test::{Command, Env};

use crate::{PANIC_EXIT_CODE, Result};

#[test]
fn does_not_panic_on_io_errors() -> Result<()> {
    let env = Env("").build();

    let output = Command::new("bash")
        .args([
            "-c",
            "sudoedit --help 2>&1 | true; echo \"${PIPESTATUS[0]}\"",
        ])
        .output(&env);

    let exit_code = output.stdout().parse()?;
    assert_ne!(PANIC_EXIT_CODE, exit_code);
    assert_eq!(0, exit_code);

    Ok(())
}

#[test]
fn no_args_gives_help() -> Result<()> {
    let env = Env("").build();

    let output = Command::new("sudoedit").output(&env);

    let output = output.stderr();
    assert_contains!(output, "usage: sudoedit");

    Ok(())
}

#[test]
fn prints_on_stdout() -> Result<()> {
    let env = Env("").build();

    let output = Command::new("sudoedit").args(["--help"]).output(&env);

    let output = output.stdout();
    assert_starts_with!(output, "sudoedit - edit files as another user");

    Ok(())
}
