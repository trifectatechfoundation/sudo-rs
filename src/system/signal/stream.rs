use std::{
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
    sync::OnceLock,
};

use crate::cutils::cerr;

use super::{info::SignalInfo, SignalNumber};

static STREAM: OnceLock<SignalStream> = OnceLock::new();

pub(super) unsafe fn send_siginfo(
    _signal: SignalNumber,
    info: *const SignalInfo,
    _context: *const libc::c_void,
) {
    if let Some(tx) = STREAM.get().map(|stream| stream.tx.as_raw_fd()) {
        unsafe { libc::send(tx, info.cast(), SignalInfo::SIZE, libc::MSG_DONTWAIT) };
    }
}

/// A type able to receive signal information from any [`super::SignalHandler`] with the
/// [`super::SignalHandlerBehavior::Stream`] behavior.
///
/// This is a singleton type. Meaning that there will be only one value of this type during the
/// execution of a program.  
pub(crate) struct SignalStream {
    rx: UnixStream,
    tx: UnixStream,
}

impl SignalStream {
    /// Create a new [`SignalStream`].
    ///
    /// # Panics
    ///
    /// If this function has been called before.
    pub(crate) fn init() -> io::Result<&'static Self> {
        let (rx, tx) = UnixStream::pair()?;

        if STREAM.set(Self { rx, tx }).is_err() {
            panic!("`SignalStream` has already been initialized");
        };

        Ok(STREAM.get().unwrap())
    }

    /// Receives the information related to the arrival of a signal.
    pub(crate) fn recv(&self) -> io::Result<SignalInfo> {
        let mut info = MaybeUninit::<SignalInfo>::uninit();
        let fd = self.rx.as_raw_fd();
        let bytes = cerr(unsafe { libc::recv(fd, info.as_mut_ptr().cast(), SignalInfo::SIZE, 0) })?;

        if bytes as usize != SignalInfo::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Not enough bytes when receiving `siginfo_t`",
            ));
        }
        // SAFETY: we can assume `info` is initialized because `recv` wrote enough bytes to fill
        // the value and `siginfo_t` is POD.
        Ok(unsafe { info.assume_init() })
    }
}

impl AsRawFd for SignalStream {
    fn as_raw_fd(&self) -> RawFd {
        self.rx.as_raw_fd()
    }
}
