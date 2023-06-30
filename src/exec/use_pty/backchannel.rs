use std::{
    ffi::c_int,
    io::{self, Read, Write},
    mem::size_of,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
};

use crate::exec::signal_fmt;
use crate::system::interface::ProcessId;

use super::CommandStatus;

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

        #[cfg(debug_assertions)]
        {
            sock1.set_nonblocking(true)?;
            sock2.set_nonblocking(true)?;
        }

        Ok(Self {
            parent: ParentBackchannel { socket: sock1 },
            monitor: MonitorBackchannel { socket: sock2 },
        })
    }
}

pub(super) enum ParentMessage {
    IoError(c_int),
    CommandStatus(CommandStatus),
    CommandPid(ProcessId),
    ShortRead,
}

impl ParentMessage {
    const LEN: usize = PREFIX_LEN + PARENT_DATA_LEN;
    const IO_ERROR: Prefix = 0;
    const CMD_STAT_EXIT: Prefix = 1;
    const CMD_STAT_TERM: Prefix = 2;
    const CMD_STAT_STOP: Prefix = 3;
    const CMD_PID: Prefix = 4;
    const SHORT_READ: Prefix = 5;

    fn from_parts(prefix: Prefix, data: ParentData) -> Self {
        match prefix {
            Self::IO_ERROR => Self::IoError(data),
            Self::CMD_STAT_EXIT => Self::CommandStatus(CommandStatus::Exit(data)),
            Self::CMD_STAT_TERM => Self::CommandStatus(CommandStatus::Term(data)),
            Self::CMD_STAT_STOP => Self::CommandStatus(CommandStatus::Stop(data)),
            Self::CMD_PID => Self::CommandPid(data),
            Self::SHORT_READ => Self::ShortRead,
            _ => unreachable!(),
        }
    }

    fn to_parts(&self) -> (Prefix, ParentData) {
        let prefix = match self {
            ParentMessage::IoError(_) => Self::IO_ERROR,
            ParentMessage::CommandStatus(CommandStatus::Exit(_)) => Self::CMD_STAT_EXIT,
            ParentMessage::CommandStatus(CommandStatus::Term(_)) => Self::CMD_STAT_TERM,
            ParentMessage::CommandStatus(CommandStatus::Stop(_)) => Self::CMD_STAT_STOP,
            ParentMessage::CommandPid(_) => Self::CMD_PID,
            ParentMessage::ShortRead => Self::SHORT_READ,
        };

        let data = match self {
            ParentMessage::IoError(data) | ParentMessage::CommandPid(data) => *data,
            ParentMessage::CommandStatus(status) => match status {
                CommandStatus::Exit(data)
                | CommandStatus::Term(data)
                | CommandStatus::Stop(data) => *data,
            },
            ParentMessage::ShortRead => 0,
        };

        (prefix, data)
    }
}

impl TryFrom<io::Error> for ParentMessage {
    type Error = io::Error;

    fn try_from(err: io::Error) -> Result<Self, Self::Error> {
        err.raw_os_error()
            .map(Self::IoError)
            .or_else(|| (err.kind() == io::ErrorKind::UnexpectedEof).then_some(Self::ShortRead))
            .ok_or(err)
    }
}

impl From<CommandStatus> for ParentMessage {
    fn from(status: CommandStatus) -> Self {
        Self::CommandStatus(status)
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

        self.socket.write_all(&buf).map_err(|err| {
            debug_assert!(err.kind() != io::ErrorKind::WouldBlock);
            err
        })
    }

    /// Receive a [`ParentMessage`].
    ///
    /// Calling this method will block until the socket is ready for reading.
    pub(super) fn recv(&mut self) -> io::Result<ParentMessage> {
        let mut buf = [0; ParentMessage::LEN];

        self.socket.read_exact(&mut buf).map_err(|err| {
            debug_assert!(err.kind() != io::ErrorKind::WouldBlock);
            err
        })?;

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
#[derive(PartialEq, Eq)]
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

impl std::fmt::Debug for MonitorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecCommand => "ExecCommand".fmt(f),
            &Self::Signal(signal) => write!(f, "Signal({})", signal_fmt(signal)),
        }
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

        self.socket.write_all(&buf).map_err(|err| {
            debug_assert!(err.kind() != io::ErrorKind::WouldBlock);
            err
        })
    }

    /// Receive a [`MonitorMessage`].
    ///
    /// Calling this method will block until the socket is ready for reading.
    pub(super) fn recv(&mut self) -> io::Result<MonitorMessage> {
        let mut buf = [0; MonitorMessage::LEN];

        self.socket.read_exact(&mut buf).map_err(|err| {
            debug_assert!(err.kind() != io::ErrorKind::WouldBlock);
            err
        })?;

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
