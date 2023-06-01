use std::{
    ffi::c_int,
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
};

use libc::{c_void, siginfo_t, MSG_DONTWAIT};
use signal_hook_registry::{register_sigaction, unregister, SigId};
use sudo_cutils::cerr;

use crate::interface::ProcessId;

const SIGINFO_SIZE: usize = std::mem::size_of::<siginfo_t>();

pub struct SignalInfo {
    info: siginfo_t,
}

impl SignalInfo {
    pub fn is_user_signaled(&self) -> bool {
        // FIXME: we should check if si_code is equal to SI_USER
        self.info.si_code <= 0
    }

    pub fn get_pid(&self) -> ProcessId {
        unsafe { self.info.si_pid() }
    }

    pub fn get_signal(&self) -> c_int {
        self.info.si_signo
    }
}

pub struct SignalReceiver<const SIGNO: c_int> {
    sig_id: SigId,
    rx: UnixStream,
}

impl<const SIGNO: c_int> SignalReceiver<SIGNO> {
    pub fn new() -> io::Result<Self> {
        let (rx, tx) = UnixStream::pair()?;

        let tx = SignalSender { tx };

        let sig_id = unsafe { register_sigaction(SIGNO, move |info| tx.send(info)) }?;

        Ok(Self { sig_id, rx })
    }

    pub fn recv(&mut self) -> io::Result<SignalInfo> {
        let mut info = MaybeUninit::<siginfo_t>::uninit();
        let fd = self.rx.as_raw_fd();
        let bytes =
            cerr(unsafe { libc::recv(fd, info.as_mut_ptr() as *mut c_void, SIGINFO_SIZE, 0) })?;

        if bytes != SIGINFO_SIZE as _ {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, ""));
        }

        let info = unsafe { info.assume_init() };
        Ok(SignalInfo { info })
    }
}

impl<const SIGNO: c_int> Drop for SignalReceiver<SIGNO> {
    fn drop(&mut self) {
        unregister(self.sig_id);
    }
}

impl<const SIGNO: c_int> AsRawFd for SignalReceiver<SIGNO> {
    fn as_raw_fd(&self) -> RawFd {
        self.rx.as_raw_fd()
    }
}

struct SignalSender {
    tx: UnixStream,
}

impl SignalSender {
    fn send(&self, info: &siginfo_t) {
        let fd = self.tx.as_raw_fd();

        unsafe {
            libc::send(
                fd,
                info as *const siginfo_t as *const libc::c_void,
                std::mem::size_of::<siginfo_t>(),
                MSG_DONTWAIT,
            );
        }
    }
}
