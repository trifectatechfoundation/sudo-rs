use log::{Level, Log, Metadata};

use crate::system::syslog;

const LOG_AUTH: libc::c_int = 4 << 3;

#[allow(dead_code)]
#[derive(Copy, Clone)]
pub enum Priority {
    Emergency,
    Alert,
    Critical,
    Error,
    Warning,
    Notice,
    Informational,
    Debug,
}

pub struct Syslog;

impl Log for Syslog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level() && metadata.level() <= log::STATIC_MAX_LEVEL
    }

    fn log(&self, record: &log::Record) {
        let priority = match record.level() {
            Level::Error => Priority::Error,
            Level::Warn => Priority::Warning,
            Level::Info => Priority::Informational,
            Level::Debug => Priority::Debug,
            Level::Trace => Priority::Debug,
        };

        let message = format!("{}", record.args());
        syslog(priority as libc::c_int, LOG_AUTH, &message);
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
}
