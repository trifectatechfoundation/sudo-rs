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
    const VERSION_OFFSET: u64 = Self::MAGIC_NUM.to_le_bytes().len() as u64;
    const FIRST_RECORD_OFFSET: u64 =
        Self::VERSION_OFFSET + Self::FILE_VERSION.to_le_bytes().len() as u64;

    /// Create a new SessionRecordFile from the given i/o stream.
    /// Timestamps in this file are considered valid if they were created or
    /// updated at most `timeout` time ago.
    pub fn new(io: IO, timeout: Duration) -> io::Result<SessionRecordFile<IO>> {
        let mut session_records = SessionRecordFile { io, timeout };

        // match the magic number, otherwise reset the file
        match session_records.read_magic()? {
            Some(magic) if magic == Self::MAGIC_NUM => (),
            x => {
                if let Some(_magic) = x {
                    // TODO: warn about invalid magic number
                    eprintln!("Session ts file is invalid, resetting");
                }

                session_records.init(Self::VERSION_OFFSET, true)?;
            }
        }

        // match the file version
        match session_records.read_version()? {
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

                session_records.init(Self::FIRST_RECORD_OFFSET, true)?;
            }
        }

        // we are ready to read records
        Ok(session_records)
    }

    /// Read the magic number from the input stream
    fn read_magic(&mut self) -> io::Result<Option<u16>> {
        let mut magic_bytes = [0; std::mem::size_of::<u16>()];
        match self.io.read_exact(&mut magic_bytes) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(e),
            Ok(()) => Ok(Some(u16::from_le_bytes(magic_bytes))),
        }
    }

    /// Read the version number from the input stream
    fn read_version(&mut self) -> io::Result<Option<u16>> {
        let mut version_bytes = [0; std::mem::size_of::<u16>()];
        match self.io.read_exact(&mut version_bytes) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(e),
            Ok(()) => Ok(Some(u16::from_le_bytes(version_bytes))),
        }
    }

    /// Initialize a new empty stream. If the stream/file was already filled
    /// before it will be truncated.
    fn init(&mut self, offset: u64, must_lock: bool) -> io::Result<()> {
        // lock the file to indicate that we are currently writing to it
        if must_lock {
            self.io.lock_exclusive()?;
        }
        self.io.set_len(0)?;
        self.io.seek(io::SeekFrom::Start(0))?;
        self.io.write_all(&Self::MAGIC_NUM.to_le_bytes())?;
        self.io.write_all(&Self::FILE_VERSION.to_le_bytes())?;
        self.io.seek(io::SeekFrom::Start(offset))?;
        if must_lock {
            self.io.unlock()?;
        }
        Ok(())
    }

    /// Read the next record and keep note of the start and end positions in the file of that record
    ///
    /// This method assumes that the file is already exclusively locked.
    fn next_record(&mut self) -> io::Result<Option<SessionRecord>> {
        // record the position at which this record starts (including size bytes)
        let mut record_length_bytes = [0; std::mem::size_of::<u16>()];

        let curr_pos = self.io.stream_position()?;

        // if eof occurs here we assume we reached the end of the file
        let record_length = match self.io.read_exact(&mut record_length_bytes) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
            Ok(()) => u16::from_le_bytes(record_length_bytes),
        };

        // special case when record_length is zero
        if record_length == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Found empty record",
            ));
        }

        let mut buf = vec![0; record_length as usize];
        match self.io.read_exact(&mut buf) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                // there was half a record here, we clear the rest of the file
                self.io.set_len(curr_pos as usize)?;
                eprintln!("Found incomplete record, removing record");
                return Ok(None);
            }
            Err(e) => return Err(e),
            Ok(()) => (),
        }

        // we now try and decode the data read into a session record
        match SessionRecord::from_bytes(&buf) {
            Err(_) => {
                // any error assumes that this file is nonsense from this point
                // onwards, so we clear the file up to the start of this record
                eprintln!("Found invalid record, clearing rest of the file");

                self.io.set_len(curr_pos as usize)?;
                Ok(None)
            }
            Ok(record) => Ok(Some(record)),
        }
    }

    /// Try and find a record for the given limit and target user and update
    /// that record time to the current time. This will not create a new record
    /// when one is not found. A record will only be updated if it is still
    /// valid at this time.
    pub fn touch(
        &mut self,
        record_limit: RecordLimit,
        target_user: UserId,
    ) -> io::Result<TouchResult> {
        // lock the file to indicate that we are currently in a writing operation
        self.io.lock_exclusive()?;
        self.seek_to_first_record()?;
        while let Some(record) = self.next_record()? {
            if record.matches(&record_limit, target_user) {
                let now = SystemTime::now()?;
                if record.written_between(now - self.timeout, now) {
                    // move back 16 bytes (size of the timestamp) and overwrite with the latest time
                    self.io.seek(io::SeekFrom::Current(-16))?;
                    let new_time = SystemTime::now()?;
                    new_time.encode(&mut self.io)?;
                    self.io.unlock()?;
                    return Ok(TouchResult::Updated {
                        old_time: record.timestamp,
                        new_time,
                    });
                } else {
                    self.io.unlock()?;
                    return Ok(TouchResult::Outdated {
                        time: record.timestamp,
                    });
                }
            }
        }

        self.io.unlock()?;
        Ok(TouchResult::NotFound)
    }

    /// Create a new record for the given limit and target user.
    /// If there is an existing record that matches the limit and target user,
    /// then that record will be updated.
    pub fn create(
        &mut self,
        record_limit: RecordLimit,
        target_user: UserId,
    ) -> io::Result<CreateResult> {
        // lock the file to indicate that we are currently writing to it
        self.io.lock_exclusive()?;
        self.seek_to_first_record()?;
        while let Some(record) = self.next_record()? {
            if record.matches(&record_limit, target_user) {
                self.io.seek(io::SeekFrom::Current(-16))?;
                let new_time = SystemTime::now()?;
                new_time.encode(&mut self.io)?;
                self.io.unlock()?;
                return Ok(CreateResult::Updated {
                    old_time: record.timestamp,
                    new_time,
                });
            }
        }

        // record was not found in the list so far, create a new one
        let record = SessionRecord::new(record_limit, target_user)?;

        // make sure we really are at the end of the file
        self.io.seek(io::SeekFrom::End(0))?;

        self.write_record(&record)?;
        self.io.unlock()?;

        Ok(CreateResult::Created {
            time: record.timestamp,
        })
    }

    /// Completely resets the entire file and removes all records.
    pub fn reset(&mut self) -> io::Result<()> {
        self.init(0, true)
    }

    /// Write a new record at the current position in the file.
    fn write_record(&mut self, record: &SessionRecord) -> io::Result<()> {
        // convert the new record to byte representation and make sure that it fits
        let bytes = record.as_bytes()?;
        let record_length = bytes.len();
        if record_length > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A record with an unexpectedly large size was created",
            ));
        }
        let record_length = record_length as u16; // store as u16

        // write the record
        self.io.write_all(&record_length.to_le_bytes())?;
        self.io.write_all(&bytes)?;

        Ok(())
    }

    /// Move to where the first record starts.
    fn seek_to_first_record(&mut self) -> io::Result<()> {
        self.io
            .seek(io::SeekFrom::Start(Self::FIRST_RECORD_OFFSET))?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TouchResult {
    /// The record was found and within the timeout, and it was refreshed
    Updated {
        old_time: SystemTime,
        new_time: SystemTime,
    },
    /// A record was found, but it was no longer valid
    Outdated { time: SystemTime },
    /// A record was not found that matches the input
    NotFound,
}

pub enum CreateResult {
    /// The record was found and it was refreshed
    Updated {
        old_time: SystemTime,
        new_time: SystemTime,
    },
    /// A new record was created and was set to the time returned
    Created { time: SystemTime },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordLimit {
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
            RecordLimit::TTY {
                tty_device,
                session_pid,
                init_time,
            } => {
                target.write_all(&[1u8])?;
                let b = tty_device.to_le_bytes();
                target.write_all(&b)?;
                let b = session_pid.to_le_bytes();
                target.write_all(&b)?;
                init_time.encode(target)?;
            }
            RecordLimit::PPID {
                group_pid,
                init_time,
            } => {
                target.write_all(&[2u8])?;
                let b = group_pid.to_le_bytes();
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
                let mut buf = [0; std::mem::size_of::<libc::dev_t>()];
                from.read_exact(&mut buf)?;
                let tty_device = libc::dev_t::from_le_bytes(buf);
                let mut buf = [0; std::mem::size_of::<libc::pid_t>()];
                from.read_exact(&mut buf)?;
                let session_pid = libc::pid_t::from_le_bytes(buf);
                let init_time = SystemTime::decode(from)?;
                Ok(RecordLimit::TTY {
                    tty_device,
                    session_pid,
                    init_time,
                })
            }
            2 => {
                let mut buf = [0; std::mem::size_of::<libc::pid_t>()];
                from.read_exact(&mut buf)?;
                let group_pid = libc::pid_t::from_le_bytes(buf);
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
#[derive(Debug, PartialEq, Eq)]
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

        let buf = self.auth_user.to_le_bytes();
        target.write_all(&buf)?;

        self.timestamp.encode(target)?;

        Ok(())
    }

    /// Decode a record from the given stream
    fn decode(from: &mut impl Read) -> std::io::Result<SessionRecord> {
        let limit = RecordLimit::decode(from)?;
        let mut buf = [0; std::mem::size_of::<libc::uid_t>()];
        from.read_exact(&mut buf)?;
        let auth_user = libc::uid_t::from_le_bytes(buf);
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

    /// Returns true if this record was written somewhere in the time range
    /// between `early_time` (inclusive) and `later_time` (inclusive), where
    /// early timestamp may not be later than the later timestamp.
    pub fn written_between(&self, early_time: SystemTime, later_time: SystemTime) -> bool {
        early_time <= later_time && self.timestamp >= early_time && self.timestamp <= later_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl SetLength for Cursor<Vec<u8>> {
        fn set_len(&mut self, new_len: usize) -> io::Result<()> {
            self.get_mut().truncate(new_len);
            while self.get_mut().len() < new_len {
                self.get_mut().push(0);
            }
            Ok(())
        }
    }

    impl SetLength for Cursor<&mut Vec<u8>> {
        fn set_len(&mut self, new_len: usize) -> io::Result<()> {
            self.get_mut().truncate(new_len);
            while self.get_mut().len() < new_len {
                self.get_mut().push(0);
            }
            Ok(())
        }
    }

    #[test]
    fn can_encode_and_decode() {
        let tty_sample = SessionRecord::new(
            RecordLimit::TTY {
                tty_device: 10,
                session_pid: 42,
                init_time: SystemTime::now().unwrap() - Duration::seconds(150),
            },
            999,
        )
        .unwrap();

        let mut bytes = tty_sample.as_bytes().unwrap();
        let decoded = SessionRecord::from_bytes(&bytes).unwrap();
        assert_eq!(tty_sample, decoded);

        // we provide some invalid input
        assert!(SessionRecord::from_bytes(&bytes[1..]).is_err());

        // we have remaining input after decoding
        bytes.push(0);
        assert!(SessionRecord::from_bytes(&bytes).is_err());

        let ppid_sample = SessionRecord::new(
            RecordLimit::PPID {
                group_pid: 42,
                init_time: SystemTime::now().unwrap(),
            },
            123,
        )
        .unwrap();
        let bytes = ppid_sample.as_bytes().unwrap();
        let decoded = SessionRecord::from_bytes(&bytes).unwrap();
        assert_eq!(ppid_sample, decoded);
    }

    #[test]
    fn timestamp_record_matches_works() {
        let init_time = SystemTime::now().unwrap();
        let limit = RecordLimit::TTY {
            tty_device: 12,
            session_pid: 1234,
            init_time,
        };

        let tty_sample = SessionRecord::new(limit, 675).unwrap();

        assert!(tty_sample.matches(&limit, 675));
        assert!(!tty_sample.matches(&limit, 789));
        assert!(!tty_sample.matches(
            &RecordLimit::TTY {
                tty_device: 20,
                session_pid: 1234,
                init_time
            },
            675
        ));
        assert!(!tty_sample.matches(
            &RecordLimit::PPID {
                group_pid: 42,
                init_time
            },
            675
        ));

        // make sure time is different
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(!tty_sample.matches(
            &RecordLimit::TTY {
                tty_device: 12,
                session_pid: 1234,
                init_time: SystemTime::now().unwrap()
            },
            675
        ));
    }

    #[test]
    fn timestamp_record_written_between_works() {
        let some_time = SystemTime::now().unwrap() + Duration::minutes(100);
        let limit = RecordLimit::TTY {
            tty_device: 12,
            session_pid: 1234,
            init_time: some_time,
        };
        let sample = SessionRecord::init(limit, 1234, some_time);

        let dur = Duration::seconds(30);

        assert!(sample.written_between(some_time, some_time));
        assert!(sample.written_between(some_time, some_time + dur));
        assert!(sample.written_between(some_time - dur, some_time));
        assert!(!sample.written_between(some_time + dur, some_time - dur));
        assert!(!sample.written_between(some_time + dur, some_time + dur + dur));
        assert!(!sample.written_between(some_time - dur - dur, some_time - dur));
    }

    #[test]
    fn session_record_file_header_checks() {
        // valid header should remain valid
        let mut v = vec![0xD0, 0x50, 0x01, 0x00];
        let c = Cursor::new(&mut v);
        let timeout = Duration::seconds(30);
        assert!(SessionRecordFile::new(c, timeout).is_ok());
        assert_eq!(&v[..], &[0xD0, 0x50, 0x01, 0x00]);

        // invalid headers should be corrected
        let mut v = vec![0xAB, 0xBA];
        let c = Cursor::new(&mut v);
        assert!(SessionRecordFile::new(c, timeout).is_ok());
        assert_eq!(&v[..], &[0xD0, 0x50, 0x01, 0x00]);

        // empty header should be filled in
        let mut v = vec![];
        let c = Cursor::new(&mut v);
        assert!(SessionRecordFile::new(c, timeout).is_ok());
        assert_eq!(&v[..], &[0xD0, 0x50, 0x01, 0x00]);

        // invalid version should reset file
        let mut v = vec![0xD0, 0x50, 0xAB, 0xBA, 0x0, 0x0];
        let c = Cursor::new(&mut v);
        assert!(SessionRecordFile::new(c, timeout).is_ok());
        assert_eq!(&v[..], &[0xD0, 0x50, 0x01, 0x00]);
    }

    #[test]
    fn can_create_and_update_valid_file() {
        let timeout = Duration::seconds(30);
        let mut data = vec![];
        let c = Cursor::new(&mut data);
        let mut srf = SessionRecordFile::new(c, timeout).unwrap();
        let tty_limit = RecordLimit::TTY {
            tty_device: 0,
            session_pid: 0,
            init_time: SystemTime::new(0, 0),
        };
        let target_user = 2424;
        let res = srf.create(tty_limit, target_user).unwrap();
        let CreateResult::Created { time } = res else {
            panic!("Expected record to be created");
        };

        std::thread::sleep(std::time::Duration::from_millis(1));
        let second = srf.touch(tty_limit, target_user).unwrap();
        let TouchResult::Updated { old_time, new_time } = second else {
            panic!("Expected record to be updated");
        };
        assert_eq!(time, old_time);
        assert_ne!(old_time, new_time);

        std::thread::sleep(std::time::Duration::from_millis(1));
        let res = srf.create(tty_limit, target_user).unwrap();
        let CreateResult::Updated { .. } = res else {
            panic!("Expected record to be updated");
        };

        // reset the file
        assert!(srf.reset().is_ok());

        // after all this the data should be just an empty header
        assert_eq!(&data, &[0xD0, 0x50, 0x01, 0x00]);
    }
}
