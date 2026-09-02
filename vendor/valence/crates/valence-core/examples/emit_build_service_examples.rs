//! Emit the checked-in build-service consumer examples under
//! `fixtures/build-service-evidence/`. Regenerate after changing the
//! `build_service` profile surface and review the diff before committing.

use std::fs;
use std::path::Path;

use valence_core::build_service::*;

const OUT_DIR: &str = "fixtures/build-service-evidence";

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

fn write_json(path: &Path, value: &impl serde::Serialize) {
    let mut text = serde_json::to_string_pretty(value).expect("example serializes");
    text.push('\n');
    fs::write(path, text).expect("example writes");
}

fn main() {
    let out = Path::new(OUT_DIR);
    fs::create_dir_all(out).expect("fixture directory");

    let request_one = ServiceRequestRecord {
        request_id: "request-one".to_owned(),
        concrete_request_hash: hex("request-one"),
        idempotency_key: "idem-request-one".to_owned(),
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    };
    let request_two = ServiceRequestRecord {
        request_id: "request-two".to_owned(),
        concrete_request_hash: hex("request-two"),
        idempotency_key: "idem-request-two".to_owned(),
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    };
    let request_one_identity = service_request_identity(&request_one);
    let request_two_identity = service_request_identity(&request_two);

    let attempt_one = AttemptRecord {
        attempt_id: "attempt-one".to_owned(),
        request_identity: request_one_identity.clone(),
        generation: 1,
        parent_attempt_identity: None,
        service_policy: "mantle-service-policy.v1".to_owned(),
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    };
    let attempt_one_identity = attempt_identity(&attempt_one);
    let attempt_two = AttemptRecord {
        attempt_id: "attempt-two".to_owned(),
        request_identity: request_one_identity.clone(),
        generation: 2,
        parent_attempt_identity: Some(attempt_one_identity.clone()),
        service_policy: "mantle-service-policy.v1".to_owned(),
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    };
    let attempt_two_identity = attempt_identity(&attempt_two);

    let transitions = vec![
        ("queued", "scheduled", 1u32, &attempt_one_identity),
        ("scheduled", "running", 2, &attempt_one_identity),
        ("running", "failed", 3, &attempt_one_identity),
        ("queued", "scheduled", 1, &attempt_two_identity),
        ("scheduled", "running", 2, &attempt_two_identity),
        ("running", "succeeded", 3, &attempt_two_identity),
    ]
    .into_iter()
    .map(|(prior, next, sequence, attempt_ref)| AttemptTransitionRecord {
        attempt_identity: attempt_ref.clone(),
        prior_state: prior.to_owned(),
        next_state: next.to_owned(),
        sequence,
        observed_at: "2026-08-05T12:00:00Z".to_owned(),
        producer: "mantle".to_owned(),
        reason: format!("{prior}-to-{next}"),
        role: BuildServiceRole::RecordedOnly,
        non_claims: non_claims(),
    })
    .collect::<Vec<_>>();

    let build_decision = AuthorityDecisionRecord {
        decision_id: "decision-build".to_owned(),
        attempt_identity: attempt_two_identity.clone(),
        subject_projection: "basalt-public-subject:builder".to_owned(),
        policy_identity: hex("basalt-policy"),
        operation: "build.execute".to_owned(),
        decision: "allow".to_owned(),
        verifier_evidence_ref: Some("basalt-verification:receipt-1".to_owned()),
        role: BuildServiceRole::Verified,
        non_claims: non_claims(),
    };
    let reuse_decision = AuthorityDecisionRecord {
        decision_id: "decision-reuse".to_owned(),
        attempt_identity: attempt_two_identity.clone(),
        subject_projection: "basalt-public-subject:consumer".to_owned(),
        policy_identity: hex("basalt-policy"),
        operation: "build.reuse".to_owned(),
        decision: "allow".to_owned(),
        verifier_evidence_ref: Some("basalt-verification:receipt-2".to_owned()),
        role: BuildServiceRole::Verified,
        non_claims: non_claims(),
    };
    let reuse_decision_identity = authority_decision_identity(&reuse_decision);

    let class_small = MachineClassRecord {
        class_id: "small".to_owned(),
        platform: "x86_64-linux".to_owned(),
        resource_class: "class-small".to_owned(),
        ordinal: 1,
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    };
    let class_large = MachineClassRecord {
        class_id: "large".to_owned(),
        platform: "x86_64-linux".to_owned(),
        resource_class: "class-large".to_owned(),
        ordinal: 2,
        role: BuildServiceRole::Linked,
        non_claims: non_claims(),
    };
    let class_small_identity = machine_class_identity(&class_small);
    let class_large_identity = machine_class_identity(&class_large);

    let observation = ResourceObservationRecord {
        observation_id: "observation-one".to_owned(),
        attempt_identity: attempt_two_identity.clone(),
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
    };
    let observation_identity = resource_observation_identity(&observation);

    let resource_decision = ResourceDecisionRecord {
        decision_id: "resource-decision-one".to_owned(),
        attempt_identity: attempt_two_identity.clone(),
        declared_minima: vec!["mem>=512MiB".to_owned()],
        eligible_class_identities: vec![class_small_identity.clone(), class_large_identity.clone()],
        selected_class_identity: class_large_identity.clone(),
        policy: "mantle-resource-policy.v1".to_owned(),
        compatible_observation_identities: vec![observation_identity],
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
        authority_decisions: vec![build_decision, reuse_decision],
        machine_classes: vec![class_small, class_large],
        resource_decisions: vec![resource_decision],
        resource_observations: vec![observation],
        retry_edges: vec![retry_edge],
        usage_records,
        build_results: vec![result],
        reuse_edges: vec![reuse_edge],
        orchestration_jobs: vec![job],
        external_statuses: vec![status],
    };

    let report = validate_build_service_bundle(&bundle);
    assert!(report.valid, "example bundle must validate: {:?}", report.issues);

    write_json(&out.join("mantle-build.json"), &bundle);
    write_json(
        &out.join("basalt-authority.json"),
        &serde_json::json!({
            "consumer": "basalt",
            "produces": ["authority_decisions"],
            "record_identities": [authority_decision_identity(&bundle.authority_decisions[0]), reuse_decision_identity],
            "non_claims": non_claims(),
        }),
    );
    write_json(
        &out.join("lattice-orchestration.json"),
        &serde_json::json!({
            "consumer": "lattice",
            "produces": ["orchestration_jobs"],
            "record_identities": [job_identity],
            "non_claims": non_claims(),
        }),
    );
    write_json(
        &out.join("onixos-consumption.json"),
        &serde_json::json!({
            "consumer": "onixos",
            "consumes": ["machine_classes", "resource_decisions", "usage_records"],
            "record_identities": [
                class_small_identity,
                class_large_identity,
                resource_decision_identity(&bundle.resource_decisions[0]),
                usage_identity(&bundle.usage_records[0]),
                usage_identity(&bundle.usage_records[1]),
            ],
            "non_claims": non_claims(),
        }),
    );
    write_json(
        &out.join("cairn-check-run.json"),
        &serde_json::json!({
            "consumer": "cairn",
            "consumes": ["external_statuses"],
            "record_identities": [external_status_identity(&bundle.external_statuses[0])],
            "non_claims": non_claims(),
        }),
    );

    println!("wrote build-service examples to {OUT_DIR}");
}
