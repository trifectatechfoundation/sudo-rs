use std::{
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
    sync::atomic::{AtomicI32, Ordering},
};

use crate::cutils::cerr;

use super::{info::SignalInfo, SignalNumber};

/// The sender socket of the most recently created `SignalStream`.
static TX: AtomicI32 = AtomicI32::new(-1);

pub(super) unsafe fn send_siginfo(
    _signal: SignalNumber,
    info: *const SignalInfo,
    _context: *const libc::c_void,
) {
    let tx = TX.load(Ordering::SeqCst);
    if tx != -1 {
        unsafe { libc::send(tx, info.cast(), SignalInfo::SIZE, libc::MSG_DONTWAIT) };
    }
}

/// A type able to receive signal information from any [`super::SignalHandler`] with the
/// [`super::SignalHandlerBehavior::Stream`] behavior.
///
/// If more than one value of this type is created, the information will only be sent to the most
/// recently created value.
pub(crate) struct SignalStream {
    rx: UnixStream,
    _tx: UnixStream,
}

impl SignalStream {
    /// Create a new [`SignalStream`].
    pub(crate) fn new() -> io::Result<Self> {
        let (rx, tx) = UnixStream::pair()?;

        TX.store(tx.as_raw_fd(), Ordering::SeqCst);

        Ok(Self { rx, _tx: tx })
    }
    /// Receives the information related to the arrival of a signal.
    pub(crate) fn recv(&mut self) -> io::Result<SignalInfo> {
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
