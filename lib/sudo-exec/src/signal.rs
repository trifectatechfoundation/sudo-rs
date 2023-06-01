use std::{ffi::c_int, io};

use signal_hook::low_level::emulate_default_handler;
use sudo_system::signal::SignalStream;

pub(crate) struct SignalHandler<const SIGNO: c_int> {
    pub(crate) stream: SignalStream<SIGNO>,
    pub(crate) emulate_default_handler: bool,
}

impl<const SIGNO: c_int> SignalHandler<SIGNO> {
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Self {
            stream: SignalStream::new()?,
            emulate_default_handler: false,
        })
    }
}
