use sudo_test::{Command, Env, TextFile};

use crate::Result;

#[test]
fn it_works() -> Result<()> {
    let env = Env("").build()?;

    Command::new("su")
        .args(["-c", "true"])
        .output(&env)?
        .assert_success()?;

    let output = Command::new("su").args(["-c", "false"]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    Ok(())
}

#[test]
fn pass_to_shell_via_c_flag() -> Result<()> {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env("")
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build()?;

    let command = "command";
    let output = Command::new("su")
        .args(["-s", shell_path, "-c", command])
        .output(&env)?
        .stdout()?;

    assert_eq!(format!("-c {command}"), output);

    Ok(())
}
