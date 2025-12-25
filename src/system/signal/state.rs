use std::io;

use crate::cutils::cerr;
use crate::log::dev_debug;
use crate::system::make_zeroed_sigaction;
use crate::system::signal::signal_name;

use super::{SignalNumber,consts::*};

struct State
{
    sa: libc::sigaction,
    restore: bool,
}

pub(crate) struct SignalsState
{
    sa_handlers: [State;Self::SAVED_SIGNALS.len()],
}

impl SignalsState {
    ///SIGKILL SIGSTOP are not included since forbidden
    const SAVED_SIGNALS : [SignalNumber; 14] = [
        SIGINT, SIGQUIT, SIGTSTP, SIGTERM, SIGHUP, SIGALRM, SIGPIPE, SIGUSR1,
        SIGUSR2, SIGCHLD, SIGCONT, SIGWINCH, SIGTTIN, SIGTTOU];

    pub(crate) fn save() -> io::Result<Self>{
        let mut state_array = std::array::from_fn(|_|{
            State{sa: make_zeroed_sigaction(), restore:false}
        });

        for (idx, &signal) in Self::SAVED_SIGNALS.iter().enumerate(){
            let state = &mut state_array[idx];

            // safety: `signal` is a constant value and a valid signal
            // second parameter can be null
            // `state.sa` is a valid already initialized `sigaction`
            cerr(unsafe{libc::sigaction(signal, std::ptr::null(), &mut state.sa)})?;
        }

        Ok(Self { sa_handlers: state_array })
    }

    pub(super) fn updated(&mut self, signal: SignalNumber) -> io::Result<()>{
        match Self::SAVED_SIGNALS.iter().enumerate().find(|&(_,&s)| s == signal){
            None => Err(io::Error::new(io::ErrorKind::NotFound, "invalid signal")),
            Some((idx,_)) => {
                self.sa_handlers[idx].restore = true;
                Ok(())
            },
        }
    }

    pub(crate) fn restore(&mut self) -> io::Result<()>{
        for (idx, state)in self.sa_handlers.iter_mut().enumerate(){
            if  state.restore {
                let signal = Self::SAVED_SIGNALS[idx];
                dev_debug!("restoring updated signal: {}", signal_name(signal));
                // safety: `signal` is a constant value and a valid signal
                // `sa` is a valid already initialized sigaction
                // Third parameter can be NULL
                cerr(unsafe{libc::sigaction(signal, &state.sa, std::ptr::null_mut())})?;
                state.restore = false;
            }
        }
        Ok(())
    }
}
