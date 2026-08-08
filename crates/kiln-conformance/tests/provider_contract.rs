use std::collections::BTreeSet;

use kiln_core::domain::Outcome;
use kiln_core::provider::{
    CapabilityProfile, Operation, OperationRequest, ProviderLimits, ProviderRole, admit_operation,
};
use kiln_core::types::{CONTRACT_VERSION_V1, ExternalRef, ProviderProfileId};

const MAX_REQUEST_BYTES: usize = 1024;
const MAX_RESULT_BYTES: usize = 1024;
const MAX_IN_FLIGHT: usize = 1;

fn profile() -> CapabilityProfile {
    CapabilityProfile {
        contract_version: CONTRACT_VERSION_V1,
        profile_id: ProviderProfileId::new("fixture-build").unwrap_or_else(|_| unreachable!()),
        role: ProviderRole::Build,
        namespace: ExternalRef::new("fixture", "provider", "build")
            .unwrap_or_else(|_| unreachable!()),
        operations: BTreeSet::from([Operation::Dispatch, Operation::Cancel, Operation::Reconcile]),
        outcomes: BTreeSet::from([
            Outcome::Success,
            Outcome::DomainFailure,
            Outcome::Cancelled,
            Outcome::TimedOut,
            Outcome::ProviderError,
            Outcome::Unknown,
        ]),
        limits: ProviderLimits {
            max_request_bytes: MAX_REQUEST_BYTES,
            max_result_bytes: MAX_RESULT_BYTES,
            max_in_flight: MAX_IN_FLIGHT,
        },
    }
}

#[test]
fn declared_lifecycle_is_admitted() {
    let request = OperationRequest {
        contract_version: CONTRACT_VERSION_V1,
        selected_profile: ProviderProfileId::new("fixture-build")
            .unwrap_or_else(|_| unreachable!()),
        role: ProviderRole::Build,
        operation: Operation::Dispatch,
        request_bytes: MAX_REQUEST_BYTES,
        required_outcomes: BTreeSet::from([Outcome::Unknown]),
    };
    assert_eq!(admit_operation(&profile(), &request), Ok(()));
}

#[test]
fn undeclared_pause_fails_closed() {
    let request = OperationRequest {
        contract_version: CONTRACT_VERSION_V1,
        selected_profile: ProviderProfileId::new("fixture-build")
            .unwrap_or_else(|_| unreachable!()),
        role: ProviderRole::Build,
        operation: Operation::Pause,
        request_bytes: MAX_REQUEST_BYTES,
        required_outcomes: BTreeSet::new(),
    };
    assert!(admit_operation(&profile(), &request).is_err());
}
