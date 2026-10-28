use base64::Engine as _;
use git2::{IndexEntry, IndexTime, Oid, Repository};
use kiln_core::source::GitSnapshotLimits;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

pub const MAX_SNAPSHOT_BYTES: usize = 524_288;
pub const MAX_SNAPSHOT_FILES: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SnapshotFile {
    pub path: String,
    pub executable: bool,
    pub body_base64: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SnapshotManifest {
    pub schema: String,
    pub project_id: String,
    pub reference: String,
    pub base_revision: String,
    pub files: Vec<SnapshotFile>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotProvenance {
    pub manifest_id: String,
    pub base_revision: String,
    pub synthetic_revision: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotRecord {
    pub manifest: SnapshotManifest,
    pub provenance: SnapshotProvenance,
}

pub fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && !path.contains(['\\', '\0'])
        && !path.bytes().any(|b| b.is_ascii_control())
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != ".." && !p.eq_ignore_ascii_case(".git"))
}
pub fn hex(value: &str, bytes: usize) -> bool {
    value.len() == bytes * 2
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl SnapshotManifest {
    pub fn admit(&self) -> Result<(), String> {
        if self.schema != "kiln.dirty-snapshot.v1"
            || !crate::profile::identifier(&self.project_id)
            || !hex(&self.base_revision, 20)
            || !self.reference.starts_with("refs/")
            || self.reference.len() > 1024
            || self.files.is_empty()
            || self.files.len() > MAX_SNAPSHOT_FILES
        {
            return Err("snapshot_manifest_invalid".into());
        }
        let mut prior: Option<&str> = None;
        let mut total = 0usize;
        for file in &self.files {
            if !safe_path(&file.path) || prior.is_some_and(|p| p >= file.path.as_str()) {
                return Err("snapshot_path_invalid".into());
            }
            prior = Some(&file.path);
            if let Some(body) = &file.body_base64 {
                if body.len() > MAX_SNAPSHOT_BYTES.saturating_mul(4).div_ceil(3) {
                    return Err("snapshot_byte_bound".into());
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(body)
                    .map_err(|_| "snapshot_encoding_invalid")?;
                total = total
                    .checked_add(bytes.len())
                    .ok_or("snapshot_byte_bound")?;
                if total > MAX_SNAPSHOT_BYTES {
                    return Err("snapshot_byte_bound".into());
                }
            } else if file.executable {
                return Err("snapshot_delete_invalid".into());
            }
        }
        Ok(())
    }
    pub fn identity(&self) -> Result<String, String> {
        self.admit()?;
        let bytes = serde_json::to_vec(self).map_err(|_| "snapshot_encoding_invalid")?;
        Ok(blake3::hash(&bytes).to_hex().to_string())
    }
}

/// Only explicitly selected files are read. Neither index nor refs are written.
pub fn capture(
    repository_path: &Path,
    project_id: String,
    reference: String,
    base_revision: String,
    includes: &[String],
    deletes: &[String],
) -> Result<SnapshotManifest, String> {
    if includes.len().saturating_add(deletes.len()) == 0
        || includes.len().saturating_add(deletes.len()) > MAX_SNAPSHOT_FILES
    {
        return Err("snapshot_entry_bound".into());
    }
    let repository =
        Repository::open(repository_path).map_err(|_| "snapshot_repository_unavailable")?;
    let root = repository.workdir().ok_or("snapshot_worktree_required")?;
    if repository
        .head()
        .and_then(|h| h.peel_to_commit())
        .map_err(|_| "snapshot_head_unavailable")?
        .id()
        .to_string()
        != base_revision
    {
        return Err("snapshot_base_is_not_head".into());
    }
    let base = repository
        .find_commit(Oid::from_str(&base_revision).map_err(|_| "snapshot_base_invalid")?)
        .map_err(|_| "snapshot_base_unavailable")?;
    let tree = base.tree().map_err(|_| "snapshot_base_unavailable")?;
    let mut paths = BTreeSet::new();
    let mut files = Vec::new();
    let mut observed = Vec::new();
    let mut total = 0usize;
    for path in includes {
        if !safe_path(path) || !paths.insert(path.clone()) {
            return Err("snapshot_path_invalid".into());
        }
        // Ignored input is never discovered or implicitly included; even explicit secrets
        // marked ignored require removing that ignore policy before submission.
        if repository
            .is_path_ignored(Path::new(path))
            .map_err(|_| "snapshot_ignore_unavailable")?
        {
            return Err("snapshot_ignored_path_denied".into());
        }
        let full = root.join(path);
        let before = std::fs::symlink_metadata(&full).map_err(|_| "snapshot_file_unavailable")?;
        let bytes = crate::worker::files::artifact(
            root,
            Path::new(path),
            MAX_SNAPSHOT_BYTES.saturating_sub(total),
        )?;
        let again = crate::worker::files::artifact(
            root,
            Path::new(path),
            MAX_SNAPSHOT_BYTES.saturating_sub(total),
        )?;
        let after = std::fs::symlink_metadata(&full).map_err(|_| "snapshot_file_changed")?;
        if !before.is_file()
            || before.nlink() != 1
            || before.ino() != after.ino()
            || before.dev() != after.dev()
            || before.mode() != after.mode()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || bytes != again
        {
            return Err("snapshot_file_changed".into());
        }
        total = total
            .checked_add(bytes.len())
            .ok_or("snapshot_byte_bound")?;
        observed.push((path.clone(), before));
        files.push(SnapshotFile {
            path: path.clone(),
            executable: after.mode() & 0o111 != 0,
            body_base64: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
        });
    }
    for path in deletes {
        if !safe_path(path)
            || !paths.insert(path.clone())
            || tree.get_path(Path::new(path)).is_err()
            || !absent_without_links(root, path)?
        {
            return Err("snapshot_delete_invalid".into());
        }
        files.push(SnapshotFile {
            path: path.clone(),
            executable: false,
            body_base64: None,
        });
    }
    // Revalidate all selections together, not just during each individual read.
    for (path, before) in &observed {
        let file = files
            .iter()
            .find(|f| &f.path == path)
            .ok_or("snapshot_file_changed")?;
        let expected = base64::engine::general_purpose::STANDARD
            .decode(file.body_base64.as_ref().ok_or("snapshot_file_changed")?)
            .map_err(|_| "snapshot_encoding_invalid")?;
        let current = crate::worker::files::artifact(root, Path::new(path), MAX_SNAPSHOT_BYTES)?;
        let after =
            std::fs::symlink_metadata(root.join(path)).map_err(|_| "snapshot_file_changed")?;
        if current != expected
            || before.ino() != after.ino()
            || before.dev() != after.dev()
            || before.mode() != after.mode()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
        {
            return Err("snapshot_file_changed".into());
        }
    }
    for path in deletes {
        if !absent_without_links(root, path)? {
            return Err("snapshot_delete_changed".into());
        }
    }
    if repository
        .head()
        .and_then(|h| h.peel_to_commit())
        .map_err(|_| "snapshot_head_unavailable")?
        .id()
        .to_string()
        != base_revision
    {
        return Err("snapshot_base_changed".into());
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let manifest = SnapshotManifest {
        schema: "kiln.dirty-snapshot.v1".into(),
        project_id,
        reference,
        base_revision,
        files,
    };
    manifest.admit()?;
    Ok(manifest)
}

fn absent_without_links(root: &Path, relative: &str) -> Result<bool, String> {
    let mut path = root.to_owned();
    for component in relative.split('/') {
        path.push(component);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
            Err(_) => return Err("snapshot_delete_unavailable".into()),
        }
    }
    Ok(false)
}

#[derive(Debug)]
pub struct SyntheticRepository {
    pub root: tempfile::TempDir,
    pub repository_path: PathBuf,
    pub revision: String,
}
/// Copy only bounded base-tree objects, not history, refs, config, or the worktree.
/// The original repository is opened read-only; all Git writes target private storage.
pub fn reconstruct(
    base_path: &Path,
    manifest: &SnapshotManifest,
    limits: &GitSnapshotLimits,
) -> Result<SyntheticRepository, String> {
    manifest.admit()?;
    let source = Repository::open(base_path).map_err(|_| "snapshot_repository_unavailable")?;
    let base_id = Oid::from_str(&manifest.base_revision).map_err(|_| "snapshot_base_invalid")?;
    let base = source
        .find_commit(base_id)
        .map_err(|_| "snapshot_base_unavailable")?;
    let tree = base.tree().map_err(|_| "snapshot_tree_unavailable")?;
    let root = tempfile::Builder::new()
        .prefix("kiln-snapshot-")
        .tempdir()
        .map_err(|_| "snapshot_storage_unavailable")?;
    let repository_path = root.path().join("objects.git");
    let target =
        Repository::init_bare(&repository_path).map_err(|_| "snapshot_storage_unavailable")?;
    let mut budget = TreeBudget {
        limits,
        entries: 0,
        bytes: 0,
    };
    copy_tree(&source, &target, &tree, 0, 0, &mut budget)?;
    let source_odb = source.odb().map_err(|_| "snapshot_object_unavailable")?;
    let object = source_odb
        .read(base_id)
        .map_err(|_| "snapshot_object_unavailable")?;
    if object.data().len() > MAX_SNAPSHOT_BYTES {
        return Err("snapshot_commit_bound".into());
    }
    target
        .odb()
        .and_then(|odb| odb.write(object.kind(), object.data()))
        .map_err(|_| "snapshot_object_unavailable")?;
    let mut index = git2::Index::new().map_err(|_| "snapshot_index_failed")?;
    index
        .read_tree(
            &target
                .find_tree(tree.id())
                .map_err(|_| "snapshot_tree_unavailable")?,
        )
        .map_err(|_| "snapshot_index_failed")?;
    for file in &manifest.files {
        if let Some(body) = &file.body_base64 {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(body)
                .map_err(|_| "snapshot_encoding_invalid")?;
            if bytes.len() > limits.max_file_bytes {
                return Err("snapshot_file_bound".into());
            }
            let id = target.blob(&bytes).map_err(|_| "snapshot_blob_failed")?;
            index
                .add(&IndexEntry {
                    ctime: IndexTime::new(0, 0),
                    mtime: IndexTime::new(0, 0),
                    dev: 0,
                    ino: 0,
                    mode: if file.executable { 0o100755 } else { 0o100644 },
                    uid: 0,
                    gid: 0,
                    file_size: bytes.len() as u32,
                    id,
                    flags: 0,
                    flags_extended: 0,
                    path: file.path.as_bytes().to_vec(),
                })
                .map_err(|_| "snapshot_path_conflict")?;
        } else {
            index
                .remove_path(Path::new(&file.path))
                .map_err(|_| "snapshot_delete_invalid")?;
        }
    }
    let tree_id = index
        .write_tree_to(&target)
        .map_err(|_| "snapshot_tree_failed")?;
    if tree_id == tree.id() {
        return Err("snapshot_has_no_changes".into());
    }
    let result_tree = target
        .find_tree(tree_id)
        .map_err(|_| "snapshot_tree_failed")?;
    budget.entries = 0;
    budget.bytes = 0;
    copy_tree(&target, &target, &result_tree, 0, 0, &mut budget)?;
    let signature = git2::Signature::new(
        "Kiln dirty snapshot",
        "snapshot@kiln.invalid",
        &git2::Time::new(0, 0),
    )
    .map_err(|_| "snapshot_signature_failed")?;
    let parent = target
        .find_commit(base_id)
        .map_err(|_| "snapshot_base_unavailable")?;
    let message = format!(
        "Kiln explicit dirty snapshot v1\n\nmanifest-blake3: {}\nbase: {}\n",
        manifest.identity()?,
        manifest.base_revision
    );
    let revision = target
        .commit(
            None,
            &signature,
            &signature,
            &message,
            &result_tree,
            &[&parent],
        )
        .map_err(|_| "snapshot_commit_failed")?
        .to_string();
    Ok(SyntheticRepository {
        root,
        repository_path,
        revision,
    })
}
struct TreeBudget<'a> {
    limits: &'a GitSnapshotLimits,
    entries: usize,
    bytes: usize,
}

fn copy_tree(
    source: &Repository,
    target: &Repository,
    tree: &git2::Tree<'_>,
    depth: usize,
    prefix: usize,
    budget: &mut TreeBudget<'_>,
) -> Result<(), String> {
    if depth > 128 {
        return Err("snapshot_depth_bound".into());
    }
    for entry in tree {
        let name = entry.name().ok_or("snapshot_path_invalid")?;
        if !safe_path(name) || prefix + name.len() > budget.limits.max_path_bytes {
            return Err("snapshot_path_invalid".into());
        }
        budget.entries = budget
            .entries
            .checked_add(1)
            .ok_or("snapshot_entry_bound")?;
        if budget.entries > budget.limits.max_entries {
            return Err("snapshot_entry_bound".into());
        }
        if entry.filemode() == 0o040000 {
            copy_tree(
                source,
                target,
                &source
                    .find_tree(entry.id())
                    .map_err(|_| "snapshot_tree_unavailable")?,
                depth + 1,
                prefix + name.len() + 1,
                budget,
            )?;
        } else if matches!(entry.filemode(), 0o100644 | 0o100755) {
            let blob = source
                .find_blob(entry.id())
                .map_err(|_| "snapshot_blob_unavailable")?;
            budget.bytes = budget
                .bytes
                .checked_add(blob.size())
                .ok_or("snapshot_total_bound")?;
            if blob.size() > budget.limits.max_file_bytes
                || budget.bytes > budget.limits.max_total_bytes
            {
                return Err("snapshot_total_bound".into());
            }
            if source.path() != target.path() {
                target
                    .blob(blob.content())
                    .map_err(|_| "snapshot_blob_failed")?;
            }
        } else {
            return Err("snapshot_links_denied".into());
        }
    }
    if source.path() != target.path() {
        let odb = source.odb().map_err(|_| "snapshot_object_unavailable")?;
        let object = odb
            .read(tree.id())
            .map_err(|_| "snapshot_object_unavailable")?;
        target
            .odb()
            .and_then(|odb| odb.write(object.kind(), object.data()))
            .map_err(|_| "snapshot_tree_failed")?;
    }
    Ok(())
}
