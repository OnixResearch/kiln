use kiln_core::types::{ContentId, ExternalRef};
use kiln_provider_api::{
    InfrastructureFailure, PortFailure, PortResult, TriggerIntakePort, TriggerLifecyclePolicy,
    TriggerRequest, TriggerRun, TriggerTerminal,
};

use super::*;
use crate::profile::{ASPEN_NATIVE_ALPN, ASPEN_NATIVE_FRAMING, AdmittedAspenRuntimeProfile};

#[derive(Clone, Debug)]
pub struct AspenTriggerClient<T> {
    profile: AdmittedAspenRuntimeProfile,
    source: IngressSource,
    transport: T,
    last_operation: Option<ExternalRef>,
}

impl<T> AspenTriggerClient<T> {
    #[must_use]
    pub const fn new(
        profile: AdmittedAspenRuntimeProfile,
        source: IngressSource,
        transport: T,
    ) -> Self {
        Self {
            profile,
            source,
            transport,
            last_operation: None,
        }
    }

    #[must_use]
    pub const fn transport(&self) -> &T {
        &self.transport
    }

    pub const fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    #[must_use]
    pub const fn last_operation(&self) -> Option<&ExternalRef> {
        self.last_operation.as_ref()
    }
}

pub type RadicleAspenIngressClient<T> = AspenTriggerClient<T>;
pub type OperatorAspenIngressClient<T> = AspenTriggerClient<T>;

impl<T: AspenServiceTransport> TriggerIntakePort for AspenTriggerClient<T> {
    // r[impl kiln.aspen_runtime.ingress] r[impl kiln.aspen_runtime.failure]
    fn accept(&mut self, trigger: TriggerRequest) -> PortResult<TriggerRun> {
        let request = build_ingress(&self.profile, self.source, trigger)?;
        self.last_operation = Some(request.operation_id.clone());
        classify_transport(self.transport.submit(&self.profile, &request))
    }

    fn observe(
        &mut self,
        accepted: &TriggerRun,
        policy: TriggerLifecyclePolicy,
    ) -> PortResult<TriggerRun> {
        if policy.max_polls == 0 {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_ingress_zero_poll_bound",
                "aspen.ingress.max_polls",
                "Aspen trigger observation requires a nonzero poll bound",
            )));
        }
        let operation = self.last_operation.as_ref().ok_or_else(|| {
            PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_ingress_missing_operation",
                "aspen.ingress.operation",
                "Aspen trigger observation requires the accepted operation identity",
            ))
        })?;
        let observed = classify_transport(self.transport.reconcile(&self.profile, operation))?;
        if observed.run_id != accepted.run_id {
            return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_ingress_run_drift",
                "aspen.ingress.run_id",
                "reconciled Aspen service response names a different Kiln run",
            )));
        }
        Ok(observed)
    }
}

// r[impl kiln.aspen_runtime.ingress] r[impl kiln.aspen_runtime.identity]
pub fn canonical_trigger_identity(
    trigger: &TriggerRequest,
    maximum_bytes: usize,
) -> PortResult<ContentId> {
    kiln_codec::encode_and_identify(trigger, maximum_bytes)
        .map(|(_bytes, identity)| identity)
        .map_err(|_| {
            PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_ingress_encoding",
                "aspen.ingress.trigger",
                "canonical trigger encoding failed or exceeded its admitted bound",
            ))
        })
}

// r[impl kiln.aspen_runtime.ingress] r[impl kiln.aspen_runtime.identity]
pub fn build_ingress(
    profile: &AdmittedAspenRuntimeProfile,
    source: IngressSource,
    trigger: TriggerRequest,
) -> PortResult<AspenServiceIngress> {
    trigger.repository.validate().map_err(PortFailure::Domain)?;
    trigger.actor.validate().map_err(PortFailure::Domain)?;
    trigger.after.validate().map_err(PortFailure::Domain)?;
    if let Some(before) = &trigger.before {
        before.validate().map_err(PortFailure::Domain)?;
    }
    if let Some(patch) = &trigger.patch {
        patch.validate().map_err(PortFailure::Domain)?;
    }
    let trigger_identity =
        canonical_trigger_identity(&trigger, profile.profile.bounds.max_callback_input_bytes)?;
    let source_name = match source {
        IngressSource::Radicle => "radicle",
        IngressSource::Operator => "operator",
    };
    let operation_material = format!(
        "{}\0{}\0{}\0{}",
        profile.profile.service_id,
        profile.profile.generation,
        source_name,
        trigger_identity.as_str()
    );
    let operation_digest = blake3::hash(operation_material.as_bytes())
        .to_hex()
        .to_string();
    let operation_id = ExternalRef::new("molten", "service-operation-blake3", &operation_digest)
        .map_err(PortFailure::Domain)?;
    let idempotency_key = ExternalRef::new(
        "kiln",
        "trigger-idempotency-blake3",
        trigger_identity.as_str(),
    )
    .map_err(PortFailure::Domain)?;
    Ok(AspenServiceIngress {
        schema: SERVICE_INGRESS_SCHEMA.to_string(),
        source,
        service_id: profile.profile.service_id.clone(),
        generation: profile.profile.generation,
        profile_identity: profile.profile_identity.clone(),
        alpn: ASPEN_NATIVE_ALPN.to_string(),
        framing: ASPEN_NATIVE_FRAMING.to_string(),
        operation_id,
        idempotency_key,
        authority_ref: profile.profile.authority_ref.clone(),
        resource_ref: profile.profile.resource_ref.clone(),
        trigger_identity,
        trigger,
    })
}

// r[impl kiln.aspen_runtime.ingress]
pub fn admit_ingress(
    profile: &AdmittedAspenRuntimeProfile,
    request: &AspenServiceIngress,
) -> PortResult<()> {
    if request.schema != SERVICE_INGRESS_SCHEMA
        || request.service_id != profile.profile.service_id
        || request.generation != profile.profile.generation
        || request.profile_identity != profile.profile_identity
        || request.alpn != ASPEN_NATIVE_ALPN
        || request.framing != ASPEN_NATIVE_FRAMING
        || request.authority_ref != profile.profile.authority_ref
        || request.resource_ref != profile.profile.resource_ref
    {
        return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
            "aspen_ingress_profile_drift",
            "aspen.ingress",
            "service ingress does not match the active exact Aspen profile",
        )));
    }
    let rebuilt = build_ingress(profile, request.source, request.trigger.clone())?;
    if rebuilt.operation_id != request.operation_id
        || rebuilt.idempotency_key != request.idempotency_key
        || rebuilt.trigger_identity != request.trigger_identity
    {
        return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
            "aspen_ingress_identity_drift",
            "aspen.ingress.identity",
            "service ingress identity does not match its canonical trigger",
        )));
    }
    Ok(())
}

fn classify_transport(outcome: ServiceTransportOutcome) -> PortResult<TriggerRun> {
    match outcome {
        ServiceTransportOutcome::Acknowledged(ack) | ServiceTransportOutcome::Rejected(ack) => {
            admit_ack(&ack)?;
            let run_id = ack.run_id.ok_or_else(|| {
                PortFailure::Domain(kiln_core::types::Diagnostic::new(
                    "aspen_ingress_missing_run",
                    "aspen.ingress.ack",
                    "service acknowledgement omits its Kiln run identity",
                ))
            })?;
            Ok(TriggerRun {
                run_id,
                terminal: terminal_for(ack.decision),
                log_url: None,
            })
        }
        ServiceTransportOutcome::DisconnectedAfterAcceptance { operation_id } => {
            Err(PortFailure::Unknown(infrastructure(
                "aspen_ingress_unknown",
                &format!(
                    "Aspen accepted operation {operation_id:?}, but acknowledgement was lost; reconcile before retry"
                ),
            )))
        }
        ServiceTransportOutcome::TimedOut => Err(PortFailure::TimedOut(infrastructure(
            "aspen_ingress_timeout",
            "Aspen service ingress exceeded its admitted deadline",
        ))),
        ServiceTransportOutcome::Cancelled => Err(PortFailure::Cancelled(infrastructure(
            "aspen_ingress_cancelled",
            "Aspen service ingress was cancelled",
        ))),
        ServiceTransportOutcome::Unavailable => Err(PortFailure::Infrastructure(infrastructure(
            "aspen_ingress_unavailable",
            "selected Aspen service is unavailable and no fallback is allowed",
        ))),
        ServiceTransportOutcome::Malformed => {
            Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
                "aspen_ingress_malformed",
                "aspen.ingress.ack",
                "Aspen service returned a malformed acknowledgement",
            )))
        }
    }
}

fn admit_ack(ack: &AspenServiceAck) -> PortResult<()> {
    if ack.schema != SERVICE_ACK_SCHEMA || ack.generation == 0 {
        return Err(PortFailure::Domain(kiln_core::types::Diagnostic::new(
            "aspen_ingress_ack_schema",
            "aspen.ingress.ack",
            "Aspen service acknowledgement schema or generation is invalid",
        )));
    }
    ack.operation_id.validate().map_err(PortFailure::Domain)?;
    ack.receipt_ref.validate().map_err(PortFailure::Domain)?;
    Ok(())
}

const fn terminal_for(decision: AspenServiceDecision) -> TriggerTerminal {
    match decision {
        AspenServiceDecision::Accepted
        | AspenServiceDecision::Replay
        | AspenServiceDecision::Unknown => TriggerTerminal::Unknown,
        AspenServiceDecision::Completed => TriggerTerminal::Success,
        AspenServiceDecision::Failed => TriggerTerminal::Failure,
        AspenServiceDecision::Rejected | AspenServiceDecision::Denied => TriggerTerminal::Denied,
        AspenServiceDecision::Cancelled => TriggerTerminal::Cancelled,
        AspenServiceDecision::TimedOut => TriggerTerminal::TimedOut,
    }
}

fn infrastructure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}
