#![allow(
    unknown_lints,
    clippy::expect_used,
    assertion_density,
    non_trait_imports,
    no_unwrap,
    reason = "checked-in producer fixtures cannot continue when their reviewed schema fails to decode"
)]

use lattice_workflow_transport::*;
use serde::Deserialize;

const PRODUCER_FIXTURES: &str = include_str!("../fixtures/producer-fixtures-v1.json");

#[derive(Deserialize)]
struct TransportFixtures {
    profile: TransportProfile,
    request: ExchangeRequest,
    response: ExchangeResponse,
    rejected: RejectedFixtures,
}

#[derive(Deserialize)]
struct RejectedFixtures {
    changed_body_with_stale_identity: ExchangeRequest,
    oversized_profile: TransportProfile,
}

// r[verify lattice.workflow_interchange.transport_evidence]
// r[verify lattice.workflow_interchange.transport_evidence.scenario.nonclaims]
#[test]
fn accepted_producer_fixture_passes_independent_admission() {
    let fixtures: TransportFixtures = serde_json::from_str(PRODUCER_FIXTURES).expect("producer fixtures decode");

    assert_eq!(validate_transport_profile(&fixtures.profile), Ok(()));
    assert_eq!(validate_exchange_request(&fixtures.request), Ok(()));
    assert_eq!(validate_exchange_response(&fixtures.request, &fixtures.response), Ok(()));
}

// r[verify lattice.workflow_interchange.transport_evidence.scenario.negative]
#[test]
fn rejected_producer_fixtures_keep_stable_failure_classes() {
    let fixtures: TransportFixtures = serde_json::from_str(PRODUCER_FIXTURES).expect("producer fixtures decode");

    assert_eq!(
        validate_exchange_request(&fixtures.rejected.changed_body_with_stale_identity),
        Err(AdmissionError::RequestIdentity)
    );
    assert_eq!(validate_transport_profile(&fixtures.rejected.oversized_profile), Err(AdmissionError::ProfileLimit));
}
