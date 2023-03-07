use std::{
    io::{Seek, Write},
    process::{Command, Stdio},
};

use crate::{docker::ExecOutput, Result};

pub fn run(cmd: &mut Command, stdin: Option<&str>) -> Result<ExecOutput> {
    let mut temp_file;
    if let Some(stdin) = stdin {
        temp_file = tempfile::tempfile()?;
        temp_file.write_all(stdin.as_bytes())?;
        temp_file.seek(std::io::SeekFrom::Start(0))?;
        cmd.stdin(Stdio::from(temp_file));
    }

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

pub fn stdout(cmd: &mut Command, stdin: Option<&str>) -> Result<String> {
    let output = run(cmd, stdin)?;

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
