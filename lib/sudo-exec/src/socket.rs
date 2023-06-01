use std::{
    ffi::c_int,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
};

use sudo_system::{interface::ProcessId, WaitStatus};

pub(crate) fn socketpair() -> io::Result<(Backchannel, Backchannel)> {
    let (pty, mon) = UnixStream::pair()?;

    Ok((Backchannel { socket: pty }, Backchannel { socket: mon }))
}

pub(crate) struct Backchannel {
    socket: UnixStream,
}

impl Backchannel {
    pub(crate) fn send_status(&mut self, status: &CommandStatus) -> io::Result<()> {
        let mut buf = [0; std::mem::size_of::<CommandStatusKind>() + std::mem::size_of::<c_int>()];
        buf[0] = status.kind as u8;
        buf[1..].copy_from_slice(&status.data.to_ne_bytes());

        self.socket.write_all(&buf)?;
        Ok(())
    }

    pub(crate) fn receive_status(&mut self) -> io::Result<CommandStatus> {
        let mut buf = [0; std::mem::size_of::<CommandStatusKind>() + std::mem::size_of::<c_int>()];
        self.socket.read_exact(&mut buf)?;
        let kind = match buf[0] {
            0 => CommandStatusKind::Invalid,
            1 => CommandStatusKind::Errno,
            2 => CommandStatusKind::WStatus,
            3 => CommandStatusKind::Signo,
            4 => CommandStatusKind::Pid,
            kind => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid command status kind {kind}"),
                ));
            }
        };

        Ok(CommandStatus {
            kind,
            data: c_int::from_ne_bytes(buf[1..].try_into().unwrap()),
        })
    }
}

impl AsRawFd for Backchannel {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

#[derive(Clone)]
pub(crate) struct CommandStatus {
    kind: CommandStatusKind,
    data: c_int,
}

impl Default for CommandStatus {
    fn default() -> Self {
        Self {
            kind: CommandStatusKind::Invalid,
            data: 0,
        }
    }
}

impl std::fmt::Debug for CommandStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            CommandStatusKind::Invalid => write!(f, "Invalid"),
            CommandStatusKind::Errno => write!(f, "Errno({})", self.data),
            CommandStatusKind::WStatus => write!(f, "WStatus({})", self.data),
            CommandStatusKind::Signo => write!(f, "Signo({})", self.data),
            CommandStatusKind::Pid => write!(f, "Pid({})", self.data),
        }
    }
}

impl From<WaitStatus> for CommandStatus {
    fn from(wait_status: WaitStatus) -> Self {
        Self {
            kind: CommandStatusKind::WStatus,
            data: wait_status.as_raw(),
        }
    }
}

impl CommandStatus {
    pub(crate) fn is_invalid(&self) -> bool {
        self.kind == CommandStatusKind::Invalid
    }

    pub(crate) fn take(&mut self) -> Self {
        std::mem::take(self)
    }

    pub(crate) fn from_pid(pid: ProcessId) -> Self {
        Self {
            kind: CommandStatusKind::Pid,
            data: pid,
        }
    }

    pub(crate) fn from_signal(signal: c_int) -> Self {
        Self {
            kind: CommandStatusKind::Signo,
            data: signal,
        }
    }

    pub(crate) fn from_io_error(err: &io::Error) -> Self {
        Self {
            kind: CommandStatusKind::Errno,
            data: err.raw_os_error().unwrap_or(-1),
        }
    }

    pub(crate) fn command_pid(&self) -> Option<ProcessId> {
        (self.kind == CommandStatusKind::Pid).then_some(self.data)
    }

    pub(crate) fn monitor_err(&self) -> Option<c_int> {
        (self.kind == CommandStatusKind::Errno).then_some(self.data)
    }

    pub(crate) fn wait(&self) -> Option<c_int> {
        (self.kind == CommandStatusKind::WStatus).then_some(self.data)
    }

    pub(crate) fn signal(&self) -> Option<c_int> {
        (self.kind == CommandStatusKind::Signo).then_some(self.data)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum CommandStatusKind {
    Invalid = 0,
    Errno = 1,
    WStatus = 2,
    Signo = 3,
    Pid = 4,
}
