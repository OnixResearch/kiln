use kiln_adapter_aspen::protocol::CallbackEffectPlan;
use kiln_adapter_radicle::plan_trigger;
use kiln_core::types::ExternalRef;
use kiln_core::workflow::WorkflowObservation;
use kiln_provider_api::WorkflowPort;
use molten::fabric::{CanonicalFabricPortBinding, FabricPortError, FabricPortResult};
use molten::system_extension::{CanonicalEffectCompletion, TypedEffectRequest};

use super::LatticeEffectPort;
use super::completion::recovered_effect_completion;
use super::port::{map_provider_failure, provider_failure_code};
use crate::durability::{DurableEffectIntent, DurableOperationRecord, DurableOperationState};
use crate::error::{HostBridgeError, Result};

impl LatticeEffectPort {
    pub(super) fn route_workflow(
        &mut self,
        binding: &CanonicalFabricPortBinding,
        effect: &TypedEffectRequest,
        plan: &CallbackEffectPlan,
    ) -> FabricPortResult<WorkflowObservation> {
        let operation = self
            .ledger
            .lock()
            .map_err(|_| FabricPortError::storage("operation ledger lock is unavailable"))?
            .operation_for_effect(plan)
            .map_err(|error| FabricPortError::storage(error.to_string()))?
            .ok_or_else(|| {
                FabricPortError::malformed("hosted workflow effect has no durable ingress")
            })?;
        let trigger = operation.ingress.trigger.clone();
        let host_operation_id = operation.ingress.operation_id.clone();
        let callback_receipt_ref = operation.callback_receipt_ref.ok_or_else(|| {
            FabricPortError::storage("hosted workflow effect has no durable callback receipt")
        })?;
        let intent = DurableEffectIntent {
            callback_receipt_ref,
            binding_ref: binding.binding_ref.clone(),
            port_id: binding.binding.key.port_id.clone(),
            port_version: binding.binding.key.version.clone(),
            request_ref: effect.request_ref.clone(),
            generation: effect.generation,
            output_schema_ref: effect.output_schema_ref.clone(),
            plan: plan.clone(),
        };
        self.ledger
            .lock()
            .map_err(|_| FabricPortError::storage("operation ledger lock is unavailable"))?
            .record_dispatch_intent(&host_operation_id, intent)
            .map_err(|error| {
                eprintln!("Kiln Aspen host dispatch-intent persistence failed: {error}");
                FabricPortError::storage(error.to_string())
            })?;
        let mut record = plan_trigger(&self.radicle, trigger).map_err(map_provider_failure)?;
        if record.event_identity != plan.request_identity.as_str()
            || record.plan.idempotency_key != plan.request_identity
            || record.plan.provider_profile != plan.provider_profile
        {
            return Err(FabricPortError::malformed(
                "hosted workflow effect does not match canonical Radicle planning",
            ));
        }
        record.plan.effect_id = plan.effect_id.clone();
        let mut observation = match self.lattice.dispatch(&record.plan) {
            Ok(observation) => observation,
            Err(error) => {
                self.mark_uncertain(&host_operation_id)?;
                eprintln!(
                    "Kiln Aspen host Lattice dispatch failed: {}",
                    provider_failure_code(&error),
                );
                return Err(map_provider_failure(error));
            }
        };
        for _poll in 0..self.maximum_polls {
            if observation.terminal {
                return self.persist_observation(&host_operation_id, observation);
            }
            observation = match self.lattice.observe(&record.plan.effect_id) {
                Ok(observation) => observation,
                Err(error) => {
                    self.mark_uncertain(&host_operation_id)?;
                    eprintln!(
                        "Kiln Aspen host Lattice observation failed: {}",
                        provider_failure_code(&error),
                    );
                    return Err(map_provider_failure(error));
                }
            };
        }
        if observation.terminal {
            self.persist_observation(&host_operation_id, observation)
        } else {
            self.mark_uncertain(&host_operation_id)?;
            Err(FabricPortError::Timeout {
                message:
                    "Lattice workflow did not reach a terminal observation within its poll bound"
                        .to_string(),
            })
        }
    }

    pub fn reconcile_completion(
        &mut self,
        record: &DurableOperationRecord,
    ) -> Result<Option<CanonicalEffectCompletion>> {
        if record.state == DurableOperationState::Terminal {
            return Ok(None);
        }
        let Some(intent) = record.effect_intent.as_ref() else {
            self.ledger
                .lock()
                .map_err(|_| {
                    HostBridgeError::State("operation ledger lock is unavailable".to_string())
                })?
                .mark_uncertain(&record.ingress.operation_id)?;
            return Ok(None);
        };
        let observation = if let Some(observation) = record.provider_observation.clone() {
            observation
        } else {
            let Some(observation) = self.recover_observation(record, intent)? else {
                return Ok(None);
            };
            observation
        };
        recovered_effect_completion(&self.profile, intent, observation).map(Some)
    }

    fn recover_observation(
        &mut self,
        record: &DurableOperationRecord,
        intent: &DurableEffectIntent,
    ) -> Result<Option<WorkflowObservation>> {
        let mut planned =
            plan_trigger(&self.radicle, record.ingress.trigger.clone()).map_err(|error| {
                HostBridgeError::Provider(provider_failure_code(&error).to_string())
            })?;
        if planned.event_identity != intent.plan.request_identity.as_str()
            || planned.plan.idempotency_key != intent.plan.request_identity
            || planned.plan.provider_profile != intent.plan.provider_profile
        {
            return Err(HostBridgeError::State(
                "recovered effect differs from canonical Radicle planning".to_string(),
            ));
        }
        planned.plan.effect_id = intent.plan.effect_id.clone();
        let mut observation = match self.lattice.recover_dispatch(&planned.plan) {
            Ok(observation) => observation,
            Err(error) => {
                eprintln!(
                    "Kiln Aspen host Lattice recovery failed: {}",
                    provider_failure_code(&error),
                );
                self.retain_recovery_uncertainty(&record.ingress.operation_id)?;
                return Ok(None);
            }
        };
        for _poll in 0..self.maximum_polls {
            if observation.terminal {
                break;
            }
            observation = match self.lattice.observe(&intent.plan.effect_id) {
                Ok(observation) => observation,
                Err(error) => {
                    eprintln!(
                        "Kiln Aspen host Lattice recovery observation failed: {}",
                        provider_failure_code(&error),
                    );
                    self.retain_recovery_uncertainty(&record.ingress.operation_id)?;
                    return Ok(None);
                }
            };
        }
        if !observation.terminal {
            self.retain_recovery_uncertainty(&record.ingress.operation_id)?;
            return Ok(None);
        }
        self.ledger
            .lock()
            .map_err(|_| {
                HostBridgeError::State("operation ledger lock is unavailable".to_string())
            })?
            .record_provider_observation(&record.ingress.operation_id, observation.clone())?;
        Ok(Some(observation))
    }

    fn persist_observation(
        &mut self,
        host_operation_id: &ExternalRef,
        observation: WorkflowObservation,
    ) -> FabricPortResult<WorkflowObservation> {
        if !observation.terminal {
            return Err(FabricPortError::malformed(
                "only terminal provider observations may become completion evidence",
            ));
        }
        self.ledger
            .lock()
            .map_err(|_| FabricPortError::storage("operation ledger lock is unavailable"))?
            .record_provider_observation(host_operation_id, observation.clone())
            .map_err(|error| {
                eprintln!("Kiln Aspen host provider-observation persistence failed: {error}");
                FabricPortError::storage(error.to_string())
            })?;
        Ok(observation)
    }

    fn mark_uncertain(&mut self, operation_id: &ExternalRef) -> FabricPortResult<()> {
        self.ledger
            .lock()
            .map_err(|_| FabricPortError::storage("operation ledger lock is unavailable"))?
            .mark_uncertain(operation_id)
            .map(|_| ())
            .map_err(|error| FabricPortError::storage(error.to_string()))
    }

    fn retain_recovery_uncertainty(&mut self, operation_id: &ExternalRef) -> Result<()> {
        self.ledger
            .lock()
            .map_err(|_| {
                HostBridgeError::State("operation ledger lock is unavailable".to_string())
            })?
            .mark_uncertain(operation_id)
            .map(|_| ())
    }
}
