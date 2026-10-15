use std::collections::BTreeMap;

use kiln_core::operations::{JournalRecord, OutboxRecord};
use kiln_core::types::{ContentId, CoreResult, Diagnostic, ExternalRef};
use kiln_provider_api::EventAppendRequest;

use crate::admission::{AppendPlan, admit_append_batch};
use crate::partition::map_partition;
use crate::profile::CelldProfile;
use crate::protocol::{AuthEnvelope, WorkerResponse, sign_envelope, verify_envelope};

// r[impl kiln.celld.dependency_boundary] r[impl kiln.celld.partitioning]
// r[impl kiln.celld.atomic_append] r[impl kiln.celld.ambiguity]
/// One named Durable Object backing one journal partition. The model keeps the
/// exact facts the SQLite schema records: stream revision, event rows with
/// canonical bytes and identity, outbox rows with lease facts, and the
/// operation->revision idempotency map. The real Worker persists the same
/// facts in one SQLite transaction; this model exercises the same admission
/// and commit semantics in memory so fixtures run without celld.
#[derive(Clone, Debug)]
pub struct PartitionStore {
    pub key: String,
    pub stream: ExternalRef,
    pub revision: u64,
    pub event_identities: std::collections::BTreeSet<ExternalRef>,
    pub records: Vec<JournalRecord>,
    pub outbox: Vec<OutboxRecord>,
    pub operations: BTreeMap<ExternalRef, (ContentId, u64)>,
}

impl PartitionStore {
    fn new(key: String, stream: ExternalRef) -> Self {
        Self {
            key,
            stream,
            revision: 0,
            event_identities: std::collections::BTreeSet::new(),
            records: Vec::new(),
            outbox: Vec::new(),
            operations: BTreeMap::new(),
        }
    }

    fn append_atomic(
        &mut self,
        profile: &CelldProfile,
        request: &EventAppendRequest,
    ) -> CoreResult<AppendPlan> {
        let plan = admit_append_batch(profile, self.revision, &self.event_identities, request)?;
        // One atomic commit: every admitted fact lands together. An
        // interrupted transaction before this point exposes no partial state.
        self.revision = plan.next_revision;
        self.records.extend(request.records.iter().cloned());
        self.outbox.extend(request.outbox.iter().cloned());
        for record in &request.records {
            self.event_identities.insert(record.event_identity.clone());
        }
        Ok(plan)
    }
}

fn object_name_for(stream: &ExternalRef, partition_count: usize) -> CoreResult<String> {
    Ok(map_partition(stream, partition_count)?.object_name)
}

// r[impl kiln.celld.transport]
/// Stateless worker router: validates the authenticated envelope, resolves the
/// one named object for the stream, and applies the operation atomically.
/// Partition placement never becomes Kiln fairness or dispatch authority.
#[derive(Clone, Debug)]
pub struct CelldWorker {
    pub profile: CelldProfile,
    pub partition_count: usize,
    pub secret: Vec<u8>,
    pub objects: BTreeMap<String, PartitionStore>,
}

impl CelldWorker {
    pub fn new(profile: CelldProfile, partition_count: usize, secret: Vec<u8>) -> Self {
        Self {
            profile,
            partition_count,
            secret,
            objects: BTreeMap::new(),
        }
    }

    pub fn handle_append(
        &mut self,
        envelope: &AuthEnvelope,
        signature: &[u8; crate::HMAC_DIGEST_BYTES],
    ) -> CoreResult<WorkerResponse> {
        verify_envelope(&self.secret, envelope, signature)?;
        if envelope.path != self.profile.path {
            return Err(Diagnostic::new(
                "celld_route",
                "celld.transport.path",
                "worker route differs from the admitted profile path",
            ));
        }
        let operation_id = &envelope.operation_id;
        let payload_identity = &envelope.payload_identity;
        let request: EventAppendRequest = serde_json::from_slice(&envelope.body).map_err(|_| {
            Diagnostic::new(
                "celld_payload_decode",
                "celld.payload",
                "worker operation payload is malformed",
            )
        })?;
        // Idempotency: an equal operation returns the stored result without
        // another canonical mutation.
        let object_name = object_name_for(&request.stream, self.partition_count)?;
        let store = self
            .objects
            .entry(object_name.clone())
            .or_insert_with(|| PartitionStore::new(object_name.clone(), request.stream.clone()));
        if let Some((stored_payload, revision)) = store.operations.get(operation_id) {
            if stored_payload == payload_identity {
                return Ok(WorkerResponse::Repeated {
                    revision: *revision,
                });
            }
            return Ok(WorkerResponse::Conflict {
                reason: "operation identity already stored a different payload".to_string(),
            });
        }
        match store.append_atomic(&self.profile, &request) {
            Ok(plan) => {
                store.operations.insert(
                    operation_id.clone(),
                    (payload_identity.clone(), plan.next_revision),
                );
                Ok(WorkerResponse::Accepted {
                    revision: plan.next_revision,
                })
            }
            Err(error) => Ok(WorkerResponse::Rejected {
                code: error.code,
                message: error.message,
            }),
        }
    }

    pub fn replay(&self, stream: &ExternalRef) -> Option<&[JournalRecord]> {
        let assignment = map_partition(stream, self.partition_count).ok()?;
        self.objects
            .get(&assignment.object_name)
            .map(|store| store.records.as_slice())
    }

    /// Produce a signed append request suitable for dispatch fixtures.
    pub fn signed_request(
        &self,
        request: &EventAppendRequest,
        operation_id: &ExternalRef,
        nonce: &str,
    ) -> CoreResult<(AuthEnvelope, [u8; crate::HMAC_DIGEST_BYTES])> {
        let payload_identity = crate::protocol::payload_identity_of(&self.profile, request)?;
        let body = serde_json::to_vec(request).map_err(|_| {
            Diagnostic::new(
                "celld_payload_encoding",
                "celld.payload",
                "operation payload canonical JSON encoding failed",
            )
        })?;
        let envelope = crate::protocol::build_envelope(
            &self.profile.path,
            &ExternalRef::new("celld", "hmac-key", "kiln-primary")?,
            100,
            nonce,
            operation_id,
            &payload_identity,
            body,
        )?;
        let signature = sign_envelope(&self.secret, &envelope)?;
        Ok((envelope, signature))
    }

    pub fn delta_after(&self, request: &EventAppendRequest) -> CoreResult<u64> {
        admit_append_batch(
            &self.profile,
            0,
            &std::collections::BTreeSet::new(),
            request,
        )
        .map(|plan| plan.next_revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{admit_profile, profile_fixture};
    use kiln_core::operations::{JournalRecord, LeaseState, OutboxRecord};
    use kiln_core::types::{CONTRACT_VERSION_V1, CommandId, ProviderProfileId, RunId};

    fn stream() -> ExternalRef {
        ExternalRef::new("kiln", "stream", "builds")
            .unwrap_or_else(|_| unreachable!("stream builds"))
    }

    fn event() -> kiln_core::domain::Event {
        kiln_core::domain::Event::RunStarted {
            command_id: CommandId::new("kiln-command-1").unwrap_or_else(|_| unreachable!("builds")),
            run_id: RunId::new("kiln-run-1").unwrap_or_else(|_| unreachable!("builds")),
            attempt_id: kiln_core::types::AttemptId::new("kiln-attempt-1")
                .unwrap_or_else(|_| unreachable!("builds")),
            provider_profile: ProviderProfileId::new("nix")
                .unwrap_or_else(|_| unreachable!("builds")),
        }
    }

    fn record(revision: u64) -> JournalRecord {
        let identity = crate::protocol::identity_from_event(&event())
            .unwrap_or_else(|_| unreachable!("identity"));
        JournalRecord {
            schema_version: CONTRACT_VERSION_V1,
            stream: stream(),
            expected_revision: revision,
            event_identity: ExternalRef::new("kiln", "event-blake3", identity.as_str())
                .unwrap_or_else(|_| unreachable!("event identity builds")),
            event: event(),
        }
    }

    fn worker() -> CelldWorker {
        CelldWorker::new(profile_fixture(), 16, vec![0x5c; 32])
    }

    fn operation_id() -> ExternalRef {
        ExternalRef::new("kiln", "operation", "op-1").unwrap_or_else(|_| unreachable!("op builds"))
    }

    #[test]
    fn worker_commits_one_append() {
        let mut worker = worker();
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![record(0)],
            outbox: Vec::new(),
        };
        let (envelope, signature) = worker
            .signed_request(&request, &operation_id(), "nonce-1")
            .unwrap_or_else(|_| unreachable!("signs"));
        let response = worker
            .handle_append(&envelope, &signature)
            .unwrap_or_else(|_| unreachable!("handles"));
        assert_eq!(response, WorkerResponse::Accepted { revision: 1 });
        assert_eq!(
            worker
                .replay(&stream())
                .unwrap_or_else(|| unreachable!("replay"))
                .len(),
            1
        );
    }

    #[test]
    fn worker_replays_equal_operation_without_mutation() {
        let mut worker = worker();
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![record(0)],
            outbox: Vec::new(),
        };
        let operation_id = operation_id();
        let (envelope, signature) = worker
            .signed_request(&request, &operation_id, "nonce-1")
            .unwrap_or_else(|_| unreachable!("signs"));
        assert_eq!(
            worker
                .handle_append(&envelope, &signature)
                .unwrap_or_else(|_| unreachable!("first")),
            WorkerResponse::Accepted { revision: 1 }
        );
        let (envelope, signature) = worker
            .signed_request(&request, &operation_id, "nonce-2")
            .unwrap_or_else(|_| unreachable!("signs"));
        assert_eq!(
            worker
                .handle_append(&envelope, &signature)
                .unwrap_or_else(|_| unreachable!("repeat")),
            WorkerResponse::Repeated { revision: 1 }
        );
        assert_eq!(
            worker
                .replay(&stream())
                .unwrap_or_else(|| unreachable!("replay"))
                .len(),
            1
        );
    }

    #[test]
    fn worker_rejects_a_bad_signature() {
        let mut worker = worker();
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![record(0)],
            outbox: Vec::new(),
        };
        let (envelope, mut signature) = worker
            .signed_request(&request, &operation_id(), "nonce-1")
            .unwrap_or_else(|_| unreachable!("signs"));
        signature[0] ^= 0xff;
        let error = worker
            .handle_append(&envelope, &signature)
            .err()
            .unwrap_or_else(|| unreachable!("fails"));
        assert_eq!(error.code, "celld_signature");
    }

    #[test]
    fn worker_does_not_commit_past_a_rejection() {
        let mut worker = worker();
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![record(0), record(7)],
            outbox: Vec::new(),
        };
        let (envelope, signature) = worker
            .signed_request(&request, &operation_id(), "nonce-1")
            .unwrap_or_else(|_| unreachable!("signs"));
        let response = worker
            .handle_append(&envelope, &signature)
            .unwrap_or_else(|_| unreachable!("reject"));
        assert!(matches!(response, WorkerResponse::Rejected { .. }));
        // The resolved object exists but no event committed.
        assert!(
            worker
                .replay(&stream())
                .unwrap_or_else(|| unreachable!("object resolved"))
                .is_empty()
        );
    }

    #[test]
    fn worker_commits_outbox_and_event_together() {
        let mut worker = worker();
        let outbox = OutboxRecord {
            effect_id: kiln_core::types::EffectId::new("kiln-effect-1")
                .unwrap_or_else(|_| unreachable!("builds")),
            provider_profile: ProviderProfileId::new("nix")
                .unwrap_or_else(|_| unreachable!("builds")),
            idempotency_key: ExternalRef::new("kiln", "outbox", "outbox-1")
                .unwrap_or_else(|_| unreachable!("builds")),
            lease: LeaseState::Unclaimed,
        };
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![record(0)],
            outbox: vec![outbox],
        };
        let (envelope, signature) = worker
            .signed_request(&request, &operation_id(), "nonce-1")
            .unwrap_or_else(|_| unreachable!("signs"));
        let response = worker
            .handle_append(&envelope, &signature)
            .unwrap_or_else(|_| unreachable!("handles"));
        assert_eq!(response, WorkerResponse::Accepted { revision: 1 });
        let assignment = map_partition(&stream(), 16).unwrap_or_else(|_| unreachable!("maps"));
        let store = worker
            .objects
            .get(&assignment.object_name)
            .unwrap_or_else(|| unreachable!("object store exists"));
        assert_eq!(store.outbox.len(), 1);
        assert_eq!(store.records.len(), 1);
    }

    #[test]
    fn cohort_admission_requires_output_gate_and_probe() {
        let mut gated = profile_fixture();
        gated.output_gate_enabled = false;
        assert!(admit_profile(&gated).is_err());
        let mut probed = profile_fixture();
        probed.storage_probe_required = false;
        assert!(admit_profile(&probed).is_err());
    }

    #[test]
    fn cohort_restart_preserves_committed_facts() {
        let mut worker = worker();
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![record(0)],
            outbox: Vec::new(),
        };
        let operation_id = operation_id();
        let (envelope, signature) = worker
            .signed_request(&request, &operation_id, "nonce-1")
            .unwrap_or_else(|_| unreachable!("signs"));
        assert_eq!(
            worker
                .handle_append(&envelope, &signature)
                .unwrap_or_else(|_| unreachable!("first append")),
            WorkerResponse::Accepted { revision: 1 }
        );
        let assignment = map_partition(&stream(), 16).unwrap_or_else(|_| unreachable!("maps"));
        let facts = worker
            .objects
            .get(&assignment.object_name)
            .cloned()
            .unwrap_or_else(|| unreachable!("object exists"));
        let mut restarted = CelldWorker::new(profile_fixture(), 16, vec![0x5c; 32]);
        restarted.objects.insert(facts.key.clone(), facts);
        let (envelope, signature) = worker
            .signed_request(&request, &operation_id, "nonce-2")
            .unwrap_or_else(|_| unreachable!("signs"));
        assert_eq!(
            restarted
                .handle_append(&envelope, &signature)
                .unwrap_or_else(|_| unreachable!("repeat after restart")),
            WorkerResponse::Repeated { revision: 1 }
        );
    }

    #[test]
    fn conflicting_payload_is_rejected_after_commit() {
        let mut worker = worker();
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![record(0)],
            outbox: Vec::new(),
        };
        let operation_id = operation_id();
        let (envelope, signature) = worker
            .signed_request(&request, &operation_id, "nonce-1")
            .unwrap_or_else(|_| unreachable!("signs"));
        assert_eq!(
            worker
                .handle_append(&envelope, &signature)
                .unwrap_or_else(|_| unreachable!("first")),
            WorkerResponse::Accepted { revision: 1 }
        );
        let conflicting = EventAppendRequest {
            stream: stream(),
            expected_revision: 1,
            records: vec![record(1), record(2)],
            outbox: Vec::new(),
        };
        let (envelope, signature) = worker
            .signed_request(&conflicting, &operation_id, "nonce-3")
            .unwrap_or_else(|_| unreachable!("signs"));
        let response = worker
            .handle_append(&envelope, &signature)
            .unwrap_or_else(|_| unreachable!("conflict"));
        assert!(matches!(response, WorkerResponse::Conflict { .. }));
    }

    #[test]
    fn compatible_cohort_differential_is_identical() {
        // Differential behavior across an equivalent cohort: two logically
        // equivalent partitions (same revision and event facts) must accept
        // the same append and return the same revision with no effect on
        // ordering. This is the model-level equivalent of running against a
        // compatible workerd cohort.
        let mut left = worker();
        let mut right = worker();
        let request = EventAppendRequest {
            stream: stream(),
            expected_revision: 0,
            records: vec![record(0)],
            outbox: Vec::new(),
        };
        let (left_env, left_sig) = left
            .signed_request(&request, &operation_id(), "nonce-1")
            .unwrap_or_else(|_| unreachable!("signs"));
        let (right_env, right_sig) = right
            .signed_request(&request, &operation_id(), "nonce-1")
            .unwrap_or_else(|_| unreachable!("signs"));
        let left_response = left
            .handle_append(&left_env, &left_sig)
            .unwrap_or_else(|_| unreachable!("left"));
        let right_response = right
            .handle_append(&right_env, &right_sig)
            .unwrap_or_else(|_| unreachable!("right"));
        assert_eq!(left_response, right_response);
        assert_eq!(
            left.replay(&stream())
                .unwrap_or_else(|| unreachable!("left replay")),
            right
                .replay(&stream())
                .unwrap_or_else(|| unreachable!("right replay"))
        );
    }
}
