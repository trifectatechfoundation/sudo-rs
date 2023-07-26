use log::{Level, Log, Metadata};

use crate::system::syslog;

pub struct Syslog;

impl Log for Syslog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level() && metadata.level() <= log::STATIC_MAX_LEVEL
    }

    fn log(&self, record: &log::Record) {
        let priority = match record.level() {
            Level::Error => libc::LOG_ERR,
            Level::Warn => libc::LOG_WARNING,
            Level::Info => libc::LOG_INFO,
            Level::Debug => libc::LOG_DEBUG,
            Level::Trace => libc::LOG_DEBUG,
        };

        let message = format!("{}", record.args());
        let message_len = message.bytes().len();

        let mut x: usize = 960;
        let mut x_prev: usize = 0;

        loop {
            // floor_char_boundary is currently unstable
            while !message.is_char_boundary(x) {
                x -= 1;
            }

            if x < message_len {
                x = message[x_prev..x].rfind(char::is_whitespace).unwrap_or(x) + x_prev + 1;
            } else {
                x = message_len
            }

            syslog(priority, libc::LOG_AUTH, &message[x_prev..x]);

            x_prev = x;
            x = if x + 960 > message_len {
                x + 960
            } else {
                message_len
            };

            if x_prev >= message_len {
                break;
            }
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
        let logger = Syslog;
        let record = log::Record::builder()
            .args(format_args!("Hello World!"))
            .level(log::Level::Info)
            .build();

        logger.log(&record);
    }

    #[test]
    fn can_truncate_syslog() {
        let logger = Syslog;
        let record = log::Record::builder()
            .args(format_args!("This is supposed to be a very long syslog message but idk what to write, so I am just going to tell you about the time I tried to make coffee with a teapot. So I woke up one morning and decided to make myself a pot of coffee, however after all the wild coffee parties and mishaps the coffee pot had evetually given it's last cup on a tragic morning I call wednsday. So it came to, that the only object capable of giving me hope for the day was my teapot. As I stood in the kitchen and reached for my teapot it, as if sensing the impending horrors that awaited the innocent little teapot, emmited a horse sheak of desperation. \"three hundred and seven\", it said. \"What?\" I asked with a voice of someone who clearly did not want to be bothered until he had his daily almost medically necessary dose of caffine. \"I am a teapot\" it responded with a voice of increasing forcefulness. \"I am a teapot, not a coffee pot\". It was then, in my moments of confusion that my brain finally understood, this was a teapot."))
            .level(log::Level::Info)
            .build();

        logger.log(&record);
    }
}
