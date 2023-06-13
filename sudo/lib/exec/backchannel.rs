use std::{
    ffi::c_int,
    io::{self, Read, Write},
    mem::size_of,
    os::{
        fd::{AsRawFd, RawFd},
        unix::{net::UnixStream, process::ExitStatusExt},
    },
    process::ExitStatus,
};

use crate::system::{interface::ProcessId, signal::SignalNumber};

type Prefix = u8;
type ParentData = c_int;
type MonitorData = c_int;

const PREFIX_LEN: usize = size_of::<Prefix>();
const PARENT_DATA_LEN: usize = size_of::<ParentData>();
const MONITOR_DATA_LEN: usize = size_of::<MonitorData>();

pub(super) struct BackchannelPair {
    pub(super) parent: ParentBackchannel,
    pub(super) monitor: MonitorBackchannel,
}

impl BackchannelPair {
    pub(super) fn new() -> io::Result<Self> {
        let (sock1, sock2) = UnixStream::pair()?;
        sock1.set_nonblocking(true)?;
        sock2.set_nonblocking(true)?;

        Ok(Self {
            parent: ParentBackchannel { socket: sock1 },
            monitor: MonitorBackchannel { socket: sock2 },
        })
    }
}

pub(super) enum ParentMessage {
    IoError(c_int),
    CommandExit(c_int),
    CommandSignal(SignalNumber),
    CommandPid(ProcessId),
}

impl ParentMessage {
    const LEN: usize = PREFIX_LEN + PARENT_DATA_LEN;
    const IO_ERROR: Prefix = 0;
    const CMD_EXIT: Prefix = 1;
    const CMD_SIGNAL: Prefix = 2;
    const CMD_PID: Prefix = 3;

    fn from_parts(prefix: Prefix, data: ParentData) -> Self {
        match prefix {
            Self::IO_ERROR => Self::IoError(data),
            Self::CMD_EXIT => Self::CommandExit(data),
            Self::CMD_SIGNAL => Self::CommandSignal(data),
            Self::CMD_PID => Self::CommandPid(data),
            _ => unreachable!(),
        }
    }

    fn to_parts(&self) -> (Prefix, ParentData) {
        let prefix = match self {
            ParentMessage::IoError(_) => Self::IO_ERROR,
            ParentMessage::CommandExit(_) => Self::CMD_EXIT,
            ParentMessage::CommandSignal(_) => Self::CMD_SIGNAL,
            ParentMessage::CommandPid(_) => Self::CMD_PID,
        };

        let data = match self {
            ParentMessage::IoError(data)
            | ParentMessage::CommandExit(data)
            | ParentMessage::CommandSignal(data)
            | ParentMessage::CommandPid(data) => *data,
        };

        (prefix, data)
    }
}

impl From<io::Error> for ParentMessage {
    fn from(err: io::Error) -> Self {
        // This only panics if an error is created using `io::Error::new`.
        Self::IoError(err.raw_os_error().unwrap())
    }
}

impl From<ExitStatus> for ParentMessage {
    fn from(status: ExitStatus) -> Self {
        if let Some(code) = status.code() {
            Self::CommandExit(code)
        } else {
            // `ExitStatus::code` docs state that it only returns `None` if the process was
            // terminated by a signal so this should always succeed.
            Self::CommandSignal(status.signal().unwrap())
        }
    }
}

/// A socket use for commmunication between the monitor and the parent process.
pub(super) struct ParentBackchannel {
    socket: UnixStream,
}

impl ParentBackchannel {
    /// Send a [`MonitorMessage`].
    ///
    /// Calling this method will block until the socket is ready for writing.
    pub(super) fn send(&mut self, event: &MonitorMessage) -> io::Result<()> {
        let mut buf = [0; MonitorMessage::LEN];

        let (prefix_buf, data_buf) = buf.split_at_mut(PREFIX_LEN);
        let (prefix, data) = event.to_parts();

        prefix_buf.copy_from_slice(&prefix.to_ne_bytes());
        data_buf.copy_from_slice(&data.to_ne_bytes());

        self.socket.write_all(&buf)
    }

    /// Receive a [`ParentMessage`].
    ///
    /// Calling this method will block until the socket is ready for reading.
    pub(super) fn recv(&mut self) -> io::Result<ParentMessage> {
        let mut buf = [0; ParentMessage::LEN];
        self.socket.read_exact(&mut buf)?;

        let (prefix_buf, data_buf) = buf.split_at(PREFIX_LEN);

        let prefix = Prefix::from_ne_bytes(prefix_buf.try_into().unwrap());
        let data = ParentData::from_ne_bytes(data_buf.try_into().unwrap());

        Ok(ParentMessage::from_parts(prefix, data))
    }
}

impl AsRawFd for ParentBackchannel {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

/// Different messages exchanged between the monitor and the parent process using a [`ParentBackchannel`].
#[derive(Debug, PartialEq, Eq)]
pub(super) enum MonitorMessage {
    ExecCommand,
    Signal(c_int),
}

impl MonitorMessage {
    const LEN: usize = PREFIX_LEN + MONITOR_DATA_LEN;
    const EXEC_CMD: Prefix = 0;
    const SIGNAL: Prefix = 1;

    fn from_parts(prefix: Prefix, data: MonitorData) -> Self {
        match prefix {
            Self::EXEC_CMD => Self::ExecCommand,
            Self::SIGNAL => Self::Signal(data),
            _ => unreachable!(),
        }
    }

    fn to_parts(&self) -> (Prefix, MonitorData) {
        let prefix = match self {
            MonitorMessage::ExecCommand => Self::EXEC_CMD,
            MonitorMessage::Signal(_) => Self::SIGNAL,
        };

        let data = match self {
            MonitorMessage::ExecCommand => 0,
            MonitorMessage::Signal(data) => *data,
        };

        (prefix, data)
    }
}

/// A socket use for commmunication between the monitor and the parent process.
pub(super) struct MonitorBackchannel {
    socket: UnixStream,
}

impl MonitorBackchannel {
    /// Send a [`ParentMessage`].
    ///
    /// Calling this method will block until the socket is ready for writing.
    pub(super) fn send(&mut self, event: &ParentMessage) -> io::Result<()> {
        let mut buf = [0; ParentMessage::LEN];

        let (prefix_buf, data_buf) = buf.split_at_mut(PREFIX_LEN);
        let (prefix, data) = event.to_parts();

        prefix_buf.copy_from_slice(&prefix.to_ne_bytes());
        data_buf.copy_from_slice(&data.to_ne_bytes());

        self.socket.write_all(&buf)
    }

    /// Receive a [`MonitorMessage`].
    ///
    /// Calling this method will block until the socket is ready for reading.
    pub(super) fn recv(&mut self) -> io::Result<MonitorMessage> {
        let mut buf = [0; MonitorMessage::LEN];
        self.socket.read_exact(&mut buf)?;

        let (prefix_buf, data_buf) = buf.split_at(PREFIX_LEN);

        let prefix = Prefix::from_ne_bytes(prefix_buf.try_into().unwrap());
        let data = MonitorData::from_ne_bytes(data_buf.try_into().unwrap());

        Ok(MonitorMessage::from_parts(prefix, data))
    }
}

impl AsRawFd for MonitorBackchannel {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}
