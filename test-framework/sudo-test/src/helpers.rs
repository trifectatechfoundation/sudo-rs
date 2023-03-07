use std::process::Command;

use crate::{docker::ExecOutput, Result};

pub fn run(cmd: &mut Command) -> Result<ExecOutput> {
    let output = cmd.output()?;

    let mut stderr = String::from_utf8(output.stderr)?;
    let mut stdout = String::from_utf8(output.stdout)?;

    // it's a common pitfall to forget to remove the trailing '\n' so remove it here
    if stderr.ends_with('\n') {
        stderr.pop();
    }

    if stdout.ends_with('\n') {
        stdout.pop();
    }

    Ok(ExecOutput {
        status: output.status,
        stderr,
        stdout,
    })
}

pub fn stdout(cmd: &mut Command) -> Result<String> {
    let output = run(cmd)?;

    if !output.status.success() {
        let reason = if let Some(code) = output.status.code() {
            format!("exit code {code}")
        } else {
            "a non-zero exit code".to_string()
        };

        return Err(format!("`{cmd:?}` exited with {reason}. stderr:\n{}", output.stderr).into());
    }

    Ok(output.stdout)
}
