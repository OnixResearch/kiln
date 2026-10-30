//! Iroh-connected hosted CI composition. CI semantics remain in existing adapters.
use serde::{Deserialize, Serialize};

pub mod accounts;
pub mod bridge;
pub mod cache;
pub mod connections;
pub mod execution;
pub mod finance;
pub mod fleet;
pub mod microvm;
pub mod operations;
pub mod pipelines;
pub mod secrets;
pub mod service;
pub mod transport;

pub const SCHEMA: &str = "kiln.hosted.v1";
pub const MAX_FRAME_BYTES: usize = 1_048_576;
pub const MAX_CONNECTIONS: usize = 32;
pub const EXCHANGE_TIMEOUT_MS: u64 = 30_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub peer: String,
    pub organization: String,
    pub role: String,
}

pub fn require(actor: &Actor, roles: &[&str]) -> Result<(), String> {
    if roles.contains(&actor.role.as_str()) {
        Ok(())
    } else {
        Err("role is not authorized for this operation".into())
    }
}

pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

pub fn identity(value: &impl Serialize) -> Result<String, String> {
    kiln_codec::encode_and_identify(value, MAX_FRAME_BYTES)
        .map(|(_, id)| id.as_str().to_owned())
        .map_err(|_| "canonical identity exceeds bound".into())
}

pub fn now_ms() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "host clock unavailable".to_owned())
        .and_then(|v| u64::try_from(v.as_millis()).map_err(|_| "host clock exceeds bound".into()))
}

pub(crate) fn sqlite_integer(value: u64) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| "unsigned value exceeds SQLite integer bound".into())
}

pub(crate) fn sqlite_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, i64>(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub organization: String,
    pub request_id: String,
    pub action: String,
    pub input: serde_json::Value,
}

impl Request {
    pub fn admit(&self) -> Result<(), String> {
        if self.schema != SCHEMA
            || !identifier(&self.organization)
            || !identifier(&self.request_id)
            || self.action.len() > 128
            || !self.action.contains('.')
            || !self
                .action
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'_')
        {
            return Err("request schema, identity or action rejected".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub schema: String,
    pub request_id: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}
