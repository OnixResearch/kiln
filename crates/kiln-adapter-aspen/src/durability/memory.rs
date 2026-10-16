use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use kiln_core::operations::{JournalRecord, OutboxRecord};
use kiln_core::types::{Diagnostic, ExternalRef};
use kiln_provider_api::{EventStoreSnapshot, InfrastructureFailure, PortFailure};

use super::*;

#[derive(Clone, Debug, Default)]
struct DurableStream {
    revision: u64,
    records: Vec<JournalRecord>,
    outbox: Vec<OutboxRecord>,
}

#[derive(Clone, Debug, Default)]
struct InMemoryDurabilityState {
    streams: BTreeMap<ExternalRef, DurableStream>,
    operations: BTreeMap<ExternalRef, AspenAppendReceipt>,
    next_fault: Option<DurabilityFault>,
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryAspenDurability {
    state: Arc<Mutex<InMemoryDurabilityState>>,
}

impl InMemoryAspenDurability {
    pub fn set_next_fault(&self, fault: DurabilityFault) -> Result<(), PortFailure> {
        let mut state = self.state.lock().map_err(|_| poisoned_failure())?;
        state.next_fault = Some(fault);
        Ok(())
    }

    pub fn revision(&self, stream: &ExternalRef) -> Result<u64, PortFailure> {
        let state = self.state.lock().map_err(|_| poisoned_failure())?;
        Ok(state.streams.get(stream).map_or(0, |value| value.revision))
    }
}

impl AspenDurabilityPort for InMemoryAspenDurability {
    fn append_atomic(&mut self, request: &AspenAtomicAppend) -> AspenDurabilityOutcome {
        let Ok(mut state) = self.state.lock() else {
            return AspenDurabilityOutcome::Unavailable;
        };
        if let Some(receipt) = state.operations.get(&request.operation_id) {
            return AspenDurabilityOutcome::Accepted(Box::new(receipt.clone()));
        }
        let current_revision = state
            .streams
            .get(&request.stream)
            .map_or(0, |stream| stream.revision);
        if current_revision != request.expected_revision {
            return AspenDurabilityOutcome::Conflict {
                actual_revision: current_revision,
            };
        }
        let fault = state.next_fault.take().unwrap_or(DurabilityFault::None);
        match fault {
            DurabilityFault::TimeoutBeforeCommit => return AspenDurabilityOutcome::TimedOut,
            DurabilityFault::CancelBeforeCommit => return AspenDurabilityOutcome::Cancelled,
            DurabilityFault::UnavailableBeforeCommit => return AspenDurabilityOutcome::Unavailable,
            DurabilityFault::None | DurabilityFault::UnknownAfterCommit => {}
        }
        let Some(receipt) = append_receipt(request) else {
            return AspenDurabilityOutcome::Unavailable;
        };
        let stream = state.streams.entry(request.stream.clone()).or_default();
        stream.records.extend(request.records.clone());
        stream.outbox.extend(request.outbox.clone());
        stream.revision = request.next_revision;
        state
            .operations
            .insert(request.operation_id.clone(), receipt.clone());
        if fault == DurabilityFault::UnknownAfterCommit {
            AspenDurabilityOutcome::Unknown {
                operation_id: request.operation_id.clone(),
            }
        } else {
            AspenDurabilityOutcome::Accepted(Box::new(receipt))
        }
    }

    fn load_stream(
        &self,
        stream: &ExternalRef,
        maximum_records: usize,
    ) -> Result<EventStoreSnapshot, PortFailure> {
        stream.validate().map_err(PortFailure::Domain)?;
        let state = self.state.lock().map_err(|_| poisoned_failure())?;
        let Some(value) = state.streams.get(stream) else {
            return Ok(EventStoreSnapshot {
                records: Vec::new(),
                outbox: Vec::new(),
            });
        };
        let total = value.records.len().saturating_add(value.outbox.len());
        if total > maximum_records {
            return Err(PortFailure::Domain(Diagnostic::new(
                "aspen_durable_load_bound",
                "aspen.durability.load",
                "durable stream exceeds the caller's record bound",
            )));
        }
        Ok(EventStoreSnapshot {
            records: value.records.clone(),
            outbox: value.outbox.clone(),
        })
    }

    fn reconcile_append(
        &self,
        operation_id: &ExternalRef,
    ) -> Result<Option<AspenAppendReceipt>, PortFailure> {
        operation_id.validate().map_err(PortFailure::Domain)?;
        let state = self.state.lock().map_err(|_| poisoned_failure())?;
        Ok(state.operations.get(operation_id).cloned())
    }
}

fn append_receipt(request: &AspenAtomicAppend) -> Option<AspenAppendReceipt> {
    let material = format!(
        "{}\0{}\0{}\0{}",
        request.operation_id.native_id.as_str(),
        request.stream.native_id.as_str(),
        request.next_revision,
        request.profile_identity
    );
    let digest = blake3::hash(material.as_bytes()).to_hex().to_string();
    let receipt_ref = ExternalRef::new("molten", "durable-receipt-blake3", &digest).ok()?;
    Some(AspenAppendReceipt {
        schema: DURABLE_APPEND_RECEIPT_SCHEMA.to_string(),
        operation_id: request.operation_id.clone(),
        stream: request.stream.clone(),
        committed_revision: request.next_revision,
        journal_count: request.records.len(),
        outbox_count: request.outbox.len(),
        receipt_ref,
    })
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn poisoned_failure() -> PortFailure {
    PortFailure::Infrastructure(infrastructure(
        "aspen_durable_lock",
        "local Aspen durability fixture lock is unavailable",
    ))
}
