// r[verify kiln.web.developer_loop.cache]
use super::*;
use std::os::unix::fs::PermissionsExt;

pub(super) fn key() -> CacheKey {
    CacheKey {
        project_id: "project".into(),
        node_id: "build".into(),
        producer: "builder-v1".into(),
        toolchain: "rustc-reviewed-v1".into(),
        platform: "linux-x86_64".into(),
        inputs: BTreeMap::from([(
            PathBuf::from("Cargo.lock"),
            blake3::hash(b"lock-v1").to_hex().to_string(),
        )]),
        paths: vec!["target/cache".into()],
    }
}
pub(super) fn envelope(bytes: &[u8]) -> CacheEnvelope {
    CacheEnvelope::new(CacheEntry {
        key: key(),
        files: vec![CacheFile {
            path: "target/cache/object".into(),
            blake3: blake3::hash(bytes).to_hex().to_string(),
            body_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        }],
    })
    .unwrap_or_else(|error| unreachable!("{}: {:?}", "cache fixture operation succeeds", error))
}
fn profile(path: &Path) -> CacheProfile {
    CacheProfile {
        schema: "kiln.web-cache.v1".into(),
        directory: path.into(),
        maximum_entry_bytes: 4096,
        maximum_total_bytes: 16384,
        maximum_entries: 2,
        maximum_age_seconds: 60,
        maximum_files: 8,
        projects: vec![CacheProject {
            project_id: "project".into(),
            producers: vec!["builder-v1".into()],
            toolchains: vec!["rustc-reviewed-v1".into()],
            platforms: vec!["linux-x86_64".into()],
            paths: vec!["target/cache".into()],
        }],
    }
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "cache fixture operation succeeds", error))
        .as_secs()
}
fn private_root() -> tempfile::TempDir {
    tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "cache fixture operation succeeds", error))
}

#[test]
fn cold_publish_warm_restore_is_immutable_and_survives_store_reopen() {
    let root = private_root();
    let policy = profile(root.path());
    let mut store = CacheStore::open(policy.clone()).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    assert!(
        store
            .restore(&key(), now())
            .unwrap_or_else(|error| unreachable!("cold cache restore: {error}"))
            .entry
            .is_none()
    );
    let entry = envelope(b"actual compiler cache bytes");
    assert!(
        store
            .publish(&entry, now())
            .unwrap_or_else(|error| unreachable!("initial publication: {error}"))
    );
    assert!(
        !store
            .publish(&entry, now())
            .unwrap_or_else(|error| unreachable!("idempotent publication: {error}"))
    );
    assert_eq!(
        store
            .restore(&key(), now())
            .unwrap_or_else(|error| unreachable!("warm restore: {error}"))
            .entry
            .unwrap_or_else(|| unreachable!("published entry"))
            .entry,
        entry.entry
    );
    assert!(
        store
            .publish(&envelope(b"different bytes under same input key"), now())
            .is_err()
    );
    drop(store);
    let mut second = CacheStore::open(policy).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    assert_eq!(
        second
            .restore(&key(), now())
            .unwrap_or_else(|error| unreachable!("reopened restore: {error}"))
            .entry
            .unwrap_or_else(|| unreachable!("retained entry"))
            .blake3,
        entry.blake3
    );
}
#[test]
fn content_key_scope_path_and_bounds_are_admitted_before_storage() {
    let root = private_root();
    let mut store = CacheStore::open(profile(root.path())).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    let good = envelope(b"verified");
    assert_eq!(
        good.verify(4096, 8)
            .unwrap_or_else(|error| unreachable!("verified content: {error}")),
        8
    );
    let mut bad = good.clone();
    bad.entry
        .files
        .first_mut()
        .unwrap_or_else(|| unreachable!("{}", "fixture includes a cache file"))
        .body_base64 = base64::engine::general_purpose::STANDARD.encode(b"tampered");
    assert!(store.publish(&bad, now()).is_err());
    bad = good.clone();
    bad.entry
        .files
        .first_mut()
        .unwrap_or_else(|| unreachable!("{}", "fixture includes a cache file"))
        .path = "../escape".into();
    assert!(store.publish(&bad, now()).is_err());
    bad = good.clone();
    bad.entry
        .files
        .first_mut()
        .unwrap_or_else(|| unreachable!("{}", "fixture includes a cache file"))
        .path = "target/cache/.env".into();
    assert!(store.publish(&bad, now()).is_err());
    bad = good.clone();
    bad.entry.key.project_id = "other-project".into();
    assert!(store.publish(&bad, now()).is_err());
    bad = good.clone();
    bad.entry.key.toolchain = "unreviewed".into();
    assert!(store.publish(&bad, now()).is_err());
    assert!(good.verify(7, 8).is_err());
    assert!(good.verify(4096, 0).is_err());
    assert!(
        store
            .restore(&key(), now())
            .unwrap_or_else(|error| unreachable!("cold restore: {error}"))
            .entry
            .is_none()
    );
}
#[test]
fn corrupted_retained_content_and_symlinks_never_restore() {
    let root = private_root();
    let mut store = CacheStore::open(profile(root.path())).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    let entry = envelope(b"verified");
    store.publish(&entry, now()).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    let path = store
        .directory
        .join(key().identity().unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        }));
    std::fs::write(&path, b"corrupt").unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    let restored = store
        .restore(&key(), now())
        .unwrap_or_else(|error| unreachable!("measured corrupt restore: {error}"));
    assert!(restored.entry.is_none());
    assert_eq!(restored.outcome, RestoreOutcome::CorruptMiss);
    std::fs::remove_file(&path).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    let outside = root.path().join("outside");
    std::fs::write(&outside, b"private").unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    std::os::unix::fs::symlink(&outside, &path).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    let restored = store
        .restore(&key(), now())
        .unwrap_or_else(|error| unreachable!("measured symlink restore: {error}"));
    assert!(restored.entry.is_none());
    assert_eq!(restored.outcome, RestoreOutcome::CorruptMiss);
    assert_eq!(
        std::fs::read(outside).unwrap_or_else(|error| unreachable!("outside file: {error}")),
        b"private"
    );
}
#[test]
fn count_and_age_retention_only_remove_verified_cache_entries() {
    let root = private_root();
    let canonical = root.path().join("canonical-journal");
    std::fs::write(&canonical, b"never prune").unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    let mut policy = profile(root.path());
    policy.maximum_entries = 1;
    let mut store = CacheStore::open(policy).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    let first = envelope(b"first");
    store.publish(&first, now()).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    let mut second = envelope(b"second");
    second.entry.key.inputs.insert(
        "Cargo.lock".into(),
        blake3::hash(b"new lock").to_hex().to_string(),
    );
    second = CacheEnvelope::new(second.entry).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    store.publish(&second, now()).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    assert!(
        store
            .restore(&first.entry.key, now())
            .unwrap_or_else(|error| unreachable!("evicted lookup: {error}"))
            .entry
            .is_none()
    );
    assert!(
        store
            .restore(&second.entry.key, now())
            .unwrap_or_else(|error| unreachable!("retained lookup: {error}"))
            .entry
            .is_some()
    );
    assert!(
        store
            .restore(&second.entry.key, now() + 61)
            .unwrap_or_else(|error| unreachable!("expired lookup: {error}"))
            .entry
            .is_none()
    );
    assert_eq!(
        std::fs::read(canonical).unwrap_or_else(|error| unreachable!("canonical journal: {error}")),
        b"never prune"
    );
}
#[test]
fn unknown_retained_data_and_nonprivate_directory_are_preserved_and_rejected() {
    let root = private_root();
    let policy = profile(root.path());
    let mut store = CacheStore::open(policy.clone()).unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    let historical = store.directory.join("unknown-historical-data");
    std::fs::write(&historical, b"keep").unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    assert!(store.publish(&envelope(b"new"), now()).is_err());
    assert_eq!(
        std::fs::read(historical).unwrap_or_else(|error| unreachable!("historical file: {error}")),
        b"keep"
    );
    drop(store);
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o755)).unwrap_or_else(
        |error| unreachable!("{}: {:?}", "cache fixture operation succeeds", error),
    );
    assert!(policy.preflight().is_err());
}

#[test]
fn staging_rejects_symlinks_and_nonprivate_directories_without_touching_targets() {
    let root = private_root();
    let outside = private_root();
    let policy = profile(root.path());
    let store = CacheStore::open(policy.clone()).unwrap_or_else(|error| unreachable!("{error}"));
    drop(store);
    let staging = root.path().join("kiln-cache-v1/staging");
    fs::remove_dir(&staging).unwrap_or_else(|error| unreachable!("{error}"));
    let retained = outside.path().join("retained");
    fs::write(&retained, b"not an orphan").unwrap_or_else(|error| unreachable!("{error}"));
    std::os::unix::fs::symlink(outside.path(), &staging)
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(CacheStore::open(policy.clone()).is_err());
    assert_eq!(
        fs::read(&retained).unwrap_or_else(|error| unreachable!("{error}")),
        b"not an orphan"
    );
    fs::remove_file(&staging).unwrap_or_else(|error| unreachable!("{error}"));
    fs::create_dir(&staging).unwrap_or_else(|error| unreachable!("{error}"));
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert!(CacheStore::open(policy).is_err());
}
#[test]
fn input_producer_toolchain_platform_and_output_declarations_change_cache_identity() {
    let original = key();
    let original_id = original.identity().unwrap_or_else(|error| {
        unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
    });
    for variant in 0..6 {
        let mut changed = original.clone();
        match variant {
            0 => changed.producer.push_str("-new"),
            1 => changed.toolchain.push_str("-new"),
            2 => changed.platform.push_str("-new"),
            3 => {
                changed.inputs.insert(
                    "Cargo.lock".into(),
                    blake3::hash(b"other").to_hex().to_string(),
                );
            }
            4 => changed.project_id.push_str("-new"),
            _ => changed.paths.push("other-cache".into()),
        }
        assert_ne!(
            changed
                .identity()
                .unwrap_or_else(|error| unreachable!("changed key: {error}")),
            original_id
        );
    }
}

// r[verify kiln.hosted.microvm.cache_integrity]
#[test]
fn hostile_returns_cannot_replace_existing_entry() {
    let root = private_root();
    let mut store = CacheStore::open(profile(root.path())).unwrap_or_else(|e| unreachable!("{e}"));
    let original = envelope(b"immutable");
    assert!(
        store
            .publish(&original, now())
            .unwrap_or_else(|e| unreachable!("{e}"))
    );
    let path = store
        .directory
        .join(key().identity().unwrap_or_else(|e| unreachable!("{e}")));
    let before = fs::read(&path).unwrap_or_else(|e| unreachable!("{e}"));
    let mut mismatch = original.clone();
    mismatch.blake3 = "0".repeat(64);
    for hostile in [envelope(&vec![0; 4097]), mismatch, envelope(b"second send")] {
        assert!(store.publish(&hostile, now()).is_err());
        assert_eq!(
            fs::read(&path).unwrap_or_else(|e| unreachable!("{e}")),
            before
        );
    }
}

#[test]
fn interrupted_publication_child() {
    let Some(root) = std::env::var_os("KILN_CACHE_FAULT_ROOT") else {
        return;
    };
    let phase = std::env::var("KILN_CACHE_FAULT_PHASE").unwrap_or_else(|e| unreachable!("{e}"));
    let mut store =
        CacheStore::open(profile(Path::new(&root))).unwrap_or_else(|e| unreachable!("{e}"));
    let mut entry = envelope(b"new complete entry");
    entry.entry.key.node_id = "second".into();
    let entry = CacheEnvelope::new(entry.entry).unwrap_or_else(|e| unreachable!("{e}"));
    store
        .publish_with_hook(&entry, now(), |current| {
            if format!("{current:?}") == phase {
                fs::write(Path::new(&root).join("phase-ready"), b"ready")
                    .unwrap_or_else(|e| unreachable!("{e}"));
                loop {
                    std::thread::park();
                }
            }
        })
        .unwrap_or_else(|e| unreachable!("{e}"));
    unreachable!("requested phase was not reached");
}

#[test]
fn sigkill_each_publish_phase_recovers_without_partial_keys() {
    use std::os::unix::process::ExitStatusExt;
    for phase in [
        PublishPhase::Receiving,
        PublishPhase::Verifying,
        PublishPhase::BeforeRename,
        PublishPhase::AfterRename,
    ] {
        let root = private_root();
        let policy = profile(root.path());
        let original = envelope(b"existing entry");
        let mut store = CacheStore::open(policy.clone()).unwrap_or_else(|e| unreachable!("{e}"));
        store
            .publish(&original, now())
            .unwrap_or_else(|e| unreachable!("{e}"));
        let id = key().identity().unwrap_or_else(|e| unreachable!("{e}"));
        let original_bytes =
            fs::read(store.directory.join(&id)).unwrap_or_else(|e| unreachable!("{e}"));
        drop(store);
        let mut child = std::process::Command::new(
            std::env::current_exe().unwrap_or_else(|e| unreachable!("{e}")),
        )
        .args([
            "--exact",
            "cache::tests::interrupted_publication_child",
            "--nocapture",
        ])
        .env("KILN_CACHE_FAULT_ROOT", root.path())
        .env("KILN_CACHE_FAULT_PHASE", format!("{phase:?}"))
        .spawn()
        .unwrap_or_else(|e| unreachable!("{e}"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !root.path().join("phase-ready").exists() {
            if let Some(status) = child.try_wait().unwrap_or_else(|e| unreachable!("{e}")) {
                unreachable!("fault child exited before {phase:?}: {status}");
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                unreachable!("fault child did not reach {phase:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        child.kill().unwrap_or_else(|e| unreachable!("{e}")); // Unix Child::kill sends SIGKILL.
        assert_eq!(
            child
                .wait()
                .unwrap_or_else(|e| unreachable!("{e}"))
                .signal(),
            Some(9)
        );
        let mut reopened = CacheStore::open(policy).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            fs::read(reopened.directory.join(&id)).unwrap_or_else(|e| unreachable!("{e}")),
            original_bytes
        );
        let mut second = key();
        second.node_id = "second".into();
        let restored = reopened
            .restore(&second, now())
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(restored.entry.is_some(), phase == PublishPhase::AfterRename);
        if let Some(entry) = restored.entry {
            assert_eq!(
                entry
                    .entry
                    .files
                    .first()
                    .unwrap_or_else(|| unreachable!("one fixture file"))
                    .body_base64,
                base64::engine::general_purpose::STANDARD.encode(b"new complete entry")
            );
            assert_eq!(
                entry
                    .verify(4096, 8)
                    .unwrap_or_else(|e| unreachable!("{e}")),
                18
            );
        }
        assert_eq!(
            fs::read_dir(reopened.directory.join("staging"))
                .unwrap_or_else(|e| unreachable!("{e}"))
                .count(),
            0
        );
    }
}

#[test]
fn excessive_corrupt_inventory_disables_cache_without_mutating_retained_bytes() {
    let root = private_root();
    let mut policy = profile(root.path());
    policy.maximum_entries = 1;
    let mut store = CacheStore::open(policy).unwrap_or_else(|error| unreachable!("{error}"));
    let cold = store
        .restore(&key(), now())
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(cold.outcome, RestoreOutcome::Miss);
    let original = envelope(b"retained verified entry");
    store
        .publish(&original, now())
        .unwrap_or_else(|error| unreachable!("{error}"));
    let path = store.directory.join(
        key()
            .identity()
            .unwrap_or_else(|error| unreachable!("{error}")),
    );
    let before = fs::read(&path).unwrap_or_else(|error| unreachable!("{error}"));
    let mut corrupt = Vec::new();
    for index in 0_u64..32 {
        let path = store
            .directory
            .join(blake3::hash(&index.to_be_bytes()).to_hex().as_str());
        fs::write(&path, b"corrupt").unwrap_or_else(|error| unreachable!("{error}"));
        corrupt.push(path);
    }
    let degraded = store
        .restore(&key(), now())
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(degraded.outcome, RestoreOutcome::DisabledInventoryBound);
    assert!(degraded.entry.is_none());
    assert_eq!(
        fs::read(path).unwrap_or_else(|error| unreachable!("{error}")),
        before
    );
    for path in corrupt {
        assert_eq!(
            fs::read(path).unwrap_or_else(|error| unreachable!("{error}")),
            b"corrupt"
        );
    }
}

#[test]
fn publication_authority_excludes_only_degraded_restore_outcomes() {
    for (outcome, allowed) in [
        ("miss", true),
        ("hit", true),
        ("corrupt_miss", false),
        ("disabled_inventory_bound", false),
    ] {
        let observation = CacheObservation {
            key: "a".repeat(64),
            outcome: outcome.into(),
            transferred_bytes: 0,
            duration_ms: 1,
            entry_blake3: None,
            source_tier: "local".into(),
        };
        assert_eq!(observation.permits_publication(), allowed);
    }
}
