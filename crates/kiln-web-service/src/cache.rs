//! Explicit, immutable, content-verified shared cache policy and storage.
// r[impl kiln.web.developer_loop.cache]
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Component, Path, PathBuf};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

pub const MAX_POLICY_BYTES: usize = 262_144;
pub const MAX_ENTRY_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_FILES: usize = 4096;
const MAX_ENTRIES: usize = 100_000;
const MAX_AGE_SECONDS: u64 = 365 * 24 * 60 * 60;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheProfile {
    pub schema: String,
    pub directory: PathBuf,
    pub maximum_entry_bytes: usize,
    pub maximum_total_bytes: u64,
    pub maximum_entries: usize,
    pub maximum_age_seconds: u64,
    pub maximum_files: usize,
    pub projects: Vec<CacheProject>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheProject {
    pub project_id: String,
    pub producers: Vec<String>,
    pub toolchains: Vec<String>,
    pub platforms: Vec<String>,
    pub paths: Vec<PathBuf>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorkerCacheProfile {
    pub producer: String,
    pub toolchain: String,
    pub platform: String,
    pub inputs: Vec<PathBuf>,
    pub paths: Vec<PathBuf>,
    pub maximum_entry_bytes: usize,
    pub maximum_files: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CacheKey {
    pub project_id: String,
    pub node_id: String,
    pub producer: String,
    pub toolchain: String,
    pub platform: String,
    pub inputs: BTreeMap<PathBuf, String>,
    pub paths: Vec<PathBuf>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CacheObservation {
    pub key: String,
    pub outcome: String,
    pub transferred_bytes: u64,
    pub duration_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CacheFile {
    pub path: PathBuf,
    pub blake3: String,
    pub body_base64: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CacheEntry {
    pub key: CacheKey,
    pub files: Vec<CacheFile>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheEnvelope {
    pub entry: CacheEntry,
    pub blake3: String,
}

pub fn hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && value.bytes().all(|b| (32..=126).contains(&b))
}
/// Cache data is never a credential transport. Deny sensitive components even
/// when an operator accidentally declares a broad output directory.
pub fn safe_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.as_os_str().len() <= 1024
        && path.components().all(|part| {
            let Component::Normal(name) = part else {
                return false;
            };
            let Some(name) = name.to_str() else {
                return false;
            };
            let lower = name.to_ascii_lowercase();
            !name.is_empty()
                && name.bytes().all(|b| (33..=126).contains(&b) && b != b'\\')
                && !matches!(
                    lower.as_str(),
                    ".git"
                        | ".ssh"
                        | ".aws"
                        | ".azure"
                        | ".config"
                        | ".gnupg"
                        | ".netrc"
                        | ".npmrc"
                        | ".pypirc"
                        | "credentials"
                        | "id_rsa"
                        | "id_ed25519"
                )
                && !lower.starts_with(".env")
                && !lower.contains("credential")
                && !lower.contains("secret")
                && !lower.contains("token")
                && !lower.ends_with(".pem")
                && !lower.ends_with(".key")
        })
}
fn paths(paths: &[PathBuf]) -> bool {
    !paths.is_empty()
        && paths.len() <= MAX_FILES
        && paths.iter().all(|p| safe_path(p))
        && paths.iter().enumerate().all(|(i, p)| {
            paths
                .iter()
                .skip(i + 1)
                .all(|q| !p.starts_with(q) && !q.starts_with(p))
        })
}
impl WorkerCacheProfile {
    pub fn admit(&self) -> Result<(), String> {
        if !text(&self.producer)
            || !text(&self.toolchain)
            || !text(&self.platform)
            || !paths(&self.inputs)
            || !paths(&self.paths)
            || self.maximum_entry_bytes == 0
            || self.maximum_entry_bytes > MAX_ENTRY_BYTES
            || self.maximum_files == 0
            || self.maximum_files > MAX_FILES
        {
            return Err("worker cache policy rejected".into());
        }
        Ok(())
    }
}
impl CacheKey {
    pub fn admit(&self) -> Result<(), String> {
        if !crate::profile::identifier(&self.project_id)
            || !crate::profile::identifier(&self.node_id)
            || !text(&self.producer)
            || !text(&self.toolchain)
            || !text(&self.platform)
            || !paths(&self.paths)
            || self.inputs.is_empty()
            || self.inputs.len() > MAX_FILES
            || self.inputs.iter().any(|(p, h)| !safe_path(p) || !hex(h))
        {
            return Err("cache key rejected".into());
        }
        Ok(())
    }
    pub fn identity(&self) -> Result<String, String> {
        self.admit()?;
        let bytes = serde_json::to_vec(&("kiln.shared-cache.key.v1", self))
            .map_err(|_| "cache key encoding failed")?;
        Ok(blake3::hash(&bytes).to_hex().to_string())
    }
}
impl CacheEntry {
    pub fn verify(&self, maximum: usize, maximum_files: usize) -> Result<usize, String> {
        self.key.admit()?;
        if self.files.len() > maximum_files {
            return Err("cache file count exceeded".into());
        }
        let mut seen = BTreeSet::new();
        let mut total = 0usize;
        for file in &self.files {
            if !safe_path(&file.path)
                || !hex(&file.blake3)
                || !seen.insert(file.path.as_path())
                || !self
                    .key
                    .paths
                    .iter()
                    .any(|root| file.path.starts_with(root))
                || file.body_base64.len() > maximum.saturating_add(2) / 3 * 4
            {
                return Err("cache manifest path or size rejected".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&file.body_base64)
                .map_err(|_| "cache file encoding rejected")?;
            total = total
                .checked_add(bytes.len())
                .ok_or("cache byte overflow")?;
            if total > maximum || blake3::hash(&bytes).to_hex().as_str() != file.blake3 {
                return Err("cache content integrity rejected".into());
            }
        }
        for path in &seen {
            if path
                .ancestors()
                .skip(1)
                .any(|ancestor| seen.contains(ancestor))
            {
                return Err("cache file directory conflict".into());
            }
        }
        Ok(total)
    }
}
impl CacheEnvelope {
    pub fn new(entry: CacheEntry) -> Result<Self, String> {
        let blake3 =
            blake3::hash(&serde_json::to_vec(&entry).map_err(|_| "cache encoding failed")?)
                .to_hex()
                .to_string();
        Ok(Self { entry, blake3 })
    }
    pub fn verify(&self, maximum: usize, maximum_files: usize) -> Result<usize, String> {
        let total = self.entry.verify(maximum, maximum_files)?;
        let digest =
            blake3::hash(&serde_json::to_vec(&self.entry).map_err(|_| "cache encoding failed")?);
        if !hex(&self.blake3) || digest.to_hex().as_str() != self.blake3 {
            return Err("cache envelope integrity rejected".into());
        }
        Ok(total)
    }
}
impl CacheProfile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, MAX_POLICY_BYTES, false)?;
        let profile: Self = serde_json::from_slice(&bytes).map_err(|_| "cache policy malformed")?;
        profile.preflight()?;
        Ok(profile)
    }
    /// Validate deployment prerequisites without creating or pruning anything.
    pub fn preflight(&self) -> Result<(), String> {
        self.admit()?;
        crate::worker::files::owned_directory(&self.directory)
    }
    pub fn preflight_for(&self, service: &crate::profile::ServiceProfile) -> Result<(), String> {
        self.preflight()?;
        if self.maximum_wire_bytes() > service.bounds.max_request_bytes
            || self.maximum_wire_bytes() > service.bounds.max_response_bytes
        {
            return Err("cache envelope bound exceeds service HTTP transport policy".into());
        }
        if self.directory.starts_with(&service.state_directory)
            || service.state_directory.starts_with(&self.directory)
            || service.projects.iter().any(|p| {
                self.directory.starts_with(&p.repository_path)
                    || p.repository_path.starts_with(&self.directory)
            })
            || self.projects.iter().any(|p| {
                !service
                    .projects
                    .iter()
                    .any(|configured| configured.id == p.project_id)
            })
        {
            return Err("cache storage or project scope overlaps canonical state".into());
        }
        Ok(())
    }
    pub fn admit(&self) -> Result<(), String> {
        if self.schema != "kiln.web-cache.v1"
            || !crate::profile::absolute(&self.directory)
            || self.maximum_entry_bytes == 0
            || self.maximum_entry_bytes > MAX_ENTRY_BYTES
            || self.maximum_total_bytes < self.maximum_entry_bytes as u64
            || self.maximum_entries == 0
            || self.maximum_entries > MAX_ENTRIES
            || self.maximum_age_seconds == 0
            || self.maximum_age_seconds > MAX_AGE_SECONDS
            || self.maximum_files == 0
            || self.maximum_files > MAX_FILES
            || self.projects.is_empty()
            || self.projects.len() > 256
        {
            return Err("cache policy rejected".into());
        }
        let mut seen = BTreeSet::new();
        for project in &self.projects {
            if !crate::profile::identifier(&project.project_id)
                || !seen.insert(&project.project_id)
                || !paths(&project.paths)
                || [&project.producers, &project.toolchains, &project.platforms]
                    .iter()
                    .any(|list| {
                        list.is_empty() || list.len() > 256 || list.iter().any(|v| !text(v))
                    })
            {
                return Err("cache project policy rejected".into());
            }
        }
        Ok(())
    }
    pub fn admits_key(&self, key: &CacheKey) -> bool {
        key.admit().is_ok()
            && self.projects.iter().any(|p| {
                p.project_id == key.project_id
                    && p.producers.contains(&key.producer)
                    && p.toolchains.contains(&key.toolchain)
                    && p.platforms.contains(&key.platform)
                    && key
                        .paths
                        .iter()
                        .all(|path| p.paths.iter().any(|allowed| path.starts_with(allowed)))
            })
    }
    pub fn maximum_wire_bytes(&self) -> usize {
        // JSON/base64 expansion plus bounded per-file paths and hashes.
        self.maximum_entry_bytes
            .saturating_mul(2)
            .saturating_add(self.maximum_files.saturating_mul(2048))
            .saturating_add(MAX_POLICY_BYTES)
    }
}

/// A dedicated private directory, separately locked from canonical state. Only
/// verified cache entries created in its marked child are eligible for eviction.
#[derive(Debug)]
pub struct CacheStore {
    pub profile: CacheProfile,
    directory: PathBuf,
    _lock: File,
    directory_handle: File,
}
impl CacheStore {
    pub fn open(profile: CacheProfile) -> Result<Self, String> {
        use fs2::FileExt;
        profile.admit()?;
        crate::worker::files::owned_directory(&profile.directory)?;
        let directory = profile.directory.join("kiln-cache-v1");
        let created = match fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => true,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(_) => return Err("cache directory creation failed".into()),
        };
        crate::worker::files::owned_directory(&directory)?;
        // Anchor subsequent paths to the opened directory, not a mutable ancestor.
        let fd = crate::worker::files::directory(&directory)?;
        let directory_handle = File::from(fd);
        use std::os::fd::AsRawFd;
        let directory = PathBuf::from(format!("/proc/self/fd/{}", directory_handle.as_raw_fd()));
        const MARKER: &[u8] = b"kiln.shared-cache.storage.v1\n";
        let marker_fd = nix::fcntl::openat(
            &directory_handle,
            Path::new("owner.marker"),
            if created {
                nix::fcntl::OFlag::O_WRONLY
                    | nix::fcntl::OFlag::O_CREAT
                    | nix::fcntl::OFlag::O_EXCL
                    | nix::fcntl::OFlag::O_NOFOLLOW
                    | nix::fcntl::OFlag::O_CLOEXEC
            } else {
                nix::fcntl::OFlag::O_RDONLY
                    | nix::fcntl::OFlag::O_NOFOLLOW
                    | nix::fcntl::OFlag::O_NONBLOCK
                    | nix::fcntl::OFlag::O_CLOEXEC
            },
            nix::sys::stat::Mode::from_bits_truncate(0o600),
        )
        .map_err(|_| "cache ownership marker unavailable; historical data preserved")?;
        let mut marker = File::from(marker_fd);
        if created {
            marker
                .write_all(MARKER)
                .map_err(|_| "cache ownership marker write failed")?;
            marker
                .sync_all()
                .map_err(|_| "cache ownership marker sync failed")?;
        } else {
            let metadata = marker
                .metadata()
                .map_err(|_| "cache ownership marker unavailable")?;
            if !metadata.is_file()
                || metadata.nlink() != 1
                || metadata.uid() != nix::unistd::geteuid().as_raw()
                || metadata.mode() & 0o077 != 0
                || metadata.len() != MARKER.len() as u64
            {
                return Err("cache ownership marker denied".into());
            }
            let mut bytes = Vec::with_capacity(MARKER.len());
            marker
                .take(MARKER.len() as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "cache ownership marker read failed")?;
            if bytes != MARKER {
                return Err("cache ownership marker conflict".into());
            }
        }
        let lock_path = directory.join("owner.lock");
        let lock_fd = nix::fcntl::open(
            &lock_path,
            nix::fcntl::OFlag::O_RDWR
                | nix::fcntl::OFlag::O_CREAT
                | nix::fcntl::OFlag::O_NOFOLLOW
                | nix::fcntl::OFlag::O_CLOEXEC,
            nix::sys::stat::Mode::from_bits_truncate(0o600),
        )
        .map_err(|_| "cache lock unavailable")?;
        let lock = File::from(lock_fd);
        let lock_metadata = lock
            .metadata()
            .map_err(|_| "cache lock metadata unavailable")?;
        if !lock_metadata.is_file()
            || lock_metadata.nlink() != 1
            || lock_metadata.uid() != nix::unistd::geteuid().as_raw()
            || lock_metadata.mode() & 0o077 != 0
        {
            return Err("cache lock file denied".into());
        }
        lock.try_lock_exclusive()
            .map_err(|_| "cache directory already owned")?;
        Ok(Self {
            profile,
            directory,
            _lock: lock,
            directory_handle,
        })
    }
    fn read(&self, key: &str) -> Result<CacheEnvelope, String> {
        let fd = nix::fcntl::openat(
            &self.directory_handle,
            Path::new(key),
            nix::fcntl::OFlag::O_RDONLY
                | nix::fcntl::OFlag::O_NOFOLLOW
                | nix::fcntl::OFlag::O_NONBLOCK
                | nix::fcntl::OFlag::O_CLOEXEC,
            nix::sys::stat::Mode::empty(),
        )
        .map_err(|_| "cache retained file unavailable")?;
        let mut file = File::from(fd);
        let before = file
            .metadata()
            .map_err(|_| "cache retained metadata unavailable")?;
        let maximum = self.profile.maximum_wire_bytes();
        if !before.is_file()
            || before.nlink() != 1
            || before.uid() != nix::unistd::geteuid().as_raw()
            || before.mode() & 0o077 != 0
            || before.len() > maximum as u64
        {
            return Err("cache retained file denied".into());
        }
        let mut bytes = Vec::with_capacity(before.len() as usize);
        Read::by_ref(&mut file)
            .take(maximum as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "cache retained read failed")?;
        let after = file
            .metadata()
            .map_err(|_| "cache retained metadata unavailable")?;
        if bytes.len() as u64 != before.len()
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err("cache retained file changed".into());
        }
        let envelope: CacheEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| "cache retained entry malformed")?;
        envelope.verify(self.profile.maximum_entry_bytes, self.profile.maximum_files)?;
        if envelope.entry.key.identity()? != key || !self.profile.admits_key(&envelope.entry.key) {
            return Err("cache retained key conflict".into());
        }
        Ok(envelope)
    }
    pub fn restore(
        &mut self,
        key: &CacheKey,
        now_seconds: u64,
    ) -> Result<Option<CacheEnvelope>, String> {
        if !self.profile.admits_key(key) {
            return Err("cache key policy denied".into());
        }
        self.prune(now_seconds, 0, false)?;
        let id = key.identity()?;
        match fs::symlink_metadata(self.directory.join(&id)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err("cache entry unavailable".into()),
            Ok(_) => self.read(&id).map(Some),
        }
    }
    pub fn publish(&mut self, envelope: &CacheEnvelope, now_seconds: u64) -> Result<bool, String> {
        if !self.profile.admits_key(&envelope.entry.key) {
            return Err("cache key policy denied".into());
        }
        envelope.verify(self.profile.maximum_entry_bytes, self.profile.maximum_files)?;
        let id = envelope.entry.key.identity()?;
        if fs::symlink_metadata(self.directory.join(&id)).is_ok() {
            let previous = self.read(&id)?;
            return if previous.blake3 == envelope.blake3 {
                Ok(false)
            } else {
                Err("cache immutable key conflict".into())
            };
        }
        let bytes = serde_json::to_vec(envelope).map_err(|_| "cache encoding failed")?;
        if bytes.len() > self.profile.maximum_wire_bytes()
            || bytes.len() as u64 > self.profile.maximum_total_bytes
        {
            return Err("cache retained byte bound".into());
        }
        self.prune(now_seconds, bytes.len() as u64, true)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)
            .map_err(|_| "cache staging unavailable")?;
        temporary
            .write_all(&bytes)
            .map_err(|_| "cache staging write failed")?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| "cache staging sync failed")?;
        temporary
            .persist_noclobber(self.directory.join(&id))
            .map_err(|_| "cache atomic publication conflict")?;
        self.directory_handle
            .sync_all()
            .map_err(|_| "cache directory sync failed")?;
        Ok(true)
    }
    fn prune(&mut self, now: u64, incoming: u64, adding: bool) -> Result<(), String> {
        let mut entries = Vec::new();
        let mut total = 0u64;
        let scan_bound = self.profile.maximum_entries.saturating_add(16);
        for (index, entry) in fs::read_dir(&self.directory)
            .map_err(|_| "cache inventory unavailable")?
            .enumerate()
        {
            if index > scan_bound {
                return Err("cache inventory bound exceeded".into());
            }
            let entry = entry.map_err(|_| "cache inventory unavailable")?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return Err("cache unknown retained name".into());
            };
            if matches!(name.as_str(), "owner.lock" | "owner.marker") {
                continue;
            }
            // Unknown historical data is never pruned.
            if !hex(&name) {
                return Err("cache unknown retained data".into());
            }
            self.read(&name)?;
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| "cache metadata unavailable")?;
            if !metadata.is_file() || metadata.nlink() != 1 {
                return Err("cache unsafe retained file".into());
            }
            let modified = metadata
                .modified()
                .map_err(|_| "cache age unavailable")?
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "cache age invalid")?
                .as_secs();
            total = total
                .checked_add(metadata.len())
                .ok_or("cache retained byte overflow")?;
            entries.push((modified, name, metadata.len()));
        }
        entries.sort();
        let mut count = entries.len();
        for (modified, name, bytes) in entries {
            let expired = now.saturating_sub(modified) >= self.profile.maximum_age_seconds;
            let capacity = adding
                && (count >= self.profile.maximum_entries
                    || total.saturating_add(incoming) > self.profile.maximum_total_bytes);
            if !expired && !capacity {
                continue;
            }
            fs::remove_file(self.directory.join(name)).map_err(|_| "cache eviction failed")?;
            total -= bytes;
            count -= 1;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
