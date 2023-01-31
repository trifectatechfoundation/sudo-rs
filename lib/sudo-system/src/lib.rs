use std::{
    ffi::{CStr, CString},
    fs::OpenOptions,
    mem::MaybeUninit,
    os::fd::AsRawFd,
    path::PathBuf,
};

fn cerr(res: libc::c_int) -> std::io::Result<libc::c_int> {
    match res {
        -1 => Err(std::io::Error::last_os_error()),
        _ => Ok(res),
    }
}

fn cerr_long(res: libc::c_long) -> std::io::Result<libc::c_long> {
    match res {
        -1 => Err(std::io::Error::last_os_error()),
        _ => Ok(res),
    }
}

extern "C" {
    #[cfg_attr(
        any(target_os = "macos", target_os = "ios", target_os = "freebsd"),
        link_name = "__error"
    )]
    #[cfg_attr(
        any(target_os = "openbsd", target_os = "netbsd", target_os = "android"),
        link_name = "__errno"
    )]
    #[cfg_attr(target_os = "linux", link_name = "__errno_location")]
    fn errno_location() -> *mut libc::c_int;
}

fn set_errno(no: libc::c_int) {
    unsafe { *errno_location() = no };
}

fn sysconf(name: libc::c_int) -> Option<libc::c_long> {
    set_errno(0);
    match cerr_long(unsafe { libc::sysconf(name) }) {
        Ok(res) => Some(res),
        Err(_) => None,
    }
}

fn string_from_ptr(ptr: *const libc::c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        let cstr = unsafe { CStr::from_ptr(ptr) };
        cstr.to_string_lossy().to_string()
    }
}

pub fn hostname() -> String {
    let max_hostname_size = sysconf(libc::_SC_HOST_NAME_MAX).unwrap_or(256);
    let mut buf = vec![0; max_hostname_size as usize];
    match cerr(unsafe { libc::gethostname(buf.as_mut_ptr(), buf.len() - 1) }) {
        Ok(_) => string_from_ptr(buf.as_ptr()),
        Err(_) => {
            // there aren't any known conditions under which the gethostname call should fail
            panic!("Unexpected error while retrieving hostname, this should not happen");
        }
    }
}

#[derive(Debug)]
pub struct User {
    pub uid: libc::uid_t,
    pub gid: libc::gid_t,
    pub name: String,
    pub gecos: String,
    pub home: String,
    pub shell: String,
    pub passwd: String,
}

impl User {
    pub fn from_libc(pwd: &libc::passwd) -> User {
        User {
            uid: pwd.pw_uid,
            gid: pwd.pw_gid,
            name: string_from_ptr(pwd.pw_name),
            gecos: string_from_ptr(pwd.pw_gecos),
            home: string_from_ptr(pwd.pw_dir),
            shell: string_from_ptr(pwd.pw_shell),
            passwd: string_from_ptr(pwd.pw_passwd),
        }
    }

    pub fn from_uid(uid: libc::uid_t) -> std::io::Result<Option<User>> {
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
            Ok(Some(Self::from_libc(&pwd)))
        }
    }

    pub fn effective_uid() -> libc::uid_t {
        unsafe { libc::geteuid() }
    }

    pub fn effective() -> std::io::Result<Option<User>> {
        Self::from_uid(Self::effective_uid())
    }

    pub fn real_uid() -> libc::uid_t {
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
            Ok(Some(Self::from_libc(&pwd)))
        }
    }
}

#[derive(Debug)]
pub struct Group {
    pub gid: libc::gid_t,
    pub name: String,
    pub passwd: String,
    pub members: Vec<String>,
}

impl Group {
    pub fn from_libc(grp: &libc::group) -> Group {
        // find out how many members we have
        let mut mem_count = 0;
        while unsafe { !(*grp.gr_mem.offset(mem_count)).is_null() } {
            mem_count += 1;
        }

        // convert the members to a slice and then put them into a vec of strings
        let mut members = Vec::with_capacity(mem_count as usize);
        let mem_slice = unsafe { std::slice::from_raw_parts(grp.gr_mem, mem_count as usize) };
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

    pub fn effective_gid() -> libc::gid_t {
        unsafe { libc::getegid() }
    }

    pub fn effective() -> std::io::Result<Option<Group>> {
        Self::from_gid(Self::effective_gid())
    }

    pub fn real_gid() -> libc::uid_t {
        unsafe { libc::getgid() }
    }

    pub fn real() -> std::io::Result<Option<Group>> {
        Self::from_gid(Self::real_gid())
    }

    pub fn from_gid(gid: libc::gid_t) -> std::io::Result<Option<Group>> {
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
            Ok(Some(Group::from_libc(&grp)))
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
            Ok(Some(Group::from_libc(&grp)))
        }
    }
}

#[derive(Debug)]
pub struct Process {
    pub pid: libc::pid_t,
    pub parent_pid: libc::pid_t,
    pub group_id: libc::pid_t,
    pub session_id: libc::pid_t,
    pub term_foreground_group_id: libc::pid_t,
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
    pub fn process_id() -> libc::pid_t {
        unsafe { libc::getpid() }
    }

    /// Return the parent process identifier for the current process
    pub fn parent_id() -> libc::pid_t {
        unsafe { libc::getppid() }
    }

    /// Return the process group id for the current process
    pub fn group_id() -> libc::pid_t {
        unsafe { libc::getpgid(0) }
    }

    /// Get the session id for the current process
    pub fn session_id() -> libc::pid_t {
        unsafe { libc::getsid(0) }
    }

    /// Get the process group id of the process group that is currently in
    /// the foreground of our terminal
    pub fn term_foreground_group_id() -> libc::pid_t {
        match OpenOptions::new().read(true).write(true).open("/dev/tty") {
            Ok(f) => {
                let res = unsafe { libc::tcgetpgrp(f.as_raw_fd()) };
                if res == -1 {
                    0
                } else {
                    res
                }
            }
            Err(_) => 0,
        }
    }
}
