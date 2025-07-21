#![cfg_attr(not(feature = "sudoedit"), allow(dead_code))]
use std::ffi::{CStr, CString};
use std::fs::{DirBuilder, File, Metadata, OpenOptions};
use std::io::{self, Error, ErrorKind};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::{
    ffi::OsStrExt,
    fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    prelude::OpenOptionsExt,
};
use std::path::{Component, Path};

use super::{cerr, Group, User};

/// Temporary change privileges --- essentially a 'mini sudo'
/// This is only used for sudoedit.
fn sudo_call<T>(user: &User, group: &Group, operation: impl FnOnce() -> T) -> io::Result<T> {
    struct ResetUserGuard(libc::uid_t, libc::gid_t);

    impl Drop for ResetUserGuard {
        fn drop(&mut self) {
            forgetful_seteugid(self.0, self.1).expect("could not restore to saved user id");
        }
    }

    fn switch_user(euid: libc::uid_t, egid: libc::gid_t) -> io::Result<ResetUserGuard> {
        // SAFETY: these functions are always safe to call
        let guard = unsafe { ResetUserGuard(libc::geteuid(), libc::getegid()) };
        forgetful_seteugid(euid, egid)?;

        Ok(guard)
    }

    fn forgetful_seteugid(euid: libc::uid_t, egid: libc::gid_t) -> io::Result<()> {
        const KEEP_UID: libc::gid_t = -1i32 as libc::gid_t;
        const KEEP_GID: libc::uid_t = -1i32 as libc::uid_t;
        // SAFETY: this function is always safe to call
        cerr(unsafe { libc::setresgid(KEEP_UID, egid, KEEP_UID) })?;
        // SAFETY: this function is always safe to call
        cerr(unsafe { libc::setresuid(KEEP_GID, euid, KEEP_GID) })?;

        Ok(())
    }

    let guard = switch_user(user.uid.inner(), group.gid.inner())?;
    let result = operation();

    std::mem::drop(guard);
    Ok(result)
}

// of course we can also write "file & 0o040 != 0", but this makes the intent explicit
enum Op {
    Read = 4,
    Write = 2,
    Exec = 1,
}
enum Category {
    Owner = 2,
    Group = 1,
    World = 0,
}

fn mode(who: Category, what: Op) -> u32 {
    (what as u32) << (3 * who as u32)
}

/// Open sudo configuration using various security checks
pub fn secure_open_sudoers(path: impl AsRef<Path>, check_parent_dir: bool) -> io::Result<File> {
    let mut open_options = OpenOptions::new();
    open_options.read(true);

    secure_open_impl(path.as_ref(), &mut open_options, check_parent_dir, false)
}

/// Open a timestamp cookie file using various security checks
pub fn secure_open_cookie_file(path: impl AsRef<Path>) -> io::Result<File> {
    let mut open_options = OpenOptions::new();
    open_options
        .read(true)
        .write(true)
        .create(true)
        .mode(mode(Category::Owner, Op::Write) | mode(Category::Owner, Op::Read));

    secure_open_impl(path.as_ref(), &mut open_options, true, true)
}

fn checks(path: &Path, meta: Metadata) -> io::Result<()> {
    let error = |msg| Error::new(ErrorKind::PermissionDenied, msg);

    let path_mode = meta.permissions().mode();
    if meta.uid() != 0 {
        Err(error(format!("{} must be owned by root", path.display())))
    } else if meta.gid() != 0 && (path_mode & mode(Category::Group, Op::Write) != 0) {
        Err(error(format!(
            "{} cannot be group-writable",
            path.display()
        )))
    } else if path_mode & mode(Category::World, Op::Write) != 0 {
        Err(error(format!(
            "{} cannot be world-writable",
            path.display()
        )))
    } else {
        Ok(())
    }
}

// Open `path` with options `open_options`, provided that it is "secure".
// "Secure" means that it passes the `checks` function above.
// If `check_parent_dir` is set, also check that the parent directory is "secure" also.
// If `create_parent_dirs` is set, create the path to the file if it does not already exist.
fn secure_open_impl(
    path: &Path,
    open_options: &mut OpenOptions,
    check_parent_dir: bool,
    create_parent_dirs: bool,
) -> io::Result<File> {
    let error = |msg| Error::new(ErrorKind::PermissionDenied, msg);
    if check_parent_dir || create_parent_dirs {
        if let Some(parent_dir) = path.parent() {
            // if we should create parent dirs and it does not yet exist, create it
            if create_parent_dirs && !parent_dir.exists() {
                DirBuilder::new()
                    .recursive(true)
                    .mode(
                        mode(Category::Owner, Op::Write)
                            | mode(Category::Owner, Op::Read)
                            | mode(Category::Owner, Op::Exec)
                            | mode(Category::Group, Op::Exec)
                            | mode(Category::World, Op::Exec),
                    )
                    .create(parent_dir)?;
            }

            if check_parent_dir {
                let parent_meta = std::fs::metadata(parent_dir)?;
                checks(parent_dir, parent_meta)?;
            }
        } else {
            return Err(error(format!(
                "{} has no valid parent directory",
                path.display()
            )));
        }
    }

    let file = open_options.open(path)?;
    let meta = file.metadata()?;
    checks(path, meta)?;

    Ok(file)
}

fn open_at(parent: BorrowedFd, file_name: &CStr, create: bool) -> io::Result<OwnedFd> {
    let flags = if create {
        libc::O_NOFOLLOW | libc::O_RDWR | libc::O_CREAT
    } else {
        libc::O_NOFOLLOW | libc::O_RDONLY
    };

    // the mode for files that are created is hardcoded, as it is in ogsudo
    let mode = libc::S_IRUSR | libc::S_IWUSR | libc::S_IRGRP | libc::S_IROTH;

    // SAFETY: by design, a correct CStr pointer is passed to openat; only if this call succeeds
    // is the file descriptor it returns (which is then necessarily valid) passed to from_raw_fd
    unsafe {
        let fd = cerr(libc::openat(
            parent.as_raw_fd(),
            file_name.as_ptr(),
            flags,
            mode,
        ))?;

        Ok(OwnedFd::from_raw_fd(fd))
    }
}

/// This opens a file for sudoedit, performing security checks (see below) and
/// opening with reduced privileges.
pub fn secure_open_for_sudoedit(
    path: impl AsRef<Path>,
    user: &User,
    group: &Group,
) -> io::Result<File> {
    sudo_call(user, group, || traversed_secure_open(path, user))?
}

/// This opens a file making sure that
/// - no directory leading up to the file is editable by the user
/// - no components are a symbolic link
fn traversed_secure_open(path: impl AsRef<Path>, user: &User) -> io::Result<File> {
    let path = path.as_ref();

    let Some(file_name) = path.file_name() else {
        return Err(io::Error::new(ErrorKind::InvalidInput, "invalid path"));
    };

    let mut components = path.parent().unwrap_or(Path::new("")).components();
    if components.next() != Some(Component::RootDir) {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "path must be absolute",
        ));
    }

    let user_cannot_write = |file: &File| -> io::Result<()> {
        let meta = file.metadata()?;
        let perms = meta.permissions().mode();

        if perms & mode(Category::World, Op::Write) != 0
            || (perms & mode(Category::Group, Op::Write) != 0) && user.gid.inner() == meta.gid()
            || (perms & mode(Category::Owner, Op::Write) != 0) && user.uid.inner() == meta.uid()
        {
            Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "cannot open a file in a path writeable by the user",
            ))
        } else {
            Ok(())
        }
    };

    let mut cur = File::open("/")?;
    user_cannot_write(&cur)?;

    for component in components {
        let dir: CString = match component {
            Component::Normal(dir) => CString::new(dir.as_bytes())?,
            Component::CurDir => cstr!(".").to_owned(),
            Component::ParentDir => cstr!("..").to_owned(),
            _ => {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    "error in provided path",
                ))
            }
        };

        cur = open_at(cur.as_fd(), &dir, false)?.into();
        user_cannot_write(&cur)?;
    }

    cur = open_at(cur.as_fd(), &CString::new(file_name.as_bytes())?, true)?.into();
    user_cannot_write(&cur)?;

    Ok(cur)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn secure_open_is_predictable() {
        // /etc/hosts should be readable and "secure" (if this test fails, you have been compromised)
        assert!(std::fs::File::open("/etc/hosts").is_ok());
        assert!(secure_open_sudoers("/etc/hosts", false).is_ok());

        // /tmp should be readable, but not secure (writeable by group other than root)
        assert!(std::fs::File::open("/tmp").is_ok());
        assert!(secure_open_sudoers("/tmp", false).is_err());

        #[cfg(target_os = "linux")]
        {
            // /var/log/wtmp should be readable, but not secure (writeable by group other than root)
            // It doesn't exist on many non-Linux systems however.
            if std::fs::File::open("/var/log/wtmp").is_ok() {
                assert!(secure_open_sudoers("/var/log/wtmp", false).is_err());
            }
        }

        // /etc/shadow should not be readable
        assert!(std::fs::File::open("/etc/shadow").is_err());
        assert!(secure_open_sudoers("/etc/shadow", false).is_err());
    }

    #[test]
    fn test_secure_open_cookie_file() {
        assert!(secure_open_cookie_file("/etc/hosts").is_err());
    }

    #[test]
    fn test_traverse_secure_open_negative() {
        use crate::common::resolve::CurrentUser;

        let root = User::from_name(cstr!("root")).unwrap().unwrap();
        let user = CurrentUser::resolve().unwrap();

        // not allowed -- invalid
        assert!(traversed_secure_open("/", &root).is_err());
        // not allowed since the path is not absolute
        assert!(traversed_secure_open("./hello.txt", &root).is_err());
        // not allowed since root can write to "/"
        assert!(traversed_secure_open("/hello.txt", &root).is_err());
        // not allowed since "/tmp" is a directory
        assert!(traversed_secure_open("/tmp", &user).is_err());
        // not allowed since anybody can write to "/tmp"
        assert!(traversed_secure_open("/tmp/foo/hello.txt", &user).is_err());
        // not allowed since "/bin" is a symlink
        assert!(traversed_secure_open("/bin/hello.txt", &user).is_err());
    }

    #[test]
    fn test_traverse_secure_open_positive() {
        use crate::common::resolve::CurrentUser;
        use crate::system::{GroupId, UserId};

        let other_user = CurrentUser::fake(User {
            uid: UserId::new(1042),
            gid: GroupId::new(1042),

            name: "test".into(),
            home: "/home/test".into(),
            shell: "/bin/sh".into(),
            groups: vec![],
        });

        // allowed!
        let path = std::env::current_dir()
            .unwrap()
            .join("sudo-rs-test-file.txt");
        let file = traversed_secure_open(&path, &other_user).unwrap();
        if file.metadata().is_ok_and(|meta| meta.len() == 0) {
            std::fs::remove_file(path).unwrap();
        }
    }
}
