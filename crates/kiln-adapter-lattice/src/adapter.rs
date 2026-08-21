use std::collections::BTreeMap;

use kiln_core::types::{Diagnostic, EffectId};
use kiln_core::workflow::{
    WorkflowControlAction, WorkflowControlRequest, WorkflowEffectPlan, WorkflowLogChunk,
    WorkflowLogReadRequest, WorkflowObservation,
};
use kiln_provider_api::{
    InfrastructureFailure, PortFailure, PortResult, WorkflowExchangeOperation,
    WorkflowExchangePort, WorkflowExchangeRequest, WorkflowExchangeResponse, WorkflowPort,
};
use lattice_workflow_contract::{
    CONTROL_SCHEMA, ControlAction, ControlRequest, ExecutionObservation, Identity, LOG_READ_SCHEMA,
    Label, LogReadRequest, WorkflowAccepted, WorkflowRequest, observation_identity,
    validate_control_request, validate_log_read_request,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::mapping::{
    accepted_observation, admitted_observation, body_identity, lattice_stream, lower_request,
    matching_log_reference,
};
use crate::profile::{LatticeProfile, admit_profile};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutionRecord {
    plan: WorkflowEffectPlan,
    request: WorkflowRequest,
    accepted: WorkflowAccepted,
    last_sequence: u64,
    last_observation_identity: Option<Identity>,
    last_observation: WorkflowObservation,
}

// r[impl kiln.lattice.conformance]
#[derive(Debug)]
pub struct LatticeAdapter<T> {
    profile: LatticeProfile,
    transport: T,
    executions: BTreeMap<EffectId, ExecutionRecord>,
}

impl<T: WorkflowExchangePort> LatticeAdapter<T> {
    pub fn new(profile: LatticeProfile, transport: T) -> PortResult<Self> {
        admit_profile(&profile)?;
        Ok(Self {
            profile,
            transport,
            executions: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    #[must_use]
    pub fn into_transport(self) -> T {
        self.transport
    }

    fn exchange<Request: Serialize>(
        &mut self,
        operation: WorkflowExchangeOperation,
        request: &Request,
    ) -> PortResult<WorkflowExchangeResponse> {
        let body = serde_json::to_vec(request).map_err(|_| {
            infrastructure(
                "lattice_request_encoding",
                "Lattice request could not be encoded",
            )
        })?;
        if body.len() > self.profile.limits.max_request_bytes {
            return Err(domain(
                "lattice_request_bound",
                "lattice.request",
                "Lattice request exceeds the selected profile bound",
            ));
        }
        let response = self.transport.exchange(WorkflowExchangeRequest {
            operation,
            body_identity: body_identity(&body)?,
            body,
            max_response_bytes: self.profile.limits.max_response_bytes,
        })?;
        if response.body.len() > self.profile.limits.max_response_bytes {
            return Err(infrastructure(
                "lattice_response_bound",
                "Lattice response exceeds the selected profile bound",
            ));
        }
        Ok(response)
    }

    fn decode<Response: DeserializeOwned>(&self, bytes: &[u8]) -> PortResult<Response> {
        serde_json::from_slice(bytes).map_err(|_| {
            infrastructure(
                "lattice_response_encoding",
                "Lattice response is not valid for the pinned schema",
            )
        })
    }

    fn record(&self, effect_id: &EffectId) -> PortResult<ExecutionRecord> {
        self.executions.get(effect_id).cloned().ok_or_else(|| {
            domain(
                "lattice_execution_missing",
                "lattice.execution",
                "Lattice execution has not been accepted for this effect",
            )
        })
    }

    fn remember_acceptance(
        &mut self,
        plan: &WorkflowEffectPlan,
        request: WorkflowRequest,
        response: WorkflowExchangeResponse,
    ) -> PortResult<WorkflowObservation> {
        let accepted: WorkflowAccepted = self.decode(&response.body)?;
        let observation = accepted_observation(
            plan,
            &request,
            &accepted,
            response.disconnected_after_acceptance,
        )?;
        self.executions.insert(
            plan.effect_id.clone(),
            ExecutionRecord {
                plan: plan.clone(),
                request,
                accepted,
                last_sequence: 0,
                last_observation_identity: None,
                last_observation: observation.clone(),
            },
        );
        Ok(observation)
    }

    fn exchange_observation(
        &mut self,
        effect_id: &EffectId,
        operation: WorkflowExchangeOperation,
    ) -> PortResult<(ExecutionObservation, WorkflowObservation)> {
        let record = self.record(effect_id)?;
        let response = self.exchange(operation, &record.accepted)?;
        if response.disconnected_after_acceptance {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "lattice_observation_disconnect".to_string(),
                message: "Lattice observation disconnected after request acceptance".to_string(),
            }));
        }
        let observation: ExecutionObservation = self.decode(&response.body)?;
        let mapped = admitted_observation(
            &self.profile,
            &record.plan,
            &record.request,
            &record.accepted,
            &observation,
        )?;
        self.update_observation(effect_id, &observation, &mapped)?;
        Ok((observation, mapped))
    }

    fn update_observation(
        &mut self,
        effect_id: &EffectId,
        observation: &ExecutionObservation,
        mapped: &WorkflowObservation,
    ) -> PortResult<()> {
        let record = self.executions.get_mut(effect_id).ok_or_else(|| {
            domain(
                "lattice_execution_missing",
                "lattice.execution",
                "Lattice execution disappeared before observation update",
            )
        })?;
        if observation.sequence < record.last_sequence {
            return Err(domain(
                "lattice_observation_stale",
                "lattice.observation.sequence",
                "Lattice observation is older than the active execution state",
            ));
        }
        if observation.sequence == record.last_sequence
            && record
                .last_observation_identity
                .as_ref()
                .is_some_and(|identity| identity != &observation.identity)
        {
            return Err(domain(
                "lattice_observation_conflict",
                "lattice.observation.sequence",
                "Lattice returned conflicting observations for one sequence",
            ));
        }
        record.last_sequence = observation.sequence;
        record.last_observation_identity = Some(observation_identity(observation));
        record.last_observation = mapped.clone();
        Ok(())
    }
}

impl<T: WorkflowExchangePort> WorkflowPort for LatticeAdapter<T> {
    fn profile(&self) -> &kiln_core::provider::CapabilityProfile {
        &self.profile.provider
    }

    // r[impl kiln.lattice.request] r[impl kiln.lattice.reconciliation.scenario.disconnect]
    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let request = lower_request(&self.profile, plan)?;
        if let Some(record) = self.executions.get(&plan.effect_id) {
            if record.plan != *plan || record.request.identity != request.identity {
                return Err(domain(
                    "lattice_dispatch_conflict",
                    "lattice.request",
                    "repeated dispatch differs from the accepted request",
                ));
            }
            return Ok(record.last_observation.clone());
        }
        if self.executions.len() >= self.profile.limits.max_in_flight {
            return Err(PortFailure::Retryable(InfrastructureFailure {
                code: "lattice_in_flight_bound".to_string(),
                message: "Lattice adapter reached its in-flight execution bound".to_string(),
            }));
        }
        let response = match self.exchange(WorkflowExchangeOperation::Dispatch, &request) {
            Ok(response) => response,
            Err(PortFailure::Unknown(_failure)) => {
                self.exchange(WorkflowExchangeOperation::RecoverDispatch, &request)?
            }
            Err(failure) => return Err(failure),
        };
        self.remember_acceptance(plan, request, response)
    }

    // r[impl kiln.lattice.transport_recovery]
    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let request = lower_request(&self.profile, plan)?;
        if let Some(record) = self.executions.get(&plan.effect_id) {
            if record.plan != *plan || record.request.identity != request.identity {
                return Err(domain(
                    "lattice_recovery_conflict",
                    "lattice.request",
                    "dispatch recovery differs from the accepted request",
                ));
            }
            return Ok(record.last_observation.clone());
        }
        let response = self.exchange(WorkflowExchangeOperation::RecoverDispatch, &request)?;
        self.remember_acceptance(plan, request, response)
    }

    fn observe(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation> {
        self.exchange_observation(effect_id, WorkflowExchangeOperation::Observe)
            .map(|(_native, mapped)| mapped)
    }

    fn control(&mut self, request: &WorkflowControlRequest) -> PortResult<WorkflowObservation> {
        let record = self.record(&request.effect_id)?;
        if request.execution.native_id.as_str() != record.accepted.execution_identity.as_str()
            || request.expected_sequence != record.last_sequence
        {
            return Err(domain(
                "lattice_control_stale",
                "lattice.control",
                "Lattice control does not name the current accepted execution and sequence",
            ));
        }
        let (operation, action, is_requested) = match request.action {
            WorkflowControlAction::Cancel => (
                WorkflowExchangeOperation::Cancel,
                ControlAction::Cancel,
                record.plan.controls.cancel,
            ),
            WorkflowControlAction::Pause => (
                WorkflowExchangeOperation::Pause,
                ControlAction::Pause,
                record.plan.controls.pause,
            ),
            WorkflowControlAction::Resume => (
                WorkflowExchangeOperation::Resume,
                ControlAction::Resume,
                record.plan.controls.resume,
            ),
        };
        if !is_requested {
            return Err(domain(
                "lattice_control_not_requested",
                "lattice.control",
                "workflow plan did not request this Lattice control capability",
            ));
        }
        let control = ControlRequest {
            schema: CONTROL_SCHEMA.to_string(),
            request_identity: record.accepted.request_identity.clone(),
            workflow_identity: record.accepted.workflow_identity.clone(),
            execution_identity: record.accepted.execution_identity.clone(),
            expected_sequence: request.expected_sequence,
            action,
        };
        validate_control_request(
            &self.profile.contract,
            &record.request,
            &record.accepted,
            record.last_sequence,
            &control,
        )
        .map_err(contract_error)?;
        let response = self.exchange(operation, &control)?;
        if response.disconnected_after_acceptance {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "lattice_control_disconnect".to_string(),
                message: "Lattice control outcome is unknown after disconnect".to_string(),
            }));
        }
        let observation: ExecutionObservation = self.decode(&response.body)?;
        let mapped = admitted_observation(
            &self.profile,
            &record.plan,
            &record.request,
            &record.accepted,
            &observation,
        )?;
        self.update_observation(&request.effect_id, &observation, &mapped)?;
        Ok(mapped)
    }

    fn read_log(&mut self, request: &WorkflowLogReadRequest) -> PortResult<WorkflowLogChunk> {
        let record = self.record(&request.effect_id)?;
        if request.execution.native_id.as_str() != record.accepted.execution_identity.as_str()
            || request.max_bytes == 0
            || request.max_bytes > record.plan.bounds.max_log_read_bytes
        {
            return Err(domain(
                "lattice_log_request",
                "lattice.log",
                "Lattice log request is stale or exceeds its named bound",
            ));
        }
        let log_read = LogReadRequest {
            schema: LOG_READ_SCHEMA.to_string(),
            request_identity: record.accepted.request_identity.clone(),
            workflow_identity: record.accepted.workflow_identity.clone(),
            execution_identity: record.accepted.execution_identity.clone(),
            node_id: Label::new(request.node_id.as_str().to_string()).map_err(|_| {
                domain(
                    "lattice_log_node",
                    "lattice.log.node",
                    "Kiln log node does not fit the Lattice contract",
                )
            })?,
            stream: lattice_stream(request.stream),
            cursor: request.cursor,
            maximum_bytes: u64::try_from(request.max_bytes).map_err(|_| {
                domain(
                    "lattice_log_bound",
                    "lattice.log.bytes",
                    "Kiln log byte bound does not fit the Lattice contract",
                )
            })?,
        };
        validate_log_read_request(&record.request, &record.accepted, &log_read)
            .map_err(contract_error)?;
        let response = self.exchange(WorkflowExchangeOperation::ReadLog, &log_read)?;
        if response.disconnected_after_acceptance {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "lattice_log_disconnect".to_string(),
                message: "Lattice log read outcome is unknown after disconnect".to_string(),
            }));
        }
        if response.log_bytes.len() > request.max_bytes {
            return Err(infrastructure(
                "lattice_log_response_bound",
                "Lattice returned more log bytes than requested",
            ));
        }
        let native: ExecutionObservation = self.decode(&response.body)?;
        let mapped = admitted_observation(
            &self.profile,
            &record.plan,
            &record.request,
            &record.accepted,
            &native,
        )?;
        let reference =
            matching_log_reference(&native, &request.node_id, request.stream, request.cursor)?;
        if reference.bytes != response.log_bytes.len() {
            return Err(infrastructure(
                "lattice_log_length",
                "Lattice log bytes do not match the admitted reference",
            ));
        }
        self.update_observation(&request.effect_id, &native, &mapped)?;
        Ok(WorkflowLogChunk {
            reference,
            bytes: response.log_bytes,
        })
    }

    fn reconcile(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation> {
        self.exchange_observation(effect_id, WorkflowExchangeOperation::Reconcile)
            .map(|(_native, mapped)| mapped)
    }
}

fn contract_error(error: lattice_workflow_contract::ContractError) -> PortFailure {
    domain(
        "lattice_contract",
        error.path,
        "Lattice value failed contract admission",
    )
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

fn infrastructure(code: &str, message: &str) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}
