use std::io::Write;
use std::ops::Deref;

#[doc(hidden)]
pub use log::log as _private_log;
pub use log::Level;
use syslog::{BasicLogger, Facility, Formatter3164};

macro_rules! logger_macro {
    ($name:ident is $rule_level:ident to $target:expr, $d:tt) => {
        #[macro_export(local_inner_macros)]
        macro_rules! $name {
            ($d($d arg:tt)+) => (_private_log!(target: $target, $crate::Level::$rule_level, $d($d arg)+));
        }
    };
    ($name:ident is $rule_level:ident to $target:expr) => {
        logger_macro!($name is $rule_level to $target, $);
    };
}

logger_macro!(auth_error is Error to "sudo::auth");
logger_macro!(auth_warn is Warn to "sudo::auth");
logger_macro!(auth_info is Info to "sudo::auth");
logger_macro!(auth_debug is Debug to "sudo::auth");
logger_macro!(auth_trace is Trace to "sudo::auth");

logger_macro!(user_error is Error to "sudo::user");
logger_macro!(user_warn is Warn to "sudo::user");
logger_macro!(user_info is Info to "sudo::user");
logger_macro!(user_debug is Debug to "sudo::user");
logger_macro!(user_trace is Trace to "sudo::user");

#[derive(Default)]
pub struct SudoLogger(Vec<(String, Box<dyn log::Log>)>);

impl SudoLogger {
    pub fn new() -> Self {
        let mut logger: Self = Default::default();

        match syslog::unix(Formatter3164 {
            facility: Facility::LOG_AUTH,
            hostname: None,
            process: "sudo".into(),
            pid: std::process::id(),
        }) {
            // if we cannot connect to syslog daemon we silently ignore messages
            Err(_) => (),
            Ok(writer) => {
                logger.add_logger("sudo::auth", BasicLogger::new(writer));
            }
        }

        let stderr_logger = env_logger::Builder::new()
            .filter_level(log::LevelFilter::Trace)
            .format(|buf, record| writeln!(buf, "sudo: {}", record.args()))
            .build();
        logger.add_logger("sudo::user", stderr_logger);

        logger
    }

    pub fn into_global_logger(self) {
        log::set_boxed_logger(Box::new(self))
            .map(|()| log::set_max_level(log::LevelFilter::Trace))
            .expect("Could not set previously set logger");
    }

    /// Add a logger for a specific prefix to the stack
    fn add_logger(
        &mut self,
        prefix: impl ToString + Deref<Target = str>,
        logger: impl log::Log + 'static,
    ) {
        self.add_boxed_logger(prefix, Box::new(logger))
    }

    /// Add a boxed logger for a specific prefix to the stack
    fn add_boxed_logger(
        &mut self,
        prefix: impl ToString + Deref<Target = str>,
        logger: Box<dyn log::Log>,
    ) {
        let prefix = if prefix.ends_with("::") {
            prefix.to_string()
        } else {
            // given a prefix `my::prefix`, we want to match `my::prefix::somewhere`
            // but not `my::prefix_to_somewhere`
            format!("{}::", prefix.to_string())
        };
        self.0.push((prefix, logger))
    }
}

impl log::Log for SudoLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.0.iter().any(|(_, l)| l.enabled(metadata))
    }

    fn log(&self, record: &log::Record) {
        for (prefix, l) in self.0.iter() {
            if record.target() == &prefix[..prefix.len() - 2] || record.target().starts_with(prefix)
            {
                l.log(record);
            }
        }
    }

    fn flush(&self) {
        for (_, l) in self.0.iter() {
            l.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::SudoLogger;

    #[test]
    fn can_construct_logger() {
        let logger = SudoLogger::new();

        assert_eq!(logger.0.len(), 2);
    }
}
