use std::collections::{BTreeMap, BTreeSet};

use kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile;
use kiln_adapter_aspen::protocol::decode_callback_outcome;
use kiln_adapter_aspen::service::build_service_ack;
use kiln_adapter_aspen::transport::{
    AspenServiceAck, AspenServiceDecision, AspenServiceIngress, AspenServiceTransport,
    ServiceTransportOutcome, admit_ingress,
};
use kiln_core::types::ExternalRef;
use molten::system_extension::{CanonicalCallbackReceipt, HostDispatchResult};

use crate::cohort::{HostCohort, HostService};
use crate::effect::LatticeEffectPort;
use crate::error::{HostBridgeError, Result};

const START_TICK: u64 = 1;
const TICK_INCREMENT: u64 = 1;

pub struct KilnAspenHostRuntime {
    profile: AdmittedAspenRuntimeProfile,
    cohort: HostCohort,
    service: HostService,
    effects: LatticeEffectPort,
    acknowledgements: BTreeMap<ExternalRef, AspenServiceAck>,
    uncertain_operations: BTreeSet<ExternalRef>,
    logical_tick: u64,
}

impl std::fmt::Debug for KilnAspenHostRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KilnAspenHostRuntime")
            .field("profile_identity", &self.profile.profile_identity)
            .field("acknowledgement_count", &self.acknowledgements.len())
            .field(
                "uncertain_operation_count",
                &self.uncertain_operations.len(),
            )
            .field("logical_tick", &self.logical_tick)
            .finish_non_exhaustive()
    }
}

impl KilnAspenHostRuntime {
    pub fn start(cohort: HostCohort, effects: LatticeEffectPort) -> Result<Self> {
        let profile = cohort.profile.clone();
        let mut service = cohort.install()?;
        if let Err(error) = service.start(START_TICK) {
            return Err(HostBridgeError::Admission(format!(
                "native service start: {error:?}; observations={:?}",
                service.host().executor().observations(),
            )));
        }
        Ok(Self {
            profile,
            cohort,
            service,
            effects,
            acknowledgements: BTreeMap::new(),
            uncertain_operations: BTreeSet::new(),
            logical_tick: START_TICK,
        })
    }

    pub fn shutdown(&mut self) -> Result<()> {
        let drain_tick = self.next_tick()?;
        self.service
            .drain(drain_tick)
            .map_err(|error| HostBridgeError::State(format!("native service drain: {error:?}")))?;
        let stop_tick = self.next_tick()?;
        self.service
            .stop(stop_tick)
            .map_err(|error| HostBridgeError::State(format!("native service stop: {error:?}")))?;
        Ok(())
    }

    fn submit_inner(&mut self, ingress: &AspenServiceIngress) -> ServiceTransportOutcome {
        if admit_ingress(&self.profile, ingress).is_err() {
            return ServiceTransportOutcome::Malformed;
        }
        if let Some(existing) = self.acknowledgements.get(&ingress.operation_id) {
            return terminal_outcome(existing.clone());
        }
        if self.uncertain_operations.contains(&ingress.operation_id) {
            return disconnected_after_acceptance(&ingress.operation_id);
        }
        if self.effects.register_ingress(ingress).is_err() {
            return ServiceTransportOutcome::Malformed;
        }
        let native_ingress = match self.cohort.native_ingress(ingress) {
            Ok(native_ingress) => native_ingress,
            Err(_) => return ServiceTransportOutcome::Malformed,
        };
        let ingress_tick = match self.next_tick() {
            Ok(tick) => tick,
            Err(_) => return ServiceTransportOutcome::Unavailable,
        };
        let native_result = match self.service.ingress(&native_ingress, ingress_tick) {
            Ok(result) => result,
            Err(_) => return self.retain_unknown(&ingress.operation_id),
        };
        let (callback_receipt, mut callback_outcome, mut output_bytes) =
            match self.decode_dispatch(native_result.dispatch) {
                Ok(result) => result,
                Err(_) => return ServiceTransportOutcome::Malformed,
            };
        if !callback_receipt.approved_effects.is_empty() {
            let completions = match self
                .service
                .route_effects(&callback_receipt, &mut self.effects)
            {
                Ok(completions) if !completions.is_empty() => completions,
                Ok(_) => {
                    eprintln!("Kiln Aspen host received no completion for an approved effect");
                    return self.retain_unknown(&ingress.operation_id);
                }
                Err(error) => {
                    eprintln!("Kiln Aspen host effect routing failed: {error:?}");
                    return self.retain_unknown(&ingress.operation_id);
                }
            };
            for completion in completions {
                let completion_tick = match self.next_tick() {
                    Ok(tick) => tick,
                    Err(_) => return ServiceTransportOutcome::Unavailable,
                };
                let dispatch = match self
                    .service
                    .deliver_effect_completion(&completion, completion_tick)
                {
                    Ok(dispatch) => dispatch,
                    Err(_) => return self.retain_unknown(&ingress.operation_id),
                };
                let (_receipt, next_outcome, next_output_bytes) =
                    match self.decode_dispatch(dispatch) {
                        Ok(result) => result,
                        Err(_) => return ServiceTransportOutcome::Malformed,
                    };
                callback_outcome = next_outcome;
                output_bytes = next_output_bytes;
            }
        }
        let ack = match build_service_ack(ingress, &callback_outcome, &output_bytes) {
            Ok(ack) => ack,
            Err(_) => return ServiceTransportOutcome::Malformed,
        };
        self.acknowledgements
            .insert(ingress.operation_id.clone(), ack.clone());
        terminal_outcome(ack)
    }

    fn retain_unknown(&mut self, operation_id: &ExternalRef) -> ServiceTransportOutcome {
        let maximum_operations = self.profile.profile.bounds.max_records;
        if self.uncertain_operations.len() < maximum_operations
            || self.uncertain_operations.contains(operation_id)
        {
            self.uncertain_operations.insert(operation_id.clone());
        } else {
            eprintln!("Kiln Aspen host reached its uncertain-operation retention bound");
        }
        disconnected_after_acceptance(operation_id)
    }

    fn decode_dispatch(
        &self,
        dispatch: HostDispatchResult,
    ) -> Result<(
        CanonicalCallbackReceipt,
        kiln_adapter_aspen::protocol::CallbackOutcome,
        Vec<u8>,
    )> {
        let (receipt, outcome) = match dispatch {
            HostDispatchResult::Executed {
                receipt, outcome, ..
            } => (receipt, outcome),
            HostDispatchResult::Deferred { .. } | HostDispatchResult::Failed { .. } => {
                return Err(HostBridgeError::Protocol(
                    "Aspen callback did not produce an executed outcome".to_string(),
                ));
            }
        };
        let output_ref = outcome.output_refs.first().ok_or_else(|| {
            HostBridgeError::Protocol("Aspen callback omitted the Kiln output value".to_string())
        })?;
        let maximum_bytes = u64::try_from(self.profile.profile.bounds.max_materialized_value_bytes)
            .map_err(|_| {
                HostBridgeError::Protocol("callback output bound does not fit u64".to_string())
            })?;
        let value = self
            .cohort
            .values
            .lock()
            .map_err(|_| {
                HostBridgeError::State("callback value port lock is unavailable".to_string())
            })?
            .materialize(output_ref, maximum_bytes)
            .map_err(|error| HostBridgeError::Protocol(error.message))?;
        let decoded = decode_callback_outcome(
            &value.bytes,
            self.profile.profile.bounds.max_materialized_value_bytes,
            self.profile.profile.bounds.max_effects,
        )
        .map_err(|error| HostBridgeError::Protocol(error.to_string()))?;
        Ok((receipt, decoded, value.bytes))
    }

    fn next_tick(&mut self) -> Result<u64> {
        self.logical_tick = self
            .logical_tick
            .checked_add(TICK_INCREMENT)
            .ok_or_else(|| HostBridgeError::State("host logical tick overflow".to_string()))?;
        Ok(self.logical_tick)
    }
}

impl AspenServiceTransport for KilnAspenHostRuntime {
    // r[impl kiln.aspen_host.transport]
    fn submit(
        &mut self,
        profile: &AdmittedAspenRuntimeProfile,
        request: &AspenServiceIngress,
    ) -> ServiceTransportOutcome {
        if profile.profile_identity != self.profile.profile_identity {
            ServiceTransportOutcome::Unavailable
        } else {
            self.submit_inner(request)
        }
    }

    fn reconcile(
        &mut self,
        profile: &AdmittedAspenRuntimeProfile,
        operation_id: &ExternalRef,
    ) -> ServiceTransportOutcome {
        if profile.profile_identity != self.profile.profile_identity {
            return ServiceTransportOutcome::Unavailable;
        }
        if let Some(acknowledgement) = self.acknowledgements.get(operation_id).cloned() {
            return terminal_outcome(acknowledgement);
        }
        if self.uncertain_operations.contains(operation_id) {
            disconnected_after_acceptance(operation_id)
        } else {
            ServiceTransportOutcome::Unavailable
        }
    }
}

fn disconnected_after_acceptance(operation_id: &ExternalRef) -> ServiceTransportOutcome {
    ServiceTransportOutcome::DisconnectedAfterAcceptance {
        operation_id: operation_id.clone(),
    }
}

fn terminal_outcome(ack: AspenServiceAck) -> ServiceTransportOutcome {
    if matches!(
        ack.decision,
        AspenServiceDecision::Rejected | AspenServiceDecision::Denied
    ) {
        ServiceTransportOutcome::Rejected(ack)
    } else {
        ServiceTransportOutcome::Acknowledged(ack)
    }
}
