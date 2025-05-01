// On Linux we can use a seccomp() filter to disable exec.

use std::alloc::{handle_alloc_error, GlobalAlloc, Layout};
use std::ffi::c_void;
use std::mem::{align_of, size_of, zeroed};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::ptr::{self, addr_of};
use std::{cmp, io, thread};

use libc::{
    c_int, c_uint, c_ulong, close, cmsghdr, iovec, msghdr, prctl, recvmsg, seccomp_data,
    seccomp_notif, seccomp_notif_resp, seccomp_notif_sizes, sendmsg, sock_filter, sock_fprog,
    syscall, SYS_execve, SYS_execveat, SYS_seccomp, __errno_location, BPF_ABS, BPF_JEQ, BPF_JMP,
    BPF_JUMP, BPF_K, BPF_LD, BPF_RET, BPF_STMT, CMSG_DATA, CMSG_FIRSTHDR, CMSG_LEN, CMSG_SPACE,
    EACCES, ENOENT, MSG_TRUNC, PR_SET_NO_NEW_PRIVS, SCM_RIGHTS, SECCOMP_FILTER_FLAG_NEW_LISTENER,
    SECCOMP_GET_NOTIF_SIZES, SECCOMP_RET_ALLOW, SECCOMP_SET_MODE_FILTER,
    SECCOMP_USER_NOTIF_FLAG_CONTINUE, SOL_SOCKET,
};

use crate::system::FileCloser;

const SECCOMP_RET_USER_NOTIF: c_uint = 0x7fc00000;
const SECCOMP_IOCTL_NOTIF_RECV: c_ulong = 0xc0502100;
const SECCOMP_IOCTL_NOTIF_SEND: c_ulong = 0xc0182101;

/// # Safety
///
/// You must follow the rules the Linux man page specifies for the chosen
/// seccomp operation.
unsafe fn seccomp<T>(operation: c_uint, flags: c_uint, args: *mut T) -> c_int {
    // SAFETY: By function invariant.
    unsafe { syscall(SYS_seccomp, operation, flags, args) as c_int }
}

struct NotifyAllocs {
    req: *mut seccomp_notif,
    req_size: usize,
    resp: *mut seccomp_notif_resp,
}

/// Linux reserves the right to demand the memory for an object of type T
/// to be over-allocated; this function ensures that happens.
fn alloc_dynamic<T>(runtime_size: u16) -> (*mut T, usize) {
    // FIXME put this in a const block once the MSRV has been bumped enough
    assert!(size_of::<T>() > 0);

    let layout = Layout::from_size_align(
        cmp::max(runtime_size.into(), size_of::<T>()),
        align_of::<seccomp_notif>(),
    )
    .unwrap();

    // SAFETY: We assert that T is bigger than 0 bytes and as such the computed layout is also
    // bigger.
    let ptr = unsafe { std::alloc::System.alloc_zeroed(layout).cast::<T>() };
    if ptr.is_null() {
        handle_alloc_error(layout);
    }

    (ptr, layout.size())
}

fn alloc_notify_allocs() -> NotifyAllocs {
    let mut sizes = seccomp_notif_sizes {
        seccomp_notif: 0,
        seccomp_notif_resp: 0,
        seccomp_data: 0,
    };
    // SAFETY: A valid seccomp_notif_sizes pointer is passed in
    if unsafe { seccomp(SECCOMP_GET_NOTIF_SIZES, 0, &mut sizes) } == -1 {
        panic!(
            "failed to get sizes for seccomp unotify data structures: {}",
            io::Error::last_os_error(),
        );
    }

    let (req, req_size) = alloc_dynamic::<seccomp_notif>(sizes.seccomp_notif);
    let (resp, _) = alloc_dynamic::<seccomp_notif_resp>(sizes.seccomp_notif_resp);

    NotifyAllocs {
        req,
        req_size,
        resp,
    }
}

/// Returns 'false' if the ioctl failed with E_NOENT, 'true' if it succeeded.
/// This aborts the program in any other situation.
///
/// # Safety
///
/// `ioctl(fd, request, ptr)` must be safe to call
unsafe fn ioctl<T>(fd: RawFd, request: libc::c_ulong, ptr: *mut T) -> bool {
    // SAFETY: By function contract
    if unsafe { libc::ioctl(fd, request, ptr) } == -1 {
        // SAFETY: Trivial
        if unsafe { *__errno_location() } == ENOENT {
            false
        } else {
            // SAFETY: Not actually unsafe
            unsafe {
                libc::abort();
            }
        }
    } else {
        true
    }
}

/// # Safety
///
/// The argument must be a valid seccomp_unotify fd.
unsafe fn handle_notifications(notify_fd: OwnedFd) -> ! {
    let NotifyAllocs {
        req,
        req_size,
        resp,
    } = alloc_notify_allocs();

    // The first notification must be allowed to pass unhindered.
    let mut error = 0;
    let mut flags = SECCOMP_USER_NOTIF_FLAG_CONTINUE as _;

    loop {
        // SECCOMP_IOCTL_NOTIF_RECV expects the target struct to be zeroed
        // SAFETY: req is at least req_size bytes big.
        unsafe { std::ptr::write_bytes(req.cast::<u8>(), 0, req_size) };

        // SAFETY: A valid pointer to a seccomp_notify is passed in; notify_fd is valid.
        if unsafe { !ioctl(notify_fd.as_raw_fd(), SECCOMP_IOCTL_NOTIF_RECV, req) } {
            continue;
        }

        // Allow the first execve call as this is sudo itself starting the target executable.
        // SAFETY: resp is a valid pointer to a seccomp_notify_resp.
        unsafe {
            (*resp).id = (*req).id;
            (*resp).val = 0;
            (*resp).error = error;
            (*resp).flags = flags;
        }

        // SAFETY: A valid pointer to a seccomp_notify_resp is passed in; notify_fd is valid.
        if unsafe { !ioctl(notify_fd.as_raw_fd(), SECCOMP_IOCTL_NOTIF_SEND, resp) } {
            continue;
        }

        // As soon as we have reached this point, all notifications will be acted upon.
        error = -EACCES;
        flags = 0;
    }
}

//We must use vectored reads with ancillary data.
//
//NOTE: some day we can witch to using send/recv_vectored_with_ancillary; see:
// - https://doc.rust-lang.org/std/os/unix/net/struct.UnixDatagram.html#method.recv_vectored_with_ancillary
// - https://doc.rust-lang.org/std/os/unix/net/struct.UnixDatagram.html#method.send_vectored_with_ancillary
// but this is (at the time of writing) unstable.

#[repr(C)]
union SingleRightAnciliaryData {
    // SAFETY: Not actually unsafe
    #[allow(clippy::undocumented_unsafe_blocks)] // Clippy doesn't understand the safety comment
    buf: [u8; unsafe { CMSG_SPACE(size_of::<c_int>() as u32) as usize }],
    _align: cmsghdr,
}

/// Receives a raw file descriptor from the provided UnixStream
fn receive_fd(rx_fd: UnixStream) -> RawFd {
    let mut data = [0u8; 1];
    let mut iov = iovec {
        iov_base: &mut data as *mut [u8; 1] as *mut c_void,
        iov_len: 1,
    };

    // SAFETY: msghdr can be zero-initialized
    let mut msg: msghdr = unsafe { zeroed() };
    msg.msg_name = ptr::null_mut();
    msg.msg_namelen = 0;
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;

    // SAFETY: SingleRightAnciliaryData can be zero-initialized.
    let mut control: SingleRightAnciliaryData = unsafe { zeroed() };
    // SAFETY: The buf field is valid when zero-initialized.
    msg.msg_controllen = unsafe { control.buf.len() };
    msg.msg_control = &mut control as *mut _ as *mut libc::c_void;

    // SAFETY: A valid socket fd and a valid initialized msghdr are passed in.
    if unsafe { recvmsg(rx_fd.as_raw_fd(), &mut msg, 0) } == -1 {
        panic!("failed to recvmsg: {}", io::Error::last_os_error());
    }

    if msg.msg_flags & MSG_TRUNC == MSG_TRUNC {
        unreachable!("unexpected internal error in seccomp filter");
    }

    // SAFETY: The kernel correctly initializes everything on recvmsg for this to be safe.
    unsafe {
        let cmsgp = CMSG_FIRSTHDR(&msg);
        if cmsgp.is_null()
            || (*cmsgp).cmsg_len != CMSG_LEN(size_of::<c_int>() as u32) as usize
            || (*cmsgp).cmsg_level != SOL_SOCKET
            || (*cmsgp).cmsg_type != SCM_RIGHTS
        {
            unreachable!("unexpected response from Linux kernel");
        }
        CMSG_DATA(cmsgp).cast::<c_int>().read()
    }
}

fn send_fd(tx_fd: UnixStream, notify_fd: RawFd) -> io::Result<()> {
    let mut data = [0u8; 1];
    let mut iov = iovec {
        iov_base: &mut data as *mut [u8; 1] as *mut c_void,
        iov_len: 1,
    };

    // SAFETY: msghdr can be zero-initialized
    let mut msg: msghdr = unsafe { zeroed() };
    msg.msg_name = ptr::null_mut();
    msg.msg_namelen = 0;
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;

    // SAFETY: SingleRightAnciliaryData can be zero-initialized.
    let mut control: SingleRightAnciliaryData = unsafe { zeroed() };
    // SAFETY: The buf field is valid when zero-initialized.
    msg.msg_controllen = unsafe { control.buf.len() };
    msg.msg_control = &mut control as *mut _ as *mut _;
    // SAFETY: msg.msg_control is correctly initialized and this follows
    // the contract of the various CMSG_* macros.
    unsafe {
        let cmsgp = CMSG_FIRSTHDR(&msg);
        (*cmsgp).cmsg_level = SOL_SOCKET;
        (*cmsgp).cmsg_type = SCM_RIGHTS;
        (*cmsgp).cmsg_len = CMSG_LEN(size_of::<c_int>() as u32) as usize;
        ptr::write(CMSG_DATA(cmsgp).cast::<c_int>(), notify_fd);
    }

    // SAFETY: A valid socket fd and a valid initialized msghdr are passed in.
    if unsafe { sendmsg(tx_fd.as_raw_fd(), &msg, 0) } == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(crate) fn add_noexec_filter(command: &mut Command, file_closer: &mut FileCloser) {
    let (tx_fd, rx_fd) = UnixStream::pair().unwrap();

    thread::spawn(move || {
        let notify_fd = receive_fd(rx_fd);
        // SAFETY: notify_fd is a valid seccomp_unotify fd.
        unsafe { handle_notifications(OwnedFd::from_raw_fd(notify_fd)) };
    });

    file_closer.except(&tx_fd);

    // wrap tx_fd so it can be moved into the closure
    let mut tx_fd = Some(tx_fd);

    // SAFETY: See individual SAFETY comments
    unsafe {
        // SAFETY: The closure only calls async-signal-safe functions.
        command.pre_exec(move || {
            let tx_fd = tx_fd.take().unwrap();

            // FIXME replace with offset_of!(seccomp_data, nr) once MSRV is bumped to 1.77
            // SAFETY: seccomp_data can be safely zero-initialized.
            let dummy: seccomp_data = zeroed();
            let nr_offset = (&dummy.nr) as *const _ as usize - &dummy as *const _ as usize;

            // SAFETY: libc unnecessarily marks these functions as unsafe
            let exec_filter: [sock_filter; 5] = [
                // Load syscall number into the accumulator
                BPF_STMT((BPF_LD | BPF_ABS) as _, nr_offset as _),
                // Jump to user notify for execve/execveat
                BPF_JUMP((BPF_JMP | BPF_JEQ | BPF_K) as _, SYS_execve as _, 2, 0),
                BPF_JUMP((BPF_JMP | BPF_JEQ | BPF_K) as _, SYS_execveat as _, 1, 0),
                // Allow non-matching syscalls
                BPF_STMT((BPF_RET | BPF_K) as _, SECCOMP_RET_ALLOW),
                // Notify sudo about execve/execveat syscall
                BPF_STMT((BPF_RET | BPF_K) as _, SECCOMP_RET_USER_NOTIF as _),
            ];

            let exec_fprog = sock_fprog {
                len: 5,
                filter: addr_of!(exec_filter) as *mut sock_filter,
            };

            // SAFETY: Trivially safe as it doesn't touch any memory.
            // SECCOMP_SET_MODE_FILTER will fail unless the process has
            // CAP_SYS_ADMIN or the no_new_privs bit is set.
            if prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) == -1 {
                return Err(io::Error::last_os_error());
            }

            // While the man page warns againt using seccomp_unotify as security
            // mechanism, the TOCTOU problem that is described there isn't
            // relevant here. We only SECCOMP_USER_NOTIF_FLAG_CONTINUE the first
            // execve which is done by ourself and thus trusted.
            // SAFETY: Passes a valid sock_fprog as argument.
            let notify_fd = seccomp(
                SECCOMP_SET_MODE_FILTER,
                SECCOMP_FILTER_FLAG_NEW_LISTENER as _,
                addr_of!(exec_fprog).cast_mut(),
            );
            if notify_fd < 0 {
                return Err(io::Error::last_os_error());
            }

            send_fd(tx_fd, notify_fd)?;

            // SAFETY: Nothing will access the notify_fd after this call.
            close(notify_fd);

            Ok(())
        });
    }
}
