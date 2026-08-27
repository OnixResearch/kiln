#![allow(
    dead_code,
    reason = "shared integration support is compiled separately for each focused test target"
)]

use kiln_adapter_aspen::profile::{
    AdmittedAspenRuntimeProfile, DEFAULT_CALLBACK_TIMEOUT_MILLIS, INITIAL_SERVICE_GENERATION,
    KILN_EXTENSION_PROTOCOL_SCHEMA, admit_profile, profile_fixture,
};
use kiln_adapter_aspen::protocol::{
    CALLBACK_REQUEST_SCHEMA, CallbackCommand, CallbackRequest, CallbackState,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_provider_api::{TriggerEventKind, TriggerRequest};

pub const PROCESS_FAILURE_EXIT_CODE: i32 = 1;
pub const OVERSIZED_INPUT_BYTES: usize =
    kiln_adapter_aspen::profile::DEFAULT_MAX_CALLBACK_INPUT_BYTES + 1;

pub fn admitted() -> AdmittedAspenRuntimeProfile {
    admit_profile(&profile_fixture()).unwrap_or_else(|_| unreachable!("profile admits"))
}

pub fn reference(namespace: &str, kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!("reference"))
}

pub fn trigger(same_revision: bool) -> TriggerRequest {
    let after = reference("radicle", "oid", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    TriggerRequest {
        repository: reference("radicle", "rid", "rad:fixture"),
        actor: reference("radicle", "did", "did:key:fixture"),
        before: Some(if same_revision {
            after.clone()
        } else {
            reference("radicle", "oid", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        }),
        after,
        branch: None,
        patch: None,
        event: TriggerEventKind::Push,
    }
}

pub fn identity_of<T: serde::Serialize>(value: &T) -> ContentId {
    let bytes = serde_json::to_vec(value).unwrap_or_else(|_| unreachable!("encode"));
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
        .unwrap_or_else(|_| unreachable!("identity"))
}

pub fn request(
    profile: &AdmittedAspenRuntimeProfile,
    state: CallbackState,
    command: CallbackCommand,
    label: &str,
) -> CallbackRequest {
    CallbackRequest {
        schema: CALLBACK_REQUEST_SCHEMA.to_string(),
        protocol_schema: KILN_EXTENSION_PROTOCOL_SCHEMA.to_string(),
        profile_identity: profile.profile_identity.clone(),
        generation: profile.profile.generation,
        sequence: state.revision.saturating_add(1),
        operation_id: reference("molten", "operation", label),
        authority_ref: profile.profile.authority_ref.clone(),
        policy_ref: profile.profile.policy_ref.clone(),
        resource_ref: profile.profile.resource_ref.clone(),
        deadline_tick: DEFAULT_CALLBACK_TIMEOUT_MILLIS,
        prior_state: state,
        command,
    }
}

pub fn submit_request(
    profile: &AdmittedAspenRuntimeProfile,
    trigger: TriggerRequest,
) -> CallbackRequest {
    let identity = identity_of(&trigger);
    request(
        profile,
        CallbackState::initial(INITIAL_SERVICE_GENERATION),
        CallbackCommand::SubmitTrigger {
            trigger: Box::new(trigger),
            trigger_identity: identity.clone(),
            idempotency_key: reference("kiln", "trigger-idempotency-blake3", identity.as_str()),
        },
        "submit-1",
    )
}
