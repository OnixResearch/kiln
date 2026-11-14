use super::*;
use kiln_core::implementation::{
    AcceptanceObligation, AttemptLedger, CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence,
    DEFAULT_LEASE_MS, EvidenceKind, EvidenceOutcome, IMPLEMENTATION_WORK_SCHEMA, SourceRef,
    WorkBudget, WorkSourceKind, acceptance_digest,
};
use kiln_core::types::ExternalRef;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn identity(byte: char) -> TestResult<ContentId> {
    Ok(ContentId::from_blake3_hex(byte.to_string().repeat(64))
        .map_err(|error| format!("{error:?}"))?)
}
fn reference(kind: &str, value: &str) -> TestResult<ExternalRef> {
    Ok(ExternalRef::new("git", kind, value).map_err(|error| format!("{error:?}"))?)
}

fn fixture_work(profile: &VerificationProfile, base: &str) -> TestResult<ImplementationWork> {
    Ok(ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "fixture-operator".into(),
        repository: "fixture/repository".into(),
        project: "fixture".into(),
        base: Some(reference("commit", base)?),
        title: "independent fixture".into(),
        requested_outcome: "accepted behavior".into(),
        acceptance: vec![AcceptanceObligation {
            id: "case".into(),
            statement: "frozen oracle passes".into(),
        }],
        dependencies: vec![],
        permitted_paths: vec!["src".into()],
        permitted_effects: BTreeSet::from(["write_source".into()]),
        verification_profile: profile.identity()?,
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 5,
        },
        terminal_policy: TerminalPolicy::VerifiedOrAlreadySatisfied,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "fixture/repository#1")
                .map_err(|error| format!("{error:?}"))?,
            native_id: "fixture/repository#1".into(),
            version: identity('a')?,
        }],
    })
}

fn program(variable: &str) -> TestResult<FrozenProgram> {
    let path = fs::canonicalize(
        std::env::var_os(variable)
            .ok_or_else(|| format!("set {variable} to an immutable Nix-store executable"))?,
    )?;
    let blake3 = digest(&bounded_file(&path, MAX_FILE_BYTES)?);
    Ok(FrozenProgram { path, blake3 })
}

fn git(authority: &VerifierAuthority, args: &[&str]) -> TestResult<String> {
    let environment = BTreeMap::from([
        ("GIT_AUTHOR_NAME".into(), "fixture".into()),
        ("GIT_AUTHOR_EMAIL".into(), "fixture@example.invalid".into()),
        ("GIT_COMMITTER_NAME".into(), "fixture".into()),
        (
            "GIT_COMMITTER_EMAIL".into(),
            "fixture@example.invalid".into(),
        ),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
    ]);
    let output = process::execute(
        &authority.git.path,
        &args.iter().map(|arg| (*arg).into()).collect::<Vec<_>>(),
        &authority.repository_root,
        &environment,
        10_000,
    )?;
    assert_eq!(
        output.status,
        "passed",
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

/// Real local Git and bounded command fixture. Requires an explicitly private
/// root (not /tmp), immutable Git and shell. No network, model or remote effects.
pub(super) fn real_fixture(
    bug: bool,
    no_change: bool,
) -> TestResult<(
    tempfile::TempDir,
    VerifierAuthority,
    VerificationProfile,
    ImplementationWork,
    ImplementationAttempt,
    CandidateHandoff,
)> {
    let root = tempfile::Builder::new()
        .prefix("kiln-verification-")
        .tempdir_in(
            std::env::var_os("KILN_VERIFICATION_TEST_ROOT").ok_or("set private test root")?,
        )?;
    let repository_root = root.path().join("repository");
    let scratch_root = root.path().join("evidence");
    fs::create_dir(&repository_root)?;
    fs::create_dir(&scratch_root)?;
    let authority = VerifierAuthority {
        repository: "fixture/repository".into(),
        repository_root,
        scratch_root,
        git: program("KILN_VERIFICATION_TEST_GIT")?,
        worker_uid: u32::MAX,
    };
    git(&authority, &["init"])?;
    fs::create_dir(authority.repository_root.join("src"))?;
    fs::write(
        authority.repository_root.join("requirements"),
        "frozen external oracle",
    )?;
    fs::write(
        authority.repository_root.join("src/value"),
        if bug && !no_change {
            "broken"
        } else {
            "accepted"
        },
    )?;
    git(&authority, &["add", "."])?;
    git(&authority, &["commit", "-m", "baseline"])?;
    let base = git(&authority, &["rev-parse", "HEAD"])?;
    if !no_change {
        fs::write(authority.repository_root.join("src/value"), "accepted")?;
        fs::write(authority.repository_root.join("src/feature"), "implemented")?;
        git(&authority, &["add", "."])?;
        git(&authority, &["commit", "-m", "candidate"])?;
    }
    let candidate = git(&authority, &["rev-parse", "HEAD"])?;
    let tree = git(&authority, &["rev-parse", "HEAD^{tree}"])?;
    // Program and oracle argv are frozen outside the candidate; no source script
    // or worker-provided success document is read to determine the exit outcome.
    let oracle = if bug {
        "read value < src/value; printf '%s' \"$value\" > observed; test \"$value\" = accepted"
    } else {
        "read value < src/feature; printf '%s' \"$value\" > observed; test \"$value\" = implemented"
    };
    let mut profile = VerificationProfile {
        schema: PROFILE_SCHEMA.into(),
        repository: authority.repository.clone(),
        toolchain: identity('b')?,
        environment_identity: identity('c')?,
        environment: BTreeMap::new(),
        checks: vec![VerificationCheck {
            id: "case".into(),
            kind: "acceptance".into(),
            program: program("KILN_VERIFICATION_TEST_SHELL")?,
            arguments: vec!["-c".into(), oracle.into()],
            prerequisites: vec![],
            acceptance: vec!["case".into()],
            artifacts: vec!["observed".into()],
            timeout_ms: 10_000,
        }],
        reproducer: bug.then(|| "case".into()),
        protected_paths: vec!["requirements".into()],
        allow_already_satisfied: true,
        cairn: None,
    };
    profile.toolchain = profile.derived_toolchain_identity()?;
    profile.environment_identity = profile.derived_environment_identity()?;
    let work = fixture_work(&profile, &base)?;
    let mut ledger = AttemptLedger::default();
    let attempt = ledger
        .claim(&work, 0, DEFAULT_LEASE_MS)
        .map_err(|error| format!("{error:?}"))?;
    let handoff = CandidateHandoff {
        schema: CANDIDATE_HANDOFF_SCHEMA.into(),
        work_identity: work.identity().map_err(|error| format!("{error:?}"))?,
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        candidate: reference("commit", &candidate)?,
        tree: reference("tree", &tree)?,
        changed_paths: if no_change {
            vec![]
        } else if bug {
            vec!["src/feature".into(), "src/value".into()]
        } else {
            vec!["src/feature".into()]
        },
        acceptance_digest: acceptance_digest(&work).map_err(|error| format!("{error:?}"))?,
        verification_profile: work.verification_profile.clone(),
        toolchain: profile.toolchain.clone(),
        environment: profile.environment_identity.clone(),
        evidence: vec![CandidateEvidence {
            identity: identity('d')?,
            kind: EvidenceKind::Check,
            outcome: EvidenceOutcome::Passed,
            reference: ExternalRef::new("worker", "proposal", "not-trusted")
                .map_err(|error| format!("{error:?}"))?,
        }],
    };
    Ok((root, authority, profile, work, attempt, handoff))
}

#[test]
#[ignore = "requires KILN_VERIFICATION_TEST_ROOT/GIT/SHELL private Linux Nix cohort"]
fn real_bug_and_feature_execute_frozen_oracle_and_refuse_replay_or_substitution() -> TestResult {
    for bug in [true, false] {
        let (_root, authority, profile, work, attempt, handoff) = real_fixture(bug, false)?;
        let report = verify_candidate(&work, &attempt, &handoff, &profile, &authority)?;
        assert!(report.run().success);
        let accepted = report.candidate_verification(&work, &attempt, &handoff)?;
        assert_eq!(accepted.candidate, handoff.candidate);
        if bug {
            let baseline = report
                .observations()
                .first()
                .ok_or("baseline observation missing")?;
            let candidate = report
                .observations()
                .get(1)
                .ok_or("candidate observation missing")?;
            assert_eq!(baseline.status, "failed");
            assert_eq!(candidate.status, "passed");
            assert_eq!(baseline.command, candidate.command);
        }
        assert!(verify_candidate(&work, &attempt, &handoff, &profile, &authority).is_err());
        let mut stale = handoff.clone();
        stale.fence += 1;
        assert!(
            report
                .candidate_verification(&work, &attempt, &stale)
                .is_err()
        );
        let mut old = work.clone();
        old.revision += 1;
        assert!(
            report
                .candidate_verification(&old, &attempt, &handoff)
                .is_err()
        );
        let row = report
            .observations()
            .iter()
            .find(|row| !row.baseline)
            .ok_or("candidate observation missing")?;
        let artifact = report
            .evidence_directory()
            .join(format!("candidate-{}/artifacts/observed", row.check_id));
        fs::write(artifact, "forged success")?;
        assert!(
            report
                .candidate_verification(&work, &attempt, &handoff)
                .is_err()
        );
    }
    Ok(())
}

#[test]
#[ignore = "requires KILN_VERIFICATION_TEST_ROOT/GIT/SHELL private Linux Nix cohort"]
fn independent_no_change_needs_explicit_policy_and_passing_evidence() -> TestResult {
    let (_root, authority, profile, work, attempt, handoff) = real_fixture(true, true)?;
    let report = verify_candidate(&work, &attempt, &handoff, &profile, &authority)?;
    assert!(report.already_satisfied);
    assert!(
        report
            .candidate_verification(&work, &attempt, &handoff)
            .is_ok()
    );
    let mut denied = work;
    denied.terminal_policy = TerminalPolicy::VerifiedPullRequest;
    assert!(admit_candidate_handoff(&denied, &attempt, &handoff).is_err());
    Ok(())
}

#[test]
#[ignore = "requires KILN_VERIFICATION_TEST_ROOT/GIT/SHELL private Linux Nix cohort"]
fn missing_artifact_incomplete_paths_and_changed_policy_cannot_pass() -> TestResult {
    let (_root, authority, mut profile, mut work, mut attempt, mut handoff) =
        real_fixture(true, false)?;
    let mut incomplete = handoff.clone();
    incomplete.changed_paths.pop();
    assert!(verify_candidate(&work, &attempt, &incomplete, &profile, &authority).is_err());
    // A frozen verifier exits zero but produces no artifact: worker's proposed
    // passed evidence cannot fill the missing independently observed output.
    profile
        .checks
        .first_mut()
        .ok_or("verification check missing")?
        .arguments = vec!["-c".into(), "exit 0".into()];
    work.verification_profile = profile.identity()?;
    attempt.work_identity = work.identity().map_err(|error| format!("{error:?}"))?;
    handoff.work_identity = work.identity().map_err(|error| format!("{error:?}"))?;
    handoff.verification_profile = work.verification_profile.clone();
    handoff.acceptance_digest = acceptance_digest(&work).map_err(|error| format!("{error:?}"))?;
    let report = verify_candidate(&work, &attempt, &handoff, &profile, &authority)?;
    assert!(!report.run().success);
    assert!(
        report
            .candidate_verification(&work, &attempt, &handoff)
            .is_err()
    );
    git(
        &authority,
        &["checkout", "--detach", handoff.candidate.native_id.as_str()],
    )?;
    fs::write(
        authority.repository_root.join("requirements"),
        "disabled oracle",
    )?;
    git(&authority, &["add", "."])?;
    git(&authority, &["commit", "-m", "weaken requirements"])?;
    handoff.candidate = reference("commit", &git(&authority, &["rev-parse", "HEAD"])?)?;
    assert!(source::protect(&authority, &handoff, &profile.protected_paths, None).is_err());
    Ok(())
}

#[test]
fn artifacts_require_nonempty_regular_files_not_symlinks_or_missing_paths() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::write(root.path().join("result"), "real observation")?;
    assert_eq!(
        retained_artifact(root.path(), "result")?,
        b"real observation"
    );
    assert!(retained_artifact(root.path(), "missing").is_err());
    fs::write(root.path().join("empty"), "")?;
    assert!(retained_artifact(root.path(), "empty").is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("result", root.path().join("substituted"))?;
        assert!(retained_artifact(root.path(), "substituted").is_err());
    }
    Ok(())
}

#[test]
fn bounded_process_timeout_and_signal_cannot_synthesize_success() -> TestResult {
    let root = tempfile::tempdir()?;
    let environment = BTreeMap::new();
    let timed = process::execute(
        Path::new("/bin/sh"),
        &["-c".into(), "while :; do :; done".into()],
        root.path(),
        &environment,
        20,
    )?;
    assert_eq!(timed.status, "timed_out");
    let crashed = process::execute(
        Path::new("/bin/sh"),
        &["-c".into(), "kill -KILL $$".into()],
        root.path(),
        &environment,
        1_000,
    )?;
    assert_ne!(crashed.status, "passed");
    assert!(crashed.exit_code.is_none());
    Ok(())
}

#[test]
#[ignore = "requires KILN_VERIFICATION_TEST_ROOT/GIT/SHELL private Linux Nix cohort"]
fn lifecycle_source_requires_exact_trusted_preparation_not_worker_archive() -> TestResult {
    let (_root, authority, _profile, _work, _attempt, mut handoff) = real_fixture(true, false)?;
    let active = authority.repository_root.join(".cairn/changes/fixture");
    fs::create_dir_all(&active)?;
    fs::write(
        active.join("tasks.md"),
        "frozen task and acceptance version",
    )?;
    git(&authority, &["add", "."])?;
    git(&authority, &["commit", "-m", "frozen lifecycle baseline"])?;
    handoff.base = Some(reference(
        "commit",
        &git(&authority, &["rev-parse", "HEAD"])?,
    )?);
    let archive = authority.repository_root.join(".cairn/archive");
    fs::create_dir(&archive)?;
    fs::rename(&active, archive.join("fixture"))?;
    git(&authority, &["add", "."])?;
    git(&authority, &["commit", "-m", "trusted staged archive"])?;
    handoff.candidate = reference("commit", &git(&authority, &["rev-parse", "HEAD"])?)?;
    let prepared = git(&authority, &["rev-parse", "HEAD:.cairn"])?;
    let protected = vec![".cairn".into(), "requirements".into()];
    assert!(source::protect(&authority, &handoff, &protected, None).is_err());
    assert!(source::protect(&authority, &handoff, &protected, Some(&prepared)).is_ok());
    assert!(source::protect(&authority, &handoff, &protected, Some(&"0".repeat(40))).is_err());
    fs::write(authority.repository_root.join("requirements"), "weakened")?;
    git(&authority, &["add", "."])?;
    git(&authority, &["commit", "-m", "worker disabled acceptance"])?;
    handoff.candidate = reference("commit", &git(&authority, &["rev-parse", "HEAD"])?)?;
    assert!(source::protect(&authority, &handoff, &protected, Some(&prepared)).is_err());
    Ok(())
}
