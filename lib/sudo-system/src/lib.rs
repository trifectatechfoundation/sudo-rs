use std::{
    ffi::{c_int, CString},
    fs::OpenOptions,
    io,
    mem::MaybeUninit,
    os::fd::AsRawFd,
    path::PathBuf,
    str::FromStr,
};

pub use audit::secure_open;
use interface::{DeviceId, GroupId, ProcessId, UserId};
pub use libc::PATH_MAX;
use sudo_cutils::*;
use time::SystemTime;

mod audit;
// generalized traits for when we want to hide implementations
pub mod interface;

pub mod file;

pub mod time;

pub mod timestamp;

pub fn hostname() -> String {
    let max_hostname_size = sysconf(libc::_SC_HOST_NAME_MAX).unwrap_or(256);
    let mut buf = vec![0; max_hostname_size as usize];
    match cerr(unsafe { libc::gethostname(buf.as_mut_ptr(), buf.len() - 1) }) {
        Ok(_) => unsafe { string_from_ptr(buf.as_ptr()) },
        Err(_) => {
            // there aren't any known conditions under which the gethostname call should fail
            panic!("Unexpected error while retrieving hostname, this should not happen");
        }
    }
}

/// set target user and groups (uid, gid, additional groups) for a command
pub fn set_target_user(
    cmd: &mut std::process::Command,
    mut target_user: User,
    target_group: Group,
) {
    use std::os::unix::process::CommandExt;

    // add target group to list of additional groups if not present
    if !target_user.groups.contains(&target_group.gid) {
        target_user.groups.push(target_group.gid);
    }

    // we need to do this in a `pre_exec` call since the `groups` method in `process::Command` is unstable
    // see https://github.com/rust-lang/rust/blob/a01b4cc9f375f1b95fa8195daeea938d3d9c4c34/library/std/src/sys/unix/process/process_unix.rs#L329-L352
    // for the std implementation of the libc calls to `setgroups`, `setgid` and `setuid`
    unsafe {
        cmd.pre_exec(move || {
            cerr(libc::setgroups(
                target_user.groups.len(),
                target_user.groups.as_ptr(),
            ))?;
            cerr(libc::setgid(target_group.gid))?;
            cerr(libc::setuid(target_user.uid))?;

            Ok(())
        });
    }
}

/// Send a signal to a process.
pub fn kill(pid: ProcessId, signal: c_int) -> c_int {
    // SAFETY: This function cannot cause UB even if `pid` is not a valid process ID or if
    // `signal` is not a valid signal code.
    unsafe { libc::kill(pid, signal) }
}

/// Get a process group ID.
pub fn getpgid(pid: ProcessId) -> ProcessId {
    // SAFETY: This function cannot cause UB even if `pid` is not a valid process ID
    unsafe { libc::getpgid(pid) }
}

#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub uid: UserId,
    pub gid: GroupId,
    pub name: String,
    pub gecos: String,
    pub home: PathBuf,
    pub shell: PathBuf,
    pub passwd: String,
    pub groups: Vec<GroupId>,
}

impl User {
    /// # Safety
    /// This function expects `pwd` to be a result from a succesful call to `getpwXXX_r`.
    /// (It can cause UB if any of `pwd`'s pointed-to strings does not have a null-terminator.)
    unsafe fn from_libc(pwd: &libc::passwd) -> User {
        let mut buf_len: libc::c_int = 32;
        let mut groups_buffer: Vec<libc::gid_t>;

        while {
            groups_buffer = vec![0; buf_len as usize];
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

        User {
            uid: pwd.pw_uid,
            gid: pwd.pw_gid,
            name: string_from_ptr(pwd.pw_name),
            gecos: string_from_ptr(pwd.pw_gecos),
            home: os_string_from_ptr(pwd.pw_dir).into(),
            shell: os_string_from_ptr(pwd.pw_shell).into(),
            passwd: string_from_ptr(pwd.pw_passwd),
            groups: groups_buffer,
        }
    }

    pub fn from_uid(uid: UserId) -> std::io::Result<Option<User>> {
        let max_pw_size = sysconf(libc::_SC_GETPW_R_SIZE_MAX).unwrap_or(16_384);
        let mut buf = vec![0; max_pw_size as usize];
        let mut pwd = MaybeUninit::uninit();
        let mut pwd_ptr = std::ptr::null_mut();
        cerr(unsafe {
            libc::getpwuid_r(
                uid,
                pwd.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &mut pwd_ptr,
            )
        })?;
        if pwd_ptr.is_null() {
            Ok(None)
        } else {
            let pwd = unsafe { pwd.assume_init() };
            Ok(Some(unsafe { Self::from_libc(&pwd) }))
        }
    }

    pub fn effective_uid() -> UserId {
        unsafe { libc::geteuid() }
    }

    pub fn effective() -> std::io::Result<Option<User>> {
        Self::from_uid(Self::effective_uid())
    }

    pub fn real_uid() -> UserId {
        unsafe { libc::getuid() }
    }

    pub fn real() -> std::io::Result<Option<User>> {
        Self::from_uid(Self::real_uid())
    }

    pub fn from_name(name: &str) -> std::io::Result<Option<User>> {
        let max_pw_size = sysconf(libc::_SC_GETPW_R_SIZE_MAX).unwrap_or(16_384);
        let mut buf = vec![0; max_pw_size as usize];
        let mut pwd = MaybeUninit::uninit();
        let mut pwd_ptr = std::ptr::null_mut();
        let name_c = CString::new(name).expect("String contained null bytes");
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
            let pwd = unsafe { pwd.assume_init() };
            Ok(Some(unsafe { Self::from_libc(&pwd) }))
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Group {
    pub gid: GroupId,
    pub name: String,
    pub passwd: String,
    pub members: Vec<String>,
}

impl Group {
    /// # Safety
    /// This function expects `grp` to be a result from a succesful call to `getgrXXX_r`.
    /// In particular the grp.gr_mem pointer is assumed to be non-null, and pointing to a
    /// null-terminated list; the pointed-to strings are expected to be null-terminated.
    unsafe fn from_libc(grp: &libc::group) -> Group {
        // find out how many members we have
        let mut mem_count = 0;
        while !(*grp.gr_mem.offset(mem_count)).is_null() {
            mem_count += 1;
        }

        // convert the members to a slice and then put them into a vec of strings
        let mut members = Vec::with_capacity(mem_count as usize);
        let mem_slice = std::slice::from_raw_parts(grp.gr_mem, mem_count as usize);
        for mem in mem_slice {
            members.push(string_from_ptr(*mem));
        }

        Group {
            gid: grp.gr_gid,
            name: string_from_ptr(grp.gr_name),
            passwd: string_from_ptr(grp.gr_passwd),
            members,
        }
    }

    pub fn effective_gid() -> GroupId {
        unsafe { libc::getegid() }
    }

    pub fn effective() -> std::io::Result<Option<Group>> {
        Self::from_gid(Self::effective_gid())
    }

    pub fn real_gid() -> UserId {
        unsafe { libc::getgid() }
    }

    pub fn real() -> std::io::Result<Option<Group>> {
        Self::from_gid(Self::real_gid())
    }

    pub fn from_gid(gid: GroupId) -> std::io::Result<Option<Group>> {
        let max_gr_size = sysconf(libc::_SC_GETGR_R_SIZE_MAX).unwrap_or(16_384);
        let mut buf = vec![0; max_gr_size as usize];
        let mut grp = MaybeUninit::uninit();
        let mut grp_ptr = std::ptr::null_mut();
        cerr(unsafe {
            libc::getgrgid_r(
                gid,
                grp.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
                &mut grp_ptr,
            )
        })?;
        if grp_ptr.is_null() {
            Ok(None)
        } else {
            let grp = unsafe { grp.assume_init() };
            Ok(Some(unsafe { Group::from_libc(&grp) }))
        }
    }

    pub fn from_name(name: &str) -> std::io::Result<Option<Group>> {
        let max_gr_size = sysconf(libc::_SC_GETGR_R_SIZE_MAX).unwrap_or(16_384);
        let mut buf = vec![0; max_gr_size as usize];
        let mut grp = MaybeUninit::uninit();
        let mut grp_ptr = std::ptr::null_mut();
        let name_c = CString::new(name).expect("String contained null bytes");
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
            let grp = unsafe { grp.assume_init() };
            Ok(Some(unsafe { Group::from_libc(&grp) }))
        }
    }
}

pub enum WithProcess {
    Current,
    Other(ProcessId),
}

impl WithProcess {
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
    pub group_id: ProcessId,
    pub session_id: ProcessId,
    pub term_foreground_group_id: Option<ProcessId>,
    pub name: PathBuf,
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
            group_id: Self::group_id(),
            session_id: Self::session_id(),
            term_foreground_group_id: Self::term_foreground_group_id(),
            name: Self::process_name().unwrap_or_else(|| PathBuf::from("sudo")),
        }
    }

    pub fn process_name() -> Option<PathBuf> {
        std::env::args().next().map(PathBuf::from)
    }

    /// Return the process identifier for the current process
    pub fn process_id() -> ProcessId {
        unsafe { libc::getpid() }
    }

    /// Return the parent process identifier for the current process
    pub fn parent_id() -> Option<ProcessId> {
        let pid = unsafe { libc::getppid() };
        if pid == 0 {
            None
        } else {
            Some(pid)
        }
    }

    /// Return the process group id for the current process
    pub fn group_id() -> ProcessId {
        unsafe { libc::getpgid(0) }
    }

    /// Get the session id for the current process
    pub fn session_id() -> ProcessId {
        unsafe { libc::getsid(0) }
    }

    /// Get the process group id of the process group that is currently in
    /// the foreground of our terminal
    pub fn term_foreground_group_id() -> Option<ProcessId> {
        match OpenOptions::new().read(true).write(true).open("/dev/tty") {
            Ok(f) => {
                let res = unsafe { libc::tcgetpgrp(f.as_raw_fd()) };
                if res == -1 {
                    None
                } else {
                    Some(res)
                }
            }
            Err(_) => None,
        }
    }

    /// Returns the device identifier of the TTY device that is currently
    /// attached to the given process
    pub fn tty_device_id(pid: WithProcess) -> std::io::Result<Option<DeviceId>> {
        // device id of tty is displayed as a signed integer of 32 bits
        let data: i32 = read_proc_stat(pid, 6)?;
        if data == 0 {
            Ok(None)
        } else {
            // While the integer was displayed as signed in the proc stat file,
            // we actually need to interpret the bits of that integer as an unsigned
            // int. We convert via u32 because a direct conversion to DeviceId
            // would use sign extension, which would result in a different bit
            // representation
            Ok(Some(data as u32 as DeviceId))
        }
    }

    /// Get the process starting time of a specific process
    pub fn starting_time(pid: WithProcess) -> io::Result<SystemTime> {
        let process_start: u64 = read_proc_stat(pid, 21)?;

        // the startime field is stored in ticks since the system start, so we need to know how many
        // ticks go into a second
        let ticks_per_second = sudo_cutils::sysconf(libc::_SC_CLK_TCK).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "Could not retrieve system config variable for ticks per second",
            )
        })? as u64;

        // finally compute the system time at which the process was started
        Ok(SystemTime::new(
            (process_start / ticks_per_second) as i64,
            ((process_start % ticks_per_second) * (1_000_000_000 / ticks_per_second)) as i64,
        ))
    }
}

fn read_proc_stat<T: FromStr>(pid: WithProcess, field_idx: isize) -> io::Result<T> {
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

#[cfg(test)]
mod tests {
    use crate::{Group, User, WithProcess};

    #[test]
    fn test_get_user_and_group_by_id() {
        let fixed_users = &[(0, "root"), (1, "daemon")];
        for &(id, name) in fixed_users {
            let root = User::from_uid(id).unwrap().unwrap();
            assert_eq!(root.uid, id as libc::uid_t);
            assert_eq!(root.name, name);
        }
        for &(id, name) in fixed_users {
            let root = Group::from_gid(id).unwrap().unwrap();
            assert_eq!(root.gid, id as libc::gid_t);
            assert_eq!(root.name, name);
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
                                .collect::<Vec<*mut libc::c_char>>()
                                .as_mut_ptr(),
                        })
                    }
                },
                Group {
                    name: name.to_string(),
                    passwd: passwd.to_string(),
                    gid,
                    members: mem.iter().map(|s| s.to_string()).collect(),
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
}
