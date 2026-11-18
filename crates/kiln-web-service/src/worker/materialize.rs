//! Eager local materialization of planned projection inputs and attempt
//! overlays (#78).
//!
//! `kiln_core::projection::materialization::plan` decides what to write. This
//! shell verifies every Git blob against its object identity, writes only the
//! planned entries into a private preparing directory, publishes the immutable
//! input by a no-replace rename and only then records its binding. An attempt
//! overlay is a private writable copy of a published input, prepared for one
//! dispatch `WorkspacePin` and bound to it. A directory whose binding is
//! missing or different is never reused or overwritten, and a reused input is
//! verified again. Materializing grants no read, write or execution
//! authority; callers admit those separately.
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use git2::{ObjectType, Oid, Repository};
use kiln_core::projection::materialization::{MaterializationPlan, PlannedEntry, WorkspacePin};
use kiln_core::types::ContentId;
use nix::fcntl::{AT_FDCWD, RenameFlags, renameat2};
use serde::{Deserialize, Serialize};

use super::files::owned_directory;

const BINDING_SCHEMA: &str = "kiln.materialization.binding.v2";
const INPUTS: &str = "inputs";
const OVERLAYS: &str = "overlays";
const PREPARING_PREFIX: &str = ".preparing-";
/// Largest binding record read back.
const MAX_BINDING_BYTES: u64 = 4_096;
/// Modes of a published, immutable input.
const SEALED_FILE: u32 = 0o444;
const SEALED_EXECUTABLE: u32 = 0o555;
const SEALED_DIRECTORY: u32 = 0o555;
/// Modes of an attempt's writable overlay.
const WRITABLE_FILE: u32 = 0o644;
const WRITABLE_EXECUTABLE: u32 = 0o755;
const WRITABLE_DIRECTORY: u32 = 0o755;
const PRIVATE_DIRECTORY: u32 = 0o700;

/// What a published input or overlay directory was prepared for. An overlay
/// records the whole dispatch pin of its attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    schema: String,
    manifest: ContentId,
    input: ContentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    overlay: Option<WorkspacePin>,
}

impl Binding {
    fn input(plan: &MaterializationPlan) -> Self {
        Self {
            schema: BINDING_SCHEMA.into(),
            manifest: plan.manifest().clone(),
            input: plan.input_identity().clone(),
            overlay: None,
        }
    }
}

// r[impl kiln.source.projection.materialization.scenario.repeat]
// r[impl kiln.source.projection.materialization.scenario.refused]
/// Publish, or reuse after re-verification, the immutable input of `plan`
/// under the private `store`, reading blobs from `repository`.
pub fn prepare_input(
    store: &Path,
    plan: &MaterializationPlan,
    repository: &Repository,
) -> Result<PathBuf, String> {
    owned_directory(store)?;
    let inputs = private_child(store, INPUTS)?;
    let name = plan.input_identity().as_str();
    let published = inputs.join(name);
    let binding_path = inputs.join(format!("{name}.json"));
    let expected = Binding::input(plan);
    if bound(&published, &binding_path, &expected, "input")? {
        verify_input(&published, plan)?;
        return Ok(published);
    }
    let preparing = preparing_directory(&inputs, name)?;
    let written = write_entries(&preparing, plan.entries(), |entry| blob(repository, entry))
        .and_then(|()| seal(&preparing, plan.entries()));
    if let Err(error) = written {
        discard(&preparing);
        return Err(error);
    }
    publish(&preparing, &published, &binding_path, &expected)?;
    Ok(published)
}

// r[impl kiln.source.projection.materialization.scenario.isolated]
// r[impl kiln.source.projection.materialization.scenario.pinned]
/// Prepare, or resume, the private writable overlay that `pin` names over the
/// published input of `plan`. The pin must be exactly the one `plan` derives
/// for its attempt and fence, and an existing overlay is resumed only when its
/// binding records that same pin.
pub fn prepare_overlay(
    store: &Path,
    plan: &MaterializationPlan,
    pin: &WorkspacePin,
) -> Result<PathBuf, String> {
    owned_directory(store)?;
    let derived = plan
        .pin(&pin.attempt, pin.fence)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    if &derived != pin {
        return Err(
            "the dispatch pin differs from the materialization plan; prepare a new overlay generation"
                .into(),
        );
    }
    let input = store.join(INPUTS).join(plan.input_identity().as_str());
    let input_binding = store
        .join(INPUTS)
        .join(format!("{}.json", plan.input_identity().as_str()));
    if !bound(&input, &input_binding, &Binding::input(plan), "input")? {
        return Err("the overlay's input has not been published".into());
    }
    let overlays = private_child(store, OVERLAYS)?;
    let name = pin.overlay.as_str();
    let published = overlays.join(name);
    let binding_path = overlays.join(format!("{name}.json"));
    let mut expected = Binding::input(plan);
    expected.overlay = Some(pin.clone());
    // The attempt resumes its own overlay, edits included.
    if bound(&published, &binding_path, &expected, "overlay")? {
        return Ok(published);
    }
    let preparing = preparing_directory(&overlays, name)?;
    let written = write_entries(&preparing, plan.entries(), |entry| {
        read_file(&input.join(relative(&entry.destination)?))
    });
    if let Err(error) = written {
        discard(&preparing);
        return Err(error);
    }
    publish(&preparing, &published, &binding_path, &expected)?;
    Ok(published)
}

/// Whether `directory` is published with exactly `expected`. A directory or
/// binding without its counterpart, or a different binding, is refused.
fn bound(
    directory: &Path,
    binding_path: &Path,
    expected: &Binding,
    what: &str,
) -> Result<bool, String> {
    let present = exists(directory)?;
    let binding = read_binding(binding_path)?;
    match (present, binding) {
        (true, Some(found)) if &found == expected => Ok(true),
        (true, Some(_)) => Err(format!(
            "the {what} directory is bound to another plan; it is neither reused nor overwritten"
        )),
        (true, None) => Err(format!(
            "the {what} directory has no binding; it is neither reused nor overwritten"
        )),
        (false, Some(_)) => Err(format!(
            "the {what} binding has no directory; reconcile before preparing"
        )),
        (false, None) => Ok(false),
    }
}

fn exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(_) => Err("a materialization path is not a directory".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn read_binding(path: &Path) -> Result<Option<Binding>, String> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BINDING_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_BINDING_BYTES {
        return Err("a materialization binding exceeds its bound".into());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "a materialization binding is malformed".into())
}

/// An owned private child directory of the store, created when absent.
fn private_child(store: &Path, name: &str) -> Result<PathBuf, String> {
    let child = store.join(name);
    match fs::DirBuilder::new().mode(PRIVATE_DIRECTORY).create(&child) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.to_string()),
    }
    owned_directory(&child)?;
    Ok(child)
}

/// A fresh preparing directory; an earlier interrupted one is never adopted.
fn preparing_directory(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let path = parent.join(format!(
        "{PREPARING_PREFIX}{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::DirBuilder::new()
        .mode(PRIVATE_DIRECTORY)
        .create(&path)
        .map_err(|error| error.to_string())?;
    Ok(path)
}

/// Move a complete preparing directory into place without replacing anything,
/// then record its binding. A crash in between leaves an unbound directory,
/// which later preparation refuses rather than adopts.
fn publish(
    preparing: &Path,
    published: &Path,
    binding_path: &Path,
    binding: &Binding,
) -> Result<(), String> {
    if let Err(error) = renameat2(
        AT_FDCWD,
        preparing,
        AT_FDCWD,
        published,
        RenameFlags::RENAME_NOREPLACE,
    ) {
        discard(preparing);
        return Err(format!("publishing a materialization failed: {error}"));
    }
    let bytes = serde_json::to_vec(binding).map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(binding_path)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| error.to_string())
}

/// Best-effort removal of a failed preparing directory.
fn discard(path: &Path) {
    let _ = make_writable(path);
    let _ = fs::remove_dir_all(path);
}

fn make_writable(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_DIRECTORY))?;
        for child in fs::read_dir(path)? {
            make_writable(&child?.path())?;
        }
    }
    Ok(())
}

/// A planned destination as a relative path of normal components.
fn relative(destination: &str) -> Result<PathBuf, String> {
    let path = Path::new(destination);
    if destination.is_empty()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe planned destination {destination:?}"));
    }
    Ok(path.to_path_buf())
}

/// Write every planned entry with create-new, in writable modes.
fn write_entries(
    root: &Path,
    entries: &[PlannedEntry],
    mut bytes: impl FnMut(&PlannedEntry) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    for entry in entries {
        let path = root.join(relative(&entry.destination)?);
        if let Some(parent) = path.parent() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(WRITABLE_DIRECTORY)
                .create(parent)
                .map_err(|error| error.to_string())?;
        }
        let content = bytes(entry)?;
        let mode = if entry.executable {
            WRITABLE_EXECUTABLE
        } else {
            WRITABLE_FILE
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|error| format!("{}: {error}", entry.destination))?;
        file.write_all(&content)
            .map_err(|error| format!("{}: {error}", entry.destination))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// The bytes of a planned Git blob, verified against its object identity.
fn blob(repository: &Repository, entry: &PlannedEntry) -> Result<Vec<u8>, String> {
    let oid = blob_oid(entry)?;
    let blob = repository
        .find_blob(oid)
        .map_err(|_| format!("{}: object {oid} is unavailable", entry.destination))?;
    let identity =
        Oid::hash_object(ObjectType::Blob, blob.content()).map_err(|error| error.to_string())?;
    if identity != oid {
        return Err(format!(
            "{}: object {oid} does not match its content",
            entry.destination
        ));
    }
    Ok(blob.content().to_vec())
}

fn blob_oid(entry: &PlannedEntry) -> Result<Oid, String> {
    if entry.object.provider_namespace.as_str() != "git"
        || entry.object.native_kind.as_str() != "blob"
    {
        return Err(format!(
            "{}: eager local materialization reads Git blobs only",
            entry.destination
        ));
    }
    Oid::from_str(entry.object.native_id.as_str())
        .map_err(|_| format!("{}: malformed Git object id", entry.destination))
}

/// Make a complete input immutable: read-only files and directories.
fn seal(root: &Path, entries: &[PlannedEntry]) -> Result<(), String> {
    for entry in entries {
        let mode = if entry.executable {
            SEALED_EXECUTABLE
        } else {
            SEALED_FILE
        };
        fs::set_permissions(
            root.join(relative(&entry.destination)?),
            fs::Permissions::from_mode(mode),
        )
        .map_err(|error| error.to_string())?;
    }
    seal_directories(root)
}

fn seal_directories(directory: &Path) -> Result<(), String> {
    for child in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = child.map_err(|error| error.to_string())?.path();
        if fs::symlink_metadata(&path)
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            seal_directories(&path)?;
        }
    }
    fs::set_permissions(directory, fs::Permissions::from_mode(SEALED_DIRECTORY))
        .map_err(|error| error.to_string())
}

/// A reused input must hold exactly the planned entries with sealed modes and
/// contents matching their objects.
fn verify_input(root: &Path, plan: &MaterializationPlan) -> Result<(), String> {
    let mut found = Vec::new();
    list_files(root, root, &mut found)?;
    found.sort();
    let mut planned: Vec<String> = plan
        .entries()
        .iter()
        .map(|entry| entry.destination.clone())
        .collect();
    planned.sort();
    if found != planned {
        return Err("a reused input holds other entries than its plan".into());
    }
    for entry in plan.entries() {
        let path = root.join(relative(&entry.destination)?);
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        let mode = if entry.executable {
            SEALED_EXECUTABLE
        } else {
            SEALED_FILE
        };
        let content = read_file(&path)?;
        let identity =
            Oid::hash_object(ObjectType::Blob, &content).map_err(|error| error.to_string())?;
        if !metadata.is_file() || metadata.mode() & 0o7777 != mode || identity != blob_oid(entry)? {
            return Err(format!(
                "{}: a reused input differs from its plan",
                entry.destination
            ));
        }
    }
    Ok(())
}

fn list_files(root: &Path, directory: &Path, found: &mut Vec<String>) -> Result<(), String> {
    for child in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = child.map_err(|error| error.to_string())?.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            list_files(root, &path, found)?;
        } else {
            let name = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_str()
                .ok_or("a materialized path is not UTF-8")?;
            found.push(name.to_owned());
        }
    }
    Ok(())
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

#[cfg(test)]
#[path = "materialize_tests.rs"]
mod tests;
