// TODO: remove unused attribute when system is cleaned up
#[cfg(target_os = "linux")]
use std::str::FromStr;
use std::{
    ffi::{c_char, c_int, c_long, c_uint, CStr},
    fmt, fs, io,
    mem::MaybeUninit,
    ops,
    os::unix,
    path::PathBuf,
};

use crate::{
    common::{Error, SudoPath, SudoString},
    cutils::*,
};
use interface::{DeviceId, GroupId, ProcessId, UserId};
pub use libc::PATH_MAX;
use libc::{CLOSE_RANGE_CLOEXEC, EINVAL, ENOSYS, STDERR_FILENO};
use time::ProcessCreateTime;

use self::signal::SignalNumber;

pub(crate) mod audit;
// generalized traits for when we want to hide implementations
pub mod interface;

pub mod file;

pub mod time;

pub mod timestamp;

pub mod signal;

pub mod term;

pub mod wait;

#[cfg(not(any(target_os = "freebsd", target_os = "linux")))]
compile_error!("sudo-rs only works on Linux and FreeBSD");

pub(crate) fn _exit(status: c_int) -> ! {
    // SAFETY: this function is safe to call
    unsafe { libc::_exit(status) }
}

/// Mark every file descriptor that is not one of the IO streams as CLOEXEC.
pub(crate) fn mark_fds_as_cloexec() -> io::Result<()> {
    let lowfd = STDERR_FILENO + 1;

    // SAFETY: this function is safe to call:
    // - any errors while closing a specific fd will be effectively ignored
    #[allow(clippy::diverging_sub_expression)]
    let res = unsafe {
        'a: {
            #[cfg(not(target_os = "linux"))]
            break 'a cerr(libc::close_range(
                lowfd as c_uint,
                c_uint::MAX,
                CLOSE_RANGE_CLOEXEC as c_int,
            ));
            // on Linux, close_range was only added in glibc 2.34, and is not
            // part of musl, so we go perform a straight syscall instead
            #[cfg(target_os = "linux")]
            break 'a cerr(libc::syscall(
                libc::SYS_close_range,
                lowfd as c_uint,
                c_uint::MAX,
                CLOSE_RANGE_CLOEXEC as c_uint,
            ));
        }
    };

    match res {
        Err(err) if err.raw_os_error() == Some(ENOSYS) || err.raw_os_error() == Some(EINVAL) => {
            // The kernel doesn't support close_range or CLOSE_RANGE_CLOEXEC,
            // fallback to finding all open fds using /proc/self/fd.

            // FIXME use /dev/fd on macOS
            for entry in fs::read_dir("/proc/self/fd")? {
                let entry = entry?;
                let file_name = entry.file_name();
                let file_name = file_name.to_str().ok_or(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "procfs returned non-integer fd name",
                ))?;
                if file_name == "." || file_name == ".." {
                    continue;
                }
                let fd = file_name.parse::<c_int>().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "procfs returned non-integer fd name",
                    )
                })?;
                if fd < lowfd {
                    continue;
                }
                // SAFETY: This only sets the CLOEXEC flag for the given fd. Nothing is
                // going to need it after exec.
                unsafe {
                    cerr(libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC))?;
                }
            }

            Ok(())
        }
        Err(err) => Err(err),
        Ok(_) => Ok(()),
    }
}

pub(crate) enum ForkResult {
    // Parent process branch with the child process' PID.
    Parent(ProcessId),
    // Child process branch.
    Child,
}

/// Create a new process.
///
/// # Safety
///
/// Must not be called in multithreaded programs.
pub(crate) unsafe fn fork() -> io::Result<ForkResult> {
    // FIXME add debug assertion that we are not currently using multiple threads.
    // SAFETY: Calling async-signal-unsafe functions after fork is safe as the program is single
    // threaded at this point according to the safety invariant of this function.
    let pid = cerr(unsafe { libc::fork() })?;
    if pid == 0 {
        Ok(ForkResult::Child)
    } else {
        Ok(ForkResult::Parent(ProcessId::new(pid)))
    }
}

/// Create a new process with extra precautions for usage in tests.
///
/// # Safety
///
/// In a multithreaded program, only async-signal-safe functions are guaranteed to work in the
/// child process until a call to `execve` or a similar function is done.
#[cfg(test)]
unsafe fn fork_for_test(child_func: impl FnOnce() -> std::convert::Infallible) -> ProcessId {
    use std::io::Write;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::process::exit;

    // SAFETY: Not really safe, but this is test only code.
    match unsafe { fork() }.unwrap() {
        ForkResult::Child => {
            // Make sure that panics in the child always abort the process.
            let err = match catch_unwind(AssertUnwindSafe(child_func)) {
                Ok(res) => match res {},
                Err(err) => err,
            };

            let s = if let Some(s) = err.downcast_ref::<&str>() {
                s
            } else if let Some(s) = err.downcast_ref::<String>() {
                s
            } else {
                "Box<dyn Any>"
            };
            let _ = writeln!(std::io::stderr(), "{s}");

            exit(101);
        }
        ForkResult::Parent(pid) => pid,
    }
}

pub fn setsid() -> io::Result<ProcessId> {
    // SAFETY: this function is memory-safe to call
    Ok(ProcessId::new(cerr(unsafe { libc::setsid() })?))
}

#[derive(Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Hostname {
    inner: String,
}

impl fmt::Debug for Hostname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Hostname").field(&self.inner).finish()
    }
}

impl fmt::Display for Hostname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}

impl ops::Deref for Hostname {
    type Target = str;

    fn deref(&self) -> &str {
        &self.inner
    }
}

impl Hostname {
    #[cfg(test)]
    pub fn fake(hostname: &str) -> Self {
        Self {
            inner: hostname.to_string(),
        }
    }

    pub fn resolve() -> Self {
        // see `man 2 gethostname`
        const MAX_HOST_NAME_SIZE_ACCORDING_TO_SUSV2: c_long = 255;

        // POSIX.1 systems limit hostnames to `HOST_NAME_MAX` bytes
        // not including null-byte in the count
        let max_hostname_size = sysconf(libc::_SC_HOST_NAME_MAX)
            .unwrap_or(MAX_HOST_NAME_SIZE_ACCORDING_TO_SUSV2)
            as usize;

        let buffer_size = max_hostname_size + 1 /* null byte delimiter */ ;
        let mut buf = vec![0; buffer_size];

        // SAFETY: we are passing a valid pointer to gethostname
        match cerr(unsafe { libc::gethostname(buf.as_mut_ptr(), buffer_size) }) {
            Ok(_) => Self {
                // SAFETY: gethostname succeeded, so `buf` will hold a null-terminated C string
                inner: unsafe { string_from_ptr(buf.as_ptr()) },
            },

            // ENAMETOOLONG is returned when hostname is greater than `buffer_size`
            Err(_) => {
                // but we have chosen a `buffer_size` larger than `max_hostname_size` so no truncation error is possible
                panic!("Unexpected error while retrieving hostname, this should not happen");
            }
        }
    }
}

pub fn syslog(priority: c_int, facility: c_int, message: &CStr) {
    const MSG: *const c_char = match CStr::from_bytes_until_nul(b"%s\0") {
        Ok(cstr) => cstr.as_ptr(),
        Err(_) => panic!("syslog formatting string is not null-terminated"),
    };

    // SAFETY:
    // - "MSG" is a constant expression that is a null-terminated C string that represents "%s";
    //   this also means that to achieve safety we MUST pass one more argument to syslog that is a proper
    //   pointer to a null-terminated C string
    // - message.as_ptr() is a pointer to a proper null-terminated C string (message being a &CStr)
    // for more info: read the manpage for syslog(2)
    unsafe {
        libc::syslog(priority | facility, MSG, message.as_ptr());
    }
}

/// Makes sure that that the target is included in the groups, and is its first element
fn inject_group(target: GroupId, groups: &mut Vec<GroupId>) {
    if let Some(index) = groups.iter().position(|id| id == &target) {
        // make sure the requested group id is the first in the list (necessary on FreeBSD)
        groups.swap(0, index)
    } else {
        // add target group to list of additional groups if not present
        groups.insert(0, target);
    }
}

/// Set the supplementary groups -- returns a c_int to mimic a libc function
fn set_supplementary_groups(groups: &[GroupId]) -> io::Result<()> {
    // On FreeBSD, setgruops expects the size to be passed as a i32, so the below
    // conversion protects a very extreme case of arithmetic conversion error
    #[allow(irrefutable_let_patterns)]
    #[allow(clippy::useless_conversion)]
    let Ok(len) = groups.len().try_into() else {
        return Err(io::Error::new(io::ErrorKind::Other, "too many groups"));
    };
    // SAFETY: setgroups is passed a valid pointer to a chunk of memory of the correct size
    // We can cast to gid_t because `GroupId` is marked as transparent
    cerr(unsafe { libc::setgroups(len, groups.as_ptr().cast::<libc::gid_t>()) })?;

    Ok(())
}

/// set target user and groups (uid, gid, additional groups) for a command
pub fn set_target_user(
    cmd: &mut std::process::Command,
    mut target_user: User,
    target_group: Group,
) {
    use std::os::unix::process::CommandExt;

    inject_group(target_group.gid, &mut target_user.groups);

    // we need to do this in a `pre_exec` call since the `groups` method in `process::Command` is unstable
    // see https://github.com/rust-lang/rust/blob/a01b4cc9f375f1b95fa8195daeea938d3d9c4c34/library/std/src/sys/unix/process/process_unix.rs#L329-L352
    // for the std implementation of the libc calls to `setgroups`, `setgid` and `setuid`
    // SAFETY: Setuid, setgid and setgroups are async-signal-safe.
    unsafe {
        cmd.pre_exec(move || {
            set_supplementary_groups(&target_user.groups)?;
            // setgid and setuid set the real, effective and saved version of the gid and uid
            // respectively rather than just the real gid and uid. The original sudo uses setresgid
            // and setresuid instead with all three arguments equal, but as this does the same as
            // setgid and setuid using the latter is fine too.
            cerr(libc::setgid(target_group.gid.inner()))?;
            cerr(libc::setuid(target_user.uid.inner()))?;

            Ok(())
        });
    }
}

/// Send a signal to a process with the specified ID.
pub fn kill(pid: ProcessId, signal: SignalNumber) -> io::Result<()> {
    // SAFETY: This function cannot cause UB even if `pid` is not a valid process ID or if
    // `signal` is not a valid signal code.
    cerr(unsafe { libc::kill(pid.inner(), signal) }).map(|_| ())
}

/// Send a signal to a process group with the specified ID.
pub fn killpg(pgid: ProcessId, signal: SignalNumber) -> io::Result<()> {
    // SAFETY: This function cannot cause UB even if `pgid` is not a valid process ID or if
    // `signal` is not a valid signal code.
    cerr(unsafe { libc::killpg(pgid.inner(), signal) }).map(|_| ())
}

/// Get the process group ID of the current process.
pub fn getpgrp() -> ProcessId {
    // SAFETY: This function is always safe to call
    ProcessId::new(unsafe { libc::getpgrp() })
}

/// Get a process group ID.
pub fn getpgid(pid: ProcessId) -> io::Result<ProcessId> {
    // SAFETY: This function cannot cause UB even if `pid` is not a valid process ID
    Ok(ProcessId::new(cerr(unsafe { libc::getpgid(pid.inner()) })?))
}

/// Set a process group ID.
pub fn setpgid(pid: ProcessId, pgid: ProcessId) -> io::Result<()> {
    // SAFETY: This function cannot cause UB even if `pid` or `pgid` are not a valid process IDs:
    // https://pubs.opengroup.org/onlinepubs/007904975/functions/setpgid.html
    cerr(unsafe { libc::setpgid(pid.inner(), pgid.inner()) }).map(|_| ())
}

pub fn chown<S: AsRef<CStr>>(
    path: &S,
    uid: impl Into<UserId>,
    gid: impl Into<GroupId>,
) -> io::Result<()> {
    let path = path.as_ref().as_ptr();
    let uid = uid.into();
    let gid = gid.into();

    // SAFETY: path is a valid pointer to a null-terminated C string; chown cannot cause safety
    // issues even if uid and/or gid would be invalid identifiers.
    cerr(unsafe { libc::chown(path, uid.inner(), gid.inner()) }).map(|_| ())
}

#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub uid: UserId,
    pub gid: GroupId,
    pub name: SudoString,
    pub home: SudoPath,
    pub shell: PathBuf,
    pub groups: Vec<GroupId>,
}

impl User {
    /// # Safety
    /// This function expects `pwd` to be a result from a successful call to `getpwXXX_r`.
    /// (It can cause UB if any of `pwd`'s pointed-to strings does not have a null-terminator.)
    unsafe fn from_libc(pwd: &libc::passwd) -> Result<User, Error> {
        let mut buf_len: c_int = 32;
        let mut groups_buffer: Vec<libc::gid_t>;

        while {
            groups_buffer = vec![0; buf_len as usize];
            // SAFETY: getgrouplist is passed valid pointers
            // in particular `groups_buffer` is an array of `buf.len()` bytes, as required
            let result = unsafe {
                libc::getgrouplist(
                    pwd.pw_name,
                    pwd.pw_gid,
                    groups_buffer.as_mut_ptr(),
                    &mut buf_len,
                )
            };

            result == -1
        } {
            if buf_len >= 65536 {
                panic!("user has too many groups (> 65536), this should not happen");
            }

            buf_len *= 2;
        }

        groups_buffer.resize_with(buf_len as usize, || {
            panic!("invalid groups count returned from getgrouplist, this should not happen")
        });

        // SAFETY: All pointers were initialized by a successful call to `getpwXXX_r` as per the
        // safety invariant of this function.
        unsafe {
            Ok(User {
                uid: UserId::new(pwd.pw_uid),
                gid: GroupId::new(pwd.pw_gid),
                name: SudoString::new(string_from_ptr(pwd.pw_name))?,
                home: SudoPath::new(os_string_from_ptr(pwd.pw_dir).into())?,
                shell: os_string_from_ptr(pwd.pw_shell).into(),
                groups: groups_buffer
                    .iter()
                    .map(|id| GroupId::new(*id))
                    .collect::<Vec<_>>(),
            })
        }
    }

    pub fn from_uid(uid: UserId) -> Result<Option<User>, Error> {
        let max_pw_size = sysconf(libc::_SC_GETPW_R_SIZE_MAX).unwrap_or(16_384);
        let mut buf = vec![0; max_pw_size as usize];
        let mut pwd = MaybeUninit::uninit();
        let mut pwd_ptr = std::ptr::null_mut();
        // SAFETY: getpwuid_r is passed valid (although partly uninitialized) pointers to memory,
        // in particular `buf` points to an array of `buf.len()` bytes, as required.
        // After this call, if `pwd_ptr` is not NULL, `*pwd_ptr` and `pwd` will be aliased;
        // but we never dereference `pwd_ptr`.
        cerr(unsafe {
            libc::getpwuid_r(
                uid.inner(),
                pwd.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &mut pwd_ptr,
            )
        })?;
        if pwd_ptr.is_null() {
            Ok(None)
        } else {
            // SAFETY: pwd_ptr was not null, and getpwuid_r succeeded, so we have assurances that
            // the `pwd` structure was written to by getpwuid_r
            let pwd = unsafe { pwd.assume_init() };
            // SAFETY: `pwd` was obtained by a call to getpwXXX_r, as required.
            unsafe { Self::from_libc(&pwd).map(Some) }
        }
    }

    pub fn effective_uid() -> UserId {
        // SAFETY: this function cannot cause memory safety issues
        UserId::new(unsafe { libc::geteuid() })
    }

    pub fn effective_gid() -> GroupId {
        // SAFETY: this function cannot cause memory safety issues
        GroupId::new(unsafe { libc::getegid() })
    }

    pub fn real_uid() -> UserId {
        // SAFETY: this function cannot cause memory safety issues
        UserId::new(unsafe { libc::getuid() })
    }

    pub fn real_gid() -> GroupId {
        // SAFETY: this function cannot cause memory safety issues
        GroupId::new(unsafe { libc::getgid() })
    }

    pub fn real() -> Result<Option<User>, Error> {
        Self::from_uid(Self::real_uid())
    }

    pub fn primary_group(&self) -> std::io::Result<Group> {
        // Use from_gid_unchecked here to ensure that we can still resolve when
        // the /etc/group entry for the primary group is missing.
        Group::from_gid_unchecked(self.gid)
    }

    pub fn from_name(name_c: &CStr) -> Result<Option<User>, Error> {
        let max_pw_size = sysconf(libc::_SC_GETPW_R_SIZE_MAX).unwrap_or(16_384);
        let mut buf = vec![0; max_pw_size as usize];
        let mut pwd = MaybeUninit::uninit();
        let mut pwd_ptr = std::ptr::null_mut();

        // SAFETY: analogous to getpwuid_r above
        cerr(unsafe {
            libc::getpwnam_r(
                name_c.as_ptr(),
                pwd.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &mut pwd_ptr,
            )
        })?;
        if pwd_ptr.is_null() {
            Ok(None)
        } else {
            // SAFETY: pwd_ptr was not null, and getpwnam_r succeeded, so we have assurances that
            // the `pwd` structure was written to by getpwnam_r
            let pwd = unsafe { pwd.assume_init() };
            // SAFETY: `pwd` was obtained by a call to getpwXXX_r, as required.
            unsafe { Self::from_libc(&pwd).map(Some) }
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Group {
    pub gid: GroupId,
    pub name: Option<String>,
}

impl Group {
    /// # Safety
    /// This function expects `grp` to be a result from a successful call to `getgrXXX_r`.
    /// In particular the grp.gr_mem pointer is assumed to be non-null, and pointing to a
    /// null-terminated list; the pointed-to strings are expected to be null-terminated.
    unsafe fn from_libc(grp: &libc::group) -> Group {
        // SAFETY: The name pointer is initialized by a successful call to `getgrXXX_r` as per the
        // safety invariant of this function.
        let name = unsafe { string_from_ptr(grp.gr_name) };
        Group {
            gid: GroupId::new(grp.gr_gid),
            name: Some(name),
        }
    }

    /// Lookup group for gid without returning an error when a /etc/group entry is missing.
    fn from_gid_unchecked(gid: GroupId) -> std::io::Result<Group> {
        let max_gr_size = sysconf(libc::_SC_GETGR_R_SIZE_MAX).unwrap_or(16_384);
        let mut buf = vec![0; max_gr_size as usize];
        let mut grp = MaybeUninit::uninit();
        let mut grp_ptr = std::ptr::null_mut();
        // SAFETY: analogous to getpwuid_r above
        cerr(unsafe {
            libc::getgrgid_r(
                gid.inner(),
                grp.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &mut grp_ptr,
            )
        })?;
        if grp_ptr.is_null() {
            Ok(Group { gid, name: None })
        } else {
            // SAFETY: grp_ptr was not null, and getgrgid_r succeeded, so we have assurances that
            // the `grp` structure was written to by getgrgid_r
            let grp = unsafe { grp.assume_init() };
            // SAFETY: `pwd` was obtained by a call to getgrXXX_r, as required.
            Ok(unsafe { Group::from_libc(&grp) })
        }
    }

    pub fn from_gid(gid: GroupId) -> std::io::Result<Option<Group>> {
        let group = Self::from_gid_unchecked(gid)?;
        if group.name.is_none() {
            // No entry in /etc/group
            Ok(None)
        } else {
            Ok(Some(group))
        }
    }

    pub fn from_name(name_c: &CStr) -> std::io::Result<Option<Group>> {
        let max_gr_size = sysconf(libc::_SC_GETGR_R_SIZE_MAX).unwrap_or(16_384);
        let mut buf = vec![0; max_gr_size as usize];
        let mut grp = MaybeUninit::uninit();
        let mut grp_ptr = std::ptr::null_mut();
        // SAFETY: analogous to getpwuid_r above
        cerr(unsafe {
            libc::getgrnam_r(
                name_c.as_ptr(),
                grp.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &mut grp_ptr,
            )
        })?;
        if grp_ptr.is_null() {
            Ok(None)
        } else {
            // SAFETY: grp_ptr was not null, and getgrgid_r succeeded, so we have assurances that
            // the `grp` structure was written to by getgrgid_r
            let grp = unsafe { grp.assume_init() };
            // SAFETY: `pwd` was obtained by a call to getgrXXX_r, as required.
            Ok(Some(unsafe { Group::from_libc(&grp) }))
        }
    }
}

pub enum WithProcess {
    Current,
    Other(ProcessId),
}

impl WithProcess {
    #[cfg(target_os = "linux")]
    fn to_proc_string(&self) -> String {
        match self {
            WithProcess::Current => "self".into(),
            WithProcess::Other(pid) => pid.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Process {
    pub pid: ProcessId,
    pub parent_pid: Option<ProcessId>,
    pub session_id: ProcessId,
}

impl Default for Process {
    fn default() -> Self {
        Self::new()
    }
}

impl Process {
    pub fn new() -> Process {
        Process {
            pid: Self::process_id(),
            parent_pid: Self::parent_id(),
            session_id: Self::session_id(),
        }
    }

    /// Return the process identifier for the current process
    pub fn process_id() -> ProcessId {
        // NOTE libstd casts the `i32` that `libc::getpid` returns into `u32`
        // here we cast it back into `i32` (`ProcessId`)
        ProcessId::new(std::process::id() as i32)
    }

    /// Return the parent process identifier for the current process
    pub fn parent_id() -> Option<ProcessId> {
        // NOTE libstd casts the `i32` that `libc::getppid` returns into `u32`
        // here we cast it back into `i32` (`ProcessId`)
        let pid = ProcessId::new(unix::process::parent_id() as i32);
        if !pid.is_valid() {
            None
        } else {
            Some(pid)
        }
    }

    /// Get the session id for the current process
    pub fn session_id() -> ProcessId {
        // SAFETY: this function is explicitly safe to call with argument 0,
        // and more generally getsid will never cause memory safety issues.
        ProcessId::new(unsafe { libc::getsid(0) })
    }

    /// Returns the device identifier of the TTY device that is currently
    /// attached to the given process
    #[cfg(target_os = "linux")]
    pub fn tty_device_id(pid: WithProcess) -> std::io::Result<Option<DeviceId>> {
        // device id of tty is displayed as a signed integer of 32 bits
        let data: i32 = read_proc_stat(pid, 6 /* tty_nr */)?;
        if data == 0 {
            Ok(None)
        } else {
            // While the integer was displayed as signed in the proc stat file,
            // we actually need to interpret the bits of that integer as an unsigned
            // int. We convert via u32 because a direct conversion to DeviceId
            // would use sign extension, which would result in a different bit
            // representation
            Ok(Some(DeviceId::new(data as u64)))
        }
    }

    #[cfg(target_os = "freebsd")]
    fn get_proc_info(pid: WithProcess) -> std::io::Result<libc::kinfo_proc> {
        use std::ffi::c_void;
        use std::ptr;

        let mut ki_proc: Vec<libc::kinfo_proc> = Vec::with_capacity(1);

        let pid = match pid {
            WithProcess::Current => std::process::id() as i32,
            WithProcess::Other(pid) => pid.inner(),
        };

        loop {
            let mut size = ki_proc.capacity() * size_of::<libc::kinfo_proc>();
            // SAFETY: KERN_PROC_PID only reads data into the ki_proc list. It
            // does not write more than `size` bytes to the pointer.
            match cerr(unsafe {
                libc::sysctl(
                    [
                        libc::CTL_KERN,
                        libc::KERN_PROC,
                        libc::KERN_PROC_PID,
                        pid,
                        size_of::<libc::kinfo_proc>() as i32,
                        1,
                    ]
                    .as_ptr(),
                    4,
                    ki_proc.as_mut_ptr().cast::<c_void>(),
                    &mut size,
                    ptr::null(),
                    0,
                )
            }) {
                Ok(_) => {
                    assert!(size >= size_of::<libc::kinfo_proc>());
                    // SAFETY: The above sysctl has initialized at least `size` bytes. We have
                    // asserted that this is at least a single element.
                    unsafe {
                        ki_proc.set_len(1);
                    }
                    break;
                }
                Err(e) if e.raw_os_error() == Some(libc::ENOMEM) => {
                    // Vector not big enough. Grow it by 10% and try again.
                    ki_proc.reserve(ki_proc.capacity() + (ki_proc.capacity() + 9) / 10);
                }
                Err(e) => return Err(e),
            }
        }

        Ok(ki_proc[0])
    }

    /// Returns the device identifier of the TTY device that is currently
    /// attached to the given process
    #[cfg(target_os = "freebsd")]
    pub fn tty_device_id(pid: WithProcess) -> std::io::Result<Option<DeviceId>> {
        let ki_proc = Self::get_proc_info(pid)?;

        if ki_proc.ki_tdev == !0 {
            Ok(None)
        } else {
            Ok(Some(DeviceId::new(ki_proc.ki_tdev)))
        }
    }

    /// Get the process starting time of a specific process
    #[cfg(target_os = "linux")]
    pub fn starting_time(pid: WithProcess) -> io::Result<ProcessCreateTime> {
        let process_start: u64 = read_proc_stat(pid, 21 /* start_time */)?;

        // the startime field is stored in ticks since the system start, so we need to know how many
        // ticks go into a second
        let ticks_per_second = crate::cutils::sysconf(libc::_SC_CLK_TCK).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "Could not retrieve system config variable for ticks per second",
            )
        })? as u64;

        // finally compute the system time at which the process was started
        Ok(ProcessCreateTime::new(
            (process_start / ticks_per_second) as i64,
            ((process_start % ticks_per_second) * (1_000_000_000 / ticks_per_second)) as i64,
        ))
    }

    /// Get the process starting time of a specific process
    #[cfg(target_os = "freebsd")]
    pub fn starting_time(pid: WithProcess) -> io::Result<ProcessCreateTime> {
        let ki_proc = Self::get_proc_info(pid)?;

        let ki_start = ki_proc.ki_start;
        #[allow(clippy::useless_conversion)]
        Ok(ProcessCreateTime::new(
            i64::from(ki_start.tv_sec),
            i64::from(ki_start.tv_usec) * 1000,
        ))
    }
}

/// Read the n-th field (with 0-based indexing) from `/proc/<pid>/self`.
///
/// See ["Table 1-4: Contents of the stat fields" of "The /proc
/// Filesystem"][proc_stat_fields] in the Linux docs for all available fields.
///
/// IMPORTANT: the first two fields are not accessible with this routine.
///
/// [proc_stat_fields]: https://www.kernel.org/doc/html/latest/filesystems/proc.html#id10
#[cfg(target_os = "linux")]
fn read_proc_stat<T: FromStr>(pid: WithProcess, field_idx: isize) -> io::Result<T> {
    // the first two fields are skipped by the code below, and we never need them,
    // so no point in implementing code for it in this private function.
    debug_assert!(field_idx >= 2);

    // read from a specific pid file, or use `self` to refer to our own process
    let pidref = pid.to_proc_string();

    // read the data from the stat file for the process with the given pid
    let path = PathBuf::from_iter(&["/proc", &pidref, "stat"]);
    let proc_stat = std::fs::read(path)?;

    // first get the part of the stat file past the second argument, we then reverse
    // search for a ')' character and start the search for the starttime field from there on
    let skip_past_second_arg = proc_stat.iter().rposition(|b| *b == b')').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Could not find position of 'comm' field in process stat",
        )
    })?;
    let mut stat = &proc_stat[skip_past_second_arg..];

    // we've now passed the first two fields, so we are at index 1, now we skip over
    // fields until we arrive at the field we are searching for
    let mut curr_field = 1;
    while curr_field < field_idx && !stat.is_empty() {
        if stat[0] == b' ' {
            curr_field += 1;
        }
        stat = &stat[1..];
    }

    // The expected field cannot be in the file anymore when we are at EOF
    if stat.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Stat file was not of the expected format",
        ));
    }

    // we've now arrived at the field we are looking for, we now check how
    // long this field is by finding where the next space is
    let mut idx = 0;
    while stat[idx] != b' ' && idx < stat.len() {
        idx += 1;
    }
    let field = &stat[0..idx];

    // we first convert the data to a string slice, this should not fail with a normal /proc filesystem
    let fielddata = std::str::from_utf8(field).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Could not interpret byte slice as string",
        )
    })?;

    // then we convert the string slice to whatever the requested type was
    fielddata.parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Could not interpret string as number",
        )
    })
}

pub fn escape_os_str_lossy(s: &std::ffi::OsStr) -> String {
    s.to_string_lossy().escape_default().collect()
}

pub fn make_zeroed_sigaction() -> libc::sigaction {
    // SAFETY: since sigaction is a C struct, all-zeroes is a valid representation
    // We cannot use a "literal struct" initialization method since the exact representation
    // of libc::sigaction is not fixed, see e.g. https://github.com/trifectatechfoundation/sudo-rs/issues/829
    unsafe { std::mem::zeroed() }
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) const ROOT_GROUP_NAME: &str = "root";

#[cfg(all(test, not(target_os = "linux")))]
pub(crate) const ROOT_GROUP_NAME: &str = "wheel";

#[allow(clippy::undocumented_unsafe_blocks)]
#[cfg(test)]
mod tests {
    use std::{
        ffi::c_char,
        io::{self, Read, Write},
        os::{
            fd::{AsFd, AsRawFd},
            unix::net::UnixStream,
        },
        process::exit,
    };

    use libc::SIGKILL;

    use crate::system::interface::{GroupId, ProcessId, UserId};

    use super::{
        fork_for_test, getpgrp, setpgid,
        wait::{Wait, WaitOptions},
        Group, User, WithProcess, ROOT_GROUP_NAME,
    };

    pub(super) fn tempfile() -> std::io::Result<std::fs::File> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Failed to get system time")
            .as_nanos();
        let pid = std::process::id();

        let filename = format!("sudo_rs_test_{pid}_{timestamp}");
        let path = std::path::PathBuf::from("/tmp").join(filename);
        std::fs::File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
    }

    #[test]
    fn test_get_user_and_group_by_id() {
        let fixed_users = &[
            (UserId::ROOT, "root"),
            (
                User::from_name(cstr!("daemon")).unwrap().unwrap().uid,
                "daemon",
            ),
        ];
        for &(id, name) in fixed_users {
            let root = User::from_uid(id).unwrap().unwrap();
            assert_eq!(root.uid, id);
            assert_eq!(root.name, name);
        }

        let fixed_groups = &[
            (GroupId::new(0), ROOT_GROUP_NAME),
            (
                Group::from_name(cstr!("daemon")).unwrap().unwrap().gid,
                "daemon",
            ),
        ];
        for &(id, name) in fixed_groups {
            let root = Group::from_gid(id).unwrap().unwrap();
            assert_eq!(root.gid, id);
            assert_eq!(root.name.unwrap(), name);
        }
    }

    #[test]
    fn miri_test_group_impl() {
        use super::Group;
        use std::ffi::CString;

        fn test(name: &str, passwd: &str, gid: libc::gid_t, mem: &[&str]) {
            assert_eq!(
                {
                    let c_mem: Vec<CString> =
                        mem.iter().map(|&s| CString::new(s).unwrap()).collect();
                    let c_name = CString::new(name).unwrap();
                    let c_passwd = CString::new(passwd).unwrap();
                    unsafe {
                        Group::from_libc(&libc::group {
                            gr_name: c_name.as_ptr() as *mut _,
                            gr_passwd: c_passwd.as_ptr() as *mut _,
                            gr_gid: gid,
                            gr_mem: c_mem
                                .iter()
                                .map(|cs| cs.as_ptr() as *mut _)
                                .chain(std::iter::once(std::ptr::null_mut()))
                                .collect::<Vec<*mut c_char>>()
                                .as_mut_ptr(),
                        })
                    }
                },
                Group {
                    name: Some(name.to_string()),
                    gid: GroupId::new(gid),
                }
            )
        }

        test("dr. bill", "fidelio", 1999, &["eyes", "wide", "shut"]);
        test("eris", "fnord", 5, &[]);
        test("abc", "password123", 42, &[""]);
    }

    #[test]
    fn get_process_tty_device() {
        assert!(super::Process::tty_device_id(WithProcess::Current).is_ok());
    }

    #[test]
    fn pgid_test() {
        use super::{getpgid, setpgid};

        let pgrp = getpgrp();
        assert_eq!(getpgid(ProcessId::new(0)).unwrap(), pgrp);
        assert_eq!(
            getpgid(ProcessId::new(std::process::id() as i32)).unwrap(),
            pgrp
        );

        let child_pid = unsafe {
            super::fork_for_test(|| {
                // wait for the parent.
                std::thread::sleep(std::time::Duration::from_secs(1));
                exit(0);
            })
        };

        // The child should be in our process group.
        assert_eq!(
            getpgid(child_pid).unwrap(),
            getpgid(ProcessId::new(0)).unwrap(),
        );
        // Move the child to its own process group
        setpgid(child_pid, child_pid).unwrap();
        // The process group of the child should have changed.
        assert_eq!(getpgid(child_pid).unwrap(), child_pid);
    }
    #[test]
    fn kill_test() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("1")
            .spawn()
            .unwrap();
        super::kill(ProcessId::new(child.id() as i32), SIGKILL).unwrap();
        assert!(!child.wait().unwrap().success());
    }
    #[test]
    fn killpg_test() {
        // Create a socket so the children write to it if they aren't terminated by `killpg`.
        let (mut rx, mut tx) = UnixStream::pair().unwrap();

        let pid1 = unsafe {
            fork_for_test(|| {
                std::thread::sleep(std::time::Duration::from_secs(1));
                tx.write_all(&[42]).unwrap();
                exit(0);
            })
        };

        let pid2 = unsafe {
            fork_for_test(|| {
                std::thread::sleep(std::time::Duration::from_secs(1));
                tx.write_all(&[42]).unwrap();
                exit(0);
            })
        };

        drop(tx);

        let pgid = pid1;
        // Move the children to their own process group.
        setpgid(pid1, pgid).unwrap();
        setpgid(pid2, pgid).unwrap();
        // Send `SIGKILL` to the children process group.
        super::killpg(pgid, SIGKILL).unwrap();
        // Ensure that the child were terminated before writing.
        assert_eq!(
            rx.read_exact(&mut [0; 2]).unwrap_err().kind(),
            std::io::ErrorKind::UnexpectedEof
        );
    }

    fn is_cloexec<F: AsFd>(fd: &F) -> bool {
        crate::cutils::cerr(unsafe { libc::fcntl(fd.as_fd().as_raw_fd(), libc::F_GETFD) }).unwrap()
            & libc::FD_CLOEXEC
            == libc::FD_CLOEXEC
    }

    #[test]
    fn mark_fds_as_cloexec() {
        let child_pid = unsafe {
            fork_for_test(|| {
                let should_close =
                    std::fs::File::create(std::env::temp_dir().join("should_close.txt")).unwrap();
                crate::cutils::cerr(libc::fcntl(
                    should_close.as_fd().as_raw_fd(),
                    libc::F_SETFD,
                    0,
                ))
                .unwrap();
                assert!(!is_cloexec(&should_close));

                super::mark_fds_as_cloexec().unwrap();

                assert!(is_cloexec(&should_close));

                assert!(!is_cloexec(&io::stdin()));
                assert!(!is_cloexec(&io::stdout()));
                assert!(!is_cloexec(&io::stderr()));

                exit(0)
            })
        };

        let (_, status) = child_pid.wait(WaitOptions::new()).unwrap();
        assert_eq!(status.exit_status(), Some(0));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_stat_test() {
        use super::{read_proc_stat, Process, WithProcess::Current};
        // The process can be '(uninterruptible) sleeping' or 'running': it looks like the state
        // field of /proc/pid/stat will show the state for the main thread of the process rather
        // than for the process as a whole.
        let state = read_proc_stat::<char>(Current, 2).unwrap();
        assert!("SDR".contains(state), "{state} is not S, D or R");
        let parent = Process::parent_id().unwrap();
        // field 3 is always the parent process
        assert_eq!(
            parent,
            ProcessId::new(read_proc_stat::<i32>(Current, 3).unwrap())
        );
        // this next field should always be 0 (which precedes an important bit of info for us!)
        assert_eq!(0, read_proc_stat::<i32>(Current, 20).unwrap());
    }
}
