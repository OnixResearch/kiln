use kiln_core::provider::{CapabilityProfile, ProviderObservation, ProviderRole};
use kiln_core::types::{CoreResult, EffectId, ExternalRef};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub effect_id: EffectId,
    pub idempotency_key: ExternalRef,
    pub payload_identity: ExternalRef,
}

pub trait ProviderPort {
    fn profile(&self) -> &CapabilityProfile;
    fn dispatch(&mut self, request: &ProviderRequest) -> CoreResult<ProviderObservation>;
    fn reconcile(&mut self, effect_id: &EffectId) -> CoreResult<ProviderObservation>;
}

pub trait SourcePort: ProviderPort {}
pub trait CollaborationPort: ProviderPort {}
pub trait TriggerPort: ProviderPort {}
pub trait StatusPort: ProviderPort {}
pub trait WorkflowPort: ProviderPort {}
pub trait BuildPort: ProviderPort {}
pub trait EventStorePort: ProviderPort {}
pub trait NotificationPort: ProviderPort {}
pub trait EvidencePort: ProviderPort {}
pub trait GatePort: ProviderPort {}

pub fn role_matches_port(profile: &CapabilityProfile, expected: ProviderRole) -> CoreResult<()> {
    if profile.role != expected {
        return Err(kiln_core::Diagnostic::new(
            "provider_role_mismatch",
            "provider.role",
            "provider profile does not match the selected narrow port",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_remain_distinct() {
        assert_ne!(ProviderRole::Source, ProviderRole::Collaboration);
        assert_ne!(ProviderRole::Workflow, ProviderRole::Build);
    }
}
