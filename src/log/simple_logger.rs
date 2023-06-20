use std::io::{Stderr, Write};

#[cfg(feature = "dev")]
use std::{path::Path, sync::Mutex};

use log::Log;

pub trait LoggerWrite {
    fn write_log(&self, buf: &[u8]) -> std::io::Result<usize>;
    fn flush_log(&self) -> std::io::Result<()>;
}

impl LoggerWrite for Stderr {
    fn write_log(&self, buf: &[u8]) -> std::io::Result<usize> {
        self.lock().write(buf)
    }

    fn flush_log(&self) -> std::io::Result<()> {
        self.lock().flush()
    }
}

pub struct SimpleLogger<W: LoggerWrite + Send + Sync> {
    target: W,
    prefix: &'static str,
}

impl<W: LoggerWrite + Send + Sync> Log for SimpleLogger<W> {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level() && metadata.level() <= log::STATIC_MAX_LEVEL
    }

    fn log(&self, record: &log::Record) {
        let _ = self
            .target
            .write_log(format!("{}{}\n", self.prefix, record.args()).as_bytes());
    }

    fn flush(&self) {
        let _ = self.target.flush_log();
    }
}

impl SimpleLogger<std::io::Stderr> {
    pub fn to_stderr(prefix: &'static str) -> SimpleLogger<std::io::Stderr> {
        SimpleLogger {
            target: std::io::stderr(),
            prefix,
        }
    }
}

#[cfg(feature = "dev")]
pub struct MutexTarget(Box<Mutex<dyn Write + Send + Sync>>);

#[cfg(feature = "dev")]
impl LoggerWrite for MutexTarget {
    fn write_log(&self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush_log(&self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

#[cfg(feature = "dev")]
impl SimpleLogger<MutexTarget> {
    pub fn to_file<P: AsRef<Path>>(
        name: P,
        prefix: &'static str,
    ) -> Result<SimpleLogger<MutexTarget>, std::io::Error> {
        let file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(name)?;
        Ok(SimpleLogger {
            target: MutexTarget(Box::new(Mutex::new(file))),
            prefix,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use super::{LoggerWrite, SimpleLogger};
    use log::{LevelFilter, Log};

    impl LoggerWrite for Arc<RwLock<String>> {
        fn write_log(&self, buf: &[u8]) -> std::io::Result<usize> {
            self.write()
                .unwrap()
                .push_str(std::str::from_utf8(buf).unwrap());

            Ok(buf.len())
        }

        fn flush_log(&self) -> std::io::Result<()> {
            self.write().unwrap().push_str("flushed");

            Ok(())
        }
    }

    #[test]
    fn test_default_level() {
        let logger = SimpleLogger::to_stderr("test");
        let metadata = log::Metadata::builder().level(log::Level::Trace).build();

        log::set_max_level(LevelFilter::Trace);
        assert!(logger.enabled(&metadata));

        log::set_max_level(LevelFilter::Info);
        assert!(!logger.enabled(&metadata));
    }

    #[test]
    fn test_write_and_flush() {
        let target = Arc::new(RwLock::new(String::new()));
        let logger = SimpleLogger {
            target: target.clone(),
            prefix: "[test] ",
        };
        let record = log::Record::builder()
            .args(format_args!("Hello World!"))
            .level(log::Level::Info)
            .build();

        logger.log(&record);

        let value = target.read().unwrap();
        assert_eq!(*value, "[test] Hello World!\n");
        drop(value);

        logger.flush();

        let value = target.read().unwrap();
        assert_eq!(*value, "[test] Hello World!\nflushed");
        drop(value);
    }
}
