use super::super::tests::TestResult;
use super::*;
use kiln_core::implementation::{
    AcceptanceObligation, AttemptLedger, DEFAULT_LEASE_MS, DISPATCH_PLAN_SCHEMA, DispatchControls,
    DispatchWorkspace, IMPLEMENTATION_WORK_SCHEMA, INTERACTIVE_AGENT_EFFECT, SourceRef,
    TerminalPolicy, WorkBudget, WorkSourceKind, WorkspaceIsolation,
};
use kiln_core::types::ProviderProfileId;
use serde_json::json;

#[test]
fn native_hash_framing_keeps_add_modify_delete_distinct() -> TestResult {
    let fact = |value: &[u8]| FileFact {
        length: value.len() as u64,
        hash: *blake3::hash(value).as_bytes(),
        object: Oid::ZERO_SHA1,
        mode: 0o100644,
    };
    let base = BTreeMap::from([
        ("src/a".into(), fact(b"old")),
        ("src/deleted".into(), fact(b"gone")),
    ]);
    let candidate = BTreeMap::from([
        ("src/a".into(), fact(b"new")),
        ("src/added".into(), fact(b"added")),
    ]);
    let paths = vec!["src/a".into(), "src/added".into(), "src/deleted".into()];
    assert_ne!(tree_identity(&base), tree_identity(&candidate));
    assert_ne!(
        diff_identity(&base, &candidate, &paths),
        diff_identity(
            &base,
            &candidate,
            paths.get(..2).ok_or("changed paths missing")?
        )
    );
    assert_ne!(
        diff_identity(&base, &candidate, &paths),
        diff_identity(&candidate, &base, &paths)
    );
    Ok(())
}

#[test]
#[ignore = "requires KILN_VERIFICATION_TEST_ROOT/GIT/SHELL private Linux Nix cohort"]
fn actual_host_snapshot_materializes_git_and_refuses_forged_receipt_or_omitted_deletion()
-> TestResult {
    let (root, authority, profile, work, _attempt, original) =
        super::super::tests::real_fixture(true, false)?;
    let (dispatch, attempt) = dispatched(&work)?;
    let repository = Repository::open(&authority.repository_root)?;
    let baseline_tree = repository
        .find_commit(Oid::from_str(
            work.base.as_ref().ok_or("base missing")?.native_id.as_str(),
        )?)?
        .tree()?;
    let mut baseline = Snapshot::new();
    let mut total = 0;
    collect_baseline(&repository, &baseline_tree, "", &mut baseline, &mut total)?;
    let worker = root.path().join("host-workspace");
    fs::create_dir(&worker)?;
    for (path, fact) in &baseline {
        let destination = worker.join(path);
        fs::create_dir_all(destination.parent().ok_or("destination parent missing")?)?;
        fs::write(destination, repository.find_blob(fact.object)?.content())?;
    }
    fs::write(worker.join("src/value"), "accepted")?;
    fs::write(worker.join("src/added"), "actual new file")?;
    let candidate = capture_workspace(&repository, &worker)?;
    let changed = vec!["src/added".to_string(), "src/value".to_string()];
    let receipt = json!({"schema":"lattice.candidate-receipt.v1", "work_id":work.identity().map_err(|error| format!("{error:?}"))?.as_str(),
        "attempt_id":attempt.identity.as_str(), "fence":attempt.fence.to_string(), "workspace_id":dispatch.workspace.identity.as_str(),
        "base_identity":tree_identity(&baseline), "candidate_identity":tree_identity(&candidate),
        "diff_identity":diff_identity(&baseline,&candidate,&changed), "changed_paths":changed,
        "disposition":"produced", "blockers":[]});
    let mut forged = receipt.clone();
    *forged
        .get_mut("candidate_identity")
        .ok_or("candidate identity missing")? = json!("0".repeat(64));
    assert!(
        materialize_candidate(
            &work,
            &attempt,
            &dispatch,
            &profile,
            &authority,
            &worker,
            &serde_json::to_vec(&forged)?
        )
        .is_err()
    );
    let handoff = materialize_candidate(
        &work,
        &attempt,
        &dispatch,
        &profile,
        &authority,
        &worker,
        &serde_json::to_vec(&receipt)?,
    )?;
    assert_ne!(handoff.candidate, original.candidate);
    let commit = repository.find_commit(Oid::from_str(handoff.candidate.native_id.as_str())?)?;
    assert_eq!(
        commit.tree_id().to_string(),
        handoff.tree.native_id.as_str()
    );
    assert_eq!(
        commit.parent_id(0)?.to_string(),
        work.base.as_ref().ok_or("base missing")?.native_id.as_str()
    );
    assert_eq!(handoff.changed_paths, vec!["src/added", "src/value"]);
    assert_eq!(
        materialize_candidate(
            &work,
            &attempt,
            &dispatch,
            &profile,
            &authority,
            &worker,
            &serde_json::to_vec(&receipt)?
        )?,
        handoff
    );
    // Worker VCS metadata is excluded and cannot substitute acquisition refs.
    fs::create_dir(worker.join(".git"))?;
    fs::write(worker.join(".git/HEAD"), "forged")?;
    assert_eq!(
        materialize_candidate(
            &work,
            &attempt,
            &dispatch,
            &profile,
            &authority,
            &worker,
            &serde_json::to_vec(&receipt)?
        )?,
        handoff
    );
    // A deletion omitted by the host receipt cannot survive independent capture.
    fs::remove_file(worker.join("src/value"))?;
    assert!(
        materialize_candidate(
            &work,
            &attempt,
            &dispatch,
            &profile,
            &authority,
            &worker,
            &serde_json::to_vec(&receipt)?
        )
        .is_err()
    );
    Ok(())
}

fn dispatched(
    work: &ImplementationWork,
) -> TestResult<(ImplementationDispatchPlan, ImplementationAttempt)> {
    dispatched_as(work, WorkspaceIsolation::IsolatedSnapshot)
}

/// A reserved plan in the given isolation: a Lattice snapshot plan, or an
/// interactive worktree with the herdr profile and its declared authority.
fn dispatched_as(
    work: &ImplementationWork,
    isolation: WorkspaceIsolation,
) -> TestResult<(ImplementationDispatchPlan, ImplementationAttempt)> {
    let interactive = isolation == WorkspaceIsolation::InteractiveWorktree;
    let mut ledger = AttemptLedger::default();
    let attempt = ledger
        .claim(work, 0, DEFAULT_LEASE_MS)
        .map_err(|error| format!("{error:?}"))?;
    let content = |byte: char| {
        ContentId::from_blake3_hex(byte.to_string().repeat(64))
            .map_err(|error| format!("{error:?}"))
    };
    let dispatch = ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: work.identity().map_err(|error| format!("{error:?}"))?,
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        workspace: DispatchWorkspace {
            identity: content('a')?,
            isolation,
            network: interactive,
            credentials: interactive,
            projection: None,
        },
        provider_profile: ProviderProfileId::new(if interactive {
            INTERACTIVE_PROVIDER_PROFILE
        } else {
            "lattice-implementation"
        })
        .map_err(|error| format!("{error:?}"))?,
        permitted_paths: work.permitted_paths.clone(),
        permitted_effects: work.permitted_effects.clone(),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: 5,
        token_budget: 1_000,
        invocation_budget: 1,
        expected_output: content('b')?,
        execution: None,
    };
    ledger
        .reserve_dispatch_plan(work, &attempt.identity, attempt.fence, 1, &dispatch)
        .map_err(|error| format!("{error:?}"))?;
    Ok((
        dispatch,
        ledger
            .get(&attempt.identity)
            .ok_or("attempt missing")?
            .clone(),
    ))
}

fn content(byte: char) -> TestResult<ContentId> {
    Ok(ContentId::from_blake3_hex(byte.to_string().repeat(64))
        .map_err(|error| format!("{error:?}"))?)
}

/// Work that permits interactive agents; no repository or frozen program is
/// needed to bind receipts to it.
fn interactive_work() -> TestResult<ImplementationWork> {
    Ok(ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "fixture-operator".into(),
        repository: "fixture/repository".into(),
        project: "fixture".into(),
        base: Some(
            ExternalRef::new("git", "commit", &"1".repeat(40))
                .map_err(|error| format!("{error:?}"))?,
        ),
        title: "interactive fixture".into(),
        requested_outcome: "accepted behavior".into(),
        acceptance: vec![AcceptanceObligation {
            id: "case".into(),
            statement: "frozen oracle passes".into(),
        }],
        dependencies: vec![],
        permitted_paths: vec!["src".into()],
        permitted_effects: BTreeSet::from(["write_source".into(), INTERACTIVE_AGENT_EFFECT.into()]),
        verification_profile: content('9')?,
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
            version: content('a')?,
        }],
    })
}

fn lattice_receipt(
    work: &ImplementationWork,
    attempt: &ImplementationAttempt,
    dispatch: &ImplementationDispatchPlan,
    schema: &str,
    disposition: &str,
) -> TestResult<serde_json::Value> {
    Ok(
        json!({"schema":schema, "work_id":work.identity().map_err(|error| format!("{error:?}"))?.as_str(),
        "attempt_id":attempt.identity.as_str(), "fence":attempt.fence.to_string(), "workspace_id":dispatch.workspace.identity.as_str(),
        "base_identity":"0".repeat(64), "candidate_identity":"0".repeat(64),
        "diff_identity":"0".repeat(64), "changed_paths":["src/value"],
        "disposition":disposition, "blockers":[]}),
    )
}

/// The receipt with one field replaced or added.
fn with(
    receipt: &serde_json::Value,
    field: &str,
    value: serde_json::Value,
) -> TestResult<serde_json::Value> {
    let mut changed = receipt.clone();
    changed
        .as_object_mut()
        .ok_or("receipt is not an object")?
        .insert(field.into(), value);
    Ok(changed)
}

#[test]
// r[verify kiln.fleet.capture.scenario.receipt]
fn each_plan_profile_admits_only_its_own_exactly_bound_receipt() -> TestResult {
    let work = interactive_work()?;
    let (herdr_plan, herdr_attempt) =
        dispatched_as(&work, WorkspaceIsolation::InteractiveWorktree)?;
    let (lattice_plan, lattice_attempt) = dispatched(&work)?;
    let session = content('5')?;
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let herdr = serde_json::to_value(HerdrCandidateReceipt::for_attempt(
        &work,
        &herdr_attempt,
        &herdr_plan,
        HerdrCandidateDisposition::Produced,
        commit,
        &session,
    )?)?;
    assert_eq!(
        herdr.get("schema"),
        Some(&json!(HERDR_CANDIDATE_RECEIPT_SCHEMA))
    );
    let lattice = lattice_receipt(
        &work,
        &lattice_attempt,
        &lattice_plan,
        LATTICE_CANDIDATE_RECEIPT_SCHEMA,
        "produced",
    )?;
    let lattice_unchanged = lattice_receipt(
        &work,
        &lattice_attempt,
        &lattice_plan,
        LATTICE_UNCHANGED_OBSERVATION_SCHEMA,
        "unchanged",
    )?;
    let admit = |plan: &ImplementationDispatchPlan,
                 attempt: &ImplementationAttempt,
                 receipt: &serde_json::Value|
     -> TestResult<Result<AdmittedReceipt, String>> {
        Ok(admit_receipt(
            &work,
            attempt,
            plan,
            &serde_json::to_vec(receipt)?,
        ))
    };

    // Each profile admits its own receipt kind.
    match admit(&herdr_plan, &herdr_attempt, &herdr)? {
        Ok(AdmittedReceipt::Herdr(receipt)) => {
            assert_eq!(receipt.candidate_commit, commit);
            assert_eq!(receipt.session, session);
            assert_eq!(receipt.disposition, HerdrCandidateDisposition::Produced);
        }
        other => return Err(format!("herdr receipt not admitted: {other:?}").into()),
    }
    let herdr_unchanged = with(&herdr, "disposition", json!("unchanged"))?;
    assert!(
        admit(&herdr_plan, &herdr_attempt, &herdr_unchanged)?
            .is_ok_and(|receipt| receipt.unchanged())
    );
    assert!(matches!(
        admit(&lattice_plan, &lattice_attempt, &lattice)?,
        Ok(AdmittedReceipt::Lattice(_))
    ));
    assert!(
        admit(&lattice_plan, &lattice_attempt, &lattice_unchanged)?
            .is_ok_and(|receipt| receipt.unchanged())
    );

    // Neither profile admits the other's receipt, even relabelled.
    let herdr_as_lattice = with(&herdr, "schema", json!(LATTICE_CANDIDATE_RECEIPT_SCHEMA))?;
    let lattice_as_herdr = with(&lattice, "schema", json!(HERDR_CANDIDATE_RECEIPT_SCHEMA))?;
    for (name, plan, attempt, receipt) in [
        ("herdr on lattice", &lattice_plan, &lattice_attempt, &herdr),
        (
            "relabelled herdr on lattice",
            &lattice_plan,
            &lattice_attempt,
            &herdr_as_lattice,
        ),
        ("lattice on herdr", &herdr_plan, &herdr_attempt, &lattice),
        (
            "lattice unchanged on herdr",
            &herdr_plan,
            &herdr_attempt,
            &lattice_unchanged,
        ),
        (
            "relabelled lattice on herdr",
            &herdr_plan,
            &herdr_attempt,
            &lattice_as_herdr,
        ),
        (
            "relabelled herdr on herdr",
            &herdr_plan,
            &herdr_attempt,
            &herdr_as_lattice,
        ),
    ] {
        assert!(admit(plan, attempt, receipt)?.is_err(), "{name}");
    }

    // A herdr receipt for another attempt, fence, workspace or work, or with a
    // blocker, an inexact commit, an invalid session or an extra field.
    for (name, field, value) in [
        ("another attempt", "attempt_id", json!("b".repeat(64))),
        ("another fence", "fence", json!(herdr_attempt.fence + 1)),
        (
            "fence as text",
            "fence",
            json!(herdr_attempt.fence.to_string()),
        ),
        ("another workspace", "workspace_id", json!("c".repeat(64))),
        ("another work", "work_id", json!("d".repeat(64))),
        ("blocked", "blockers", json!(["needs input"])),
        ("abbreviated commit", "candidate_commit", json!("0123456")),
        (
            "uppercase commit",
            "candidate_commit",
            json!("0123456789ABCDEF0123456789ABCDEF01234567"),
        ),
        ("invalid session", "session", json!("not-a-session")),
        ("unknown disposition", "disposition", json!("partial")),
        ("unknown field", "changed_paths", json!(["src/value"])),
    ] {
        assert!(
            admit(&herdr_plan, &herdr_attempt, &with(&herdr, field, value)?)?.is_err(),
            "{name}"
        );
    }
    // The Lattice receipt keeps its own attempt, fence and workspace binding.
    for field in ["attempt_id", "fence", "workspace_id"] {
        assert!(
            admit(
                &lattice_plan,
                &lattice_attempt,
                &with(&lattice, field, json!("9"))?
            )?
            .is_err(),
            "{field}"
        );
    }
    let oversized = vec![b' '; MAX_RECEIPT_BYTES + 1];
    assert!(admit_receipt(&work, &herdr_attempt, &herdr_plan, &oversized).is_err());
    Ok(())
}

#[test]
#[ignore = "requires KILN_VERIFICATION_TEST_ROOT/GIT/SHELL private Linux Nix cohort"]
// r[verify kiln.fleet.capture.scenario.receipt]
fn herdr_commit_export_materializes_only_the_reported_commit_tree() -> TestResult {
    let (root, authority, profile, mut work, _attempt, original) =
        super::super::tests::real_fixture(true, false)?;
    work.permitted_effects
        .insert(INTERACTIVE_AGENT_EFFECT.into());
    let (dispatch, attempt) = dispatched_as(&work, WorkspaceIsolation::InteractiveWorktree)?;
    let repository = Repository::open(&authority.repository_root)?;
    let agent_commit = original.candidate.native_id.as_str();
    let agent_tree = repository
        .find_commit(Oid::from_str(agent_commit)?)?
        .tree()?;
    let mut exported = Snapshot::new();
    let mut total = 0;
    collect_baseline(&repository, &agent_tree, "", &mut exported, &mut total)?;
    let worker = root.path().join("herdr-export");
    fs::create_dir(&worker)?;
    for (path, fact) in &exported {
        let destination = worker.join(path);
        fs::create_dir_all(destination.parent().ok_or("destination parent missing")?)?;
        fs::write(destination, repository.find_blob(fact.object)?.content())?;
    }
    let session = content('5')?;
    let receipt = HerdrCandidateReceipt::for_attempt(
        &work,
        &attempt,
        &dispatch,
        HerdrCandidateDisposition::Produced,
        agent_commit,
        &session,
    )?;
    let materialize = |receipt: &[u8]| {
        materialize_candidate(
            &work, &attempt, &dispatch, &profile, &authority, &worker, receipt,
        )
    };
    // A receipt naming another commit than the exported tree is refused.
    let mut base_named = receipt.clone();
    base_named.candidate_commit = work
        .base
        .as_ref()
        .ok_or("base missing")?
        .native_id
        .as_str()
        .into();
    assert!(materialize(&serde_json::to_vec(&base_named)?).is_err());
    // A Lattice receipt for the herdr plan is refused.
    let lattice = lattice_receipt(
        &work,
        &attempt,
        &dispatch,
        LATTICE_CANDIDATE_RECEIPT_SCHEMA,
        "produced",
    )?;
    assert!(materialize(&serde_json::to_vec(&lattice)?).is_err());
    let handoff = materialize(&serde_json::to_vec(&receipt)?)?;
    assert_eq!(handoff.tree.native_id.as_str(), agent_tree.id().to_string());
    assert_eq!(handoff.changed_paths, vec!["src/feature", "src/value"]);
    Ok(())
}

#[test]
#[ignore = "requires honest host unchanged disposition plus private Linux Nix cohort"]
fn unchanged_host_source_reaches_independent_verification_only_under_both_policies() -> TestResult {
    for (allow, terminal) in [(true, true), (false, true), (true, false)] {
        let (root, authority, mut profile, mut work, _attempt, _handoff) =
            super::super::tests::real_fixture(true, true)?;
        profile.allow_already_satisfied = allow;
        work.verification_profile = profile.identity()?;
        if !terminal {
            work.terminal_policy = kiln_core::implementation::TerminalPolicy::VerifiedPullRequest;
        }
        let (dispatch, attempt) = dispatched(&work)?;
        let repository = Repository::open(&authority.repository_root)?;
        let tree = repository
            .find_commit(Oid::from_str(
                work.base.as_ref().ok_or("base missing")?.native_id.as_str(),
            )?)?
            .tree()?;
        let mut baseline = Snapshot::new();
        let mut bytes = 0;
        collect_baseline(&repository, &tree, "", &mut baseline, &mut bytes)?;
        let worker = root.path().join("unchanged-host-workspace");
        fs::create_dir(&worker)?;
        for (path, fact) in &baseline {
            let destination = worker.join(path);
            fs::create_dir_all(destination.parent().ok_or("destination parent missing")?)?;
            fs::write(destination, repository.find_blob(fact.object)?.content())?;
        }
        let receipt = json!({"schema":"lattice.unchanged-candidate-observation.v1", "work_id":work.identity().map_err(|error| format!("{error:?}"))?.as_str(),
            "attempt_id":attempt.identity.as_str(), "fence":attempt.fence.to_string(), "workspace_id":dispatch.workspace.identity.as_str(),
            "base_identity":tree_identity(&baseline), "candidate_identity":tree_identity(&baseline),
            "diff_identity":diff_identity(&baseline,&baseline,&[]), "changed_paths":[], "disposition":"unchanged", "blockers":[]});
        let mut forged_produced = receipt.clone();
        *forged_produced
            .get_mut("disposition")
            .ok_or("disposition missing")? = json!("produced");
        *forged_produced.get_mut("schema").ok_or("schema missing")? =
            json!("lattice.candidate-receipt.v1");
        let mut wrong_schema = receipt.clone();
        *wrong_schema.get_mut("schema").ok_or("schema missing")? =
            json!("lattice.candidate-receipt.v1");
        assert!(
            materialize_candidate(
                &work,
                &attempt,
                &dispatch,
                &profile,
                &authority,
                &worker,
                &serde_json::to_vec(&wrong_schema)?
            )
            .is_err()
        );
        assert!(
            materialize_candidate(
                &work,
                &attempt,
                &dispatch,
                &profile,
                &authority,
                &worker,
                &serde_json::to_vec(&forged_produced)?
            )
            .is_err()
        );
        let result = materialize_candidate(
            &work,
            &attempt,
            &dispatch,
            &profile,
            &authority,
            &worker,
            &serde_json::to_vec(&receipt)?,
        );
        if allow && terminal {
            let handoff = result?;
            assert_eq!(handoff.base.as_ref(), Some(&handoff.candidate));
            assert!(handoff.changed_paths.is_empty());
            let verification =
                super::super::verify_candidate(&work, &attempt, &handoff, &profile, &authority)?;
            assert!(verification.run().success);
            assert!(
                verification
                    .candidate_verification(&work, &attempt, &handoff)
                    .is_ok()
            );
        } else {
            assert!(result.is_err());
        }
    }
    Ok(())
}

fn commit_files(
    repository: &Repository,
    parent: Option<&git2::Commit<'_>>,
    entries: &[(&str, &[u8], i32)],
) -> TestResult<Oid> {
    let mut index = git2::Index::new()?;
    for (path, bytes, mode) in entries {
        let blob = repository.blob(bytes)?;
        index.add(&IndexEntry {
            ctime: IndexTime::new(0, 0),
            mtime: IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: *mode as u32,
            uid: 0,
            gid: 0,
            file_size: bytes.len() as u32,
            id: blob,
            flags: 0,
            flags_extended: 0,
            path: path.as_bytes().to_vec(),
        })?;
    }
    let tree_id = index.write_tree_to(repository)?;
    let tree = repository.find_tree(tree_id)?;
    let sig = Signature::new("fixture", "fixture@kiln.invalid", &Time::new(1, 0))?;
    let parents: Vec<&git2::Commit<'_>> = parent.into_iter().collect();
    Ok(repository.commit(None, &sig, &sig, "fixture commit", &tree, &parents)?)
}

fn herdr_fixture(
    root: &Path,
    repository_root: &Path,
    base_oid: Oid,
) -> TestResult<(
    ImplementationWork,
    ImplementationDispatchPlan,
    ImplementationAttempt,
    VerificationProfile,
    VerifierAuthority,
)> {
    let scratch = root.join("scratch");
    fs::create_dir_all(&scratch)?;
    let mut profile = VerificationProfile {
        schema: "kiln.verification-profile.v1".into(),
        repository: "onixresearch/kiln".into(),
        toolchain: content('b')?,
        environment_identity: content('c')?,
        environment: BTreeMap::new(),
        checks: Vec::new(),
        reproducer: None,
        protected_paths: vec!["requirements".into()],
        allow_already_satisfied: true,
        cairn: None,
        proof: None,
    };
    profile.toolchain = profile.derived_toolchain_identity()?;
    profile.environment_identity = profile.derived_environment_identity()?;
    let mut work = interactive_work()?;
    work.verification_profile = profile.identity()?;
    work.base = Some(
        ExternalRef::new("git", "commit", &base_oid.to_string()).map_err(|error| error.message)?,
    );
    work.permitted_paths = vec!["source.txt".into()];
    work.terminal_policy = TerminalPolicy::VerifiedOrAlreadySatisfied;
    profile.repository = work.repository.clone();
    work.verification_profile = profile.identity()?;
    let (dispatch, attempt) = dispatched_as(&work, WorkspaceIsolation::InteractiveWorktree)?;
    let authority = VerifierAuthority {
        repository: work.repository.clone(),
        repository_root: fs::canonicalize(repository_root)?,
        scratch_root: fs::canonicalize(&scratch)?,
        git: super::super::FrozenProgram {
            path: PathBuf::from("/usr/bin/git"),
            blake3: "0".repeat(64),
        },
        worker_uid: u32::MAX,
    };
    Ok((work, dispatch, attempt, profile, authority))
}

#[test]
fn export_commit_tree_ignores_export_attributes_and_materializes_exact_tree() -> TestResult {
    let dir = tempfile::tempdir()?;
    let repo_path = dir.path().join("authority.git");
    let repository = Repository::init_bare(&repo_path)?;
    let base_oid = commit_files(
        &repository,
        None,
        &[
            (
                ".gitattributes",
                b"docs/ export-ignore\nversion.txt export-subst\n",
                0o100644,
            ),
            (
                "sub/.gitattributes",
                b"ignored.txt export-ignore\n",
                0o100644,
            ),
            (
                "docs/tracked.txt",
                b"This file is part of the commit.\n",
                0o100644,
            ),
            ("sub/ignored.txt", b"Nested export-ignore file.\n", 0o100644),
            ("version.txt", b"commit=$Format:%H$\n", 0o100644),
            ("source.txt", b"baseline\n", 0o100644),
        ],
    )?;
    let base_commit = repository.find_commit(base_oid)?;
    let candidate_oid = commit_files(
        &repository,
        Some(&base_commit),
        &[
            (
                ".gitattributes",
                b"docs/ export-ignore\nversion.txt export-subst\n",
                0o100644,
            ),
            (
                "sub/.gitattributes",
                b"ignored.txt export-ignore\n",
                0o100644,
            ),
            (
                "docs/tracked.txt",
                b"This file is part of the commit.\n",
                0o100644,
            ),
            ("sub/ignored.txt", b"Nested export-ignore file.\n", 0o100644),
            ("version.txt", b"commit=$Format:%H$\n", 0o100644),
            ("source.txt", b"candidate\n", 0o100644),
        ],
    )?;
    let (work, dispatch, attempt, profile, authority) =
        herdr_fixture(dir.path(), &repo_path, base_oid)?;

    let export_dir = dir.path().join("export");
    fs::create_dir(&export_dir)?;
    export_commit_tree(&repo_path, &candidate_oid.to_string(), &export_dir)?;
    let export_root = fs::canonicalize(&export_dir)?;

    assert_eq!(
        fs::read_to_string(export_root.join("docs/tracked.txt"))?,
        "This file is part of the commit.\n"
    );
    assert_eq!(
        fs::read_to_string(export_root.join("sub/ignored.txt"))?,
        "Nested export-ignore file.\n"
    );
    assert_eq!(
        fs::read_to_string(export_root.join("version.txt"))?,
        "commit=$Format:%H$\n"
    );

    let receipt = HerdrCandidateReceipt::for_attempt(
        &work,
        &attempt,
        &dispatch,
        HerdrCandidateDisposition::Produced,
        &candidate_oid.to_string(),
        &content('5')?,
    )?;
    let receipt_bytes = serde_json::to_vec(&receipt)?;
    let handoff = materialize_admitted_candidate(
        &work,
        &attempt,
        &dispatch,
        &profile,
        &authority,
        &export_root,
        &receipt_bytes,
    )?;
    let expected_tree = repository.find_commit(candidate_oid)?.tree_id().to_string();
    assert_eq!(handoff.tree.native_id.as_str(), expected_tree);
    assert_eq!(handoff.changed_paths, vec!["source.txt"]);

    // Tampering with or dropping an exported file is still refused.
    fs::remove_file(export_root.join("docs/tracked.txt"))?;
    let err = materialize_admitted_candidate(
        &work,
        &attempt,
        &dispatch,
        &profile,
        &authority,
        &export_root,
        &receipt_bytes,
    )
    .err()
    .ok_or("expected tree mismatch after deleting exported file")?;
    assert!(err.contains("exported candidate tree differs"), "{err}");

    // Symlink entries in a Git tree remain refused.
    let symlink_oid = commit_files(
        &repository,
        Some(&base_commit),
        &[("link", b"target", 0o120000)],
    )?;
    let symlink_export = dir.path().join("symlink-export");
    fs::create_dir(&symlink_export)?;
    assert!(export_commit_tree(&repo_path, &symlink_oid.to_string(), &symlink_export).is_err());
    Ok(())
}
