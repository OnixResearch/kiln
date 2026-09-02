use hegel::TestCase;
use hegel::generators as gs;
use valence_core::build_service::BUILD_SERVICE_REQUIRED_NON_CLAIM;
use valence_core::build_service::BuildOutputRef;
use valence_core::build_service::BuildResultRecord;
use valence_core::build_service::BuildServiceRole;
use valence_core::build_service::ResourceDecisionRecord;
use valence_core::build_service::build_result_identity;
use valence_core::build_service::resource_decision_identity;
use valence_core::hegel_property::HEGEL_FAST_CASE_BUDGET;

const PROPERTY_MIN_SALT: u64 = 1;
const PROPERTY_MAX_SALT: u64 = 1_000;
const MAX_PERMUTATION_SWAPS: u64 = 8;

fn non_claims() -> Vec<String> {
    vec![BUILD_SERVICE_REQUIRED_NON_CLAIM.to_owned()]
}

fn hex(seed: &str) -> String {
    let mut out = String::with_capacity(64);
    let bytes = seed.as_bytes();
    for index in 0..32 {
        let byte = bytes[index % bytes.len()];
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn sample_decision(salt: u64) -> ResourceDecisionRecord {
    ResourceDecisionRecord {
        decision_id: format!("decision-{salt}"),
        attempt_identity: hex(&format!("attempt-{salt}")),
        declared_minima: vec!["mem>=512MiB".to_owned(), "cpu>=2".to_owned()],
        eligible_class_identities: vec![hex("small"), hex("large"), hex("xlarge")],
        selected_class_identity: hex("large"),
        policy: "mantle-resource-policy.v1".to_owned(),
        compatible_observation_identities: vec![hex("obs-a"), hex("obs-b")],
        reasons: vec!["first-reason".to_owned(), "second-reason".to_owned()],
        role: BuildServiceRole::Verified,
        non_claims: non_claims(),
    }
}

fn sample_result(salt: u64) -> BuildResultRecord {
    BuildResultRecord {
        result_id: format!("result-{salt}"),
        request_identity: hex("request"),
        attempt_identity: hex(&format!("attempt-{salt}")),
        platform: "x86_64-linux".to_owned(),
        policy: "mantle-build-policy.v1".to_owned(),
        outputs: vec![
            BuildOutputRef {
                path: "out/a".to_owned(),
                content_hash: hex("a"),
            },
            BuildOutputRef {
                path: "out/b".to_owned(),
                content_hash: hex("b"),
            },
            BuildOutputRef {
                path: "out/c".to_owned(),
                content_hash: hex("c"),
            },
        ],
        producer_signature_ref: "mantle-signer:sig-1".to_owned(),
        cas_observation: "cas:present".to_owned(),
        role: BuildServiceRole::Signed,
        non_claims: non_claims(),
    }
}

fn permute<T>(items: &mut [T], tc: &TestCase) {
    let length = items.len();
    if length < 2 {
        return;
    }
    let swaps = tc.draw(gs::integers::<u64>().min_value(1).max_value(MAX_PERMUTATION_SWAPS));
    for _ in 0..swaps {
        let left = tc.draw(gs::integers::<u64>().min_value(0).max_value((length - 1) as u64)) as usize;
        let right = tc.draw(gs::integers::<u64>().min_value(0).max_value((length - 1) as u64)) as usize;
        items.swap(left, right);
    }
}

// r[verify valence.evidence_ir.build_service_identity]
#[hegel::test(test_cases = HEGEL_FAST_CASE_BUDGET)]
fn canonical_identity_is_deterministic(tc: TestCase) {
    let salt = tc.draw(gs::integers::<u64>().min_value(PROPERTY_MIN_SALT).max_value(PROPERTY_MAX_SALT));
    let first = sample_decision(salt);
    let second = sample_decision(salt);
    assert_eq!(resource_decision_identity(&first), resource_decision_identity(&second));
    assert_eq!(build_result_identity(&sample_result(salt)), build_result_identity(&sample_result(salt)));
}

// r[verify valence.evidence_ir.build_service_identity]
#[hegel::test(test_cases = HEGEL_FAST_CASE_BUDGET)]
fn set_like_fields_are_permutation_invariant(tc: TestCase) {
    let salt = tc.draw(gs::integers::<u64>().min_value(PROPERTY_MIN_SALT).max_value(PROPERTY_MAX_SALT));
    let base = sample_decision(salt);
    let mut permuted = base.clone();
    permute(&mut permuted.declared_minima, &tc);
    permute(&mut permuted.eligible_class_identities, &tc);
    permute(&mut permuted.compatible_observation_identities, &tc);
    assert_eq!(resource_decision_identity(&base), resource_decision_identity(&permuted));

    let result = sample_result(salt);
    let mut reordered = result.clone();
    permute(&mut reordered.outputs, &tc);
    assert_eq!(build_result_identity(&result), build_result_identity(&reordered));
}

// r[verify valence.evidence_ir.build_service_identity]
#[hegel::test(test_cases = HEGEL_FAST_CASE_BUDGET)]
fn ordered_fields_reject_permutation(tc: TestCase) {
    let salt = tc.draw(gs::integers::<u64>().min_value(PROPERTY_MIN_SALT).max_value(PROPERTY_MAX_SALT));
    let base = sample_decision(salt);
    let mut swapped = base.clone();
    permute(&mut swapped.reasons, &tc);
    if swapped.reasons != base.reasons {
        assert_ne!(resource_decision_identity(&base), resource_decision_identity(&swapped));
    }
}
