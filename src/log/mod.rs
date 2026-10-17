use self::simple_logger::SimpleLogger;
use self::syslog::Syslog;
use std::fmt;
use std::sync::OnceLock;

mod simple_logger;
mod syslog;

macro_rules! logger_macro {
    ($name:ident is $rule_level:ident to $target:ident with $filter:ident, $d:tt) => {
        macro_rules! $name {
            ($d($d arg:tt)+) => {
                if let Some(logger) = $crate::log::LOGGER.get() {
                    logger.log(
                        $crate::log::Level::$rule_level,
                        $crate::log::Sink::$target,
                        $filter!($d($d arg)+)
                    );
                }
            };
        }

        pub(crate) use $name;
    };

    ($name:ident is $rule_level:ident to $target:ident with $filter:ident) => {
        logger_macro!($name is $rule_level to $target with $filter, $);
    };
}

logger_macro!(auth_warn is Warn to AuthLog with format_args);
logger_macro!(auth_info is Info to AuthLog with format_args);

logger_macro!(user_error is Error to User with xlat);
logger_macro!(user_warn is Warn to User with xlat);
logger_macro!(user_info is Info to User with xlat);

macro_rules! dev_logger_macro {
    ($name:ident is $rule_level:ident, $d:tt) => {
        macro_rules! $name {
            ($d($d arg:tt)+) => {
                if std::cfg!(feature = "dev") {
                    if let Some(logger) = $crate::log::LOGGER.get() {
                        logger.log(
                            $crate::log::Level::$rule_level,
                            $crate::log::Sink::DevLog,
                            format_args!("{}: {}",
                                std::panic::Location::caller(),
                                format_args!($d($d arg)+)
                            )
                        );
                    }
                }
            };
        }

        pub(crate) use $name;
    };
    ($name:ident is $rule_level:ident) => {
        dev_logger_macro!($name is $rule_level, $);
    };
}

dev_logger_macro!(dev_error is Error);
dev_logger_macro!(dev_warn is Warn);
dev_logger_macro!(dev_info is Info);
dev_logger_macro!(dev_debug is Debug);

pub static LOGGER: OnceLock<SudoLogger> = OnceLock::new();

#[derive(Default)]
pub struct SudoLogger(Vec<(Sink, Box<dyn Log>)>);

impl SudoLogger {
    pub fn new(prefix: &'static str) -> Self {
        let mut logger: Self = Default::default();

        logger.add_logger(Sink::AuthLog, Syslog);

        logger.add_logger(Sink::User, SimpleLogger::to_stderr(prefix));

        #[cfg(feature = "dev")]
        {
            let path = option_env!("SUDO_DEV_LOGS")
                .map(|s| s.into())
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!("sudo-dev-{}.log", std::process::id()))
                });
            logger.add_logger(Sink::DevLog, SimpleLogger::to_file(path, "").unwrap());
        }

        logger
    }

    pub fn into_global_logger(self) {
        if LOGGER.set(self).is_err() {
            panic!("Could not set previously set logger");
        }
    }

    /// Add a logger for a specific prefix to the stack
    fn add_logger(&mut self, sink: Sink, logger: impl Log + 'static) {
        self.0.push((sink, Box::new(logger)))
    }
}

impl SudoLogger {
    pub fn log(&self, level: Level, target: Sink, args: impl fmt::Display) {
        for (sink, l) in self.0.iter() {
            if target == *sink {
                l.log(level, &args);
            }
        }
    }
}

#[repr(u32)]
#[derive(PartialEq)]
pub enum Sink {
    AuthLog = crate::common::HARDENED_ENUM_VALUE_0,
    User = crate::common::HARDENED_ENUM_VALUE_1,
    DevLog = crate::common::HARDENED_ENUM_VALUE_2,
}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    Error = crate::common::HARDENED_ENUM_VALUE_0,
    Warn = crate::common::HARDENED_ENUM_VALUE_1,
    Info = crate::common::HARDENED_ENUM_VALUE_2,
    Debug = crate::common::HARDENED_ENUM_VALUE_3,
}

trait Log: Send + Sync {
    fn log(&self, level: Level, args: &dyn fmt::Display);
}

#[cfg(test)]
mod tests {
    use super::SudoLogger;

    #[test]
    fn can_construct_logger() {
        let logger = SudoLogger::new("sudo: ");
        let len = if cfg!(feature = "dev") { 3 } else { 2 };
        assert_eq!(logger.0.len(), len);
    }
}
