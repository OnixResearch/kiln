use super::*;
use fs2::FileExt as _;
use radicle::cob::change::Storage as _;
use radicle::cob::patch::{Action, MergeTarget};
use radicle::crypto::Signer as _;
use radicle::storage::ReadStorage as _;
use radicle::storage::SignRepository as _;
use radicle::storage::WriteRepository as _;

struct IssueCache;
impl Update<radicle::cob::issue::Issue> for IssueCache {
    type Out = ();
    type UpdateError = NoCacheError;
    fn update(
        &mut self,
        _: &RepoId,
        _: &radicle::cob::ObjectId,
        _: &radicle::cob::issue::Issue,
    ) -> Result<(), NoCacheError> {
        Ok(())
    }
}

// r[verify kiln.radicle.multiwriter]
// r[related kiln.radicle.mutation_conformance]
#[test]
fn concurrent_delegate_reviews_native_projection_append_and_stale_refusal() {
    let directory = tempfile::tempdir().unwrap();
    let alice = SigningKey::from_seed(Seed::new([11; Seed::BYTES]));
    let bob = SigningKey::from_seed(Seed::new([12; Seed::BYTES]));
    let kiln = SigningKey::from_seed(Seed::new([13; Seed::BYTES]));
    let outsider = SigningKey::from_seed(Seed::new([14; Seed::BYTES]));
    let storage = radicle::storage::git::Storage::open(
        directory.path().join("storage"),
        radicle::git::UserInfo {
            alias: radicle::node::Alias::new("fixture"),
            key: *alice.public_key(),
        },
    )
    .unwrap();
    let (rid, _, _, head) =
        radicle::test::fixtures::project(directory.path().join("project"), &storage, &alice)
            .unwrap();
    let repo = storage.repository(rid).unwrap();
    let mut identity = radicle::cob::identity::Identity::load_mut(&repo, &alice).unwrap();
    let mut doc = identity.doc().clone().edit();
    doc.delegate(bob.public_key().into());
    let allowed: radicle::identity::Did = alice.public_key().into();
    doc.payload.insert(
        radicle::identity::doc::PayloadId::canonical_refs(),
        radicle::identity::doc::Payload::from(serde_json::json!({"rules": {
            "refs/heads/master": {"allow": [allowed], "threshold": 1}
        }})),
    );
    identity
        .update(
            radicle::cob::Title::new("Add fixture delegate").unwrap(),
            "local fixture",
            &doc.verified().unwrap(),
        )
        .unwrap();
    repo.set_identity_head_to(identity.head()).unwrap();
    drop(identity);
    assert!(
        repo.delegates()
            .unwrap()
            .iter()
            .any(|did| did.as_key() == bob.public_key())
    );
    let mut patches = Patches::open(&repo, WriteAs::new(&alice)).unwrap();
    let mut cache = NoCache;
    let patch = patches
        .create(
            radicle::cob::Title::new("Concurrent fixture").unwrap(),
            "fixture",
            MergeTarget::default(),
            head,
            head,
            &[],
            &mut cache,
        )
        .unwrap();
    let id = *patch.id();
    let revision = patch.revisions().next().unwrap().0;
    drop(patch);
    drop(patches);
    let typename = &radicle::cob::patch::TYPENAME;
    let (root_heads, _) = history::history(&repo, typename, &id).unwrap();
    let writer_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(
            repo.backend
                .path()
                .join(format!("kiln-review-{id}-{}", alice.public_key())),
        )
        .unwrap();
    writer_lock.try_lock_exclusive().unwrap();
    assert!(
        append_review(
            &repo,
            &id.to_string(),
            &revision.to_string(),
            "accept",
            "must not race own writer",
            &root_heads,
            &alice
        )
        .is_err()
    );
    assert_eq!(
        history::history(&repo, &radicle::cob::patch::TYPENAME, &id)
            .unwrap()
            .0,
        root_heads
    );
    fs2::FileExt::unlock(&writer_lock).unwrap();
    let stale_snapshot = review::ReviewSnapshot::capture(&repo).unwrap();
    let alice_review = append_review(
        &repo,
        &id.to_string(),
        &revision.to_string(),
        "accept",
        "Alice review",
        &root_heads,
        &alice,
    )
    .unwrap();
    let alice_tip = alice_review["review_id"].as_str().unwrap();
    let alice_ref = format!(
        "refs/namespaces/{}/refs/cobs/xyz.radicle.patch/{id}",
        alice.public_key()
    );
    assert!(
        stale_snapshot
            .publish(&repo, &id, alice.public_key())
            .is_err()
    );
    assert_eq!(
        repo.backend.refname_to_id(&alice_ref).unwrap().to_string(),
        alice_tip
    );
    let (after_external_write, _) = history::history(&repo, typename, &id).unwrap();
    assert!(history::require_heads(&root_heads, &after_external_write).is_err());
    // Simulate two peers reviewing the same observed root without any network.
    repo.backend
        .reference(
            &alice_ref,
            radicle::git::raw::Oid::from_str(&root_heads.first().unwrap()).unwrap(),
            true,
            "fixture concurrent root",
        )
        .unwrap();
    repo.sign_refs(&alice).unwrap();
    append_review(
        &repo,
        &id.to_string(),
        &revision.to_string(),
        "reject",
        "Bob review",
        &root_heads,
        &bob,
    )
    .unwrap();
    repo.backend
        .reference(
            &alice_ref,
            radicle::git::raw::Oid::from_str(alice_tip).unwrap(),
            true,
            "fixture restore peer head",
        )
        .unwrap();
    repo.sign_refs(&alice).unwrap();
    let (heads, _) = history::history(&repo, typename, &id).unwrap();
    assert_eq!(heads.len(), 2);
    let upstream = Patches::open(&repo, ReadOnly)
        .unwrap()
        .get(&id)
        .unwrap()
        .unwrap();
    assert_eq!(upstream.reviews_of(revision).count(), 2);
    let projection = read_patch(
        storage.path().to_path_buf(),
        &rid.to_string(),
        &id.to_string(),
    )
    .unwrap();
    assert_eq!(
        projection["native"],
        serde_json::to_value(&upstream).unwrap()
    );
    assert_eq!(
        projection["revisions"][0]["reviews"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let result = append_review(
        &repo,
        &id.to_string(),
        &revision.to_string(),
        "accept",
        "Kiln review",
        &heads,
        &kiln,
    )
    .unwrap();
    let written = radicle::git::Oid::from_str(result["review_id"].as_str().unwrap()).unwrap();
    let entry = repo.load(written).unwrap();
    for observed in &heads {
        assert!(
            entry
                .parents
                .iter()
                .any(|parent| parent.to_string() == *observed)
        );
    }
    let final_patch = Patches::open(&repo, ReadOnly)
        .unwrap()
        .get(&id)
        .unwrap()
        .unwrap();
    assert_eq!(final_patch.reviews_of(revision).count(), 3);
    assert_eq!(final_patch.merges().count(), 0);
    // Native authorization must discard a non-delegate merge outside the explicit crefs allow list.
    let action = serde_json::to_vec(&Action::Merge {
        revision,
        commit: head.into(),
    })
    .unwrap();
    let denied = repo
        .store(
            Some(repo.identity_head().unwrap()),
            Vec::new(),
            &outsider,
            radicle::cob::change::Template {
                tips: vec![written],
                embeds: Vec::new(),
                contents: (action, Vec::new()).into(),
                type_name: (**typename).clone(),
                message: "foreign denied merge fixture".into(),
            },
        )
        .unwrap();
    radicle::cob::object::Storage::update(&repo, outsider.public_key(), typename, &id, &denied.id)
        .unwrap();
    repo.sign_refs(&outsider).unwrap();
    let reduced = Patches::open(&repo, ReadOnly)
        .unwrap()
        .get(&id)
        .unwrap()
        .unwrap();
    assert_eq!(reduced.merges().count(), 0);
    assert_eq!(reduced.reviews_of(revision).count(), 3);
    let foreign = |contents: Vec<u8>, tips: Vec<radicle::git::Oid>, message: &str| {
        repo.store(
            Some(repo.identity_head().unwrap()),
            Vec::new(),
            &outsider,
            radicle::cob::change::Template {
                tips,
                embeds: Vec::new(),
                contents: (contents, Vec::new()).into(),
                type_name: (**typename).clone(),
                message: message.into(),
            },
        )
        .unwrap()
    };
    let install = |tip: radicle::git::Oid| {
        radicle::cob::object::Storage::update(&repo, outsider.public_key(), typename, &id, &tip)
            .unwrap();
        repo.sign_refs(&outsider).unwrap();
    };
    let disconnected = foreign(
        serde_json::to_vec(&Action::Merge {
            revision,
            commit: head.into(),
        })
        .unwrap(),
        Vec::new(),
        "disconnected signed foreign branch",
    );
    install(disconnected.id);
    assert!(
        read_patch(
            storage.path().to_path_buf(),
            &rid.to_string(),
            &id.to_string()
        )
        .is_err()
    );
    install(denied.id);
    let opaque = foreign(
        br#"{"type":"unknown.future.action"}"#.to_vec(),
        vec![written],
        "opaque signed foreign action",
    );
    install(opaque.id);
    assert!(
        read_patch(
            storage.path().to_path_buf(),
            &rid.to_string(),
            &id.to_string()
        )
        .is_err()
    );
    install(denied.id);
    install(written);
    assert!(
        read_patch(
            storage.path().to_path_buf(),
            &rid.to_string(),
            &id.to_string()
        )
        .is_err()
    );
    install(denied.id);
    let moved = foreign(
        serde_json::to_vec(&Action::Merge {
            revision,
            commit: head.into(),
        })
        .unwrap(),
        vec![written],
        "valid change without signed ref update",
    );
    radicle::cob::object::Storage::update(&repo, outsider.public_key(), typename, &id, &moved.id)
        .unwrap();
    assert!(
        read_patch(
            storage.path().to_path_buf(),
            &rid.to_string(),
            &id.to_string()
        )
        .is_err()
    );
    install(denied.id);
    let accepted_projection = read_patch(
        storage.path().to_path_buf(),
        &rid.to_string(),
        &id.to_string(),
    )
    .unwrap();
    assert_eq!(
        accepted_projection["revisions"][0]["reviews"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(
        append_review(
            &repo,
            &id.to_string(),
            &revision.to_string(),
            "accept",
            "stale review",
            &heads,
            &kiln
        )
        .is_err()
    );
    let git_signature =
        radicle::git::raw::Signature::now("unsigned fixture", "unsigned@kiln.invalid").unwrap();
    let mut issues = radicle::cob::issue::Issues::open(&repo, WriteAs::new(&alice)).unwrap();
    let mut issue_cache = IssueCache;
    let issue = issues
        .create(
            radicle::cob::Title::new("Concurrent issue").unwrap(),
            "root issue",
            &[],
            &[],
            Vec::new(),
            &mut issue_cache,
        )
        .unwrap();
    let issue_id = *issue.id();
    let root_comment = *issue.comments().next().unwrap().0;
    drop(issue);
    let (issue_roots, _) =
        history::history(&repo, &radicle::cob::issue::TYPENAME, &issue_id).unwrap();
    let mut issue = issues.get_mut(&issue_id, &mut issue_cache).unwrap();
    let alice_comment = issue
        .comment("Alice concurrent comment", root_comment, Vec::new())
        .unwrap();
    drop(issue);
    drop(issues);
    let issue_ref = format!(
        "refs/namespaces/{}/refs/cobs/xyz.radicle.issue/{issue_id}",
        alice.public_key()
    );
    repo.backend
        .reference(
            &issue_ref,
            radicle::git::raw::Oid::from_str(issue_roots.first().unwrap()).unwrap(),
            true,
            "concurrent issue fixture",
        )
        .unwrap();
    repo.sign_refs(&alice).unwrap();
    let mut issues = radicle::cob::issue::Issues::open(&repo, WriteAs::new(&bob)).unwrap();
    let mut issue = issues.get_mut(&issue_id, &mut issue_cache).unwrap();
    issue
        .comment("Bob concurrent comment", root_comment, Vec::new())
        .unwrap();
    drop(issue);
    drop(issues);
    repo.backend
        .reference(
            &issue_ref,
            alice_comment.into(),
            true,
            "restore concurrent issue head",
        )
        .unwrap();
    repo.sign_refs(&alice).unwrap();
    let issue_projection = read_issue(
        storage.path().to_path_buf(),
        &rid.to_string(),
        &issue_id.to_string(),
    )
    .unwrap();
    assert_eq!(issue_projection["heads"].as_array().unwrap().len(), 2);
    let native_issue = radicle::cob::issue::Issues::open(&repo, ReadOnly)
        .unwrap()
        .get(&issue_id)
        .unwrap()
        .unwrap();
    assert_eq!(native_issue.comments().count(), 3);
    assert_eq!(
        issue_projection["native"],
        serde_json::to_value(native_issue).unwrap()
    );
    let unsigned_tree = repo.backend.find_tree(denied.revision.into()).unwrap();
    let unsigned = repo
        .backend
        .commit(
            None,
            &git_signature,
            &git_signature,
            "unsigned foreign change",
            &unsigned_tree,
            &[],
        )
        .unwrap();
    let outsider_ref = format!(
        "refs/namespaces/{}/refs/cobs/xyz.radicle.patch/{id}",
        outsider.public_key()
    );
    repo.backend
        .reference(&outsider_ref, unsigned, true, "unsigned fixture only")
        .unwrap();
    repo.sign_refs(&outsider).unwrap();
    assert!(
        read_patch(
            storage.path().to_path_buf(),
            &rid.to_string(),
            &id.to_string()
        )
        .is_err()
    );
    println!(
        "fixture {rid}/{id}: two concurrent delegate reviews match native state; denied merge ignored; stale write refused; own review includes all observed heads"
    );
    if std::env::var_os("KILN_RETAIN_RADICLE_FIXTURE").is_some() {
        repo.backend
            .reference(
                &outsider_ref,
                denied.id.into(),
                true,
                "restore signed fixture head",
            )
            .unwrap();
        repo.sign_refs(&outsider).unwrap();
        let heads = history::history(&repo, typename, &id).unwrap().0;
        println!(
            "{}",
            serde_json::json!({"fixture_root":directory.keep(), "repository":rid.to_string(),
            "patch":id.to_string(), "revision":revision.to_string(), "heads":heads, "issue":issue_id.to_string()})
        );
    }
}
