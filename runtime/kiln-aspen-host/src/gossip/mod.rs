//! Bounded, non-authoritative gossip hints.
//!
//! A hint announces that some content or state may be available. It is not a
//! reservation, authority grant, execution request, or proof of delivery.
//! Receivers must fetch canonical state and validate it independently.

mod hint;
mod listener;

pub use hint::{
    ContentReference, GossipError, GossipHint, HINT_SCHEMA, HintBounds, HintKind, TOPIC_DOMAIN,
    now_ms, topic_id,
};
pub use listener::{GossipListener, GossipProfile};

#[cfg(test)]
mod tests;
