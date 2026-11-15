use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_RETAINED_SOURCE_OBSERVATIONS: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkspaceProfile {
    pub repository: String,
    pub repository_path: PathBuf,
    pub work_root: PathBuf,
    pub max_entries: usize,
    pub max_path_bytes: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
}

pub(super) fn validate(profile: &WorkspaceProfile) -> Result<(), String> {
    if profile.repository.is_empty()
        || !profile.repository_path.is_absolute()
        || !profile.work_root.is_absolute()
        || profile.max_entries == 0
        || profile.max_path_bytes == 0
        || profile.max_file_bytes == 0
        || profile.max_total_bytes < profile.max_file_bytes
    {
        return Err(
            "campaign workspace requires an exact repository and positive bounded source limits"
                .into(),
        );
    }
    Ok(())
}

pub(super) fn preflight(profile: &WorkspaceProfile, worker_gid: u32) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    validate(profile)?;
    let metadata = fs::symlink_metadata(&profile.work_root).map_err(super::text)?;
    if !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.gid() != worker_gid
        || metadata.mode() & 0o7777 != 0o750
        || fs::canonicalize(&profile.work_root).map_err(super::text)? != profile.work_root
    {
        return Err("campaign workspace parent must be canonical root-owned mode 0750 in the worker data group".into());
    }
    Ok(())
}

pub(super) fn prepare(
    profile: &super::ImplementationCampaignDriveProfile,
    work: &kiln_core::implementation::ImplementationWork,
    attempt: &kiln_core::implementation::ImplementationAttempt,
) -> Result<PathBuf, String> {
    if work.repository != profile.workspace.repository {
        return Err("workspace source mapping differs from the admitted repository".into());
    }
    let base = work
        .base
        .as_ref()
        .ok_or("campaign workspace requires an exact admitted base")?;
    let destination = profile.workspace.work_root.join(attempt.identity.as_str());
    if fs::symlink_metadata(&destination).is_ok() {
        return Err("workspace exists without complete bound request metadata; retain it for reconciliation, never overwrite possible effects".into());
    }
    let limits = kiln_core::source::GitSnapshotLimits {
        max_entries: profile.workspace.max_entries,
        max_path_bytes: profile.workspace.max_path_bytes,
        max_file_bytes: profile.workspace.max_file_bytes,
        max_total_bytes: profile.workspace.max_total_bytes,
    };
    crate::worker::source::materialize_exact(
        &profile.workspace.repository_path,
        base.native_id.as_str(),
        &limits,
        &destination,
    )?;
    // The root-owned 0700 top directory stays inaccessible until the complete
    // bounded ordinary-file tree has been handed to the trusted dispatcher.
    assign_tree(
        &destination,
        profile.roles.worker.uid,
        profile.roles.worker.gid,
    )?;
    Ok(destination)
}

/// Retain a bounded exact Git source observation for Cairn without exposing an
/// operator checkout or worker-writable tree as the next unit's source.
pub(super) fn materialize_observation(
    profile: &WorkspaceProfile,
    backing: &Path,
    revision: &str,
) -> Result<PathBuf, String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if !profile.work_root.is_absolute()
        || !backing.is_absolute()
        || revision.is_empty()
        || revision.len() > 64
        || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("native source observation requires an exact bounded Git revision".into());
    }
    let parent = fs::symlink_metadata(&profile.work_root).map_err(super::text)?;
    if !parent.is_dir() || parent.uid() != 0 || parent.mode() & 0o7777 != 0o750 {
        return Err(
            "native source observation parent is not the admitted root-owned workspace".into(),
        );
    }
    let root = profile.work_root.join("source-observations");
    if !root.exists() {
        fs::create_dir(&root).map_err(super::text)?;
        std::os::unix::fs::chown(&root, Some(0), Some(parent.gid())).map_err(super::text)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o750)).map_err(super::text)?;
    }
    let metadata = fs::symlink_metadata(&root).map_err(super::text)?;
    if !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.gid() != parent.gid()
        || metadata.mode() & 0o7777 != 0o750
        || fs::canonicalize(&root).map_err(super::text)? != root
    {
        return Err(
            "native source observation store is not a canonical root-owned bounded workspace"
                .into(),
        );
    }
    let observation_key = blake3::hash(format!("{}\0{}", profile.repository, revision).as_bytes())
        .to_hex()
        .to_string();
    let destination = root.join(observation_key);
    let mut retained = 0usize;
    for entry in fs::read_dir(&root).map_err(super::text)? {
        validate_read_only(&entry.map_err(super::text)?.path(), parent.gid())?;
        retained = retained
            .checked_add(1)
            .ok_or("retained source observation count overflow")?;
    }
    if !destination.exists() && retained >= MAX_RETAINED_SOURCE_OBSERVATIONS {
        return Err(
            "bounded native source observation store is full; retained snapshots were not deleted"
                .into(),
        );
    }
    let limits = kiln_core::source::GitSnapshotLimits {
        max_entries: profile.max_entries,
        max_path_bytes: profile.max_path_bytes,
        max_file_bytes: profile.max_file_bytes,
        max_total_bytes: profile.max_total_bytes,
    };
    if !destination.exists() {
        crate::worker::source::materialize_exact(backing, revision, &limits, &destination)?;
        make_read_only(&destination, parent.gid())?;
    }
    Ok(destination)
}

fn make_read_only(path: &Path, gid: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    for entry in fs::read_dir(path).map_err(super::text)? {
        let member = entry.map_err(super::text)?.path();
        let metadata = fs::symlink_metadata(&member).map_err(super::text)?;
        if metadata.is_dir() {
            make_read_only(&member, gid)?;
        } else if metadata.is_file() {
            std::os::unix::fs::chown(&member, Some(0), Some(gid)).map_err(super::text)?;
            fs::set_permissions(&member, fs::Permissions::from_mode(0o440)).map_err(super::text)?;
        } else {
            return Err("native source observation contains a non-ordinary file".into());
        }
    }
    std::os::unix::fs::chown(path, Some(0), Some(gid)).map_err(super::text)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o550)).map_err(super::text)
}

fn validate_read_only(path: &Path, gid: u32) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(path).map_err(super::text)?;
    if metadata.uid() != 0
        || metadata.gid() != gid
        || fs::canonicalize(path).map_err(super::text)? != path
    {
        return Err("retained native source observation has an unexpected owner or path".into());
    }
    if metadata.is_dir() {
        if metadata.mode() & 0o7777 != 0o550 {
            return Err("retained native source observation directory is writable".into());
        }
        for entry in fs::read_dir(path).map_err(super::text)? {
            validate_read_only(&entry.map_err(super::text)?.path(), gid)?;
        }
    } else if !metadata.is_file() || metadata.mode() & 0o7777 != 0o440 {
        return Err(
            "retained native source observation member is not an immutable ordinary file".into(),
        );
    }
    Ok(())
}

fn assign_tree(path: &Path, uid: u32, gid: u32) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(super::text)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(super::text)? {
            assign_tree(&entry.map_err(super::text)?.path(), uid, gid)?;
        }
    } else if !metadata.is_file() {
        return Err("campaign source materialization contains a non-ordinary member".into());
    }
    std::os::unix::fs::chown(path, Some(uid), Some(gid)).map_err(super::text)
}
