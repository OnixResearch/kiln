use kiln_adapter_aspen::profile::{
    AdmittedAspenRuntimeProfile, KILN_EXTENSION_ID, KILN_EXTENSION_SERVICE_ID,
};
use molten::fabric_execution::*;
use molten::system_extension::*;

use super::profile::{
    CALLBACK_POLL_INTERVAL_MS, CALLBACK_TEARDOWN_TIMEOUT_MS, CONCURRENCY_UNITS,
    EXTENSION_STATE_SCHEMA, QUEUE_UNITS,
};
use super::{HostBridgeError, HostPaths, Result, RoleRefs, role_ref, to_u64};

const LOGICAL_DEADLINE_TICKS: u64 = 10_000;
const ACCEPTED_EXIT_CODE: i32 = 0;

pub(super) fn executable_evidence(
    admitted: &CanonicalAdmittedSystemExtensionManifest,
    execution: &CanonicalExecutionProfile,
    executable_bytes_ref: &str,
    refs: &RoleRefs,
) -> NativeExecutableEvidence {
    NativeExecutableEvidence {
        schema: NATIVE_EXECUTABLE_EVIDENCE_SCHEMA.to_string(),
        executable_ref: role_ref(executable_bytes_ref, "executable-artifact"),
        executable_bytes_ref: executable_bytes_ref.to_string(),
        artifact_kind_ref: role_ref(executable_bytes_ref, "artifact-kind"),
        target_ref: role_ref(executable_bytes_ref, "target"),
        dependency_closure_ref: role_ref(executable_bytes_ref, "dependency-closure"),
        materialization_ref: role_ref(executable_bytes_ref, "materialization"),
        provenance_ref: refs.provenance.clone(),
        source_gate_ref: refs.evidence.clone(),
        policy_ref: refs.policy.clone(),
        authority_ref: refs.authority.clone(),
        resource_ref: refs.resource.clone(),
        execution_profile_ref: execution.profile_ref.clone(),
        manifest_ref: admitted.manifest_ref().to_string(),
        state_schema_ref: role_ref(EXTENSION_STATE_SCHEMA, "state-schema"),
        port_binding_refs: admitted.all_binding_refs().map(str::to_string).collect(),
    }
}

pub(super) struct TemplateInput<'a> {
    pub(super) profile: &'a AdmittedAspenRuntimeProfile,
    pub(super) native_profile: &'a AdmittedNativeHostProfile,
    pub(super) executable: &'a AdmittedNativeExecutable,
    pub(super) execution: &'a CanonicalExecutionProfile,
    pub(super) paths: &'a HostPaths,
    pub(super) instance_id: String,
    pub(super) admitted: &'a CanonicalAdmittedSystemExtensionManifest,
    pub(super) refs: &'a RoleRefs,
}

#[allow(
    tigerstyle::function_length,
    reason = "the execution template binds one closed Aspen authority and resource contract"
)]
pub(super) fn execution_template(input: TemplateInput<'_>) -> Result<NativeExecutionTemplate> {
    let TemplateInput {
        profile,
        native_profile,
        executable,
        execution,
        paths,
        instance_id,
        admitted,
        refs,
    } = input;
    let input_bound = to_u64(
        profile.profile.bounds.max_callback_input_bytes,
        "execution input bound",
    )?;
    let output_bound = to_u64(
        profile.profile.bounds.max_callback_output_bytes,
        "execution output bound",
    )?;
    let input_output_bytes = input_bound.checked_add(output_bound).ok_or_else(|| {
        HostBridgeError::Admission("execution I/O memory bound overflow".to_string())
    })?;
    let capture_bytes = output_bound.checked_add(output_bound).ok_or_else(|| {
        HostBridgeError::Admission("execution capture bound overflow".to_string())
    })?;
    let memory_bytes = input_output_bytes.max(capture_bytes);
    Ok(NativeExecutionTemplate {
        host_profile: native_profile.clone(),
        executable: executable.clone(),
        admitted: admitted.clone(),
        request: ExecutionRequest {
            schema: EXECUTION_REQUEST_SCHEMA.to_string(),
            operation_ref: role_ref(profile.profile_identity.as_str(), "operation-template"),
            idempotency_ref: role_ref(profile.profile_identity.as_str(), "idempotency-template"),
            extension_id: KILN_EXTENSION_ID.to_string(),
            service_id: KILN_EXTENSION_SERVICE_ID.to_string(),
            callback_ref: role_ref(profile.profile_identity.as_str(), "callback-template"),
            effect_ref: role_ref(profile.profile_identity.as_str(), "effect-template"),
            generation: profile.profile.generation,
            profile_ref: execution.profile.descriptor.profile_ref.clone(),
            executable_artifact_ref: executable.executable.executable_ref.clone(),
            executable_identity_ref: executable.executable.executable_bytes_ref.clone(),
            arguments: Vec::new(),
            environment: Vec::new(),
            environment_mode: ExecutionEnvironmentMode::Clear,
            invocation_mode: ExecutionInvocationMode::Direct,
            executable_resolution: ExecutableResolutionMode::ExactArtifact,
            workspace_ref: refs.workspace.clone(),
            workspace_mode: WorkspaceMode::CapabilityRoot,
            stdin_ref: Some(role_ref(
                profile.profile_identity.as_str(),
                "stdin-template",
            )),
            limits: ExecutionRequestLimits {
                timeout_ms: profile.profile.bounds.callback_timeout_millis,
                stdin_max_bytes: input_bound,
                stdout_max_bytes: output_bound,
                stderr_max_bytes: output_bound,
                poll_interval_ms: CALLBACK_POLL_INTERVAL_MS,
                teardown_timeout_ms: CALLBACK_TEARDOWN_TIMEOUT_MS,
                concurrency_units: CONCURRENCY_UNITS,
                queue_units: CONCURRENCY_UNITS,
            },
            termination_scope: ExecutionTerminationScope::ProcessGroup,
            accepted_exit_codes: vec![ACCEPTED_EXIT_CODE],
            reject_stdout_truncation: true,
            reject_stderr_truncation: true,
            authority_ref: refs.authority.clone(),
            resource_grant_ref: refs.resource.clone(),
        },
        authority: ExecutionAuthorityFacts {
            authority_ref: refs.authority.clone(),
            executable_authority_ref: refs.authority.clone(),
            provenance_ref: refs.provenance.clone(),
            effect_admission_ref: refs.evidence.clone(),
            workspace_authority_ref: refs.authority.clone(),
            process_authority_ref: refs.authority.clone(),
            resource_grant_ref: refs.resource.clone(),
            policy_ref: refs.policy.clone(),
            executable_artifact_ref: executable.executable.executable_ref.clone(),
            executable_identity_ref: executable.executable.executable_bytes_ref.clone(),
            workspace_ref: refs.workspace.clone(),
            operation_ref: role_ref(profile.profile_identity.as_str(), "operation-template"),
            extension_id: KILN_EXTENSION_ID.to_string(),
            service_id: KILN_EXTENSION_SERVICE_ID.to_string(),
            generation: profile.profile.generation,
            profile_ref: execution.profile.descriptor.profile_ref.clone(),
        },
        resources: ExecutionResourceGrant {
            memory_bytes,
            storage_bytes: input_bound,
            diagnostic_bytes: capture_bytes,
            logical_deadline_ticks: LOGICAL_DEADLINE_TICKS,
            concurrency_units: CONCURRENCY_UNITS,
            queue_units: QUEUE_UNITS,
        },
        resolved: ResolvedExecutionContext {
            executable_path: paths.extension_executable.clone(),
            executable_artifact_ref: executable.executable.executable_ref.clone(),
            executable_identity_ref: executable.executable.executable_bytes_ref.clone(),
            workspace_path: paths.workspace.clone(),
            workspace_ref: refs.workspace.clone(),
            stdin_ref: None,
            stdin_bytes: None,
        },
        context: NativeCallbackContext {
            manifest_ref: admitted.manifest_ref().to_string(),
            executable_ref: executable.executable.executable_ref.clone(),
            instance_id,
            extension_id: KILN_EXTENSION_ID.to_string(),
            service_id: KILN_EXTENSION_SERVICE_ID.to_string(),
            state_ref: None,
            policy_refs: admitted.manifest().policy_refs.clone(),
            resource_ref: refs.resource.clone(),
            port_binding_refs: admitted.all_binding_refs().map(str::to_string).collect(),
        },
    })
}
