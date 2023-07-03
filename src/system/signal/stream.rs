use std::{
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
};

use crate::cutils::cerr;

use super::{info::SignalInfo, SignalNumber};

static mut TX: RawFd = -1;

pub(super) unsafe fn send_siginfo(
    _signal: SignalNumber,
    info: *const SignalInfo,
    _context: *const libc::c_void,
) {
    let tx = unsafe { TX };
    if tx != -1 {
        unsafe { libc::send(tx, info.cast(), SignalInfo::SIZE, libc::MSG_DONTWAIT) };
    }
}

pub(crate) struct SignalStream {
    rx: UnixStream,
    _tx: UnixStream,
}

impl SignalStream {
    pub(crate) fn new() -> io::Result<Self> {
        let (rx, tx) = UnixStream::pair()?;

        unsafe { TX = tx.as_raw_fd() };

        Ok(Self { rx, _tx: tx })
    }
    /// Receives the information related to the arrival of a signal.
    ///
    /// Calling this function will block until a signal whose action is set to
    /// [`SignalAction::stream`] arrives. Otherwise it will block indefinitely.
    ///
    /// Note that calling this function will only retrieve the information related to a single
    /// signal arrival even if several signals have been sent to the process.
    pub(crate) fn recv(&mut self) -> io::Result<SignalInfo> {
        let mut info = MaybeUninit::<SignalInfo>::uninit();
        let fd = self.rx.as_raw_fd();
        let bytes =
            cerr(unsafe { libc::recv(fd, info.as_mut_ptr().cast(), SignalInfo::SIZE, 0) })?;

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

