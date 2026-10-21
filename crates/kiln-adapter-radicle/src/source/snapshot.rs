use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};

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
    pub entries: Vec<GitSnapshotEntry>,
    pub directories: Vec<String>,
}

#[derive(Debug, Default)]
struct SnapshotContents {
    entries: Vec<GitSnapshotEntry>,
    total_bytes: u64,
    directories: Vec<String>,
}

fn collect_tree(
    profile: &RadicleSourceProfile,
    repository: &Repository,
    tree: &Tree<'_>,
    prefix: &str,
    contents: &mut SnapshotContents,
    materializable: bool,
) -> PortResult<()> {
    for entry in tree {
        let entry_count = u64::try_from(contents.entries.len()).map_err(|_| {
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
        if materializable
            && (name.is_empty()
                || name == "."
                || name == ".."
                || name.eq_ignore_ascii_case(".git")
                || name.contains(['/', '\\', '\0'])
                || name.bytes().any(|byte| byte.is_ascii_control()))
        {
            return Err(domain(
                "radicle_source_checkout_path",
                "radicle_source.snapshot",
                "Source contains a path unsafe for checkout materialization",
            ));
        }
        if materializable {
            let members = contents
                .entries
                .len()
                .checked_add(contents.directories.len())
                .ok_or_else(platform_bound_failure)?;
            if u64::try_from(members).map_err(|_| platform_bound_failure())?
                >= profile.request.limits.max_entries
            {
                return Err(domain(
                    "radicle_source_entry_bound",
                    "radicle_source.snapshot",
                    "Checkout members exceed the request bound",
                ));
            }
            if !matches!(
                entry.filemode_raw(),
                0o040_000 | MODE_REGULAR | MODE_EXECUTABLE
            ) {
                return Err(domain(
                    "radicle_source_checkout_kind",
                    "radicle_source.snapshot",
                    "Checkout admits only directories, regular files and executable files",
                ));
            }
        }
        match entry.kind() {
            Some(ObjectType::Tree) => {
                let subtree = repository.find_tree(entry.id()).map_err(git_failure)?;
                if materializable {
                    contents.directories.push(path.clone());
                }
                collect_tree(
                    profile,
                    repository,
                    &subtree,
                    &path,
                    contents,
                    materializable,
                )?;
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
                contents.total_bytes = contents.total_bytes.checked_add(size).ok_or_else(|| {
                    domain(
                        "radicle_source_total_bound",
                        "radicle_source.snapshot",
                        "Snapshot total byte count overflowed",
                    )
                })?;
                if contents.total_bytes > profile.request.limits.max_total_bytes {
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
                contents.entries.push(GitSnapshotEntry {
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
    materializable: bool,
) -> PortResult<AcquiredSnapshot> {
    let commit = repository.find_commit(revision).map_err(git_failure)?;
    let tree = commit.tree().map_err(git_failure)?;
    let mut contents = SnapshotContents::default();
    collect_tree(
        profile,
        repository,
        &tree,
        "",
        &mut contents,
        materializable,
    )?;
    let SnapshotContents {
        entries,
        total_bytes,
        directories,
    } = contents;
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
        entries: if materializable { entries } else { Vec::new() },
        directories,
    })
}

pub(super) struct Checkout {
    path: PathBuf,
    retained: bool,
}

impl Checkout {
    pub fn retain(mut self) {
        self.retained = true;
    }
}

impl Drop for Checkout {
    fn drop(&mut self) {
        if !self.retained {
            let _removed = fs::remove_dir_all(&self.path);
        }
    }
}

// r[impl kiln.radicle_ci.source]
pub(super) fn materialize_snapshot(
    snapshot: &AcquiredSnapshot,
    destination: &Path,
) -> PortResult<Checkout> {
    if !destination.is_absolute()
        || destination
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || destination.file_name().is_none()
    {
        return Err(domain(
            "radicle_source_checkout_destination",
            "radicle_source.checkout",
            "Checkout destination must be a new absolute path without traversal",
        ));
    }
    // The caller owns the private parent/state root. These checks reject existing
    // substitution, not concurrent mutation by another process with that authority.
    // Never traverse an existing symlink, including a symlink in an ancestor.
    for ancestor in destination.ancestors().skip(1) {
        let metadata = fs::symlink_metadata(ancestor).map_err(super::error::io_failure)?;
        if !metadata.file_type().is_dir() {
            return Err(domain(
                "radicle_source_checkout_destination",
                "radicle_source.checkout",
                "Checkout ancestors must be existing real directories",
            ));
        }
    }
    // Atomic creation rejects existing directories, files and symlinks alike.
    DirBuilder::new()
        .mode(0o700)
        .create(destination)
        .map_err(super::error::io_failure)?;
    let checkout = Checkout {
        path: destination.to_path_buf(),
        retained: false,
    };
    for directory in &snapshot.directories {
        DirBuilder::new()
            .mode(0o700)
            .create(destination.join(directory))
            .map_err(super::error::io_failure)?;
    }
    for entry in &snapshot.entries {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(destination.join(&entry.path))
            .map_err(super::error::io_failure)?;
        file.write_all(&entry.bytes)
            .map_err(super::error::io_failure)?;
        let mode = match entry.kind {
            GitEntryKind::Regular => 0o644,
            GitEntryKind::Executable => 0o755,
            _ => {
                return Err(domain(
                    "radicle_source_checkout_kind",
                    "radicle_source.checkout",
                    "Only admitted regular and executable source can be materialized",
                ));
            }
        };
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(super::error::io_failure)?;
    }
    Ok(checkout)
}
