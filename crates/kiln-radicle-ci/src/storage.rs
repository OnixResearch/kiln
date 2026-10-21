use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use kiln_core::ContentId;
use kiln_onixos_pilot::{MAX_CONFIG_BYTES, load_json, publish_new};

use crate::Result;
use crate::project::{ProjectSourceIntent, ProjectWorkerProfile};

const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
const PUBLIC_PERMISSION_BITS: u32 = 0o077;

pub(crate) fn load_worker(path: &Path, expected: &ContentId) -> Result<ProjectWorkerProfile> {
    let mut bytes = Vec::new();
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| error.to_string())?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("reviewed worker profile must be a regular file".into());
    }
    file.take((MAX_CONFIG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_CONFIG_BYTES || blake3::hash(&bytes).to_hex().as_str() != expected.as_str()
    {
        return Err("worker profile bytes differ from their reviewed identity or bound".into());
    }
    let profile: ProjectWorkerProfile =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    profile.admit()?;
    Ok(profile)
}

pub(crate) fn prepare_candidate(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or("candidate root lacks a private parent")?;
    let parent_metadata = fs::symlink_metadata(parent).map_err(|error| error.to_string())?;
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.permissions().mode() & PUBLIC_PERMISSION_BITS != 0
        || fs::canonicalize(parent).map_err(|error| error.to_string())? != parent
    {
        return Err("project state root must be canonical and private".into());
    }
    match fs::DirBuilder::new()
        .mode(PRIVATE_DIRECTORY_MODE)
        .create(path)
    {
        Ok(()) => File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.to_string()),
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & PUBLIC_PERMISSION_BITS != 0
        || fs::canonicalize(path).map_err(|error| error.to_string())? != path
    {
        return Err("candidate directory must be canonical and private".into());
    }
    Ok(())
}

pub(crate) fn publish_source_intent(path: &Path, intent: &ProjectSourceIntent) -> Result<()> {
    if path.try_exists().map_err(|error| error.to_string())? {
        let old: ProjectSourceIntent = load_json(path, MAX_CONFIG_BYTES)?;
        if &old != intent {
            return Err("candidate already reserves different exact source facts".into());
        }
        return Ok(());
    }
    publish_new(path, intent)
}
