use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const HINT_SCHEMA: &str = "kiln.gossip.hint.v1";
pub const TOPIC_DOMAIN: &[u8] = b"kiln.gossip.topic.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HintKind {
    CacheAvailable,
    StateChanged,
    CapacityChanged,
}

impl HintKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CacheAvailable => "cache_available",
            Self::StateChanged => "state_changed",
            Self::CapacityChanged => "capacity_changed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentReference {
    pub blake3: String,
    pub length: u64,
}

// r[impl kiln.aspen_runtime.gossip]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GossipHint {
    pub schema: String,
    pub kind: HintKind,
    pub scope: String,
    pub sender: String,
    pub epoch: u64,
    pub expires_at_ms: u64,
    pub content: Option<ContentReference>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HintBounds {
    pub max_message_bytes: usize,
    pub max_content_bytes: u64,
    pub max_lifetime_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GossipError {
    Malformed,
    Oversized,
    Expired,
    Lifetime,
    InvalidField,
    ContentBound,
    Encode,
    Bind,
    Transport,
    Timeout,
    EventBudget,
    Clock,
}

impl std::fmt::Display for GossipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Malformed => "gossip hint is malformed",
            Self::Oversized => "gossip hint exceeds its message bound",
            Self::Expired => "gossip hint has expired",
            Self::Lifetime => "gossip hint lifetime exceeds its bound",
            Self::InvalidField => "gossip hint field is invalid",
            Self::ContentBound => "gossip hint content exceeds its bound",
            Self::Encode => "gossip hint could not be encoded",
            Self::Bind => "gossip endpoint could not bind",
            Self::Transport => "gossip transport failed",
            Self::Timeout => "gossip listener timed out",
            Self::EventBudget => "gossip listener exceeded its event bound",
            Self::Clock => "host clock cannot time a gossip hint",
        })
    }
}

impl std::error::Error for GossipError {}

impl GossipHint {
    pub fn encode(&self, bounds: &HintBounds) -> Result<Vec<u8>, GossipError> {
        self.encode_at(bounds, now_ms()?)
    }

    pub fn encode_at(&self, bounds: &HintBounds, now_ms: u64) -> Result<Vec<u8>, GossipError> {
        self.validate(bounds, now_ms)?;
        let bytes = serde_json::to_vec(self).map_err(|_| GossipError::Encode)?;
        if bytes.len() > bounds.max_message_bytes {
            return Err(GossipError::Oversized);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8], bounds: &HintBounds) -> Result<Self, GossipError> {
        Self::decode_at(bytes, bounds, now_ms()?)
    }

    pub fn decode_at(bytes: &[u8], bounds: &HintBounds, now_ms: u64) -> Result<Self, GossipError> {
        if bytes.len() > bounds.max_message_bytes {
            return Err(GossipError::Oversized);
        }
        let hint: Self = serde_json::from_slice(bytes).map_err(|_| GossipError::Malformed)?;
        hint.validate(bounds, now_ms)?;
        Ok(hint)
    }

    pub fn validate(&self, bounds: &HintBounds, now_ms: u64) -> Result<(), GossipError> {
        if self.schema != HINT_SCHEMA {
            return Err(GossipError::InvalidField);
        }
        if !bounded_text(&self.scope) || !bounded_text(&self.sender) {
            return Err(GossipError::InvalidField);
        }
        if self.epoch == 0 {
            return Err(GossipError::InvalidField);
        }
        if self.expires_at_ms <= now_ms {
            return Err(GossipError::Expired);
        }
        if self.expires_at_ms - now_ms > bounds.max_lifetime_ms {
            return Err(GossipError::Lifetime);
        }
        if let Some(content) = &self.content
            && (!lower_hex_64(&content.blake3)
                || content.length == 0
                || content.length > bounds.max_content_bytes)
        {
            return Err(GossipError::ContentBound);
        }
        Ok(())
    }

    #[must_use]
    pub fn dedupe_key(&self) -> String {
        let content = self
            .content
            .as_ref()
            .map(|content| format!("{}\0{}", content.blake3, content.length))
            .unwrap_or_default();
        blake3::hash(
            format!(
                "{}\0{}\0{}\0{}\0{}",
                HINT_SCHEMA,
                self.scope,
                self.kind.as_str(),
                self.epoch,
                content
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string()
    }
}

/// Derives a public gossip topic from a stable scope string.
///
/// Topic secrecy is not confidentiality. Use a random secret topic for private
/// scopes, distributed over an authenticated channel.
#[must_use]
pub fn topic_id(scope: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(TOPIC_DOMAIN);
    hasher.update(scope.as_bytes());
    *hasher.finalize().as_bytes()
}

/// Reads the host wall clock as Unix milliseconds.
///
/// This is the module's only clock read. `encode_at`, `decode_at` and
/// `validate` take the time as an argument.
#[allow(
    tigerstyle::ambient_clock,
    reason = "shell boundary: hint expiry is compared against the host wall clock"
)]
pub fn now_ms() -> Result<u64, GossipError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| GossipError::Clock)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| GossipError::Clock)
}

pub(super) fn bounded_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && value.bytes().all(|byte| (32..=126).contains(&byte))
}

fn lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
