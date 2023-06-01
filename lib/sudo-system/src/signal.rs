use std::{
    ffi::c_int,
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

use libc::{c_void, siginfo_t, MSG_DONTWAIT};
use signal_hook::low_level::emulate_default_handler;
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

#[repr(u8)]
pub enum SignalHandler {
    Send = 0,
    Default = 1,
    Ignore = 2,
}

pub struct SignalStream<const SIGNO: c_int> {
    sig_id: SigId,
    rx: UnixStream,
    handler: Arc<AtomicU8>,
}

impl<const SIGNO: c_int> SignalStream<SIGNO> {
    pub fn new() -> io::Result<Self> {
        let (rx, tx) = UnixStream::pair()?;
        let handler = Arc::<AtomicU8>::default();

        let sig_id = {
            let handler = Arc::clone(&handler);
            let tx = SignalSender { tx };
            unsafe {
                register_sigaction(SIGNO, move |info| {
                    let handler = handler.load(Ordering::SeqCst);
                    if handler == SignalHandler::Default as u8 {
                        emulate_default_handler(SIGNO).ok();
                    } else if handler == SignalHandler::Send as u8 {
                        tx.send(info)
                    }
                })
            }?
        };

        Ok(Self {
            sig_id,
            rx,
            handler,
        })
    }

    pub fn set_handler(&self, handler: SignalHandler) -> SignalHandler {
        let prev_handler = self.handler.swap(handler as u8, Ordering::SeqCst);
        if prev_handler == SignalHandler::Default as u8 {
            SignalHandler::Default
        } else if prev_handler == SignalHandler::Send as u8 {
            SignalHandler::Send
        } else {
            SignalHandler::Ignore
        }
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

impl<const SIGNO: c_int> Drop for SignalStream<SIGNO> {
    fn drop(&mut self) {
        unregister(self.sig_id);
    }
}

impl<const SIGNO: c_int> AsRawFd for SignalStream<SIGNO> {
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
