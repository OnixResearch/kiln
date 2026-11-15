use super::*;
use kiln_core::implementation::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn id(c: char) -> ContentId {
    ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}
fn git(kind: &str, c: char) -> ExternalRef {
    ExternalRef::new("git", kind, &c.to_string().repeat(40)).unwrap_or_else(|_| unreachable!())
}
fn enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from(["onix/kiln".into()]),
        actors: BTreeSet::from(["operator".into()]),
    }
}
fn open(path: &Path) -> Implementer {
    Implementer::open(path, enrollment()).unwrap_or_else(|error| unreachable!("{error}"))
}

// Synthetic retained core facts, not an execution/provider qualification.
fn fixture(path: &Path) -> (Implementer, ImplementationWork, ImplementationAttempt) {
    let work = ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "operator".into(),
        repository: "onix/kiln".into(),
        project: "kiln".into(),
        base: Some(git("commit", 'b')),
        title: "Recover publication".into(),
        requested_outcome: "exact".into(),
        acceptance: vec![AcceptanceObligation {
            id: "exact".into(),
            statement: "verified candidate".into(),
        }],
        dependencies: vec![],
        permitted_paths: vec!["crates".into()],
        permitted_effects: BTreeSet::from(["write_source".into()]),
        verification_profile: id('9'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedIntegrated,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "onix/kiln#47")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "onix/kiln#47".into(),
            version: id('1'),
        }],
    };
    let mut shell = open(path);
    shell
        .admit(&work, None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let mut attempt = shell
        .claim_next(0, 1000)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    attempt.phase = AttemptPhase::Verified;
    attempt.candidate = Some(git("commit", 'a'));
    attempt.publication = Some(PublicationRecord {
        schema: PUBLICATION_SCHEMA.into(),
        phase: PublicationPhase::Prepared,
        plan: PublicationPlan {
            schema: PUBLICATION_SCHEMA.into(),
            work_identity: work.identity().unwrap_or_else(|_| unreachable!()),
            work_revision: 1,
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            handoff: id('3'),
            verification: id('4'),
            repository: work.repository.clone(),
            base: git("commit", 'b'),
            base_branch: "main".into(),
            candidate: git("commit", 'a'),
            tree: git("tree", 'e'),
            branch: branch_for(&attempt.work_identity, &attempt.identity),
            title: "Implement".into(),
            body: "Actual changes and validation".into(),
        },
        remote_head: None,
        pull_request: None,
        integration: None,
        source_status: None,
    });
    let attempts: AttemptLedger = serde_json::from_value(serde_json::json!({
        "fence_counter": attempt.fence, "attempts": {(attempt.identity.as_str()): attempt}, "current": {}, "consumed": {}
    })).unwrap_or_else(|error| unreachable!("{error}"));
    let tx = shell
        .connection
        .transaction()
        .unwrap_or_else(|_| unreachable!());
    let (_, version) = load::<AttemptLedger>(&tx, ATTEMPT_KIND).unwrap_or_else(|_| unreachable!());
    save(&tx, ATTEMPT_KIND, version, &attempts).unwrap_or_else(|_| unreachable!());
    tx.commit().unwrap_or_else(|_| unreachable!());
    (shell, work, attempt)
}

#[test]
// r[verify kiln.publication.recovery.scenario.crash]
fn branch_and_pr_lost_ack_reconcile_once_and_equal_replay_never_transports() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("state.db");
    let (mut shell, work, attempt) = fixture(&path);
    let identity = &attempt.identity;
    let fence = attempt.fence;
    let mut mutations = 0;
    assert!(
        shell
            .consume_publication_effect(
                &work,
                identity,
                fence,
                PublicationEffectKind::Branch,
                |mode, _| {
                    assert_eq!(mode, EffectDispatch::FirstDispatch);
                    mutations += 1;
                    Err("ack lost after remote success".into())
                }
            )
            .is_err()
    );
    drop(shell);
    let mut shell = open(&path);
    let branch = shell
        .consume_publication_effect(
            &work,
            identity,
            fence,
            PublicationEffectKind::Branch,
            |mode, _| {
                assert_eq!(mode, EffectDispatch::ReconcileOnly);
                Ok(Some(PublicationEffectResult::Branch(git("commit", 'a'))))
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(branch.observed.is_some());
    let again = shell
        .consume_publication_effect(
            &work,
            identity,
            fence,
            PublicationEffectKind::Branch,
            |_, _| unreachable!("acknowledged replay contacted provider"),
        )
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(branch, again);
    assert!(
        shell
            .consume_publication_effect(
                &work,
                identity,
                fence,
                PublicationEffectKind::PullRequest,
                |mode, _| {
                    assert_eq!(mode, EffectDispatch::FirstDispatch);
                    mutations += 1;
                    Err("PR created, ack lost".into())
                }
            )
            .is_err()
    );
    drop(shell);
    let mut shell = open(&path);
    let result = shell
        .consume_publication_effect(
            &work,
            identity,
            fence,
            PublicationEffectKind::PullRequest,
            |mode, _| {
                assert_eq!(mode, EffectDispatch::ReconcileOnly);
                Ok(Some(PublicationEffectResult::PullRequest {
                    pull_request: ExternalRef::new("github", "pull_request", "onix/kiln#9")
                        .unwrap_or_else(|_| unreachable!()),
                    head: git("commit", 'a'),
                }))
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(result.observed.is_some());
    assert_eq!(mutations, 2);
    assert_eq!(
        shell
            .publication_record(identity)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!())
            .phase,
        PublicationPhase::PullRequestPublished
    );
}

#[test]
fn unknown_absence_wrong_head_and_wrong_result_cannot_advance_or_retry() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("state.db");
    let (mut shell, work, attempt) = fixture(&path);
    let (reserved, mode) = shell
        .reserve_publication_effect(
            &work,
            &attempt.identity,
            attempt.fence,
            PublicationEffectKind::Branch,
        )
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(mode, EffectDispatch::FirstDispatch);
    assert!(
        shell
            .acknowledge_publication_effect(
                &work,
                &reserved,
                PublicationEffectResult::Branch(git("commit", 'f'))
            )
            .is_err()
    );
    assert!(
        shell
            .acknowledge_publication_effect(
                &work,
                &reserved,
                PublicationEffectResult::PullRequest {
                    pull_request: git("commit", 'a'),
                    head: git("commit", 'a')
                }
            )
            .is_err()
    );
    drop(shell);
    let mut shell = open(&path);
    for _ in 0..2 {
        let pending = shell
            .consume_publication_effect(
                &work,
                &attempt.identity,
                attempt.fence,
                PublicationEffectKind::Branch,
                |mode, _| {
                    assert_eq!(mode, EffectDispatch::ReconcileOnly);
                    Ok(None)
                },
            )
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(pending.operation, reserved.operation);
        assert!(pending.observed.is_none());
    }
    assert_eq!(
        shell
            .publication_record(&attempt.identity)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!())
            .phase,
        PublicationPhase::Prepared
    );
}

#[test]
fn stale_fence_or_source_revision_refuses_transport_and_pending_ack() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("state.db");
    let (mut shell, work, attempt) = fixture(&path);
    assert!(
        shell
            .consume_publication_effect(
                &work,
                &attempt.identity,
                attempt.fence + 1,
                PublicationEffectKind::Branch,
                |_, _| unreachable!("stale dispatched")
            )
            .is_err()
    );
    let (reserved, _) = shell
        .reserve_publication_effect(
            &work,
            &attempt.identity,
            attempt.fence,
            PublicationEffectKind::Branch,
        )
        .unwrap_or_else(|_| unreachable!());
    let mut newer = work.clone();
    newer.revision += 1;
    newer.title = "Changed source".into();
    shell
        .admit(&newer, None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(
        shell
            .consume_publication_effect(
                &work,
                &attempt.identity,
                attempt.fence,
                PublicationEffectKind::Branch,
                |_, _| unreachable!("superseded dispatched")
            )
            .is_err()
    );
    assert!(
        shell
            .acknowledge_publication_effect(
                &work,
                &reserved,
                PublicationEffectResult::Branch(git("commit", 'a'))
            )
            .is_err()
    );
}

#[test]
fn integration_and_source_crashes_recover_independently_without_reimplementation() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("state.db");
    let (mut shell, work, attempt) = fixture(&path);
    let identity = &attempt.identity;
    let fence = attempt.fence;
    shell
        .consume_publication_effect(
            &work,
            identity,
            fence,
            PublicationEffectKind::Branch,
            |_, _| Ok(Some(PublicationEffectResult::Branch(git("commit", 'a')))),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let pr = ExternalRef::new("github", "pull_request", "onix/kiln#9")
        .unwrap_or_else(|_| unreachable!());
    shell
        .consume_publication_effect(
            &work,
            identity,
            fence,
            PublicationEffectKind::PullRequest,
            |_, _| {
                Ok(Some(PublicationEffectResult::PullRequest {
                    pull_request: pr.clone(),
                    head: git("commit", 'a'),
                }))
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let publication = shell
        .publication_record(identity)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let plan = &publication.plan;
    let integration = IntegrationReceipt {
        schema: INTEGRATION_SCHEMA.into(),
        work_identity: plan.work_identity.clone(),
        work_revision: plan.work_revision,
        attempt: identity.clone(),
        fence,
        handoff: plan.handoff.clone(),
        verification: plan.verification.clone(),
        candidate: plan.candidate.clone(),
        pull_request: pr,
        merge_commit: git("commit", 'c'),
        base_ref: plan.base_branch.clone(),
        base_head: git("commit", 'd'),
        mapping: IntegrationMapping {
            expected_base: plan.base.clone(),
            candidate_tree: plan.tree.clone(),
            landed_tree: plan.tree.clone(),
        },
    };
    shell
        .consume_publication_effect(
            &work,
            identity,
            fence,
            PublicationEffectKind::Integration,
            |mode, _| {
                assert_eq!(mode, EffectDispatch::FirstDispatch);
                Ok(None)
            },
        )
        .unwrap_or_else(|_| unreachable!());
    drop(shell);
    let mut shell = open(&path);
    let integrated = shell
        .consume_publication_effect(
            &work,
            identity,
            fence,
            PublicationEffectKind::Integration,
            |mode, _| {
                assert_eq!(mode, EffectDispatch::ReconcileOnly);
                Ok(Some(PublicationEffectResult::Integration(
                    integration.clone(),
                )))
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let current_attempt = shell
        .attempt(identity)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let current_publication = current_attempt
        .publication
        .as_ref()
        .unwrap_or_else(|| unreachable!());
    let integration_id =
        admit_integration(&work, &current_attempt, current_publication, &integration)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
    let source = work
        .aliases
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    let source_plan = SourceStatusPlan {
        schema: SOURCE_STATUS_SCHEMA.into(),
        work_identity: plan.work_identity.clone(),
        work_revision: work.revision,
        attempt: identity.clone(),
        fence,
        integration: integration_id.clone(),
        source: source.clone(),
        action: SourceAction::Close,
        expected_observed_revision: work.revision + 1,
    };
    let receipt = SourceStatusReceipt {
        schema: SOURCE_STATUS_SCHEMA.into(),
        work_identity: plan.work_identity.clone(),
        work_revision: work.revision,
        attempt: identity.clone(),
        fence,
        integration: integration_id,
        source,
        state: "closed".into(),
        observed_revision: work.revision + 1,
    };
    assert!(
        shell
            .consume_publication_effect(
                &work,
                identity,
                fence,
                PublicationEffectKind::SourceStatus(source_plan.clone()),
                |_, _| Err("lost source acknowledgement".into())
            )
            .is_err()
    );
    drop(shell);
    let mut shell = open(&path);
    let source_result = shell
        .consume_publication_effect(
            &work,
            identity,
            fence,
            PublicationEffectKind::SourceStatus(source_plan.clone()),
            |mode, _| {
                assert_eq!(mode, EffectDispatch::ReconcileOnly);
                Ok(Some(PublicationEffectResult::SourceStatus {
                    plan: source_plan.clone(),
                    receipt,
                }))
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_ne!(source_result.operation, integrated.operation);
    let replay = shell
        .consume_publication_effect(
            &work,
            identity,
            fence,
            PublicationEffectKind::Integration,
            |_, _| unreachable!("completed integration repeated"),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(replay, integrated);
    assert_eq!(
        shell
            .publication_record(identity)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!())
            .phase,
        PublicationPhase::SourceReconciled
    );
}

#[test]
fn cairn_lifecycle_cannot_reuse_another_task_spec_candidate_or_authoritative_result() {
    use kiln_core::change_review::{CiCheck, CiFact};
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("state.db");
    let (mut shell, mut work, mut attempt) = fixture(&path);
    let source = SourceRef {
        kind: WorkSourceKind::CairnTask,
        source: ExternalRef::new("cairn", "task", "demo/task:build")
            .unwrap_or_else(|_| unreachable!()),
        native_id: "demo/task:build".into(),
        version: id('7'),
    };
    work.aliases = vec![source.clone()];
    let work_id = work.identity().unwrap_or_else(|_| unreachable!());
    attempt.work_identity = work_id.clone();
    let mut publication = attempt
        .publication
        .clone()
        .unwrap_or_else(|| unreachable!());
    publication.plan.work_identity = work_id.clone();
    publication.phase = PublicationPhase::Integrated;
    let plan = &publication.plan;
    let pr = ExternalRef::new("github", "pull_request", "onix/kiln#9")
        .unwrap_or_else(|_| unreachable!());
    publication.pull_request = Some(pr.clone());
    publication.remote_head = Some(plan.candidate.clone());
    publication.integration = Some(IntegrationReceipt {
        schema: INTEGRATION_SCHEMA.into(),
        work_identity: work_id.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        handoff: plan.handoff.clone(),
        verification: plan.verification.clone(),
        candidate: plan.candidate.clone(),
        pull_request: pr,
        merge_commit: git("commit", 'c'),
        base_ref: plan.base_branch.clone(),
        base_head: git("commit", 'c'),
        mapping: IntegrationMapping {
            expected_base: plan.base.clone(),
            candidate_tree: plan.tree.clone(),
            landed_tree: plan.tree.clone(),
        },
    });
    let run = CiFact {
        project: work.project.clone(),
        repository: work.repository.clone(),
        revision: plan.candidate.native_id.as_str().into(),
        run_id: "trusted-verifier".into(),
        ordinal: 1,
        success: true,
        full_run: true,
        clean: true,
        scope_identity: None,
        terminal: true,
        untrusted: None,
        checks: vec![CiCheck {
            name: "cairn-authoritative-completion".into(),
            kind: "cairn-agent-completion-v2".into(),
            outcome: "pass".into(),
            artifact_digest: Some(id('6').as_str().into()),
        }],
    };
    attempt.verification = Some(CandidateVerification {
        schema: VERIFICATION_SCHEMA.into(),
        handoff: plan.handoff.clone(),
        work_identity: work_id,
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        candidate: plan.candidate.clone(),
        tree: plan.tree.clone(),
        base: Some(plan.base.clone()),
        verification_profile: work.verification_profile.clone(),
        run,
    });
    let binding = CairnLifecycleBinding {
        source,
        handoff: plan.handoff.clone(),
        verification: plan.verification.clone(),
        authoritative_completion: id('6'),
        candidate: plan.candidate.clone(),
        action: CairnLifecycleAction::ObserveReviewedSync,
    };
    assert!(admit_lifecycle(&work, &attempt, &publication, &binding).is_ok());
    for wrong in [
        {
            let mut wrong = binding.clone();
            wrong.source.version = id('8');
            wrong
        },
        {
            let mut wrong = binding.clone();
            wrong.source.native_id = "other/task:build".into();
            wrong
        },
        {
            let mut wrong = binding.clone();
            wrong.candidate = git("commit", 'f');
            wrong
        },
        {
            let mut wrong = binding.clone();
            wrong.authoritative_completion = id('9');
            wrong
        },
        {
            let mut wrong = binding.clone();
            wrong.handoff = id('a');
            wrong
        },
    ] {
        assert!(admit_lifecycle(&work, &attempt, &publication, &wrong).is_err());
    }
    // Seed this synthetic accepted core state, then exercise actual SQLite
    // reservation/acknowledgement and replay rather than only the predicate.
    attempt.canonical_key = work.canonical_key().unwrap_or_else(|_| unreachable!());
    attempt.publication = Some(publication.clone());
    shell
        .admit(&work, None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let attempts: AttemptLedger = serde_json::from_value(serde_json::json!({
        "fence_counter": attempt.fence, "attempts": {(attempt.identity.as_str()): attempt}, "current": {}, "consumed": {}
    })).unwrap_or_else(|_| unreachable!());
    let tx = shell
        .connection
        .transaction()
        .unwrap_or_else(|_| unreachable!());
    let (_, version) = load::<AttemptLedger>(&tx, ATTEMPT_KIND).unwrap_or_else(|_| unreachable!());
    save(&tx, ATTEMPT_KIND, version, &attempts).unwrap_or_else(|_| unreachable!());
    tx.commit().unwrap_or_else(|_| unreachable!());
    let policy = kiln_core::fair_admission::AdmissionPolicy {
        version: 1,
        accounting_basis: id('a'),
        max_items: 16,
        max_bytes: 100_000,
        max_wait_ms: 1000,
        max_observation_age_ms: 100,
        max_retry_after_ms: 100,
        max_children: 4,
        max_active: 2,
        strategy: kiln_core::operations::FairnessStrategy::DominantShare,
        shares: BTreeMap::from([(
            kiln_core::types::Alias::new("campaign").unwrap_or_else(|_| unreachable!()),
            1,
        )]),
        group_limits: vec![],
    };
    let policy_identity = policy.identity().unwrap_or_else(|_| unreachable!());
    let tx = shell
        .connection
        .transaction()
        .unwrap_or_else(|_| unreachable!());
    let (mut admission, version) =
        load::<kiln_core::fair_admission::AdmissionLedger>(&tx, "fair-admission-v1")
            .unwrap_or_else(|_| unreachable!());
    admission.policy = Some(policy);
    save(&tx, "fair-admission-v1", version, &admission).unwrap_or_else(|_| unreachable!());
    tx.commit().unwrap_or_else(|_| unreachable!());
    assert!(
        shell
            .consume_publication_effect(
                &work,
                &attempt.identity,
                attempt.fence,
                PublicationEffectKind::CairnLifecycle(binding.clone()),
                |_, _| Err("lost lifecycle observation acknowledgement".into())
            )
            .is_err()
    );
    assert!(
        shell
            .read_cairn_lifecycle_effect(&work, &attempt.identity, attempt.fence, &binding)
            .unwrap_or_else(|_| unreachable!())
            .is_none()
    );
    drop(shell);
    let mut shell = open(&path);
    let observed = CairnLifecycleObservation {
        binding: binding.clone(),
        observed_tree: publication.plan.tree.clone(),
        lifecycle_policy: policy_identity.clone(),
        lifecycle_receipt: ExternalRef::new("cairn", "lifecycle_receipt", id('e').as_str())
            .unwrap_or_else(|_| unreachable!()),
    };
    let retained = shell
        .consume_publication_effect(
            &work,
            &attempt.identity,
            attempt.fence,
            PublicationEffectKind::CairnLifecycle(binding.clone()),
            |mode, _| {
                assert_eq!(mode, EffectDispatch::ReconcileOnly);
                Ok(Some(PublicationEffectResult::CairnLifecycle(observed)))
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        shell
            .read_cairn_lifecycle_effect(&work, &attempt.identity, attempt.fence, &binding)
            .unwrap_or_else(|error| unreachable!("{error}")),
        Some(retained.clone())
    );
    assert!(
        shell
            .read_cairn_lifecycle_effect(&work, &attempt.identity, attempt.fence + 1, &binding)
            .is_err()
    );
    let mut wrong_binding = binding.clone();
    wrong_binding.authoritative_completion = id('9');
    assert!(
        shell
            .read_cairn_lifecycle_effect(&work, &attempt.identity, attempt.fence, &wrong_binding)
            .is_err()
    );
    let changed_policy = kiln_core::fair_admission::AdmissionPolicy {
        version: 1,
        accounting_basis: id('b'),
        max_items: 16,
        max_bytes: 100_000,
        max_wait_ms: 1000,
        max_observation_age_ms: 100,
        max_retry_after_ms: 100,
        max_children: 4,
        max_active: 2,
        strategy: kiln_core::operations::FairnessStrategy::DominantShare,
        shares: BTreeMap::from([(
            kiln_core::types::Alias::new("campaign").unwrap_or_else(|_| unreachable!()),
            1,
        )]),
        group_limits: vec![],
    };
    let tx = shell
        .connection
        .transaction()
        .unwrap_or_else(|_| unreachable!());
    let (mut admission, version) =
        load::<kiln_core::fair_admission::AdmissionLedger>(&tx, "fair-admission-v1")
            .unwrap_or_else(|_| unreachable!());
    admission.policy = Some(changed_policy);
    save(&tx, "fair-admission-v1", version, &admission).unwrap_or_else(|_| unreachable!());
    tx.commit().unwrap_or_else(|_| unreachable!());
    assert!(
        shell
            .read_cairn_lifecycle_effect(&work, &attempt.identity, attempt.fence, &binding)
            .is_err()
    );
    drop(shell);
    let mut shell = open(&path);
    let replay = shell
        .consume_publication_effect(
            &work,
            &attempt.identity,
            attempt.fence,
            PublicationEffectKind::CairnLifecycle(binding.clone()),
            |_, _| unreachable!("accepted lifecycle repeated"),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(retained, replay);
    work.terminal_policy = TerminalPolicy::VerifiedPullRequest;
    assert!(admit_lifecycle(&work, &attempt, &publication, &binding).is_err());
}
