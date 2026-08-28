use std::sync::{Arc, Mutex};

use kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile;
use kiln_adapter_aspen::transport::{
    AspenServiceAck, AspenServiceDecision, AspenServiceIngress, AspenServiceTransport,
    ServiceTransportOutcome,
};
use kiln_core::types::ExternalRef;

use crate::cohort::{HostCohort, HostService};
use crate::durability::{HostStateMode, OperationLedger};
use crate::effect::LatticeEffectPort;
use crate::error::{HostBridgeError, Result};

mod dispatch;
mod recovery;

pub struct KilnAspenHostRuntime {
    profile: AdmittedAspenRuntimeProfile,
    cohort: HostCohort,
    service: HostService,
    effects: LatticeEffectPort,
    ledger: Arc<Mutex<OperationLedger>>,
}

impl std::fmt::Debug for KilnAspenHostRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KilnAspenHostRuntime")
            .field("profile_identity", &self.profile.profile_identity)
            .field("state_mode", &self.cohort.state_mode)
            .finish_non_exhaustive()
    }
}

impl KilnAspenHostRuntime {
    pub fn start(cohort: HostCohort, effects: LatticeEffectPort) -> Result<Self> {
        let profile = cohort.profile.clone();
        let ledger = cohort.ledger.clone();
        let mut service = cohort.open_service()?;
        match cohort.state_mode {
            HostStateMode::Fresh => {
                let start_tick = next_tick(&ledger)?;
                if let Err(error) = service.start(start_tick) {
                    return Err(HostBridgeError::Admission(format!(
                        "native service start: {error:?}; observations={:?}",
                        service.host().executor().observations(),
                    )));
                }
                let checkpoint_tick = next_tick(&ledger)?;
                service.checkpoint(checkpoint_tick).map_err(|error| {
                    HostBridgeError::State(format!("initial native checkpoint: {error:?}"))
                })?;
            }
            HostStateMode::Recovered => {
                let restart_tick = next_tick(&ledger)?;
                service.restart(restart_tick).map_err(|error| {
                    HostBridgeError::State(format!("restart recovered native service: {error:?}"))
                })?;
            }
        }
        let mut runtime = Self {
            profile,
            cohort,
            service,
            effects,
            ledger,
        };
        runtime.reconcile_all()?;
        Ok(runtime)
    }

    pub fn shutdown(&mut self) -> Result<()> {
        let checkpoint_tick = self.next_tick()?;
        self.service.checkpoint(checkpoint_tick).map_err(|error| {
            HostBridgeError::State(format!("native service checkpoint: {error:?}"))
        })?;
        Ok(())
    }

    pub(super) fn next_tick(&mut self) -> Result<u64> {
        next_tick(&self.ledger)
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
        if let Err(error) = self.reconcile_one(operation_id) {
            eprintln!("Kiln Aspen host reconciliation failed: {error}");
        }
        self.current_outcome(operation_id)
    }
}

fn next_tick(ledger: &Arc<Mutex<OperationLedger>>) -> Result<u64> {
    ledger
        .lock()
        .map_err(|_| HostBridgeError::State("operation ledger lock is unavailable".to_string()))?
        .next_logical_tick()
}

pub(super) fn disconnected_after_acceptance(operation_id: &ExternalRef) -> ServiceTransportOutcome {
    ServiceTransportOutcome::DisconnectedAfterAcceptance {
        operation_id: operation_id.clone(),
    }
}

pub(super) fn terminal_outcome(ack: AspenServiceAck) -> ServiceTransportOutcome {
    if matches!(
        ack.decision,
        AspenServiceDecision::Rejected | AspenServiceDecision::Denied
    ) {
        ServiceTransportOutcome::Rejected(ack)
    } else {
        ServiceTransportOutcome::Acknowledged(ack)
    }
}
