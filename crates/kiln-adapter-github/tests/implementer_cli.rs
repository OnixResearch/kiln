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
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "{\"work\":1,\"attempts\":0,\"active\":0}"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("run")
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
        base: None,
        workspace: kiln_core::implementation::DispatchWorkspace {
            identity: content('d'),
            isolation: kiln_core::implementation::WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
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

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("cancel")
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

    let output = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"))
        .arg("status")
        .arg("--database")
        .arg(&database)
        .arg("--enrollment")
        .arg(&enrollment_file)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "{\"work\":1,\"attempts\":1,\"active\":0}"
    );
    let _ = ImplementationQueueConfig {
        path: queue_path,
        maximum_deliveries: 8,
    };
}
