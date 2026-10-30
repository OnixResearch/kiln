//! Tangled forge adapter: Kiln as a spindle-compatible CI service.
//!
//! Tangled repositories live on knots and run CI on spindles. Kiln serves the
//! spindle XRPC surface itself (`docs/tangled-integration.md`). Pure modules
//! decide admission, workflow mapping, pipeline state and status views. Shell
//! modules (`store`, `source`, `resolver`, `status`, `server`) own SQLite, Git,
//! HTTPS and sockets. Tangled DIDs, object ids and pipeline ids stay provider
//! facts; Kiln identities are BLAKE3.
//!
//! r[impl kiln.tangled.note] r[impl kiln.tangled.note.scenario.unverified]
//! `docs/tangled-integration.md` is this crate's design note: dated protocol
//! sources, the mapping onto Kiln ports and the "Not established" list.

pub mod auth;
pub mod error;
pub mod events;
pub mod identity;
pub mod pipeline;
pub mod profile;
pub mod resolver;
pub mod server;
pub mod service;
pub mod source;
pub mod status;
pub mod store;
pub mod time;
pub mod when;
pub mod workflows;
pub mod xrpc;

/// Lexicon NSIDs this adapter reads or serves.
pub mod nsid {
    pub const NOTIFY_EVENT: &str = "org.tangled.event.notifyEvent";
    pub const PUSH: &str = "org.tangled.event.push";
    pub const PULL_REQUEST: &str = "org.tangled.event.pullRequest";
    pub const MANUAL: &str = "org.tangled.ci.trigger.manual";
    pub const SCHEDULE: &str = "org.tangled.ci.trigger.schedule";
    pub const LIST_WORKFLOWS: &str = "org.tangled.ci.listWorkflowDefinitions";
    pub const GET_WORKFLOW: &str = "org.tangled.ci.getWorkflowDefinition";
    pub const RETRY_PIPELINE: &str = "org.tangled.ci.retryPipeline";
    pub const APPROVE_PIPELINE: &str = "org.tangled.ci.approvePipeline";
    pub const TRIGGER_PIPELINE: &str = "org.tangled.ci.triggerPipeline";
    pub const GET_PIPELINE: &str = "org.tangled.ci.getPipeline";
    pub const FILE_SOURCE: &str = "org.tangled.ci.defs#fileSource";
    /// Kiln-defined event name for signed status deliveries; Tangled defines none.
    pub const STATUS_EVENT: &str = "org.tangled.ci.pipeline";
}

#[cfg(test)]
mod tests;
