use sudo_test::{Command, Env};

use crate::{Result, PANIC_EXIT_CODE};

#[test]
fn does_not_panic_on_io_errors() -> Result<()> {
    let env = Env("").build()?;

    let output = Command::new("bash")
        .args([
            "-c",
            "sudo --version 2>&1 | true; echo \"${PIPESTATUS[0]}\"",
        ])
        .output(&env)?;

    let exit_code = output.stdout()?.parse()?;
    assert_ne!(PANIC_EXIT_CODE, exit_code);
    assert_eq!(0, exit_code);

    Ok(())
}
