//! Optional renews/iroh swarm claim client for Kiln.
//!
//! The adapter is inert unless an operator supplies a configuration. It runs a
//! pinned `iroh-news` executable with a cleared environment, passes no
//! credentials, and classifies every post as accepted, rejected, unknown or
//! unavailable. Claim formatting, redaction and overlap rules live in
//! `kiln_core::implementation::swarm`.

pub mod client;
pub mod config;
pub mod mirror;

pub use client::{Article, PostOutcome, PostReceipt, SwarmClient, SwarmFailure};
pub use config::SwarmConfig;
pub use kiln_core::implementation::swarm::AttemptSnapshot;
pub use mirror::{
    MirrorAction, MirrorContext, MirrorEntry, MirrorJournal, MirrorPost, MirrorTransport, reconcile,
};
