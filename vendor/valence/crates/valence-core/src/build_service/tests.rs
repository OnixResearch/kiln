use super::*;

fn hex(seed: &str) -> String {
    let mut out = String::with_capacity(64);
    let bytes = seed.as_bytes();
    for index in 0..32 {
        let byte = bytes[index % bytes.len()];
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn non_claims() -> Vec<String> {
    vec![BUILD_SERVICE_REQUIRED_NON_CLAIM.to_owned()]
}

fn request(id: &str) -> ServiceRequestRecord {
    ServiceRequestRecord {
        request_id: id.to_owned(),
        concrete_request_hash: hex(id),
        idempotency_key: format!("idem-{id}"),
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    }
}

fn attempt(id: &str, generation: u32, request_identity: &str, parent: Option<String>) -> AttemptRecord {
    AttemptRecord {
        attempt_id: id.to_owned(),
        request_identity: request_identity.to_owned(),
        generation,
        parent_attempt_identity: parent,
        service_policy: "mantle-service-policy.v1".to_owned(),
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    }
}

fn transition(attempt_identity: &str, prior: &str, next: &str, sequence: u32) -> AttemptTransitionRecord {
    AttemptTransitionRecord {
        attempt_identity: attempt_identity.to_owned(),
        prior_state: prior.to_owned(),
        next_state: next.to_owned(),
        sequence,
        observed_at: "2026-08-05T12:00:00Z".to_owned(),
        producer: "mantle".to_owned(),
        reason: format!("{prior}-to-{next}"),
        role: BuildServiceRole::RecordedOnly,
        non_claims: non_claims(),
    }
}

fn authority(id: &str, attempt_identity: &str) -> AuthorityDecisionRecord {
    AuthorityDecisionRecord {
        decision_id: id.to_owned(),
        attempt_identity: attempt_identity.to_owned(),
        subject_projection: "basalt-public-subject:builder".to_owned(),
        policy_identity: hex(id),
        operation: "build.execute".to_owned(),
        decision: "allow".to_owned(),
        verifier_evidence_ref: Some("basalt-verification:receipt-1".to_owned()),
        role: BuildServiceRole::Verified,
        non_claims: non_claims(),
    }
}

fn machine_class(id: &str, ordinal: u32) -> MachineClassRecord {
    MachineClassRecord {
        class_id: id.to_owned(),
        platform: "x86_64-linux".to_owned(),
        resource_class: format!("class-{id}"),
        ordinal,
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    }
}

fn observation(id: &str, attempt_identity: &str) -> ResourceObservationRecord {
    ResourceObservationRecord {
        observation_id: id.to_owned(),
        attempt_identity: attempt_identity.to_owned(),
        measurements: vec![
            ResourceMeasurement {
                name: "wall-clock".to_owned(),
                value: 42,
                unit: "s".to_owned(),
            },
            ResourceMeasurement {
                name: "peak-rss".to_owned(),
                value: 1024,
                unit: "MiB".to_owned(),
            },
        ],
        collector: "mantle-executor".to_owned(),
        compatibility_scope: "class-comparison".to_owned(),
        terminal_outcome: "succeeded".to_owned(),
        role: BuildServiceRole::RecordedOnly,
        non_claims: non_claims(),
    }
}

struct ValidGraph {
    bundle: BuildServiceBundle,
    request_one_identity: String,
    attempt_two_identity: String,
    class_small_identity: String,
    class_large_identity: String,
}

fn valid_graph() -> ValidGraph {
    let request_one = request("request-one");
    let request_two = request("request-two");
    let request_one_identity = service_request_identity(&request_one);
    let request_two_identity = service_request_identity(&request_two);

    let attempt_one = attempt("attempt-one", 1, &request_one_identity, None);
    let attempt_one_identity = attempt_identity(&attempt_one);
    let attempt_two = attempt("attempt-two", 2, &request_one_identity, Some(attempt_one_identity.clone()));
    let attempt_two_identity = attempt_identity(&attempt_two);

    let transitions = vec![
        transition(&attempt_one_identity, "queued", "scheduled", 1),
        transition(&attempt_one_identity, "scheduled", "running", 2),
        transition(&attempt_one_identity, "running", "failed", 3),
        transition(&attempt_two_identity, "queued", "scheduled", 1),
        transition(&attempt_two_identity, "scheduled", "running", 2),
        transition(&attempt_two_identity, "running", "succeeded", 3),
    ];

    let decision = authority("decision-one", &attempt_two_identity);
    let reuse_decision = authority("decision-reuse", &attempt_two_identity);
    let reuse_decision_identity = authority_decision_identity(&reuse_decision);

    let class_small = machine_class("small", 1);
    let class_large = machine_class("large", 2);
    let class_small_identity = machine_class_identity(&class_small);
    let class_large_identity = machine_class_identity(&class_large);

    let obs = observation("observation-one", &attempt_two_identity);
    let observation_identity = resource_observation_identity(&obs);

    let resource_decision = ResourceDecisionRecord {
        decision_id: "resource-decision-one".to_owned(),
        attempt_identity: attempt_two_identity.clone(),
        declared_minima: vec!["mem>=512MiB".to_owned()],
        eligible_class_identities: vec![class_small_identity.clone(), class_large_identity.clone()],
        selected_class_identity: class_large_identity.clone(),
        policy: "mantle-resource-policy.v1".to_owned(),
        compatible_observation_identities: vec![observation_identity.clone()],
        reasons: vec!["oom-escalation".to_owned(), "larger-class-eligible".to_owned()],
        role: BuildServiceRole::Verified,
        non_claims: non_claims(),
    };

    let retry_edge = RetryEdgeRecord {
        predecessor_attempt_identity: attempt_one_identity.clone(),
        successor_attempt_identity: attempt_two_identity.clone(),
        trigger: "oom_escalation".to_owned(),
        trigger_evidence: format!("oom-kill-event:{}", hex("oom")),
        prior_class_identity: class_small_identity.clone(),
        next_class_identity: class_large_identity.clone(),
        limit_state: "memory-limit-exceeded".to_owned(),
        policy: "mantle-retry-policy.v1".to_owned(),
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    };

    let usage_records = vec![
        UsageRecord {
            usage_id: "usage-reservation".to_owned(),
            attempt_identity: attempt_two_identity.clone(),
            usage_kind: "reservation".to_owned(),
            unit_schedule: "cpu-second:1".to_owned(),
            window: "2026-08-05T12:00:00Z/2026-08-05T12:05:00Z".to_owned(),
            scope: "project:onix".to_owned(),
            outcome: "reserved".to_owned(),
            role: BuildServiceRole::RecordedOnly,
            non_claims: non_claims(),
        },
        UsageRecord {
            usage_id: "usage-reconciliation".to_owned(),
            attempt_identity: attempt_two_identity.clone(),
            usage_kind: "reconciliation".to_owned(),
            unit_schedule: "cpu-second:1".to_owned(),
            window: "2026-08-05T12:00:00Z/2026-08-05T12:05:00Z".to_owned(),
            scope: "project:onix".to_owned(),
            outcome: "reconciled".to_owned(),
            role: BuildServiceRole::RecordedOnly,
            non_claims: non_claims(),
        },
    ];

    let result = BuildResultRecord {
        result_id: "result-one".to_owned(),
        request_identity: request_one_identity.clone(),
        attempt_identity: attempt_two_identity.clone(),
        platform: "x86_64-linux".to_owned(),
        policy: "mantle-build-policy.v1".to_owned(),
        outputs: vec![
            BuildOutputRef {
                path: "out/bin/tool".to_owned(),
                content_hash: hex("tool"),
            },
            BuildOutputRef {
                path: "out/lib/core".to_owned(),
                content_hash: hex("core"),
            },
        ],
        producer_signature_ref: "mantle-signer:sig-1".to_owned(),
        cas_observation: "cas:present".to_owned(),
        role: BuildServiceRole::Signed,
        non_claims: non_claims(),
    };
    let result_identity = build_result_identity(&result);

    let reuse_edge = ReuseEdgeRecord {
        producer_result_identity: result_identity.clone(),
        consumer_request_identity: request_two_identity.clone(),
        sharing_policy: "mantle-sharing-policy.v1".to_owned(),
        authorization_decision_identity: Some(reuse_decision_identity.clone()),
        request_compatible: true,
        platform_compatible: true,
        output_check_ref: Some("output-check:ok".to_owned()),
        signature_ref: Some("mantle-signer:sig-1".to_owned()),
        cas_observation: Some("cas:present".to_owned()),
        usability_outcome: "usable".to_owned(),
        role: BuildServiceRole::Verified,
        non_claims: non_claims(),
    };

    let job = OrchestrationRecord {
        job_id: "lattice-job-1".to_owned(),
        workflow_identity: "lattice-workflow:build".to_owned(),
        matrix_job_identity: "matrix:x86_64-linux".to_owned(),
        attempt_identity: attempt_two_identity.clone(),
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    };
    let job_identity = orchestration_identity(&job);

    let status = ExternalStatusRecord {
        status_id: "check-run-1".to_owned(),
        provider: "github-check-run".to_owned(),
        job_identity: job_identity.clone(),
        attempt_identity: attempt_two_identity.clone(),
        observed_state: "success".to_owned(),
        role: BuildServiceRole::External,
        non_claims: non_claims(),
    };

    let bundle = BuildServiceBundle {
        profile: BuildServiceEvidenceProfile {
            profile_id: BUILD_SERVICE_PROFILE_ID.to_owned(),
            profile_version: 1,
            canonicalization: BUILD_SERVICE_CANONICALIZATION.to_owned(),
            producer_policy: hex("mantle-policy"),
            claimed_observation_classes: vec![
                "build".to_owned(),
                "retry".to_owned(),
                "reuse".to_owned(),
                "usage".to_owned(),
                "external-status".to_owned(),
            ],
            non_claims: non_claims(),
        },
        service_requests: vec![request_one, request_two],
        attempts: vec![attempt_one, attempt_two],
        transitions,
        authority_decisions: vec![decision, reuse_decision],
        machine_classes: vec![class_small, class_large],
        resource_decisions: vec![resource_decision],
        resource_observations: vec![obs],
        retry_edges: vec![retry_edge],
        usage_records,
        build_results: vec![result],
        reuse_edges: vec![reuse_edge],
        orchestration_jobs: vec![job],
        external_statuses: vec![status],
    };

    ValidGraph {
        bundle,
        request_one_identity,
        attempt_two_identity,
        class_small_identity,
        class_large_identity,
    }
}

fn issue_codes(report: &BuildServiceReport) -> Vec<&'static str> {
    report.issues.iter().map(|issue| issue.code).collect()
}

#[test]
fn complete_successful_build_validates() {
    // r[verify valence.evidence_ir.build_service_profile]
    let graph = valid_graph();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(report.valid, "issues: {:?}", report.issues);
    assert_eq!(report.record_count, 23);
}

#[test]
fn optional_records_may_be_absent() {
    // r[verify valence.evidence_ir.build_service_profile]
    let mut graph = valid_graph();
    graph.bundle.retry_edges.clear();
    graph.bundle.reuse_edges.clear();
    graph.bundle.external_statuses.clear();
    graph.bundle.usage_records.clear();
    graph.bundle.profile.claimed_observation_classes = vec!["build".to_owned()];
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(report.valid, "issues: {:?}", report.issues);
}

#[test]
fn identity_is_stable_and_domain_separated() {
    // r[verify valence.evidence_ir.build_service_identity]
    let request_one = request("request-one");
    let again = request("request-one");
    assert_eq!(service_request_identity(&request_one), service_request_identity(&again));

    let attempt_one = attempt("request-one", 1, &service_request_identity(&request_one), None);
    assert_ne!(service_request_identity(&request_one), attempt_identity(&attempt_one));
}

#[test]
fn set_like_fields_canonicalize_under_permutation() {
    // r[verify valence.evidence_ir.build_service_identity]
    let graph = valid_graph();
    let base = &graph.bundle.resource_decisions[0];
    let mut permuted = base.clone();
    permuted.eligible_class_identities.reverse();
    assert_eq!(resource_decision_identity(base), resource_decision_identity(&permuted));

    let result = &graph.bundle.build_results[0];
    let mut reordered = result.clone();
    reordered.outputs.reverse();
    assert_eq!(build_result_identity(result), build_result_identity(&reordered));

    let mut reasons_swapped = base.clone();
    reasons_swapped.reasons.reverse();
    assert_ne!(resource_decision_identity(base), resource_decision_identity(&reasons_swapped));
}

#[test]
fn malformed_digest_and_version_fail() {
    // r[verify valence.evidence_ir.build_service_profile]
    let mut graph = valid_graph();
    graph.bundle.service_requests[0].concrete_request_hash = "not-a-digest".to_owned();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(!report.valid);
    assert!(issue_codes(&report).contains(&"request_identity_malformed"));

    let mut graph = valid_graph();
    graph.bundle.profile.profile_version = 2;
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"profile_version_unsupported"));

    let mut graph = valid_graph();
    graph.bundle.profile.canonicalization = "unspecified".to_owned();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"canonicalization_unsupported"));
}

#[test]
fn duplicate_and_dangling_links_fail() {
    // r[verify valence.evidence_ir.build_service_profile]
    let mut graph = valid_graph();
    let duplicate = graph.bundle.service_requests[0].clone();
    graph.bundle.service_requests.push(duplicate);
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"duplicate_identity"));

    let mut graph = valid_graph();
    graph.bundle.attempts[0].request_identity = hex("unknown-request");
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"dangling_link"));
}

#[test]
fn wrong_kind_reference_is_dangling() {
    // r[verify valence.evidence_ir.result_reuse_edges]
    let mut graph = valid_graph();
    graph.bundle.reuse_edges[0].authorization_decision_identity = Some(graph.attempt_two_identity.clone());
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(!report.valid);
    assert!(issue_codes(&report).contains(&"dangling_link"));
}

#[test]
fn ambiguous_timestamp_fails() {
    // r[verify valence.evidence_ir.build_service_identity]
    let mut graph = valid_graph();
    graph.bundle.transitions[0].observed_at = "2026-08-05 12:00".to_owned();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"timestamp_ambiguous"));
}

#[test]
fn oom_retry_requires_evidence_and_increasing_class() {
    // r[verify valence.evidence_ir.resource_retry_links]
    let mut graph = valid_graph();
    graph.bundle.retry_edges[0].trigger_evidence = String::new();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"oom_evidence_missing"));

    let mut graph = valid_graph();
    graph.bundle.retry_edges[0].next_class_identity = graph.class_small_identity.clone();
    graph.bundle.retry_edges[0].prior_class_identity = graph.class_large_identity.clone();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"class_not_increasing"));

    let mut graph = valid_graph();
    graph.bundle.retry_edges[0].trigger = "manual".to_owned();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"trigger_unsupported"));
}

#[test]
fn usage_reconciliation_conflicts_fail() {
    // r[verify valence.evidence_ir.usage_links]
    let mut graph = valid_graph();
    let duplicate = graph.bundle.usage_records[1].clone();
    graph.bundle.usage_records.push(duplicate);
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"duplicate_identity"));

    let mut graph = valid_graph();
    graph.bundle.usage_records[0].usage_kind = "accrual".to_owned();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"usage_kind_unsupported"));
}

#[test]
fn reuse_requires_distinct_facts() {
    // r[verify valence.evidence_ir.result_reuse_edges]
    let mut graph = valid_graph();
    graph.bundle.reuse_edges[0].authorization_decision_identity = None;
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"reuse_authority_missing"));

    let mut graph = valid_graph();
    graph.bundle.reuse_edges[0].signature_ref = None;
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"reuse_signature_missing"));

    let mut graph = valid_graph();
    graph.bundle.reuse_edges[0].cas_observation = None;
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"reuse_cas_missing"));

    let mut graph = valid_graph();
    graph.bundle.reuse_edges[0].consumer_request_identity = graph.request_one_identity.clone();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"producer_consumer_confusion"));

    let mut graph = valid_graph();
    graph.bundle.reuse_edges[0].usability_outcome = "assumed".to_owned();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"usability_unsupported"));
}

#[test]
fn external_status_cannot_overclaim() {
    // r[verify valence.evidence_ir.external_status_links]
    let mut graph = valid_graph();
    graph.bundle.external_statuses[0].role = BuildServiceRole::Verified;
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"external_status_role_overclaim"));

    let mut graph = valid_graph();
    graph.bundle.external_statuses[0].job_identity = hex("unknown-job");
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"dangling_link"));
}

#[test]
fn privacy_markers_fail_closed() {
    // r[verify valence.evidence_ir.build_service_privacy]
    let mut graph = valid_graph();
    graph.bundle.authority_decisions[0].subject_projection = "Bearer abc.def.ghi".to_owned();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"forbidden_material"));

    let mut graph = valid_graph();
    graph.bundle.resource_observations[0].collector = "raw_log:build.log".to_owned();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"forbidden_material"));
}

#[test]
fn non_claims_are_required_and_bounded() {
    // r[verify valence.evidence_ir.build_service_non_claims]
    let mut graph = valid_graph();
    graph.bundle.profile.non_claims = vec!["linkage is nice".to_owned()];
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"non_claims_weakened"));

    let mut graph = valid_graph();
    graph.bundle.profile.non_claims = vec![
        BUILD_SERVICE_REQUIRED_NON_CLAIM.to_owned(),
        "this evidence proves release eligibility".to_owned(),
    ];
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"overclaim_detected"));
}

#[test]
fn claimed_class_without_records_fails() {
    // r[verify valence.evidence_ir.build_service_profile]
    let mut graph = valid_graph();
    graph.bundle.retry_edges.clear();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"claimed_class_unsupported_by_records"));
}

#[test]
fn measurements_require_units() {
    // r[verify valence.evidence_ir.build_service_identity]
    let mut graph = valid_graph();
    graph.bundle.resource_observations[0].measurements[0].unit = String::new();
    let report = validate_build_service_bundle(&graph.bundle);
    assert!(issue_codes(&report).contains(&"unit_missing"));
}
