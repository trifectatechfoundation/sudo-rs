#![cfg_attr(not(feature = "unstable-remote-sudoers"), allow(unused_imports))]
use std::collections::HashSet;
use std::ffi::{CStr, CString, c_int, c_uint};
use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io::{self, BufReader, Error, ErrorKind};
use std::net::Shutdown;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::{
    ffi::OsStrExt,
    fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    net::UnixStream,
    prelude::OpenOptionsExt,
};
use std::path::{Component, Path};

use super::{
    Group, GroupId, User, UserId, cerr, inject_group, interface::UnixUser, set_supplementary_groups,
};
use crate::common::resolve::CurrentUser;
#[cfg(feature = "unstable-remote-sudoers")]
use crate::sudoers::{Identifier, PeerSpec};

#[cfg(target_os = "linux")]
pub(crate) fn no_new_privs_enabled() -> io::Result<bool> {
    // SAFETY: prctl(PR_GET_NO_NEW_PRIVS) can never cause UB
    let no_new_privs =
        crate::cutils::cerr(unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) })?;
    Ok(no_new_privs != 0)
}

/// Temporary change privileges --- essentially a 'mini sudo'
/// This is only used for sudoedit.
pub(crate) fn sudo_call<T>(
    target_user: &User,
    target_group: &Group,
    operation: impl FnOnce() -> T,
) -> io::Result<T> {
    const KEEP_UID: libc::uid_t = -1i32 as libc::uid_t;
    const KEEP_GID: libc::gid_t = -1i32 as libc::gid_t;

    // SAFETY: these libc functions are always safe to call
    let (cur_user_id, cur_group_id) =
        unsafe { (UserId::new(libc::geteuid()), GroupId::new(libc::getegid())) };

    let cur_groups = {
        // SAFETY: calling with size 0 does not modify through the pointer, and is
        // a documented way of getting the length needed.
        let len = cerr(unsafe { libc::getgroups(0, std::ptr::null_mut()) })?;

        let mut buf: Vec<GroupId> = vec![GroupId::new(KEEP_GID); len as usize];
        // SAFETY: we pass a correct pointer to a slice of the given length
        cerr(unsafe {
            // We can cast to gid_t because `GroupId` is marked as transparent
            libc::getgroups(len, buf.as_mut_ptr().cast::<libc::gid_t>())
        })?;

        buf
    };

    let mut target_groups = target_user.groups.clone();
    inject_group(target_group.gid, &mut target_groups);

    if cfg!(test)
        && target_user.uid == cur_user_id
        && target_group.gid == cur_group_id
        && target_groups.iter().collect::<HashSet<_>>() == cur_groups.iter().collect::<HashSet<_>>()
    {
        // we are not actually switching users, simply run the closure
        // (this would also be safe in production mode, but it is a needless check)
        return Ok(operation());
    }

    struct ResetUserGuard(UserId, GroupId, Vec<GroupId>);

    impl Drop for ResetUserGuard {
        fn drop(&mut self) {
            // restore privileges in reverse order
            (|| {
                // SAFETY: this function is always safe to call
                cerr(unsafe { libc::setresuid(KEEP_UID, UserId::inner(&self.0), KEEP_UID) })?;
                // SAFETY: this function is always safe to call
                cerr(unsafe { libc::setresgid(KEEP_GID, GroupId::inner(&self.1), KEEP_GID) })?;
                set_supplementary_groups(&self.2)
            })()
            .expect("could not restore to saved user id");
        }
    }

    let guard = ResetUserGuard(cur_user_id, cur_group_id, cur_groups);

    set_supplementary_groups(&target_groups)?;
    // SAFETY: this function is always safe to call
    cerr(unsafe { libc::setresgid(KEEP_GID, GroupId::inner(&target_group.gid), KEEP_GID) })?;
    // SAFETY: this function is always safe to call
    cerr(unsafe { libc::setresuid(KEEP_UID, UserId::inner(&target_user.uid), KEEP_UID) })?;

    let result = operation();

    std::mem::drop(guard);
    Ok(result)
}

#[cfg(feature = "unstable-remote-sudoers")]
/// Get the credentials of the peer at the other side of the socket
fn get_peer_credentials(stream: &UnixStream) -> io::Result<libc::ucred> {
    let mut ucred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut ucred_size = size_of::<libc::ucred>() as libc::socklen_t;

    // SAFETY: An out pointer for the correct type and length are passed.
    // FIXME use UnixStream::peer_cred() once stable in our MSRV.
    unsafe {
        cerr(libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut ucred as *mut _ as *mut libc::c_void,
            &mut ucred_size,
        ))?;
    }

    Ok(ucred)
}

/// Drop root privileges by setting effective user id equal to the real user id.
/// This routine will panic if the process is not privileged.
pub fn irrevocably_drop_privileges() {
    // SAFETY:
    // - getuid() and geteuid() are always safe to call
    // - setuid() does not change any memory and only affects OS state.
    unsafe {
        let real_uid = libc::getuid();
        let effective_uid = libc::geteuid();
        // we never use setuid/setgid/setgroups except in the pre_exec hook before exec'ing,
        // or in sudo_call (which always resets the user state after the closure finishes).
        // this extra check is here to punish programming mistakes due to sloppiness.
        assert_eq!(effective_uid, UserId::ROOT.inner(), "setuid violation");

        cerr(libc::setuid(real_uid)).expect("setuid violation");
    }
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
pub fn secure_open_sudoers(path: impl AsRef<Path>) -> io::Result<File> {
    let mut open_options = OpenOptions::new();
    open_options.read(true);

    secure_open_impl(path.as_ref(), &mut open_options, false)
}

#[cfg(feature = "unstable-remote-sudoers")]
pub fn secure_open_remote_sudoers(
    path: impl AsRef<Path>,
    peer_spec: &PeerSpec,
) -> io::Result<BufReader<UnixStream>> {
    secure_open_socket_impl(path.as_ref(), peer_spec)
}

/// Open a timestamp cookie file using various security checks
pub fn secure_open_cookie_file(path: impl AsRef<Path>) -> io::Result<File> {
    let mut open_options = OpenOptions::new();
    open_options
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(mode(Category::Owner, Op::Write) | mode(Category::Owner, Op::Read));

    secure_open_impl(path.as_ref(), &mut open_options, true)
}

/// Return the system zoneinfo path after validating that it is safe
pub fn zoneinfo_path() -> Option<&'static str> {
    let paths = [
        "/usr/share/zoneinfo",
        "/usr/share/lib/zoneinfo",
        "/usr/lib/zoneinfo",
    ];

    paths.into_iter().find(|p| {
        let path = Path::new(p);
        path.metadata().and_then(|meta| checks(path, meta)).is_ok()
    })
}

fn checks(path: &Path, meta: Metadata) -> io::Result<()> {
    let error = |msg| Error::new(ErrorKind::PermissionDenied, msg);

    let path_mode = meta.permissions().mode();
    if meta.uid() != 0 {
        Err(error(xlat!(
            "{path} must be owned by root",
            path = path.display()
        )))
    } else if meta.gid() != 0 && (path_mode & mode(Category::Group, Op::Write) != 0) {
        Err(error(xlat!(
            "{path} cannot be group-writable",
            path = path.display()
        )))
    } else if path_mode & mode(Category::World, Op::Write) != 0 {
        Err(error(xlat!(
            "{path} cannot be world-writable",
            path = path.display()
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
    create_parent_dirs: bool,
) -> io::Result<File> {
    let error = |msg| Error::new(ErrorKind::PermissionDenied, msg);
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

        let parent_meta = std::fs::metadata(parent_dir)?;
        checks(parent_dir, parent_meta)?;
    } else {
        return Err(error(xlat!(
            "{path} has no valid parent directory",
            path = path.display()
        )));
    }

    let file = open_options.open(path)?;
    let meta = file.metadata()?;
    checks(path, meta)?;

    Ok(file)
}

#[cfg(feature = "unstable-remote-sudoers")]
fn check_user(peer_uid: libc::uid_t, user_id: &Identifier) -> io::Result<()> {
    match user_id {
        Identifier::Name(name) => {
            let name_cstr = CString::new(name.as_ref()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "user name contains null byte")
            })?;
            let expected_user = User::from_name(&name_cstr)
                .map_err(|e| io::Error::other(e.to_string()))?
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, format!("user '{name}' not found"))
                })?;
            if peer_uid != expected_user.uid.inner() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "peer process must run as user '{name}' (uid {}) but runs as {peer_uid}",
                        expected_user.uid.inner()
                    ),
                ));
            }
        }
        Identifier::ID(uid) => {
            if peer_uid != *uid {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("peer process must run as uid {uid} but runs as {peer_uid}"),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(feature = "unstable-remote-sudoers")]
fn check_group(peer_gid: libc::gid_t, group_id: Option<&Identifier>) -> io::Result<()> {
    let Some(group_id) = group_id else {
        return Ok(());
    };

    match group_id {
        Identifier::Name(name) => {
            let name_cstr = CString::new(name.as_ref()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "group name contains null byte")
            })?;
            let expected_group = Group::from_name(&name_cstr)
                .map_err(|e| io::Error::other(e.to_string()))?
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, format!("group '{name}' not found"))
                })?;
            if peer_gid != expected_group.gid.inner() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "peer process must run as group '{name}' (gid {}) but runs as gid {peer_gid}",
                        expected_group.gid.inner()
                    ),
                ));
            }
        }
        Identifier::ID(gid) => {
            if peer_gid != *gid {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("peer process must run as gid {gid} but runs as {peer_gid}"),
                ));
            }
        }
    }
    Ok(())
}

// Open the socket at path, provided that it is "secure".
// "Secure" means that it passes the `checks` function above.
#[cfg(feature = "unstable-remote-sudoers")]
fn secure_open_socket_impl(path: &Path, peer_spec: &PeerSpec) -> io::Result<BufReader<UnixStream>> {
    // Check the metadata on the filesystem as extra security,
    // even knowing that it produces a TOCTOU.
    let meta = fs::metadata(path)?;
    checks(path, meta)?;
    if let Some(parent_dir) = path.parent() {
        let parent_meta = std::fs::metadata(parent_dir)?;
        checks(parent_dir, parent_meta)?;
    } else {
        let error = |msg| Error::new(ErrorKind::PermissionDenied, msg);

        return Err(error(xlat!(
            "{path} has no valid parent directory",
            path = path.display()
        )));
    }

    let stream = UnixStream::connect(path)?;

    // Check the peer's credentials to avoid the TOCTOU.
    let peer_creds = get_peer_credentials(&stream)?;

    // If there is an error in these checks, the function returns immediately
    // leaving `stream` out of scope and forcing the closing of the socket.
    check_user(peer_creds.uid, &peer_spec.user)?;
    check_group(peer_creds.gid, peer_spec.group.as_ref())?;

    stream.shutdown(Shutdown::Write)?;
    let reader = BufReader::new(stream);

    Ok(reader)
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
            c_uint::from(mode),
        ))?;

        Ok(OwnedFd::from_raw_fd(fd))
    }
}

fn faccess_at(parent: BorrowedFd, path: &CStr, mode: c_int, flags: c_int) -> io::Result<()> {
    // SAFETY: by design, a correct CStr pointer is passed to faccessat
    cerr(unsafe { libc::faccessat(parent.as_raw_fd(), path.as_ptr(), mode, flags) }).map(|_| ())
}

/// This opens a file for sudoedit, performing security checks (see below) and
/// opening with reduced privileges.
pub fn secure_open_for_sudoedit(
    path: impl AsRef<Path>,
    current_user: &CurrentUser,
    target_user: &User,
    target_group: &Group,
) -> io::Result<File> {
    if current_user.is_root() {
        sudo_call(target_user, target_group, || {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)
        })?
    } else {
        traversed_secure_open(path, current_user, target_user, target_group)
    }
}

/// This opens a file making sure that
/// - no directory leading up to the file is editable by the user
/// - no components are a symbolic link
fn traversed_secure_open(
    path: impl AsRef<Path>,
    #[cfg(not(test))] forbidden_user: &CurrentUser,
    #[cfg(test)] forbidden_user: &User,
    target_user: &User,
    target_group: &Group,
) -> io::Result<File> {
    let path = path.as_ref();

    let Some(file_name) = path.file_name() else {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            xlat!("invalid path"),
        ));
    };

    let mut components = path.parent().unwrap_or(Path::new("")).components();
    if components.next() != Some(Component::RootDir) {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            xlat!("path must be absolute"),
        ));
    }

    let user_cannot_write = |file: &File| -> io::Result<()> {
        let meta = file.metadata()?;
        let perms = meta.permissions().mode();

        if meta.uid() == forbidden_user.uid.inner() {
            // Owner can change file permissions
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                xlat!("cannot open a file in a path writable by the user"),
            ));
        }

        let user_has_write_perms = if cfg!(test) {
            // During testing we do a less comprehensive check as we don't have
            // permission to set the real user id to arbitrary users, but faccessat
            // looks at the real user id.
            perms & mode(Category::World, Op::Write) != 0
                || (perms & mode(Category::Group, Op::Write) != 0)
                    && forbidden_user.in_group_by_gid(GroupId::new(meta.gid()))
                || (perms & mode(Category::Owner, Op::Write) != 0)
                    && forbidden_user.uid.inner() == meta.uid()
        } else {
            // Only works when forbidden_user is current user. This is enforced
            // by accepting CurrentUser outside of test mode.
            // We don't pass AT_EACCESS to faccessat to make it check using the
            // real user id rather than the effective user id.
            faccess_at(file.as_fd(), c"", libc::W_OK, libc::AT_EMPTY_PATH).is_ok()
        };

        if user_has_write_perms {
            Err(io::Error::new(
                ErrorKind::PermissionDenied,
                xlat!("cannot open a file in a path writable by the user"),
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
            Component::CurDir => c".".to_owned(),
            Component::ParentDir => c"..".to_owned(),
            _ => {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    xlat!("error in provided path"),
                ));
            }
        };

        sudo_call(target_user, target_group, || {
            cur = open_at(cur.as_fd(), &dir, false)?.into();
            io::Result::Ok(())
        })??;
        user_cannot_write(&cur)?;
    }
    sudo_call(target_user, target_group, || {
        cur = open_at(cur.as_fd(), &CString::new(file_name.as_bytes())?, true)?.into();
        io::Result::Ok(())
    })??;
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
        assert!(secure_open_sudoers("/etc/hosts").is_ok());

        // /tmp should be readable, but not secure (writable by group other than root)
        assert!(std::fs::File::open("/tmp").is_ok());
        assert!(secure_open_sudoers("/tmp").is_err());

        #[cfg(target_os = "linux")]
        {
            // /var/log/wtmp should be readable, but not secure (writable by group other than root)
            // It doesn't exist on many non-Linux systems however.
            if std::fs::File::open("/var/log/wtmp").is_ok() {
                assert!(secure_open_sudoers("/var/log/wtmp").is_err());
            }
        }

        // /etc/shadow should not be readable
        assert!(std::fs::File::open("/etc/shadow").is_err());
        assert!(secure_open_sudoers("/etc/shadow").is_err());
    }

    #[test]
    fn test_secure_open_cookie_file() {
        assert!(secure_open_cookie_file("/etc/hosts").is_err());
    }

    #[test]
    fn test_traverse_secure_open_negative() {
        use crate::common::resolve::CurrentUser;

        let root = User::from_name(c"root").unwrap().unwrap();
        let user = CurrentUser::resolve().unwrap();

        // not allowed -- invalid
        assert!(traversed_secure_open("/", &root, &user, &user.group()).is_err());
        // not allowed since the path is not absolute
        assert!(traversed_secure_open("./hello.txt", &root, &user, &user.group()).is_err());
        // not allowed since root can write to "/"
        assert!(traversed_secure_open("/hello.txt", &root, &user, &user.group()).is_err());
        // not allowed since "/tmp" is a directory
        assert!(traversed_secure_open("/tmp", &user, &user, &user.group()).is_err());
        // not allowed since anybody can write to "/tmp"
        assert!(traversed_secure_open("/tmp/foo/hello.txt", &user, &user, &user.group()).is_err());
        // not allowed since "/bin" is a symlink
        assert!(traversed_secure_open("/bin/hello.txt", &user, &user, &user.group()).is_err());
    }

    #[test]
    fn test_traverse_secure_open_positive() {
        use crate::common::resolve::CurrentUser;
        use crate::system::{GroupId, UserId};

        let user = CurrentUser::resolve().unwrap();
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
        let file = traversed_secure_open(&path, &other_user, &user, &user.group()).unwrap();
        if file.metadata().is_ok_and(|meta| meta.len() == 0) {
            std::fs::remove_file(path).unwrap();
        }
    }
}
