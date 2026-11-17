//! Real controller execution: a Completed worker is not a verified pass.
use super::*;
use kiln_core::implementation::{
    AcceptanceObligation, AttemptPhase, AttemptUsage, CandidateEvidence, Enrollment, EvidenceKind,
    EvidenceOutcome, IMPLEMENTATION_WORK_SCHEMA, ModelTurnObservation, SourceRef, TerminalPolicy,
    VerificationOutcome, WorkBudget, WorkerObservation, WorkerTerminalState, acceptance_digest,
};
use kiln_implementer::verification::{
    FrozenProgram, VerificationCheck, VerificationProfile, VerifierAuthority,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn frozen(variable: &str) -> TestResult<FrozenProgram> {
    let path = std::fs::canonicalize(std::env::var_os(variable).ok_or(variable.to_owned())?)?;
    let blake3 = blake3::hash(&std::fs::read(&path)?).to_hex().to_string();
    Ok(FrozenProgram { path, blake3 })
}

#[test]
#[ignore = "requires KILN_VERIFICATION_TEST_ROOT/GIT/SHELL private Linux Nix cohort"]
// r[verify kiln.implementation.outcome_metrics.scenario.authority]
fn completed_worker_keeps_real_controller_outcomes_separate_after_restart() -> TestResult {
    for expected in [
        VerificationOutcome::Verified,
        VerificationOutcome::Rejected,
        VerificationOutcome::Unknown,
    ] {
        let root = tempfile::Builder::new()
            .prefix("kiln-outcomes-")
            .tempdir_in(
                std::env::var_os("KILN_VERIFICATION_TEST_ROOT")
                    .ok_or("private test root is required")?,
            )?;
        let (root, mut engine, source) = engine_in(root);
        let repository_path = engine
            .profile
            .projects
            .first()
            .ok_or("project missing")?
            .repository_path
            .clone();
        let repository = git2::Repository::open(&repository_path)?;
        let parent = repository.find_commit(git2::Oid::from_str(
            source.exact_revision.native_id.as_str(),
        )?)?;
        let tree = parent.tree()?;
        let blob = repository.blob(b"candidate source\n")?;
        let mut builder = repository.treebuilder(Some(&tree))?;
        builder.insert("source.txt", blob, 0o100644)?;
        let candidate_tree = repository.find_tree(builder.write()?)?;
        let signature =
            git2::Signature::new("fixture", "fixture@example.invalid", &git2::Time::new(0, 0))?;
        let candidate = repository.commit(
            Some("refs/heads/candidate"),
            &signature,
            &signature,
            "candidate",
            &candidate_tree,
            &[&parent],
        )?;
        repository.set_head("refs/heads/main")?;
        let command = match expected {
            VerificationOutcome::Verified => "printf observed > observed; exit 0",
            VerificationOutcome::Rejected => "printf observed > observed; exit 1",
            _ => "printf observed > observed; while :; do :; done",
        };
        let mut profile = VerificationProfile {
            schema: "kiln.verification-profile.v1".into(),
            repository: "fixture/repository".into(),
            toolchain: content('b'),
            environment_identity: content('c'),
            environment: BTreeMap::new(),
            checks: vec![VerificationCheck {
                id: "job".into(),
                kind: "test".into(),
                program: frozen("KILN_VERIFICATION_TEST_SHELL")?,
                arguments: vec!["-c".into(), command.into()],
                prerequisites: vec![],
                acceptance: vec!["case".into()],
                artifacts: vec!["observed".into()],
                timeout_ms: if expected == VerificationOutcome::Unknown {
                    100
                } else {
                    10_000
                },
            }],
            reproducer: None,
            protected_paths: vec!["requirements".into()],
            allow_already_satisfied: false,
            cairn: None,
            proof: None,
        };
        profile.toolchain = profile.derived_toolchain_identity()?;
        profile.environment_identity = profile.derived_environment_identity()?;
        let project = engine
            .profile
            .projects
            .first_mut()
            .ok_or("project missing")?;
        let mut graph = serde_json::to_value(&project.workflow)?;
        *graph
            .pointer_mut("/nodes/0/operation_identity")
            .ok_or("fixture node missing")? =
            json!(Engine::implementation_verification_operation(&profile, "job")?.as_str());
        project.workflow = serde_json::from_value(graph)?;
        engine.profile.bounds.max_artifact_bytes = 64 * 1024;
        engine.profile.bounds.max_run_artifact_bytes = 128 * 1024;
        engine.profile.bounds.max_total_artifact_bytes = 256 * 1024;
        let work = kiln_core::implementation::ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.into(),
            revision: 1,
            actor: "operator".into(),
            repository: profile.repository.clone(),
            project: "fixture".into(),
            base: Some(source.exact_revision.clone()),
            title: "frozen acceptance".into(),
            requested_outcome: "accepted exact candidate".into(),
            acceptance: vec![AcceptanceObligation {
                id: "case".into(),
                statement: "frozen oracle passes".into(),
            }],
            dependencies: vec![],
            permitted_paths: vec!["source.txt".into()],
            permitted_effects: BTreeSet::from(["write_source".into()]),
            verification_profile: profile.identity()?,
            budget: WorkBudget {
                attempts: 1,
                minutes_per_attempt: 1,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: kiln_core::implementation::WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "fixture/repository#90")
                    .map_err(|error| format!("{error:?}"))?,
                native_id: "fixture/repository#90".into(),
                version: content('d'),
            }],
        };
        let enrollment = Enrollment {
            repositories: BTreeSet::from([work.repository.clone()]),
            actors: BTreeSet::from([work.actor.clone()]),
        };
        let database = root.path().join("attempts.sqlite");
        let mut ledger = kiln_implementer::Implementer::open(&database, enrollment.clone())?;
        ledger.admit(&work, None)?;
        let now = host_time_ms()?;
        let attempt = ledger.claim_next(now, 60_000)?.ok_or("claim missing")?;
        ledger.reserve_dispatch(
            &attempt.identity,
            attempt.fence,
            now,
            &content('e'),
            &ProviderProfileId::new("fixture-worker").map_err(|error| format!("{error:?}"))?,
        )?;
        ledger.start_execution(
            &attempt.identity,
            attempt.fence,
            now,
            &ExternalRef::new("lattice", "run", "real-outcome-fixture")
                .map_err(|error| format!("{error:?}"))?,
        )?;
        ledger.record_worker_observation(
            &attempt.identity,
            attempt.fence,
            &WorkerObservation {
                terminal: WorkerTerminalState::Completed,
                worker_profile: Some(content('f')),
                harness: Some("fixture".into()),
                model: None,
                usage: AttemptUsage {
                    model_turns: ModelTurnObservation::Observed { turns: 5 },
                    ..AttemptUsage::default()
                },
            },
        )?;
        let handoff = kiln_core::implementation::CandidateHandoff {
            schema: CANDIDATE_HANDOFF_SCHEMA.into(),
            work_identity: attempt.work_identity.clone(),
            work_revision: 1,
            attempt_identity: attempt.identity.clone(),
            fence: attempt.fence,
            repository: work.repository.clone(),
            base: work.base.clone(),
            candidate: ExternalRef::new("git", "commit", &candidate.to_string())
                .map_err(|error| format!("{error:?}"))?,
            tree: ExternalRef::new("git", "tree", &candidate_tree.id().to_string())
                .map_err(|error| format!("{error:?}"))?,
            changed_paths: vec!["source.txt".into()],
            acceptance_digest: acceptance_digest(&work).map_err(|error| format!("{error:?}"))?,
            verification_profile: profile.identity()?,
            toolchain: profile.toolchain.clone(),
            environment: profile.environment_identity.clone(),
            evidence: vec![CandidateEvidence {
                identity: content('1'),
                kind: EvidenceKind::Check,
                outcome: EvidenceOutcome::Unknown,
                reference: ExternalRef::new("kiln", "capture", "fixture")
                    .map_err(|error| format!("{error:?}"))?,
            }],
        };
        ledger.admit_candidate_handoff(&work, &attempt.identity, attempt.fence, now, &handoff)?;
        let scratch_root = root.path().join("verification");
        std::fs::create_dir(&scratch_root)?;
        let authority = VerifierAuthority {
            repository: work.repository.clone(),
            repository_root: repository_path,
            scratch_root,
            git: frozen("KILN_VERIFICATION_TEST_GIT")?,
            worker_uid: u32::MAX,
        };
        let result = engine.verify_implementation_candidate(
            &mut ledger,
            &attempt.identity,
            attempt.fence,
            &profile,
            &authority,
        );
        assert_eq!(
            result.is_ok(),
            expected == VerificationOutcome::Verified,
            "{result:?}"
        );
        let finished = ledger
            .attempt(&attempt.identity)?
            .ok_or("attempt missing")?;
        assert_eq!(finished.verification_outcome, Some(expected));
        assert_eq!(
            finished.verification.is_some(),
            expected == VerificationOutcome::Verified
        );
        assert_eq!(
            finished.phase,
            if expected == VerificationOutcome::Verified {
                AttemptPhase::Verified
            } else {
                AttemptPhase::Verifying
            }
        );
        assert!(
            finished.metrics.is_none(),
            "observations never opt into a metrics receipt"
        );
        drop(ledger);
        let restored = kiln_implementer::Implementer::open(&database, enrollment)?
            .attempt(&attempt.identity)?
            .ok_or("restored attempt missing")?;
        assert_eq!(
            restored
                .worker_observation
                .as_ref()
                .map(|worker| worker.terminal),
            Some(WorkerTerminalState::Completed)
        );
        assert_eq!(restored.verification_outcome, Some(expected));
    }
    Ok(())
}
