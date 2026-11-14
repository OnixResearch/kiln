use super::super::tests::TestResult;
use super::*;
use kiln_core::implementation::{
    AttemptLedger, DEFAULT_LEASE_MS, DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace,
    WorkspaceIsolation,
};
use kiln_core::types::ProviderProfileId;
use serde_json::json;

#[test]
fn native_hash_framing_keeps_add_modify_delete_distinct() -> TestResult {
    let fact = |value: &[u8]| FileFact {
        length: value.len() as u64,
        hash: *blake3::hash(value).as_bytes(),
        object: Oid::zero(),
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
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
        },
        provider_profile: ProviderProfileId::new("lattice-implementation")
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
