use std::collections::BTreeSet;

use kiln_core::types::{CoreResult, Diagnostic, ExternalRef};
use kiln_provider_api::{
    EventAppendRequest, EventStorePort, EventStoreSnapshot, InfrastructureFailure, PortFailure,
};

use super::*;
use crate::profile::{AdmittedAspenRuntimeProfile, AspenRuntimeProfile, admit_profile};

#[derive(Clone, Debug)]
pub struct AspenEventStore<R> {
    admitted: AdmittedAspenRuntimeProfile,
    runtime: R,
}

impl<R> AspenEventStore<R> {
    pub fn new(profile: AspenRuntimeProfile, runtime: R) -> Result<Self, PortFailure> {
        let admitted = admit_profile(&profile).map_err(PortFailure::Domain)?;
        Ok(Self { admitted, runtime })
    }

    #[must_use]
    pub const fn runtime(&self) -> &R {
        &self.runtime
    }

    pub const fn runtime_mut(&mut self) -> &mut R {
        &mut self.runtime
    }
}

impl<R: AspenDurabilityPort> AspenEventStore<R> {
    pub fn reconcile(
        &self,
        operation_id: &ExternalRef,
    ) -> Result<Option<AspenAppendReceipt>, PortFailure> {
        self.runtime.reconcile_append(operation_id)
    }
}

// r[impl kiln.aspen_runtime.durability] r[impl kiln.aspen_runtime.failure]
impl<R: AspenDurabilityPort> EventStorePort for AspenEventStore<R> {
    fn append(&mut self, request: &EventAppendRequest) -> Result<u64, PortFailure> {
        let atomic = plan_atomic_append(&self.admitted, request).map_err(PortFailure::Domain)?;
        match self.runtime.append_atomic(&atomic) {
            AspenDurabilityOutcome::Accepted(receipt) => {
                admit_append_receipt(&atomic, &receipt).map_err(PortFailure::Domain)?;
                Ok(receipt.committed_revision)
            }
            AspenDurabilityOutcome::Conflict { actual_revision: _ } => {
                Err(PortFailure::Domain(Diagnostic::new(
                    "aspen_durable_conflict",
                    "aspen.durability.revision",
                    "expected durable revision does not match the observed Aspen revision",
                )))
            }
            AspenDurabilityOutcome::TimedOut => Err(PortFailure::TimedOut(infrastructure(
                "aspen_durable_timeout",
                "Aspen durable append exceeded its admitted deadline",
            ))),
            AspenDurabilityOutcome::Cancelled => Err(PortFailure::Cancelled(infrastructure(
                "aspen_durable_cancelled",
                "Aspen durable append was cancelled before a terminal result",
            ))),
            AspenDurabilityOutcome::Unavailable => {
                Err(PortFailure::Infrastructure(infrastructure(
                    "aspen_durable_unavailable",
                    "selected Aspen durability port is unavailable and no fallback is allowed",
                )))
            }
            AspenDurabilityOutcome::Unknown { operation_id } => {
                Err(PortFailure::Unknown(infrastructure(
                    "aspen_durable_unknown",
                    &format!(
                        "Aspen append outcome for operation {operation_id:?} is unknown until reconciliation"
                    ),
                )))
            }
        }
    }

    fn load(
        &self,
        stream: &ExternalRef,
        max_records: usize,
    ) -> Result<EventStoreSnapshot, PortFailure> {
        if max_records == 0 || max_records > self.admitted.profile.bounds.max_records {
            return Err(PortFailure::Domain(Diagnostic::new(
                "aspen_durable_load_bound",
                "aspen.durability.load",
                "load bound must be nonzero and within the admitted profile",
            )));
        }
        self.runtime.load_stream(stream, max_records)
    }
}

// r[impl kiln.aspen_runtime.durability] r[impl kiln.aspen_runtime.identity]
pub fn plan_atomic_append(
    profile: &AdmittedAspenRuntimeProfile,
    request: &EventAppendRequest,
) -> CoreResult<AspenAtomicAppend> {
    request.stream.validate()?;
    validate_batch_bound(profile, request)?;
    let next_revision = next_revision(request)?;
    validate_records(request)?;
    validate_outbox(request)?;
    let operation_id = operation_id(request)?;
    Ok(AspenAtomicAppend {
        schema: DURABLE_APPEND_SCHEMA.to_string(),
        operation_id,
        stream: request.stream.clone(),
        expected_revision: request.expected_revision,
        next_revision,
        records: request.records.clone(),
        outbox: request.outbox.clone(),
        profile_identity: profile.profile_identity.as_str().to_string(),
    })
}

fn validate_batch_bound(
    profile: &AdmittedAspenRuntimeProfile,
    request: &EventAppendRequest,
) -> CoreResult<()> {
    let total = request.records.len().saturating_add(request.outbox.len());
    if total == 0 || total > profile.profile.bounds.max_records {
        return Err(Diagnostic::new(
            "aspen_durable_batch_bound",
            "aspen.durability.batch",
            "atomic append must contain a bounded journal or outbox record",
        ));
    }
    Ok(())
}

fn next_revision(request: &EventAppendRequest) -> CoreResult<u64> {
    let record_count = u64::try_from(request.records.len()).map_err(|_| {
        Diagnostic::new(
            "aspen_durable_revision_overflow",
            "aspen.durability.revision",
            "journal record count does not fit the durable revision domain",
        )
    })?;
    request
        .expected_revision
        .checked_add(record_count)
        .ok_or_else(|| {
            Diagnostic::new(
                "aspen_durable_revision_overflow",
                "aspen.durability.revision",
                "durable revision overflowed",
            )
        })
}

fn validate_records(request: &EventAppendRequest) -> CoreResult<()> {
    let mut event_ids = BTreeSet::new();
    for (offset, record) in request.records.iter().enumerate() {
        let offset = u64::try_from(offset).map_err(|_| {
            Diagnostic::new(
                "aspen_durable_revision_overflow",
                "aspen.durability.revision",
                "journal offset does not fit the durable revision domain",
            )
        })?;
        let expected = request
            .expected_revision
            .checked_add(offset)
            .ok_or_else(|| {
                Diagnostic::new(
                    "aspen_durable_revision_overflow",
                    "aspen.durability.revision",
                    "journal expected revision overflowed",
                )
            })?;
        if record.stream != request.stream || record.expected_revision != expected {
            return Err(Diagnostic::new(
                "aspen_durable_record_drift",
                "aspen.durability.records",
                "journal records must bind the exact stream and contiguous expected revisions",
            ));
        }
        if !event_ids.insert(record.event_identity.clone()) {
            return Err(Diagnostic::new(
                "aspen_durable_duplicate_event",
                "aspen.durability.records",
                "atomic append contains a duplicate event identity",
            ));
        }
    }
    Ok(())
}

fn validate_outbox(request: &EventAppendRequest) -> CoreResult<()> {
    let mut effect_ids = BTreeSet::new();
    for record in &request.outbox {
        if !effect_ids.insert(record.effect_id.clone()) {
            return Err(Diagnostic::new(
                "aspen_durable_duplicate_effect",
                "aspen.durability.outbox",
                "atomic append contains a duplicate outbox effect identity",
            ));
        }
        record.idempotency_key.validate()?;
    }
    Ok(())
}

fn operation_id(request: &EventAppendRequest) -> CoreResult<ExternalRef> {
    let material = serde_json::to_vec(request).map_err(|_| {
        Diagnostic::new(
            "aspen_durable_encoding",
            "aspen.durability.batch",
            "atomic append identity encoding failed",
        )
    })?;
    let digest = blake3::hash(&material).to_hex().to_string();
    ExternalRef::new("molten", "durable-operation-blake3", &digest)
}

fn admit_append_receipt(
    request: &AspenAtomicAppend,
    receipt: &AspenAppendReceipt,
) -> CoreResult<()> {
    if receipt.schema != DURABLE_APPEND_RECEIPT_SCHEMA
        || receipt.operation_id != request.operation_id
        || receipt.stream != request.stream
        || receipt.committed_revision != request.next_revision
        || receipt.journal_count != request.records.len()
        || receipt.outbox_count != request.outbox.len()
    {
        return Err(Diagnostic::new(
            "aspen_durable_receipt_drift",
            "aspen.durability.receipt",
            "durable receipt does not match the submitted atomic batch",
        ));
    }
    receipt.receipt_ref.validate()?;
    Ok(())
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}
