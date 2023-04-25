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

impl Default for Mailer {
    fn default() -> Self {
        let hostname = hostname();
        let user: String = match User::real() {
            Ok(Some(u)) => u.name,
            _ => String::from("root"),
        };

        Mailer {
            from: user,
            subject: format!("*** SECURITY information for {hostname} ***"),
            to: "root",
            mailer_flags: "-t",
            mailer_path: "/usr/sbin/sendmail",
        }
    }
}

impl Mailer {
    pub fn send(&self, notification: &str) -> io::Result<ExitStatus> {
        let mut message = String::new();
        message.push_str(&format!("To: {}\n", self.to));
        message.push_str(&format!("From: {}\n", self.from));
        message.push_str("Auto-Submitted: auto-generated\n");
        message.push_str(&format!("Subject: {}\n", self.subject));
        message.push_str(&format!("{}\n", notification));

        let mut mail_command = Command::new(self.mailer_path)
            .arg(self.mailer_flags)
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(input) = mail_command.stdin.as_mut() {
            input.write_all(message.as_bytes())?;
        }

        mail_command.wait()
    }
}

#[cfg(test)]
mod tests {
    use super::Mailer;

    #[test]
    #[ignore]
    fn test_mail() {
        let mailer = Mailer::new();

        mailer.send("3 incorrect password attempts").unwrap();
    }
}
