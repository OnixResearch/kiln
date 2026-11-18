use kiln_core::implementation::{
    AbsenceReason, AcceptanceObligation, AttemptUsage, Enrollment, FailureClass,
    IMPLEMENTATION_WORK_SCHEMA, ImplementationWork, ModelTurnObservation, SourceMapping, SourceRef,
    TerminalPolicy, VerificationOutcome, WorkBudget, WorkSourceKind, WorkerObservation,
    WorkerTerminalState,
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

// r[verify kiln.implementation.durable_store.scenario.concurrent]
#[test]
fn concurrent_schedulers_claim_independent_work() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = Arc::new(directory.path().join("implementation.db"));
    let first = work();
    let mut second = work();
    second.title = "Second durable implementation work".to_string();
    second.priority = 2;
    let alias = second.aliases.first_mut().unwrap_or_else(|| unreachable!());
    alias.source =
        ExternalRef::new("github", "issue", "onix/kiln#46").unwrap_or_else(|_| unreachable!());
    alias.native_id = "onix/kiln#46".to_string();
    alias.version = content('2');
    let mut implementer = open(path.as_path());
    implementer
        .admit(&first, None)
        .unwrap_or_else(|_| unreachable!());
    implementer
        .admit(&second, None)
        .unwrap_or_else(|_| unreachable!());
    drop(implementer);
    let outcomes: Vec<Option<ContentId>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let path = Arc::clone(&path);
                scope.spawn(move || {
                    let mut implementer = open(path.as_path());
                    implementer
                        .claim_next(0, LEASE)
                        .unwrap_or_else(|_| unreachable!())
                        .map(|attempt| attempt.work_identity)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or_else(|_| unreachable!()))
            .collect()
    });
    assert!(matches!(
        outcomes.as_slice(),
        [Some(first), Some(second)] if first != second
    ));
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

#[test]
// r[verify kiln.implementation.outcome_metrics.scenario.binding]
fn retained_observations_survive_restart_without_rewriting_first_metrics()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("implementation.db");
    let mut store = open(&path);
    store.admit(&work(), None)?;
    let attempt = store.claim_next(10, LEASE)?.ok_or("missing claim")?;
    let first = store.record_metrics(
        &attempt.identity,
        attempt.fence,
        11,
        AttemptUsage::default(),
        vec![],
    )?;
    let worker = WorkerObservation {
        terminal: WorkerTerminalState::Completed,
        worker_profile: Some(content('a')),
        harness: Some("harness".into()),
        model: Some("model".into()),
        usage: AttemptUsage {
            model_turns: ModelTurnObservation::Incomplete {
                turns: 3,
                reason: AbsenceReason::ProviderUnavailable,
            },
            ..AttemptUsage::default()
        },
    };
    store.record_worker_observation(&attempt.identity, attempt.fence, &worker)?;
    store.record_verification_outcome(
        &attempt.identity,
        attempt.fence,
        VerificationOutcome::Unknown,
    )?;
    drop(store);

    let mut store = open(&path);
    store.record_worker_observation(&attempt.identity, attempt.fence, &worker)?;
    assert!(
        store
            .record_worker_observation(&attempt.identity, attempt.fence + 1, &worker)
            .is_err()
    );
    let mut conflict = worker.clone();
    conflict.usage.model_turns = ModelTurnObservation::Observed { turns: 4 };
    assert!(
        store
            .record_worker_observation(&attempt.identity, attempt.fence, &conflict)
            .is_err()
    );
    assert!(
        store
            .record_verification_outcome(
                &attempt.identity,
                attempt.fence,
                VerificationOutcome::Verified
            )
            .is_err()
    );
    store.record_verification_outcome(
        &attempt.identity,
        attempt.fence,
        VerificationOutcome::Rejected,
    )?;
    assert!(
        store
            .record_verification_outcome(
                &attempt.identity,
                attempt.fence,
                VerificationOutcome::Incomplete
            )
            .is_err()
    );
    let retained = store
        .attempt(&attempt.identity)?
        .ok_or("missing retained attempt")?;
    assert_eq!(retained.worker_observation.as_ref(), Some(&worker));
    assert_eq!(
        retained.verification_outcome,
        Some(VerificationOutcome::Rejected)
    );
    assert_eq!(
        retained.phase,
        kiln_core::implementation::AttemptPhase::Claimed
    );
    assert_eq!(retained.ended_at_ms, None);
    assert_eq!(retained.verification, None);
    assert_eq!(
        store.record_metrics(
            &attempt.identity,
            attempt.fence,
            20,
            worker.usage.clone(),
            vec![]
        )?,
        first
    );
    let fresh = store.observe_metrics(
        &attempt.identity,
        attempt.fence,
        20,
        AttemptUsage::default(),
        vec![],
    )?;
    assert_eq!(fresh.usage, worker.usage);
    assert_eq!(
        fresh.verification_outcome,
        Some(VerificationOutcome::Rejected)
    );
    assert!(fresh.open);
    assert!(
        store
            .authorize_current(&attempt.identity, attempt.fence, 20)
            .is_ok()
    );
    store.cancel(&attempt.identity, attempt.fence, 21, "cancel")?;
    let next = store.claim_next(22, LEASE)?.ok_or("missing replacement")?;
    drop(store);

    let mut store = open(&path);
    store.record_worker_observation(&attempt.identity, attempt.fence, &worker)?;
    assert!(
        store
            .authorize_current(&attempt.identity, attempt.fence, 23)
            .is_err()
    );
    let replacement = store
        .attempt(&next.identity)?
        .ok_or("missing replacement")?;
    assert_eq!(replacement.worker_observation, None);
    assert_eq!(replacement.verification_outcome, None);
    assert_eq!(replacement.metrics, None);
    Ok(())
}

#[test]
fn metrics_report_never_writes_state_versions_or_legacy_rows()
-> Result<(), Box<dyn std::error::Error>> {
    fn rows(path: &Path) -> rusqlite::Result<Vec<(String, i64, Vec<u8>)>> {
        let connection = rusqlite::Connection::open(path)?;
        let mut statement = connection
            .prepare("SELECT kind, version, bytes FROM implementation_state ORDER BY kind")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect()
    }
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("implementation.db");
    let mut store = open(&path);
    let empty = rows(&path)?;
    store.metrics_report(0, 100)?;
    assert_eq!(rows(&path)?, empty, "reports must not create state rows");
    store.admit(&work(), None)?;
    let attempt = store.claim_next(10, LEASE)?.ok_or("missing claim")?;
    store.record_metrics(
        &attempt.identity,
        attempt.fence,
        20,
        AttemptUsage::default(),
        vec![],
    )?;
    drop(store);
    let connection = rusqlite::Connection::open(&path)?;
    let bytes: Vec<u8> = connection.query_row(
        "SELECT bytes FROM implementation_state WHERE kind = 'attempts'",
        [],
        |row| row.get(0),
    )?;
    let mut legacy: serde_json::Value = serde_json::from_slice(&bytes)?;
    let row = legacy
        .pointer_mut(&format!("/attempts/{}", attempt.identity.as_str()))
        .ok_or("missing legacy attempt")?;
    row.as_object_mut()
        .ok_or("missing legacy attempt")?
        .remove("worker_observation");
    row.as_object_mut()
        .ok_or("missing legacy attempt")?
        .remove("verification_outcome");
    row.pointer_mut("/metrics/usage")
        .ok_or("missing legacy metrics usage")?
        .as_object_mut()
        .ok_or("missing legacy metrics usage")?
        .remove("model_turns");
    row.get_mut("metrics")
        .ok_or("missing legacy metrics")?
        .as_object_mut()
        .ok_or("missing legacy metrics")?
        .remove("worker_terminal");
    row.get_mut("metrics")
        .ok_or("missing legacy metrics")?
        .as_object_mut()
        .ok_or("missing legacy metrics")?
        .remove("verification_outcome");
    connection.execute(
        "UPDATE implementation_state SET bytes = ?1 WHERE kind = 'attempts'",
        [serde_json::to_vec(&legacy)?],
    )?;
    drop(connection);
    let before = rows(&path)?;
    let mut store = open(&path);
    store.metrics_report(0, 100)?;
    assert_eq!(rows(&path)?, before);
    assert!(store.metrics_report(100, 0).is_err());
    assert_eq!(rows(&path)?, before, "invalid windows cannot write either");
    drop(store);
    let mut store = open(&path);
    let restored = store
        .attempt(&attempt.identity)?
        .ok_or("missing legacy attempt")?;
    assert_eq!(restored.worker_observation, None);
    assert_eq!(restored.verification_outcome, None);
    assert_eq!(
        restored
            .metrics
            .ok_or("missing legacy metrics")?
            .usage
            .model_turns,
        ModelTurnObservation::default()
    );
    Ok(())
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
    work.terminal_policy = TerminalPolicy::VerifiedIntegrated;
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
        mapping: kiln_core::implementation::IntegrationMapping {
            expected_base: publication.base.clone(),
            candidate_tree: publication.tree.clone(),
            landed_tree: publication.tree.clone(),
        },
    };
    let integration_identity = implementer
        .record_integration(&work, &attempt.identity, attempt.fence, &integration)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        implementer
            .record_integration(&work, &attempt.identity, attempt.fence, &integration)
            .unwrap_or_else(|_| unreachable!()),
        integration_identity
    );
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
        expected_observed_revision: work.revision,
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
        observed_revision: work.revision,
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
    assert!(
        implementer
            .claim_next(11, LEASE)
            .unwrap_or_else(|_| unreachable!())
            .is_none(),
        "verified work must not be reclaimed"
    );
}

// Local ledger receipts exercise callback ordering, not native producer qualification.
fn racing_callbacks(
    path: &Path,
) -> Result<
    (
        ImplementationWork,
        kiln_core::implementation::ImplementationAttempt,
        kiln_core::implementation::CandidateHandoff,
        kiln_core::implementation::CandidateVerification,
    ),
    Box<dyn std::error::Error>,
> {
    use kiln_core::change_review::{CiCheck, CiFact};
    use kiln_core::implementation::{
        CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence, CandidateHandoff, CandidateVerification,
        DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace, EvidenceKind, EvidenceOutcome,
        ImplementationDispatchPlan, VERIFICATION_SCHEMA, WorkspaceIsolation,
        admit_candidate_handoff,
    };

    let mut work = work();
    work.base = Some(ExternalRef::new("git", "commit", &"a".repeat(40)).map_err(|e| e.message)?);
    let mut store = open(path);
    store.admit(&work, None)?;
    let attempt = store.claim_next(0, LEASE)?.ok_or("missing attempt")?;
    let dispatch = ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: attempt.work_identity.clone(),
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        workspace: DispatchWorkspace {
            identity: content('e'),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
        },
        provider_profile: provider(),
        permitted_paths: work.permitted_paths.clone(),
        permitted_effects: work.permitted_effects.clone(),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 1,
        token_budget: 1000,
        invocation_budget: 1,
        expected_output: content('f'),
        execution: None,
    };
    let plan = store
        .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &dispatch)?
        .plan;
    let binding =
        store.bind_campaign_request(&work, &attempt.identity, attempt.fence, 1, b"race request")?;
    assert_eq!(binding.plan, plan);
    assert!(!store.begin_dispatch_transport(&attempt.identity, attempt.fence, 2, &plan)?);
    store.start_execution(
        &attempt.identity,
        attempt.fence,
        2,
        &reference("run", "racing-worker"),
    )?;
    let handoff = CandidateHandoff {
        schema: CANDIDATE_HANDOFF_SCHEMA.into(),
        work_identity: attempt.work_identity.clone(),
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        candidate: reference("commit", "racing-candidate"),
        tree: reference("tree", "racing-tree"),
        changed_paths: vec!["crates/src/lib.rs".into()],
        acceptance_digest: kiln_core::implementation::acceptance_digest(&work)
            .map_err(|e| e.message)?,
        verification_profile: work.verification_profile.clone(),
        toolchain: content('7'),
        environment: content('8'),
        evidence: vec![CandidateEvidence {
            identity: content('e'),
            kind: EvidenceKind::Check,
            outcome: EvidenceOutcome::Passed,
            reference: reference("run", "racing-check"),
        }],
    };
    store.admit_candidate_handoff(&work, &attempt.identity, attempt.fence, 3, &handoff)?;
    store.begin_verification(&attempt.identity, attempt.fence, 4)?;
    let verification = CandidateVerification {
        schema: VERIFICATION_SCHEMA.into(),
        handoff: admit_candidate_handoff(&work, &attempt, &handoff).map_err(|e| e.message)?,
        work_identity: handoff.work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        candidate: handoff.candidate.clone(),
        tree: handoff.tree.clone(),
        base: handoff.base.clone(),
        verification_profile: work.verification_profile.clone(),
        run: CiFact {
            project: work.project.clone(),
            repository: work.repository.clone(),
            revision: handoff.candidate.native_id.as_str().to_owned(),
            run_id: "racing-verification".into(),
            ordinal: 1,
            success: true,
            full_run: true,
            clean: true,
            scope_identity: None,
            terminal: true,
            untrusted: None,
            checks: vec![CiCheck {
                name: "test".into(),
                kind: "test".into(),
                outcome: "pass".into(),
                artifact_digest: Some("f".repeat(64)),
            }],
        },
    };
    Ok((work, attempt, handoff, verification))
}

// r[verify kiln.implementation.current_authority.scenario.cancelled]
#[test]
fn completion_before_cancel_retains_exact_evidence_across_restart()
-> Result<(), Box<dyn std::error::Error>> {
    use kiln_core::implementation::AttemptPhase;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("implementation.db");
    let (work, attempt, handoff, verification) = racing_callbacks(&path)?;
    let mut store = open(&path);
    store.record_verification_outcome(
        &attempt.identity,
        attempt.fence,
        VerificationOutcome::Rejected,
    )?;
    let receipt = store.record_verification(
        &work,
        &attempt.identity,
        attempt.fence,
        5,
        &handoff,
        &verification,
    )?;
    drop(store);

    let mut store = open(&path);
    assert_eq!(
        store.record_verification(
            &work,
            &attempt.identity,
            attempt.fence,
            6,
            &handoff,
            &verification
        )?,
        receipt
    );
    assert!(
        store
            .cancel(&attempt.identity, attempt.fence, 7, "late cancel")
            .is_err()
    );
    assert!(
        store
            .record_verification(
                &work,
                &attempt.identity,
                attempt.fence + 1,
                7,
                &handoff,
                &verification
            )
            .is_err()
    );
    let mut stale_work = work.clone();
    stale_work.revision += 1;
    assert!(
        store
            .record_verification(
                &stale_work,
                &attempt.identity,
                attempt.fence,
                7,
                &handoff,
                &verification
            )
            .is_err()
    );
    let retained = store
        .attempt(&attempt.identity)?
        .ok_or("missing completed attempt")?;
    assert_eq!(retained.phase, AttemptPhase::Verified);
    assert_eq!(retained.ended_at_ms, Some(5));
    assert_eq!(retained.handoff.as_ref(), Some(&handoff));
    assert_eq!(retained.verification.as_ref(), Some(&verification));
    assert_eq!(
        retained.verification_outcome,
        Some(VerificationOutcome::Verified)
    );
    store.record_verification_outcome(
        &attempt.identity,
        attempt.fence,
        VerificationOutcome::Verified,
    )?;
    assert!(
        store
            .record_verification_outcome(
                &attempt.identity,
                attempt.fence,
                VerificationOutcome::Rejected
            )
            .is_err()
    );
    assert_eq!(
        retained.workflow.as_ref(),
        Some(&reference("run", "racing-worker"))
    );
    assert!(retained.dispatch.is_some());
    assert!(retained.dispatch_request_binding.is_some());
    assert_eq!(
        store.campaign_request_binding(&attempt.identity)?,
        retained.dispatch_request_binding
    );
    assert!(store.dispatch_requires_recovery(&attempt.identity)?);
    assert!(
        store
            .attempts()?
            .authorize_publication(&attempt.identity, attempt.fence)
            .is_ok()
    );
    assert!(store.recover(8)?.active.is_empty());
    assert!(store.claim_next(8, LEASE)?.is_none());
    assert_eq!(store.attempt(&attempt.identity)?, Some(retained));
    Ok(())
}

// r[verify kiln.implementation.unknown_barrier.scenario.restart]
#[test]
fn cancel_before_late_completion_requires_observed_termination_across_restart()
-> Result<(), Box<dyn std::error::Error>> {
    use kiln_core::implementation::AttemptPhase;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("implementation.db");
    let (work, attempt, handoff, verification) = racing_callbacks(&path)?;
    let mut store = open(&path);
    store.cancel(&attempt.identity, attempt.fence, 5, "operator cancelled")?;
    assert!(
        store
            .record_verification(
                &work,
                &attempt.identity,
                attempt.fence,
                6,
                &handoff,
                &verification
            )
            .is_err()
    );
    drop(store);

    let mut store = open(&path);
    assert_eq!(store.recover(7)?.active, vec![attempt.identity.clone()]);
    assert!(
        store.claim_next(7, LEASE)?.is_none(),
        "cancellation does not prove provider death"
    );
    assert!(
        store
            .cancel(&attempt.identity, attempt.fence, 7, "replayed cancel")
            .is_err()
    );
    assert!(
        store
            .record_verification(
                &work,
                &attempt.identity,
                attempt.fence,
                7,
                &handoff,
                &verification
            )
            .is_err()
    );
    assert!(
        store
            .record_verification(
                &work,
                &attempt.identity,
                attempt.fence + 1,
                7,
                &handoff,
                &verification
            )
            .is_err()
    );
    let mut stale_work = work.clone();
    stale_work.revision += 1;
    assert!(
        store
            .record_verification(
                &stale_work,
                &attempt.identity,
                attempt.fence,
                7,
                &handoff,
                &verification
            )
            .is_err()
    );
    let cancelled = store
        .attempt(&attempt.identity)?
        .ok_or("missing cancelled attempt")?;
    assert_eq!(cancelled.phase, AttemptPhase::Cancelled);
    assert_eq!(cancelled.failure, Some(FailureClass::Unknown));
    assert_eq!(cancelled.ended_at_ms, Some(5));
    assert_eq!(cancelled.handoff.as_ref(), Some(&handoff));
    assert!(cancelled.verification.is_none());
    assert_eq!(
        cancelled.workflow.as_ref(),
        Some(&reference("run", "racing-worker"))
    );
    assert!(cancelled.dispatch.is_some());
    assert!(cancelled.dispatch_request_binding.is_some());
    assert_eq!(
        store.campaign_request_binding(&attempt.identity)?,
        cancelled.dispatch_request_binding
    );
    assert!(store.dispatch_requires_recovery(&attempt.identity)?);
    assert!(
        store
            .attempts()?
            .authorize_publication(&attempt.identity, attempt.fence)
            .is_err()
    );
    assert!(
        store
            .reconcile_unknown(
                &attempt.identity,
                attempt.fence + 1,
                &reference("termination", "stopped")
            )
            .is_err()
    );
    let observed = reference("termination", "stopped");
    store.reconcile_unknown(&attempt.identity, attempt.fence, &observed)?;
    drop(store);

    let mut store = open(&path);
    store.reconcile_unknown(&attempt.identity, attempt.fence, &observed)?;
    assert!(
        store
            .reconcile_unknown(
                &attempt.identity,
                attempt.fence,
                &reference("termination", "different")
            )
            .is_err()
    );
    assert!(store.recover(8)?.active.is_empty());
    let replacement = store
        .claim_next(8, LEASE)?
        .ok_or("missing replacement after termination")?;
    assert_eq!(replacement.ordinal, 2);
    assert!(replacement.fence > attempt.fence);
    assert!(
        store
            .record_verification(
                &work,
                &attempt.identity,
                attempt.fence,
                9,
                &handoff,
                &verification
            )
            .is_err()
    );
    assert!(
        store
            .authorize_current(&attempt.identity, attempt.fence, 9)
            .is_err()
    );
    assert_eq!(store.attempt(&attempt.identity)?, Some(cancelled));
    assert!(
        store
            .attempt(&replacement.identity)?
            .ok_or("replacement absent")?
            .verification
            .is_none()
    );
    Ok(())
}

// r[verify kiln.implementation.unknown_barrier.scenario.restart]
#[test]
fn possible_dispatch_requires_recovery_and_unknown_retry_is_fenced()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("implementation.db");
    let mut store = open(&path);
    use kiln_core::implementation::{
        DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace, ImplementationDispatchPlan,
        WorkspaceIsolation,
    };
    let mut work = work();
    work.base =
        Some(ExternalRef::new("git", "commit", &"a".repeat(40)).map_err(|error| error.message)?);
    store.admit(&work, None)?;
    let attempt = store.claim_next(0, LEASE)?.ok_or("missing attempt")?;
    let dispatch = ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: attempt.work_identity.clone(),
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        workspace: DispatchWorkspace {
            identity: content('e'),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
        },
        provider_profile: provider(),
        permitted_paths: work.permitted_paths.clone(),
        permitted_effects: work.permitted_effects.clone(),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 1,
        token_budget: 1000,
        invocation_budget: 1,
        expected_output: content('f'),
        execution: None,
    };
    let plan = store
        .reserve_dispatch_plan(&work, &attempt.identity, attempt.fence, 1, &dispatch)?
        .plan;
    store.bind_campaign_request(
        &work,
        &attempt.identity,
        attempt.fence,
        1,
        b"exact request bytes",
    )?;
    assert!(!store.begin_dispatch_transport(&attempt.identity, attempt.fence, 1, &plan)?);
    drop(store);
    let mut store = open(&path);
    assert!(
        store
            .bind_campaign_request(
                &work,
                &attempt.identity,
                attempt.fence,
                2,
                b"substituted request bytes"
            )
            .is_err()
    );
    assert!(store.begin_dispatch_transport(&attempt.identity, attempt.fence, 2, &plan)?);
    store.recover(LEASE)?;
    assert!(store.claim_next(LEASE, LEASE)?.is_none());
    assert_eq!(store.recover(LEASE)?.active, vec![attempt.identity.clone()]);
    let termination = reference("termination", "observed-stopped");
    assert!(
        store
            .reconcile_unknown(&attempt.identity, attempt.fence + 1, &termination)
            .is_err()
    );
    store.reconcile_unknown(&attempt.identity, attempt.fence, &termination)?;
    store.reconcile_unknown(&attempt.identity, attempt.fence, &termination)?;
    assert!(
        store
            .reconcile_unknown(
                &attempt.identity,
                attempt.fence,
                &reference("termination", "other")
            )
            .is_err()
    );
    let next = store
        .claim_next(LEASE, LEASE)?
        .ok_or("missing replacement")?;
    assert_eq!(next.ordinal, 2);
    assert!(next.fence > attempt.fence);
    assert!(
        store
            .authorize_current(&attempt.identity, attempt.fence, LEASE)
            .is_err()
    );
    Ok(())
}

// r[verify kiln.implementation.current_authority.scenario.cancelled]
#[test]
fn cancelled_and_superseded_attempts_lose_unexpired_authority_atomically()
-> Result<(), Box<dyn std::error::Error>> {
    use kiln_core::implementation::{AttemptPhase, SupersessionRule};
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("implementation.db");
    let mut store = open(&path);
    let mut work = work();
    store.admit(&work, None)?;
    let old = store.claim_next(0, LEASE)?.ok_or("missing attempt")?;
    work.revision += 1;
    work.title.push_str(" changed");
    store.admit_with_supersession(&work, None, SupersessionRule::AbortActive, 1)?;
    assert_eq!(
        store
            .attempt(&old.identity)?
            .ok_or("missing prior attempt")?
            .phase,
        AttemptPhase::Superseded
    );
    assert!(
        store
            .authorize_current(&old.identity, old.fence, 2)
            .is_err()
    );
    assert!(
        store
            .reserve_dispatch(&old.identity, old.fence, 2, &content('d'), &provider())
            .is_err()
    );
    let next = store.claim_next(2, LEASE)?.ok_or("missing replacement")?;
    store.cancel(&next.identity, next.fence, 3, "operator cancelled")?;
    assert!(
        store
            .authorize_current(&next.identity, next.fence, 4)
            .is_err()
    );
    assert!(
        store
            .start_execution(&next.identity, next.fence, 4, &reference("run", "late"))
            .is_err()
    );
    Ok(())
}

// r[verify kiln.implementation.supersession.scenario.frozen]
#[test]
fn finish_active_resolves_only_its_exact_frozen_work_across_restart()
-> Result<(), Box<dyn std::error::Error>> {
    use kiln_core::implementation::SupersessionRule;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("work.db");
    let original = work();
    let mut store = open(&path);
    store.admit(&original, None)?;
    let attempt = store.claim_next(10, LEASE)?.ok_or("claim missing")?;
    assert_eq!(
        store.active_work(&attempt.identity, attempt.fence, 11)?,
        original
    );
    store.reserve_dispatch(
        &attempt.identity,
        attempt.fence,
        11,
        &content('d'),
        &provider(),
    )?;
    store.start_execution(
        &attempt.identity,
        attempt.fence,
        11,
        &reference("run", "original"),
    )?;
    let mut newer = original.clone();
    newer.revision += 1;
    newer.title.push_str(" changed requirements");
    newer
        .acceptance
        .first_mut()
        .ok_or("acceptance missing")?
        .statement = "new acceptance must not rewrite running work".into();
    store.admit_with_supersession(&newer, None, SupersessionRule::FinishActive, 12)?;
    drop(store);
    let mut store = open(&path);
    assert_eq!(
        store.active_work(&attempt.identity, attempt.fence, 13)?,
        original
    );
    assert_eq!(store.work()?.current().next(), Some(&newer));
    assert_eq!(
        store
            .attempt(&attempt.identity)?
            .ok_or("attempt missing")?
            .superseded_by,
        Some(newer.revision)
    );
    assert!(
        store
            .active_work(&attempt.identity, attempt.fence + 1, 13)
            .is_err()
    );
    store.produce_candidate(
        &attempt.identity,
        attempt.fence,
        13,
        &reference("candidate", "frozen"),
    )?;
    store.begin_verification(&attempt.identity, attempt.fence, 14)?;
    assert_eq!(
        store.active_work(&attempt.identity, attempt.fence, 15)?,
        original
    );
    let publication_error = store
        .attempts()?
        .authorize_publication(&attempt.identity, attempt.fence)
        .err()
        .ok_or("superseded publication unexpectedly admitted")?;
    assert_eq!(
        publication_error.code.as_str(),
        "implementation_fence_stale"
    );
    store.cancel(
        &attempt.identity,
        attempt.fence,
        16,
        "stop frozen verification",
    )?;
    assert!(
        store
            .active_work(&attempt.identity, attempt.fence, 17)
            .is_err()
    );
    assert!(store.claim_next(17, LEASE)?.is_none());
    store.reconcile_unknown(
        &attempt.identity,
        attempt.fence,
        &reference("termination", "finish-active-stopped"),
    )?;
    drop(store);
    let mut store = open(&path);
    let replacement = store.claim_next(17, LEASE)?.ok_or("replacement missing")?;
    assert_eq!(
        store.active_work(&replacement.identity, replacement.fence, 18)?,
        newer
    );
    assert!(
        store
            .active_work(&attempt.identity, attempt.fence, 18)
            .is_err()
    );
    Ok(())
}

// r[verify kiln.implementation.current_authority.scenario.cancelled]
#[test]
fn abort_active_replaced_and_expired_fences_never_resolve_history()
-> Result<(), Box<dyn std::error::Error>> {
    use kiln_core::implementation::SupersessionRule;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("work.db");
    let original = work();
    let mut store = open(&path);
    store.admit(&original, None)?;
    let old = store.claim_next(0, 100)?.ok_or("claim missing")?;
    let mut newer = original;
    newer.revision += 1;
    newer.title.push_str(" new");
    store.admit_with_supersession(&newer, None, SupersessionRule::AbortActive, 1)?;
    assert!(store.active_work(&old.identity, old.fence, 2).is_err());
    let replacement = store.claim_next(2, 100)?.ok_or("replacement missing")?;
    assert_eq!(
        store.active_work(&replacement.identity, replacement.fence, 3)?,
        newer
    );
    assert!(store.active_work(&old.identity, old.fence, 3).is_err());
    assert!(
        store
            .active_work(&replacement.identity, replacement.fence, 102)
            .is_err()
    );
    drop(store);
    let mut store = open(&path);
    assert!(store.active_work(&old.identity, old.fence, 4).is_err());
    assert!(
        store
            .active_work(&replacement.identity, replacement.fence, 102)
            .is_err()
    );
    Ok(())
}

// r[verify kiln.implementation.plan_review.scenario.blocked]
#[test]
fn opted_in_work_is_claimed_only_after_its_plan_is_accepted() {
    use kiln_core::implementation::{
        ImplementationPlan, PLAN_REVIEW_POLICY_SCHEMA, PLAN_SCHEMA, PlanDecision, PlanGate,
        PlanReviewCommand, PlanReviewPolicy, PlanReviewResult, PlanStep, PlanVerdict,
    };
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = directory.path().join("implementation.db");
    let work = work();
    let identity = work.identity().unwrap_or_else(|_| unreachable!());
    let mut store = open(&path);
    store
        .admit(&work, None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    store
        .plan_review(&PlanReviewCommand::Configure {
            policy: Some(PlanReviewPolicy {
                schema: PLAN_REVIEW_POLICY_SCHEMA.to_string(),
                repositories: BTreeSet::from(["onix/kiln".to_string()]),
                reviewers: BTreeSet::from(["reviewer".to_string()]),
            }),
        })
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        store.claim_next(1_000, LEASE).map(|claim| claim.is_some()),
        Ok(false)
    );
    let proposal = PlanReviewCommand::Propose {
        plan: ImplementationPlan {
            schema: PLAN_SCHEMA.to_string(),
            work_identity: identity.clone(),
            revision: 1,
            previous: None,
            author: "agent".to_string(),
            steps: vec![PlanStep {
                id: "implement".to_string(),
                text: "Implement the admitted work.".to_string(),
            }],
        },
    };
    let plan = match store.plan_review(&proposal) {
        Ok(PlanReviewResult::Proposed { plan }) => plan,
        other => unreachable!("{other:?}"),
    };
    assert_eq!(
        store.plan_review(&PlanReviewCommand::Decide {
            work: identity.clone(),
            decision: PlanDecision {
                plan: plan.clone(),
                reviewer: "reviewer".to_string(),
                verdict: PlanVerdict::Accepted,
            },
        }),
        Ok(PlanReviewResult::Reviewed {
            gate: PlanGate::Accepted { plan },
        })
    );
    drop(store);
    // Acceptance is durable and records a decision only.
    let mut reopened = open(&path);
    assert_eq!(
        reopened.attempts().map(|ledger| ledger.attempts().count()),
        Ok(0)
    );
    let attempt = reopened
        .claim_next(2_000, LEASE)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .unwrap_or_else(|| unreachable!());
    assert_eq!(attempt.work_identity, identity);
}
