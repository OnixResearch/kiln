use std::fmt;
use std::io;

#[derive(Debug)]
pub struct CheckerError {
    pub code: &'static str,
    pub message: String,
}

impl CheckerError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for CheckerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CheckerError {}

impl From<io::Error> for CheckerError {
    fn from(error: io::Error) -> Self {
        Self::new("io", error.to_string())
    }
}
