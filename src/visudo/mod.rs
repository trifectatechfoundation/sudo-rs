#![forbid(unsafe_code)]

mod cli;
mod help;

use std::{
    env, ffi,
    fs::{File, Permissions},
    io::{self, Read, Seek, Write},
    os::unix::prelude::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    str,
};

use crate::{
    common::resolve::CurrentUser,
    sudo::{candidate_sudoers_file, diagnostic},
    sudoers::{self, Sudoers},
    system::{
        file::{create_temporary_dir, Chown, FileLock},
        interface::{GroupId, UserId},
        signal::{consts::*, register_handlers, SignalStream},
        Hostname, User,
    },
};

use self::cli::{VisudoAction, VisudoOptions};
use self::help::{long_help_message, USAGE_MSG};

const VERSION: &str = env!("CARGO_PKG_VERSION");

macro_rules! io_msg {
    ($err:expr, $($tt:tt)*) => {
        io::Error::new($err.kind(), format!("{}: {}", format_args!($($tt)*), $err))
    };
}

pub fn main() {
    if User::effective_uid() != User::real_uid() || User::effective_gid() != User::real_gid() {
        println_ignore_io_error!(
            "Visudo must not be installed as setuid binary.\n\
             Please notify your packager about this misconfiguration.\n\
             To prevent privilege escalation visudo will now abort.
             "
        );
        std::process::exit(1);
    }

    let options = match VisudoOptions::from_env() {
        Ok(options) => options,
        Err(error) => {
            println_ignore_io_error!("visudo: {error}\n{USAGE_MSG}");
            std::process::exit(1);
        }
    };

    let cmd = match options.action {
        VisudoAction::Help => {
            println_ignore_io_error!("{}", long_help_message());
            std::process::exit(0);
        }
        VisudoAction::Version => {
            println_ignore_io_error!("visudo version {VERSION}");
            std::process::exit(0);
        }
        VisudoAction::Check => check,
        VisudoAction::Run => run,
    };

    match cmd(options.file.as_deref(), options.perms, options.owner) {
        Ok(()) => {}
        Err(error) => {
            eprintln_ignore_io_error!("visudo: {error}");
            std::process::exit(1);
        }
    }
}

fn check(file_arg: Option<&str>, perms: bool, owner: bool) -> io::Result<()> {
    let sudoers_path = &file_arg
        .map(PathBuf::from)
        .unwrap_or_else(candidate_sudoers_file);

    let sudoers_file = File::open(sudoers_path)
        .map_err(|err| io_msg!(err, "unable to open {}", sudoers_path.display()))?;

    let metadata = sudoers_file.metadata()?;

    if file_arg.is_none() || perms {
        // For some reason, the MSB of the mode is on so we need to mask it.
        let mode = metadata.permissions().mode() & 0o777;

        if mode != 0o440 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "{}: bad permissions, should be mode 0440, but found {mode:04o}",
                    sudoers_path.display()
                ),
            ));
        }
    }

    if file_arg.is_none() || owner {
        let owner = (metadata.uid(), metadata.gid());

        if owner != (0, 0) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "{}: wrong owner (uid, gid) should be (0, 0), but found {owner:?}",
                    sudoers_path.display()
                ),
            ));
        }
    }

    let (_sudoers, errors) = Sudoers::read(&sudoers_file, sudoers_path)?;

    if errors.is_empty() {
        writeln!(io::stdout(), "{}: parsed OK", sudoers_path.display())?;
        return Ok(());
    }

    for crate::sudoers::Error {
        message,
        source,
        location,
    } in errors
    {
        let path = source.as_deref().unwrap_or(sudoers_path);
        diagnostic::diagnostic!("syntax error: {message}", path @ location);
    }

    Err(io::Error::new(io::ErrorKind::Other, "invalid sudoers file"))
}

fn run(file_arg: Option<&str>, perms: bool, owner: bool) -> io::Result<()> {
    let sudoers_path = &file_arg
        .map(PathBuf::from)
        .unwrap_or_else(candidate_sudoers_file);

    let (sudoers_file, existed) = if sudoers_path.exists() {
        let file = File::options()
            .read(true)
            .write(true)
            .open(sudoers_path)
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!("Failed to open existing sudoers file at {sudoers_path:?}: {e}"),
                )
            })?;

        (file, true)
    } else {
        // Create a sudoers file if it doesn't exist.
        let file = File::create(sudoers_path).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("Failed to create sudoers file at {sudoers_path:?}: {e}"),
            )
        })?;
        // ogvisudo sets the permissions of the file so it can be read and written by the user and
        // read by the group if the `-f` argument was passed.
        if file_arg.is_some() {
            file.set_permissions(Permissions::from_mode(0o640))
                .map_err(|e| {
                    io::Error::new(
                        e.kind(),
                        format!(
                            "Failed to set permissions on new sudoers file at {sudoers_path:?}: {e}"
                        ),
                    )
                })?;
        }
        (file, false)
    };

    let lock = FileLock::exclusive(&sudoers_file, true).map_err(|err| {
        if err.kind() == io::ErrorKind::WouldBlock {
            io_msg!(err, "{} busy, try again later", sudoers_path.display())
        } else {
            err
        }
    })?;

    if perms || file_arg.is_none() {
        sudoers_file.set_permissions(Permissions::from_mode(0o440))?;
    }

    if owner || file_arg.is_none() {
        sudoers_file.chown(UserId::ROOT, GroupId::new(0))?;
    }

    let signal_stream = SignalStream::init()?;

    let handlers = register_handlers([SIGTERM, SIGHUP, SIGINT, SIGQUIT])?;

    let tmp_dir = create_temporary_dir()?;
    let tmp_path = tmp_dir.join("sudoers");

    {
        let tmp_dir = tmp_dir.clone();
        std::thread::spawn(|| -> io::Result<()> {
            signal_stream.recv()?;

            let _ = std::fs::remove_dir_all(tmp_dir);

            drop(handlers);

            std::process::exit(1)
        });
    }

    let tmp_file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp_path)?;

    tmp_file.set_permissions(Permissions::from_mode(0o600))?;

    let result = edit_sudoers_file(
        existed,
        sudoers_file,
        sudoers_path,
        lock,
        tmp_file,
        &tmp_path,
    );

    std::fs::remove_dir_all(tmp_dir)?;

    result
}

fn edit_sudoers_file(
    existed: bool,
    mut sudoers_file: File,
    sudoers_path: &Path,
    lock: FileLock,
    mut tmp_file: File,
    tmp_path: &Path,
) -> io::Result<()> {
    let mut stderr = io::stderr();

    let mut sudoers_contents = Vec::new();

    // Since visudo is meant to run as root, resolve shouldn't fail
    let current_user: User = match CurrentUser::resolve() {
        Ok(user) => user.into(),
        Err(err) => {
            writeln!(stderr, "visudo: cannot resolve : {err}")?;
            return Ok(());
        }
    };

    let host_name = Hostname::resolve();

    let editor_path = if existed {
        // If the sudoers file existed, read its contents and write them into the temporary file.
        sudoers_file.read_to_end(&mut sudoers_contents)?;
        // Rewind the sudoers file so it can be written later.
        sudoers_file.rewind()?;
        // Write to the temporary file.
        tmp_file.write_all(&sudoers_contents)?;

        let (sudoers, _errors) = Sudoers::read(sudoers_contents.as_slice(), sudoers_path)?;

        sudoers.visudo_editor_path(&host_name, &current_user, &current_user)
    } else {
        // there is no /etc/sudoers config yet, so use a system default
        PathBuf::from(crate::defaults::SYSTEM_EDITOR)
    };

    loop {
        Command::new(&editor_path)
            .arg("--")
            .arg(tmp_path)
            .spawn()
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "specified editor ({}) could not be used",
                        editor_path.display()
                    ),
                )
            })?
            .wait_with_output()?;

        let (sudoers, errors) = File::open(tmp_path)
            .and_then(|reader| Sudoers::read(reader, tmp_path))
            .map_err(|err| {
                io_msg!(
                    err,
                    "unable to re-open temporary file ({}), {} unchanged",
                    tmp_path.display(),
                    sudoers_path.display()
                )
            })?;

        if !errors.is_empty() {
            writeln!(stderr, "The provided sudoers file format is not recognized or contains syntax errors. Please review:\n")?;

            for crate::sudoers::Error {
                message,
                source,
                location,
            } in errors
            {
                let path = source.as_deref().unwrap_or(sudoers_path);
                diagnostic::diagnostic!("syntax error: {message}", path @ location);
            }

            writeln!(stderr)?;

            match ask_response(b"What now? e(x)it without saving / (e)dit again: ", b"xe")? {
                b'x' => return Ok(()),
                _ => continue,
            }
        } else {
            if sudo_visudo_is_allowed(sudoers, &host_name) == Some(false) {
                writeln!(
                    stderr,
                    "It looks like you have removed your ability to run 'sudo visudo' again.\n"
                )?;
                match ask_response(
                    b"What now? e(x)it without saving / (e)dit again / lock me out and (S)ave: ",
                    b"xeS",
                )? {
                    b'x' => return Ok(()),
                    b'S' => {}
                    _ => continue,
                }
            }

            break;
        }
    }

    let tmp_contents = std::fs::read(tmp_path)?;
    // Only write to the sudoers file if the contents changed.
    if tmp_contents == sudoers_contents {
        writeln!(stderr, "visudo: {} unchanged", tmp_path.display())?;
    } else {
        sudoers_file.write_all(&tmp_contents)?;
        let new_size = sudoers_file.stream_position()?;
        sudoers_file.set_len(new_size)?;
    }

    lock.unlock()?;

    Ok(())
}

// To detect potential lock-outs if the user called "sudo visudo".
// Note that SUDO_USER will normally be set by sudo.
//
// This returns Some(false) if visudo is forbidden under the given config;
// Some(true) if it is allowed; and None if it cannot be determined, which
// will be the case if e.g. visudo was simply run as root.
fn sudo_visudo_is_allowed(mut sudoers: Sudoers, host_name: &Hostname) -> Option<bool> {
    let sudo_user =
        User::from_name(&ffi::CString::new(env::var("SUDO_USER").ok()?).ok()?).ok()??;

    let super_user = User::from_uid(UserId::ROOT).ok()??;

    let request = sudoers::Request {
        user: &super_user,
        group: &super_user.primary_group().ok()?,
        command: &env::current_exe().ok()?,
        arguments: &[],
    };

    Some(matches!(
        sudoers
            .check(&sudo_user, host_name, request)
            .authorization(),
        sudoers::Authorization::Allowed { .. }
    ))
}

// Make sure that the first valid response is the "safest" choice
pub(crate) fn ask_response(prompt: &[u8], valid_responses: &[u8]) -> io::Result<u8> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stderr = io::stderr();

    let mut stdin_handle = stdin.lock();
    let mut stdout_handle = stdout.lock();

    loop {
        stdout_handle.write_all(prompt)?;
        stdout_handle.flush()?;

        let mut input = [0u8];
        if let Err(err) = stdin_handle.read_exact(&mut input) {
            writeln!(stderr, "visudo: cannot read user input: {err}")?;
            return Ok(valid_responses[0]);
        }

        // read the trailing newline
        loop {
            let mut skipped = [0u8];
            match stdin_handle.read_exact(&mut skipped) {
                Ok(()) if &skipped != b"\n" => continue,
                _ => break,
            }
        }

        if valid_responses.contains(&input[0]) {
            return Ok(input[0]);
        } else {
            writeln!(
                stderr,
                "Invalid option: '{}'\n",
                str::from_utf8(&input).unwrap_or("<INVALID>")
            )?;
        }
    }
}
