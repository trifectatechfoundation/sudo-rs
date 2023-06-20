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
