use blake3::Hash;
use cob_git2 as git2;
use git2::{ObjectType, Oid, Repository, Tree};
use kiln_core::source::{
    GitEntryKind, GitSnapshotEntry, GitSnapshotLimits, build_git_snapshot_manifest,
};
use kiln_core::types::ExternalRef;
use kiln_provider_api::{PortFailure, PortResult};

use super::error::{domain, git_failure, platform_bound_failure};
use super::profile::RadicleSourceProfile;

const MODE_REGULAR: i32 = 0o100_644;
const MODE_EXECUTABLE: i32 = 0o100_755;
const MODE_SYMLINK: i32 = 0o120_000;
const MODE_GITLINK: i32 = 0o160_000;

#[derive(Debug)]
pub(super) struct AcquiredSnapshot {
    pub manifest: Vec<u8>,
    pub native_tree: Oid,
    pub entry_count: u64,
    pub total_bytes: u64,
    pub canonical_tree_blake3: Hash,
}

fn collect_tree(
    profile: &RadicleSourceProfile,
    repository: &Repository,
    tree: &Tree<'_>,
    prefix: &str,
    entries: &mut Vec<GitSnapshotEntry>,
    total_bytes: &mut u64,
) -> PortResult<()> {
    for entry in tree {
        let entry_count = u64::try_from(entries.len()).map_err(|_| {
            domain(
                "radicle_source_entry_bound",
                "radicle_source.snapshot",
                "Snapshot entry count exceeds the supported range",
            )
        })?;
        if entry_count >= profile.request.limits.max_entries {
            return Err(domain(
                "radicle_source_entry_bound",
                "radicle_source.snapshot",
                "Snapshot entry count exceeds the request bound",
            ));
        }
        let name = std::str::from_utf8(entry.name_bytes()).map_err(|_| {
            domain(
                "radicle_source_path_encoding",
                "radicle_source.snapshot",
                "Snapshot path is not valid UTF-8",
            )
        })?;
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        let path_bytes = u64::try_from(path.len()).map_err(|_| {
            domain(
                "radicle_source_path_bound",
                "radicle_source.snapshot",
                "Snapshot path exceeds the supported range",
            )
        })?;
        if path_bytes > profile.request.limits.max_path_bytes {
            return Err(domain(
                "radicle_source_path_bound",
                "radicle_source.snapshot",
                "Snapshot path exceeds the request bound",
            ));
        }
        match entry.kind() {
            Some(ObjectType::Tree) => {
                let subtree = repository.find_tree(entry.id()).map_err(git_failure)?;
                collect_tree(profile, repository, &subtree, &path, entries, total_bytes)?;
            }
            Some(ObjectType::Blob) => {
                let blob = repository.find_blob(entry.id()).map_err(git_failure)?;
                let size = u64::try_from(blob.size()).map_err(|_| {
                    domain(
                        "radicle_source_file_bound",
                        "radicle_source.snapshot",
                        "Snapshot file size exceeds the supported range",
                    )
                })?;
                if size > profile.request.limits.max_file_bytes {
                    return Err(domain(
                        "radicle_source_file_bound",
                        "radicle_source.snapshot",
                        "Snapshot file exceeds the request bound",
                    ));
                }
                *total_bytes = total_bytes.checked_add(size).ok_or_else(|| {
                    domain(
                        "radicle_source_total_bound",
                        "radicle_source.snapshot",
                        "Snapshot total byte count overflowed",
                    )
                })?;
                if *total_bytes > profile.request.limits.max_total_bytes {
                    return Err(domain(
                        "radicle_source_total_bound",
                        "radicle_source.snapshot",
                        "Snapshot bytes exceed the request bound",
                    ));
                }
                let kind = match entry.filemode() {
                    MODE_REGULAR => GitEntryKind::Regular,
                    MODE_EXECUTABLE => GitEntryKind::Executable,
                    MODE_SYMLINK => GitEntryKind::Symlink,
                    _ => GitEntryKind::Unsupported,
                };
                entries.push(GitSnapshotEntry {
                    path,
                    kind,
                    native_object: ExternalRef::new(
                        "radicle",
                        "git-object",
                        &entry.id().to_string(),
                    )?,
                    bytes: blob.content().to_vec(),
                });
            }
            Some(ObjectType::Commit) if entry.filemode() == MODE_GITLINK => {
                return Err(domain(
                    "radicle_source_gitlink",
                    "radicle_source.snapshot",
                    "Radicle source profile does not admit Git links",
                ));
            }
            _ => {
                return Err(domain(
                    "radicle_source_entry_kind",
                    "radicle_source.snapshot",
                    "Snapshot contains an unsupported Git entry kind",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn acquire_snapshot(
    profile: &RadicleSourceProfile,
    repository: &Repository,
    revision: Oid,
) -> PortResult<AcquiredSnapshot> {
    let commit = repository.find_commit(revision).map_err(git_failure)?;
    let tree = commit.tree().map_err(git_failure)?;
    let mut entries = Vec::new();
    let mut total_bytes = 0;
    collect_tree(
        profile,
        repository,
        &tree,
        "",
        &mut entries,
        &mut total_bytes,
    )?;
    let limits = GitSnapshotLimits {
        max_entries: usize::try_from(profile.request.limits.max_entries)
            .map_err(|_| platform_bound_failure())?,
        max_path_bytes: usize::try_from(profile.request.limits.max_path_bytes)
            .map_err(|_| platform_bound_failure())?,
        max_file_bytes: usize::try_from(profile.request.limits.max_file_bytes)
            .map_err(|_| platform_bound_failure())?,
        max_total_bytes: usize::try_from(profile.request.limits.max_total_bytes)
            .map_err(|_| platform_bound_failure())?,
    };
    let manifest = build_git_snapshot_manifest(&entries, &limits).map_err(PortFailure::Domain)?;
    let entry_count = u64::try_from(entries.len()).map_err(|_| platform_bound_failure())?;
    let canonical_tree_blake3 = blake3::hash(&manifest);
    Ok(AcquiredSnapshot {
        manifest,
        native_tree: tree.id(),
        entry_count,
        total_bytes,
        canonical_tree_blake3,
    })
}
