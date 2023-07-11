mod cli;
mod help;

use std::{
    fs::{File, Permissions},
    io::{self, Read, Seek, Write},
    os::unix::prelude::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{sudoers::Sudoers, system::file::Lockable};

use self::cli::VisudoOptions;
use self::help::{long_help_message, USAGE_MSG};

const VERSION: &str = env!("CARGO_PKG_VERSION");

macro_rules! io_msg {
    ($err:expr, $($tt:tt)*) => {
        io::Error::new($err.kind(), format!("{}: {}", format_args!($($tt)*), $err))
    };
}

pub fn main() {
    let options = match VisudoOptions::from_env() {
        Ok(options) => options,
        Err(error) => {
            println!("visudo: {error}\n{USAGE_MSG}");
            std::process::exit(1);
        }
    };

    match options.action {
        cli::VisudoAction::Help => {
            println!("{}", long_help_message());
            std::process::exit(0);
        }
        cli::VisudoAction::Version => {
            println!("visudo version {VERSION}");
            std::process::exit(0);
        }
        cli::VisudoAction::Check => {
            eprintln!("check is unimplemented");
            std::process::exit(1);
        }
        cli::VisudoAction::Run => match run_visudo(options.file.as_deref()) {
            Ok(()) => {}
            Err(error) => {
                eprintln!("visudo: {error}");
                std::process::exit(1);
            }
        },
    }
}

fn run_visudo(file_arg: Option<&str>) -> io::Result<()> {
    let sudoers_path = Path::new(file_arg.unwrap_or("/etc/sudoers"));

    let (mut sudoers_file, existed) = if sudoers_path.exists() {
        let file = File::options().read(true).write(true).open(sudoers_path)?;
        (file, true)
    } else {
        // Create a sudoers file if it doesn't exist.
        let file = File::create(sudoers_path)?;
        // ogvisudo sets the permissions of the file based on whether the `-f` argument was passed
        // or not:
        // - If `-f` was passed, the file can be read and written by the user.
        // - If `-f` was not passed, the file can only be read by the user.
        // In both cases, the file can be read by the group.
        let mode = if file_arg.is_some() { 0o640 } else { 0o440 };
        file.set_permissions(Permissions::from_mode(mode))?;
        (file, false)
    };

    sudoers_file.lock_exclusive(true).map_err(|err| {
        if err.kind() == io::ErrorKind::WouldBlock {
            io_msg!(err, "{} busy, try again later", sudoers_path.display())
        } else {
            err
        }
    })?;

    let result: io::Result<()> = (|| {
        let tmp_path = sudoers_path.with_extension("tmp");

        let mut tmp_file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(&tmp_path)?;
        tmp_file.set_permissions(Permissions::from_mode(0o700))?;

        let mut sudoers_contents = Vec::new();
        if existed {
            // If the sudoers file existed, read its contents and write them into the temporary file.
            sudoers_file.read_to_end(&mut sudoers_contents)?;
            // Rewind the sudoers file so it can be written later.
            sudoers_file.rewind()?;
            // Write to the temporary file.
            tmp_file.write_all(&sudoers_contents)?;
        }

        let editor_path = solve_editor_path()?;

        loop {
            Command::new(&editor_path)
                .arg("--")
                .arg(&tmp_path)
                .spawn()?
                .wait_with_output()?;

            let (_sudoers, errors) =
                File::open(&tmp_path)
                    .and_then(Sudoers::read)
                    .map_err(|err| {
                        io_msg!(
                            err,
                            "unable to re-open temporary file ({}), {} unchanged",
                            tmp_path.display(),
                            sudoers_path.display()
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
