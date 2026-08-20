use std::collections::BTreeMap;
use std::path::Component;
use std::path::PathBuf;

use kiln_codec::identity_from_bytes;
use kiln_core::types::Diagnostic;
use kiln_provider_api::InfrastructureFailure;
use kiln_provider_api::PortFailure;
use kiln_provider_api::PortResult;
use kiln_provider_api::WorkflowExchangeOperation;
use kiln_provider_api::WorkflowExchangePort;
use kiln_provider_api::WorkflowExchangeRequest;
use kiln_provider_api::WorkflowExchangeResponse;
use lattice_workflow_contract::CapabilityProfile;
use lattice_workflow_contract::ControlAction;
use lattice_workflow_contract::ControlRequest;
use lattice_workflow_contract::LogReadRequest;
use lattice_workflow_contract::WorkflowAccepted;
use lattice_workflow_contract::WorkflowRequest;
use lattice_workflow_contract::validate_profile;
use lattice_workflow_transport::ClientFailure;
use lattice_workflow_transport::ExchangeCommand;
use lattice_workflow_transport::ExchangeResult;
use lattice_workflow_transport::ExchangeSuccess;
use lattice_workflow_transport::Rejection;
use lattice_workflow_transport::RejectionCode;
use lattice_workflow_transport::TransportProfile;
use lattice_workflow_transport::UnixClient;
use lattice_workflow_transport::UnixEndpoint;
use lattice_workflow_transport::new_request;
use lattice_workflow_transport::validate_transport_profile;

#[derive(Clone, Debug)]
struct ExecutionLink {
    request: WorkflowRequest,
    accepted: WorkflowAccepted,
}

// r[impl kiln.lattice.transport_client]
#[derive(Debug)]
pub struct UnixWorkflowExchange {
    socket_path: PathBuf,
    transport_profile: TransportProfile,
    contract_profile: CapabilityProfile,
    maximum_records: usize,
    records: BTreeMap<String, ExecutionLink>,
}

impl UnixWorkflowExchange {
    pub fn new(
        socket_path: PathBuf,
        transport_profile: TransportProfile,
        contract_profile: CapabilityProfile,
        maximum_records: usize,
    ) -> PortResult<Self> {
        validate_path(&socket_path)?;
        validate_transport_profile(&transport_profile).map_err(|_| {
            domain(
                "lattice_transport_profile",
                "lattice.transport.profile",
                "Lattice transport profile is not admitted",
            )
        })?;
        validate_profile(&contract_profile).map_err(|_| {
            domain(
                "lattice_contract_profile",
                "lattice.transport.contract",
                "Lattice contract profile is not admitted",
            )
        })?;
        if maximum_records == 0 {
            return Err(domain(
                "lattice_transport_records",
                "lattice.transport.maximum_records",
                "Lattice transport record bound must be positive",
            ));
        }
        Ok(Self {
            socket_path,
            transport_profile,
            contract_profile,
            maximum_records,
            records: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn transport_profile(&self) -> &TransportProfile {
        &self.transport_profile
    }

    #[must_use]
    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }

    fn exchange_command(
        &mut self,
        request: &WorkflowExchangeRequest,
    ) -> PortResult<(ExchangeCommand, Option<WorkflowRequest>)> {
        match request.operation {
            WorkflowExchangeOperation::Dispatch => {
                let workflow: WorkflowRequest = decode(&request.body)?;
                Ok((
                    ExchangeCommand::Dispatch {
                        profile: self.contract_profile.clone(),
                        request: workflow.clone(),
                    },
                    Some(workflow),
                ))
            }
            WorkflowExchangeOperation::RecoverDispatch => {
                let workflow: WorkflowRequest = decode(&request.body)?;
                Ok((
                    ExchangeCommand::RecoverDispatch {
                        profile: self.contract_profile.clone(),
                        request: workflow.clone(),
                    },
                    Some(workflow),
                ))
            }
            WorkflowExchangeOperation::Observe | WorkflowExchangeOperation::Reconcile => {
                let accepted: WorkflowAccepted = decode(&request.body)?;
                let workflow = self.workflow_for(&accepted.request_identity)?;
                let command = if request.operation == WorkflowExchangeOperation::Observe {
                    ExchangeCommand::Observe {
                        profile: self.contract_profile.clone(),
                        request: workflow,
                        accepted,
                    }
                } else {
                    ExchangeCommand::Reconcile {
                        profile: self.contract_profile.clone(),
                        request: workflow,
                        accepted,
                    }
                };
                Ok((command, None))
            }
            WorkflowExchangeOperation::Cancel
            | WorkflowExchangeOperation::Pause
            | WorkflowExchangeOperation::Resume => {
                let control: ControlRequest = decode(&request.body)?;
                validate_control_operation(request.operation, control.action)?;
                let link = self.link_for(&control.request_identity)?;
                Ok((
                    ExchangeCommand::Control {
                        profile: self.contract_profile.clone(),
                        request: link.request,
                        accepted: link.accepted,
                        current_sequence: control.expected_sequence,
                        control,
                    },
                    None,
                ))
            }
            WorkflowExchangeOperation::ReadLog => {
                let log_read: LogReadRequest = decode(&request.body)?;
                let link = self.link_for(&log_read.request_identity)?;
                Ok((
                    ExchangeCommand::ReadLog {
                        profile: self.contract_profile.clone(),
                        request: link.request,
                        accepted: link.accepted,
                        log_read,
                    },
                    None,
                ))
            }
        }
    }

    fn link_for(
        &self,
        request_identity: &lattice_workflow_contract::Identity,
    ) -> PortResult<ExecutionLink> {
        self.records
            .get(request_identity.as_str())
            .cloned()
            .ok_or_else(|| {
                domain(
                    "lattice_transport_link",
                    "lattice.transport.request_identity",
                    "Lattice transport has no accepted link for this request",
                )
            })
    }

    fn workflow_for(
        &self,
        request_identity: &lattice_workflow_contract::Identity,
    ) -> PortResult<WorkflowRequest> {
        self.link_for(request_identity).map(|link| link.request)
    }

    fn remember(
        &mut self,
        workflow: WorkflowRequest,
        accepted: WorkflowAccepted,
    ) -> PortResult<()> {
        let key = workflow.identity.as_str().to_string();
        if let Some(existing) = self.records.get(&key) {
            if existing.request != workflow || existing.accepted != accepted {
                return Err(domain(
                    "lattice_transport_conflict",
                    "lattice.transport.accepted",
                    "Lattice transport returned conflicting acceptance facts",
                ));
            }
            return Ok(());
        }
        if self.records.len() >= self.maximum_records {
            return Err(PortFailure::Retryable(infrastructure(
                "lattice_transport_record_bound",
                "Lattice transport reached its accepted-record bound",
            )));
        }
        self.records.insert(
            key,
            ExecutionLink {
                request: workflow,
                accepted,
            },
        );
        Ok(())
    }
}

impl WorkflowExchangePort for UnixWorkflowExchange {
    // r[impl kiln.lattice.transport_client]
    // r[impl kiln.lattice.transport_recovery]
    fn exchange(
        &mut self,
        request: WorkflowExchangeRequest,
    ) -> PortResult<WorkflowExchangeResponse> {
        if identity_from_bytes(&request.body) != Ok(request.body_identity.clone()) {
            return Err(domain(
                "lattice_transport_body_identity",
                "lattice.transport.body",
                "Lattice transport body identity does not match its bytes",
            ));
        }
        if request.max_response_bytes == 0
            || request.max_response_bytes
                > usize::try_from(self.transport_profile.limits.maximum_frame_bytes).map_err(
                    |_| {
                        domain(
                            "lattice_transport_response_bound",
                            "lattice.transport.max_response_bytes",
                            "Lattice transport response bound does not fit this platform",
                        )
                    },
                )?
        {
            return Err(domain(
                "lattice_transport_response_bound",
                "lattice.transport.max_response_bytes",
                "Lattice transport response bound is invalid",
            ));
        }
        let (command, workflow) = self.exchange_command(&request)?;
        let native_request = new_request(command).map_err(|_| {
            domain(
                "lattice_transport_request",
                "lattice.transport.request",
                "Lattice transport request failed producer admission",
            )
        })?;
        let endpoint = UnixEndpoint {
            socket_path: &self.socket_path,
            profile: &self.transport_profile,
        };
        let native_response = UnixClient::new(endpoint)
            .exchange(&native_request)
            .map_err(map_client_failure)?;
        match native_response.result {
            ExchangeResult::Success(success) => {
                let (body, log_bytes) = match *success {
                    ExchangeSuccess::Accepted(accepted) => {
                        let Some(workflow) = workflow else {
                            return Err(domain(
                                "lattice_transport_acceptance",
                                "lattice.transport.response",
                                "Lattice acceptance was returned for another operation",
                            ));
                        };
                        self.remember(workflow, accepted.clone())?;
                        (encode(&accepted)?, Vec::new())
                    }
                    ExchangeSuccess::Observation(observation) => {
                        (encode(&observation)?, Vec::new())
                    }
                    ExchangeSuccess::Log { observation, bytes } => (encode(&observation)?, bytes),
                };
                Ok(WorkflowExchangeResponse {
                    body,
                    log_bytes,
                    disconnected_after_acceptance: false,
                })
            }
            ExchangeResult::Rejected(rejection) => Err(map_rejection(rejection)),
        }
    }
}

fn validate_path(path: &std::path::Path) -> PortResult<()> {
    let has_unsafe_component = path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir));
    if !path.is_absolute() || path.file_name().is_none() || has_unsafe_component {
        return Err(domain(
            "lattice_transport_path",
            "lattice.transport.socket_path",
            "Lattice transport socket path is not an admitted absolute path",
        ));
    }
    Ok(())
}

fn validate_control_operation(
    operation: WorkflowExchangeOperation,
    action: ControlAction,
) -> PortResult<()> {
    let matches = matches!(
        (operation, action),
        (WorkflowExchangeOperation::Cancel, ControlAction::Cancel)
            | (WorkflowExchangeOperation::Pause, ControlAction::Pause)
            | (WorkflowExchangeOperation::Resume, ControlAction::Resume)
    );
    if !matches {
        return Err(domain(
            "lattice_transport_control",
            "lattice.transport.control",
            "Lattice transport operation does not match its control action",
        ));
    }
    Ok(())
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> PortResult<T> {
    serde_json::from_slice(bytes).map_err(|_| {
        domain(
            "lattice_transport_decode",
            "lattice.transport.body",
            "Lattice transport body does not match the selected operation",
        )
    })
}

fn encode<T: serde::Serialize>(value: &T) -> PortResult<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| {
        PortFailure::Infrastructure(infrastructure(
            "lattice_transport_encode",
            "Lattice transport response could not be encoded",
        ))
    })
}

fn map_client_failure(failure: ClientFailure) -> PortFailure {
    match failure {
        ClientFailure::InvalidProfile | ClientFailure::InvalidRequest => domain(
            "lattice_transport_request",
            "lattice.transport",
            "Lattice transport rejected the local request before connection",
        ),
        ClientFailure::ConnectFailed => PortFailure::Infrastructure(infrastructure(
            "lattice_transport_connect",
            "Lattice transport endpoint is unavailable",
        )),
        ClientFailure::NotAccepted => PortFailure::Retryable(infrastructure(
            "lattice_transport_not_accepted",
            "Lattice transport request was not fully written",
        )),
        ClientFailure::UnknownAfterWrite => PortFailure::Unknown(infrastructure(
            "lattice_transport_unknown",
            "Lattice transport outcome is unknown after request write",
        )),
    }
}

fn map_rejection(rejection: Rejection) -> PortFailure {
    match rejection.code {
        RejectionCode::NotFound => PortFailure::Unknown(infrastructure(
            "lattice_transport_not_found",
            "Lattice transport could not find the exact dispatch request",
        )),
        RejectionCode::HandlerUnavailable => PortFailure::Infrastructure(infrastructure(
            "lattice_transport_handler",
            "Lattice workflow handler is unavailable",
        )),
        RejectionCode::FrameTooLarge | RejectionCode::LogTooLarge => domain(
            "lattice_transport_limit",
            "lattice.transport.response",
            "Lattice transport value exceeds the admitted bound",
        ),
        RejectionCode::InvalidSchema
        | RejectionCode::InvalidIdentity
        | RejectionCode::InvalidContract
        | RejectionCode::InvalidOperationResult
        | RejectionCode::PeerDenied
        | RejectionCode::EncodingFailure => domain(
            "lattice_transport_rejected",
            "lattice.transport.response",
            "Lattice transport rejected the exchange",
        ),
    }
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}
