use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, Read, Seek, Write},
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
    let mut sudoers_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(sudoers_path)?;

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

        let mut buf = Vec::new();
        sudoers_file.read_to_end(&mut buf)?;

        File::create(&tmp_path)?.write_all(&buf)?;

        let editor_path = solve_editor_path()?;

        loop {
            Command::new(&editor_path)
                .arg(&tmp_path)
                .spawn()?
                .wait_with_output()?;

            let (_sudoers, errors) = Sudoers::new(&tmp_path)?;

            if errors.is_empty() {
                break;
            }

            println!("Come on... you can do better than that.\n");

            for crate::sudoers::Error(_position, message) in errors {
                println!("\t{message}");
            }

            println!();

            let stdin = io::stdin();
            let stdout = io::stdout();

            let mut stdin_handle = stdin.lock();
            let mut stdout_handle = stdout.lock();

            loop {
                stdout_handle
                    .write_all("What now? e(x)it without saving / (e)dit again: ".as_bytes())?;
                stdout_handle.flush()?;

                let mut input = String::new();
                stdin_handle.read_line(&mut input)?;

                match input.trim_end() {
                    "e" => break,
                    "x" => return Ok(()),
                    input => println!("Invalid option: {input:?}\n"),
                }
            }
        }

        sudoers_file.rewind()?;

        let buf = std::fs::read(&tmp_path)?;
        sudoers_file.write_all(&buf)?;

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
