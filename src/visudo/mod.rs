use std::{
    fs::{File, Permissions},
    io::{self, BufRead, IsTerminal, Read, Seek, Write},
    os::unix::prelude::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{sudoers::Sudoers, system::file::Lockable};

pub fn main() {
    match visudo_process() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("visudo: {error}");
            std::process::exit(1);
        }
    }
}

fn visudo_process() -> io::Result<()> {
    let sudoers_path = Path::new("/etc/sudoers");

    let (mut sudoers_file, existed) = if sudoers_path.exists() {
        let file = File::options().read(true).write(true).open(sudoers_path)?;
        (file, true)
    } else {
        // Create a sudoers file if it doesn't exist and set the permissions so it can only be read
        // by the root user and group.
        let file = File::create(sudoers_path)?;
        file.set_permissions(Permissions::from_mode(0o440))?;
        (file, false)
    };

    sudoers_file.lock_exclusive(true).map_err(|err| {
        if err.kind() == io::ErrorKind::WouldBlock {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                format!("{} busy, try again later", sudoers_path.display()),
            )
        } else {
            err
        }
    })?;

    let result: io::Result<()> = (|| {
        let tmp_path = sudoers_path.with_extension("tmp");

        let mut tmp_file = File::create(&tmp_path)?;
        tmp_file.set_permissions(Permissions::from_mode(0o700))?;

        let mut sudoers_contents = Vec::new();
        if existed {
            // If the sudoers file existed, read its contents and write them into the temporary file.
            sudoers_file.read_to_end(&mut sudoers_contents)?;
            // Rewind the sudoers file so it can be written later.
            sudoers_file.rewind()?;
            tmp_file.write_all(&sudoers_contents)?;
        }

        let editor_path = solve_editor_path()?;

        loop {
            Command::new(&editor_path)
                .arg("--")
                .arg(&tmp_path)
                .spawn()?
                .wait_with_output()?;

            let (_sudoers, errors) = Sudoers::new(&tmp_path).map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!(
                        "unable to re-open temporary file ({}), {} unchanged",
                        tmp_path.display(),
                        sudoers_path.display()
                    ),
                )
            })?;

            if errors.is_empty() {
                break;
            }

            eprintln!("Come on... you can do better than that.\n");

            for crate::sudoers::Error(_position, message) in errors {
                eprintln!("syntax error: {message}");
            }

            eprintln!();

            let stdin = io::stdin();
            let stdout = io::stdout();

            let mut stdin_handle = stdin.lock();
            let mut stdout_handle = stdout.lock();

            loop {
                stdout_handle
                    .write_all("What now? e(x)it without saving / (e)dit again: ".as_bytes())?;
                stdout_handle.flush()?;

                let mut input = [0u8];
                if let Err(err) = stdin_handle.read_exact(&mut input) {
                    eprintln!("visudo: cannot read user input: {err}");
                    return Ok(());
                }

                match &input {
                    b"e" => break,
                    b"x" => return Ok(()),
                    input => println!("Invalid option: {:?}\n", std::str::from_utf8(input)),
                }
            }
        }

        let tmp_contents = std::fs::read(&tmp_path)?;
        // Only write to the sudoers file if the contents changed.
        if tmp_contents == sudoers_contents {
            eprintln!("visudo: {} unchanged", tmp_path.display());
        } else {
            sudoers_file.write_all(&tmp_contents)?;
        }

        Ok(())
    })();

    sudoers_file.unlock()?;

    result?;

    Ok(())
}

fn solve_editor_path() -> io::Result<PathBuf> {
    let path = Path::new("/usr/bin/editor");
    if path.exists() {
        return Ok(path.to_owned());
    }

    for key in ["SUDO_EDITOR", "VISUAL", "EDITOR"] {
        if let Some(var) = std::env::var_os(key) {
            let path = Path::new(&var);
            if path.exists() {
                return Ok(path.to_owned());
            }
        }
    }

    let path = Path::new("/usr/bin/vi");
    if path.exists() {
        return Ok(path.to_owned());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "cannot find text editor",
    ))
}
