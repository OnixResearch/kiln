use std::{error::Error, fmt};

#[derive(Debug)]
pub enum PolicyError {
    Parse {
        source: serde_json::Error,
    },
    MissingField(&'static str),
    InvalidField(&'static str),
    InvalidWorkflow {
        code: &'static str,
        path: String,
        message: String,
    },
    UnsupportedVersion {
        found: u64,
        expected: u64,
    },
    UnsupportedPolicySchema {
        found: String,
        expected: &'static str,
    },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { source } => write!(f, "failed to parse policy: {source}"),
            Self::MissingField(field) => write!(f, "policy missing field {field}"),
            Self::InvalidField(field) => write!(f, "policy has invalid field {field}"),
            Self::InvalidWorkflow {
                code,
                path,
                message,
            } => write!(f, "{code} at {path}: {message}"),
            Self::UnsupportedVersion { found, expected } => {
                write!(f, "unsupported policy version {found}; expected {expected}")
            }
            Self::UnsupportedPolicySchema { found, expected } => {
                write!(f, "unsupported policy schema {found}; expected {expected}")
            }
        }
    }
}

impl Error for PolicyError {}
