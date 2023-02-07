#[derive(Debug)]
pub enum Error {
    InvalidCommand,
    UserNotFound,
    Exec,
    Authentication(String),
    Configuration(String),
}

impl Error {
    pub fn auth(message: &str) -> Self {
        Self::Authentication(message.to_string())
    }

    pub fn conf(message: &str) -> Self {
        Self::Configuration(message.to_string())
    }
}
