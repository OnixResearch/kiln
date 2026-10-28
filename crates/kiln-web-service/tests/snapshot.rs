use base64::Engine as _;
use git2::{Repository, Signature};
use kiln_core::source::GitSnapshotLimits;
use kiln_web_service::snapshot::{MAX_SNAPSHOT_BYTES, SnapshotFile, capture, reconstruct};
use std::path::Path;

fn fixture() -> (tempfile::TempDir, Repository, String) {
    let root = tempfile::tempdir()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture directory", error));
    let repository = Repository::init(root.path())
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture repository", error));
    std::fs::write(root.path().join("input"), b"committed\n")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture input", error));
    std::fs::write(root.path().join("remove"), b"remove me\n")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture input", error));
    std::fs::write(root.path().join(".gitignore"), b"secret\n")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture ignore", error));
    let mut index = repository
        .index()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture index", error));
    for name in ["input", "remove", ".gitignore"] {
        index
            .add_path(Path::new(name))
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture stage", error));
    }
    index
        .write()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture index write", error));
    let tree = index
        .write_tree()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture tree", error));
    let signature = Signature::now("Fixture", "fixture@kiln.invalid")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture signature", error));
    let base = repository
        .commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            "base",
            &repository
                .find_tree(tree)
                .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture tree", error)),
            &[],
        )
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture commit", error));
    repository
        .set_head("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture head", error));
    (root, repository, base.to_string())
}
fn limits() -> GitSnapshotLimits {
    GitSnapshotLimits {
        max_entries: 100,
        max_path_bytes: 4096,
        max_file_bytes: MAX_SNAPSHOT_BYTES,
        max_total_bytes: MAX_SNAPSHOT_BYTES * 2,
    }
}

#[test]
fn explicit_snapshot_reconstructs_identically_on_independent_backing_without_changing_user_git() {
    let (root, repository, base) = fixture();
    let remote_root = tempfile::tempdir()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "worker storage", error));
    let independent = Repository::clone(
        root.path()
            .to_str()
            .unwrap_or_else(|| unreachable!("{}", "fixture path")),
        remote_root.path().join("backing"),
    )
    .unwrap_or_else(|error| unreachable!("{}: {:?}", "independent clone", error));
    let index_before = std::fs::read(repository.path().join("index"))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "index snapshot", error));
    std::fs::write(
        root.path().join("input"),
        b"explicit modified bytes\0\xff\n",
    )
    .unwrap_or_else(|error| unreachable!("{}: {:?}", "dirty bytes", error));
    std::fs::write(root.path().join("new"), b"explicit untracked\n")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "untracked bytes", error));
    std::fs::write(root.path().join("secret"), b"not uploaded")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "ignored fixture", error));
    std::fs::remove_file(root.path().join("remove"))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "explicit deletion", error));
    let manifest = capture(
        root.path(),
        "project".into(),
        "refs/heads/main".into(),
        base.clone(),
        &["new".into(), "input".into()],
        &["remove".into()],
    )
    .unwrap_or_else(|error| unreachable!("{}: {:?}", "snapshot capture", error));
    assert_eq!(
        manifest
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>(),
        ["input", "new", "remove"]
    );
    let a = reconstruct(root.path(), &manifest, &limits())
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "controller reconstruction", error));
    let b = reconstruct(
        independent
            .workdir()
            .unwrap_or_else(|| unreachable!("{}", "worker workdir")),
        &manifest,
        &limits(),
    )
    .unwrap_or_else(|error| unreachable!("{}: {:?}", "independent reconstruction", error));
    assert_eq!(a.revision, b.revision);
    assert_ne!(a.revision, base);
    assert!(
        repository
            .find_commit(
                git2::Oid::from_str(&a.revision)
                    .unwrap_or_else(|error| unreachable!("snapshot oid: {error}"))
            )
            .is_err()
    );
    let synthetic = Repository::open(&b.repository_path)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "synthetic repo", error));
    let commit = synthetic
        .find_commit(
            git2::Oid::from_str(&b.revision)
                .unwrap_or_else(|error| unreachable!("{}: {:?}", "snapshot oid", error)),
        )
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "synthetic commit", error));
    assert_eq!(
        commit
            .parent_id(0)
            .unwrap_or_else(|error| unreachable!("bound parent: {error}"))
            .to_string(),
        base
    );
    assert!(
        commit
            .message()
            .unwrap_or_else(|| unreachable!("provenance message"))
            .contains(
                &manifest
                    .identity()
                    .unwrap_or_else(|error| unreachable!("manifest identity: {error}"))
            )
    );
    let tree = commit
        .tree()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "synthetic tree", error));
    assert_eq!(
        synthetic
            .find_blob(
                tree.get_path(Path::new("input"))
                    .unwrap_or_else(|error| unreachable!("modified entry: {error}"))
                    .id()
            )
            .unwrap_or_else(|error| unreachable!("modified blob: {error}"))
            .content(),
        b"explicit modified bytes\0\xff\n"
    );
    assert_eq!(
        synthetic
            .find_blob(
                tree.get_path(Path::new("new"))
                    .unwrap_or_else(|error| unreachable!("new entry: {error}"))
                    .id()
            )
            .unwrap_or_else(|error| unreachable!("new blob: {error}"))
            .content(),
        b"explicit untracked\n"
    );
    assert!(tree.get_path(Path::new("remove")).is_err());
    assert!(tree.get_path(Path::new("secret")).is_err());
    assert_eq!(
        repository
            .head()
            .unwrap_or_else(|error| unreachable!("unchanged head: {error}"))
            .target()
            .unwrap_or_else(|| unreachable!("head target"))
            .to_string(),
        base
    );
    assert_eq!(
        std::fs::read(repository.path().join("index"))
            .unwrap_or_else(|error| unreachable!("unchanged index: {error}")),
        index_before
    );
    assert_eq!(
        std::fs::read(root.path().join("input"))
            .unwrap_or_else(|error| unreachable!("unchanged worktree: {error}")),
        b"explicit modified bytes\0\xff\n"
    );
}

#[test]
fn stale_link_ignored_duplicate_and_oversized_inputs_fail_closed() {
    let (root, _, base) = fixture();
    std::fs::write(root.path().join("secret"), b"private")
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "ignored fixture", error));
    std::os::unix::fs::symlink("input", root.path().join("link"))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "link fixture", error));
    for path in ["secret", "link", "../input", ".git/config"] {
        assert!(
            capture(
                root.path(),
                "project".into(),
                "refs/heads/main".into(),
                base.clone(),
                &[path.into()],
                &[]
            )
            .is_err()
        );
    }
    assert!(
        capture(
            root.path(),
            "project".into(),
            "refs/heads/main".into(),
            "f".repeat(40),
            &["input".into()],
            &[]
        )
        .is_err()
    );
    assert!(
        capture(
            root.path(),
            "project".into(),
            "refs/heads/main".into(),
            base.clone(),
            &["input".into(), "input".into()],
            &[]
        )
        .is_err()
    );
    assert!(
        capture(
            root.path(),
            "project".into(),
            "refs/heads/main".into(),
            base.clone(),
            &[],
            &["remove".into()]
        )
        .is_err()
    );
    let mut manifest = capture(
        root.path(),
        "project".into(),
        "refs/heads/main".into(),
        base,
        &["input".into()],
        &[],
    )
    .unwrap_or_else(|error| unreachable!("{}: {:?}", "unchanged selected input", error));
    assert!(reconstruct(root.path(), &manifest, &limits()).is_err());
    manifest
        .files
        .first_mut()
        .unwrap_or_else(|| unreachable!("{}", "selected fixture file"))
        .body_base64 =
        Some(base64::engine::general_purpose::STANDARD.encode(vec![0; MAX_SNAPSHOT_BYTES + 1]));
    assert!(manifest.admit().is_err());
    *manifest
        .files
        .first_mut()
        .unwrap_or_else(|| unreachable!("{}", "selected fixture file")) = SnapshotFile {
        path: "../escape".into(),
        executable: false,
        body_base64: Some("".into()),
    };
    assert!(manifest.admit().is_err());
}
