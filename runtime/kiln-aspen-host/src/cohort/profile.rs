use kiln_adapter_aspen::profile::{
    AdmittedAspenRuntimeProfile, AspenPortBinding, AspenPortPurpose, KILN_EXTENSION_ID,
    KILN_EXTENSION_SERVICE_ID,
};
use molten::fabric::*;
use molten::fabric_execution::*;
use molten::system_extension::*;

use super::{HostBridgeError, Result, RoleRefs, role_ref, to_u64};

const EXECUTION_PROFILE_ID: &str = "kiln-native-callback-live-v1";
const HOST_IMPLEMENTATION_PROFILE: &str = "kiln-aspen-host-v1";
pub(super) const EXTENSION_STATE_SCHEMA: &str = "kiln.aspen-extension.state.v2";
const EFFECT_OPERATION: &str = "kiln-provider-effect-v2";
const MAX_ARGUMENTS: usize = 16;
const MAX_ARGUMENT_BYTES: usize = 4_096;
const MAX_ENVIRONMENT_ENTRIES: usize = 16;
const MAX_ENVIRONMENT_NAME_BYTES: usize = 128;
const MAX_ENVIRONMENT_VALUE_BYTES: usize = 4_096;
const MAX_INSTANCES: usize = 1;
const MAX_POLICY_REFS: usize = 16;
const MAX_CONCURRENT_CALLBACKS: u64 = 1;
const MAX_QUEUED_EVENTS: u64 = 64;
const MAX_OPEN_STREAMS: u64 = 1;
const MAX_TIMERS: u64 = 1;
pub(super) const CALLBACK_POLL_INTERVAL_MS: u64 = 5;
pub(super) const CALLBACK_TEARDOWN_TIMEOUT_MS: u64 = 1_000;
const CALLBACK_DEADLINE_TICKS: u64 = 100;
const SHUTDOWN_GRACE_TICKS: u64 = 100;
const MAX_RESTART_ATTEMPTS: u64 = 2;
pub(super) const CONCURRENCY_UNITS: u64 = 1;
pub(super) const QUEUE_UNITS: u64 = 64;

pub(super) fn execution_profile_descriptor(
    profile: &AdmittedAspenRuntimeProfile,
    refs: &RoleRefs,
) -> Result<ExecutionProfileDescriptor> {
    let bounds = &profile.profile.bounds;
    Ok(ExecutionProfileDescriptor {
        schema: EXECUTION_PROFILE_SCHEMA.to_string(),
        profile_id: EXECUTION_PROFILE_ID.to_string(),
        profile_ref: refs.profile.clone(),
        kind: ExecutionProfileKind::LiveBoundedProcess,
        platform: ExecutionPlatform::UnixProcessGroup,
        supported_termination_scopes: vec![ExecutionTerminationScope::ProcessGroup],
        max_timeout_ms: bounds.callback_timeout_millis,
        max_stdin_bytes: to_u64(bounds.max_callback_input_bytes, "callback input bound")?,
        max_stdout_bytes: to_u64(bounds.max_callback_output_bytes, "callback output bound")?,
        max_stderr_bytes: to_u64(
            bounds.max_callback_output_bytes,
            "callback diagnostic bound",
        )?,
        max_poll_interval_ms: CALLBACK_POLL_INTERVAL_MS,
        max_teardown_timeout_ms: CALLBACK_TEARDOWN_TIMEOUT_MS,
        max_arguments: MAX_ARGUMENTS,
        max_argument_bytes: MAX_ARGUMENT_BYTES,
        max_environment_entries: MAX_ENVIRONMENT_ENTRIES,
        max_environment_name_bytes: MAX_ENVIRONMENT_NAME_BYTES,
        max_environment_value_bytes: MAX_ENVIRONMENT_VALUE_BYTES,
        max_concurrency_units: CONCURRENCY_UNITS,
        max_queue_units: QUEUE_UNITS,
        component_repository: BOUNDED_EXEC_REPOSITORY.to_string(),
        component_revision: BOUNDED_EXEC_REVISION.to_string(),
        component_license: BOUNDED_EXEC_LICENSE.to_string(),
        component_package: BOUNDED_EXEC_PACKAGE.to_string(),
        conformance_refs: vec![refs.evidence.clone()],
        fabric_non_claims: REQUIRED_FABRIC_NON_CLAIMS.to_vec(),
        non_claims: REQUIRED_EXECUTION_NON_CLAIMS.to_vec(),
    })
}

pub(super) fn native_profile(
    profile: &AdmittedAspenRuntimeProfile,
    execution: &CanonicalExecutionProfile,
    refs: &RoleRefs,
) -> Result<NativeHostProfile> {
    Ok(NativeHostProfile {
        schema: NATIVE_HOST_PROFILE_SCHEMA.to_string(),
        profile_id: profile.profile.execution_profile.clone(),
        profile_ref: role_ref(profile.profile_identity.as_str(), "native-host-profile"),
        execution_profile_ref: execution.profile_ref.clone(),
        transport_profile_ref: refs.transport.clone(),
        alpn: NATIVE_ALPN.to_string(),
        framing: NATIVE_FRAMING.to_string(),
        max_callback_input_bytes: to_u64(
            profile.profile.bounds.max_callback_input_bytes,
            "native callback input bound",
        )?,
        max_callback_output_bytes: to_u64(
            profile.profile.bounds.max_callback_output_bytes,
            "native callback output bound",
        )?,
        max_diagnostic_bytes: to_u64(
            profile.profile.bounds.max_callback_output_bytes,
            "native diagnostic bound",
        )?,
        max_materialized_value_bytes: to_u64(
            profile.profile.bounds.max_materialized_value_bytes,
            "native materialized value bound",
        )?,
        max_instances: MAX_INSTANCES,
        max_unresolved_operations: profile.profile.bounds.max_records,
        max_port_bindings: profile.profile.ports.len(),
        max_policy_refs: MAX_POLICY_REFS,
        max_materialized_values: profile.profile.bounds.max_materialized_values,
        is_local_live_pilot: true,
        requires_materialized_values: true,
        non_claims: REQUIRED_NATIVE_HOST_NON_CLAIMS.to_vec(),
    })
}

#[allow(
    tigerstyle::function_length,
    reason = "the admitted manifest binds one closed Aspen extension contract without hidden builders"
)]
pub(super) fn admitted_manifest(
    profile: &AdmittedAspenRuntimeProfile,
    refs: &RoleRefs,
) -> Result<CanonicalAdmittedSystemExtensionManifest> {
    let descriptors = profile
        .profile
        .ports
        .iter()
        .map(|binding| descriptor(binding, refs))
        .collect::<Vec<_>>();
    let requirements = profile
        .profile
        .ports
        .iter()
        .map(requirement)
        .collect::<Vec<_>>();
    let tier = canonical_extension_tier_admission(&ExtensionTierRequest {
        tier: ExtensionTier::SystemExtension,
        requested_authorities: vec![
            FabricAuthority::Transport,
            FabricAuthority::DurableState,
            FabricAuthority::Execution,
            FabricAuthority::Time,
            FabricAuthority::Scheduling,
            FabricAuthority::Resources,
            FabricAuthority::Supervision,
            FabricAuthority::Evidence,
        ],
        admission_evidence: REQUIRED_SYSTEM_EXTENSION_EVIDENCE.to_vec(),
    })
    .map_err(|error| HostBridgeError::Admission(format!("extension tier: {error:?}")))?;
    let bounds = &profile.profile.bounds;
    canonical_admit_system_extension_manifest(
        &SystemExtensionManifestInput {
            schema: SYSTEM_EXTENSION_MANIFEST_SCHEMA.to_string(),
            extension_id: KILN_EXTENSION_ID.to_string(),
            service_id: KILN_EXTENSION_SERVICE_ID.to_string(),
            implementation_ref: role_ref(profile.profile_identity.as_str(), "implementation"),
            callback_groups: [
                "initialize",
                "start",
                "request",
                "message",
                "health",
                "checkpoint",
                "recover",
                "drain",
                "shutdown",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            required_ports: requirements,
            optional_ports: Vec::new(),
            capability_refs: vec![refs.authority.clone()],
            policy_refs: vec![refs.policy.clone()],
            provenance_refs: vec![refs.provenance.clone()],
            resources: ResourceEnvelope {
                max_concurrent_callbacks: MAX_CONCURRENT_CALLBACKS,
                max_queued_events: MAX_QUEUED_EVENTS,
                max_inflight_bytes: to_u64(bounds.max_callback_input_bytes, "inflight bound")?,
                max_open_streams: MAX_OPEN_STREAMS,
                max_timers: MAX_TIMERS,
                max_effect_requests: to_u64(bounds.max_effects, "effect request bound")?,
                callback_deadline_ticks: CALLBACK_DEADLINE_TICKS,
                shutdown_grace_ticks: SHUTDOWN_GRACE_TICKS,
                max_restart_attempts: MAX_RESTART_ATTEMPTS,
                overload_policy: OverloadPolicy::UpstreamBackpressure,
            },
            execution_profile: ExecutionProfile::NativeProcess,
            state_schema: EXTENSION_STATE_SCHEMA.to_string(),
            compatible_state_schemas: vec![EXTENSION_STATE_SCHEMA.to_string()],
            evidence_profile_ref: refs.evidence.clone(),
            initial_generation: profile.profile.generation,
            non_claims: REQUIRED_SYSTEM_EXTENSION_NON_CLAIMS.to_vec(),
        },
        &descriptors,
        &tier,
        &[ExecutionProfile::NativeProcess],
    )
    .map_err(|error| HostBridgeError::Admission(format!("extension manifest: {error}")))
}

fn descriptor(binding: &AspenPortBinding, refs: &RoleRefs) -> FabricPortDescriptor {
    let (class, authority, resource) = port_shape(binding.purpose.clone());
    FabricPortDescriptor {
        schema: FABRIC_PORT_DESCRIPTOR_SCHEMA.to_string(),
        port_id: binding.port_id.clone(),
        version: binding.version.clone(),
        class,
        operation_classes: vec![EFFECT_OPERATION.to_string()],
        input_schema_refs: vec![binding.input_schema.clone()],
        output_schema_refs: vec![binding.output_schema.clone()],
        authority_requirements: vec![authority],
        resource_requirements: vec![resource],
        determinism: DeterminismClass::ExternalEffect,
        replay: ReplayClass::RecordedEffectRequired,
        implementation_profile: HOST_IMPLEMENTATION_PROFILE.to_string(),
        conformance_refs: vec![refs.evidence.clone()],
        non_claims: REQUIRED_FABRIC_NON_CLAIMS.to_vec(),
        enabled: true,
    }
}

fn requirement(binding: &AspenPortBinding) -> FabricPortRequirement {
    let (class, authority, resource) = port_shape(binding.purpose.clone());
    FabricPortRequirement {
        port_id: binding.port_id.clone(),
        version: binding.version.clone(),
        class,
        operation_classes: vec![EFFECT_OPERATION.to_string()],
        input_schema_refs: vec![binding.input_schema.clone()],
        output_schema_refs: vec![binding.output_schema.clone()],
        allowed_authorities: vec![authority],
        available_resources: vec![resource],
        expected_determinism: DeterminismClass::ExternalEffect,
        expected_replay: ReplayClass::RecordedEffectRequired,
        expected_profile: HOST_IMPLEMENTATION_PROFILE.to_string(),
    }
}

const fn port_shape(
    purpose: AspenPortPurpose,
) -> (FabricPortClass, FabricAuthority, FabricResource) {
    match purpose {
        AspenPortPurpose::Durability | AspenPortPurpose::Content => (
            FabricPortClass::DurableState,
            FabricAuthority::DurableState,
            FabricResource::StorageBytes,
        ),
        AspenPortPurpose::Execution => (
            FabricPortClass::Execution,
            FabricAuthority::Execution,
            FabricResource::ExecutionMillis,
        ),
        AspenPortPurpose::Transport => (
            FabricPortClass::Transport,
            FabricAuthority::Transport,
            FabricResource::NetworkBytes,
        ),
        AspenPortPurpose::Time => (
            FabricPortClass::Time,
            FabricAuthority::Time,
            FabricResource::LogicalTime,
        ),
        AspenPortPurpose::Scheduling => (
            FabricPortClass::Scheduling,
            FabricAuthority::Scheduling,
            FabricResource::QueueDepth,
        ),
        AspenPortPurpose::Evidence => (
            FabricPortClass::Evidence,
            FabricAuthority::Evidence,
            FabricResource::Diagnostics,
        ),
    }
}
