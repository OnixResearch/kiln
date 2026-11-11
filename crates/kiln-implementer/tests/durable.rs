use kiln_core::implementation::{
    AcceptanceObligation, Enrollment, FailureClass, IMPLEMENTATION_WORK_SCHEMA, ImplementationWork,
    SourceMapping, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::Implementer;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

const LEASE: u64 = 60_000;

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from(["onix/kiln".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    }
}

fn work() -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: 1,
        actor: "operator".to_string(),
        repository: "onix/kiln".to_string(),
        project: "kiln".to_string(),
        base: None,
        title: "Durable implementation work".to_string(),
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: content('9'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "onix/kiln#45")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "onix/kiln#45".to_string(),
            version: content('1'),
        }],
    }
}

fn reference(kind: &str, id: &str) -> ExternalRef {
    ExternalRef::new("lattice", kind, id).unwrap_or_else(|_| unreachable!())
}

fn open(path: &Path) -> Implementer {
    Implementer::open(path, enrollment()).unwrap_or_else(|error| unreachable!("{error}"))
}

// r[verify kiln.implementation.durable_store.scenario.restart]
#[test]
fn duplicate_deliveries_persist_as_one_work_version() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("implementation.db");
    let work = work();
    let issue = work
        .aliases
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    let mapping = SourceMapping {
        issue,
        tasks: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", "change/task")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "change/task".to_string(),
            version: content('2'),
        }],
    };
    let linked = {
        let mut linked = work.clone();
        linked.aliases.extend(mapping.tasks.clone());
        linked
    };
    let mut implementer = open(path.as_path());
    assert!(implementer.admit(&linked, Some(&mapping)).is_ok());
    assert!(matches!(
        implementer.admit(&linked, Some(&mapping)),
        Ok(kiln_core::implementation::AdmissionDecision::Duplicate { .. })
    ));
    drop(implementer);
    let mut reopened = open(path.as_path());
    assert_eq!(
        reopened
            .work()
            .unwrap_or_else(|_| unreachable!())
            .current()
            .count(),
        1
    );
    assert!(matches!(
        reopened.admit(&linked, Some(&mapping)),
        Ok(kiln_core::implementation::AdmissionDecision::Duplicate { .. })
    ));
}

// r[verify kiln.implementation.durable_store.scenario.concurrent]
#[test]
fn concurrent_schedulers_obtain_one_claim() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = Arc::new(directory.path().join("implementation.db"));
    let work = work();
    let mut first = open(path.as_path());
    first.admit(&work, None).unwrap_or_else(|_| unreachable!());
    drop(first);
    let outcomes: Vec<bool> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let path = Arc::clone(&path);
                scope.spawn(move || {
                    let mut implementer = open(path.as_path());
                    implementer
                        .claim_next(0, LEASE)
                        .unwrap_or_else(|_| unreachable!())
                        .is_some()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or_else(|_| unreachable!()))
            .collect()
    });
    assert_eq!(outcomes.iter().filter(|claimed| **claimed).count(), 1);
}

// r[verify kiln.implementation.recovery.scenario.restart]
#[test]
fn stale_fence_is_refused_after_restart_and_replacement() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("implementation.db");
    let work = work();
    let mut implementer = open(path.as_path());
    implementer
        .admit(&work, None)
        .unwrap_or_else(|_| unreachable!());
    let first = implementer
        .claim_next(0, 100)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("first claim"));
    let recovery = implementer.recover(100).unwrap_or_else(|_| unreachable!());
    assert_eq!(recovery.expired, vec![first.identity.clone()]);
    let replacement = implementer
        .claim_next(100, LEASE)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("replacement claim"));
    assert!(replacement.fence > first.fence);
    drop(implementer);

    let mut reopened = open(path.as_path());
    assert!(
        reopened
            .produce_candidate(&first.identity, first.fence, 200, &reference("run", "old"))
            .is_err()
    );
    reopened
        .start_execution(
            &replacement.identity,
            replacement.fence,
            200,
            &reference("run", "new"),
        )
        .unwrap_or_else(|_| unreachable!());
    assert!(
        reopened
            .produce_candidate(
                &replacement.identity,
                replacement.fence,
                200,
                &reference("candidate", "new")
            )
            .is_ok()
    );
    let stored = reopened
        .attempt(&replacement.identity)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        stored.phase,
        kiln_core::implementation::AttemptPhase::CandidateProduced
    );
}

// r[verify kiln.implementation.recovery.scenario.restart]
#[test]
fn dispatch_reconciliation_survives_restart_without_duplicate_execution() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("implementation.db");
    let work = work();
    let mut implementer = open(path.as_path());
    implementer
        .admit(&work, None)
        .unwrap_or_else(|_| unreachable!());
    let attempt = implementer
        .claim_next(0, LEASE)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("claim"));
    drop(implementer);

    let run = reference("run", "one");
    let mut reopened = open(path.as_path());
    reopened
        .reconcile_dispatch(&attempt.identity, attempt.fence, 1, &run)
        .unwrap_or_else(|_| unreachable!());
    drop(reopened);
    let mut again = open(path.as_path());
    again
        .reconcile_dispatch(&attempt.identity, attempt.fence, 2, &run)
        .unwrap_or_else(|_| unreachable!());
    assert!(
        again
            .reconcile_dispatch(
                &attempt.identity,
                attempt.fence,
                2,
                &reference("run", "two")
            )
            .is_err()
    );
    let stored = again
        .attempt(&attempt.identity)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(stored.workflow.as_ref(), Some(&run));
}

#[test]
fn consumed_budget_survives_restart() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("implementation.db");
    let work = work();
    let mut implementer = open(path.as_path());
    implementer
        .admit(&work, None)
        .unwrap_or_else(|_| unreachable!());
    for now in [0, 1] {
        let attempt = implementer
            .claim_next(now, LEASE)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!("claim"));
        implementer
            .fail(
                &attempt.identity,
                attempt.fence,
                now,
                FailureClass::Implementation,
                "tests failed",
            )
            .unwrap_or_else(|_| unreachable!());
    }
    drop(implementer);
    let mut reopened = open(path.as_path());
    assert_eq!(
        reopened
            .claim_next(2, LEASE)
            .unwrap_or_else(|_| unreachable!()),
        None
    );
    assert_eq!(
        reopened
            .work()
            .unwrap_or_else(|_| unreachable!())
            .current()
            .count(),
        1
    );
}
