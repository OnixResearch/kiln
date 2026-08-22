//! Kiln celld event-store adapter.
//!
//! This crate implements Kiln's application-owned [`EventStorePort`] on top of
//! a versioned Kiln Worker deployed to the `denoland/celld` cohort. Pure
//! admission, partition, append, replay, lease, migration, and ambiguity
//! decisions stay in value-based functions. A thin runtime trait owns secrets,
//! transport, timing, and dispatch so tests run without a live worker.
//!
//! Core and application contracts contain no celld, V8, Worker, or
//! object-store types.
//!
//! ## Layout
//!
//! - [`mod protocol`] owns the bounded authenticated request/response
//!   envelope and the Worker wire response model.
//! - [`mod worker`] owns the versioned Worker model: a stateless router that
//!   maps one partition to one named object and applies one SQLite-style
//!   atomic transaction per operation.
//! - [`mod client`] owns the [`EventStorePort`] surface over the runtime
//!   trait, response admission, and unknown-outcome classification.
//! - crate root re-exports the reusable pure decisions (profile admission,
//!   partition mapping, append/replay/lease/migration admission, ambiguity).

pub mod client;
pub mod protocol;
pub mod worker;

mod admission;
mod partition;
mod profile;

pub use admission::{
    AppendPlan, DurableLease, MigrationPlan, OperationMatch, ReplayIntent, admit_append_batch,
    classify_repeat, lease_recovery_action, parse_lease_state, plan_lease_claim, plan_migration,
    plan_replay_page, replay_checked,
};
pub use client::{CelldEventStore, CelldRuntime, DispatchOutcome};
pub use partition::map_partition;
pub use profile::{
    CELLD_TRUST_DOMAIN_HOSTILE, CELLD_TRUST_DOMAIN_SINGLE, CelldProfile, CelldProfileBounds,
    admit_profile,
};
pub use protocol::{
    CELLD_OPERATION_KIND, CELLD_PARTITION_KIND, CELLD_PARTITION_NAMESPACE, CELLD_PAYLOAD_KIND,
    ENVELOPE_HEADER, WorkerResponse, build_envelope, classify_worker_response, payload_identity_of,
    sign_envelope, verify_envelope,
};
pub use worker::CelldWorker;

// r[impl kiln.celld.cohort]
pub const CELLD_RELEASE: &str = "v0.2.1";
pub const CELLD_REVISION: &str = "ae8fac053d79f971bfcb996054bb43eb2f9b05da";
pub const CELLD_LICENSE: &str = "Apache-2.0";
pub const WORKER_PROTOCOL_VERSION: u16 = 1;
pub const STORE_SCHEMA_VERSION: u16 = 1;
pub const INITIAL_SCHEMA_VERSION: u16 = 0;
pub const HMAC_SHA256_ID: &str = "hmac-sha256";
pub const HMAC_DIGEST_BYTES: usize = 32;
pub const HTTPS_SCHEME: &str = "https";
pub const POST_METHOD: &str = "POST";
pub const JSON_CONTENT_TYPE: &str = "application/json";

// r[impl kiln.celld.dependency_boundary] r[impl kiln.celld.conformance]
/// Named bounds that a celld profile must admit. Keep this in sync with the
/// Nickel `CelldProfileBounds` contract and the profile fixtures.
pub const NAMED_BOUNDS: &[&str] = &[
    "max_request_bytes",
    "max_response_bytes",
    "max_replay_page_bytes",
    "max_events_per_append",
    "max_outbox_records",
    "max_event_bytes",
    "max_partition_count",
    "max_lease_ticks",
    "max_migration_steps",
    "timeout_millis",
];
