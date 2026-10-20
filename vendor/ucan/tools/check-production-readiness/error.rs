use std::fmt;
use std::io;

#[derive(Debug)]
pub struct ShellError {
    pub code: &'static str,
    pub message: String,
}

impl ShellError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for ShellError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ShellError {}

impl From<io::Error> for ShellError {
    fn from(error: io::Error) -> Self {
        Self::new("io", error.to_string())
    }
}
