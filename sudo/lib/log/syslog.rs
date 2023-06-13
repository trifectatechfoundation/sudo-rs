use std::io::ErrorKind;
use std::os::unix::net::UnixDatagram;
use std::sync::Mutex;

use log::{Level, Log, Metadata};

use crate::system::hostname;

const LOG_AUTH: u8 = 4 << 3;

const PROCESS_NAME: &str = "sudo";

const UNIX_SOCK_PATHS: [&str; 3] = ["/dev/log", "/var/run/syslog", "/var/run/log"];

#[allow(dead_code)]
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum Severity {
    Emergency,
    Alert,
    Critical,
    Error,
    Warning,
    Notice,
    Informational,
    Debug,
}

pub struct Syslog {
    logger: Mutex<UnixDatagram>,
    hostname: String,
    pid: u32,
}

impl Syslog {
    pub fn new() -> Result<Syslog, std::io::Error> {
        let logger = UNIX_SOCK_PATHS
            .iter()
            .find_map(|path| -> Option<Result<UnixDatagram, std::io::Error>> {
                match UnixDatagram::unbound() {
                    Ok(sock) => match sock.connect(path) {
                        Ok(_) => Some(Ok(sock)),
                        Err(e) if e.kind() == ErrorKind::NotFound => Some(Err(e)),
                        _ => None,
                    },
                    Err(e) => Some(Err(e)),
                }
            })
            .unwrap_or_else(|| {
                Err(std::io::Error::new(
                    ErrorKind::NotFound,
                    "Could not initialize syslog",
                ))
            })?;

        Ok(Syslog {
            logger: Mutex::new(logger),
            hostname: hostname(),
            pid: std::process::id(),
        })
    }
}

impl Log for Syslog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level() && metadata.level() <= log::STATIC_MAX_LEVEL
    }

    fn log(&self, record: &log::Record) {
        if let Ok(logger) = self.logger.lock() {
            let severity = match record.level() {
                Level::Error => Severity::Error,
                Level::Warn => Severity::Warning,
                Level::Info => Severity::Informational,
                Level::Debug => Severity::Debug,
                Level::Trace => Severity::Debug,
            };

            let message = format!(
                "<{}> {} {}[{}]: {}",
                severity as u8 | LOG_AUTH,
                self.hostname,
                PROCESS_NAME,
                self.pid,
                record.args()
            );

            let _ = logger.send(message.as_bytes());
        }
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
        let logger = Syslog::new().unwrap();
        let record = log::Record::builder()
            .args(format_args!("Hello World!"))
            .level(log::Level::Info)
            .build();

        logger.log(&record);
    }
}
