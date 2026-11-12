use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheIndexProfile {
    pub store_root: PathBuf,
    pub max_key_bytes: usize,
    pub max_value_bytes: usize,
}

impl CacheIndexProfile {
    pub fn admit(&self) -> Result<(), CacheIndexError> {
        if !self.store_root.is_absolute() || self.max_key_bytes == 0 || self.max_value_bytes == 0 {
            return Err(CacheIndexError::Invalid);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheIndexEntry {
    pub key: Vec<u8>,
    pub author: String,
    pub hash: String,
    pub length: u64,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheIndexError {
    Invalid,
    Bound,
    Missing,
    Transport(String),
}

impl std::fmt::Display for CacheIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "cache index profile is invalid",
            Self::Bound => "cache index entry exceeds its bound",
            Self::Missing => "cache index document is unavailable",
            Self::Transport(message) => message,
        })
    }
}

impl std::error::Error for CacheIndexError {}
