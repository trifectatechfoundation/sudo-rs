use std::{
    fs::File,
    io::{self, Cursor, Read, Seek, Write},
    path::PathBuf,
};

use crate::{
    audit::secure_open_cookie_file,
    file::Lockable,
    interface::UserId,
    time::{Duration, SystemTime},
};

/// Truncates or extends the underlying data
pub trait SetLength {
    /// After this is called, the underlying data will either be truncated
    /// up to new_len bytes, or it will have been extended by zero bytes up to
    /// new_len.
    fn set_len(&mut self, new_len: usize) -> io::Result<()>;
}

impl SetLength for File {
    fn set_len(&mut self, new_len: usize) -> io::Result<()> {
        File::set_len(self, new_len as u64)
    }
}

impl SetLength for Vec<u8> {
    fn set_len(&mut self, new_len: usize) -> io::Result<()> {
        self.truncate(new_len);
        while self.len() < new_len {
            self.push(0);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SessionRecordFile<IO> {
    io: IO,
    timeout: Duration,
}

impl SessionRecordFile<File> {
    const BASE_PATH: &str = "/var/run/sudo-rs/ts";

    pub fn open_for_user(user: &str, timeout: Duration) -> io::Result<SessionRecordFile<File>> {
        let mut path = PathBuf::from(Self::BASE_PATH);
        path.push(user);
        SessionRecordFile::new(secure_open_cookie_file(&path)?, timeout)
    }
}

impl<IO: Read + Write + Seek + SetLength + Lockable> SessionRecordFile<IO> {
    const FILE_VERSION: u16 = 1;
    const MAGIC_NUM: u16 = 0x50D0;
    const VERSION_OFFSET: u64 = Self::MAGIC_NUM.to_ne_bytes().len() as u64;
    const FIRST_RECORD_OFFSET: u64 =
        Self::VERSION_OFFSET + Self::FILE_VERSION.to_ne_bytes().len() as u64;

    /// Create a new SessionRecordFile from the given i/o stream.
    /// Timestamps in this file are considered valid if they were created or
    /// updated at most `timeout` time ago.
    pub fn new(mut io: IO, timeout: Duration) -> io::Result<SessionRecordFile<IO>> {
        // match the magic number, otherwise reset the file
        match Self::read_magic(&mut io)? {
            Some(magic) if magic == Self::MAGIC_NUM => (),
            x => {
                if let Some(_magic) = x {
                    // TODO: warn about invalid magic number
                    eprintln!("Session ts file is invalid, resetting");
                }

                Self::init(&mut io, Self::VERSION_OFFSET)?;
            }
        }

        // match the file version
        match Self::read_version(&mut io)? {
            Some(v) if v == Self::FILE_VERSION => (),
            x => {
                if let Some(v) = x {
                    // TODO: warn about incompatible version _v
                    eprintln!("Session ts file has invalid version {v}, this sudo-rs only supports version {}, resetting", Self::FILE_VERSION)
                } else {
                    // TODO: warn about an invalid file
                    eprintln!(
                        "Session ts file did not contain file version information, resetting"
                    );
                }

                Self::init(&mut io, Self::FIRST_RECORD_OFFSET)?;
            }
        }

        // we are ready to read records
        Ok(SessionRecordFile { io, timeout })
    }

    /// Read the magic number from the input stream
    fn read_magic(file: &mut IO) -> io::Result<Option<u16>> {
        let mut magic_bytes = [0; std::mem::size_of::<u16>()];
        match file.read_exact(&mut magic_bytes) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(e),
            Ok(()) => Ok(Some(u16::from_ne_bytes(magic_bytes))),
        }
    }

    /// Read the version number from the input stream
    fn read_version(file: &mut IO) -> io::Result<Option<u16>> {
        let mut version_bytes = [0; std::mem::size_of::<u16>()];
        match file.read_exact(&mut version_bytes) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(e),
            Ok(()) => Ok(Some(u16::from_ne_bytes(version_bytes))),
        }
    }

    /// Initialize a new empty stream. If the stream/file was already filled
    /// before it will be truncated.
    fn init(file: &mut IO, offset: u64) -> io::Result<()> {
        // lock the file to indicate that we are currently writing to it
        file.lock_exclusive()?;
        file.set_len(0)?;
        file.seek(io::SeekFrom::Start(0))?;
        file.write_all(&Self::MAGIC_NUM.to_ne_bytes())?;
        file.write_all(&Self::FILE_VERSION.to_ne_bytes())?;
        file.seek(io::SeekFrom::Start(offset))?;
        file.unlock()?;
        Ok(())
    }

    /// Read the next record and keep note of the start and end positions in the file of that record
    fn next_record(&mut self) -> io::Result<Option<(RecordPosition, SessionRecord)>> {
        // record the position at which this record starts (including size bytes)
        let start = self.io.stream_position()?;
        let mut record_length_bytes = [0; std::mem::size_of::<u16>()];

        // if eof occurs here we assume we reached the end of the file
        let record_length = match self.io.read_exact(&mut record_length_bytes) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
            Ok(()) => u16::from_ne_bytes(record_length_bytes),
        };

        // special case when record_length is zero
        if record_length == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Found empty record",
            ));
        }

        let mut buf = vec![0; record_length as usize];
        // eof is allowed to fail here because the file will be invalid if this happens
        self.io.read_exact(&mut buf)?;
        let record = SessionRecord::from_bytes(&buf)?;

        // record the position at which this record ends
        let end = self.io.stream_position()?;

        Ok(Some((RecordPosition { start, end }, record)))
    }

    /// Try and find a record for the given limit and target user and update
    /// that record time to the current time. This will not create a new record
    /// when one is not found. A record will only be updated if it is still
    /// valid at this time.
    pub fn touch(
        &mut self,
        record_limit: RecordLimit,
        target_user: UserId,
    ) -> io::Result<RecordMatch> {
        // lock the file to indicate that we are currently in a writing operation
        self.io.lock_exclusive()?;
        self.seek_to_first_record()?;
        while let Some((_pos, record)) = self.next_record()? {
            if record.matches(&record_limit, target_user) {
                if record.written_after(SystemTime::now()? - self.timeout) {
                    // move back 16 bytes (size of the timestamp) and overwrite with the latest time
                    self.io.seek(io::SeekFrom::Current(-16))?;
                    let new_time = SystemTime::now()?;
                    new_time.encode(&mut self.io)?;
                    self.io.unlock()?;
                    return Ok(RecordMatch::Updated {
                        time: record.timestamp,
                        new_time,
                    });
                } else {
                    self.io.unlock()?;
                    return Ok(RecordMatch::Outdated {
                        time: record.timestamp,
                    });
                }
            }
        }

        self.io.unlock()?;
        Ok(RecordMatch::NotFound)
    }

    /// Find a record that matches the given limit and target user and return
    /// that record. This will not create a new record when one is not found.
    pub fn find(
        &mut self,
        record_limit: RecordLimit,
        target_user: UserId,
    ) -> io::Result<RecordMatch> {
        // lock the file to indicate that we are currently reading from it and
        // no writing operations should take place
        self.io.lock_shared()?;
        self.seek_to_first_record()?;
        while let Some((_pos, record)) = self.next_record()? {
            if record.matches(&record_limit, target_user) {
                if record.written_after(SystemTime::now()? - self.timeout) {
                    self.io.unlock()?;
                    return Ok(RecordMatch::Found {
                        time: record.timestamp,
                    });
                } else {
                    self.io.unlock()?;
                    return Ok(RecordMatch::Outdated {
                        time: record.timestamp,
                    });
                }
            }
        }

        self.io.unlock()?;
        Ok(RecordMatch::NotFound)
    }

    /// Create a new record for the given limit and target user.
    pub fn create_or_update(
        &mut self,
        record_limit: RecordLimit,
        target_user: UserId,
    ) -> io::Result<RecordMatch> {
        // lock the file to indicate that we are currently writing to it
        self.io.lock_exclusive()?;
        self.seek_to_first_record()?;
        while let Some((_pos, record)) = self.next_record()? {
            if record.matches(&record_limit, target_user) {
                self.io.seek(io::SeekFrom::Current(-16))?;
                let new_time = SystemTime::now()?;
                new_time.encode(&mut self.io)?;
                self.io.unlock()?;
                return Ok(RecordMatch::Updated {
                    time: record.timestamp,
                    new_time,
                });
            }
        }

        // record was not found in the list so far, create a new one
        let record = SessionRecord::new(record_limit, target_user)?;

        // make sure we really are at the end of the file
        self.io.seek(io::SeekFrom::End(0))?;

        // convert the new record to byte representation and make sure that it fits
        let bytes = record.as_bytes()?;
        let record_length = bytes.len();
        if record_length > u16::MAX as usize {
            self.io.unlock()?;
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A record with an unexpectedly large size was created",
            ));
        }
        let record_length = record_length as u16; // store as u16

        // write the record
        self.io.write_all(&record_length.to_ne_bytes())?;
        self.io.write_all(&bytes)?;
        self.io.unlock()?;

        Ok(RecordMatch::Found {
            time: record.timestamp,
        })
    }

    pub fn clear(&mut self) -> io::Result<()> {
        Self::init(&mut self.io, 0)
    }

    fn seek_to_first_record(&mut self) -> io::Result<()> {
        self.io
            .seek(io::SeekFrom::Start(Self::FIRST_RECORD_OFFSET))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordPosition {
    start: u64,
    end: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RecordMatch {
    /// The record was found and within the timeout, but it was not updated
    Found { time: SystemTime },
    /// The record was found and within the timeout, and it was refreshed
    Updated {
        time: SystemTime,
        new_time: SystemTime,
    },
    /// A record was not found that matches the input
    NotFound,
    /// A record was found, but it was no longer valid
    Outdated { time: SystemTime },
    /// A record was found, but it was removed
    Removed { time: SystemTime },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordLimit {
    Global {
        init_time: SystemTime,
    },
    TTY {
        tty_device: libc::dev_t,
        session_pid: libc::pid_t,
        init_time: SystemTime,
    },
    PPID {
        group_pid: libc::pid_t,
        init_time: SystemTime,
    },
}

impl RecordLimit {
    fn encode(&self, target: &mut impl Write) -> std::io::Result<()> {
        match self {
            RecordLimit::Global { init_time } => {
                target.write_all(&[1u8])?;
                init_time.encode(target)?;
            }
            RecordLimit::TTY {
                tty_device,
                session_pid,
                init_time,
            } => {
                target.write_all(&[2u8])?;
                let b = tty_device.to_ne_bytes();
                target.write_all(&b)?;
                let b = session_pid.to_ne_bytes();
                target.write_all(&b)?;
                init_time.encode(target)?;
            }
            RecordLimit::PPID {
                group_pid,
                init_time,
            } => {
                target.write_all(&[4u8])?;
                let b = group_pid.to_ne_bytes();
                target.write_all(&b)?;
                init_time.encode(target)?;
            }
        }

        Ok(())
    }

    fn decode(from: &mut impl Read) -> std::io::Result<RecordLimit> {
        let mut buf = [0; 1];
        from.read_exact(&mut buf)?;
        match buf[0] {
            1 => {
                let init_time = SystemTime::decode(from)?;
                Ok(RecordLimit::Global { init_time })
            }
            2 => {
                let mut buf = [0; std::mem::size_of::<libc::dev_t>()];
                from.read_exact(&mut buf)?;
                let tty_device = libc::dev_t::from_ne_bytes(buf);
                let mut buf = [0; std::mem::size_of::<libc::pid_t>()];
                from.read_exact(&mut buf)?;
                let session_pid = libc::pid_t::from_ne_bytes(buf);
                let init_time = SystemTime::decode(from)?;
                Ok(RecordLimit::TTY {
                    tty_device,
                    session_pid,
                    init_time,
                })
            }
            4 => {
                let mut buf = [0; std::mem::size_of::<libc::pid_t>()];
                from.read_exact(&mut buf)?;
                let group_pid = libc::pid_t::from_ne_bytes(buf);
                let init_time = SystemTime::decode(from)?;
                Ok(RecordLimit::PPID {
                    group_pid,
                    init_time,
                })
            }
            x => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unexpected limit variant discriminator: {x}"),
            )),
        }
    }
}

/// A record in the session record file
#[derive(Debug)]
pub struct SessionRecord {
    limit: RecordLimit,
    auth_user: libc::uid_t,
    timestamp: SystemTime,
}

impl SessionRecord {
    /// Create a new record that is limited to the specified limit and has `auth_user` as
    /// the target for the session.
    fn new(limit: RecordLimit, auth_user: UserId) -> io::Result<SessionRecord> {
        Ok(Self::init(limit, auth_user, SystemTime::now()?))
    }

    /// Initialize a new record with the given parameters
    fn init(limit: RecordLimit, auth_user: UserId, timestamp: SystemTime) -> SessionRecord {
        SessionRecord {
            limit,
            auth_user,
            timestamp,
        }
    }

    /// Encode a record into the given stream
    fn encode(&self, target: &mut impl Write) -> std::io::Result<()> {
        self.limit.encode(target)?;

        let buf = self.auth_user.to_ne_bytes();
        target.write_all(&buf)?;

        self.timestamp.encode(target)?;

        Ok(())
    }

    /// Decode a record from the given stream
    fn decode(from: &mut impl Read) -> std::io::Result<SessionRecord> {
        let limit = RecordLimit::decode(from)?;
        let mut buf = [0; std::mem::size_of::<libc::uid_t>()];
        from.read_exact(&mut buf)?;
        let auth_user = libc::uid_t::from_ne_bytes(buf);
        let timestamp = SystemTime::decode(from)?;
        Ok(SessionRecord::init(limit, auth_user, timestamp))
    }

    /// Convert the record to a vector of bytes for storage.
    pub fn as_bytes(&self) -> std::io::Result<Vec<u8>> {
        let mut v = vec![];
        self.encode(&mut v)?;
        Ok(v)
    }

    /// Convert the given byte slice to a session record, the byte slice must
    /// be fully consumed for this conversion to be valid.
    pub fn from_bytes(data: &[u8]) -> std::io::Result<SessionRecord> {
        let mut cursor = Cursor::new(data);
        let record = SessionRecord::decode(&mut cursor)?;
        if cursor.position() != data.len() as u64 {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Record size and record length did not match",
            ))
        } else {
            Ok(record)
        }
    }

    /// Returns true if this record matches the specified limit and is for the
    /// specified target auth user.
    pub fn matches(&self, limit: &RecordLimit, auth_user: UserId) -> bool {
        self.limit == *limit && self.auth_user == auth_user
    }

    /// Returns true if this record was written at or after the specified time
    pub fn written_after(&self, time: SystemTime) -> bool {
        self.timestamp >= time
    }
}
