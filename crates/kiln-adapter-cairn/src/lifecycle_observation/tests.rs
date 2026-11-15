use super::*;
use kiln_core::implementation::*;
use kiln_implementer::publication_effects::CairnLifecycleAction;

fn id(c: char) -> ContentId {
    ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}
fn external(kind: &str, id: &str) -> ExternalRef {
    ExternalRef::new("git", kind, id).unwrap_or_else(|_| unreachable!())
}
fn tree(repo: &Repository, entries: &[(&str, Oid, i32)]) -> Oid {
    let mut builder = repo.treebuilder(None).unwrap_or_else(|_| unreachable!());
    for (name, oid, mode) in entries {
        builder
            .insert(*name, *oid, *mode)
            .unwrap_or_else(|_| unreachable!());
    }
    builder.write().unwrap_or_else(|_| unreachable!())
}
fn mutation(entries: Value, kind: &str) -> Value {
    let before = serde_json::json!({"entries":[], "manifest_hash":canonical_json_hash(&serde_json::json!({"entries":[]}))});
    let after = serde_json::json!({"manifest_hash":canonical_json_hash(&serde_json::json!({"entries":entries})), "entries":entries});
    serde_json::json!({"kind":kind, "before":before, "after":after,
        "manifest_hash":canonical_json_hash(&serde_json::json!({"before":before, "after":after}))})
}

#[test]
// Native producer-shaped synthetic facts and real local Git blobs; not a live
// sync/archive execution or remote integration qualification.
fn observes_reviewed_native_sync_with_exact_accepted_blobs_and_native_receipt_hash() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let repo =
        Repository::init_bare(dir.path().join("source.git")).unwrap_or_else(|_| unreachable!());
    let spec = b"accepted reviewed specification";
    let spec_oid = repo.blob(spec).unwrap_or_else(|_| unreachable!());
    let spec_dir = tree(&repo, &[("spec.md", spec_oid, 0o100644)]);
    let specs = tree(&repo, &[("demo", spec_dir, 0o040000)]);
    let mutation = mutation(
        serde_json::json!([{"path":"./.cairn/specs/demo/spec.md", "exists":true,
        "content_hash":blake3::hash(spec).to_hex().to_string()}]),
        "sync",
    );
    let mut receipt = serde_json::json!({"change":"demo", "dry_run":false,"mutated":true,"blocked":false,
        "actions":[], "layout":"cairn", "reasons":[], "policy":"fixture", "policy_hash":id('5').as_str(),
        "input_hash":id('6').as_str(), "plan_hash":id('7').as_str(), "mutation_manifest":mutation});
    let native_hash = cairn_core::lifecycle_receipt_hash(
        &receipt,
        &cairn_core::default_lifecycle_receipt_hash_profile(),
    );
    receipt
        .as_object_mut()
        .unwrap_or_else(|| unreachable!())
        .insert("receipt_hash".into(), serde_json::json!(native_hash));
    let receipt_oid = repo
        .blob(&serde_json::to_vec(&receipt).unwrap_or_else(|_| unreachable!()))
        .unwrap_or_else(|_| unreachable!());
    let cairn = tree(
        &repo,
        &[
            ("specs", specs, 0o040000),
            ("native.json", receipt_oid, 0o100644),
        ],
    );
    let root = tree(&repo, &[(".cairn", cairn, 0o040000)]);
    let root_tree = repo.find_tree(root).unwrap_or_else(|_| unreachable!());
    let signature = git2::Signature::now("fixture", "fixture@example.invalid")
        .unwrap_or_else(|_| unreachable!());
    let commit = repo
        .commit(None, &signature, &signature, "fixture", &root_tree, &[])
        .unwrap_or_else(|_| unreachable!());
    let mut report = serde_json::json!({"schema":"cairn.agent-completion.v2", "valid":true,
        "repository_id":"onix/kiln", "policy_hash":id('5').as_str()});
    let authoritative_hash = canonical_json_hash(&report);
    report
        .as_object_mut()
        .unwrap_or_else(|| unreachable!())
        .insert("receipt_hash".into(), serde_json::json!(authoritative_hash));
    let report_path = dir.path().join("authoritative.json");
    std::fs::write(
        &report_path,
        serde_json::to_vec(&report).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let candidate = external("commit", &commit.to_string());
    let reviewed_tree = external("tree", &root.to_string());
    let plan = PublicationPlan {
        schema: PUBLICATION_SCHEMA.into(),
        work_identity: id('1'),
        work_revision: 1,
        attempt: id('2'),
        fence: 3,
        handoff: id('3'),
        verification: id('4'),
        repository: "onix/kiln".into(),
        base: candidate.clone(),
        base_branch: "main".into(),
        candidate: candidate.clone(),
        tree: reviewed_tree.clone(),
        branch: "kiln/fixture".into(),
        title: "reviewed".into(),
        body: "native lifecycle".into(),
    };
    let pr = ExternalRef::new("github", "pull_request", "onix/kiln#9")
        .unwrap_or_else(|_| unreachable!());
    let integration = IntegrationReceipt {
        schema: INTEGRATION_SCHEMA.into(),
        work_identity: plan.work_identity.clone(),
        work_revision: 1,
        attempt: plan.attempt.clone(),
        fence: 3,
        handoff: plan.handoff.clone(),
        verification: plan.verification.clone(),
        candidate: candidate.clone(),
        pull_request: pr.clone(),
        merge_commit: candidate.clone(),
        base_ref: "main".into(),
        base_head: candidate.clone(),
        mapping: IntegrationMapping {
            expected_base: candidate.clone(),
            candidate_tree: reviewed_tree.clone(),
            landed_tree: reviewed_tree.clone(),
        },
    };
    let publication = PublicationRecord {
        schema: PUBLICATION_SCHEMA.into(),
        plan,
        phase: PublicationPhase::Integrated,
        remote_head: Some(candidate.clone()),
        pull_request: Some(pr),
        integration: Some(integration),
        source_status: None,
    };
    let binding = CairnLifecycleBinding {
        source: SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", "demo/task:build")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "demo/task:build".into(),
            version: id('9'),
        },
        handoff: id('3'),
        verification: id('4'),
        authoritative_completion: ContentId::from_blake3_hex(authoritative_hash)
            .unwrap_or_else(|_| unreachable!()),
        candidate,
        action: CairnLifecycleAction::ObserveReviewedSync,
    };
    let observed = observe_reviewed_lifecycle(
        repo.path(),
        &publication,
        &binding,
        ".cairn/native.json",
        &report_path,
    )
    .unwrap_or_else(|error| unreachable!("{error}"));

    let native = NativeGitReceipt {
        schema: NATIVE_GIT_RECEIPT_SCHEMA.into(),
        plan: id('0'),
        work_identity: id('1'),
        work_revision: 1,
        attempt: id('2'),
        fence: 3,
        handoff: id('3'),
        verification: id('4'),
        review: id('a'),
        checks: id('b'),
        policy: id('5'),
        repository: "onix/kiln".into(),
        remote_identity: id('c'),
        provider_profile: "native-git-v1".into(),
        branch_expected_old: None,
        branch_ref: "refs/heads/kiln/native".into(),
        target_ref: "refs/heads/main".into(),
        expected_old_target: binding.candidate.clone(),
        candidate: binding.candidate.clone(),
        tree: reviewed_tree.clone(),
        branch_operation: id('d'),
        integration_operation: id('e'),
        resulting_commit: binding.candidate.clone(),
    };
    let native_observed = observe_native_git_lifecycle(
        repo.path(),
        &native,
        &binding,
        ".cairn/native.json",
        &report_path,
    )
    .unwrap_or_else(|error| unreachable!("native receipt consumer failed: {error}"));
    assert_eq!(native_observed.observed_tree, reviewed_tree);
    assert_eq!(
        native_observed.lifecycle_receipt.native_id.as_str(),
        native_hash
    );
    let mut wrong_native = native.clone();
    wrong_native.resulting_commit = external("commit", &"f".repeat(40));
    assert!(
        observe_native_git_lifecycle(
            repo.path(),
            &wrong_native,
            &binding,
            ".cairn/native.json",
            &report_path,
        )
        .is_err()
    );
    assert_eq!(observed.observed_tree, reviewed_tree);
    assert_eq!(observed.lifecycle_policy, id('5'));
    assert_eq!(observed.lifecycle_receipt.native_id.as_str(), native_hash);
    let mut wrong = binding.clone();
    wrong.source.native_id = "another/task:build".into();
    assert!(
        observe_reviewed_lifecycle(
            repo.path(),
            &publication,
            &wrong,
            ".cairn/native.json",
            &report_path
        )
        .is_err()
    );
    let mut wrong = binding.clone();
    wrong.authoritative_completion = id('a');
    assert!(
        observe_reviewed_lifecycle(
            repo.path(),
            &publication,
            &wrong,
            ".cairn/native.json",
            &report_path
        )
        .is_err()
    );
    assert!(
        observe_reviewed_lifecycle(
            repo.path(),
            &publication,
            &binding,
            "../native.json",
            &report_path
        )
        .is_err()
    );
}

#[test]
fn archive_requires_positive_exact_change_and_content_not_directory_absence() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let repo = Repository::init_bare(dir.path()).unwrap_or_else(|_| unreachable!());
    let empty = tree(&repo, &[]);
    let empty = repo.find_tree(empty).unwrap_or_else(|_| unreachable!());
    let only_absence = mutation(
        serde_json::json!([{"path":".cairn/changes/demo","exists":false,"content_hash":null}]),
        "archive",
    );
    assert!(
        verify_after_manifest(
            &repo,
            &empty,
            "demo",
            CairnLifecycleAction::ObserveReviewedArchive,
            &only_absence
        )
        .is_err()
    );
    let blob = repo.blob(b"reviewed").unwrap_or_else(|_| unreachable!());
    let destination = tree(
        &repo,
        &[
            ("tasks.md", blob, 0o100644),
            ("proposal.md", blob, 0o100644),
        ],
    );
    let archive = tree(&repo, &[("1970-01-01-demo", destination, 0o040000)]);
    let cairn = tree(&repo, &[("archive", archive, 0o040000)]);
    let root = tree(&repo, &[(".cairn", cairn, 0o040000)]);
    let root = repo.find_tree(root).unwrap_or_else(|_| unreachable!());
    let digest = blake3::hash(b"reviewed").to_hex().to_string();
    let entries = serde_json::json!([
        {"path":".cairn/archive/1970-01-01-demo/tasks.md","exists":true,"content_hash":digest},
        {"path":".cairn/archive/1970-01-01-demo/proposal.md","exists":true,"content_hash":digest},
        {"path":".cairn/changes/demo","exists":false,"content_hash":null}]);
    assert!(
        verify_after_manifest(
            &repo,
            &root,
            "demo",
            CairnLifecycleAction::ObserveReviewedArchive,
            &mutation(entries.clone(), "archive")
        )
        .is_ok()
    );
    assert!(
        verify_after_manifest(
            &repo,
            &root,
            "other",
            CairnLifecycleAction::ObserveReviewedArchive,
            &mutation(entries.clone(), "archive")
        )
        .is_err()
    );
    let mut corrupt = entries;
    *corrupt
        .get_mut(0)
        .and_then(|row| row.get_mut("content_hash"))
        .unwrap_or_else(|| unreachable!()) = serde_json::json!(id('f').as_str());
    assert!(
        verify_after_manifest(
            &repo,
            &root,
            "demo",
            CairnLifecycleAction::ObserveReviewedArchive,
            &mutation(corrupt, "archive")
        )
        .is_err()
    );
}
