use kiln_adapter_aspen::protocol::CallbackEffectPlan;
use kiln_adapter_aspen::transport::{AspenServiceAck, AspenServiceIngress};
use kiln_core::workflow::WorkflowObservation;
use serde::{Deserialize, Serialize};

pub const DURABLE_OPERATION_SCHEMA: &str = "kiln.aspen-host.operation.v1";
pub const DURABLE_HOST_STATE_SCHEMA: &str = "kiln.aspen-host.state.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurableOperationState {
    Reserved,
    Dispatching,
    Uncertain,
    Terminal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurableEffectIntent {
    pub callback_receipt_ref: String,
    pub binding_ref: String,
    pub port_id: String,
    pub port_version: String,
    pub request_ref: String,
    pub generation: u64,
    pub output_schema_ref: String,
    pub plan: CallbackEffectPlan,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurableOperationRecord {
    pub schema: String,
    pub ingress: AspenServiceIngress,
    pub ingress_blake3: String,
    pub state: DurableOperationState,
    pub callback_receipt_ref: Option<String>,
    pub effect_intent: Option<DurableEffectIntent>,
    pub provider_observation: Option<WorkflowObservation>,
    pub acknowledgement: Option<AspenServiceAck>,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurableHostStateMarker {
    pub schema: String,
    pub cohort_identity: String,
    pub operation_schema: String,
    pub native_journal_schema: String,
    pub callback_value_schema: String,
}

impl DurableOperationRecord {
    pub fn reserve(ingress: AspenServiceIngress) -> Result<Self, String> {
        ingress
            .operation_id
            .validate()
            .map_err(|error| error.message)?;
        let ingress_bytes = serde_json::to_vec(&ingress)
            .map_err(|_| "durable ingress encoding failed".to_string())?;
        let record = Self {
            schema: DURABLE_OPERATION_SCHEMA.to_string(),
            ingress,
            ingress_blake3: blake3::hash(&ingress_bytes).to_hex().to_string(),
            state: DurableOperationState::Reserved,
            callback_receipt_ref: None,
            effect_intent: None,
            provider_observation: None,
            acknowledgement: None,
            revision: 1,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn with_callback_receipt(&self, callback_receipt_ref: &str) -> Result<Self, String> {
        if self.state == DurableOperationState::Terminal || callback_receipt_ref.is_empty() {
            return Err(
                "callback receipt cannot change a terminal or malformed operation".to_string(),
            );
        }
        if let Some(existing) = &self.callback_receipt_ref {
            if existing == callback_receipt_ref {
                return Ok(self.clone());
            }
            return Err("callback receipt conflicts with the durable operation".to_string());
        }
        let mut next = self.clone();
        next.callback_receipt_ref = Some(callback_receipt_ref.to_string());
        next.bump_revision()?;
        next.validate()?;
        Ok(next)
    }

    pub fn with_dispatch_intent(&self, intent: DurableEffectIntent) -> Result<Self, String> {
        if let Some(existing) = &self.effect_intent {
            if existing == &intent {
                return Ok(self.clone());
            }
            return Err("effect intent conflicts with the durable operation".to_string());
        }
        if self.state != DurableOperationState::Reserved
            || self.callback_receipt_ref.as_deref() != Some(intent.callback_receipt_ref.as_str())
        {
            return Err("effect intent requires the exact reserved callback context".to_string());
        }
        let mut next = self.clone();
        next.state = DurableOperationState::Dispatching;
        next.effect_intent = Some(intent);
        next.bump_revision()?;
        next.validate()?;
        Ok(next)
    }

    pub fn with_provider_observation(
        &self,
        observation: WorkflowObservation,
    ) -> Result<Self, String> {
        let Some(intent) = &self.effect_intent else {
            return Err("provider observation has no durable effect intent".to_string());
        };
        if observation.effect_id != intent.plan.effect_id || !observation.terminal {
            return Err("provider observation is not the exact terminal effect".to_string());
        }
        if let Some(existing) = &self.provider_observation {
            if existing == &observation {
                return Ok(self.clone());
            }
            return Err("provider observation conflicts with durable evidence".to_string());
        }
        if self.state == DurableOperationState::Terminal {
            return Err("provider observation cannot change a terminal operation".to_string());
        }
        let mut next = self.clone();
        next.provider_observation = Some(observation);
        next.bump_revision()?;
        next.validate()?;
        Ok(next)
    }

    pub fn with_uncertainty(&self) -> Result<Self, String> {
        if self.state == DurableOperationState::Terminal {
            return Ok(self.clone());
        }
        if self.state == DurableOperationState::Uncertain {
            return Ok(self.clone());
        }
        let mut next = self.clone();
        next.state = DurableOperationState::Uncertain;
        next.bump_revision()?;
        next.validate()?;
        Ok(next)
    }

    pub fn with_acknowledgement(&self, acknowledgement: AspenServiceAck) -> Result<Self, String> {
        if acknowledgement.operation_id != self.ingress.operation_id
            || acknowledgement.generation != self.ingress.generation
            || acknowledgement.trigger_identity != self.ingress.trigger_identity
        {
            return Err("service acknowledgement differs from its durable ingress".to_string());
        }
        if let Some(existing) = &self.acknowledgement {
            if existing == &acknowledgement {
                return Ok(self.clone());
            }
            return Err("terminal acknowledgement conflicts with durable evidence".to_string());
        }
        let mut next = self.clone();
        next.state = DurableOperationState::Terminal;
        next.acknowledgement = Some(acknowledgement);
        next.bump_revision()?;
        next.validate()?;
        Ok(next)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.ingress
            .operation_id
            .validate()
            .map_err(|error| error.message)?;
        let ingress_bytes = serde_json::to_vec(&self.ingress)
            .map_err(|_| "durable ingress encoding failed".to_string())?;
        let observed_ingress_blake3 = blake3::hash(&ingress_bytes).to_hex().to_string();
        if self.schema != DURABLE_OPERATION_SCHEMA
            || self.revision == 0
            || self.ingress_blake3 != observed_ingress_blake3
        {
            return Err(
                "durable operation schema, revision, or ingress identity is invalid".to_string(),
            );
        }
        if let Some(intent) = &self.effect_intent
            && (self.callback_receipt_ref.as_deref() != Some(intent.callback_receipt_ref.as_str())
                || intent.plan.operation_id.validate().is_err()
                || intent.plan.idempotency_key != self.ingress.idempotency_key
                || intent.plan.generation != self.ingress.generation
                || intent.plan.request_identity != self.ingress.trigger_identity
                || intent.request_ref.is_empty()
                || intent.binding_ref.is_empty()
                || intent.port_id.is_empty()
                || intent.port_version.is_empty()
                || intent.output_schema_ref.is_empty())
        {
            return Err("durable effect intent does not match its ingress".to_string());
        }
        if let Some(observation) = &self.provider_observation {
            let Some(intent) = &self.effect_intent else {
                return Err("provider observation has no effect intent".to_string());
            };
            if observation.effect_id != intent.plan.effect_id || !observation.terminal {
                return Err(
                    "provider observation effect identity or terminal state is invalid".to_string(),
                );
            }
        }
        match self.state {
            DurableOperationState::Reserved => {
                if self.effect_intent.is_some()
                    || self.provider_observation.is_some()
                    || self.acknowledgement.is_some()
                {
                    return Err("reserved operation contains post-reservation evidence".to_string());
                }
            }
            DurableOperationState::Dispatching => {
                if self.effect_intent.is_none() || self.acknowledgement.is_some() {
                    return Err(
                        "dispatching operation lacks intent or contains an acknowledgement"
                            .to_string(),
                    );
                }
            }
            DurableOperationState::Uncertain => {
                if self.acknowledgement.is_some() {
                    return Err(
                        "uncertain operation contains a terminal acknowledgement".to_string()
                    );
                }
            }
            DurableOperationState::Terminal => {
                if self.acknowledgement.is_none() {
                    return Err("terminal operation lacks its acknowledgement".to_string());
                }
            }
        }
        Ok(())
    }

    fn bump_revision(&mut self) -> Result<(), String> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| "durable operation revision overflow".to_string())?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
