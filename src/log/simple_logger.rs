use std::io::Write;

#[cfg(feature = "dev")]
use std::{fs::File, path::Path};

use log::Log;

pub struct SimpleLogger<W: Send + Sync>
where
    for<'a> &'a W: Write,
{
    target: W,
    prefix: &'static str,
}

impl<W: Send + Sync> Log for SimpleLogger<W>
where
    for<'a> &'a W: Write,
{
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level() && metadata.level() <= log::STATIC_MAX_LEVEL
    }

    fn log(&self, record: &log::Record) {
        let _ = writeln!(&self.target, "{}{}", self.prefix, record.args());
    }

    fn flush(&self) {
        let _ = (&self.target).flush();
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
impl SimpleLogger<File> {
    pub fn to_file<P: AsRef<Path>>(name: P, prefix: &'static str) -> Result<Self, std::io::Error> {
        let target = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(name)?;
        Ok(Self { target, prefix })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{Arc, RwLock},
    };

    use super::SimpleLogger;
    use log::{LevelFilter, Log};

    #[derive(Clone, Default)]
    struct MyString {
        inner: Arc<RwLock<String>>,
    }

    impl MyString {
        fn read(&self) -> String {
            self.inner.read().unwrap().clone()
        }
    }

    impl io::Write for &'_ MyString {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.inner
                .write()
                .unwrap()
                .push_str(std::str::from_utf8(buf).unwrap());
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.write(b"flushed").map(drop)
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
        let target = MyString::default();
        let logger = SimpleLogger {
            target: target.clone(),
            prefix: "[test] ",
        };
        let record = log::Record::builder()
            .args(format_args!("Hello World!"))
            .level(log::Level::Info)
            .build();

        logger.log(&record);

        let value = target.read();
        assert_eq!(value, "[test] Hello World!\n");
        drop(value);

        logger.flush();

        let value = target.read();
        assert_eq!(value, "[test] Hello World!\nflushed");
        drop(value);
    }
}
