//! Holder seeds scoped to each entry point's roles under the verified
//! profile: a process loads only the seeds of the roles its own protected
//! operations need, and refuses an operation of any other role before it is
//! signed, verified, recorded or a credential is used.

#[path = "authority_common/mod.rs"]
mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use common::*;
use kiln_adapter_basalt::test_support::Fixture;
use kiln_core::change_review::{CiCheck, CiFact};
use kiln_core::implementation::authority::{
    AuthorityDecision, AuthorityProvenance, ENTRY_POINTS, IMPLEMENTER_CLI, ProtectedOperation,
    entry_point_roles,
};
use kiln_core::implementation::grant::GrantRole;
use kiln_core::implementation::{
    CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence, CandidateHandoff, CandidateVerification,
    EvidenceKind, EvidenceOutcome, ImplementationAttempt, ImplementationWork, PUBLICATION_SCHEMA,
    PublicationPlan, VERIFICATION_SCHEMA, admit_candidate_handoff, admit_candidate_verification,
    branch_for,
};
use kiln_core::types::{ExternalRef, ProviderProfileId};
use kiln_implementer::Implementer;
use kiln_implementer::authority_config::ImplementationAuthorityConfig;
use kiln_implementer::authority_state::{SqliteReplayLedger, authority_state_path};

const LEASE_MS: u64 = 400_000;

fn ok<V>(result: Result<V, String>) -> V {
    result.unwrap_or_else(|error| unreachable!("{error}"))
}

fn reference(kind: &str, id: &str) -> ExternalRef {
    ExternalRef::new("lattice", kind, id).unwrap_or_else(|_| unreachable!())
}

fn roles(roles: &[GrantRole]) -> BTreeSet<GrantRole> {
    roles.iter().copied().collect()
}

/// Open the ledger as one entry point: the verified profile with only the
/// holder seeds of `scope`.
fn open_scoped(db: &Path, config: &Path, scope: &[GrantRole]) -> Result<Implementer, String> {
    let mut i = Implementer::open(db, enrollment())?;
    i.set_clock_ms(T);
    i.select_authority(&ImplementationAuthorityConfig::load(config)?, &roles(scope))?;
    Ok(i)
}

fn replay_rows(db: &Path) -> u64 {
    ok(authority_state_path(db)
        .and_then(|path| SqliteReplayLedger::open(&path))
        .and_then(|ledger| ledger.recorded()))
}

fn decisions(db: &Path) -> Vec<AuthorityDecision> {
    ok(Implementer::open(db, enrollment())
        .and_then(|i| i.persisted_authority_decisions(0..u64::MAX)))
    .into_iter()
    .map(|(_, decision)| decision)
    .collect()
}

/// Remove one role's seed file, as on a host where that role's user owns it.
fn remove_seed(dir: &Path, name: &str) {
    std::fs::remove_file(dir.join(format!("{name}.seed")))
        .unwrap_or_else(|error| unreachable!("{error}"));
}

/// A claimed, started attempt with an admitted candidate handoff waiting for
/// verification, prepared by the coordinator role.
struct Candidate {
    dir: tempfile::TempDir,
    db: PathBuf,
    config: PathBuf,
    work: ImplementationWork,
    attempt: ImplementationAttempt,
    handoff: CandidateHandoff,
    verification: CandidateVerification,
}

fn candidate(name: &str, before: impl FnOnce(&Path)) -> Candidate {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let config = write_config(dir.path(), &Fixture::direct(), &[]);
    before(dir.path());
    let mut work = work(name);
    work.base = Some(reference("commit", "base"));
    let mut i = ok(open_scoped(&db, &config, &[GrantRole::Coding]));
    ok(i.admit(&work, None));
    let attempt = ok(i.claim_next(T, LEASE_MS)).unwrap_or_else(|| unreachable!("claim"));
    ok(i.reserve_dispatch(
        &attempt.identity,
        attempt.fence,
        T,
        &id("dispatch-plan"),
        &ProviderProfileId::new("lattice-implementation").unwrap_or_else(|_| unreachable!()),
    ));
    ok(i.start_execution(
        &attempt.identity,
        attempt.fence,
        T + 1,
        &reference("run", "one"),
    ));
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
        toolchain: id("toolchain"),
        environment: id("environment"),
        evidence: vec![CandidateEvidence {
            identity: id("evidence"),
            kind: EvidenceKind::Check,
            outcome: EvidenceOutcome::Passed,
            reference: reference("run", "candidate-check"),
        }],
    };
    // The capture role's operation: candidate submission, a coordinator role.
    ok(
        i.admit_candidate_handoff(&work, &attempt.identity, attempt.fence, T + 2, &handoff)
            .map(drop),
    );
    ok(i.begin_verification(&attempt.identity, attempt.fence, T + 3));
    drop(i);
    let handoff_identity =
        admit_candidate_handoff(&work, &attempt, &handoff).unwrap_or_else(|_| unreachable!());
    let verification = CandidateVerification {
        schema: VERIFICATION_SCHEMA.to_string(),
        handoff: handoff_identity,
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
    Candidate {
        dir,
        db,
        config,
        work,
        attempt,
        handoff,
        verification,
    }
}

impl Candidate {
    fn publication(&self) -> PublicationPlan {
        let handoff = admit_candidate_handoff(&self.work, &self.attempt, &self.handoff)
            .unwrap_or_else(|_| unreachable!());
        let verification = admit_candidate_verification(
            &self.work,
            &self.attempt,
            &self.handoff,
            &self.verification,
        )
        .unwrap_or_else(|_| unreachable!());
        PublicationPlan {
            schema: PUBLICATION_SCHEMA.to_string(),
            work_identity: self.handoff.work_identity.clone(),
            work_revision: self.handoff.work_revision,
            attempt: self.attempt.identity.clone(),
            fence: self.attempt.fence,
            handoff,
            verification,
            repository: self.work.repository.clone(),
            base: self.work.base.clone().unwrap_or_else(|| unreachable!()),
            base_branch: "main".to_string(),
            candidate: self.handoff.candidate.clone(),
            tree: self.handoff.tree.clone(),
            branch: branch_for(&self.handoff.work_identity, &self.attempt.identity),
            title: "Implement".to_string(),
            body: "Verified".to_string(),
        }
    }

    fn record_verification(&self, i: &mut Implementer) -> Result<(), String> {
        i.record_verification(
            &self.work,
            &self.attempt.identity,
            self.attempt.fence,
            T + 4,
            &self.handoff,
            &self.verification,
        )
        .map(drop)
    }

    fn prepare_publication(&self, i: &mut Implementer) -> Result<(), String> {
        i.prepare_publication(
            &self.work,
            &self.attempt.identity,
            self.attempt.fence,
            T + 5,
            &self.handoff,
            &self.verification,
            &self.publication(),
        )
        .map(drop)
    }
}

#[test]
// r[verify kiln.implementation.authority.selection.scenario.scoped]
fn entry_points_derive_their_holder_roles_from_the_reviewed_table() {
    let cli = |command| entry_point_roles(ENTRY_POINTS, IMPLEMENTER_CLI, command);
    assert_eq!(cli("capture-candidate"), roles(&[GrantRole::Coding]));
    assert_eq!(cli("reserve-plan"), roles(&[GrantRole::Coding]));
    assert_eq!(cli("start"), roles(&[GrantRole::Coding]));
    assert_eq!(cli("publish"), roles(&[GrantRole::Publication]));
    assert_eq!(
        cli("prepare-stack-publication"),
        roles(&[GrantRole::Publication])
    );
    assert_eq!(cli("publish-stack"), roles(&[GrantRole::Publication]));
    assert_eq!(
        cli("recover-stack-integration"),
        roles(&[GrantRole::Publication])
    );
    // Observation and operator configuration load no holder seed.
    assert!(cli("stack-gate").is_empty());
    assert!(cli("admission-explanation").is_empty());
    assert!(cli("status").is_empty());
    assert!(cli("drain").is_empty());
}

#[test]
// r[verify kiln.implementation.authority.selection.scenario.scoped]
fn the_coordinator_role_works_without_the_other_seeds_and_cannot_verify_or_publish() {
    // The verifier's seed is unreadable to this user and the publisher's is
    // absent: the coordinator process never opens either.
    let prepared = candidate("coordinator-only", |dir| {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.join("verifier.seed"),
            std::fs::Permissions::from_mode(0o000),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        remove_seed(dir, "publisher");
    });
    let rows = replay_rows(&prepared.db);
    let persisted = decisions(&prepared.db).len();
    // Claim, reservation, start and the capture role's candidate submission.
    assert_eq!(rows, 4);

    let mut worker = ok(open_scoped(
        &prepared.db,
        &prepared.config,
        &[GrantRole::Coding],
    ));
    for (operation, refused) in [
        (
            ProtectedOperation::VerificationAdmit,
            prepared.record_verification(&mut worker),
        ),
        (
            ProtectedOperation::PublicationPublish,
            prepared.prepare_publication(&mut worker),
        ),
    ] {
        let error = refused
            .err()
            .unwrap_or_else(|| unreachable!("{operation:?} must be refused"));
        assert!(
            error.starts_with("authority: holder-not-loaded"),
            "{operation:?}: {error}"
        );
    }
    assert!(ok(worker.verification_receipt(&prepared.attempt.identity)).is_none());
    assert!(ok(worker.publication_record(&prepared.attempt.identity)).is_none());
    drop(worker);
    // Nothing was signed, asked of the verifier or recorded.
    assert_eq!(replay_rows(&prepared.db), rows);
    assert_eq!(decisions(&prepared.db).len(), persisted);
}

#[test]
// r[verify kiln.implementation.authority.selection.scenario.scoped]
fn the_verifier_role_verifies_with_only_its_own_seed() {
    let prepared = candidate("verifier-only", |_| {});
    remove_seed(prepared.dir.path(), "coordinator");
    remove_seed(prepared.dir.path(), "publisher");
    // A role that needs a removed seed cannot even select the profile.
    assert!(open_scoped(&prepared.db, &prepared.config, &[GrantRole::Coding]).is_err());

    let mut verifier = ok(open_scoped(
        &prepared.db,
        &prepared.config,
        &[GrantRole::Verification],
    ));
    ok(prepared.record_verification(&mut verifier));
    assert!(ok(verifier.verification_receipt(&prepared.attempt.identity)).is_some());
    // A coordinator operation is refused before it changes anything.
    let rows = replay_rows(&prepared.db);
    let error = verifier
        .cancel(
            &prepared.attempt.identity,
            prepared.attempt.fence,
            T + 5,
            "not the verifier's",
        )
        .err()
        .unwrap_or_else(|| unreachable!("the verifier holds no coordinator key"));
    assert!(error.starts_with("authority: holder-not-loaded"), "{error}");
    drop(verifier);
    assert_eq!(replay_rows(&prepared.db), rows);
    let last = decisions(&prepared.db)
        .into_iter()
        .rev()
        .find(|decision| decision.operation == ProtectedOperation::VerificationAdmit)
        .unwrap_or_else(|| unreachable!("verification decision"));
    assert!(last.allowed);
    assert_eq!(last.provenance, AuthorityProvenance::VerifiedUcan);
}

#[test]
// r[verify kiln.implementation.authority.selection.scenario.scoped]
fn the_publisher_role_owns_stack_publication_and_other_roles_cannot_invoke_it() {
    use kiln_core::implementation::{
        IntegrationMapping, LAYER_VERIFICATION_SCHEMA, LayerVerification, STACK_INTEGRATION_SCHEMA,
        STACK_PUBLICATION_SCHEMA, STACK_SCHEMA, StackIntegration, StackLayerIntegration,
        StackLayerPlan, StackPlan, StackPublication, admit_layer_verification, layer_branch,
        stack_identity,
    };

    let git = |kind: &str, ch: char| {
        ExternalRef::new("git", kind, &ch.to_string().repeat(40)).unwrap_or_else(|_| unreachable!())
    };
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let config = write_config(dir.path(), &Fixture::direct(), &[]);
    let mut work = work("stack-roles");
    work.base = Some(git("commit", 'b'));
    work.terminal_policy = kiln_core::implementation::TerminalPolicy::VerifiedIntegrated;
    let mut coordinator = ok(open_scoped(&db, &config, &[GrantRole::Coding]));
    ok(coordinator.admit(&work, None));
    let attempt = ok(coordinator.claim_next(T, LEASE_MS)).unwrap_or_else(|| unreachable!());
    ok(coordinator.reserve_dispatch(
        &attempt.identity,
        attempt.fence,
        T,
        &id("dispatch-plan"),
        &ProviderProfileId::new("lattice-implementation").unwrap_or_else(|_| unreachable!()),
    ));
    ok(coordinator.start_execution(
        &attempt.identity,
        attempt.fence,
        T + 1,
        &reference("run", "one"),
    ));
    let work_identity = work.identity().unwrap_or_else(|_| unreachable!());
    let handoff = CandidateHandoff {
        schema: CANDIDATE_HANDOFF_SCHEMA.to_string(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        candidate: git("commit", '2'),
        tree: git("tree", 'e'),
        changed_paths: vec!["crates/src/lib.rs".to_string()],
        acceptance_digest: kiln_core::implementation::acceptance_digest(&work)
            .unwrap_or_else(|_| unreachable!()),
        verification_profile: work.verification_profile.clone(),
        toolchain: id("toolchain"),
        environment: id("environment"),
        evidence: vec![CandidateEvidence {
            identity: id("evidence"),
            kind: EvidenceKind::Check,
            outcome: EvidenceOutcome::Passed,
            reference: reference("run", "candidate-check"),
        }],
    };
    ok(coordinator
        .admit_candidate_handoff(&work, &attempt.identity, attempt.fence, T + 2, &handoff)
        .map(drop));
    ok(coordinator.begin_verification(&attempt.identity, attempt.fence, T + 3));
    drop(coordinator);

    let handoff_id =
        admit_candidate_handoff(&work, &attempt, &handoff).unwrap_or_else(|_| unreachable!());
    let ci = |rev: &str| CiFact {
        project: work.project.clone(),
        repository: work.repository.clone(),
        revision: rev.to_string(),
        run_id: format!("run-{rev}"),
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
    };
    let verification = CandidateVerification {
        schema: VERIFICATION_SCHEMA.to_string(),
        handoff: handoff_id.clone(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        candidate: handoff.candidate.clone(),
        tree: handoff.tree.clone(),
        base: handoff.base.clone(),
        verification_profile: work.verification_profile.clone(),
        run: ci(&"2".repeat(40)),
    };
    let mut verifier = ok(open_scoped(&db, &config, &[GrantRole::Verification]));
    ok(verifier.record_verification(
        &work,
        &attempt.identity,
        attempt.fence,
        T + 4,
        &handoff,
        &verification,
    ));
    let verified_attempt =
        ok(verifier.attempt(&attempt.identity)).unwrap_or_else(|| unreachable!());
    let lower = LayerVerification {
        schema: LAYER_VERIFICATION_SCHEMA.to_string(),
        handoff: handoff_id.clone(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        position: 1,
        candidate: git("commit", '1'),
        tree: git("tree", 'd'),
        parent: git("commit", 'b'),
        verification_profile: work.verification_profile.clone(),
        run: ci(&"1".repeat(40)),
    };
    let lower_id = admit_layer_verification(&work, &verified_attempt, &handoff, &lower)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let top_id = admit_candidate_verification(&work, &verified_attempt, &handoff, &verification)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let branch1 = layer_branch(&work_identity, &attempt.identity, 1);
    let branch2 = layer_branch(&work_identity, &attempt.identity, 2);
    let plan = StackPlan {
        schema: STACK_SCHEMA.to_string(),
        work_identity,
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        handoff: handoff_id,
        verification: top_id.clone(),
        repository: work.repository.clone(),
        base: git("commit", 'b'),
        trunk_branch: "main".to_string(),
        layers: vec![
            StackLayerPlan {
                position: 1,
                candidate: git("commit", '1'),
                tree: git("tree", 'd'),
                parent: git("commit", 'b'),
                verification: lower_id,
                branch: branch1.clone(),
                base_branch: "main".to_string(),
                title: "Layer 1".to_string(),
                body: "First".to_string(),
            },
            StackLayerPlan {
                position: 2,
                candidate: git("commit", '2'),
                tree: git("tree", 'e'),
                parent: git("commit", '1'),
                verification: top_id,
                branch: branch2,
                base_branch: branch1,
                title: "Layer 2".to_string(),
                body: "Second".to_string(),
            },
        ],
    };

    // Verifier cannot prepare stack publication.
    let err = verifier
        .prepare_stack_publication(
            &work,
            &attempt.identity,
            attempt.fence,
            T + 5,
            std::slice::from_ref(&lower),
            &plan,
        )
        .err()
        .unwrap_or_else(|| unreachable!("verifier must not hold publisher key"));
    assert!(err.starts_with("authority: holder-not-loaded"), "{err}");
    drop(verifier);

    // Coding role cannot prepare stack publication either.
    let mut coding = ok(open_scoped(&db, &config, &[GrantRole::Coding]));
    let err = coding
        .prepare_stack_publication(
            &work,
            &attempt.identity,
            attempt.fence,
            T + 5,
            std::slice::from_ref(&lower),
            &plan,
        )
        .err()
        .unwrap_or_else(|| unreachable!("coding must not hold publisher key"));
    assert!(err.starts_with("authority: holder-not-loaded"), "{err}");
    drop(coding);

    // Publisher works even with coordinator and verifier seeds removed.
    remove_seed(dir.path(), "coordinator");
    remove_seed(dir.path(), "verifier");
    let mut publisher = ok(open_scoped(&db, &config, &[GrantRole::Publication]));
    let stack = ok(publisher.prepare_stack_publication(
        &work,
        &attempt.identity,
        attempt.fence,
        T + 5,
        &[lower],
        &plan,
    ));
    assert_eq!(
        stack,
        stack_identity(&plan).unwrap_or_else(|_| unreachable!())
    );
    let github_ref = |kind: &str, n: u64| {
        ExternalRef::new("github", kind, &format!("{}#{n}", work.repository))
            .unwrap_or_else(|_| unreachable!())
    };
    let pub_record = StackPublication {
        schema: STACK_PUBLICATION_SCHEMA.to_string(),
        stack: stack.clone(),
        provider_stack: github_ref("stack", 5),
        pull_requests: vec![
            github_ref("pull_request", 11),
            github_ref("pull_request", 12),
        ],
    };
    ok(publisher.consume_stack_publication_effect(
        &work,
        &attempt.identity,
        attempt.fence,
        &plan,
        |_, _| Ok(Some(pub_record.clone())),
    ));
    let integration = StackIntegration {
        schema: STACK_INTEGRATION_SCHEMA.to_string(),
        stack,
        layers: vec![StackLayerIntegration {
            position: 1,
            pull_request: github_ref("pull_request", 11),
            candidate: git("commit", '1'),
            merge_commit: git("commit", 'c'),
            mapping: IntegrationMapping {
                expected_base: git("commit", 'b'),
                candidate_tree: git("tree", 'd'),
                landed_tree: git("tree", 'd'),
            },
        }],
    };
    ok(publisher.consume_stack_integration_effect(
        &work,
        &attempt.identity,
        attempt.fence,
        &plan,
        |_, _, _| Ok(Some(integration)),
    ));
}

#[test]
// r[verify kiln.implementation.authority.selection.scenario.scoped]
fn the_publisher_role_owns_publication_route_probe_and_other_roles_fail_before_transport() {
    use kiln_core::implementation::{
        LAYER_VERIFICATION_SCHEMA, LayerVerification, STACK_SCHEMA, StackLayerPlan, StackPlan,
        admit_layer_verification, layer_branch,
    };

    let git = |kind: &str, ch: char| {
        ExternalRef::new("git", kind, &ch.to_string().repeat(40)).unwrap_or_else(|_| unreachable!())
    };
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let db = dir.path().join("ledger.db");
    let fixture = Fixture::direct();
    let config = write_config(dir.path(), &fixture, &[]);
    let mut work = work("probe-roles");
    work.base = Some(git("commit", 'b'));
    work.terminal_policy = kiln_core::implementation::TerminalPolicy::VerifiedIntegrated;
    let mut coordinator = ok(open_scoped(&db, &config, &[GrantRole::Coding]));
    ok(coordinator.admit(&work, None));
    let attempt = ok(coordinator.claim_next(T, LEASE_MS)).unwrap_or_else(|| unreachable!());
    ok(coordinator.reserve_dispatch(
        &attempt.identity,
        attempt.fence,
        T,
        &id("dispatch-plan"),
        &ProviderProfileId::new("lattice-implementation").unwrap_or_else(|_| unreachable!()),
    ));
    ok(coordinator.start_execution(
        &attempt.identity,
        attempt.fence,
        T + 1,
        &reference("run", "one"),
    ));
    let work_identity = work.identity().unwrap_or_else(|_| unreachable!());
    let handoff = CandidateHandoff {
        schema: CANDIDATE_HANDOFF_SCHEMA.to_string(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        candidate: git("commit", '2'),
        tree: git("tree", 'e'),
        changed_paths: vec!["crates/src/lib.rs".to_string()],
        acceptance_digest: kiln_core::implementation::acceptance_digest(&work)
            .unwrap_or_else(|_| unreachable!()),
        verification_profile: work.verification_profile.clone(),
        toolchain: id("toolchain"),
        environment: id("environment"),
        evidence: vec![CandidateEvidence {
            identity: id("evidence"),
            kind: EvidenceKind::Check,
            outcome: EvidenceOutcome::Passed,
            reference: reference("run", "candidate-check"),
        }],
    };
    ok(coordinator
        .admit_candidate_handoff(&work, &attempt.identity, attempt.fence, T + 2, &handoff)
        .map(drop));
    ok(coordinator.begin_verification(&attempt.identity, attempt.fence, T + 3));
    drop(coordinator);

    let handoff_id =
        admit_candidate_handoff(&work, &attempt, &handoff).unwrap_or_else(|_| unreachable!());
    let ci = |rev: &str| CiFact {
        project: work.project.clone(),
        repository: work.repository.clone(),
        revision: rev.to_string(),
        run_id: format!("run-{rev}"),
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
    };
    let verification = CandidateVerification {
        schema: VERIFICATION_SCHEMA.to_string(),
        handoff: handoff_id.clone(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        candidate: handoff.candidate.clone(),
        tree: handoff.tree.clone(),
        base: handoff.base.clone(),
        verification_profile: work.verification_profile.clone(),
        run: ci(&"2".repeat(40)),
    };
    let mut verifier = ok(open_scoped(&db, &config, &[GrantRole::Verification]));
    ok(verifier.record_verification(
        &work,
        &attempt.identity,
        attempt.fence,
        T + 4,
        &handoff,
        &verification,
    ));
    let verified_attempt =
        ok(verifier.attempt(&attempt.identity)).unwrap_or_else(|| unreachable!());
    let lower = LayerVerification {
        schema: LAYER_VERIFICATION_SCHEMA.to_string(),
        handoff: handoff_id.clone(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        position: 1,
        candidate: git("commit", '1'),
        tree: git("tree", 'd'),
        parent: git("commit", 'b'),
        verification_profile: work.verification_profile.clone(),
        run: ci(&"1".repeat(40)),
    };
    let lower_id = admit_layer_verification(&work, &verified_attempt, &handoff, &lower)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let top_id = admit_candidate_verification(&work, &verified_attempt, &handoff, &verification)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let branch1 = layer_branch(&work_identity, &attempt.identity, 1);
    let branch2 = layer_branch(&work_identity, &attempt.identity, 2);
    let plan = StackPlan {
        schema: STACK_SCHEMA.to_string(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        handoff: handoff_id,
        verification: top_id.clone(),
        repository: work.repository.clone(),
        base: git("commit", 'b'),
        trunk_branch: "main".to_string(),
        layers: vec![
            StackLayerPlan {
                position: 1,
                candidate: git("commit", '1'),
                tree: git("tree", 'd'),
                parent: git("commit", 'b'),
                verification: lower_id,
                branch: branch1.clone(),
                base_branch: "main".to_string(),
                title: "Layer 1".to_string(),
                body: "First".to_string(),
            },
            StackLayerPlan {
                position: 2,
                candidate: git("commit", '2'),
                tree: git("tree", 'e'),
                parent: git("commit", '1'),
                verification: top_id,
                branch: branch2,
                base_branch: branch1,
                title: "Layer 2".to_string(),
                body: "Second".to_string(),
            },
        ],
    };

    let mut publisher = ok(open_scoped(&db, &config, &[GrantRole::Publication]));
    ok(publisher.prepare_stack_publication(
        &work,
        &attempt.identity,
        attempt.fence,
        T + 5,
        &[lower],
        &plan,
    ));
    drop(publisher);

    let rows = replay_rows(&db);
    let persisted = decisions(&db).len();

    // Non-publication roles fail before transport with zero replay/decision mutations.
    for role in [GrantRole::Coding, GrantRole::Verification] {
        let mut scoped = ok(open_scoped(&db, &config, &[role]));
        let err = scoped
            .probe_publication_route(
                &work_identity,
                &attempt.identity,
                attempt.fence,
                &work.repository,
                || unreachable!("non-publisher role must never invoke probe transport"),
            )
            .err()
            .unwrap_or_else(|| unreachable!("{role:?} must be refused"));
        assert!(
            err.starts_with("authority: holder-not-loaded"),
            "{role:?}: {err}"
        );
    }
    assert_eq!(replay_rows(&db), rows);
    assert_eq!(decisions(&db).len(), persisted);

    // Absent authority fails before transport with zero replay/decision mutations.
    let mut absent = ok(Implementer::open(&db, enrollment()));
    absent.set_clock_ms(T);
    let err = absent
        .probe_publication_route(
            &work_identity,
            &attempt.identity,
            attempt.fence,
            &work.repository,
            || unreachable!("absent authority must never invoke probe transport"),
        )
        .err()
        .unwrap_or_else(|| unreachable!("absent authority must be refused"));
    assert!(err.contains("no authority profile selected"), "{err}");
    assert_eq!(replay_rows(&db), rows);
    assert_eq!(decisions(&db).len(), persisted);

    // Stale fence fails before transport and without consuming replay rows.
    let mut publisher = ok(open_scoped(&db, &config, &[GrantRole::Publication]));
    let err = publisher
        .probe_publication_route(
            &work_identity,
            &attempt.identity,
            attempt.fence + 1,
            &work.repository,
            || unreachable!("stale fence must never invoke probe transport"),
        )
        .err()
        .unwrap_or_else(|| unreachable!("stale fence must be refused"));
    assert!(err.contains("implementation_fence_stale"), "{err}");
    assert_eq!(replay_rows(&db), rows);
    assert_eq!(decisions(&db).len(), persisted);
    drop(publisher);

    // Stale revocation view fails before transport.
    let mut stale_publisher = ok(open_scoped(&db, &config, &[GrantRole::Publication]));
    stale_publisher.set_clock_ms(T + MAX_AGE_MS + 1_000);
    let err = stale_publisher
        .probe_publication_route(
            &work_identity,
            &attempt.identity,
            attempt.fence,
            &work.repository,
            || unreachable!("stale revocation view must never invoke probe transport"),
        )
        .err()
        .unwrap_or_else(|| unreachable!("stale revocation view must be refused"));
    assert!(err.starts_with("authority:"), "{err}");
    drop(stale_publisher);

    // Authorized GrantRole::Publication with coordinator/verifier seeds removed succeeds
    // and leaves stack_publication_record / read_stack_effect unmodified.
    remove_seed(dir.path(), "coordinator");
    remove_seed(dir.path(), "verifier");
    let mut authorized = ok(open_scoped(&db, &config, &[GrantRole::Publication]));
    let mut probed = false;
    let supported = ok(authorized.probe_publication_route(
        &work_identity,
        &attempt.identity,
        attempt.fence,
        &work.repository,
        || {
            probed = true;
            Ok(true)
        },
    ));
    assert!(probed);
    assert!(supported);
    let record = ok(authorized.stack_publication_record(&attempt.identity))
        .unwrap_or_else(|| unreachable!("retained stack plan"));
    assert!(record.publication.is_none());
    assert!(
        ok(authorized.read_stack_effect(
            &work,
            &attempt.identity,
            attempt.fence,
            kiln_implementer::stack_publication::StackEffectKind::Publish,
        ))
        .is_none()
    );
    let allowed = decisions(&db)
        .into_iter()
        .rev()
        .find(|d| d.operation == ProtectedOperation::PublicationPublish && d.allowed)
        .unwrap_or_else(|| unreachable!("allowed probe decision"));
    assert_eq!(allowed.provenance, AuthorityProvenance::VerifiedUcan);
    drop(authorized);

    // Revoked Publication grant fails before transport and records denial.
    let revoked_dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let revoked_grant =
        kiln_adapter_basalt::test_support::token_ref(fixture.grant(GrantRole::Publication));
    let revoked_config = write_config(revoked_dir.path(), &fixture, &[revoked_grant]);
    let mut revoked_publisher = ok(open_scoped(&db, &revoked_config, &[GrantRole::Publication]));
    let before_revoked = decisions(&db).len();
    let err = revoked_publisher
        .probe_publication_route(
            &work_identity,
            &attempt.identity,
            attempt.fence,
            &work.repository,
            || unreachable!("revoked grant must never invoke probe transport"),
        )
        .err()
        .unwrap_or_else(|| unreachable!("revoked grant must be refused"));
    assert!(err.starts_with("authority:"), "{err}");
    let denied = decisions(&db)
        .into_iter()
        .skip(before_revoked)
        .last()
        .unwrap_or_else(|| unreachable!("revoked probe decision"));
    assert!(!denied.allowed);
    assert_eq!(denied.operation, ProtectedOperation::PublicationPublish);
}
