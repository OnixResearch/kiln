use super::*;
use kiln_adapter_radicle::status::{
    PINNED_SECRETSPEC_EXECUTABLE_BLAKE3, PINNED_SECRETSPEC_EXECUTABLE_PATH,
    PINNED_SECRETSPEC_VERSION, PinnedSecretSpecExecutable, ResolvedSigner, SecretSpecDeclaration,
};
use kiln_core::untrusted::{Approver, ApproverAuthority};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;

struct Fixture {
    root: tempfile::TempDir,
    profile: PatchProfile,
    first: RevisionBinding,
    second: RevisionBinding,
}
fn executable(path: &Path, body: &str) -> Executable {
    fs::write(path, body)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    Executable {
        path: path.to_path_buf(),
        blake3: ContentId::from_blake3_hex(blake3::hash(body.as_bytes()).to_hex().to_string())
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
    }
}
fn fixture() -> Fixture {
    let root = tempfile::tempdir()
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let storage = root.path().join("storage");
    fs::create_dir(&storage)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let rid = "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE";
    let repo = cob_git2::Repository::init_bare(storage.join(rid.trim_start_matches("rad:")))
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let signature = cob_git2::Signature::new(
        "fixture",
        "fixture@kiln.invalid",
        &cob_git2::Time::new(1, 0),
    )
    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let mut tree = repo
        .treebuilder(None)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let blob = repo
        .blob(b"first exact source\n")
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    tree.insert("input", blob, 0o100644)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let tree1 = repo
        .find_tree(
            tree.write()
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let first_oid = repo
        .commit(None, &signature, &signature, "first", &tree1, &[])
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    tree.insert(
        "input",
        repo.blob(b"second exact source\n")
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        0o100644,
    )
    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let tree2 = repo
        .find_tree(
            tree.write()
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let first_commit = repo
        .find_commit(first_oid)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let second_oid = repo
        .commit(
            None,
            &signature,
            &signature,
            "second",
            &tree2,
            &[&first_commit],
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let signer = ResolvedSigner::from_resolved_secret(&"01".repeat(32))
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let author = signer.public_key().to_owned();
    let first = RevisionBinding {
        repository: rid.into(),
        patch: "a".repeat(40),
        revision: "b".repeat(40),
        base: first_oid.to_string(),
        oid: first_oid.to_string(),
        author: author.clone(),
    };
    let second = RevisionBinding {
        revision: "c".repeat(40),
        base: first_oid.to_string(),
        oid: second_oid.to_string(),
        ..first.clone()
    };
    let projection = root.path().join("projection.json");
    let cat = std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_else(|| unreachable!("fixture environment is missing")),
    )
    .map(|p| p.join("cat"))
    .find(|p| p.is_file())
    .unwrap_or_else(|| unreachable!("fixture executable is missing"));
    let cat = fs::canonicalize(
        cat.parent()
            .unwrap_or_else(|| unreachable!("fixture path has no parent")),
    )
    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
    .join(
        cat.file_name()
            .unwrap_or_else(|| unreachable!("fixture path has no filename")),
    );
    let helper = executable(
        &root.path().join("helper"),
        &format!(
            "#!/bin/sh\nexec '{}' '{}'\n",
            cat.display(),
            projection.display()
        ),
    );
    // A fake authenticated transport tests the controller's state transitions;
    // native signatures and authorization are independently exercised by the
    // real upstream fixture in tools/radicle-patch.
    let python = std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_else(|| unreachable!("fixture environment is missing")),
    )
    .map(|p| p.join("python3"))
    .find(|p| p.is_file())
    .unwrap_or_else(|| unreachable!("fixture executable is missing"));
    let log = root.path().join("rpc.log");
    let rpc_state = root.path().join("rpc-state.json");
    fs::write(&rpc_state, b"\"running\"")
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let hosted = executable(
        &root.path().join("hosted"),
        &format!(
            "#!{}\nimport json,sys\nr=json.load(open(sys.argv[sys.argv.index('--request')+1]))\nwith open({:?}, 'a') as f: f.write(r['action']+'\\n')\nprint(json.dumps({{'schema':'kiln.hosted.v1','request_id':r['request_id'],'error':None,'result':{{'attempt_id':r['input']['attempt_id'],'state':json.load(open({:?}))}}}}))\n",
            python.display(),
            log.to_str()
                .unwrap_or_else(|| unreachable!("fixture log path is not UTF-8")),
            rpc_state
                .to_str()
                .unwrap_or_else(|| unreachable!("fixture RPC path is not UTF-8"))
        ),
    );
    let client = root.path().join("client.json");
    fs::write(&client, br#"{"organization":"fixture"}"#)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let client_id = ContentId::from_blake3_hex(
        blake3::hash(
            &fs::read(&client)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .to_hex()
        .to_string(),
    )
    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let state = root.path().join("state");
    let workspace = root.path().join("work");
    for path in [&state, &workspace] {
        fs::create_dir(path)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    }
    let profile = PatchProfile {
        schema: PATCH_PROFILE_SCHEMA.into(),
        repository_id: rid.into(),
        storage_root: storage,
        helper,
        state_root: state,
        snapshot_limits: kiln_radicle_source_contract::fixture_request().limits,
        trusted_authors: BTreeSet::new(),
        approvers: BTreeSet::from(["operator".into()]),
        hosted: HostedPatchProfile {
            executable: hosted,
            client_profile: client,
            client_profile_identity: client_id,
            organization: "fixture".into(),
            project: "fixture".into(),
            pipeline_id: "reviewed".into(),
            price_id: "fixture".into(),
            class: "fixture".into(),
            workspace_root: workspace,
        },
        native_fixtures: false,
        job_profile: RadicleJobProfile::pinned(
            BTreeSet::from([rid.into()]),
            ExternalRef::new("radicle", "node", &author)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
            SecretSpecDeclaration {
                project_root: root.path().to_path_buf(),
                project: "fixture".into(),
                profile: "fixture".into(),
                secret_key: "fixture".into(),
                provider: Some("keyring".into()),
            },
            PinnedSecretSpecExecutable {
                path: PINNED_SECRETSPEC_EXECUTABLE_PATH.into(),
                blake3: PINNED_SECRETSPEC_EXECUTABLE_BLAKE3.into(),
                version: PINNED_SECRETSPEC_VERSION.into(),
            },
        ),
        log_url_prefix: "https://ci.example.invalid/run/".into(),
    };
    let result = Fixture {
        root,
        profile,
        first,
        second,
    };
    result.projection(false);
    result
}
impl Fixture {
    fn projection(&self, second: bool) {
        let revision = |b: &RevisionBinding| {
            json!({"id":b.revision,"base":b.base,"head":b.oid,"author":b.author,
            "signed_source":{"reference":format!("refs/cobs/xyz.radicle.patch/{}",b.patch),"tip":"d".repeat(40)}})
        };
        let revisions = if second {
            vec![revision(&self.first), revision(&self.second)]
        } else {
            vec![revision(&self.first)]
        };
        fs::write(self.root.path().join("projection.json"), serde_json::to_vec(&json!({"patch_id":self.first.patch,"heads":["d".repeat(40)],"revisions":revisions})).unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))).unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    }
    fn approval(&self, journal: &PatchJournal) -> ApproveUntrustedRun {
        ApproveUntrustedRun {
            actor: Approver {
                id: "operator".into(),
                label: "Interactive fixture operator".into(),
                authority: ApproverAuthority::Operator,
            },
            run: journal.run_id.clone(),
            source_commit: journal.binding.oid.clone(),
        }
    }
}

// r[verify kiln.radicle.revision_ci]
#[test]
fn revisions_are_separately_journaled_reapproved_and_duplicate_events_never_redispatch() {
    let f = fixture();
    let first = f
        .profile
        .intake(
            &exact_trigger(&f.first)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(first.state, PatchState::Blocked);
    assert_eq!(
        f.profile
            .drive(&first.run_id)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .state,
        PatchState::Blocked
    );
    assert!(!f.root.path().join("rpc.log").exists());
    let mut wrong = f.approval(&first);
    wrong.actor.authority = ApproverAuthority::Automation;
    assert!(f.profile.approve(&wrong, 1).is_err());
    let approved = f
        .profile
        .approve(&f.approval(&first), 2)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(approved.state, PatchState::Ready);
    let running = f
        .profile
        .drive(&first.run_id)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(running.state, PatchState::Running);
    assert_eq!(
        fs::read(
            f.profile
                .hosted
                .workspace_root
                .join(&first.run_id)
                .join("input")
        )
        .unwrap_or_else(|error| unreachable!("{error:?}")),
        b"first exact source\n"
    );
    let request = running
        .request
        .as_ref()
        .unwrap_or_else(|| unreachable!("fixture value is missing"));
    assert_eq!(
        request
            .pointer("/untrusted/source_commit")
            .and_then(Value::as_str),
        Some(f.first.oid.as_str())
    );
    assert_eq!(
        request.get("trust").and_then(Value::as_str),
        Some("hostile")
    );
    f.projection(true);
    let second = f
        .profile
        .intake(
            &exact_trigger(&f.second)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_ne!(first.run_id, second.run_id);
    assert_eq!(second.state, PatchState::Blocked);
    assert_eq!(second.binding.base, f.first.oid);
    assert_eq!(second.binding.oid, f.second.oid);
    let replay = f
        .profile
        .intake(
            &exact_trigger(&f.first)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(replay.run_id, first.run_id);
    f.profile
        .drive(&replay.run_id)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let actions = fs::read_to_string(f.root.path().join("rpc.log"))
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(actions.lines().filter(|a| *a == "execution.run").count(), 1);
    assert_eq!(actions.lines().filter(|a| *a == "execution.get").count(), 1);
    let mut stale = f.approval(&first);
    stale.run = second.run_id.clone();
    assert!(f.profile.approve(&stale, 3).is_err());
    f.profile
        .approve(&f.approval(&second), 4)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let running_second = f
        .profile
        .drive(&second.run_id)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(running_second.state, PatchState::Running);
    assert_eq!(
        fs::read(
            f.profile
                .hosted
                .workspace_root
                .join(&second.run_id)
                .join("input")
        )
        .unwrap_or_else(|error| unreachable!("{error:?}")),
        b"second exact source\n"
    );
    assert_eq!(
        running_second
            .request
            .as_ref()
            .and_then(|request| request.pointer("/untrusted/source_commit"))
            .and_then(Value::as_str),
        Some(f.second.oid.as_str())
    );
    println!(
        "two exact revision journals; untrusted approval/reapproval; exact source snapshots; duplicate updates only recover original attempt"
    );
}

#[test]
fn missing_signed_author_reachability_and_substituted_revision_results_fail_closed() {
    let f = fixture();
    let mut value: Value = serde_json::from_slice(
        &fs::read(f.root.path().join("projection.json"))
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
    )
    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    *value
        .pointer_mut("/revisions/0/signed_source")
        .unwrap_or_else(|| unreachable!("signed-source fixture field missing")) = Value::Null;
    fs::write(
        f.root.path().join("projection.json"),
        serde_json::to_vec(&value)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
    )
    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert!(
        f.profile
            .intake(&exact_trigger(&f.first).unwrap_or_else(|error| unreachable!("{error:?}")))
            .is_err()
    );
    f.projection(false);
    let mut journal = f
        .profile
        .intake(
            &exact_trigger(&f.first)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert!(
        apply_observation(
            &mut journal,
            json!({"attempt_id":"another-revision", "state":"succeeded"})
        )
        .is_err()
    );
    f.projection(true);
    assert!(f.profile.approve(&f.approval(&journal), 1).is_err());
    assert_eq!(
        f.profile
            .load(&journal.run_id)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .state,
        PatchState::Blocked
    );
}

#[test]
fn oversized_reviewed_executable_is_rejected_even_when_its_digest_matches() {
    let directory = tempfile::tempdir()
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let path = directory.path().join("oversized-tool");
    let file = fs::File::create(&path)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    file.set_len(MAX_EXECUTABLE_BYTES + 1)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    drop(file);
    let mut hasher = blake3::Hasher::new();
    std::io::copy(
        &mut fs::File::open(&path)
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        &mut hasher,
    )
    .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let selected = Executable {
        path,
        blake3: ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
            .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
    };
    assert!(selected.admit().is_err());
}

#[test]
fn native_fixture_authority_cannot_be_forged_in_profile_json() {
    let f = fixture();
    let mut value = serde_json::to_value(&f.profile)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    value
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("profile fixture must be an object"))
        .insert("native_fixtures".into(), Value::Bool(true));
    assert!(serde_json::from_value::<PatchProfile>(value).is_err());
}

#[cfg(feature = "native-fixtures")]
#[test]
fn terminal_patch_results_publish_real_disposable_jobs_for_each_exact_oid() {
    let mut f = fixture();
    let signer = executable(
        &f.root.path().join("fixture-signer"),
        &format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", "01".repeat(32)),
    );
    f.profile.job_profile = RadicleJobProfile::pinned(
        f.profile.job_profile.allowed_rids.clone(),
        f.profile.job_profile.signer_identity.clone(),
        f.profile.job_profile.signer.clone(),
        PinnedSecretSpecExecutable {
            path: signer.path,
            blake3: signer.blake3.as_str().to_owned(),
            version: PINNED_SECRETSPEC_VERSION.into(),
        },
    );
    f.profile.enable_native_fixtures();
    let first = f
        .profile
        .intake(
            &exact_trigger(&f.first)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    f.profile
        .approve(&f.approval(&first), 1)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(
        f.profile
            .drive(&first.run_id)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .state,
        PatchState::Running
    );
    fs::write(f.root.path().join("rpc-state.json"), b"\"succeeded\"")
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let first_done = f
        .profile
        .drive(&first.run_id)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(first_done.state, PatchState::Succeeded);
    let Some(JobDispatch::Written(first_job)) = first_done.job else {
        unreachable!("first real job absent")
    };
    assert_eq!(first_job.revision.native_id.as_str(), f.first.oid);
    assert_eq!(first_job.run_id, first.run_id);
    f.projection(true);
    let second = f
        .profile
        .intake(
            &exact_trigger(&f.second)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(second.state, PatchState::Blocked);
    f.profile
        .approve(&f.approval(&second), 2)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let second_done = f
        .profile
        .drive(&second.run_id)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    let Some(JobDispatch::Written(second_job)) = second_done.job else {
        unreachable!("second real job absent")
    };
    assert_eq!(second_job.revision.native_id.as_str(), f.second.oid);
    assert_eq!(second_job.run_id, second.run_id);
    assert_ne!(first_job.object_id, second_job.object_id);
    let shell = f
        .profile
        .job_shell()
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(
        shell
            .read(&first_job.repository, &first_job.object_id)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .oid()
            .to_string(),
        f.first.oid
    );
    assert_eq!(
        shell
            .read(&second_job.repository, &second_job.object_id)
            .unwrap_or_else(|error| unreachable!("{error:?}"))
            .oid()
            .to_string(),
        f.second.oid
    );
}

#[test]
fn complete_job_log_url_is_admitted_before_execution_effects() {
    let mut fixture = fixture();
    let base = "https://ci.example.invalid/run/";
    fixture.profile.log_url_prefix = format!(
        "{base}{}",
        "a".repeat(MAX_LOG_URL_PREFIX_BYTES - base.len())
    );
    assert!(fixture.profile.admit().is_ok());
    let eventual = format!(
        "{}radicle-patch-{}",
        fixture.profile.log_url_prefix,
        "0".repeat(blake3::OUT_LEN * 2)
    );
    assert!(
        kiln_adapter_radicle::job::admit_log_url(
            &eventual,
            kiln_adapter_radicle::job::DEFAULT_MAX_LOG_URL_BYTES
        )
        .is_ok()
    );
    fixture.profile.log_url_prefix.push('a');
    assert!(fixture.profile.admit().is_err());
    assert!(!fixture.root.path().join("rpc.log").exists());
}

#[test]
fn author_reachability_withdrawn_during_snapshot_never_reserves_or_dispatches() {
    let mut fixture = fixture();
    let python = std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_else(|| unreachable!("fixture environment is missing")),
    )
    .map(|path| path.join("python3"))
    .find(|path| path.is_file())
    .unwrap_or_else(|| unreachable!("fixture executable is missing"));
    let projection = fixture.root.path().join("projection.json");
    let body = format!(
        "#!{}\nimport json,pathlib\np=pathlib.Path({:?})\nw=pathlib.Path({:?})\nr=json.loads(p.read_text())\nif any((entry/'input').is_file() for entry in w.iterdir()): r['revisions'][0]['signed_source']=None\nprint(json.dumps(r))\n",
        python.display(),
        projection
            .to_str()
            .unwrap_or_else(|| unreachable!("fixture projection path is not UTF-8")),
        fixture
            .profile
            .hosted
            .workspace_root
            .to_str()
            .unwrap_or_else(|| unreachable!("fixture workspace path is not UTF-8")),
    );
    fixture.profile.helper = executable(&fixture.root.path().join("helper"), &body);
    let journal = fixture
        .profile
        .intake(
            &exact_trigger(&fixture.first)
                .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    fixture
        .profile
        .approve(&fixture.approval(&journal), 1)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert!(fixture.profile.drive(&journal.run_id).is_err());
    assert_eq!(
        fs::read(
            fixture
                .profile
                .hosted
                .workspace_root
                .join(&journal.run_id)
                .join("input")
        )
        .unwrap_or_else(|error| unreachable!("{error:?}")),
        b"first exact source\n"
    );
    let retained = fixture
        .profile
        .load(&journal.run_id)
        .unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"));
    assert_eq!(retained.state, PatchState::Ready);
    assert!(retained.request.is_none());
    assert!(!fixture.root.path().join("rpc.log").exists());
}
