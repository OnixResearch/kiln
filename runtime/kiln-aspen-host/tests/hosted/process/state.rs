use super::*;

use std::collections::BTreeMap;

const DURABLE_COMPONENTS: [&str; 4] = [
    "operation-ledger",
    "native-journal",
    "callback-values",
    "host-state.json",
];
// Depth-first traversal retains at most this many open directory iterators,
// independent of the number of files. Exceeding the fixture bound fails rather
// than silently omitting any durable state from the snapshot.
const MAXIMUM_SNAPSHOT_DEPTH: usize = 128;
// Finite fixture budget per durable component, including directory unwinds.
// Exhaustion rejects the entire snapshot; no partial backup is returned.
const MAXIMUM_SNAPSHOT_STEPS: usize = 4096;

pub(in super::super) fn state_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut directories = Vec::with_capacity(MAXIMUM_SNAPSHOT_DEPTH);
    for component in DURABLE_COMPONENTS {
        if let Some(directory) = snapshot_entry(root, &root.join(component), &mut files, 0) {
            directories.push(directory);
        }
        for _ in 0..MAXIMUM_SNAPSHOT_STEPS {
            let Some(directory) = directories.last_mut() else {
                break;
            };
            let Some(entry) = directory.next() else {
                assert!(directories.pop().is_some());
                continue;
            };
            let path = entry
                .unwrap_or_else(|_| unreachable!("durable entry"))
                .path();
            if let Some(child) = snapshot_entry(root, &path, &mut files, directories.len()) {
                directories.push(child);
            }
        }
        assert!(
            directories.is_empty(),
            "durable snapshot traversal exceeds fixture step budget"
        );
    }
    let identities = files.iter().map(|(path, bytes)| {
        serde_json::json!({ "path": path, "blake3": blake3::hash(bytes).to_hex().to_string() })
    }).collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::json!({
            "schema": "kiln.aspen-host.qualification-quiescent-state.v1",
            "root": root,
            "marker": file_document(&root.join("host-state.json")),
            "files": identities,
        })
    );
    files
}

fn snapshot_entry(
    root: &Path,
    path: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
    directory_depth: usize,
) -> Option<std::fs::ReadDir> {
    let metadata = std::fs::symlink_metadata(path)
        .unwrap_or_else(|_| unreachable!("durable component metadata"));
    assert!(!metadata.file_type().is_symlink());
    if metadata.is_dir() {
        assert!(
            directory_depth < MAXIMUM_SNAPSHOT_DEPTH,
            "durable snapshot directory depth exceeds fixture bound"
        );
        Some(std::fs::read_dir(path).unwrap_or_else(|_| unreachable!("durable directory")))
    } else {
        assert!(metadata.is_file());
        files.insert(
            path.strip_prefix(root)
                .unwrap_or_else(|_| unreachable!("relative durable path"))
                .to_path_buf(),
            std::fs::read(path).unwrap_or_else(|_| unreachable!("durable bytes")),
        );
        None
    }
}

pub(in super::super) fn restore_state(root: &Path, snapshot: &BTreeMap<PathBuf, Vec<u8>>) {
    for component in DURABLE_COMPONENTS {
        let path = root.join(component);
        if path.is_dir() {
            std::fs::remove_dir_all(path)
                .unwrap_or_else(|_| unreachable!("replace owned durable directory"));
        } else if path.exists() {
            std::fs::remove_file(path)
                .unwrap_or_else(|_| unreachable!("replace owned durable file"));
        }
    }
    for component in &DURABLE_COMPONENTS[..3] {
        std::fs::create_dir(root.join(component))
            .unwrap_or_else(|_| unreachable!("restore durable directory"));
    }
    for (relative, bytes) in snapshot {
        let path = root.join(relative);
        let parent = path
            .parent()
            .unwrap_or_else(|| unreachable!("durable parent"));
        std::fs::create_dir_all(parent).unwrap_or_else(|_| unreachable!("restore durable parents"));
        std::fs::write(path, bytes).unwrap_or_else(|_| unreachable!("restore durable bytes"));
    }
    assert_eq!(&state_snapshot(root), snapshot);
}
