//! Optional renews/iroh swarm claim client for Kiln.
//!
//! The adapter is inert unless an operator supplies a configuration. It runs a
//! pinned `iroh-news` executable with a cleared environment, passes no
//! credentials, reads the newest window of the claim group, posts with Kiln's
//! own `From` address, and classifies every post as accepted, rejected, unknown or
//! unavailable. Claim formatting, redaction and overlap rules live in
//! `kiln_core::implementation::swarm`.

pub mod client;
pub mod config;
pub mod mirror;

pub use client::{Article, GroupEntry, PostOutcome, PostReceipt, SwarmClient, SwarmFailure};
pub use config::SwarmConfig;
pub use kiln_core::implementation::swarm::AttemptSnapshot;
pub use mirror::{
    MirrorAction, MirrorContext, MirrorEntry, MirrorJournal, MirrorPost, MirrorTransport, reconcile,
};
