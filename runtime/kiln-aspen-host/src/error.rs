#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HostBridgeError {
    Admission(String),
    Io(String),
    Protocol(String),
    Provider(String),
    State(String),
}

impl std::fmt::Display for HostBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(message) => write!(formatter, "host admission failed: {message}"),
            Self::Io(message) => write!(formatter, "host I/O failed: {message}"),
            Self::Protocol(message) => write!(formatter, "host protocol failed: {message}"),
            Self::Provider(message) => write!(formatter, "host provider failed: {message}"),
            Self::State(message) => write!(formatter, "host state failed: {message}"),
        }
    }
}

impl std::error::Error for HostBridgeError {}

pub type Result<T> = std::result::Result<T, HostBridgeError>;
