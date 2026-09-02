// Fixture test: decode and rejection mismatches are immediate test-harness failures, not
// recoverable product errors.
#![allow(
    unknown_lints,
    clippy::expect_used,
    non_trait_imports,
    no_unwrap,
    reason = "the independent conformance test must stop at the first malformed reviewed fixture"
)]

use lattice_workflow_contract::CapabilityProfile;
use lattice_workflow_contract::ErrorCode;
use lattice_workflow_contract::ExecutionObservation;
use lattice_workflow_contract::WorkflowAccepted;
use lattice_workflow_contract::WorkflowRequest;
use lattice_workflow_contract::validate_observation;
use lattice_workflow_contract::validate_profile;
use lattice_workflow_contract::validate_request;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProducerFixtures {
    schema: String,
    profile: CapabilityProfile,
    request: WorkflowRequest,
    accepted: WorkflowAccepted,
    observation: ExecutionObservation,
    rejected: RejectedFixtures,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RejectedFixtures {
    cyclic_request: WorkflowRequest,
    false_paused_success: ExecutionObservation,
    stale_execution: ExecutionObservation,
}

fn fixtures() -> ProducerFixtures {
    serde_json::from_str(include_str!("../fixtures/producer-fixtures-v1.json"))
        .expect("checked-in producer fixtures decode")
}

// r[verify lattice.workflow_interchange.evidence]
// r[verify lattice.workflow_interchange.evidence.scenario.nonclaims]
#[test]
fn accepted_producer_fixtures_pass_independent_admission() {
    let fixtures = fixtures();

    assert_eq!(fixtures.schema, "lattice.workflow-producer-fixtures.v1");
    assert_eq!(validate_profile(&fixtures.profile), Ok(()));
    assert_eq!(validate_request(&fixtures.profile, &fixtures.request), Ok(()));
    assert_eq!(
        validate_observation(&fixtures.profile, &fixtures.request, &fixtures.accepted, &fixtures.observation,),
        Ok(())
    );
}

// r[verify lattice.workflow_interchange.evidence.scenario.negative]
#[test]
fn rejected_producer_fixtures_keep_stable_failure_classes() {
    let fixtures = fixtures();

    let cyclic = validate_request(&fixtures.profile, &fixtures.rejected.cyclic_request)
        .expect_err("cyclic producer fixture must fail");
    assert_eq!(cyclic.code, ErrorCode::GraphCycle);

    let paused = validate_observation(
        &fixtures.profile,
        &fixtures.request,
        &fixtures.accepted,
        &fixtures.rejected.false_paused_success,
    )
    .expect_err("false paused success fixture must fail");
    assert_eq!(paused.code, ErrorCode::ObservationStateMismatch);

    let stale = validate_observation(
        &fixtures.profile,
        &fixtures.request,
        &fixtures.accepted,
        &fixtures.rejected.stale_execution,
    )
    .expect_err("stale execution fixture must fail");
    assert_eq!(stale.code, ErrorCode::ObservationLinkageMismatch);
}
