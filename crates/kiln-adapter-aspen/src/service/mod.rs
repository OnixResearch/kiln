use kiln_core::types::ExternalRef;

use crate::profile::AdmittedAspenRuntimeProfile;
use crate::protocol::{
    CALLBACK_REQUEST_SCHEMA, CallbackCommand, CallbackDecision, CallbackOutcome, CallbackRequest,
    HostedRunPhase, ProtocolError, decode_callback_outcome, encode_callback_request, hosted_run_id,
};
use crate::transport::{
    AspenServiceAck, AspenServiceDecision, AspenServiceIngress, AspenServiceTransport,
    SERVICE_ACK_SCHEMA, ServiceTransportOutcome, admit_ingress,
};

mod model;

pub use model::*;

const CALLBACK_SEQUENCE_INCREMENT: u64 = 1;

#[derive(Clone, Debug)]
pub struct LocalAspenService<C> {
    profile: AdmittedAspenRuntimeProfile,
    journal: LocalServiceJournal,
    callback: C,
}

impl<C> LocalAspenService<C> {
    #[must_use]
    pub const fn new(
        profile: AdmittedAspenRuntimeProfile,
        journal: LocalServiceJournal,
        callback: C,
    ) -> Self {
        Self {
            profile,
            journal,
            callback,
        }
    }

    #[must_use]
    pub const fn journal(&self) -> &LocalServiceJournal {
        &self.journal
    }
}

impl<C: HostedCallback> LocalAspenService<C> {
    // r[impl kiln.aspen_runtime.lifecycle] r[impl kiln.aspen_runtime.ingress]
    fn submit_inner(&mut self, ingress: &AspenServiceIngress) -> ServiceTransportOutcome {
        if admit_ingress(&self.profile, ingress).is_err() {
            return ServiceTransportOutcome::Malformed;
        }
        let snapshot = {
            let Ok(mut state) = self.journal.state.lock() else {
                return ServiceTransportOutcome::Unavailable;
            };
            if let Some(existing) = state.acknowledgements.get(&ingress.operation_id) {
                let mut replay = existing.clone();
                replay.decision = AspenServiceDecision::Replay;
                return ServiceTransportOutcome::Acknowledged(replay);
            }
            let fault = state.next_fault;
            state.next_fault = LocalServiceFault::None;
            if fault == LocalServiceFault::TimeoutBeforeCallback {
                return ServiceTransportOutcome::TimedOut;
            }
            if fault == LocalServiceFault::CancelBeforeCallback {
                return ServiceTransportOutcome::Cancelled;
            }
            if fault == LocalServiceFault::UnavailableBeforeCallback {
                return ServiceTransportOutcome::Unavailable;
            }
            (state.callback_state.clone(), fault)
        };
        let Some(sequence) = snapshot.0.revision.checked_add(CALLBACK_SEQUENCE_INCREMENT) else {
            return ServiceTransportOutcome::Malformed;
        };
        let request = CallbackRequest {
            schema: CALLBACK_REQUEST_SCHEMA.to_string(),
            protocol_schema: self.profile.profile.extension_protocol_schema.clone(),
            profile_identity: self.profile.profile_identity.clone(),
            generation: ingress.generation,
            sequence,
            operation_id: ingress.operation_id.clone(),
            authority_ref: ingress.authority_ref.clone(),
            policy_ref: self.profile.profile.policy_ref.clone(),
            resource_ref: ingress.resource_ref.clone(),
            deadline_tick: self.profile.profile.bounds.callback_timeout_millis,
            prior_state: snapshot.0,
            command: CallbackCommand::SubmitTrigger {
                trigger: Box::new(ingress.trigger.clone()),
                trigger_identity: ingress.trigger_identity.clone(),
                idempotency_key: ingress.idempotency_key.clone(),
            },
        };
        let request_bytes = match encode_callback_request(
            &request,
            self.profile.profile.bounds.max_callback_input_bytes,
        ) {
            Ok(bytes) => bytes,
            Err(_) => return ServiceTransportOutcome::Malformed,
        };
        let output_bytes = match self.callback.invoke(
            &request_bytes,
            self.profile.profile.bounds.max_callback_output_bytes,
        ) {
            Ok(bytes) => bytes,
            Err(CallbackInvocationError::TimedOut) => return ServiceTransportOutcome::TimedOut,
            Err(CallbackInvocationError::Cancelled) => return ServiceTransportOutcome::Cancelled,
            Err(CallbackInvocationError::Unavailable) => {
                return ServiceTransportOutcome::Unavailable;
            }
            Err(CallbackInvocationError::Denied(_)) => return ServiceTransportOutcome::Malformed,
        };
        let outcome = match decode_callback_outcome(
            &output_bytes,
            self.profile.profile.bounds.max_callback_output_bytes,
            self.profile.profile.bounds.max_effects,
        ) {
            Ok(outcome) => outcome,
            Err(_) => return ServiceTransportOutcome::Malformed,
        };
        if validate_outcome(&request, &outcome).is_err() {
            return ServiceTransportOutcome::Malformed;
        }
        let Ok(ack) = build_service_ack(ingress, &outcome, &output_bytes) else {
            return ServiceTransportOutcome::Malformed;
        };
        {
            let Ok(mut state) = self.journal.state.lock() else {
                return ServiceTransportOutcome::Unavailable;
            };
            if state.callback_state.revision != request.prior_state.revision {
                return ServiceTransportOutcome::Malformed;
            }
            state.callback_state = outcome.state;
            state
                .acknowledgements
                .insert(ingress.operation_id.clone(), ack.clone());
        }
        if snapshot.1 == LocalServiceFault::DisconnectAfterCommit {
            ServiceTransportOutcome::DisconnectedAfterAcceptance {
                operation_id: ingress.operation_id.clone(),
            }
        } else if matches!(
            ack.decision,
            AspenServiceDecision::Rejected | AspenServiceDecision::Denied
        ) {
            ServiceTransportOutcome::Rejected(ack)
        } else {
            ServiceTransportOutcome::Acknowledged(ack)
        }
    }
}

impl<C: HostedCallback> AspenServiceTransport for LocalAspenService<C> {
    fn submit(
        &mut self,
        profile: &AdmittedAspenRuntimeProfile,
        request: &AspenServiceIngress,
    ) -> ServiceTransportOutcome {
        if profile.profile_identity != self.profile.profile_identity {
            return ServiceTransportOutcome::Unavailable;
        }
        self.submit_inner(request)
    }

    fn reconcile(
        &mut self,
        profile: &AdmittedAspenRuntimeProfile,
        operation_id: &ExternalRef,
    ) -> ServiceTransportOutcome {
        if profile.profile_identity != self.profile.profile_identity {
            return ServiceTransportOutcome::Unavailable;
        }
        let Ok(state) = self.journal.state.lock() else {
            return ServiceTransportOutcome::Unavailable;
        };
        state.acknowledgements.get(operation_id).cloned().map_or(
            ServiceTransportOutcome::Unavailable,
            ServiceTransportOutcome::Acknowledged,
        )
    }
}

fn validate_outcome(
    request: &CallbackRequest,
    outcome: &CallbackOutcome,
) -> Result<(), ProtocolError> {
    if outcome.generation != request.generation
        || outcome.operation_id != request.operation_id
        || outcome.state.generation != request.generation
        || outcome.state.revision != request.sequence
    {
        return Err(ProtocolError::Schema);
    }
    Ok(())
}

// r[impl kiln.aspen_host.transport] r[impl kiln.aspen_host.completion]
pub fn build_service_ack(
    ingress: &AspenServiceIngress,
    outcome: &CallbackOutcome,
    output_bytes: &[u8],
) -> Result<AspenServiceAck, ProtocolError> {
    let run = outcome.state.runs.get(ingress.trigger_identity.as_str());
    let run_id = run
        .map(|state| state.run_id.clone())
        .map_or_else(|| hosted_run_id(&ingress.trigger_identity), Ok)
        .map_err(|_| ProtocolError::Identity)?;
    let decision = match outcome.decision {
        CallbackDecision::Rejected => AspenServiceDecision::Rejected,
        CallbackDecision::Replay => AspenServiceDecision::Replay,
        CallbackDecision::Stopped => AspenServiceDecision::Completed,
        CallbackDecision::Accepted
        | CallbackDecision::Reconciled
        | CallbackDecision::Checkpointed
        | CallbackDecision::Recovered
        | CallbackDecision::Status
        | CallbackDecision::Draining => run.map_or(AspenServiceDecision::Accepted, |state| {
            decision_for_phase(state.phase)
        }),
    };
    let receipt_material = [
        ingress.operation_id.native_id.as_str().as_bytes(),
        output_bytes,
    ]
    .concat();
    let receipt_digest = blake3::hash(&receipt_material).to_hex().to_string();
    let receipt_ref = ExternalRef::new("molten", "service-receipt-blake3", &receipt_digest)
        .map_err(|_| ProtocolError::Identity)?;
    Ok(AspenServiceAck {
        schema: SERVICE_ACK_SCHEMA.to_string(),
        operation_id: ingress.operation_id.clone(),
        generation: ingress.generation,
        trigger_identity: ingress.trigger_identity.clone(),
        run_id: Some(run_id),
        decision,
        receipt_ref,
    })
}

const fn decision_for_phase(phase: HostedRunPhase) -> AspenServiceDecision {
    match phase {
        HostedRunPhase::Accepted | HostedRunPhase::DispatchPending | HostedRunPhase::Running => {
            AspenServiceDecision::Accepted
        }
        HostedRunPhase::Completed => AspenServiceDecision::Completed,
        HostedRunPhase::Failed => AspenServiceDecision::Failed,
        HostedRunPhase::Denied => AspenServiceDecision::Denied,
        HostedRunPhase::Cancelled => AspenServiceDecision::Cancelled,
        HostedRunPhase::TimedOut => AspenServiceDecision::TimedOut,
        HostedRunPhase::Unknown => AspenServiceDecision::Unknown,
    }
}
