use kiln_adapter_cairn::implementation::CairnImplementationPolicy;
use kiln_core::implementation::{AcceptanceObligation, Enrollment, TerminalPolicy, WorkBudget};
use kiln_core::types::ContentId;
use kiln_implementer::Implementer;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|_| unreachable!());
}

#[test]
fn intake_binary_admits_one_cairn_unit_and_deduplicates_replay() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let database = directory.path().join("work.sqlite");
    let enrollment_file = directory.path().join("enrollment.json");
    let policy_file = directory.path().join("policy.json");
    let snapshot_file = directory.path().join("snapshot.json");
    let digest = "a".repeat(64);
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["onix/cairn".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    write(
        &enrollment_file,
        &serde_json::to_vec(&enrollment).unwrap_or_else(|_| unreachable!()),
    );
    let policy = CairnImplementationPolicy {
        repository: "onix/cairn".to_string(),
        project: "cairn".to_string(),
        actor: "operator".to_string(),
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        permitted_paths: vec!["crates/cairn-cli".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: ContentId::from_blake3_hex("b".repeat(64))
            .unwrap_or_else(|_| unreachable!()),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
    };
    write(
        &policy_file,
        &serde_json::to_vec(&policy).unwrap_or_else(|_| unreachable!()),
    );
    let snapshot = json!({
        "schema": "cairn.implementation-ready.v1",
        "outcome": "ready",
        "complete": true,
        "selection": [{
            "change": "demo",
            "unit": "task:build",
            "requirement_references": ["demo.behavior"],
            "snapshot_digest": digest,
            "archive_ready": false
        }],
        "changes": [{
            "name": "demo",
            "path": ".cairn/changes/demo",
            "archive_ready": false,
            "implementation_ready": true,
            "completion_only": false,
            "blockers": [],
            "units": [{"unit": "task:build", "requirement_references": ["demo.behavior"]}],
            "gates": {
                "proposal": {"valid": true},
                "design": {"valid": true},
                "tasks": {"valid": true}
            },
            "snapshot_digest": digest
        }]
    });
    write(
        &snapshot_file,
        &serde_json::to_vec(&snapshot).unwrap_or_else(|_| unreachable!()),
    );

    for (sequence, expected) in [
        (
            7,
            "{\"outcome\":\"ready\",\"selected\":1,\"admitted\":1,\"duplicate\":0,\"superseded\":0}",
        ),
        (
            8,
            "{\"outcome\":\"ready\",\"selected\":1,\"admitted\":0,\"duplicate\":0,\"superseded\":1}",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_kiln-cairn-intake"))
            .arg("--database")
            .arg(&database)
            .arg("--enrollment")
            .arg(&enrollment_file)
            .arg("--snapshot")
            .arg(&snapshot_file)
            .arg("--policy")
            .arg(&policy_file)
            .arg("--sequence")
            .arg(sequence.to_string())
            .output()
            .unwrap_or_else(|_| unreachable!());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
    let mut implementer =
        Implementer::open(&database, enrollment).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        implementer
            .work()
            .unwrap_or_else(|_| unreachable!())
            .current()
            .count(),
        1
    );
}
