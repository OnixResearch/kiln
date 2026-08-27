use kiln_core::domain::Event;
use kiln_core::operations::{JournalRecord, LeaseState, OutboxRecord};
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CommandId, EffectId, ExternalRef, ProviderProfileId, RunId,
};
use kiln_provider_api::EventAppendRequest;

use super::super::*;

pub(super) const TEST_MAXIMUM_BYTES: usize = 4_096;
pub(super) const TEST_MAXIMUM_RECORDS: usize = 32;
pub(super) const OVERSIZED_LOG_BYTES: usize = DEFAULT_MAX_LOG_BYTES + 1;

pub(super) fn admitted() -> AdmittedAspenRuntimeProfile {
    admit_profile(&profile_fixture()).unwrap_or_else(|_| unreachable!("fixture profile admits"))
}

pub(super) fn reference(namespace: &str, kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!("fixture ref builds"))
}

pub(super) fn event_request() -> EventAppendRequest {
    let stream = reference("kiln", "stream", "builds");
    let event = Event::RunStarted {
        command_id: CommandId::new("command-1").unwrap_or_else(|_| unreachable!("command")),
        run_id: RunId::new("run-1").unwrap_or_else(|_| unreachable!("run")),
        attempt_id: AttemptId::new("attempt-1").unwrap_or_else(|_| unreachable!("attempt")),
        provider_profile: ProviderProfileId::new("lattice-workflow-v1")
            .unwrap_or_else(|_| unreachable!("provider")),
    };
    let event_bytes = serde_json::to_vec(&event).unwrap_or_else(|_| unreachable!("event encoding"));
    let event_digest = blake3::hash(&event_bytes).to_hex().to_string();
    let effect_id = EffectId::new("effect-1").unwrap_or_else(|_| unreachable!("effect"));
    EventAppendRequest {
        stream: stream.clone(),
        expected_revision: 0,
        records: vec![JournalRecord {
            schema_version: CONTRACT_VERSION_V1,
            stream,
            expected_revision: 0,
            event_identity: reference("kiln", "event-blake3", &event_digest),
            event,
        }],
        outbox: vec![OutboxRecord {
            effect_id,
            provider_profile: ProviderProfileId::new("lattice-workflow-v1")
                .unwrap_or_else(|_| unreachable!("provider")),
            idempotency_key: reference("kiln", "idempotency", "effect-1"),
            lease: LeaseState::Unclaimed,
        }],
    }
}

pub(super) fn status_request() -> CallbackRequest {
    let admitted = admitted();
    CallbackRequest {
        schema: CALLBACK_REQUEST_SCHEMA.to_string(),
        protocol_schema: KILN_EXTENSION_PROTOCOL_SCHEMA.to_string(),
        profile_identity: admitted.profile_identity,
        generation: INITIAL_SERVICE_GENERATION,
        sequence: INITIAL_SERVICE_GENERATION,
        operation_id: reference("molten", "operation", "status-1"),
        authority_ref: "kiln-aspen-authority-v1".to_string(),
        policy_ref: "kiln-aspen-policy-v1".to_string(),
        resource_ref: "kiln-aspen-resources-v1".to_string(),
        deadline_tick: DEFAULT_CALLBACK_TIMEOUT_MILLIS,
        prior_state: CallbackState::initial(INITIAL_SERVICE_GENERATION),
        command: CallbackCommand::Status,
    }
}
