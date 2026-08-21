use alloc::collections::BTreeSet;
use serde::{Deserialize, Serialize};

use crate::domain::Outcome;
use crate::types::{CONTRACT_VERSION_V1, CoreResult, Diagnostic, ExternalRef, ProviderProfileId};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ProviderRole {
    Source,
    Collaboration,
    Trigger,
    Status,
    Discovery,
    Workflow,
    Build,
    EventStore,
    Notification,
    Evidence,
    Gate,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Operation {
    Discover,
    Resolve,
    Acquire,
    Mutate,
    Dispatch,
    Observe,
    PublishStatus,
    Cancel,
    Pause,
    Resume,
    Reconcile,
    ReadLogs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProviderFailureClass {
    Domain,
    Infrastructure,
    RetryableInfrastructure,
    TimedOut,
    Cancelled,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProviderFailureDisposition {
    RejectDomain,
    ProviderFailure,
    RetryInfrastructure,
    TimedOut,
    Cancelled,
    ReconcileUnknown,
}

// r[impl kiln.providers.error_ownership]
#[must_use]
pub const fn classify_provider_failure(
    failure: ProviderFailureClass,
) -> ProviderFailureDisposition {
    match failure {
        ProviderFailureClass::Domain => ProviderFailureDisposition::RejectDomain,
        ProviderFailureClass::Infrastructure => ProviderFailureDisposition::ProviderFailure,
        ProviderFailureClass::RetryableInfrastructure => {
            ProviderFailureDisposition::RetryInfrastructure
        }
        ProviderFailureClass::TimedOut => ProviderFailureDisposition::TimedOut,
        ProviderFailureClass::Cancelled => ProviderFailureDisposition::Cancelled,
        ProviderFailureClass::Unknown => ProviderFailureDisposition::ReconcileUnknown,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderLimits {
    pub max_request_bytes: usize,
    pub max_result_bytes: usize,
    pub max_in_flight: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityProfile {
    pub contract_version: u16,
    pub profile_id: ProviderProfileId,
    pub role: ProviderRole,
    pub namespace: ExternalRef,
    pub operations: BTreeSet<Operation>,
    pub outcomes: BTreeSet<Outcome>,
    pub limits: ProviderLimits,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OperationRequest {
    pub contract_version: u16,
    pub selected_profile: ProviderProfileId,
    pub role: ProviderRole,
    pub operation: Operation,
    pub request_bytes: usize,
    pub required_outcomes: BTreeSet<Outcome>,
}

pub fn admit_operation(profile: &CapabilityProfile, request: &OperationRequest) -> CoreResult<()> {
    if profile.contract_version != CONTRACT_VERSION_V1
        || request.contract_version != CONTRACT_VERSION_V1
    {
        return Err(Diagnostic::new(
            "unsupported_version",
            "provider.contract_version",
            "provider contract version is unsupported",
        ));
    }
    profile.namespace.validate()?;
    if profile.profile_id != request.selected_profile {
        return Err(Diagnostic::new(
            "provider_substitution",
            "provider.profile_id",
            "selected provider profile does not match",
        ));
    }
    if profile.role != request.role {
        return Err(Diagnostic::new(
            "role_mismatch",
            "provider.role",
            "provider role does not match the requested port",
        ));
    }
    if !profile.operations.contains(&request.operation) {
        return Err(Diagnostic::new(
            "unsupported_capability",
            "provider.operation",
            "provider does not declare the requested operation",
        ));
    }
    if request.request_bytes > profile.limits.max_request_bytes {
        return Err(Diagnostic::new(
            "provider_limit_exceeded",
            "provider.request_bytes",
            "request exceeds the provider profile bound",
        ));
    }
    if !request.required_outcomes.is_subset(&profile.outcomes) {
        return Err(Diagnostic::new(
            "outcome_support_missing",
            "provider.outcomes",
            "provider does not preserve all required outcome classes",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderObservation {
    pub contract_version: u16,
    pub profile_id: ProviderProfileId,
    pub outcome: Outcome,
    pub external_reference: Option<ExternalRef>,
}

pub fn admit_observation(
    profile: &CapabilityProfile,
    observation: &ProviderObservation,
) -> CoreResult<()> {
    if observation.contract_version != CONTRACT_VERSION_V1 {
        return Err(Diagnostic::new(
            "unsupported_observation_version",
            "provider_observation.contract_version",
            "provider observation version is unsupported",
        ));
    }
    if observation.profile_id != profile.profile_id {
        return Err(Diagnostic::new(
            "provider_observation_substitution",
            "provider_observation.profile_id",
            "provider observation came from another profile",
        ));
    }
    if !profile.outcomes.contains(&observation.outcome) {
        return Err(Diagnostic::new(
            "false_provider_outcome",
            "provider_observation.outcome",
            "provider returned an outcome that its profile does not declare",
        ));
    }
    if let Some(reference) = &observation.external_reference {
        reference.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeSet;

    use super::*;

    const MAX_REQUEST_BYTES: usize = 4096;
    const MAX_RESULT_BYTES: usize = 4096;
    const MAX_IN_FLIGHT: usize = 4;

    fn profile() -> CapabilityProfile {
        let mut operations = BTreeSet::new();
        operations.insert(Operation::Dispatch);
        let mut outcomes = BTreeSet::new();
        outcomes.insert(Outcome::Success);
        outcomes.insert(Outcome::Unknown);
        CapabilityProfile {
            contract_version: CONTRACT_VERSION_V1,
            profile_id: ProviderProfileId::new("build-a").unwrap_or_else(|_| unreachable!()),
            role: ProviderRole::Build,
            namespace: ExternalRef::new("build-a", "provider", "instance-a")
                .unwrap_or_else(|_| unreachable!()),
            operations,
            outcomes,
            limits: ProviderLimits {
                max_request_bytes: MAX_REQUEST_BYTES,
                max_result_bytes: MAX_RESULT_BYTES,
                max_in_flight: MAX_IN_FLIGHT,
            },
        }
    }

    #[test]
    fn admits_declared_operation() {
        let mut required_outcomes = BTreeSet::new();
        required_outcomes.insert(Outcome::Unknown);
        let request = OperationRequest {
            contract_version: CONTRACT_VERSION_V1,
            selected_profile: ProviderProfileId::new("build-a").unwrap_or_else(|_| unreachable!()),
            role: ProviderRole::Build,
            operation: Operation::Dispatch,
            request_bytes: MAX_REQUEST_BYTES,
            required_outcomes,
        };
        assert_eq!(admit_operation(&profile(), &request), Ok(()));
    }

    #[test]
    fn rejects_observation_from_substituted_profile() {
        let observation = ProviderObservation {
            contract_version: CONTRACT_VERSION_V1,
            profile_id: ProviderProfileId::new("build-b").unwrap_or_else(|_| unreachable!()),
            outcome: Outcome::Success,
            external_reference: None,
        };
        assert!(admit_observation(&profile(), &observation).is_err());
    }

    // r[verify kiln.providers.error_ownership]
    #[test]
    fn failure_translation_preserves_each_class() {
        assert_eq!(
            classify_provider_failure(ProviderFailureClass::Domain),
            ProviderFailureDisposition::RejectDomain
        );
        assert_eq!(
            classify_provider_failure(ProviderFailureClass::Infrastructure),
            ProviderFailureDisposition::ProviderFailure
        );
        assert_eq!(
            classify_provider_failure(ProviderFailureClass::RetryableInfrastructure),
            ProviderFailureDisposition::RetryInfrastructure
        );
        assert_eq!(
            classify_provider_failure(ProviderFailureClass::TimedOut),
            ProviderFailureDisposition::TimedOut
        );
        assert_eq!(
            classify_provider_failure(ProviderFailureClass::Cancelled),
            ProviderFailureDisposition::Cancelled
        );
        assert_eq!(
            classify_provider_failure(ProviderFailureClass::Unknown),
            ProviderFailureDisposition::ReconcileUnknown
        );
    }

    #[test]
    fn rejects_hidden_fallback() {
        let request = OperationRequest {
            contract_version: CONTRACT_VERSION_V1,
            selected_profile: ProviderProfileId::new("build-b").unwrap_or_else(|_| unreachable!()),
            role: ProviderRole::Build,
            operation: Operation::Dispatch,
            request_bytes: MAX_REQUEST_BYTES,
            required_outcomes: BTreeSet::new(),
        };
        assert!(admit_operation(&profile(), &request).is_err());
    }
}
