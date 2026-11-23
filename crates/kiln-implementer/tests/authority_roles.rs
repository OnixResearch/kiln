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
    ok(SqliteReplayLedger::open(&authority_state_path(db)).and_then(|ledger| ledger.recorded()))
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
    // Observation and operator configuration load no holder seed.
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
