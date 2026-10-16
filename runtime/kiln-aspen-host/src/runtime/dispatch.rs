use kiln_adapter_aspen::protocol::decode_callback_outcome;
use kiln_adapter_aspen::service::build_service_ack;
use kiln_adapter_aspen::transport::{AspenServiceIngress, ServiceTransportOutcome, admit_ingress};
use kiln_core::types::ExternalRef;
use molten::system_extension::{CanonicalCallbackReceipt, HostDispatchResult};

use super::{KilnAspenHostRuntime, disconnected_after_acceptance, terminal_outcome};
use crate::durability::DurableOperationRecord;
use crate::error::{HostBridgeError, Result};

impl KilnAspenHostRuntime {
    pub(super) fn submit_inner(
        &mut self,
        ingress: &AspenServiceIngress,
    ) -> ServiceTransportOutcome {
        if admit_ingress(&self.profile, ingress).is_err() {
            return ServiceTransportOutcome::Malformed;
        }
        let existing = match self.operation(ingress) {
            Ok(existing) => existing,
            Err(error) => {
                eprintln!("Kiln Aspen host operation lookup failed: {error}");
                return ServiceTransportOutcome::Unavailable;
            }
        };
        if let Some(existing) = existing {
            if existing.ingress != *ingress {
                return ServiceTransportOutcome::Malformed;
            }
            if let Some(acknowledgement) = existing.acknowledgement {
                return terminal_outcome(acknowledgement);
            }
            if let Err(error) = self.reconcile_one(&ingress.operation_id) {
                eprintln!("Kiln Aspen host reconciliation failed: {error}");
            }
            return self.current_outcome(&ingress.operation_id);
        }
        if let Err(error) = self.reserve_ingress(ingress) {
            eprintln!("Kiln Aspen host ingress reservation failed: {error}");
            return ServiceTransportOutcome::Malformed;
        }
        if let Err(error) = self.effects.validate_ingress(ingress) {
            eprintln!("Kiln Aspen host reserved-ingress validation failed: {error}");
            return self.retain_unknown(&ingress.operation_id);
        }
        let native_ingress = match self.cohort.native_ingress(ingress) {
            Ok(native_ingress) => native_ingress,
            Err(error) => {
                eprintln!("Kiln Aspen host native-ingress construction failed: {error}");
                return self.retain_unknown(&ingress.operation_id);
            }
        };
        let ingress_tick = match self.next_tick() {
            Ok(tick) => tick,
            Err(error) => {
                eprintln!("Kiln Aspen host ingress tick failed: {error}");
                return self.retain_unknown(&ingress.operation_id);
            }
        };
        let native_result = match self.service.ingress(&native_ingress, ingress_tick) {
            Ok(result) => result,
            Err(error) => {
                eprintln!("Kiln Aspen host native ingress failed: {error:?}");
                return self.retain_unknown(&ingress.operation_id);
            }
        };
        let (callback_receipt, mut callback_outcome, mut output_bytes) =
            match self.decode_dispatch(native_result.dispatch) {
                Ok(result) => result,
                Err(error) => {
                    eprintln!("Kiln Aspen host callback dispatch decoding failed: {error}");
                    return self.retain_unknown(&ingress.operation_id);
                }
            };
        if !callback_receipt.approved_effects.is_empty() {
            let callback_record = self
                .ledger
                .lock()
                .map_err(|_| {
                    HostBridgeError::State("operation ledger lock is unavailable".to_string())
                })
                .and_then(|mut ledger| {
                    ledger.record_callback_receipt(
                        &ingress.operation_id,
                        &callback_receipt.receipt_ref,
                    )
                });
            if callback_record.is_err() {
                return self.retain_unknown(&ingress.operation_id);
            }
            let Some((outcome, bytes)) = self.route_completions(ingress, &callback_receipt) else {
                return self.retain_unknown(&ingress.operation_id);
            };
            callback_outcome = outcome;
            output_bytes = bytes;
        }
        let ack = match build_service_ack(ingress, &callback_outcome, &output_bytes) {
            Ok(ack) => ack,
            Err(error) => {
                eprintln!("Kiln Aspen host acknowledgement construction failed: {error:?}");
                return self.retain_unknown(&ingress.operation_id);
            }
        };
        let persisted = self
            .ledger
            .lock()
            .map_err(|_| HostBridgeError::State("operation ledger lock is unavailable".to_string()))
            .and_then(|mut ledger| {
                ledger.record_acknowledgement(&ingress.operation_id, ack.clone())
            });
        if persisted.is_err() {
            return self.retain_unknown(&ingress.operation_id);
        }
        terminal_outcome(ack)
    }

    fn route_completions(
        &mut self,
        ingress: &AspenServiceIngress,
        callback_receipt: &CanonicalCallbackReceipt,
    ) -> Option<(kiln_adapter_aspen::protocol::CallbackOutcome, Vec<u8>)> {
        let completions = match self
            .service
            .route_effects(callback_receipt, &mut self.effects)
        {
            Ok(completions) if !completions.is_empty() => completions,
            Ok(_) => {
                eprintln!("Kiln Aspen host received no completion for an approved effect");
                return None;
            }
            Err(error) => {
                eprintln!("Kiln Aspen host effect routing failed: {error:?}");
                return None;
            }
        };
        let mut decoded = None;
        for completion in completions {
            let completion_tick = self.next_tick().ok()?;
            let dispatch = self
                .service
                .deliver_effect_completion(&completion, completion_tick)
                .ok()?;
            let (_receipt, outcome, output_bytes) = self.decode_dispatch(dispatch).ok()?;
            decoded = Some((outcome, output_bytes));
        }
        decoded.or_else(|| {
            eprintln!(
                "Kiln Aspen host produced no decoded completion for {}",
                ingress.operation_id.native_id.as_str(),
            );
            None
        })
    }

    pub(super) fn current_outcome(&self, operation_id: &ExternalRef) -> ServiceTransportOutcome {
        match self
            .ledger
            .lock()
            .map_err(|_| HostBridgeError::State("operation ledger lock is unavailable".to_string()))
            .and_then(|ledger| ledger.operation(operation_id))
        {
            Ok(Some(record)) => record.acknowledgement.map_or_else(
                || disconnected_after_acceptance(operation_id),
                terminal_outcome,
            ),
            Ok(None) | Err(_) => ServiceTransportOutcome::Unavailable,
        }
    }

    pub(super) fn retain_unknown(&mut self, operation_id: &ExternalRef) -> ServiceTransportOutcome {
        if let Err(error) = self
            .ledger
            .lock()
            .map_err(|_| HostBridgeError::State("operation ledger lock is unavailable".to_string()))
            .and_then(|mut ledger| ledger.mark_uncertain(operation_id))
        {
            eprintln!("Kiln Aspen host could not retain operation uncertainty: {error}");
        }
        disconnected_after_acceptance(operation_id)
    }

    pub(super) fn decode_dispatch(
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

    fn reserve_ingress(&mut self, ingress: &AspenServiceIngress) -> Result<()> {
        self.ledger
            .lock()
            .map_err(|_| {
                HostBridgeError::State("operation ledger lock is unavailable".to_string())
            })?
            .reserve(ingress)
            .map(|_| ())
    }

    fn operation(&self, ingress: &AspenServiceIngress) -> Result<Option<DurableOperationRecord>> {
        self.ledger
            .lock()
            .map_err(|_| {
                HostBridgeError::State("operation ledger lock is unavailable".to_string())
            })?
            .operation(&ingress.operation_id)
    }
}
