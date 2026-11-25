use std::collections::BTreeSet;
use std::path::Path;

use kiln_core::change_review::{CiCheck, CiFact};
use kiln_core::implementation::{
    AcceptanceObligation, AttemptLedger, CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence,
    CandidateHandoff, CandidateVerification, Enrollment, EvidenceKind, EvidenceOutcome,
    IMPLEMENTATION_WORK_SCHEMA, ImplementationAttempt, ImplementationWork, IntegrationMapping,
    LAYER_VERIFICATION_SCHEMA, LayerVerification, PUBLICATION_SCHEMA, PublicationPlan,
    STACK_INTEGRATION_SCHEMA, STACK_PUBLICATION_SCHEMA, STACK_SCHEMA, SourceRef, StackIntegration,
    StackLayerIntegration, StackLayerPlan, StackPlan, StackPublication, TerminalPolicy,
    VERIFICATION_SCHEMA, WorkBudget, WorkSourceKind, acceptance_digest, admit_candidate_handoff,
    admit_candidate_verification, admit_layer_verification, branch_for, layer_branch,
    stack_identity,
};
use kiln_core::types::{ContentId, ExternalRef};

use super::*;

const BASE_HEX: &str = "1111111111111111111111111111111111111111";
const LOWER_HEX: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const LOWER_TREE_HEX: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const TOP_HEX: &str = "cccccccccccccccccccccccccccccccccccccccc";
const TOP_TREE_HEX: &str = "dddddddddddddddddddddddddddddddddddddddd";
const LOWER_MERGE_HEX: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const TOP_MERGE_HEX: &str = "ffffffffffffffffffffffffffffffffffffffff";

fn id(c: char) -> ContentId {
    ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn git(kind: &str, hex: &str) -> ExternalRef {
    ExternalRef::new("git", kind, hex).unwrap_or_else(|_| unreachable!())
}

fn github(kind: &str, number: u64) -> ExternalRef {
    ExternalRef::new("github", kind, &format!("onix/kiln#{number}"))
        .unwrap_or_else(|_| unreachable!())
}

fn enrollment() -> Enrollment {
    Enrollment {
        repositories: BTreeSet::from(["onix/kiln".into()]),
        actors: BTreeSet::from(["operator".into()]),
    }
}

fn open(path: &Path) -> Implementer {
    let mut implementer =
        Implementer::open_with_local_operator(path, enrollment(), "test-operator")
            .unwrap_or_else(|error| unreachable!("{error}"));
    implementer.set_clock_ms(10);
    implementer
}

fn ci_run(work: &ImplementationWork, revision: &str) -> CiFact {
    CiFact {
        project: work.project.clone(),
        repository: work.repository.clone(),
        revision: revision.to_string(),
        run_id: format!("run-{revision}"),
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
    }
}

struct VerifiedFixture {
    shell: Implementer,
    work: ImplementationWork,
    attempt: ImplementationAttempt,
    handoff: CandidateHandoff,
    verification: CandidateVerification,
    layer_verifications: Vec<LayerVerification>,
    plan: StackPlan,
}

fn verified_fixture(path: &Path) -> VerifiedFixture {
    let work = ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "operator".into(),
        repository: "onix/kiln".into(),
        project: "kiln".into(),
        base: Some(git("commit", BASE_HEX)),
        title: "Ordered stack publication".into(),
        requested_outcome: "stack".into(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".into(),
            statement: "verified stack".into(),
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
            source: ExternalRef::new("github", "issue", "onix/kiln#89")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "onix/kiln#89".into(),
            version: id('1'),
        }],
    };
    let mut shell = open(path);
    shell
        .admit(&work, None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let attempt = shell
        .claim_next(0, 1000)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .unwrap_or_else(|| unreachable!("claim"));
    shell
        .reserve_dispatch(
            &attempt.identity,
            attempt.fence,
            0,
            &id('d'),
            &kiln_core::types::ProviderProfileId::new("lattice-implementation")
                .unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    shell
        .start_execution(
            &attempt.identity,
            attempt.fence,
            1,
            &ExternalRef::new("lattice", "run", "one").unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let work_identity = work.identity().unwrap_or_else(|_| unreachable!());
    let handoff = CandidateHandoff {
        schema: CANDIDATE_HANDOFF_SCHEMA.to_string(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        candidate: git("commit", TOP_HEX),
        tree: git("tree", TOP_TREE_HEX),
        changed_paths: vec!["crates/src/lib.rs".to_string()],
        acceptance_digest: acceptance_digest(&work).unwrap_or_else(|_| unreachable!()),
        verification_profile: work.verification_profile.clone(),
        toolchain: id('7'),
        environment: id('8'),
        evidence: vec![CandidateEvidence {
            identity: id('e'),
            kind: EvidenceKind::Check,
            outcome: EvidenceOutcome::Passed,
            reference: ExternalRef::new("kiln", "run", "candidate-check")
                .unwrap_or_else(|_| unreachable!()),
        }],
    };
    shell
        .admit_candidate_handoff(&work, &attempt.identity, attempt.fence, 2, &handoff)
        .unwrap_or_else(|error| unreachable!("{error}"));
    shell
        .begin_verification(&attempt.identity, attempt.fence, 3)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let handoff_identity =
        admit_candidate_handoff(&work, &attempt, &handoff).unwrap_or_else(|_| unreachable!());
    let verification = CandidateVerification {
        schema: VERIFICATION_SCHEMA.to_string(),
        handoff: handoff_identity.clone(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        candidate: handoff.candidate.clone(),
        tree: handoff.tree.clone(),
        base: handoff.base.clone(),
        verification_profile: work.verification_profile.clone(),
        run: ci_run(&work, TOP_HEX),
    };
    shell
        .record_verification(
            &work,
            &attempt.identity,
            attempt.fence,
            4,
            &handoff,
            &verification,
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let attempt = shell
        .attempt(&attempt.identity)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let lower = LayerVerification {
        schema: LAYER_VERIFICATION_SCHEMA.to_string(),
        handoff: handoff_identity.clone(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        position: 1,
        candidate: git("commit", LOWER_HEX),
        tree: git("tree", LOWER_TREE_HEX),
        parent: git("commit", BASE_HEX),
        verification_profile: work.verification_profile.clone(),
        run: ci_run(&work, LOWER_HEX),
    };
    let lower_id = admit_layer_verification(&work, &attempt, &handoff, &lower)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let top_id = admit_candidate_verification(&work, &attempt, &handoff, &verification)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let bottom_branch = layer_branch(&work_identity, &attempt.identity, 1);
    let top_branch = layer_branch(&work_identity, &attempt.identity, 2);
    let plan = StackPlan {
        schema: STACK_SCHEMA.to_string(),
        work_identity,
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        handoff: handoff_identity,
        verification: top_id.clone(),
        repository: work.repository.clone(),
        base: git("commit", BASE_HEX),
        trunk_branch: "main".to_string(),
        layers: vec![
            StackLayerPlan {
                position: 1,
                candidate: git("commit", LOWER_HEX),
                tree: git("tree", LOWER_TREE_HEX),
                parent: git("commit", BASE_HEX),
                verification: lower_id,
                branch: bottom_branch.clone(),
                base_branch: "main".to_string(),
                title: "Layer 1".to_string(),
                body: "First layer".to_string(),
            },
            StackLayerPlan {
                position: 2,
                candidate: git("commit", TOP_HEX),
                tree: git("tree", TOP_TREE_HEX),
                parent: git("commit", LOWER_HEX),
                verification: top_id,
                branch: top_branch,
                base_branch: bottom_branch,
                title: "Layer 2".to_string(),
                body: "Second layer".to_string(),
            },
        ],
    };
    VerifiedFixture {
        shell,
        work,
        attempt,
        handoff,
        verification,
        layer_verifications: vec![lower],
        plan,
    }
}

#[test]
fn stack_persistence_preserves_attempt_ledger_backwards_compatibility() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("state.db");
    let mut fixture = verified_fixture(&path);
    let stack = fixture
        .shell
        .prepare_stack_publication(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            10,
            &fixture.layer_verifications,
            &fixture.plan,
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        stack,
        stack_identity(&fixture.plan).unwrap_or_else(|_| unreachable!())
    );
    // Repeating exact preparation is idempotent.
    assert_eq!(
        fixture
            .shell
            .prepare_stack_publication(
                &fixture.work,
                &fixture.attempt.identity,
                fixture.attempt.fence,
                11,
                &fixture.layer_verifications,
                &fixture.plan,
            )
            .ok(),
        Some(stack.clone())
    );
    // Preparing a single-PR publication after a stack plan is refused.
    let single_pr = PublicationPlan {
        schema: PUBLICATION_SCHEMA.to_string(),
        work_identity: fixture.plan.work_identity.clone(),
        work_revision: fixture.plan.work_revision,
        attempt: fixture.plan.attempt.clone(),
        fence: fixture.plan.fence,
        handoff: fixture.plan.handoff.clone(),
        verification: fixture.plan.verification.clone(),
        repository: fixture.plan.repository.clone(),
        base: fixture.plan.base.clone(),
        base_branch: "main".to_string(),
        candidate: fixture.handoff.candidate.clone(),
        tree: fixture.handoff.tree.clone(),
        branch: branch_for(&fixture.plan.work_identity, &fixture.plan.attempt),
        title: "Single PR".to_string(),
        body: "Conflicts with stack".to_string(),
    };
    assert!(
        fixture
            .shell
            .prepare_publication(
                &fixture.work,
                &fixture.attempt.identity,
                fixture.attempt.fence,
                12,
                &fixture.handoff,
                &fixture.verification,
                &single_pr,
            )
            .is_err()
    );
    drop(fixture.shell);

    // An older binary reads and re-saves the `attempts` row using `AttemptLedger`:
    // byte-identical round trip with zero stack keys inside `attempts`.
    {
        let connection = rusqlite::Connection::open(&path).unwrap_or_else(|_| unreachable!());
        let (version, bytes): (i64, Vec<u8>) = connection
            .query_row(
                "SELECT version, bytes FROM implementation_state WHERE kind = 'attempts'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or_else(|_| unreachable!());
        let published: AttemptLedger =
            serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!());
        let resaved = serde_json::to_vec(&published).unwrap_or_else(|_| unreachable!());
        assert_eq!(resaved, bytes);
        assert!(
            !String::from_utf8(bytes)
                .unwrap_or_else(|_| unreachable!())
                .contains("stack")
        );
        connection
            .execute(
                "UPDATE implementation_state SET version = ?1, bytes = ?2 WHERE kind = 'attempts'",
                rusqlite::params![version + 1, resaved],
            )
            .unwrap_or_else(|_| unreachable!());
    }

    let mut reopened = open(&path);
    let record = reopened
        .stack_publication_record(&fixture.attempt.identity)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("retained stack plan"));
    assert_eq!(record.stack, stack);
    assert_eq!(record.plan, fixture.plan);
    assert_eq!(record.publication, None);
    assert_eq!(record.integration, None);
}

#[test]
fn stack_effects_reconcile_after_crash_replay_idempotently_and_refuse_stale_fences() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("state.db");
    let mut fixture = verified_fixture(&path);
    let stack = fixture
        .shell
        .prepare_stack_publication(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            10,
            &fixture.layer_verifications,
            &fixture.plan,
        )
        .unwrap_or_else(|error| unreachable!("{error}"));

    // Stale fence is refused before reserving any effect.
    let stale_err = fixture
        .shell
        .consume_stack_publication_effect(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence + 1,
            &fixture.plan,
            |_, _| unreachable!("stale fence must never invoke transport"),
        )
        .err()
        .unwrap_or_else(|| unreachable!());
    assert!(
        stale_err.contains("fence") || stale_err.contains("implementation_"),
        "{stale_err}"
    );

    // Simulated crash after reservation (`FirstDispatch` returns Err).
    let crash = fixture
        .shell
        .consume_stack_publication_effect(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            &fixture.plan,
            |dispatch, _| {
                assert_eq!(dispatch, EffectDispatch::FirstDispatch);
                Err("simulated crash".to_string())
            },
        )
        .err()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(crash, "simulated crash");

    let pending = fixture
        .shell
        .read_stack_effect(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            StackEffectKind::Publish,
        )
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!("pending reservation"));
    assert_eq!(pending.observed, None);

    // Recovery re-entry receives ReconcileOnly and acknowledges the publication.
    let publication = StackPublication {
        schema: STACK_PUBLICATION_SCHEMA.to_string(),
        stack: stack.clone(),
        provider_stack: github("stack", 5),
        pull_requests: vec![github("pull_request", 11), github("pull_request", 12)],
    };
    let acked = fixture
        .shell
        .consume_stack_publication_effect(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            &fixture.plan,
            |dispatch, _| {
                assert_eq!(dispatch, EffectDispatch::ReconcileOnly);
                Ok(Some(publication.clone()))
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        acked.observed,
        Some(StackEffectResult::Publish(publication.clone()))
    );

    // Equal replay returns immediately without invoking transport.
    let replayed = fixture
        .shell
        .consume_stack_publication_effect(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            &fixture.plan,
            |_, _| unreachable!("already acknowledged stack publication must not transport"),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(replayed, acked);

    // Integration recovery: partial prefix first, then complete prefix, then no-op replay.
    let layer1 = StackLayerIntegration {
        position: 1,
        pull_request: github("pull_request", 11),
        candidate: git("commit", LOWER_HEX),
        merge_commit: git("commit", LOWER_MERGE_HEX),
        mapping: IntegrationMapping {
            expected_base: git("commit", BASE_HEX),
            candidate_tree: git("tree", LOWER_TREE_HEX),
            landed_tree: git("tree", LOWER_TREE_HEX),
        },
    };
    let layer2 = StackLayerIntegration {
        position: 2,
        pull_request: github("pull_request", 12),
        candidate: git("commit", TOP_HEX),
        merge_commit: git("commit", TOP_MERGE_HEX),
        mapping: IntegrationMapping {
            expected_base: git("commit", LOWER_MERGE_HEX),
            candidate_tree: git("tree", TOP_TREE_HEX),
            landed_tree: git("tree", TOP_TREE_HEX),
        },
    };
    let partial = StackIntegration {
        schema: STACK_INTEGRATION_SCHEMA.to_string(),
        stack: stack.clone(),
        layers: vec![layer1.clone()],
    };
    let res1 = fixture
        .shell
        .consume_stack_integration_effect(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            &fixture.plan,
            |dispatch, _, retained_pub| {
                assert_eq!(dispatch, EffectDispatch::FirstDispatch);
                assert_eq!(retained_pub, &publication);
                Ok(Some(partial.clone()))
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(res1.observed, Some(StackEffectResult::Integration(partial)));

    let complete = StackIntegration {
        schema: STACK_INTEGRATION_SCHEMA.to_string(),
        stack,
        layers: vec![layer1, layer2],
    };
    let res2 = fixture
        .shell
        .consume_stack_integration_effect(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            &fixture.plan,
            |dispatch, _, _| {
                assert_eq!(dispatch, EffectDispatch::ReconcileOnly);
                Ok(Some(complete.clone()))
            },
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        res2.observed,
        Some(StackEffectResult::Integration(complete.clone()))
    );

    // Complete stack integration never calls transport again.
    let res3 = fixture
        .shell
        .consume_stack_integration_effect(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            &fixture.plan,
            |_, _, _| unreachable!("complete stack integration must not transport"),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(res3, res2);

    let record = fixture
        .shell
        .stack_publication_record(&fixture.attempt.identity)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert_eq!(record.publication, Some(publication));
    assert_eq!(record.integration, Some(complete));
}

#[test]
fn absent_authority_refuses_stack_operations_before_persisting() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("state.db");
    let fixture = verified_fixture(&path);
    drop(fixture.shell);
    let mut absent = Implementer::open(&path, enrollment()).unwrap_or_else(|_| unreachable!());
    absent.set_clock_ms(10);
    let error = absent
        .prepare_stack_publication(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            10,
            &fixture.layer_verifications,
            &fixture.plan,
        )
        .err()
        .unwrap_or_else(|| unreachable!());
    assert!(error.contains("no authority profile selected"), "{error}");
    assert_eq!(
        absent
            .stack_publication_record(&fixture.attempt.identity)
            .unwrap_or_else(|_| unreachable!()),
        None
    );
}

#[test]
fn prepare_publication_rejects_stack_plan_committed_while_waiting_for_write_lock() {
    use kiln_core::implementation::authority::{
        AttemptCurrentness, AuthorityDecision, AuthorityProfile, AuthorityRequest,
        AuthoritySelection, AuthoritySessionId, LocalOperatorAuthority, ProtectedAuthority,
    };
    use std::sync::mpsc;

    struct SignalingAuthority {
        inner: LocalOperatorAuthority,
        on_revalidate: Option<mpsc::Sender<()>>,
    }

    impl ProtectedAuthority for SignalingAuthority {
        fn admit(
            &mut self,
            request: &AuthorityRequest,
            profile: &AuthorityProfile,
            currentness: AttemptCurrentness,
            now_unix_s: u64,
        ) -> Result<AuthorityDecision, AuthorityDecision> {
            self.inner.admit(request, profile, currentness, now_unix_s)
        }

        fn revalidate(
            &mut self,
            session: AuthoritySessionId,
            request: &AuthorityRequest,
            currentness: AttemptCurrentness,
            now_unix_s: u64,
        ) -> Result<(), AuthorityDecision> {
            let result = self
                .inner
                .revalidate(session, request, currentness, now_unix_s);
            if let Some(sender) = self.on_revalidate.take() {
                let _ = sender.send(());
            }
            result
        }

        fn close(&mut self, session: AuthoritySessionId) {
            self.inner.close(session);
        }

        fn decisions(&self) -> &[AuthorityDecision] {
            self.inner.decisions()
        }
    }

    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = dir.path().join("state.db");
    let mut fixture = verified_fixture(&path);
    let (locked_tx, locked_rx) = mpsc::channel();
    let (revalidated_tx, revalidated_rx) = mpsc::channel();
    let local = LocalOperatorAuthority::new("test-operator", false)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    fixture.shell.set_authority(
        Box::new(SignalingAuthority {
            inner: local,
            on_revalidate: Some(revalidated_tx),
        }),
        AuthoritySelection::LocalOperator {
            operator: "test-operator".to_string(),
        },
    );
    let single_pr = PublicationPlan {
        schema: PUBLICATION_SCHEMA.to_string(),
        work_identity: fixture.plan.work_identity.clone(),
        work_revision: fixture.plan.work_revision,
        attempt: fixture.plan.attempt.clone(),
        fence: fixture.plan.fence,
        handoff: fixture.plan.handoff.clone(),
        verification: fixture.plan.verification.clone(),
        repository: fixture.plan.repository.clone(),
        base: fixture.plan.base.clone(),
        base_branch: "main".to_string(),
        candidate: fixture.handoff.candidate.clone(),
        tree: fixture.handoff.tree.clone(),
        branch: branch_for(&fixture.plan.work_identity, &fixture.plan.attempt),
        title: "Single PR".to_string(),
        body: "Races with stack".to_string(),
    };

    let stack = stack_identity(&fixture.plan).unwrap_or_else(|_| unreachable!());
    let retained = RetainedStackPlan {
        stack,
        plan: fixture.plan.clone(),
        layer_verifications: fixture.layer_verifications.clone(),
    };
    let key = plan_key(&fixture.attempt.identity);
    let bytes = serde_json::to_vec(&retained).unwrap_or_else(|error| unreachable!("{error}"));

    let result = std::thread::scope(|scope| {
        let handle = scope.spawn(move || {
            let holder =
                rusqlite::Connection::open(&path).unwrap_or_else(|error| unreachable!("{error}"));
            holder
                .busy_timeout(std::time::Duration::from_secs(10))
                .unwrap_or_else(|error| unreachable!("{error}"));
            holder
                .execute_batch("BEGIN IMMEDIATE;")
                .unwrap_or_else(|error| unreachable!("{error}"));
            locked_tx
                .send(())
                .unwrap_or_else(|error| unreachable!("{error}"));
            revalidated_rx
                .recv()
                .unwrap_or_else(|error| unreachable!("{error}"));
            std::thread::sleep(std::time::Duration::from_millis(25));
            holder
                .execute(
                    "INSERT INTO implementation_state (kind, version, bytes) VALUES (?1, 1, ?2)",
                    rusqlite::params![key, bytes],
                )
                .unwrap_or_else(|error| unreachable!("{error}"));
            holder
                .execute_batch("COMMIT;")
                .unwrap_or_else(|error| unreachable!("{error}"));
        });
        locked_rx
            .recv()
            .unwrap_or_else(|error| unreachable!("{error}"));
        let outcome = fixture.shell.prepare_publication(
            &fixture.work,
            &fixture.attempt.identity,
            fixture.attempt.fence,
            12,
            &fixture.handoff,
            &fixture.verification,
            &single_pr,
        );
        handle.join().unwrap_or_else(|_| unreachable!());
        outcome
    });

    assert_eq!(
        result.err().as_deref(),
        Some("single-PR publication conflicts with retained stack publication")
    );
    assert_eq!(
        fixture
            .shell
            .publication_record(&fixture.attempt.identity)
            .unwrap_or_else(|_| unreachable!()),
        None
    );
}
