use std::sync::{Arc, Mutex};

use kiln_adapter_aspen::profile::{
    AdmittedAspenRuntimeProfile, KILN_EXTENSION_ID, KILN_EXTENSION_SERVICE_ID,
};
use molten::fabric_execution::*;
use molten::system_extension::*;

use crate::durability::{
    HostDurability, HostJournal, HostStateMode, OperationLedger, SharedHostJournal,
};
use crate::error::{HostBridgeError, Result};

mod execution;
mod profile;

use execution::{TemplateInput, executable_evidence, execution_template};
use profile::{admitted_manifest, execution_profile_descriptor, native_profile};

pub type HostExecutionPort = LiveExecutionAdapter<HashingOutputPublisher>;
pub type HostService = NativeSystemExtensionService<HostExecutionPort, HostJournal>;

pub const EXTENSION_RUNTIME_PROFILE_ARGUMENT: &str = "--runtime-profile";
pub const EXTENSION_RUNTIME_PROFILE_IDENTITY_ARGUMENT: &str = "--runtime-profile-identity";

#[derive(Clone, Debug)]
pub struct HostPaths {
    pub extension_executable: std::path::PathBuf,
    pub runtime_profile: std::path::PathBuf,
    pub workspace: std::path::PathBuf,
}

#[derive(Clone)]
pub struct HostCohort {
    pub profile: AdmittedAspenRuntimeProfile,
    pub admitted: CanonicalAdmittedSystemExtensionManifest,
    pub executable: AdmittedNativeExecutable,
    pub execution_profile: CanonicalExecutionProfile,
    pub native_profile: AdmittedNativeHostProfile,
    pub template: NativeExecutionTemplate,
    pub journal: SharedHostJournal,
    pub values: SharedNativeCallbackValuePort,
    pub ledger: Arc<Mutex<OperationLedger>>,
    pub state_mode: HostStateMode,
    instance_id: String,
    refs: RoleRefs,
}

impl std::fmt::Debug for HostCohort {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HostCohort")
            .field("profile_identity", &self.profile.profile_identity)
            .field("manifest_ref", &self.admitted.manifest_ref())
            .field("executable_ref", &self.executable.executable.executable_ref)
            .field("state_mode", &self.state_mode)
            .finish_non_exhaustive()
    }
}

impl HostCohort {
    #[allow(
        tigerstyle::explicit_defaults,
        reason = "fresh in-memory canary ports must start from their explicit empty defaults"
    )]
    pub fn build(profile: AdmittedAspenRuntimeProfile, paths: &HostPaths) -> Result<Self> {
        let refs = RoleRefs::new(profile.profile_identity.as_str());
        let executable_bytes = std::fs::read(&paths.extension_executable)
            .map_err(|error| HostBridgeError::Io(format!("read extension executable: {error}")))?;
        let executable_bytes_ref =
            molten::preserves_rail::content_ref_from_bytes(&executable_bytes);
        let execution_profile =
            canonical_admit_execution_profile(&execution_profile_descriptor(&profile, &refs)?)
                .map_err(|error| {
                    HostBridgeError::Admission(format!("execution profile: {error:?}"))
                })?;
        let admitted = admitted_manifest(&profile, &refs)?;
        let host_profile = native_profile(&profile, &execution_profile, &refs)?;
        let native_profile = admit_native_host_profile(&host_profile).map_err(|issues| {
            HostBridgeError::Admission(format!("native host profile: {issues:?}"))
        })?;
        let executable = admit_native_executable(
            &native_profile,
            &executable_evidence(&admitted, &execution_profile, &executable_bytes_ref, &refs),
        )
        .map_err(|issues| HostBridgeError::Admission(format!("native executable: {issues:?}")))?;
        let instance_id = native_identity_ref(&[
            "native-instance-v2",
            KILN_EXTENSION_ID,
            KILN_EXTENSION_SERVICE_ID,
            admitted.manifest_ref(),
        ]);
        let template = execution_template(TemplateInput {
            profile: &profile,
            native_profile: &native_profile,
            executable: &executable,
            execution: &execution_profile,
            paths,
            instance_id: instance_id.clone(),
            admitted: &admitted,
            refs: &refs,
        })?;
        let cohort_identity = native_identity_ref(&[
            "kiln-aspen-host-cohort-v1",
            profile.profile_identity.as_str(),
            admitted.manifest_ref(),
            &executable.executable.executable_ref,
            &execution_profile.profile.descriptor.profile_ref,
        ]);
        let durability = HostDurability::open(&paths.workspace, &profile, &cohort_identity)?;
        Ok(Self {
            profile,
            admitted,
            executable,
            execution_profile,
            native_profile,
            template,
            journal: durability.journal,
            values: durability.values,
            ledger: durability.ledger,
            state_mode: durability.mode,
            instance_id,
            refs,
        })
    }

    pub fn open_service(&self) -> Result<HostService> {
        let port =
            LiveExecutionAdapter::new(self.execution_profile.clone(), HashingOutputPublisher)
                .map_err(|error| {
                    HostBridgeError::Admission(format!("live execution adapter: {error:?}"))
                })?;
        match self.state_mode {
            HostStateMode::Fresh => NativeSystemExtensionService::install(
                self.native_profile.clone(),
                self.executable.clone(),
                self.admitted.clone(),
                port,
                self.journal.clone(),
                self.values.clone(),
                self.template.clone(),
            )
            .map_err(|error| {
                HostBridgeError::Admission(format!("native service install: {error:?}"))
            }),
            HostStateMode::Recovered => {
                let instance = self
                    .journal
                    .lock()
                    .map_err(|_| {
                        HostBridgeError::State(
                            "native journal lock is unavailable during recovery".to_string(),
                        )
                    })?
                    .latest_instance(&self.instance_id)
                    .map_err(|error| {
                        HostBridgeError::State(format!("read latest native instance: {error:?}"))
                    })?
                    .ok_or_else(|| {
                        HostBridgeError::State(
                            "durable native journal has no cohort instance".to_string(),
                        )
                    })?;
                let instance = Arc::new(Mutex::new(instance));
                let executor = NativeProcessSystemExtensionExecutor::new(
                    port,
                    self.journal.clone(),
                    self.values.clone(),
                    instance.clone(),
                    self.template.clone(),
                )
                .map_err(|error| {
                    HostBridgeError::State(format!("recover native executor: {error:?}"))
                })?;
                NativeSystemExtensionService::from_recovered(
                    self.native_profile.clone(),
                    self.executable.clone(),
                    self.admitted.clone(),
                    executor,
                    self.journal.clone(),
                    instance,
                )
                .map_err(|error| {
                    HostBridgeError::State(format!("recover native service: {error:?}"))
                })
            }
        }
    }

    pub fn native_ingress(
        &self,
        ingress: &kiln_adapter_aspen::transport::AspenServiceIngress,
    ) -> Result<NativeIngressEnvelope> {
        let hosted = kiln_adapter_aspen::protocol::HostedCallbackInput {
            schema: kiln_adapter_aspen::protocol::HOSTED_CALLBACK_INPUT_SCHEMA.to_string(),
            protocol_schema: self.profile.profile.extension_protocol_schema.clone(),
            profile_identity: self.profile.profile_identity.clone(),
            operation_id: ingress.operation_id.clone(),
            authority_ref: ingress.authority_ref.clone(),
            policy_ref: self.profile.profile.policy_ref.clone(),
            resource_ref: ingress.resource_ref.clone(),
            command: kiln_adapter_aspen::protocol::CallbackCommand::SubmitTrigger {
                trigger: Box::new(ingress.trigger.clone()),
                trigger_identity: ingress.trigger_identity.clone(),
                idempotency_key: ingress.idempotency_key.clone(),
            },
        };
        let payload = kiln_adapter_aspen::protocol::encode_hosted_callback_input(
            &hosted,
            self.profile.profile.bounds.max_materialized_value_bytes,
        )
        .map_err(|error| HostBridgeError::Protocol(error.to_string()))?;
        let accounted_bytes = to_u64(payload.len(), "native ingress payload")?;
        Ok(NativeIngressEnvelope {
            schema: NATIVE_INGRESS_SCHEMA.to_string(),
            request_ref: role_ref(
                self.profile.profile_identity.as_str(),
                ingress.operation_id.native_id.as_str(),
            ),
            endpoint_ref: self.refs.endpoint.clone(),
            peer_ref: self.refs.peer.clone(),
            service_id: KILN_EXTENSION_SERVICE_ID.to_string(),
            manifest_ref: self.admitted.manifest_ref().to_string(),
            generation: self.profile.profile.generation,
            authority_ref: self.refs.authority.clone(),
            policy_ref: self.refs.policy.clone(),
            resource_ref: self.refs.resource.clone(),
            transport_profile_ref: self.refs.transport.clone(),
            alpn: NATIVE_ALPN.to_string(),
            framing: NATIVE_FRAMING.to_string(),
            payload: NativeCallbackValue {
                value_ref: molten::preserves_rail::content_ref_from_bytes(&payload),
                bytes: payload,
            },
            accounted_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HashingOutputPublisher;

impl ExecutionOutputPublisher for HashingOutputPublisher {
    fn publish(
        &mut self,
        operation_ref: &str,
        stream: &RetainedExecutionStream,
    ) -> std::result::Result<PublishedExecutionStream, ExecutionOutputPublicationError> {
        let content_ref = molten::preserves_rail::content_ref_from_bytes(&stream.retained_bytes);
        let publication_receipt_ref =
            role_ref(operation_ref, &format!("{}\0{content_ref}", stream.role));
        Ok(PublishedExecutionStream {
            content_ref,
            publication_receipt_ref,
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct RoleRefs {
    pub(super) profile: String,
    pub(super) endpoint: String,
    pub(super) peer: String,
    pub(super) authority: String,
    pub(super) policy: String,
    pub(super) resource: String,
    pub(super) transport: String,
    pub(super) provenance: String,
    pub(super) workspace: String,
    pub(super) evidence: String,
}

impl RoleRefs {
    fn new(profile_identity: &str) -> Self {
        Self {
            profile: role_ref(profile_identity, "execution-profile"),
            endpoint: role_ref(profile_identity, "endpoint"),
            peer: role_ref(profile_identity, "peer"),
            authority: role_ref(profile_identity, "authority"),
            policy: role_ref(profile_identity, "policy"),
            resource: role_ref(profile_identity, "resource"),
            transport: role_ref(profile_identity, "transport"),
            provenance: role_ref(profile_identity, "provenance"),
            workspace: role_ref(profile_identity, "workspace"),
            evidence: role_ref(profile_identity, "evidence"),
        }
    }
}

pub(super) fn role_ref(domain: &str, role: &str) -> String {
    molten::preserves_rail::content_ref_from_bytes(
        format!("kiln.aspen-host.v1\0{domain}\0{role}").as_bytes(),
    )
}

pub(super) fn to_u64(value: usize, label: &str) -> Result<u64> {
    u64::try_from(value)
        .map_err(|_| HostBridgeError::Admission(format!("{label} does not fit u64")))
}
