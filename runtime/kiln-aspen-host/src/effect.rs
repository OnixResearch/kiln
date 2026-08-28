use std::collections::BTreeMap;

use kiln_adapter_aspen::effects::admit_effect_plan;
use kiln_adapter_aspen::profile::{AdmittedAspenRuntimeProfile, AspenPortPurpose};
use kiln_adapter_aspen::protocol::{
    CallbackEffectPlan, HOSTED_PROVIDER_COMPLETION_SCHEMA, HostedProviderCompletion,
    encode_hosted_provider_completion,
};
use kiln_adapter_aspen::transport::{AspenServiceIngress, canonical_trigger_identity};
use kiln_adapter_lattice::{LatticeAdapter, UnixWorkflowExchange};
use kiln_adapter_radicle::{RadicleShellConfig, plan_trigger};
use kiln_provider_api::{PortFailure, TriggerRequest, WorkflowPort};
use molten::fabric::{CanonicalFabricPortBinding, FabricPortError, FabricPortResult};
use molten::system_extension::{
    FabricEffectPort, NativeCallbackValue, PortEffectOutput, SharedNativeCallbackValuePort,
    TypedEffectRequest,
};

use crate::error::{HostBridgeError, Result};

const EFFECT_OPERATION: &str = "kiln-provider-effect-v2";

pub struct LatticeEffectPort {
    profile: AdmittedAspenRuntimeProfile,
    radicle: RadicleShellConfig,
    lattice: LatticeAdapter<UnixWorkflowExchange>,
    values: SharedNativeCallbackValuePort,
    triggers: BTreeMap<String, TriggerRequest>,
    maximum_triggers: usize,
    maximum_polls: usize,
}

impl std::fmt::Debug for LatticeEffectPort {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LatticeEffectPort")
            .field(
                "provider_profile",
                &self.profile.profile.workflow_provider_profile,
            )
            .field("trigger_count", &self.triggers.len())
            .field("maximum_triggers", &self.maximum_triggers)
            .field("maximum_polls", &self.maximum_polls)
            .finish_non_exhaustive()
    }
}

impl LatticeEffectPort {
    pub fn new(
        profile: AdmittedAspenRuntimeProfile,
        radicle: RadicleShellConfig,
        lattice: LatticeAdapter<UnixWorkflowExchange>,
        values: SharedNativeCallbackValuePort,
    ) -> Result<Self> {
        if radicle.maximum_replay_records == 0 || radicle.maximum_polls == 0 {
            return Err(HostBridgeError::Admission(
                "Lattice effect adapter requires positive trigger and poll bounds".to_string(),
            ));
        }
        if radicle.workflow.provider_profile_id != profile.profile.workflow_provider_profile {
            return Err(HostBridgeError::Admission(
                "Lattice and Aspen workflow provider profiles differ".to_string(),
            ));
        }
        Ok(Self {
            profile,
            maximum_triggers: radicle.maximum_replay_records,
            maximum_polls: radicle.maximum_polls,
            radicle,
            lattice,
            values,
            triggers: BTreeMap::new(),
        })
    }

    // r[impl kiln.aspen_host.lattice]
    pub fn register_ingress(&mut self, ingress: &AspenServiceIngress) -> Result<()> {
        let observed = canonical_trigger_identity(
            &ingress.trigger,
            self.profile.profile.bounds.max_callback_input_bytes,
        )
        .map_err(|error| HostBridgeError::Protocol(format!("trigger identity: {error:?}")))?;
        if observed != ingress.trigger_identity {
            return Err(HostBridgeError::Protocol(
                "ingress trigger identity differs from its exact bytes".to_string(),
            ));
        }
        let key = observed.as_str().to_string();
        if let Some(existing) = self.triggers.get(&key) {
            if existing != &ingress.trigger {
                return Err(HostBridgeError::State(
                    "trigger identity already names different trigger facts".to_string(),
                ));
            }
            return Ok(());
        }
        if self.triggers.len() >= self.maximum_triggers {
            return Err(HostBridgeError::State(
                "trigger registry reached its admitted bound".to_string(),
            ));
        }
        self.triggers.insert(key, ingress.trigger.clone());
        Ok(())
    }

    fn route_workflow(
        &mut self,
        effect: &CallbackEffectPlan,
    ) -> FabricPortResult<kiln_core::workflow::WorkflowObservation> {
        let trigger = self
            .triggers
            .get(effect.request_identity.as_str())
            .cloned()
            .ok_or_else(|| {
                FabricPortError::malformed("hosted workflow effect has no exact registered trigger")
            })?;
        let mut record = plan_trigger(&self.radicle, trigger).map_err(map_provider_failure)?;
        if record.event_identity != effect.request_identity.as_str()
            || record.plan.idempotency_key != effect.request_identity
            || record.plan.provider_profile != effect.provider_profile
        {
            return Err(FabricPortError::malformed(
                "hosted workflow effect does not match canonical Radicle planning",
            ));
        }
        record.plan.effect_id = effect.effect_id.clone();
        let mut observation = self.lattice.dispatch(&record.plan).map_err(|error| {
            eprintln!(
                "Kiln Aspen host Lattice dispatch failed: {}",
                provider_failure_code(&error),
            );
            map_provider_failure(error)
        })?;
        for _poll in 0..self.maximum_polls {
            if observation.terminal {
                return Ok(observation);
            }
            observation = self
                .lattice
                .observe(&record.plan.effect_id)
                .map_err(|error| {
                    eprintln!(
                        "Kiln Aspen host Lattice observation failed: {}",
                        provider_failure_code(&error),
                    );
                    map_provider_failure(error)
                })?;
        }
        if observation.terminal {
            Ok(observation)
        } else {
            Err(FabricPortError::Timeout {
                message:
                    "Lattice workflow did not reach a terminal observation within its poll bound"
                        .to_string(),
            })
        }
    }

    fn materialize_effect(
        &self,
        effect: &TypedEffectRequest,
    ) -> FabricPortResult<CallbackEffectPlan> {
        let maximum_bytes = u64::try_from(self.profile.profile.bounds.max_materialized_value_bytes)
            .map_err(|_| {
                FabricPortError::malformed("materialized effect byte bound does not fit u64")
            })?;
        let value = self
            .values
            .lock()
            .map_err(|_| FabricPortError::storage("callback value port lock is unavailable"))?
            .materialize(&effect.request_ref, maximum_bytes)
            .map_err(|error| {
                FabricPortError::malformed(format!(
                    "callback effect materialization failed: {}",
                    error.message,
                ))
            })?;
        let plan: CallbackEffectPlan = serde_json::from_slice(&value.bytes)
            .map_err(|_| FabricPortError::malformed("callback effect JSON is invalid"))?;
        admit_effect_plan(&self.profile, &plan).map_err(|error| {
            FabricPortError::malformed(format!("{}: {}", error.code, error.message))
        })?;
        if plan.generation != effect.generation || plan.operation_id.native_id.as_str().is_empty() {
            return Err(FabricPortError::malformed(
                "callback effect plan differs from the routed Aspen effect",
            ));
        }
        Ok(plan)
    }
}

impl FabricEffectPort for LatticeEffectPort {
    // r[impl kiln.aspen_host.lattice]
    fn route(
        &mut self,
        binding: &CanonicalFabricPortBinding,
        effect: &TypedEffectRequest,
    ) -> FabricPortResult<PortEffectOutput> {
        let transport = self
            .profile
            .profile
            .ports
            .iter()
            .find(|candidate| candidate.purpose == AspenPortPurpose::Transport)
            .ok_or_else(|| FabricPortError::capability("Aspen transport port is absent"))?;
        if effect.operation != EFFECT_OPERATION
            || binding.binding.key.port_id != transport.port_id
            || binding.binding.key.version != transport.version
            || effect.input_schema_ref != transport.input_schema
            || effect.output_schema_ref != transport.output_schema
        {
            return Err(FabricPortError::malformed(
                "Aspen workflow effect does not match the exact transport binding",
            ));
        }
        let plan = self.materialize_effect(effect)?;
        let observation = self.route_workflow(&plan)?;
        let completion = HostedProviderCompletion {
            schema: HOSTED_PROVIDER_COMPLETION_SCHEMA.to_string(),
            effect_id: plan.effect_id.clone(),
            operation_id: plan.operation_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            request_identity: plan.request_identity.clone(),
            observation,
        };
        let bytes = encode_hosted_provider_completion(
            &completion,
            self.profile.profile.bounds.max_materialized_value_bytes,
        )
        .map_err(|error| FabricPortError::malformed(error.to_string()))?;
        let output_ref = molten::preserves_rail::content_ref_from_bytes(&bytes);
        Ok(PortEffectOutput {
            output_schema_ref: effect.output_schema_ref.clone(),
            output_ref: output_ref.clone(),
            materialized_output: Some(NativeCallbackValue {
                value_ref: output_ref,
                bytes,
            }),
        })
    }
}

fn provider_failure_code(error: &PortFailure) -> &str {
    match error {
        PortFailure::Domain(diagnostic) => diagnostic.code.as_str(),
        PortFailure::Infrastructure(failure)
        | PortFailure::Retryable(failure)
        | PortFailure::TimedOut(failure)
        | PortFailure::Cancelled(failure)
        | PortFailure::Unknown(failure) => failure.code.as_str(),
    }
}

fn map_provider_failure(error: PortFailure) -> FabricPortError {
    match error {
        PortFailure::Domain(diagnostic) => {
            FabricPortError::malformed(format!("{}: {}", diagnostic.code, diagnostic.message))
        }
        PortFailure::Infrastructure(failure) | PortFailure::Retryable(failure) => {
            FabricPortError::transport(format!("{}: {}", failure.code, failure.message))
        }
        PortFailure::TimedOut(failure) => FabricPortError::Timeout {
            message: format!("{}: {}", failure.code, failure.message),
        },
        PortFailure::Cancelled(failure) => FabricPortError::Cancelled {
            message: format!("{}: {}", failure.code, failure.message),
        },
        PortFailure::Unknown(failure) => {
            FabricPortError::uncertain(format!("{}: {}", failure.code, failure.message))
        }
    }
}
