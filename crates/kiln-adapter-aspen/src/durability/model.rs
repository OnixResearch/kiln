use kiln_core::operations::{JournalRecord, OutboxRecord};
use kiln_core::types::ExternalRef;
use kiln_provider_api::{EventStoreSnapshot, PortFailure};
use serde::{Deserialize, Serialize};

pub const DURABLE_APPEND_SCHEMA: &str = "kiln.aspen-runtime.atomic-append.v1";
pub const DURABLE_APPEND_RECEIPT_SCHEMA: &str = "kiln.aspen-runtime.atomic-append-receipt.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenAtomicAppend {
    pub schema: String,
    pub operation_id: ExternalRef,
    pub stream: ExternalRef,
    pub expected_revision: u64,
    pub next_revision: u64,
    pub records: Vec<JournalRecord>,
    pub outbox: Vec<OutboxRecord>,
    pub profile_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenAppendReceipt {
    pub schema: String,
    pub operation_id: ExternalRef,
    pub stream: ExternalRef,
    pub committed_revision: u64,
    pub journal_count: usize,
    pub outbox_count: usize,
    pub receipt_ref: ExternalRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurabilityFault {
    None,
    TimeoutBeforeCommit,
    CancelBeforeCommit,
    UnavailableBeforeCommit,
    UnknownAfterCommit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AspenDurabilityOutcome {
    Accepted(Box<AspenAppendReceipt>),
    Conflict { actual_revision: u64 },
    TimedOut,
    Cancelled,
    Unavailable,
    Unknown { operation_id: ExternalRef },
}

pub trait AspenDurabilityPort {
    fn append_atomic(&mut self, request: &AspenAtomicAppend) -> AspenDurabilityOutcome;
    fn load_stream(
        &self,
        stream: &ExternalRef,
        maximum_records: usize,
    ) -> Result<EventStoreSnapshot, PortFailure>;
    fn reconcile_append(
        &self,
        operation_id: &ExternalRef,
    ) -> Result<Option<AspenAppendReceipt>, PortFailure>;
}
