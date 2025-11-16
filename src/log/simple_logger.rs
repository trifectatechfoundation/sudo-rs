use std::fmt;
use std::io::Write;

#[cfg(feature = "dev")]
use std::{fs::File, path::Path};

use crate::log::{Level, Log};

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
    fn log(&self, _level: Level, args: &fmt::Arguments<'_>) {
        let s = format!("{}{}\n", self.prefix, args);
        let _ = (&self.target).write_all(s.as_bytes());
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

    use super::*;

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
    fn test_write_and_flush() {
        let target = MyString::default();
        let logger = SimpleLogger {
            target: target.clone(),
            prefix: "[test] ",
        };

        logger.log(Level::Info, &format_args!("Hello World!"));

        let value = target.read();
        assert_eq!(value, "[test] Hello World!\nflushed");
        drop(value);
    }
}
