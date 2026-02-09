use core::fmt::{self, Write};
use std::ffi::c_int;

use crate::log::{Level, Log};

pub struct Syslog;

mod internal {
    use crate::system::syslog;
    use std::ffi::{CStr, c_int};

    const DOTDOTDOT_START: &[u8] = b"[...] ";
    const DOTDOTDOT_END: &[u8] = b" [...]";

    const MAX_MSG_LEN: usize = 960;
    const NULL_BYTE_LEN: usize = 1; // for C string compatibility
    const BUFSZ: usize = MAX_MSG_LEN + DOTDOTDOT_END.len() + NULL_BYTE_LEN;

    pub struct SysLogMessageWriter {
        buffer: [u8; BUFSZ],
        cursor: usize,
        facility: c_int,
        priority: c_int,
    }

    // - whenever a SysLogMessageWriter has been constructed, a syslog message WILL be created
    // for one specific event; this struct functions as a low-level interface for that message
    // - the caller of the pub functions will have to take care never to `append` more bytes than
    // are `available`, or a panic will occur.
    // - the impl guarantees that after `line_break()`, there will be enough room available for at
    // least a single UTF8 character sequence (which is true since MAX_MSG_LEN >= 10)
    impl SysLogMessageWriter {
        pub fn new(priority: c_int, facility: c_int) -> Self {
            Self {
                buffer: [0; BUFSZ],
                cursor: 0,
                priority,
                facility,
            }
        }

        pub fn append(&mut self, bytes: &[u8]) {
            let num_bytes = bytes.len();
            self.buffer[self.cursor..self.cursor + num_bytes].copy_from_slice(bytes);
            self.cursor += num_bytes;
        }

        pub fn line_break(&mut self) {
            self.append(DOTDOTDOT_END);
            self.commit_to_syslog();
            self.append(DOTDOTDOT_START);
        }

        fn commit_to_syslog(&mut self) {
            self.append(&[0]);
            let message = CStr::from_bytes_with_nul(&self.buffer[..self.cursor]).unwrap();
            syslog(self.priority, self.facility, message);
            self.cursor = 0;
        }

        pub fn available(&self) -> usize {
            MAX_MSG_LEN - self.cursor
        }
    }

    impl Drop for SysLogMessageWriter {
        fn drop(&mut self) {
            self.commit_to_syslog();
        }
    }
}

use internal::SysLogMessageWriter;

/// `floor_char_boundary` is currently unstable in Rust
fn floor_char_boundary(data: &str, mut index: usize) -> usize {
    if index >= data.len() {
        return data.len();
    }
    while !data.is_char_boundary(index) {
        index -= 1;
    }

    index
}

/// This function REQUIRES that `message` is larger than `max_size` (or a panic will occur).
/// This function WILL return a non-zero result if `max_size` is large enough to fit
/// at least the first character of `message`.
fn suggested_break(message: &str, max_size: usize) -> usize {
    // method A: try to split the message in two non-empty parts on an ASCII white space character
    // method B: split on the utf8 character boundary that consumes the most data
    if let Some(pos) = message.as_bytes()[1..max_size]
        .iter()
        .rposition(|c| c.is_ascii_whitespace())
    {
        // since pos+1 contains ASCII whitespace, it acts as a valid utf8 boundary as well
        pos + 1
    } else {
        floor_char_boundary(message, max_size)
    }
}

impl Write for SysLogMessageWriter {
    fn write_str(&mut self, mut message: &str) -> fmt::Result {
        while message.len() > self.available() {
            let truncate_boundary = suggested_break(message, self.available());

            let left = &message[..truncate_boundary];
            let right = &message[truncate_boundary..];

            self.append(left.as_bytes());
            self.line_break();

            // This loop while terminate, since either of the following is true:
            //  1. truncate_boundary is strictly positive:
            //     message.len() has strictly decreased, and self.available() has not decreased
            //  2. truncate_boundary is zero:
            //     message.len() has remained unchanged, but self.available() has strictly increased;
            //     this latter is true since, for truncate_boundary to be 0, self.available() must
            //     have been not large enough to fit a single UTF8 character
            message = right;
        }

        self.append(message.as_bytes());

        Ok(())
    }
}

const FACILITY: c_int = libc::LOG_AUTH;

impl Log for Syslog {
    fn log(&self, level: Level, args: &dyn fmt::Display) {
        let priority = match level {
            Level::Error => libc::LOG_ERR,
            Level::Warn => libc::LOG_WARNING,
            Level::Info => libc::LOG_INFO,
            Level::Debug => libc::LOG_DEBUG,
            Level::Trace => libc::LOG_DEBUG,
        };

        let mut writer = SysLogMessageWriter::new(priority, FACILITY);
        let _ = write!(writer, "{}", args);
    }
}

#[cfg(test)]
mod tests {

    use std::fmt::Write;

    use super::*;

    #[test]
    fn can_write_to_syslog() {
        Syslog.log(Level::Info, &format_args!("Hello World!"));
    }

    #[test]
    fn can_handle_multiple_writes() {
        let mut writer = SysLogMessageWriter::new(libc::LOG_DEBUG, FACILITY);

        for i in 1..20 {
            let _ = write!(writer, "{}", "Test 123 ".repeat(i));
        }
    }

    #[test]
    fn can_truncate_syslog() {
        Syslog.log(
            Level::Info,
            &format_args!("This is supposed to be a very long syslog message but idk what to write, so I am just going to tell you about the time I tried to make coffee with a teapot. So I woke up one morning and decided to make myself a pot of coffee, however after all the wild coffee parties and mishaps the coffee pot had evetually given it's last cup on a tragic morning I call wednsday. So it came to, that the only object capable of giving me hope for the day was my teapot. As I stood in the kitchen and reached for my teapot it, as if sensing the impending horrors that awaited the innocent little teapot, emmited a horse sheak of desperation. \"three hundred and seven\", it said. \"What?\" I asked with a voice of someone who clearly did not want to be bothered until he had his daily almost medically necessary dose of caffine. \"I am a teapot\" it responded with a voice of increasing forcefulness. \"I am a teapot, not a coffee pot\". It was then, in my moments of confusion that my brain finally understood, this was a teapot."),
        );
    }

    #[test]
    fn can_truncate_syslog_with_no_spaces() {
        Syslog.log(
            Level::Info,
            &format_args!("iwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercasesiwillhandlecornercases"),
        );
    }

    #[test]
    fn will_not_break_utf8() {
        let mut writer = SysLogMessageWriter::new(libc::LOG_DEBUG, FACILITY);

        let _ = write!(writer, "{}¢", "x".repeat(959));
    }
}
