use kiln_core::domain::Outcome;
use kiln_core::provider::{CapabilityProfile, ProviderObservation, ProviderRole};
use kiln_core::types::{ContentId, CoreResult, Diagnostic, EffectId, ExternalRef, RunId};
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationRequest {
    pub effect_id: EffectId,
    pub run_id: RunId,
    pub outcome: Outcome,
    pub notification_policy: ExternalRef,
    pub endpoint_profile: ExternalRef,
    pub body_identity: ContentId,
    pub idempotency_key: ExternalRef,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NotificationOutcome {
    Accepted,
    Rejected,
    RateLimited,
    TimedOut,
    ProviderError,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationObservation {
    pub effect_id: EffectId,
    pub endpoint_profile: ExternalRef,
    pub body_identity: ContentId,
    pub outcome: NotificationOutcome,
    pub external_reference: Option<ExternalRef>,
    pub retryable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InfrastructureFailure {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortFailure {
    Domain(Diagnostic),
    Infrastructure(InfrastructureFailure),
}

pub trait NotificationPort {
    fn deliver(
        &mut self,
        request: &NotificationRequest,
    ) -> Result<NotificationObservation, PortFailure>;

    fn reconcile(&mut self, effect_id: &EffectId) -> Result<NotificationObservation, PortFailure>;
}

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
