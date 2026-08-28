use kiln_adapter_aspen::service::build_service_ack;
use kiln_core::types::ExternalRef;

use super::KilnAspenHostRuntime;
use crate::error::{HostBridgeError, Result};

impl KilnAspenHostRuntime {
    pub(super) fn reconcile_all(&mut self) -> Result<()> {
        let records = self
            .ledger
            .lock()
            .map_err(|_| {
                HostBridgeError::State("operation ledger lock is unavailable".to_string())
            })?
            .load_all()?;
        for record in records {
            if record.acknowledgement.is_none() {
                self.reconcile_one(&record.ingress.operation_id)?;
            }
        }
        Ok(())
    }

    pub(super) fn reconcile_one(&mut self, operation_id: &ExternalRef) -> Result<()> {
        let record = self
            .ledger
            .lock()
            .map_err(|_| {
                HostBridgeError::State("operation ledger lock is unavailable".to_string())
            })?
            .operation(operation_id)?
            .ok_or_else(|| {
                HostBridgeError::State("durable operation does not exist".to_string())
            })?;
        if record.acknowledgement.is_some() {
            return Ok(());
        }
        let Some(completion) = self.effects.reconcile_completion(&record)? else {
            return Ok(());
        };
        let completion_tick = self.next_tick()?;
        let dispatch = self
            .service
            .deliver_effect_completion(&completion, completion_tick)
            .map_err(|error| {
                HostBridgeError::State(format!("deliver recovered effect completion: {error:?}"))
            })?;
        let (_receipt, outcome, output_bytes) = self.decode_dispatch(dispatch)?;
        let acknowledgement = build_service_ack(&record.ingress, &outcome, &output_bytes)
            .map_err(|error| HostBridgeError::Protocol(error.to_string()))?;
        self.ledger
            .lock()
            .map_err(|_| {
                HostBridgeError::State("operation ledger lock is unavailable".to_string())
            })?
            .record_acknowledgement(operation_id, acknowledgement)?;
        Ok(())
    }
}
