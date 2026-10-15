#![allow(
    unknown_lints,
    clippy::expect_used,
    assertion_density,
    non_trait_imports,
    no_unwrap,
    reason = "checked-in recovery fixtures must stop at the first malformed producer value"
)]

use lattice_workflow_transport::*;
use serde::Deserialize;

const RECOVERY_FIXTURES: &str = include_str!("../fixtures/recovery-fixtures-v1.json");

#[derive(Deserialize)]
struct RecoveryFixtures {
    request: ExchangeRequest,
    accepted_response: ExchangeResponse,
    not_found_response: ExchangeResponse,
}

// r[verify lattice.workflow_interchange.transport_dispatch_recovery_result]
#[test]
fn recovery_producer_fixtures_pass_independent_admission() {
    let fixtures: RecoveryFixtures = serde_json::from_str(RECOVERY_FIXTURES).expect("recovery fixtures decode");
    assert_eq!(validate_exchange_request(&fixtures.request), Ok(()));
    assert_eq!(validate_exchange_response(&fixtures.request, &fixtures.accepted_response), Ok(()));
    assert_eq!(validate_exchange_response(&fixtures.request, &fixtures.not_found_response), Ok(()));
}
