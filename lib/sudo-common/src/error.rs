#[derive(Debug)]
pub enum Error {
    InvalidCommand,
    UserNotFound,
    Exec,
    Authentication(String),
}

impl Error {
    pub fn auth(message: &str) -> Self {
        Self::Authentication(message.to_string())
    }
}
