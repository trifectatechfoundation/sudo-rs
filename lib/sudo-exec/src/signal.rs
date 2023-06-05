use std::{collections::HashMap, ffi::c_int, io};

use signal_hook::consts::*;
use sudo_system::{
    poll::PollSet,
    signal::{SignalHandler, SignalInfo},
};

pub(crate) const SIGNALS: &[c_int] = &[
    SIGINT, SIGQUIT, SIGTSTP, SIGTERM, SIGHUP, SIGALRM, SIGPIPE, SIGUSR1, SIGUSR2, SIGCHLD,
    SIGCONT, SIGWINCH,
];

pub(crate) struct SignalHandlers {
    handlers: HashMap<c_int, SignalHandler>,
    poll_set: PollSet<c_int>,
}

impl SignalHandlers {
    pub(crate) fn new() -> io::Result<Self> {
        let mut handlers = HashMap::with_capacity(SIGNALS.len());
        let mut poll_set = PollSet::new();
        for &signal in SIGNALS {
            let handler = SignalHandler::new(signal)?;
            poll_set.add_fd_read(signal, &handler);
            handlers.insert(signal, handler);
        }

        Ok(Self { handlers, poll_set })
    }

    pub(crate) fn get_mut(&mut self, signal: c_int) -> Option<&mut SignalHandler> {
        self.handlers.get_mut(&signal)
    }

    pub(crate) fn poll(&mut self) -> io::Result<Vec<SignalInfo>> {
        let signals = self.poll_set.poll()?;
        let mut infos = Vec::with_capacity(signals.len());

        for signal in signals {
            infos.push(self.get_mut(signal).unwrap().recv()?);
        }

        Ok(infos)
    }
}
