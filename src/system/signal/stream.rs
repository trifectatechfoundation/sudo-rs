use std::{
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
    sync::OnceLock,
};

use crate::{cutils::cerr, log::dev_error};

use super::{
    handler::{SignalHandler, SignalHandlerBehavior},
    info::SignalInfo,
    signal_name, SignalNumber,
};

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
    #[track_caller]
    pub(crate) fn init() -> io::Result<&'static Self> {
        let (rx, tx) = UnixStream::pair().map_err(|err| {
            dev_error!("cannot create socket pair for `SignalStream`: {err}");
            err
        })?;

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

#[track_caller]
pub(crate) fn register_handlers<const N: usize>(
    signals: [SignalNumber; N],
) -> io::Result<[SignalHandler; N]> {
    let mut handlers = signals.map(|signal| (signal, MaybeUninit::uninit()));

    for (signal, handler) in &mut handlers {
        *handler = SignalHandler::register(*signal, SignalHandlerBehavior::Stream)
            .map(MaybeUninit::new)
            .map_err(|err| {
                let name = signal_name(*signal);
                dev_error!("cannot setup handler for {name}: {err}");
                err
            })?;
    }

    Ok(handlers.map(|(_, handler)| unsafe { handler.assume_init() }))
}

impl AsRawFd for SignalStream {
    fn as_raw_fd(&self) -> RawFd {
        self.rx.as_raw_fd()
    }
}
