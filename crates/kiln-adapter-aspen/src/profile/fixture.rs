use std::collections::BTreeSet;

use super::*;

#[must_use]
pub fn expected_ports() -> BTreeSet<AspenPortBinding> {
    [
        binding(
            AspenPortPurpose::Durability,
            "molten.fabric.durability.ordered-state",
            "v1",
            "molten.fabric.durability.atomic-batch.v1",
            "molten.fabric.durability.atomic-receipt.v1",
        ),
        binding(
            AspenPortPurpose::Content,
            "molten.fabric.content-store",
            "v1",
            "molten.content-store-adapter.command.v1",
            "molten.content-store-adapter.event.v1",
        ),
        binding(
            AspenPortPurpose::Execution,
            "molten.fabric.execution.bounded-process",
            "v1",
            "molten.fabric.execution-request.v1",
            "molten.fabric.execution-receipt.v1",
        ),
        binding(
            AspenPortPurpose::Transport,
            "molten.fabric.transport.session",
            "v1",
            "molten.fabric.transport.request.v1",
            "molten.fabric.transport.observation.v1",
        ),
        binding(
            AspenPortPurpose::Time,
            "molten.fabric.time",
            "v1",
            "molten.fabric.time.request.v1",
            "molten.fabric.time.observation.v1",
        ),
        binding(
            AspenPortPurpose::Scheduling,
            "molten.fabric.scheduler",
            "v1",
            "molten.fabric.scheduler.request.v1",
            "molten.fabric.scheduler.observation.v1",
        ),
        binding(
            AspenPortPurpose::Evidence,
            "molten.fabric.observation",
            "v1",
            "molten.fabric.observation.event.v1",
            "molten.fabric.observation.receipt.v1",
        ),
    ]
    .into_iter()
    .collect()
}

fn binding(
    purpose: AspenPortPurpose,
    port_id: &str,
    version: &str,
    input_schema: &str,
    output_schema: &str,
) -> AspenPortBinding {
    AspenPortBinding {
        purpose,
        port_id: port_id.to_string(),
        version: version.to_string(),
        input_schema: input_schema.to_string(),
        output_schema: output_schema.to_string(),
        authority_ref: "kiln-aspen-authority-v1".to_string(),
        resource_ref: "kiln-aspen-resources-v1".to_string(),
    }
}

#[must_use]
pub fn profile_document_fixture() -> AspenRuntimeProfileDocument {
    let profile = profile_fixture();
    AspenRuntimeProfileDocument {
        schema: profile.schema.clone(),
        profile_id: profile.profile_id.clone(),
        cohort: AspenSourceCohort {
            source: profile.source.clone(),
            revision: profile.source_revision.clone(),
            bounded_exec_source: profile.bounded_exec_source.clone(),
            bounded_exec_revision: profile.bounded_exec_revision.clone(),
        },
        native_host: AspenNativeHostCohort {
            profile_schema: profile.native_host_schema.clone(),
            callback_envelope_schema: profile.native_callback_envelope_schema.clone(),
            callback_outcome_schema: profile.native_callback_outcome_schema.clone(),
            instance_schema: profile.native_instance_schema.clone(),
            ingress_schema: profile.native_ingress_schema.clone(),
            extension_protocol_schema: profile.extension_protocol_schema.clone(),
            extension_id: profile.extension_id.clone(),
            service_id: profile.service_id.clone(),
            execution_profile: profile.execution_profile.clone(),
            alpn: profile.alpn.clone(),
            framing: profile.framing.clone(),
            generation: profile.generation,
        },
        endpoint_ref: profile.endpoint_ref.clone(),
        transport_profile_ref: profile.transport_profile_ref.clone(),
        policy_ref: profile.policy_ref.clone(),
        authority_ref: profile.authority_ref.clone(),
        resource_ref: profile.resource_ref.clone(),
        claim_level: profile.claim_level.clone(),
        port_count: profile.ports.len(),
        ports: profile.ports.clone(),
        bounds: profile.bounds.clone(),
        fallback: "none".to_string(),
        non_claim_count: profile.non_claims.len(),
        non_claims: profile.non_claims,
    }
}

#[must_use]
pub fn profile_fixture() -> AspenRuntimeProfile {
    AspenRuntimeProfile {
        schema: ASPEN_RUNTIME_PROFILE_SCHEMA.to_string(),
        profile_id: ASPEN_RUNTIME_PROFILE_ID.to_string(),
        source: ASPEN_SOURCE.to_string(),
        source_revision: ASPEN_REVISION.to_string(),
        bounded_exec_source: BOUNDED_EXEC_SOURCE.to_string(),
        bounded_exec_revision: BOUNDED_EXEC_REVISION.to_string(),
        native_host_schema: ASPEN_NATIVE_HOST_PROFILE_SCHEMA.to_string(),
        native_callback_envelope_schema: ASPEN_NATIVE_CALLBACK_ENVELOPE_SCHEMA.to_string(),
        native_callback_outcome_schema: ASPEN_NATIVE_CALLBACK_OUTCOME_SCHEMA.to_string(),
        native_instance_schema: ASPEN_NATIVE_INSTANCE_SCHEMA.to_string(),
        native_ingress_schema: ASPEN_NATIVE_INGRESS_SCHEMA.to_string(),
        extension_protocol_schema: KILN_EXTENSION_PROTOCOL_SCHEMA.to_string(),
        extension_id: KILN_EXTENSION_ID.to_string(),
        service_id: KILN_EXTENSION_SERVICE_ID.to_string(),
        execution_profile: KILN_NATIVE_EXECUTION_PROFILE.to_string(),
        endpoint_ref: "kiln-aspen-local-endpoint-v1".to_string(),
        transport_profile_ref: "kiln-aspen-local-transport-v1".to_string(),
        alpn: ASPEN_NATIVE_ALPN.to_string(),
        framing: ASPEN_NATIVE_FRAMING.to_string(),
        generation: INITIAL_SERVICE_GENERATION,
        policy_ref: "kiln-aspen-policy-v1".to_string(),
        authority_ref: "kiln-aspen-authority-v1".to_string(),
        resource_ref: "kiln-aspen-resources-v1".to_string(),
        claim_level: LOCAL_CLAIM_LEVEL.to_string(),
        ports: expected_ports().into_iter().collect(),
        bounds: AspenRuntimeBounds {
            max_callback_input_bytes: DEFAULT_MAX_CALLBACK_INPUT_BYTES,
            max_callback_output_bytes: DEFAULT_MAX_CALLBACK_OUTPUT_BYTES,
            max_materialized_value_bytes: DEFAULT_MAX_MATERIALIZED_VALUE_BYTES,
            max_materialized_values: DEFAULT_MAX_MATERIALIZED_VALUES,
            max_content_bytes: DEFAULT_MAX_CONTENT_BYTES,
            max_log_bytes: DEFAULT_MAX_LOG_BYTES,
            max_effects: DEFAULT_MAX_EFFECTS,
            max_records: DEFAULT_MAX_RECORDS,
            callback_timeout_millis: DEFAULT_CALLBACK_TIMEOUT_MILLIS,
        },
        non_claims: REQUIRED_NON_CLAIMS
            .iter()
            .map(ToString::to_string)
            .collect(),
    }
}
