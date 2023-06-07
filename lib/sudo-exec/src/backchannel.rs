use std::{
    ffi::c_int,
    io::{self, Read, Write},
    mem::size_of,
    os::unix::{net::UnixStream, process::ExitStatusExt},
    process::ExitStatus,
};

use sudo_system::interface::ProcessId;

type Prefix = u8;
type ParentData = c_int;
const PREFIX_LEN: usize = size_of::<Prefix>();
const PARENT_DATA_LEN: usize = size_of::<ParentData>();

pub(crate) struct BackchannelPair {
    pub(crate) parent: ParentBackchannel,
    pub(crate) monitor: MonitorBackchannel,
}

impl BackchannelPair {
    pub(crate) fn new() -> io::Result<Self> {
        let (sock1, sock2) = UnixStream::pair()?;
        sock1.set_nonblocking(true)?;
        sock2.set_nonblocking(true)?;

        Ok(Self {
            parent: ParentBackchannel { socket: sock1 },
            monitor: MonitorBackchannel { socket: sock2 },
        })
    }
}

pub(crate) enum ParentEvent {
    IoError(c_int),
    CommandExit(c_int),
    CommandSignal(c_int),
    CommandPid(ProcessId),
}

impl ParentEvent {
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

    fn into_parts(self) -> (Prefix, ParentData) {
        let prefix = match self {
            ParentEvent::IoError(_) => Self::IO_ERROR,
            ParentEvent::CommandExit(_) => Self::CMD_EXIT,
            ParentEvent::CommandSignal(_) => Self::CMD_SIGNAL,
            ParentEvent::CommandPid(_) => Self::CMD_PID,
        };

        let data = match self {
            ParentEvent::IoError(data)
            | ParentEvent::CommandExit(data)
            | ParentEvent::CommandSignal(data)
            | ParentEvent::CommandPid(data) => data,
        };

        (prefix, data)
    }
}

impl<'a> From<&'a io::Error> for ParentEvent {
    fn from(err: &'a io::Error) -> Self {
        // This only panics if an error is created using `io::Error::new`.
        Self::IoError(err.raw_os_error().unwrap())
    }
}

impl From<ExitStatus> for ParentEvent {
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
pub(crate) struct ParentBackchannel {
    socket: UnixStream,
}

impl ParentBackchannel {
    /// Send a [`MonitorEvent`].
    ///
    /// Calling this method will block until the socket is ready for writing.
    pub(crate) fn send(&mut self, event: MonitorEvent) -> io::Result<()> {
        let buf: [u8; MonitorEvent::LEN] = event.into_prefix().to_ne_bytes();
        self.socket.write_all(&buf)
    }

    /// Receive a [`ParentEvent`].
    ///
    /// Calling this method will block until the socket is ready for reading.
    pub(crate) fn recv(&mut self) -> io::Result<ParentEvent> {
        let mut buf = [0; ParentEvent::LEN];
        self.socket.read_exact(&mut buf)?;

        let (prefix_buf, data_buf) = buf.split_at(PREFIX_LEN);

        let prefix = Prefix::from_ne_bytes(prefix_buf.try_into().unwrap());
        let data = ParentData::from_ne_bytes(data_buf.try_into().unwrap());

        Ok(ParentEvent::from_parts(prefix, data))
    }
}

/// Different messages exchanged between the monitor and the parent process using a [`Backchannel`].
pub(crate) enum MonitorEvent {
    ExecCommand,
}

impl MonitorEvent {
    const LEN: usize = PREFIX_LEN;
    const EXEC_CMD: Prefix = 0;

    fn from_prefix(prefix: Prefix) -> Self {
        match prefix {
            Self::EXEC_CMD => Self::ExecCommand,
            _ => unreachable!(),
        }
    }

    fn into_prefix(self) -> Prefix {
        match self {
            MonitorEvent::ExecCommand => Self::EXEC_CMD,
        }
    }
}

/// A socket use for commmunication between the monitor and the parent process.
pub(crate) struct MonitorBackchannel {
    socket: UnixStream,
}

impl MonitorBackchannel {
    /// Send a [`ParentEvent`].
    ///
    /// Calling this method will block until the socket is ready for writing.
    pub(crate) fn send(&mut self, event: ParentEvent) -> io::Result<()> {
        let mut buf = [0; ParentEvent::LEN];

        let (prefix_buf, data_buf) = buf.split_at_mut(PREFIX_LEN);
        let (prefix, data) = event.into_parts();

        prefix_buf.copy_from_slice(&prefix.to_ne_bytes());
        data_buf.copy_from_slice(&data.to_ne_bytes());

        self.socket.write_all(&buf)
    }

    /// Receive a [`MonitorEvent`].
    ///
    /// Calling this method will block until the socket is ready for reading.
    pub(crate) fn recv(&mut self) -> io::Result<MonitorEvent> {
        let mut buf = [0; MonitorEvent::LEN];
        self.socket.read_exact(&mut buf)?;

        let prefix = Prefix::from_ne_bytes(buf);

        Ok(MonitorEvent::from_prefix(prefix))
    }
}
