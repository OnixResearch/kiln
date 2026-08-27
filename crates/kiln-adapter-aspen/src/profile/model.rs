use kiln_core::types::{ContentId, CoreResult, Diagnostic};
use serde::{Deserialize, Serialize};

pub const ASPEN_RUNTIME_PROFILE_SCHEMA: &str = "kiln.aspen-runtime.profile.v2";
pub const ASPEN_RUNTIME_PROFILE_ID: &str = "aspen-runtime-v2";
pub const ASPEN_SOURCE: &str = "ssh://git@github.com/OnixResearch/aspen.git";
pub const ASPEN_REVISION: &str = "171f25b5a9e878390af8f20b9797f6096cca5097";
pub const BOUNDED_EXEC_SOURCE: &str = "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git";
pub const BOUNDED_EXEC_REVISION: &str = "29dac88ecded94457572db3fdfaaaab95fa91525";
pub const ASPEN_NATIVE_HOST_PROFILE_SCHEMA: &str = "molten.system-extension.native-host-profile.v2";
pub const ASPEN_NATIVE_CALLBACK_ENVELOPE_SCHEMA: &str =
    "molten.system-extension.native-callback-envelope.v2";
pub const ASPEN_NATIVE_CALLBACK_OUTCOME_SCHEMA: &str =
    "molten.system-extension.native-callback-outcome.v2";
pub const ASPEN_NATIVE_INSTANCE_SCHEMA: &str = "molten.system-extension.native-instance-state.v2";
pub const ASPEN_NATIVE_INGRESS_SCHEMA: &str = "molten.system-extension.native-ingress.v2";
pub const ASPEN_NATIVE_ALPN: &str = "molten/system-extension/native/v2";
pub const ASPEN_NATIVE_FRAMING: &str = "preserves-packed-materialized-values-v2";
pub const KILN_EXTENSION_PROTOCOL_SCHEMA: &str = "kiln.aspen-extension.protocol.v2";
pub const KILN_EXTENSION_SERVICE_ID: &str = "kiln-ci-control-plane-v2";
pub const KILN_EXTENSION_ID: &str = "kiln-aspen-extension-v2";
pub const KILN_NATIVE_EXECUTION_PROFILE: &str = "native-host-local-pilot-v2";
pub const LOCAL_CLAIM_LEVEL: &str = "local-live-materialized-values-pilot";

pub const DEFAULT_MAX_CALLBACK_INPUT_BYTES: usize = 1_048_576;
pub const DEFAULT_MAX_CALLBACK_OUTPUT_BYTES: usize = 1_048_576;
pub const DEFAULT_MAX_MATERIALIZED_VALUE_BYTES: usize = 262_144;
pub const DEFAULT_MAX_MATERIALIZED_VALUES: usize = 64;
pub const DEFAULT_MAX_CONTENT_BYTES: usize = 16_777_216;
pub const DEFAULT_MAX_LOG_BYTES: usize = 262_144;
pub const DEFAULT_MAX_EFFECTS: usize = 64;
pub const DEFAULT_MAX_RECORDS: usize = 4_096;
pub const DEFAULT_CALLBACK_TIMEOUT_MILLIS: u64 = 30_000;
pub const INITIAL_SERVICE_GENERATION: u64 = 1;

pub(super) const REQUIRED_PORT_COUNT: usize = 7;
pub(super) const REQUIRED_NON_CLAIM_COUNT: usize = 12;

pub const REQUIRED_NON_CLAIMS: [&str; REQUIRED_NON_CLAIM_COUNT] = [
    "aspen acceptance does not prove CI correctness",
    "aspen execution does not prove provider correctness",
    "bounded execution does not prove sandboxing or hermeticity",
    "transport acknowledgement does not prove exact-once delivery",
    "content identity does not prove source trust",
    "durable acceptance does not prove global durability",
    "local conformance does not prove production availability",
    "runtime evidence does not prove release eligibility",
    "equal digest text does not merge identity roles",
    "provider observations require Kiln admission",
    "value identity does not prove application meaning",
    "in-memory conformance does not prove value durability",
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspenPortPurpose {
    Durability,
    Content,
    Execution,
    Transport,
    Time,
    Scheduling,
    Evidence,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct AspenPortBinding {
    pub purpose: AspenPortPurpose,
    pub port_id: String,
    pub version: String,
    pub input_schema: String,
    pub output_schema: String,
    pub authority_ref: String,
    pub resource_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenRuntimeBounds {
    pub max_callback_input_bytes: usize,
    pub max_callback_output_bytes: usize,
    pub max_materialized_value_bytes: usize,
    pub max_materialized_values: usize,
    pub max_content_bytes: usize,
    pub max_log_bytes: usize,
    pub max_effects: usize,
    pub max_records: usize,
    pub callback_timeout_millis: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenSourceCohort {
    pub source: String,
    pub revision: String,
    pub bounded_exec_source: String,
    pub bounded_exec_revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenNativeHostCohort {
    pub profile_schema: String,
    pub callback_envelope_schema: String,
    pub callback_outcome_schema: String,
    pub instance_schema: String,
    pub ingress_schema: String,
    pub extension_protocol_schema: String,
    pub extension_id: String,
    pub service_id: String,
    pub execution_profile: String,
    pub alpn: String,
    pub framing: String,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenRuntimeProfileDocument {
    pub schema: String,
    pub profile_id: String,
    pub cohort: AspenSourceCohort,
    pub native_host: AspenNativeHostCohort,
    pub endpoint_ref: String,
    pub transport_profile_ref: String,
    pub policy_ref: String,
    pub authority_ref: String,
    pub resource_ref: String,
    pub claim_level: String,
    pub ports: Vec<AspenPortBinding>,
    pub port_count: usize,
    pub bounds: AspenRuntimeBounds,
    pub fallback: String,
    pub non_claims: Vec<String>,
    pub non_claim_count: usize,
}

impl AspenRuntimeProfileDocument {
    pub fn into_runtime_profile(self) -> CoreResult<AspenRuntimeProfile> {
        if self.fallback != "none"
            || self.port_count != self.ports.len()
            || self.non_claim_count != self.non_claims.len()
        {
            return Err(Diagnostic::new(
                "aspen_profile_document",
                "aspen.profile.document",
                "Aspen profile document counts or fallback policy are invalid",
            ));
        }
        Ok(AspenRuntimeProfile {
            schema: self.schema,
            profile_id: self.profile_id,
            source: self.cohort.source,
            source_revision: self.cohort.revision,
            bounded_exec_source: self.cohort.bounded_exec_source,
            bounded_exec_revision: self.cohort.bounded_exec_revision,
            native_host_schema: self.native_host.profile_schema,
            native_callback_envelope_schema: self.native_host.callback_envelope_schema,
            native_callback_outcome_schema: self.native_host.callback_outcome_schema,
            native_instance_schema: self.native_host.instance_schema,
            native_ingress_schema: self.native_host.ingress_schema,
            extension_protocol_schema: self.native_host.extension_protocol_schema,
            extension_id: self.native_host.extension_id,
            service_id: self.native_host.service_id,
            execution_profile: self.native_host.execution_profile,
            endpoint_ref: self.endpoint_ref,
            transport_profile_ref: self.transport_profile_ref,
            alpn: self.native_host.alpn,
            framing: self.native_host.framing,
            generation: self.native_host.generation,
            policy_ref: self.policy_ref,
            authority_ref: self.authority_ref,
            resource_ref: self.resource_ref,
            claim_level: self.claim_level,
            ports: self.ports,
            bounds: self.bounds,
            non_claims: self.non_claims,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenRuntimeProfile {
    pub schema: String,
    pub profile_id: String,
    pub source: String,
    pub source_revision: String,
    pub bounded_exec_source: String,
    pub bounded_exec_revision: String,
    pub native_host_schema: String,
    pub native_callback_envelope_schema: String,
    pub native_callback_outcome_schema: String,
    pub native_instance_schema: String,
    pub native_ingress_schema: String,
    pub extension_protocol_schema: String,
    pub extension_id: String,
    pub service_id: String,
    pub execution_profile: String,
    pub endpoint_ref: String,
    pub transport_profile_ref: String,
    pub alpn: String,
    pub framing: String,
    pub generation: u64,
    pub policy_ref: String,
    pub authority_ref: String,
    pub resource_ref: String,
    pub claim_level: String,
    pub ports: Vec<AspenPortBinding>,
    pub bounds: AspenRuntimeBounds,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedAspenRuntimeProfile {
    pub profile: AspenRuntimeProfile,
    pub profile_identity: ContentId,
}
