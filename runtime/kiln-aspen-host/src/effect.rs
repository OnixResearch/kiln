use std::sync::{Arc, Mutex};

use kiln_adapter_aspen::effects::admit_effect_plan;
use kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile;
use kiln_adapter_aspen::protocol::CallbackEffectPlan;
use kiln_adapter_aspen::transport::{AspenServiceIngress, canonical_trigger_identity};
use kiln_adapter_lattice::{LatticeAdapter, UnixWorkflowExchange};
use kiln_adapter_radicle::RadicleShellConfig;
use molten::fabric::{FabricPortError, FabricPortResult};
use molten::system_extension::{SharedNativeCallbackValuePort, TypedEffectRequest};

use crate::durability::OperationLedger;
use crate::error::{HostBridgeError, Result};

mod completion;
mod port;
mod workflow;

pub struct LatticeEffectPort {
    profile: AdmittedAspenRuntimeProfile,
    radicle: RadicleShellConfig,
    lattice: LatticeAdapter<UnixWorkflowExchange>,
    values: SharedNativeCallbackValuePort,
    ledger: Arc<Mutex<OperationLedger>>,
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
        ledger: Arc<Mutex<OperationLedger>>,
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
            maximum_polls: radicle.maximum_polls,
            radicle,
            lattice,
            values,
            ledger,
        })
    }

    // r[impl kiln.aspen_host.lattice]
    pub fn validate_ingress(&self, ingress: &AspenServiceIngress) -> Result<()> {
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
        let ledger = self.ledger.lock().map_err(|_| {
            HostBridgeError::State("operation ledger lock is unavailable".to_string())
        })?;
        let existing = ledger.operation(&ingress.operation_id)?;
        if existing.as_ref().map(|record| &record.ingress) != Some(ingress) {
            return Err(HostBridgeError::State(
                "durable ingress differs from the routed trigger".to_string(),
            ));
        }
        Ok(())
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
