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

// r[verify kiln.implementation.operator_cli.scenario.drain] r[verify kiln.implementation.operator_cli.scenario.status]
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
    let _ = ImplementationQueueConfig {
        path: queue_path,
        maximum_deliveries: 8,
    };
}
