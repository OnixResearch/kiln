use kiln_core::types::{ContentId, ExternalRef};
use kiln_provider_api::{
    InfrastructureFailure, PortFailure, PortResult, WorkflowExchangeOperation,
    WorkflowExchangePort, WorkflowExchangeRequest, WorkflowExchangeResponse,
};
use serde::{Deserialize, Serialize};

use crate::profile::AdmittedAspenRuntimeProfile;

pub const WORKFLOW_TRANSPORT_SCHEMA: &str = "kiln.aspen-runtime.workflow-transport.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AspenWorkflowExchangeRequest {
    pub schema: String,
    pub generation: u64,
    pub operation_id: ExternalRef,
    pub operation: WorkflowExchangeOperation,
    pub body_identity: ContentId,
    pub body: Vec<u8>,
    pub max_response_bytes: usize,
    pub authority_ref: String,
    pub resource_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AspenWorkflowOutcome {
    Response(WorkflowExchangeResponse),
    DisconnectedAfterAcceptance { operation_id: ExternalRef },
    TimedOut,
    Cancelled,
    Unavailable,
    Malformed,
}

pub trait AspenWorkflowTransportPort {
    fn exchange(
        &mut self,
        profile: &AdmittedAspenRuntimeProfile,
        request: &AspenWorkflowExchangeRequest,
    ) -> AspenWorkflowOutcome;
}

#[derive(Clone, Debug)]
pub struct AspenWorkflowExchange<T> {
    profile: AdmittedAspenRuntimeProfile,
    transport: T,
}

impl<T> AspenWorkflowExchange<T> {
    #[must_use]
    pub const fn new(profile: AdmittedAspenRuntimeProfile, transport: T) -> Self {
        Self { profile, transport }
    }
}

// r[impl kiln.aspen_runtime.effects] r[impl kiln.aspen_runtime.failure]
impl<T: AspenWorkflowTransportPort> WorkflowExchangePort for AspenWorkflowExchange<T> {
    fn exchange(
        &mut self,
        request: WorkflowExchangeRequest,
    ) -> PortResult<WorkflowExchangeResponse> {
        if request.max_response_bytes == 0
            || request.max_response_bytes > self.profile.profile.bounds.max_content_bytes
            || request.body.len() > self.profile.profile.bounds.max_callback_input_bytes
        {
            return Err(domain(
                "aspen_workflow_bound",
                "aspen.workflow",
                "workflow exchange exceeds its admitted request or response bound",
            ));
        }
        let observed_identity =
            ContentId::from_blake3_hex(blake3::hash(&request.body).to_hex().to_string())
                .map_err(PortFailure::Domain)?;
        if observed_identity != request.body_identity {
            return Err(domain(
                "aspen_workflow_identity",
                "aspen.workflow.body_identity",
                "workflow exchange body identity does not match exact bytes",
            ));
        }
        let operation_material = serde_json::to_vec(&(
            WORKFLOW_TRANSPORT_SCHEMA,
            request.operation,
            &request.body_identity,
            self.profile.profile.generation,
        ))
        .map_err(|_| {
            domain(
                "aspen_workflow_encoding",
                "aspen.workflow",
                "workflow exchange operation identity encoding failed",
            )
        })?;
        let operation_digest = blake3::hash(&operation_material).to_hex().to_string();
        let operation_id =
            ExternalRef::new("molten", "transport-operation-blake3", &operation_digest)
                .map_err(PortFailure::Domain)?;
        let envelope = AspenWorkflowExchangeRequest {
            schema: WORKFLOW_TRANSPORT_SCHEMA.to_string(),
            generation: self.profile.profile.generation,
            operation_id,
            operation: request.operation,
            body_identity: request.body_identity,
            body: request.body,
            max_response_bytes: request.max_response_bytes,
            authority_ref: self.profile.profile.authority_ref.clone(),
            resource_ref: self.profile.profile.resource_ref.clone(),
        };
        match self.transport.exchange(&self.profile, &envelope) {
            AspenWorkflowOutcome::Response(response) => {
                if response.body.len() > envelope.max_response_bytes
                    || response.log_bytes.len() > self.profile.profile.bounds.max_log_bytes
                {
                    return Err(domain(
                        "aspen_workflow_response_bound",
                        "aspen.workflow.response",
                        "workflow response exceeds its admitted body or log bound",
                    ));
                }
                Ok(response)
            }
            AspenWorkflowOutcome::DisconnectedAfterAcceptance { operation_id } => {
                Err(PortFailure::Unknown(infrastructure(
                    "aspen_workflow_unknown",
                    &format!(
                        "Aspen workflow operation {operation_id:?} may be accepted; reconcile before retry"
                    ),
                )))
            }
            AspenWorkflowOutcome::TimedOut => Err(PortFailure::TimedOut(infrastructure(
                "aspen_workflow_timeout",
                "Aspen workflow transport exceeded its admitted deadline",
            ))),
            AspenWorkflowOutcome::Cancelled => Err(PortFailure::Cancelled(infrastructure(
                "aspen_workflow_cancelled",
                "Aspen workflow transport was cancelled",
            ))),
            AspenWorkflowOutcome::Unavailable => Err(PortFailure::Infrastructure(infrastructure(
                "aspen_workflow_unavailable",
                "selected Aspen transport port is unavailable and no fallback is allowed",
            ))),
            AspenWorkflowOutcome::Malformed => Err(domain(
                "aspen_workflow_malformed",
                "aspen.workflow.response",
                "Aspen workflow transport returned a malformed response",
            )),
        }
    }
}

fn domain(code: &str, path: &'static str, message: &str) -> PortFailure {
    PortFailure::Domain(kiln_core::types::Diagnostic::new(code, path, message))
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}
