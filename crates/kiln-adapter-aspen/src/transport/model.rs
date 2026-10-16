use kiln_core::types::{ContentId, ExternalRef, RunId};
use kiln_provider_api::TriggerRequest;
use serde::{Deserialize, Serialize};

use crate::profile::AdmittedAspenRuntimeProfile;

pub const SERVICE_INGRESS_SCHEMA: &str = "kiln.aspen-runtime.service-ingress.v2";
pub const SERVICE_ACK_SCHEMA: &str = "kiln.aspen-runtime.service-ack.v2";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngressSource {
    Radicle,
    Operator,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenServiceIngress {
    pub schema: String,
    pub source: IngressSource,
    pub service_id: String,
    pub generation: u64,
    pub profile_identity: ContentId,
    pub alpn: String,
    pub framing: String,
    pub operation_id: ExternalRef,
    pub idempotency_key: ExternalRef,
    pub authority_ref: String,
    pub resource_ref: String,
    pub trigger_identity: ContentId,
    pub trigger: TriggerRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspenServiceDecision {
    Accepted,
    Rejected,
    Replay,
    Completed,
    Failed,
    Denied,
    Cancelled,
    TimedOut,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenServiceAck {
    pub schema: String,
    pub operation_id: ExternalRef,
    pub generation: u64,
    pub trigger_identity: ContentId,
    pub run_id: Option<RunId>,
    pub decision: AspenServiceDecision,
    pub receipt_ref: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ServiceTransportOutcome {
    Acknowledged(AspenServiceAck),
    Rejected(AspenServiceAck),
    DisconnectedAfterAcceptance { operation_id: ExternalRef },
    TimedOut,
    Cancelled,
    Unavailable,
    Malformed,
}

pub trait AspenServiceTransport {
    fn submit(
        &mut self,
        profile: &AdmittedAspenRuntimeProfile,
        request: &AspenServiceIngress,
    ) -> ServiceTransportOutcome;

    fn reconcile(
        &mut self,
        profile: &AdmittedAspenRuntimeProfile,
        operation_id: &ExternalRef,
    ) -> ServiceTransportOutcome;
}
