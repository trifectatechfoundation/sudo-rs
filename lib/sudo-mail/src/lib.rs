use std::{
    io::{self, Write},
    process::{Command, ExitStatus, Stdio},
};
use sudo_system::{hostname, User};

/// Mailer configuration, uses defaults as configuration for now
pub struct Mailer {
    from: String,
    subject: String,
    to: &'static str,
    mailer_flags: &'static str,
    mailer_path: &'static str,
}

impl Mailer {
    pub fn new(hostname: &str, username: &str) -> Self {
        Mailer {
            from: username.to_string(),
            subject: format!("*** SECURITY information for {hostname} ***"),
            to: "root",
            mailer_flags: "-t",
            mailer_path: "/usr/sbin/sendmail",
        }
    }
}

impl Default for Mailer {
    fn default() -> Self {
        let hostname = hostname();
        let username: String = match User::real() {
            Ok(Some(u)) => u.name,
            _ => String::from("root"),
        };

        Mailer::new(&hostname, &username)
    }
}

impl Mailer {
    fn create_message(&self, notification: &str) -> String {
        let mut message = String::new();
        message.push_str(&format!("To: {}\n", self.to));
        message.push_str(&format!("From: {}\n", self.from));
        message.push_str("Auto-Submitted: auto-generated\n");
        message.push_str(&format!("Subject: {}\n", self.subject));
        message.push_str(&format!("{}\n", notification));

        message
    }

    pub fn send(&self, notification: &str) -> io::Result<ExitStatus> {
        let mut mail_command = Command::new(self.mailer_path)
            .arg(self.mailer_flags)
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(input) = mail_command.stdin.as_mut() {
            input.write_all(self.create_message(notification).as_bytes())?;
        }

        mail_command.wait()
    }
}

#[cfg(test)]
mod tests {
    use super::Mailer;

    #[test]
    #[ignore]
    fn test_message() {
        let mailer = Mailer::default();
        let result = mailer.send("3 incorrect password attempts");

        assert!(result.is_ok());
    }

    #[test]
    fn test_mail() {
        let mailer = Mailer::new("test-host", "test-user");

        assert_eq!(mailer.create_message("test message"), "To: root\nFrom: test-user\nAuto-Submitted: auto-generated\nSubject: *** SECURITY information for test-host ***\ntest message\n");
    }
}
