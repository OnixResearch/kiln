#![allow(
    dead_code,
    reason = "shared integration support is compiled separately for each focused test target"
)]

use kiln_adapter_aspen::profile::{
    AdmittedAspenRuntimeProfile, DEFAULT_CALLBACK_TIMEOUT_MILLIS, INITIAL_SERVICE_GENERATION,
    KILN_EXTENSION_PROTOCOL_SCHEMA, admit_profile, profile_fixture,
};
use kiln_adapter_aspen::protocol::{
    AspenMaterializedValue, AspenNativeCallbackEnvelope, CALLBACK_REQUEST_SCHEMA, CallbackCommand,
    CallbackRequest, CallbackState, HOSTED_CALLBACK_INPUT_SCHEMA, HostedCallbackInput,
    aspen_content_ref, encode_aspen_native_envelope, encode_hosted_callback_input,
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

pub fn native_request_frame(
    profile: &AdmittedAspenRuntimeProfile,
    request: &CallbackRequest,
) -> Vec<u8> {
    let hosted = HostedCallbackInput {
        schema: HOSTED_CALLBACK_INPUT_SCHEMA.to_string(),
        protocol_schema: profile.profile.extension_protocol_schema.clone(),
        profile_identity: profile.profile_identity.clone(),
        operation_id: request.operation_id.clone(),
        authority_ref: request.authority_ref.clone(),
        policy_ref: request.policy_ref.clone(),
        resource_ref: request.resource_ref.clone(),
        command: request.command.clone(),
    };
    let payload_bytes =
        encode_hosted_callback_input(&hosted, profile.profile.bounds.max_materialized_value_bytes)
            .unwrap_or_else(|_| unreachable!("hosted input"));
    let state_bytes =
        serde_json::to_vec(&request.prior_state).unwrap_or_else(|_| unreachable!("state bytes"));
    let manifest_ref = aspen_content_ref(b"kiln-host-manifest-v2");
    let executable_ref = aspen_content_ref(b"kiln-host-executable-v2");
    let event_ref = aspen_content_ref(request.operation_id.native_id.as_str().as_bytes());
    let resource_ref = aspen_content_ref(b"kiln-host-resources-v2");
    let policy_ref = aspen_content_ref(b"kiln-host-policy-v2");
    let port_ref = aspen_content_ref(b"kiln-host-port-v2");
    let envelope = AspenNativeCallbackEnvelope {
        manifest_ref,
        executable_ref,
        instance_id: "kiln-host-instance-v2".to_string(),
        extension_id: profile.profile.extension_id.clone(),
        service_id: profile.profile.service_id.clone(),
        generation: request.generation,
        callback: "request".to_string(),
        sequence: request.sequence,
        event_ref,
        payload: Some(AspenMaterializedValue::exact(payload_bytes)),
        logical_tick: 1,
        deadline_tick: request.deadline_tick,
        state: Some(AspenMaterializedValue::exact(state_bytes)),
        policy_refs: vec![policy_ref],
        resource_ref,
        port_binding_refs: vec![port_ref],
    };
    encode_aspen_native_envelope(
        &envelope,
        profile.profile.bounds.max_callback_input_bytes,
        profile.profile.bounds.max_materialized_value_bytes,
        profile.profile.bounds.max_materialized_values,
    )
    .unwrap_or_else(|_| unreachable!("native envelope"))
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
