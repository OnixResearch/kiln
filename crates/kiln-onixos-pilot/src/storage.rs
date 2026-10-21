use crate::{MAX_CONFIG_BYTES, Result};
use fs2::FileExt;
use serde::{Serialize, de::DeserializeOwned};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

const PRIVATE_FILE_MODE: u32 = 0o600;
const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
const OVERFLOW_SENTINEL: usize = 1;

pub(crate) fn safe_path(path: &Path) -> Result<()> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let parent = absolute.parent().ok_or("path lacks parent")?;
    if fs::canonicalize(parent).map_err(|error| error.to_string())? != parent {
        return Err("path parent must be canonical and must not traverse symlinks".into());
    }
    match fs::symlink_metadata(&absolute) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            Err("path must name a regular non-symlink file".into())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn load_json<T: DeserializeOwned>(path: &Path, bound: usize) -> Result<T> {
    safe_path(path)?;
    if bound == 0 || bound > MAX_CONFIG_BYTES {
        return Err("invalid document byte bound".into());
    }
    let mut bytes = Vec::new();
    let limit = bound
        .checked_add(OVERFLOW_SENTINEL)
        .ok_or("document bound overflow")?;
    File::open(path)
        .map_err(|error| error.to_string())?
        .take(u64::try_from(limit).map_err(|error| error.to_string())?)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > bound {
        return Err("document exceeds byte bound".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid document: {error}"))
}

pub fn publish_new<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err("document exceeds byte bound".into());
    }
    safe_path(path)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(PRIVATE_FILE_MODE)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    sync_parent(path)
}

pub(crate) fn private_directory(path: &Path) -> Result<()> {
    fs::create_dir(path).map_err(|error| error.to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_DIRECTORY_MODE))
        .map_err(|error| error.to_string())?;
    sync_parent(path)
}

fn sync_parent(path: &Path) -> Result<()> {
    let parent = path.parent().ok_or("path lacks parent")?;
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    File::open(parent)
        .and_then(|parent| parent.sync_all())
        .map_err(|error| error.to_string())
}

#[derive(Debug)]
pub struct JournalLock {
    _file: File,
    path: PathBuf,
    temporary: PathBuf,
}
impl JournalLock {
    pub fn acquire(path: &Path) -> Result<Self> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or("journal requires an explicit private parent directory")?;
        let metadata = fs::metadata(parent).map_err(|error| error.to_string())?;
        const UNTRUSTED_WRITE_BITS: u32 = 0o022;
        if metadata.permissions().mode() & UNTRUSTED_WRITE_BITS != 0 {
            return Err("journal directory must not be writable by group or other users".into());
        }
        if let Ok(metadata) = fs::symlink_metadata(path) {
            const PUBLIC_ACCESS_BITS: u32 = 0o077;
            if metadata.permissions().mode() & PUBLIC_ACCESS_BITS != 0 {
                return Err("journal file must be private to its owner".into());
            }
        }
        safe_path(path)?;
        let lock = path.with_extension("lock");
        safe_path(&lock)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(PRIVATE_FILE_MODE)
            .open(lock)
            .map_err(|error| error.to_string())?;
        file.try_lock_exclusive()
            .map_err(|error| format!("journal is locked: {error}"))?;
        let temporary = path.with_extension("pending");
        if temporary.try_exists().map_err(|error| error.to_string())? {
            return Err("partial journal publication exists; refuse ambiguous state".into());
        }
        Ok(Self {
            _file: file,
            path: path.to_owned(),
            temporary,
        })
    }
    pub fn store<T: Serialize>(&self, value: &T, bound: usize) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
        if bytes.len() > bound {
            return Err("journal exceeds byte bound".into());
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(PRIVATE_FILE_MODE)
            .open(&self.temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&self.temporary, &self.path).map_err(|error| error.to_string())?;
        sync_parent(&self.path)
    }
}
