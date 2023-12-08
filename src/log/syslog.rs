use core::fmt::{self, Write};
use std::ffi::CStr;

use log::{Level, Log, Metadata};

use crate::system::syslog;

pub struct Syslog;

const LIMIT: usize = 960;
const DOTDOTDOT_START: &[u8] = b"[...] ";
const DOTDOTDOT_END: &[u8] = b" [...]";
const NULL_BYTE: usize = 1; // for C string compatibility
const BUFSZ: usize = LIMIT + DOTDOTDOT_END.len() + NULL_BYTE;
const FACILITY: libc::c_int = libc::LOG_AUTH;

struct SysLogWriter {
    buffer: [u8; BUFSZ],
    cursor: usize,
    facility: libc::c_int,
    priority: libc::c_int,
}

impl SysLogWriter {
    fn new(priority: libc::c_int, facility: libc::c_int) -> Self {
        Self {
            buffer: [0; BUFSZ],
            cursor: 0,
            priority,
            facility,
        }
    }

    fn append(&mut self, bytes: &[u8]) {
        let num_bytes = bytes.len();
        self.buffer[self.cursor..self.cursor + num_bytes].copy_from_slice(bytes);
        self.cursor += num_bytes;
    }

    fn send_to_syslog(&mut self) {
        self.append(&[0]);
        let message = CStr::from_bytes_with_nul(&self.buffer[..self.cursor]).unwrap();
        syslog(self.priority, self.facility, message);
        self.cursor = 0;
    }
}

impl Write for SysLogWriter {
    fn write_str(&mut self, mut message: &str) -> fmt::Result {
        loop {
            if self.cursor + message.len() > LIMIT {
                // floor_char_boundary is currently unstable
                let mut mid = LIMIT;
                while !message.is_char_boundary(mid) {
                    mid -= 1;
                }

                mid = message[..mid]
                    .rfind(|c: char| c.is_ascii_whitespace())
                    .unwrap_or(mid);

                let left = &message[..mid];
                let right = &message[mid..];

                self.append(left.as_bytes());
                self.append(DOTDOTDOT_END);
                self.send_to_syslog();

                self.append(DOTDOTDOT_START);
                message = right;
            } else {
                self.append(message.as_bytes());

                break;
            }
        }

        Ok(())
    }
}

impl Log for Syslog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level() && metadata.level() <= log::STATIC_MAX_LEVEL
    }

    fn log(&self, record: &log::Record) {
        let priority = match record.level() {
            Level::Error => libc::LOG_ERR,
            Level::Warn => libc::LOG_WARNING,
            Level::Info => libc::LOG_INFO,
            Level::Debug => libc::LOG_DEBUG,
            Level::Trace => libc::LOG_DEBUG,
        };

        let mut writer = SysLogWriter::new(priority, FACILITY);
        let _ = write!(writer, "{}", record.args());
        writer.send_to_syslog();
    }

    fn flush(&self) {
        // pass
    }
}

#[cfg(test)]
mod tests {
    use super::Syslog;
    use log::Log;

    #[test]
    fn can_write_to_syslog() {
        let logger = Syslog;
        let record = log::Record::builder()
            .args(format_args!("Hello World!"))
            .level(log::Level::Info)
            .build();

        logger.log(&record);
    }

    #[test]
    fn can_truncate_syslog() {
        let logger = Syslog;
        let record = log::Record::builder()
            .args(format_args!("This is supposed to be a very long syslog message but idk what to write, so I am just going to tell you about the time I tried to make coffee with a teapot. So I woke up one morning and decided to make myself a pot of coffee, however after all the wild coffee parties and mishaps the coffee pot had evetually given it's last cup on a tragic morning I call wednsday. So it came to, that the only object capable of giving me hope for the day was my teapot. As I stood in the kitchen and reached for my teapot it, as if sensing the impending horrors that awaited the innocent little teapot, emmited a horse sheak of desperation. \"three hundred and seven\", it said. \"What?\" I asked with a voice of someone who clearly did not want to be bothered until he had his daily almost medically necessary dose of caffine. \"I am a teapot\" it responded with a voice of increasing forcefulness. \"I am a teapot, not a coffee pot\". It was then, in my moments of confusion that my brain finally understood, this was a teapot."))
            .level(log::Level::Info)
            .build();

        logger.log(&record);
    }

    #[test]
    fn can_truncate_syslog_with_no_spaces() {
        let logger = Syslog;
        let record = log::Record::builder()
            .args(format_args!("iwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercases"))
            .level(log::Level::Info)
            .build();

        logger.log(&record);
    }
}
