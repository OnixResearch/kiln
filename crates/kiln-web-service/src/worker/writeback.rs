//! Observe a projected attempt overlay and seal a captured candidate (#79).
//!
//! `kiln_core::projection::writeback::capture` decides which source
//! operations an overlay holds. This shell observes the overlay by hashing
//! each regular file as a Git blob without following links, and seals a
//! captured candidate into a commit over its base. Sealing re-reads every
//! captured file and refuses bytes that changed since capture, writes only
//! those blobs, applies the operations to the base tree, and checks that the
//! result differs from the base at exactly the operation paths and lists
//! exactly the pure reconstruction. The commit moves no reference; publishing
//! it is a separate, authorized effect.
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

use git2::build::TreeUpdateBuilder;
use git2::{
    FileMode, ObjectType, Oid, Repository, Signature, Time, Tree, TreeWalkMode, TreeWalkResult,
};
use kiln_core::projection::authority::{
    AttemptFence, OriginScope, PathScope, SnapshotRule, SourceAuthorityRequest, SourceViewOperation,
};
use kiln_core::projection::disclosure::ViewCeiling;
use kiln_core::projection::writeback::{
    MAX_OBSERVED_FILES, ObservedFile, SealedCandidate, SourceOperation,
};
use kiln_core::projection::{EntryKind, ProjectionFacts, SourceEntry};
use kiln_core::types::{ContentId, Diagnostic, ExternalRef};
use serde::Serialize;

use super::source_auth::SourceAuthorization;

/// Any execute bit makes an observed file executable.
const EXECUTE_BITS: u32 = 0o111;
const SEAL_NAME: &str = "Kiln projected writeback";
const SEAL_EMAIL: &str = "writeback@kiln.invalid";
/// Git tree entry modes of the entry kinds a candidate can hold.
const MODE_FILE: i32 = 0o100_644;
const MODE_EXECUTABLE: i32 = 0o100_755;
const MODE_SYMLINK: i32 = 0o120_000;

// r[impl kiln.source.projection.writeback.scenario.round_trip]
/// Every file of `overlay` as a destination, kind and Git blob identity, in
/// destination order, after admitting and revalidating `RetrieveResult`
/// authority via `auth`. A symbolic link is reported as one (its target is
/// the blob) so capture can refuse it; any other special file is an error.
pub fn observe(
    overlay: &Path,
    request: &SourceAuthorityRequest,
    auth: &mut SourceAuthorization<'_>,
) -> Result<Vec<ObservedFile>, String> {
    if request.operation != SourceViewOperation::RetrieveResult {
        return Err("overlay observation requires RetrieveResult source authority".into());
    }
    if request.scope
        != (OriginScope::Components {
            path: PathScope::root(),
        })
    {
        return Err("overlay observation requires root RetrieveResult source authority".into());
    }
    let scope = auth.authorize(request)?;
    if !scope.permits_scope(SourceViewOperation::RetrieveResult, &PathScope::root()) {
        return Err("overlay observation requires root RetrieveResult source authority".into());
    }
    let mut found = Vec::new();
    walk(overlay, overlay, &mut found)?;
    found.sort_by(|left, right| left.destination.cmp(&right.destination));
    Ok(found)
}

fn walk(root: &Path, directory: &Path, found: &mut Vec<ObservedFile>) -> Result<(), String> {
    for child in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = child.map_err(|error| error.to_string())?.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            walk(root, &path, found)?;
            continue;
        }
        let destination = path
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .to_str()
            .ok_or("an overlay path is not UTF-8")?
            .to_owned();
        if found.len() >= MAX_OBSERVED_FILES {
            return Err("the overlay holds more files than the observation bound".into());
        }
        let (kind, bytes) = if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).map_err(|error| error.to_string())?;
            (EntryKind::Symlink, target.as_os_str().as_bytes().to_vec())
        } else if metadata.is_file() {
            let kind = if metadata.mode() & EXECUTE_BITS == 0 {
                EntryKind::File
            } else {
                EntryKind::Executable
            };
            (kind, read_file(&path)?)
        } else {
            return Err(format!("{destination}: not a regular file or link"));
        };
        found.push(ObservedFile {
            destination,
            kind,
            object: blob_ref(&bytes)?,
        });
    }
    Ok(())
}

// r[impl kiln.source.projection.writeback.scenario.sealed]
/// Seal `candidate` into a commit whose parent is its base revision in
/// `repository`, reading captured bytes from `overlay`, after admitting and
/// revalidating per-operation `SourceAuthority` (`ModifySource`,
/// `CreateSource`, `ReadContent`) bound to the candidate's attempt fence and
/// exact base revision. `base` must be the candidate's base facts. No
/// reference moves, and no Git blob or tree is written on authority denial.
pub fn seal(
    repository: &Repository,
    overlay: &Path,
    base: &ProjectionFacts,
    candidate: &SealedCandidate,
    auth: &mut SourceAuthorization<'_>,
) -> Result<Oid, String> {
    let expected = candidate.reconstruct(base).map_err(diagnostics)?;
    authorize_candidate_operations(candidate, auth)?;
    let base_commit = repository
        .find_commit(git_oid(candidate.base_revision(), "commit")?)
        .map_err(|_| "the candidate's base commit is unavailable")?;
    let base_tree = base_commit.tree().map_err(|error| error.to_string())?;
    // Read and check every captured file before anything is written.
    let mut contents = Vec::new();
    for operation in candidate.operations() {
        let (destination, object, kind) = match operation {
            SourceOperation::Edit {
                destination,
                object,
                kind,
                ..
            }
            | SourceOperation::Create {
                destination,
                object,
                kind,
                ..
            } => (destination, object, kind),
            // A rename or fork carries its origin's exact bytes to its new path.
            SourceOperation::Rename {
                destination,
                expected,
                kind,
                ..
            }
            | SourceOperation::Fork {
                destination,
                expected,
                kind,
                ..
            } => (destination, expected, kind),
            SourceOperation::Delete { .. } => continue,
        };
        let bytes = read_file(&overlay.join(relative(destination)?))?;
        if blob_ref(&bytes)? != *object {
            return Err(format!("{destination}: the overlay changed after capture"));
        }
        contents.push((operation.path(), bytes, file_mode(*kind)?));
    }
    let mut update = TreeUpdateBuilder::new();
    for (path, bytes, mode) in contents {
        let oid = repository.blob(&bytes).map_err(|error| error.to_string())?;
        update.upsert(path, oid, mode);
    }
    for operation in candidate.operations() {
        match operation {
            SourceOperation::Delete { path, .. } => {
                update.remove(path.as_str());
            }
            SourceOperation::Rename { from, .. } => {
                update.remove(from.as_str());
            }
            _ => {}
        }
    }
    let tree_oid = update
        .create_updated(repository, &base_tree)
        .map_err(|error| error.to_string())?;
    let tree = repository
        .find_tree(tree_oid)
        .map_err(|error| error.to_string())?;
    let changed = changed_paths(repository, &base_tree, &tree)?;
    let operated = candidate.changed_paths();
    if changed != operated {
        return Err(
            "the candidate tree differs from its base outside the captured operations".into(),
        );
    }
    if listing(&tree)? != expected {
        return Err("the candidate tree does not list the reconstructed candidate".into());
    }
    let signature = Signature::new(SEAL_NAME, SEAL_EMAIL, &Time::new(0, 0))
        .map_err(|error| error.to_string())?;
    repository
        .commit(
            None,
            &signature,
            &signature,
            &format!("Projected writeback {}\n", candidate.identity().as_str()),
            &tree,
            &[&base_commit],
        )
        .map_err(|error| error.to_string())
}

/// Every blob path that differs between two trees, without rename detection.
fn changed_paths(
    repository: &Repository,
    base: &Tree<'_>,
    tree: &Tree<'_>,
) -> Result<BTreeSet<String>, String> {
    let diff = repository
        .diff_tree_to_tree(Some(base), Some(tree), None)
        .map_err(|error| error.to_string())?;
    diff.deltas()
        .map(|delta| {
            delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .and_then(Path::to_str)
                .map(str::to_owned)
                .ok_or_else(|| "a changed path is not UTF-8".to_string())
        })
        .collect()
}

/// Every blob of `tree` as a source entry, in path order.
fn listing(tree: &Tree<'_>) -> Result<Vec<SourceEntry>, String> {
    let mut entries = Vec::new();
    let mut fault = None;
    tree.walk(TreeWalkMode::PreOrder, |root, entry| {
        if entry.kind() != Some(ObjectType::Blob) {
            return TreeWalkResult::Ok;
        }
        let kind = match entry.filemode() {
            MODE_FILE => EntryKind::File,
            MODE_EXECUTABLE => EntryKind::Executable,
            MODE_SYMLINK => EntryKind::Symlink,
            _ => {
                fault = Some("the candidate tree holds an unsupported entry mode".to_string());
                return TreeWalkResult::Abort;
            }
        };
        let Ok(name) = entry.name() else {
            fault = Some("a candidate tree name is not UTF-8".to_string());
            return TreeWalkResult::Abort;
        };
        match ExternalRef::new("git", "blob", &entry.id().to_string()) {
            Ok(object) => entries.push(SourceEntry {
                path: format!("{root}{name}"),
                object,
                kind,
            }),
            Err(error) => {
                fault = Some(error.message);
                return TreeWalkResult::Abort;
            }
        }
        TreeWalkResult::Ok
    })
    .map_err(|error| fault.clone().unwrap_or_else(|| error.to_string()))?;
    if let Some(fault) = fault {
        return Err(fault);
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn file_mode(kind: EntryKind) -> Result<FileMode, String> {
    match kind {
        EntryKind::File => Ok(FileMode::Blob),
        EntryKind::Executable => Ok(FileMode::BlobExecutable),
        EntryKind::Symlink => Err("symbolic links cannot be written back".into()),
    }
}

fn blob_ref(bytes: &[u8]) -> Result<ExternalRef, String> {
    let oid = Oid::hash_object(ObjectType::Blob, bytes).map_err(|error| error.to_string())?;
    ExternalRef::new("git", "blob", &oid.to_string()).map_err(|error| error.message)
}

fn git_oid(reference: &ExternalRef, kind: &str) -> Result<Oid, String> {
    if reference.provider_namespace.as_str() != "git" || reference.native_kind.as_str() != kind {
        return Err(format!("expected a Git {kind}"));
    }
    Oid::from_str(reference.native_id.as_str()).map_err(|_| format!("malformed Git {kind} id"))
}

/// An overlay destination as a relative path of normal components.
fn relative(destination: &str) -> Result<PathBuf, String> {
    let path = Path::new(destination);
    if destination.is_empty()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe overlay destination {destination:?}"));
    }
    Ok(path.to_path_buf())
}

/// Read a regular file without following a final symbolic link.
fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

/// Authorize every [`SourceOperation`] of `candidate` against `auth` before
/// any Git blob or tree is written.
fn authorize_candidate_operations(
    candidate: &SealedCandidate,
    auth: &mut SourceAuthorization<'_>,
) -> Result<(), String> {
    let (manifest, _definition) = candidate.view();
    let request_digest = *blake3::hash(candidate.identity().as_str().as_bytes()).as_bytes();
    let attempt = Some(AttemptFence {
        attempt: candidate.attempt().clone(),
        fence: candidate.fence(),
    });
    let snapshot = SnapshotRule::Exact {
        revision: candidate.base_revision().clone(),
    };
    let mut authorize = |operation: SourceViewOperation, path: &str| -> Result<(), String> {
        let scope_path = PathScope::from_path(path, false)
            .ok_or_else(|| format!("{path}: unsafe source path in sealed candidate"))?;
        let request = SourceAuthorityRequest {
            operation,
            repository: candidate.repository().clone(),
            snapshot: snapshot.clone(),
            view_binding: manifest.clone(),
            scope: OriginScope::Components { path: scope_path },
            request_digest,
            attempt: attempt.clone(),
        };
        auth.authorize(&request).map(|_| ())
    };
    for operation in candidate.operations() {
        match operation {
            SourceOperation::Edit { path, .. } | SourceOperation::Delete { path, .. } => {
                authorize(SourceViewOperation::ModifySource, path)?;
            }
            SourceOperation::Create { path, .. } => {
                authorize(SourceViewOperation::CreateSource, path)?;
            }
            SourceOperation::Rename { from, to, .. } => {
                authorize(SourceViewOperation::ModifySource, from)?;
                authorize(SourceViewOperation::CreateSource, to)?;
            }
            SourceOperation::Fork { from, to, .. } => {
                authorize(SourceViewOperation::ReadContent, from)?;
                authorize(SourceViewOperation::CreateSource, to)?;
            }
        }
    }
    Ok(())
}

/// Authorized result projection for a sealed writeback candidate (`RetrieveResult`).
/// Operations touching any path outside the holder's `EffectiveSourceScope` or
/// `ViewCeiling` are omitted without leaking hidden path names or blob digests.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RestrictedWritebackResult {
    pub candidate_identity: ContentId,
    pub operations: Vec<SourceOperation>,
    pub omitted_operations: usize,
}

/// Project a sealed candidate's result for an observer after admitting and
/// revalidating [`SourceViewOperation::RetrieveResult`] via `auth`.
///
/// # Errors
/// Returns a redacted message when `RetrieveResult` authority is denied.
pub fn retrieve_result(
    candidate: &SealedCandidate,
    ceiling: &ViewCeiling,
    scope_path: &PathScope,
    auth: &mut SourceAuthorization<'_>,
) -> Result<RestrictedWritebackResult, String> {
    let (manifest, definition) = candidate.view();
    if ceiling.definition() != definition {
        return Err("view ceiling does not match candidate definition".into());
    }
    let request = SourceAuthorityRequest {
        operation: SourceViewOperation::RetrieveResult,
        repository: candidate.repository().clone(),
        snapshot: SnapshotRule::Exact {
            revision: candidate.base_revision().clone(),
        },
        view_binding: manifest.clone(),
        scope: OriginScope::Components {
            path: scope_path.clone(),
        },
        request_digest: *blake3::hash(candidate.identity().as_str().as_bytes()).as_bytes(),
        attempt: Some(AttemptFence {
            attempt: candidate.attempt().clone(),
            fence: candidate.fence(),
        }),
    };
    let scope = auth.authorize(&request)?;
    let mut operations = Vec::new();
    let mut omitted_operations = 0usize;
    for operation in candidate.operations() {
        let allowed = operation.changed_paths().into_iter().all(|path| {
            ceiling.allows(SourceViewOperation::RetrieveResult, path)
                && scope.permits(SourceViewOperation::RetrieveResult, path)
        }) && match operation {
            SourceOperation::Fork { from, .. } => {
                ceiling.allows(SourceViewOperation::RetrieveResult, from)
                    && scope.permits(SourceViewOperation::RetrieveResult, from)
            }
            _ => true,
        };
        if allowed {
            operations.push(operation.clone());
        } else {
            omitted_operations = omitted_operations.saturating_add(1);
        }
    }
    Ok(RestrictedWritebackResult {
        candidate_identity: candidate.identity().clone(),
        operations,
        omitted_operations,
    })
}

fn diagnostics(errors: Vec<Diagnostic>) -> String {
    errors
        .into_iter()
        .map(|error| format!("{}: {}", error.code, error.message))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
#[path = "writeback_tests.rs"]
mod tests;
