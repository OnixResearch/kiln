//! Bounded, non-authoritative gossip hints.
//!
//! A hint announces that some content or state may be available. It is not a
//! reservation, authority grant, execution request, or proof of delivery.
//! Receivers must fetch canonical state and validate it independently.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use iroh::address_lookup::memory::MemoryLookup;
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh::{Endpoint, RelayMap, RelayMode, RelayUrl};
use iroh_gossip::api::Event;
use iroh_gossip::{Gossip, TopicId};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tokio::time::timeout;

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
        })
    }
}

impl std::error::Error for GossipError {}

impl GossipHint {
    pub fn encode(&self, bounds: &HintBounds) -> Result<Vec<u8>, GossipError> {
        self.encode_at(bounds, now_ms())
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
        Self::decode_at(bytes, bounds, now_ms())
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

#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Bounded transport profile for a gossip hint listener.
#[derive(Clone, Debug)]
pub struct GossipProfile {
    pub topic_scope: String,
    pub bootstrap_peers: Vec<iroh::EndpointId>,
    pub bind_address: SocketAddr,
    pub relay_urls: Vec<String>,
    pub max_message_bytes: usize,
    pub max_content_bytes: u64,
    pub max_lifetime_ms: u64,
    pub max_events: usize,
    pub max_recent_hints: usize,
    pub join_timeout_ms: u64,
}

impl GossipProfile {
    pub fn admit(&self) -> Result<(), GossipError> {
        if !bounded_text(&self.topic_scope)
            || self.bootstrap_peers.is_empty()
            || self.bootstrap_peers.len() > 64
            || self.relay_urls.len() > 8
            || self.max_message_bytes == 0
            || self.max_content_bytes == 0
            || self.max_lifetime_ms == 0
            || self.max_events == 0
            || self.max_recent_hints == 0
            || self.join_timeout_ms == 0
        {
            return Err(GossipError::InvalidField);
        }
        for url in &self.relay_urls {
            if !url.starts_with("https://") || url.parse::<RelayUrl>().is_err() {
                return Err(GossipError::Bind);
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn topic_id(&self) -> [u8; 32] {
        topic_id(&self.topic_scope)
    }
}

/// A bounded live gossip listener. It decodes and deduplicates hints, but it
/// does not grant authority or execute effects.
// r[impl kiln.aspen_runtime.gossip]
pub struct GossipListener {
    endpoint: Endpoint,
    router: Router,
    task: Option<tokio::task::JoinHandle<Result<(), GossipError>>>,
    shutdown: Option<oneshot::Sender<()>>,
}

impl std::fmt::Debug for GossipListener {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GossipListener")
            .field("endpoint", &self.endpoint.id())
            .finish_non_exhaustive()
    }
}

impl GossipListener {
    pub async fn start<S>(
        profile: GossipProfile,
        secret_key: iroh::SecretKey,
        lookup: MemoryLookup,
        mut sink: S,
    ) -> Result<Self, GossipError>
    where
        S: FnMut(GossipHint) + Send + 'static,
    {
        profile.admit()?;
        let relays = profile
            .relay_urls
            .iter()
            .map(|url| url.parse::<RelayUrl>().map_err(|_| GossipError::Bind))
            .collect::<Result<Vec<_>, _>>()?;
        let relay_mode = if relays.is_empty() {
            RelayMode::Disabled
        } else {
            RelayMode::Custom(RelayMap::from_iter(relays))
        };
        let Ok(builder) = Endpoint::builder(presets::Minimal)
            .secret_key(secret_key)
            .relay_mode(relay_mode)
            .address_lookup(lookup)
            .alpns(vec![iroh_gossip::ALPN.to_vec()])
            .clear_ip_transports()
            .bind_addr(profile.bind_address)
        else {
            return Err(GossipError::Bind);
        };
        let Ok(endpoint) = builder.bind().await else {
            return Err(GossipError::Bind);
        };
        let gossip = Gossip::builder()
            .max_message_size(profile.max_message_bytes)
            .spawn(endpoint.clone());
        let router = Router::builder(endpoint.clone())
            .accept(iroh_gossip::ALPN, gossip.clone())
            .spawn();
        let topic = TopicId::from_bytes(profile.topic_id());
        let join = timeout(
            Duration::from_millis(profile.join_timeout_ms),
            gossip.subscribe(topic, profile.bootstrap_peers.clone()),
        )
        .await;
        let Ok(Ok(topic)) = join else {
            return Err(GossipError::Timeout);
        };
        let (_, mut receiver) = topic.split();
        let bounds = HintBounds {
            max_message_bytes: profile.max_message_bytes,
            max_content_bytes: profile.max_content_bytes,
            max_lifetime_ms: profile.max_lifetime_ms,
        };
        let max_events = profile.max_events;
        let max_recent_hints = profile.max_recent_hints;
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut seen = BTreeSet::new();
            let mut events = 0_usize;
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break Ok(()),
                    event = receiver.next() => {
                        let Some(event) = event else { break Ok(()) };
                        events = match events.checked_add(1) {
                            Some(next) => next,
                            None => break Err(GossipError::EventBudget),
                        };
                        if events > max_events {
                            break Err(GossipError::EventBudget);
                        }
                        if let Ok(Event::Received(message)) = event {
                            let Ok(hint) = GossipHint::decode(&message.content, &bounds) else {
                                continue;
                            };
                            let key = hint.dedupe_key();
                            if seen.contains(&key) {
                                continue;
                            }
                            if seen.len() >= max_recent_hints {
                                break Err(GossipError::EventBudget);
                            }
                            seen.insert(key);
                            sink(hint);
                        }
                    }
                }
            }
        });
        Ok(Self {
            endpoint,
            router,
            task: Some(task),
            shutdown: Some(shutdown_tx),
        })
    }

    #[must_use]
    pub fn id(&self) -> iroh::EndpointId {
        self.endpoint.id()
    }

    #[must_use]
    pub fn addr(&self) -> iroh::EndpointAddr {
        self.endpoint.addr()
    }

    pub async fn stop(mut self) -> Result<(), GossipError> {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }
        let Some(task) = self.task.take() else {
            return Err(GossipError::Transport);
        };
        let result = match task.await {
            Ok(result) => result,
            Err(_) => Err(GossipError::Transport),
        };
        let _ = self.router.shutdown().await;
        result
    }
}

impl Drop for GossipListener {
    fn drop(&mut self) {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && value.bytes().all(|byte| (32..=126).contains(&byte))
}

fn lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> HintBounds {
        HintBounds {
            max_message_bytes: 4096,
            max_content_bytes: 1024,
            max_lifetime_ms: 60_000,
        }
    }

    fn hint(now_ms: u64) -> GossipHint {
        GossipHint {
            schema: HINT_SCHEMA.to_string(),
            kind: HintKind::CacheAvailable,
            scope: "org:acme/project:kiln".to_string(),
            sender: "iroh:peer".to_string(),
            epoch: 7,
            expires_at_ms: now_ms + 5_000,
            content: Some(ContentReference {
                blake3: "a".repeat(64),
                length: 128,
            }),
        }
    }

    #[test]
    fn hint_round_trip_preserves_fields_and_dedupe() {
        let hint = hint(1_000);
        let bytes = hint.encode_at(&bounds(), 1_000);
        assert!(matches!(&bytes, Ok(value) if value.len() <= 4096));
        let Ok(bytes) = bytes else {
            unreachable!("hint encodes");
        };
        let decoded = GossipHint::decode_at(&bytes, &bounds(), 1_000);
        assert!(matches!(&decoded, Ok(value) if value == &hint));
        let Ok(decoded) = decoded else {
            unreachable!("hint decodes");
        };
        assert_eq!(decoded.dedupe_key(), hint.dedupe_key());
    }

    #[test]
    fn expired_and_overlong_hints_are_rejected() {
        let expired = hint(1_000);
        assert!(matches!(
            expired.validate(&bounds(), 20_000),
            Err(GossipError::Expired)
        ));
        let mut overlong = hint(1_000);
        overlong.expires_at_ms = 1_000 + 60_001;
        assert!(matches!(
            overlong.validate(&bounds(), 1_000),
            Err(GossipError::Lifetime)
        ));
    }

    // r[verify kiln.aspen_runtime.gossip] r[verify kiln.aspen_runtime.gossip.scenario.bounds]
    #[test]
    fn oversized_message_and_content_are_rejected() {
        let hint = hint(1_000);
        let tight = HintBounds {
            max_message_bytes: 8,
            ..bounds()
        };
        assert!(matches!(
            hint.encode_at(&tight, 1_000),
            Err(GossipError::Oversized)
        ));
        let mut oversized = hint;
        if let Some(content) = &mut oversized.content {
            content.length = 1025;
        }
        assert!(matches!(
            oversized.validate(&bounds(), 1_000),
            Err(GossipError::ContentBound)
        ));
    }

    #[test]
    fn malformed_schema_and_fields_are_rejected() {
        let mut wrong_schema = hint(1_000);
        wrong_schema.schema = "kiln.gossip.hint.v0".to_string();
        assert!(matches!(
            wrong_schema.validate(&bounds(), 1_000),
            Err(GossipError::InvalidField)
        ));
        let mut empty_scope = hint(1_000);
        empty_scope.scope.clear();
        assert!(matches!(
            empty_scope.validate(&bounds(), 1_000),
            Err(GossipError::InvalidField)
        ));
        let mut zero_epoch = hint(1_000);
        zero_epoch.epoch = 0;
        assert!(matches!(
            zero_epoch.validate(&bounds(), 1_000),
            Err(GossipError::InvalidField)
        ));
        assert!(matches!(
            GossipHint::decode_at(b"not json", &bounds(), 1_000),
            Err(GossipError::Malformed)
        ));
    }

    #[test]
    fn topics_are_stable_and_scope_separated() {
        assert_eq!(topic_id("scope-a"), topic_id("scope-a"));
        assert_ne!(topic_id("scope-a"), topic_id("scope-b"));
    }
}
