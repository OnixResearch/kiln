use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::time::Duration;

use futures_util::StreamExt;
use iroh::address_lookup::memory::MemoryLookup;
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh::{Endpoint, RelayMap, RelayMode, RelayUrl};
use iroh_gossip::api::Event;
use iroh_gossip::{Gossip, TopicId};
use tokio::sync::oneshot;
use tokio::time::timeout;

use super::hint::{GossipError, GossipHint, HintBounds, bounded_text, topic_id};

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
        // Dropping the sender wakes the task's shutdown branch exactly as a sent
        // value does. A task that already ended ignores it; its result is read next.
        self.shutdown = None;
        let Some(task) = self.task.take() else {
            return Err(GossipError::Transport);
        };
        let result = match task.await {
            Ok(result) => result,
            Err(_) => Err(GossipError::Transport),
        };
        let stopped = self
            .router
            .shutdown()
            .await
            .map_err(|_| GossipError::Transport);
        result.and(stopped)
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
