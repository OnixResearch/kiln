use std::collections::BTreeSet;

use serde_json::Value;
use valence_core::build_service::BuildServiceBundle;
use valence_core::build_service::attempt_identity;
use valence_core::build_service::authority_decision_identity;
use valence_core::build_service::build_result_identity;
use valence_core::build_service::external_status_identity;
use valence_core::build_service::machine_class_identity;
use valence_core::build_service::orchestration_identity;
use valence_core::build_service::resource_decision_identity;
use valence_core::build_service::resource_observation_identity;
use valence_core::build_service::retry_edge_identity;
use valence_core::build_service::service_request_identity;
use valence_core::build_service::transition_identity;
use valence_core::build_service::usage_identity;
use valence_core::build_service::validate_build_service_bundle;

const MANTLE_BUILD: &str = include_str!("../../../fixtures/build-service-evidence/mantle-build.json");
const BASALT_AUTHORITY: &str = include_str!("../../../fixtures/build-service-evidence/basalt-authority.json");
const LATTICE_ORCHESTRATION: &str = include_str!("../../../fixtures/build-service-evidence/lattice-orchestration.json");
const ONIXOS_CONSUMPTION: &str = include_str!("../../../fixtures/build-service-evidence/onixos-consumption.json");
const CAIRN_CHECK_RUN: &str = include_str!("../../../fixtures/build-service-evidence/cairn-check-run.json");

fn bundle_identities(bundle: &BuildServiceBundle) -> BTreeSet<String> {
    let mut identities = BTreeSet::new();
    identities.extend(bundle.service_requests.iter().map(service_request_identity));
    identities.extend(bundle.attempts.iter().map(attempt_identity));
    identities.extend(bundle.transitions.iter().map(transition_identity));
    identities.extend(bundle.authority_decisions.iter().map(authority_decision_identity));
    identities.extend(bundle.machine_classes.iter().map(machine_class_identity));
    identities.extend(bundle.resource_decisions.iter().map(resource_decision_identity));
    identities.extend(bundle.resource_observations.iter().map(resource_observation_identity));
    identities.extend(bundle.retry_edges.iter().map(retry_edge_identity));
    identities.extend(bundle.usage_records.iter().map(usage_identity));
    identities.extend(bundle.build_results.iter().map(build_result_identity));
    identities.extend(bundle.orchestration_jobs.iter().map(orchestration_identity));
    identities.extend(bundle.external_statuses.iter().map(external_status_identity));
    identities
}

fn projection_identities(json: &str) -> BTreeSet<String> {
    let value: Value = serde_json::from_str(json).expect("projection parses");
    value["record_identities"]
        .as_array()
        .expect("projection carries record identities")
        .iter()
        .map(|entry| entry.as_str().expect("identity is text").to_owned())
        .collect()
}

// r[verify valence.evidence_ir.build_service_profile]
#[test]
fn checked_in_bundle_validates() {
    let bundle: BuildServiceBundle = serde_json::from_str(MANTLE_BUILD).expect("bundle parses");
    let report = validate_build_service_bundle(&bundle);
    assert!(report.valid, "issues: {:?}", report.issues);
}

// r[verify valence.evidence_ir.build_service_profile]
#[test]
fn consumer_projections_reference_bundle_identities() {
    let bundle: BuildServiceBundle = serde_json::from_str(MANTLE_BUILD).expect("bundle parses");
    let identities = bundle_identities(&bundle);
    for projection in [
        BASALT_AUTHORITY,
        LATTICE_ORCHESTRATION,
        ONIXOS_CONSUMPTION,
        CAIRN_CHECK_RUN,
    ] {
        for identity in projection_identities(projection) {
            assert!(identities.contains(&identity), "projection references unknown identity {identity}");
        }
    }
}

// r[verify valence.evidence_ir.build_service_non_claims]
#[test]
fn tampered_bundle_fails_closed() {
    let mut bundle: BuildServiceBundle = serde_json::from_str(MANTLE_BUILD).expect("bundle parses");
    bundle.external_statuses[0].observed_state = "success; proves release eligibility".to_owned();
    bundle.external_statuses[0].non_claims = vec!["this status proves release eligibility".to_owned()];
    let report = validate_build_service_bundle(&bundle);
    assert!(!report.valid);
}
