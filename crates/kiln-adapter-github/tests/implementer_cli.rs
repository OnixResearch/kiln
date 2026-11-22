use hmac::{Hmac, Mac};
use kiln_adapter_github::implementation::ImplementationIssuePolicy;
use kiln_adapter_github::profile::ImplementationQueueConfig;
use kiln_core::implementation::{AcceptanceObligation, Enrollment, TerminalPolicy, WorkBudget};
use kiln_core::types::ContentId;
use kiln_implementer::{DeliveryQueue, Implementer};
use sha2::Sha256;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const SECRET: &[u8] = b"implementer-secret";

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn policy() -> ImplementationIssuePolicy {
    ImplementationIssuePolicy {
        repository: kiln_adapter_github::events::EventPolicy {
            owner: "octocat".to_string(),
            repository: "hello-world".to_string(),
            repository_id: 1296269,
            allow_fork_pull_requests: false,
            untrusted_pull_requests: Default::default(),
        },
        project: "kiln".to_string(),
        base: Some(
            kiln_core::types::ExternalRef::new("git", "commit", "base")
                .unwrap_or_else(|_| unreachable!()),
        ),
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: content('a'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        selection: None,
    }
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|_| unreachable!());
}

// r[verify kiln.implementation.operator_cli.scenario.drain] r[verify kiln.implementation.operator_cli.scenario.status] r[verify kiln.implementation.drain_loop.scenario.bounded] r[verify kiln.implementation.operator_claim.scenario.roundtrip] r[verify kiln.implementation.operator_claim.scenario.stale]
#[test]
fn drain_binary_admits_queued_issue_and_reports_status() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let queue_path = directory.path().join("queue.sqlite");
    let database = directory.path().join("work.sqlite");
    let secret_file = directory.path().join("secret");
    let policy_file = directory.path().join("policy.json");
    let enrollment_file = directory.path().join("enrollment.json");
    write(&secret_file, SECRET);
    write(
        &policy_file,
        &serde_json::to_vec(&policy()).unwrap_or_else(|_| unreachable!()),
    );
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    write(
        &enrollment_file,
        &serde_json::to_vec(&enrollment).unwrap_or_else(|_| unreachable!()),
    );

    let body = serde_json::to_vec(&serde_json::json!({
        "action": "opened",
        "repository": {"id": 1296269, "name": "Hello-World", "owner": {"login": "octocat"}},
        "sender": {"login": "operator"},
        "issue": {
            "number": 44,
            "title": "operator drain",
            "state": "open",
            "updated_at": "2026-11-11T10:00:00Z"
        }
    }))
    .unwrap_or_else(|_| unreachable!());
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET).unwrap_or_else(|_| unreachable!());
    mac.update(&body);
    let signature = format!(
        "sha256={}",
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let mut queue = DeliveryQueue::open(&queue_path, 8).unwrap_or_else(|_| unreachable!());
    queue
        .record(
            "00000000-0000-0000-0000-000000000001",
            "issues",
            &signature,
            &body,
            1,
        )
        .unwrap_or_else(|_| unreachable!());

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("drain")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--queue")
        .arg(&queue_path)
        .arg("--maximum-deliveries")
        .arg("8")
        .arg("--secret-file")
        .arg(&secret_file)
        .arg("--policy")
        .arg(&policy_file)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--limit")
        .arg("8")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "{\"drained\":1}"
    );

    let mut implementer =
        Implementer::open(&database, enrollment.clone()).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        implementer
            .work()
            .unwrap_or_else(|_| unreachable!())
            .current()
            .count(),
        1
    );
    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("status")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "{\"work\":1,\"attempts\":0,\"active\":0,\"verified\":0,\"published\":0,\"integrated\":0,\"source_reconciled\":0,\"failed\":0,\"blocked\":0,\"cancelled\":0,\"superseded\":0}"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("run")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--queue")
        .arg(&queue_path)
        .arg("--maximum-deliveries")
        .arg("8")
        .arg("--secret-file")
        .arg(&secret_file)
        .arg("--policy")
        .arg(&policy_file)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--limit")
        .arg("8")
        .arg("--interval-ms")
        .arg("1")
        .arg("--iterations")
        .arg("2")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "{\"iterations\":2,\"drained\":0}"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("claim")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--lease-ms")
        .arg("60000")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let attempt: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| unreachable!());
    let identity = attempt
        .get("identity")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| unreachable!())
        .to_string();
    let fence = attempt
        .get("fence")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        attempt.get("phase").and_then(serde_json::Value::as_str),
        Some("claimed")
    );

    let renewed = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("renew")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--attempt")
        .arg(&identity)
        .arg("--fence")
        .arg(fence.to_string())
        .arg("--lease-ms")
        .arg("600000")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        renewed.status.success(),
        "{}",
        String::from_utf8_lossy(&renewed.stderr)
    );

    let attempt_identity =
        ContentId::from_blake3_hex(identity.clone()).unwrap_or_else(|_| unreachable!());
    let mut implementer = Implementer::open(
        &database,
        Enrollment {
            repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        },
    )
    .unwrap_or_else(|_| unreachable!());
    let stored_attempt = implementer
        .attempt(&attempt_identity)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("stored attempt"));
    let work = implementer
        .work()
        .unwrap_or_else(|_| unreachable!())
        .current()
        .find(|work| {
            work.identity()
                .is_ok_and(|identity| identity == stored_attempt.work_identity)
        })
        .cloned()
        .unwrap_or_else(|| unreachable!("work"));
    drop(implementer);
    let plan = kiln_core::implementation::ImplementationDispatchPlan {
        schema: kiln_core::implementation::DISPATCH_PLAN_SCHEMA.to_string(),
        work_identity: stored_attempt.work_identity.clone(),
        work_revision: stored_attempt.work_revision,
        attempt_identity: attempt_identity.clone(),
        fence: stored_attempt.fence,
        repository: work.repository.clone(),
        base: stored_attempt.base.clone(),
        workspace: kiln_core::implementation::DispatchWorkspace {
            identity: content('d'),
            isolation: kiln_core::implementation::WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
            projection: None,
        },
        provider_profile: kiln_core::types::ProviderProfileId::new("lattice-implementation")
            .unwrap_or_else(|_| unreachable!()),
        permitted_paths: vec!["crates".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        controls: kiln_core::implementation::DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 60,
        token_budget: 10_000,
        invocation_budget: 10,
        expected_output: content('e'),
        execution: None,
    };
    let plan_digest = kiln_core::implementation::admit_dispatch_plan(&work, &stored_attempt, &plan)
        .unwrap_or_else(|_| unreachable!());
    let plan_file = directory.path().join("dispatch-plan.json");
    write(
        &plan_file,
        &serde_json::to_vec(&plan).unwrap_or_else(|_| unreachable!()),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("reserve-plan")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--plan-file")
        .arg(&plan_file)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("kiln.implementation-dispatch.v1"));

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("reserve")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--attempt")
        .arg(&identity)
        .arg("--fence")
        .arg(fence.to_string())
        .arg("--plan")
        .arg(plan_digest.as_str())
        .arg("--provider")
        .arg("lattice-implementation")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("kiln.implementation-dispatch.v1"));

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("start")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--attempt")
        .arg(&identity)
        .arg("--fence")
        .arg(fence.to_string())
        .arg("--workflow-kind")
        .arg("run")
        .arg("--workflow-id")
        .arg("one")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reconcile = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("reconcile")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--attempt")
        .arg(&identity)
        .arg("--fence")
        .arg(fence.to_string())
        .arg("--workflow-kind")
        .arg("run")
        .arg("--workflow-id")
        .arg("one")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        reconcile.status.success(),
        "{}",
        String::from_utf8_lossy(&reconcile.stderr)
    );
    let stored = Implementer::open(
        &database,
        Enrollment {
            repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        },
    )
    .unwrap_or_else(|_| unreachable!())
    .attempt(&ContentId::from_blake3_hex(identity.clone()).unwrap_or_else(|_| unreachable!()))
    .unwrap_or_else(|_| unreachable!())
    .unwrap_or_else(|| unreachable!("stored attempt"));
    assert_eq!(
        stored.phase,
        kiln_core::implementation::AttemptPhase::Executing
    );
    assert!(stored.dispatch.is_some());
    assert!(stored.workflow.is_some());

    let handoff = kiln_core::implementation::CandidateHandoff {
        schema: kiln_core::implementation::CANDIDATE_HANDOFF_SCHEMA.to_string(),
        work_identity: stored.work_identity.clone(),
        work_revision: stored.work_revision,
        attempt_identity: stored.identity.clone(),
        fence: stored.fence,
        repository: work.repository.clone(),
        base: stored.base.clone(),
        candidate: kiln_core::types::ExternalRef::new("git", "commit", "candidate")
            .unwrap_or_else(|_| unreachable!()),
        tree: kiln_core::types::ExternalRef::new("git", "tree", "tree")
            .unwrap_or_else(|_| unreachable!()),
        changed_paths: vec!["crates/src/lib.rs".to_string()],
        acceptance_digest: kiln_core::implementation::acceptance_digest(&work)
            .unwrap_or_else(|_| unreachable!()),
        verification_profile: work.verification_profile.clone(),
        toolchain: content('7'),
        environment: content('8'),
        evidence: vec![kiln_core::implementation::CandidateEvidence {
            identity: content('e'),
            kind: kiln_core::implementation::EvidenceKind::Check,
            outcome: kiln_core::implementation::EvidenceOutcome::Passed,
            reference: kiln_core::types::ExternalRef::new("kiln", "run", "candidate-check")
                .unwrap_or_else(|_| unreachable!()),
        }],
    };
    let handoff_file = directory.path().join("candidate-handoff.json");
    write(
        &handoff_file,
        &serde_json::to_vec(&handoff).unwrap_or_else(|_| unreachable!()),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("handoff")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--handoff-file")
        .arg(&handoff_file)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"handoff\""));
    let retained = Implementer::open(
        &database,
        Enrollment {
            repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        },
    )
    .unwrap_or_else(|_| unreachable!())
    .candidate_handoff(&stored.identity)
    .unwrap_or_else(|_| unreachable!())
    .unwrap_or_else(|| unreachable!("retained handoff"));
    assert_eq!(retained, handoff);

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("cancel")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--attempt")
        .arg(&identity)
        .arg("--fence")
        .arg(fence.to_string())
        .arg("--cause")
        .arg("operator-test")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stale = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("cancel")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--attempt")
        .arg(&identity)
        .arg("--fence")
        .arg(fence.to_string())
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(!stale.status.success());

    let usage_file = directory.path().join("usage.json");
    write(
        &usage_file,
        br#"{"tokens":{"status":"observed","tokens":42},"auxiliary_calls":{"status":"absent","reason":"not_instrumented"},"model_turns":{"status":"observed","turns":9}}"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("metrics")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--attempt")
        .arg(&identity)
        .arg("--usage-file")
        .arg(&usage_file)
        .arg("--record")
        .arg("true")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metrics: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        metrics.get("phase").and_then(serde_json::Value::as_str),
        Some("cancelled")
    );
    assert_eq!(metrics.get("open"), Some(&serde_json::Value::Bool(false)));
    assert_eq!(
        metrics
            .pointer("/usage/tokens/tokens")
            .and_then(serde_json::Value::as_u64),
        Some(42)
    );
    assert_eq!(
        metrics
            .pointer("/usage/auxiliary_calls/status")
            .and_then(serde_json::Value::as_str),
        Some("absent")
    );
    assert_eq!(
        metrics
            .pointer("/usage/model_turns/turns")
            .and_then(serde_json::Value::as_u64),
        Some(9)
    );
    let retained = Implementer::open(
        &database,
        Enrollment {
            repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        },
    )
    .unwrap_or_else(|_| unreachable!())
    .metrics(&ContentId::from_blake3_hex(identity.clone()).unwrap_or_else(|_| unreachable!()))
    .unwrap_or_else(|_| unreachable!())
    .unwrap_or_else(|| unreachable!("retained metrics"));
    assert_eq!(
        retained.elapsed_ms,
        metrics
            .get("elapsed_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_else(|| unreachable!())
    );

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("metrics-report")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--from-ms")
        .arg("0")
        .arg("--to-ms")
        .arg(u64::MAX.to_string())
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        report
            .pointer("/totals/finished")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/totals/failures/aborted_cancelled")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        report
            .pointer("/totals/turns/all/complete_total")
            .and_then(serde_json::Value::as_u64),
        Some(9)
    );
    assert_eq!(
        report.pointer("/totals/turns/per_verified_pass"),
        Some(&serde_json::Value::Null)
    );
    assert_eq!(
        report.pointer("/totals/turns/failed_share"),
        Some(&serde_json::json!({"numerator":9,"denominator":9}))
    );
    let invalid = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("metrics-report")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .arg("--from-ms")
        .arg("2")
        .arg("--to-ms")
        .arg("1")
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(!invalid.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("status")
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "{\"work\":1,\"attempts\":1,\"active\":0,\"verified\":0,\"published\":0,\"integrated\":0,\"source_reconciled\":0,\"failed\":0,\"blocked\":0,\"cancelled\":1,\"superseded\":0}"
    );
    let _ = ImplementationQueueConfig {
        path: queue_path,
        maximum_deliveries: 8,
    };
}

fn run_implementer(arguments: &[&str], database: &Path, enrollment: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .args(arguments)
        .arg("--local-operator")
        .arg("test-operator")
        .arg("--database")
        .arg(database)
        .arg("--enrollment")
        .arg(enrollment)
        .output()
        .unwrap_or_else(|_| unreachable!())
}

fn json_output(output: &std::process::Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| unreachable!())
}

// r[verify kiln.implementation.plan_review.scenario.blocked] r[verify kiln.implementation.plan_review.scenario.revision]
#[test]
fn plan_review_commands_gate_the_claim_and_return_feedback() {
    use kiln_core::implementation::{
        IMPLEMENTATION_WORK_SCHEMA, ImplementationPlan, ImplementationWork,
        PLAN_REVIEW_POLICY_SCHEMA, PLAN_SCHEMA, PlanAnnotation, PlanDecision, PlanNote, PlanRegion,
        PlanResolution, PlanReviewCommand, PlanReviewPolicy, PlanStep, PlanVerdict, SourceRef,
        WorkSourceKind,
    };
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let database = directory.path().join("work.sqlite");
    let enrollment_file = directory.path().join("enrollment.json");
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
        actors: BTreeSet::from(["octocat".to_string()]),
    };
    write(
        &enrollment_file,
        &serde_json::to_vec(&enrollment).unwrap_or_else(|_| unreachable!()),
    );
    let work = ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: 1,
        actor: "octocat".to_string(),
        repository: "octocat/hello-world".to_string(),
        project: "kiln".to_string(),
        base: None,
        title: "Review the plan first".to_string(),
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: content('a'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: kiln_core::types::ExternalRef::new("github", "issue", "octocat/hello-world#93")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "octocat/hello-world#93".to_string(),
            version: content('1'),
        }],
    };
    let identity = work.identity().unwrap_or_else(|_| unreachable!());
    Implementer::open(&database, enrollment)
        .and_then(|mut implementer| implementer.admit(&work, None))
        .unwrap_or_else(|error| unreachable!("{error}"));
    let apply = |name: &str, command: &PlanReviewCommand| {
        let path = directory.path().join(format!("{name}.json"));
        write(
            &path,
            &serde_json::to_vec(command).unwrap_or_else(|_| unreachable!()),
        );
        let path = path.to_str().unwrap_or_else(|| unreachable!());
        run_implementer(
            &["plan-review", "--command-file", path],
            &database,
            &enrollment_file,
        )
    };
    let claim = || run_implementer(&["claim"], &database, &enrollment_file);
    let feedback = || {
        json_output(&run_implementer(
            &["plan-feedback", "--work", identity.as_str()],
            &database,
            &enrollment_file,
        ))
    };
    let plan = |revision: u32, previous: Option<kiln_core::types::ContentId>, push: &str| {
        PlanReviewCommand::Propose {
            plan: ImplementationPlan {
                schema: PLAN_SCHEMA.to_string(),
                work_identity: identity.clone(),
                revision,
                previous,
                author: "agent".to_string(),
                steps: vec![
                    PlanStep {
                        id: "push".to_string(),
                        text: push.to_string(),
                    },
                    PlanStep {
                        id: "test".to_string(),
                        text: "Run the tests.".to_string(),
                    },
                ],
            },
        }
    };
    let pinned = |output: &std::process::Output| {
        json_output(output)
            .get("plan")
            .and_then(serde_json::Value::as_str)
            .and_then(|plan| kiln_core::types::ContentId::from_blake3_hex(plan).ok())
            .unwrap_or_else(|| unreachable!())
    };

    let configured = apply(
        "configure",
        &PlanReviewCommand::Configure {
            policy: Some(PlanReviewPolicy {
                schema: PLAN_REVIEW_POLICY_SCHEMA.to_string(),
                repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
                reviewers: BTreeSet::from(["reviewer".to_string()]),
            }),
        },
    );
    assert_eq!(
        json_output(&configured),
        serde_json::json!({"result": "configured"})
    );
    let waiting = claim();
    assert!(waiting.status.success());
    assert_eq!(String::from_utf8_lossy(&waiting.stdout).trim(), "null");

    let first = pinned(&apply("first", &plan(1, None, "Force-push the branch.")));
    let annotation = |reviewer: &str| PlanReviewCommand::Annotate {
        work: identity.clone(),
        annotation: PlanAnnotation {
            id: "no-force".to_string(),
            plan: first.clone(),
            reviewer: reviewer.to_string(),
            region: PlanRegion {
                step: "push".to_string(),
                span: None,
            },
            note: PlanNote::Replace {
                text: "Push only an absent branch.".to_string(),
            },
            blocking: true,
        },
    };
    let self_review = apply("self-review", &annotation("agent"));
    assert!(!self_review.status.success());
    assert!(String::from_utf8_lossy(&self_review.stderr).contains("implementation_plan_reviewer"));
    json_output(&apply("annotate", &annotation("reviewer")));
    let returned = feedback();
    assert_eq!(
        returned
            .pointer("/gate/gate")
            .and_then(serde_json::Value::as_str),
        Some("unresolved")
    );
    assert_eq!(
        returned
            .pointer("/items/0/quoted")
            .and_then(serde_json::Value::as_str),
        Some("Force-push the branch.")
    );

    let second = pinned(&apply(
        "second",
        &plan(2, Some(first.clone()), "Push only an absent branch."),
    ));
    json_output(&apply(
        "resolve",
        &PlanReviewCommand::Resolve {
            work: identity.clone(),
            resolution: PlanResolution {
                annotation: "no-force".to_string(),
                plan: second.clone(),
                reviewer: "reviewer".to_string(),
            },
        },
    ));
    json_output(&apply(
        "accept",
        &PlanReviewCommand::Decide {
            work: identity.clone(),
            decision: PlanDecision {
                plan: second,
                reviewer: "reviewer".to_string(),
                verdict: PlanVerdict::Accepted,
            },
        },
    ));
    let returned = feedback();
    assert_eq!(
        returned
            .pointer("/gate/gate")
            .and_then(serde_json::Value::as_str),
        Some("accepted")
    );
    assert_eq!(
        returned.pointer("/comparison/changed"),
        Some(&serde_json::json!(["push"]))
    );
    assert_eq!(
        returned.pointer("/items/0/resolved_in"),
        Some(&serde_json::json!(2))
    );
    let claimed = json_output(&claim());
    assert_eq!(
        claimed
            .get("work_identity")
            .and_then(serde_json::Value::as_str),
        Some(identity.as_str())
    );
}

// r[verify kiln.admission.host_owner.scenario.reserve] r[verify kiln.admission.operator_claim] r[verify kiln.admission.operator_claim.scenario.governed]
#[test]
fn host_owner_commands_admit_claim_and_reserve_the_sandbox_issue() {
    use kiln_core::implementation::{
        IMPLEMENTATION_WORK_SCHEMA, ImplementationWork, SourceRef, WorkSourceKind,
    };
    const PROFILE: &str = include_str!("../../../fixtures/fair-admission/host-owner.json");
    let profile: kiln_core::fair_admission::HostOwnerProfile =
        serde_json::from_str(PROFILE).unwrap_or_else(|_| unreachable!());
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let database = directory.path().join("work.sqlite");
    let enrollment_file = directory.path().join("enrollment.json");
    let profile_file = directory.path().join("host-owner.json");
    // A deadline relative to the wall clock the binary reads, never a fixed date.
    let mut profile = profile;
    profile.deadline_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| unreachable!())
        .as_millis()
        .try_into()
        .map(|now: u64| now + 86_400_000)
        .unwrap_or_else(|_| unreachable!());
    write(
        &profile_file,
        &serde_json::to_vec(&profile).unwrap_or_else(|_| unreachable!()),
    );
    let sandbox = "onixresearch/implementer-sandbox";
    let enrollment = Enrollment {
        repositories: BTreeSet::from([sandbox.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    write(
        &enrollment_file,
        &serde_json::to_vec(&enrollment).unwrap_or_else(|_| unreachable!()),
    );
    let work = ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: 1,
        actor: "operator".to_string(),
        repository: sandbox.to_string(),
        project: "implementer-sandbox".to_string(),
        base: None,
        title: "word count ignores whitespace runs".to_string(),
        requested_outcome: "fix".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["src".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: profile
            .repositories
            .first()
            .and_then(|entry| entry.classes.first())
            .unwrap_or_else(|| unreachable!())
            .verification_profile
            .clone(),
        budget: WorkBudget {
            attempts: 1,
            minutes_per_attempt: 20,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: kiln_core::types::ExternalRef::new(
                "github",
                "issue",
                "onixresearch/implementer-sandbox#1",
            )
            .unwrap_or_else(|_| unreachable!()),
            native_id: "onixresearch/implementer-sandbox#1".to_string(),
            version: content('1'),
        }],
    };
    let identity = work.identity().unwrap_or_else(|_| unreachable!());
    Implementer::open(&database, enrollment)
        .and_then(|mut implementer| implementer.admit(&work, None))
        .unwrap_or_else(|error| unreachable!("{error}"));
    let profile_path = profile_file.to_str().unwrap_or_else(|| unreachable!());
    let admitted = json_output(&run_implementer(
        &["host-owner-admit", "--profile", profile_path],
        &database,
        &enrollment_file,
    ));
    let text = |value: &serde_json::Value, pointer: &str| -> String {
        value
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| unreachable!("{pointer} missing in {value}"))
            .to_string()
    };
    assert_eq!(text(&admitted, "/queued/0/work"), identity.as_str());
    assert_eq!(text(&admitted, "/queued/0/class"), "bug");
    let observed = json_output(&run_implementer(
        &["host-owner-observe", "--profile", profile_path],
        &database,
        &enrollment_file,
    ));
    assert_eq!(text(&observed, "/selected"), identity.as_str());
    // A governed claim goes through fair admission and needs explicit authority.
    let unauthorized = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .args(["claim", "--database"])
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(!unauthorized.status.success());
    // The refused claim left no attempt behind.
    let status = json_output(&run_implementer(&["status"], &database, &enrollment_file));
    assert_eq!(status.get("attempts"), Some(&serde_json::json!(0)));
    let attempt = json_output(&run_implementer(&["claim"], &database, &enrollment_file));
    assert_eq!(text(&attempt, "/work_identity"), identity.as_str());
    let attempt_id = text(&attempt, "/identity");
    let fence = attempt
        .pointer("/fence")
        .unwrap_or_else(|| unreachable!())
        .to_string();
    let ledger = json_output(&run_implementer(
        &["admission-status"],
        &database,
        &enrollment_file,
    ));
    assert_eq!(
        text(
            &ledger,
            &format!("/items/{}/claim/attempt", identity.as_str())
        ),
        attempt_id
    );
    let reserve = [
        "host-owner-reserve",
        "--profile",
        profile_path,
        "--attempt",
        attempt_id.as_str(),
        "--fence",
        fence.as_str(),
    ];
    let committed = json_output(&run_implementer(&reserve, &database, &enrollment_file));
    assert_eq!(text(&committed, "/outcome"), "committed");
    assert_eq!(text(&committed, "/reservation/state"), "committed");
    // Host-owner work never starts through the out-of-band acknowledgement.
    let start = run_implementer(
        &[
            "start",
            "--attempt",
            attempt_id.as_str(),
            "--fence",
            fence.as_str(),
            "--workflow-kind",
            "workflow",
            "--workflow-id",
            "out-of-band",
        ],
        &database,
        &enrollment_file,
    );
    assert!(!start.status.success());
    assert!(
        String::from_utf8_lossy(&start.stderr).contains("implementation_transport_start_required")
    );
    let repeated = json_output(&run_implementer(&reserve, &database, &enrollment_file));
    assert_eq!(text(&repeated, "/outcome"), "retained");
    assert_eq!(
        text(&repeated, "/reservation/operation"),
        text(&committed, "/reservation/operation")
    );
    // The slot is held until the attempt is observed terminal.
    let release = run_implementer(
        &[
            "host-owner-release",
            "--profile",
            profile_path,
            "--attempt",
            attempt_id.as_str(),
            "--fence",
            fence.as_str(),
        ],
        &database,
        &enrollment_file,
    );
    assert!(!release.status.success());
    assert!(String::from_utf8_lossy(&release.stderr).contains("host_owner_release_unobserved"));
}

fn policy_check(policy: &Path, enrollment: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("policy-check")
        .arg("--policy")
        .arg(policy)
        .arg("--enrollment")
        .arg(enrollment)
        .output()
        .unwrap_or_else(|_| unreachable!())
}

// r[verify kiln.implementation.policy_check.scenario.missing_field] r[verify kiln.implementation.policy_check.scenario.admitted]
#[test]
fn policy_check_reports_the_field_poll_would_reject() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let policy_file = directory.path().join("github-policy.json");
    let enrollment_file = directory.path().join("enrollment.json");
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    write(
        &enrollment_file,
        &serde_json::to_vec(&enrollment).unwrap_or_else(|_| unreachable!()),
    );

    write(
        &policy_file,
        &serde_json::to_vec_pretty(&policy()).unwrap_or_else(|_| unreachable!()),
    );
    let admitted = policy_check(&policy_file, &enrollment_file);
    assert!(
        admitted.status.success(),
        "{}",
        String::from_utf8_lossy(&admitted.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&admitted.stdout)
            .unwrap_or_else(|_| unreachable!()),
        serde_json::json!({"policy": "admitted", "repository": "octocat/hello-world"})
    );

    // The #49 pilot shape: a base reference without its contract version.
    let mut document = serde_json::to_value(policy()).unwrap_or_else(|_| unreachable!());
    let base = document
        .get_mut("base")
        .and_then(serde_json::Value::as_object_mut)
        .unwrap_or_else(|| unreachable!());
    assert!(base.remove("contract_version").is_some());
    write(
        &policy_file,
        &serde_json::to_vec_pretty(&document).unwrap_or_else(|_| unreachable!()),
    );
    let refused = policy_check(&policy_file, &enrollment_file);
    assert_eq!(refused.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("github-policy.json does not match its schema: missing field `contract_version` at line "),
        "{stderr}"
    );
    assert!(stderr.contains(" column "), "{stderr}");
    assert!(refused.stdout.is_empty());
}
