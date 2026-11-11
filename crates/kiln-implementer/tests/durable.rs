use kiln_core::implementation::{
    AcceptanceObligation, AttemptUsage, Enrollment, FailureClass, IMPLEMENTATION_WORK_SCHEMA,
    ImplementationWork, SourceMapping, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
};
use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
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

fn provider() -> ProviderProfileId {
    ProviderProfileId::new("lattice-implementation").unwrap_or_else(|_| unreachable!())
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
        .reserve_dispatch(
            &replacement.identity,
            replacement.fence,
            200,
            &content('d'),
            &provider(),
        )
        .unwrap_or_else(|_| unreachable!());
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

// r[verify kiln.implementation.dispatch_reservation.scenario.restart]
#[test]
fn dispatch_reservation_survives_restart_before_transport() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("implementation.db");
    let mut implementer = open(path.as_path());
    implementer
        .admit(&work(), None)
        .unwrap_or_else(|_| unreachable!());
    let attempt = implementer
        .claim_next(0, LEASE)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("claim"));
    let reserved = implementer
        .reserve_dispatch(
            &attempt.identity,
            attempt.fence,
            1,
            &content('d'),
            &provider(),
        )
        .unwrap_or_else(|_| unreachable!());
    drop(implementer);

    let mut reopened = open(path.as_path());
    assert_eq!(
        reopened
            .dispatch_reservation(&attempt.identity)
            .unwrap_or_else(|_| unreachable!()),
        Some(reserved.clone())
    );
    assert!(
        reopened
            .start_execution(
                &attempt.identity,
                attempt.fence,
                2,
                &reference("run", "after-restart")
            )
            .is_ok()
    );
}

// r[verify kiln.implementation.metrics.scenario.retained]
#[test]
fn metrics_receipt_survives_restart_without_being_rewritten() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("implementation.db");
    let mut implementer = open(path.as_path());
    implementer
        .admit(&work(), None)
        .unwrap_or_else(|_| unreachable!());
    let attempt = implementer
        .claim_next(10, LEASE)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("claim"));
    implementer
        .cancel(&attempt.identity, attempt.fence, 20, "operator cancelled")
        .unwrap_or_else(|_| unreachable!());
    let recorded = implementer
        .record_metrics(
            &attempt.identity,
            attempt.fence,
            30,
            AttemptUsage::default(),
            Vec::new(),
        )
        .unwrap_or_else(|_| unreachable!());
    drop(implementer);

    let mut reopened = open(path.as_path());
    assert_eq!(
        reopened
            .metrics(&attempt.identity)
            .unwrap_or_else(|_| unreachable!()),
        Some(recorded.clone())
    );
    assert_eq!(
        reopened
            .record_metrics(
                &attempt.identity,
                attempt.fence,
                40,
                AttemptUsage::default(),
                Vec::new(),
            )
            .unwrap_or_else(|_| unreachable!()),
        recorded
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
    implementer
        .reserve_dispatch(
            &attempt.identity,
            attempt.fence,
            1,
            &content('d'),
            &provider(),
        )
        .unwrap_or_else(|_| unreachable!());
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

// r[verify kiln.implementation.integration] r[verify kiln.implementation.source_status.scenario.close]
#[test]
fn verified_candidate_reaches_integrated_and_source_reconciled() {
    use kiln_core::change_review::{CiCheck, CiFact};
    use kiln_core::implementation::{
        CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence, CandidateHandoff, CandidateVerification,
        EvidenceKind, EvidenceOutcome, INTEGRATION_SCHEMA, IntegrationReceipt, PUBLICATION_SCHEMA,
        PublicationPhase, PublicationPlan, SOURCE_STATUS_SCHEMA, SourceAction, SourceStatusPlan,
        SourceStatusReceipt, VERIFICATION_SCHEMA, admit_candidate_handoff,
        admit_candidate_verification, branch_for,
    };

    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("implementation.db");
    let mut work = work();
    work.base = Some(reference("commit", "base"));
    let mut implementer = open(path.as_path());
    implementer
        .admit(&work, None)
        .unwrap_or_else(|_| unreachable!());
    let attempt = implementer
        .claim_next(0, LEASE)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("claim"));
    implementer
        .reserve_dispatch(
            &attempt.identity,
            attempt.fence,
            1,
            &content('d'),
            &provider(),
        )
        .unwrap_or_else(|_| unreachable!());
    implementer
        .start_execution(
            &attempt.identity,
            attempt.fence,
            2,
            &reference("run", "one"),
        )
        .unwrap_or_else(|_| unreachable!());
    let handoff = CandidateHandoff {
        schema: CANDIDATE_HANDOFF_SCHEMA.to_string(),
        work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        candidate: reference("commit", "candidate"),
        tree: reference("tree", "tree"),
        changed_paths: vec!["crates/src/lib.rs".to_string()],
        acceptance_digest: kiln_core::implementation::acceptance_digest(&work)
            .unwrap_or_else(|_| unreachable!()),
        verification_profile: work.verification_profile.clone(),
        toolchain: content('7'),
        environment: content('8'),
        evidence: vec![CandidateEvidence {
            identity: content('e'),
            kind: EvidenceKind::Check,
            outcome: EvidenceOutcome::Passed,
            reference: reference("run", "candidate-check"),
        }],
    };
    implementer
        .admit_candidate_handoff(&work, &attempt.identity, attempt.fence, 3, &handoff)
        .unwrap_or_else(|_| unreachable!());
    implementer
        .begin_verification(&attempt.identity, attempt.fence, 4)
        .unwrap_or_else(|_| unreachable!());
    let handoff_identity =
        admit_candidate_handoff(&work, &attempt, &handoff).unwrap_or_else(|_| unreachable!());
    let verification = CandidateVerification {
        schema: VERIFICATION_SCHEMA.to_string(),
        handoff: handoff_identity.clone(),
        work_identity: handoff.work_identity.clone(),
        work_revision: handoff.work_revision,
        attempt: handoff.attempt_identity.clone(),
        fence: handoff.fence,
        candidate: handoff.candidate.clone(),
        tree: handoff.tree.clone(),
        base: handoff.base.clone(),
        verification_profile: work.verification_profile.clone(),
        run: CiFact {
            project: work.project.clone(),
            repository: work.repository.clone(),
            revision: handoff.candidate.native_id.as_str().to_string(),
            run_id: "run-1".to_string(),
            ordinal: 1,
            success: true,
            full_run: true,
            clean: true,
            scope_identity: None,
            terminal: true,
            untrusted: None,
            checks: vec![CiCheck {
                name: "test".to_string(),
                kind: "test".to_string(),
                outcome: "pass".to_string(),
                artifact_digest: Some("f".repeat(64)),
            }],
        },
    };
    implementer
        .record_verification(
            &work,
            &attempt.identity,
            attempt.fence,
            5,
            &handoff,
            &verification,
        )
        .unwrap_or_else(|_| unreachable!());
    let verification_identity =
        admit_candidate_verification(&work, &attempt, &handoff, &verification)
            .unwrap_or_else(|_| unreachable!());
    let publication = PublicationPlan {
        schema: PUBLICATION_SCHEMA.to_string(),
        work_identity: handoff.work_identity.clone(),
        work_revision: handoff.work_revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        handoff: handoff_identity.clone(),
        verification: verification_identity.clone(),
        repository: work.repository.clone(),
        base: work.base.clone().unwrap_or_else(|| unreachable!()),
        base_branch: "main".to_string(),
        candidate: handoff.candidate.clone(),
        tree: handoff.tree.clone(),
        branch: branch_for(&handoff.work_identity, &attempt.identity),
        title: "Implement".to_string(),
        body: "Verified".to_string(),
    };
    implementer
        .prepare_publication(
            &work,
            &attempt.identity,
            attempt.fence,
            &handoff,
            &verification,
            &publication,
        )
        .unwrap_or_else(|_| unreachable!());
    implementer
        .record_branch_published(&attempt.identity, attempt.fence, &handoff.candidate)
        .unwrap_or_else(|_| unreachable!());
    let pull_request = ExternalRef::new("github", "pull_request", "onix/kiln#9")
        .unwrap_or_else(|_| unreachable!());
    implementer
        .record_pull_request_published(
            &attempt.identity,
            attempt.fence,
            &pull_request,
            &handoff.candidate,
        )
        .unwrap_or_else(|_| unreachable!());
    let merge_commit =
        ExternalRef::new("git", "commit", &"c".repeat(40)).unwrap_or_else(|_| unreachable!());
    let integration = IntegrationReceipt {
        schema: INTEGRATION_SCHEMA.to_string(),
        work_identity: handoff.work_identity.clone(),
        work_revision: handoff.work_revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        handoff: handoff_identity,
        verification: verification_identity,
        candidate: handoff.candidate.clone(),
        pull_request: pull_request.clone(),
        merge_commit: merge_commit.clone(),
        base_ref: publication.base_branch.clone(),
        base_head: merge_commit,
    };
    let integration_identity = implementer
        .record_integration(&work, &attempt.identity, attempt.fence, &integration)
        .unwrap_or_else(|_| unreachable!());
    let source = work
        .aliases
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    let source_plan = SourceStatusPlan {
        schema: SOURCE_STATUS_SCHEMA.to_string(),
        work_identity: handoff.work_identity.clone(),
        work_revision: handoff.work_revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        integration: integration_identity,
        source: source.clone(),
        action: SourceAction::Close,
    };
    let source_receipt = SourceStatusReceipt {
        schema: SOURCE_STATUS_SCHEMA.to_string(),
        work_identity: handoff.work_identity,
        work_revision: handoff.work_revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        integration: source_plan.integration.clone(),
        source,
        state: "closed".to_string(),
    };
    let source_identity = implementer
        .record_source_status(
            &work,
            &attempt.identity,
            attempt.fence,
            &source_plan,
            &source_receipt,
        )
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        implementer
            .record_source_status(
                &work,
                &attempt.identity,
                attempt.fence,
                &source_plan,
                &source_receipt,
            )
            .unwrap_or_else(|_| unreachable!()),
        source_identity
    );
    let stored = implementer
        .publication_record(&attempt.identity)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("publication"));
    assert_eq!(stored.phase, PublicationPhase::SourceReconciled);
}
