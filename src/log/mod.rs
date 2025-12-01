#![allow(unused_macros)]
use self::simple_logger::SimpleLogger;
use self::syslog::Syslog;
use std::fmt;
use std::ops::Deref;
use std::sync::OnceLock;

mod simple_logger;
mod syslog;

macro_rules! logger_macro {
    ($name:ident is $rule_level:ident to $target:expr, $d:tt) => {
        macro_rules! $name {
            ($d($d arg:tt)+) => {
                if let Some(logger) = $crate::log::LOGGER.get() {
                    logger.log($crate::log::Level::$rule_level, $target, format_args!($d($d arg)+));
                }
            };
        }

        pub(crate) use $name;
    };
    ($name:ident is $rule_level:ident to $target:expr) => {
        logger_macro!($name is $rule_level to $target, $);
    };
}

// logger_macro!(auth_error is Error to "sudo::auth");
logger_macro!(auth_warn is Warn to "sudo::auth");
logger_macro!(auth_info is Info to "sudo::auth");
// logger_macro!(auth_debug is Debug to "sudo::auth");
// logger_macro!(auth_trace is Trace to "sudo::auth");

logger_macro!(user_error is Error to "sudo::user");
logger_macro!(user_warn is Warn to "sudo::user");
logger_macro!(user_info is Info to "sudo::user");
// logger_macro!(user_debug is Debug to "sudo::user");
// logger_macro!(user_trace is Trace to "sudo::user");

macro_rules! dev_logger_macro {
    ($name:ident is $rule_level:ident to $target:expr, $d:tt) => {
        macro_rules! $name {
            ($d($d arg:tt)+) => {
                if std::cfg!(feature = "dev") {
                    if let Some(logger) = $crate::log::LOGGER.get() {
                        logger.log(
                            $crate::log::Level::$rule_level,
                            $target,
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
    ($name:ident is $rule_level:ident to $target:expr) => {
        dev_logger_macro!($name is $rule_level to $target, $);
    };
}

dev_logger_macro!(dev_error is Error to "sudo::dev");
dev_logger_macro!(dev_warn is Warn to "sudo::dev");
dev_logger_macro!(dev_info is Info to "sudo::dev");
dev_logger_macro!(dev_debug is Debug to "sudo::dev");
//dev_logger_macro!(dev_trace is Trace to "sudo::dev");

pub static LOGGER: OnceLock<SudoLogger> = OnceLock::new();

#[derive(Default)]
pub struct SudoLogger(Vec<(String, Box<dyn Log>)>);

impl SudoLogger {
    pub fn new(prefix: &'static str) -> Self {
        let mut logger: Self = Default::default();

        logger.add_logger("sudo::auth", Syslog);

        logger.add_logger("sudo::user", SimpleLogger::to_stderr(prefix));

        #[cfg(feature = "dev")]
        {
            let path = option_env!("SUDO_DEV_LOGS")
                .map(|s| s.into())
                .unwrap_or_else(|| {
                    std::env::temp_dir().join(format!("sudo-dev-{}.log", std::process::id()))
                });
            logger.add_logger("sudo::dev", SimpleLogger::to_file(path, "").unwrap());
        }

        logger
    }

    pub fn into_global_logger(self) {
        if LOGGER.set(self).is_err() {
            panic!("Could not set previously set logger");
        }
    }

    /// Add a logger for a specific prefix to the stack
    fn add_logger(
        &mut self,
        prefix: impl ToString + Deref<Target = str>,
        logger: impl Log + 'static,
    ) {
        let prefix = if prefix.ends_with("::") {
            prefix.to_string()
        } else {
            // given a prefix `my::prefix`, we want to match `my::prefix::somewhere`
            // but not `my::prefix_to_somewhere`
            format!("{}::", prefix.to_string())
        };
        self.0.push((prefix, Box::new(logger)))
    }
}

impl SudoLogger {
    pub fn log(&self, level: Level, target: &str, args: fmt::Arguments<'_>) {
        for (prefix, l) in self.0.iter() {
            if target == &prefix[..prefix.len() - 2] || target.starts_with(prefix) {
                l.log(level, &args);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[allow(unused)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

trait Log: Send + Sync {
    fn log(&self, level: Level, args: &fmt::Arguments<'_>);
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
