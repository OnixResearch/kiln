//! Worker cache transport uses the same pinned HTTPS/auth client as job ingress.
//! All manifests are verified in full before any restoration writes occur.
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use base64::Engine as _;
use nix::fcntl::AtFlags;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::{Mode, SFlag, fstatat};
use serde_json::{Value, json};

use super::client::{RunBinding, ServiceClient};
use super::profile::WorkerProfile;
use crate::cache::{
    CacheEntry, CacheEnvelope, CacheFile, CacheKey, CacheObservation, WorkerCacheProfile, safe_path,
};

fn elapsed(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}
fn wire_bound(config: &WorkerCacheProfile) -> usize {
    config
        .maximum_entry_bytes
        .saturating_mul(2)
        .saturating_add(config.maximum_files.saturating_mul(2048))
        .saturating_add(crate::cache::MAX_POLICY_BYTES)
}
/// Build an exact local transfer key without transport or execution authority.
pub fn local_key(
    project_id: &str,
    node_id: &str,
    config: &WorkerCacheProfile,
    checkout: &Path,
) -> Result<CacheKey, String> {
    if config.inputs.iter().any(|input| {
        config
            .paths
            .iter()
            .any(|output| input.starts_with(output) || output.starts_with(input))
    }) {
        return Err("cache outputs overlap source inputs".into());
    }
    config.admit()?;
    let mut inputs = BTreeMap::new();
    let mut total = 0usize;
    for input in &config.inputs {
        let bytes = super::files::artifact(checkout, input, config.maximum_entry_bytes)?;
        total = total
            .checked_add(bytes.len())
            .ok_or("cache input byte overflow")?;
        if total > config.maximum_entry_bytes {
            return Err("cache input byte bound exceeded".into());
        }
        inputs.insert(input.clone(), blake3::hash(&bytes).to_hex().to_string());
    }
    let key = CacheKey {
        project_id: project_id.into(),
        node_id: node_id.into(),
        producer: config.producer.clone(),
        toolchain: config.toolchain.clone(),
        platform: config.platform.clone(),
        inputs,
        paths: config.paths.clone(),
    };
    key.admit()?;
    Ok(key)
}

/// Collect only declared bounded regular files through anchored descriptors.
pub fn collect_local(
    root: &Path,
    key: &CacheKey,
    config: &WorkerCacheProfile,
) -> Result<CacheEnvelope, String> {
    config.admit()?;
    if local_key(&key.project_id, &key.node_id, config, root)? != *key {
        return Err("cache source or policy changed before publication".into());
    }
    let mut files = Vec::new();
    let mut total = 0usize;
    let mut visited = 0usize;
    for path in &config.paths {
        collect(root, path, config, &mut files, &mut total, &mut visited)?;
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let entry = CacheEntry {
        key: key.clone(),
        files,
    };
    entry.verify(config.maximum_entry_bytes, config.maximum_files)?;
    CacheEnvelope::new(entry)
}

/// Fully verify source, policy and envelope before descriptor-relative writes.
pub fn restore_local(
    root: &Path,
    envelope: &CacheEnvelope,
    config: &WorkerCacheProfile,
) -> Result<(), String> {
    config.admit()?;
    if local_key(
        &envelope.entry.key.project_id,
        &envelope.entry.key.node_id,
        config,
        root,
    )? != envelope.entry.key
    {
        return Err("cache restore source or policy mismatch".into());
    }
    envelope.verify(config.maximum_entry_bytes, config.maximum_files)?;
    restore_entry(root, &envelope.entry)
}

/// Require absent output roots without following even intermediate symlinks.
/// Hosted source snapshots use this before restore to exclude source files.
pub fn require_absent_outputs(root: &Path, config: &WorkerCacheProfile) -> Result<(), String> {
    config.admit()?;
    for output in &config.paths {
        let mut directory = super::files::directory(root)?;
        let mut parts = output.components().peekable();
        while let Some(part) = parts.next() {
            let Component::Normal(name) = part else {
                return Err("cache output path denied".into());
            };
            let metadata = match fstatat(&directory, Path::new(name), AtFlags::AT_SYMLINK_NOFOLLOW)
            {
                Err(nix::errno::Errno::ENOENT) => break,
                Err(_) => return Err("cache output root inspection denied".into()),
                Ok(metadata) => metadata,
            };
            if parts.peek().is_none() {
                return Err("cache output root already exists in source workspace".into());
            }
            if SFlag::from_bits_truncate(metadata.st_mode) & SFlag::S_IFMT != SFlag::S_IFDIR {
                return Err("cache output parent is not a regular directory".into());
            }
            directory = openat(
                &directory,
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| "cache output traversal denied")?;
        }
    }
    Ok(())
}
fn request(
    binding: &RunBinding,
    key: &CacheKey,
    entry: Option<&CacheEnvelope>,
    success: bool,
) -> Value {
    json!({"attempt_id":binding.attempt_id,"execution":binding.execution,"source_revision":binding.source_revision,"node_id":key.node_id,"key":key,"entry":entry,"observed_success":success})
}
fn send(
    service: &ServiceClient,
    binding: &RunBinding,
    operation: &str,
    body: &Value,
    config: &WorkerCacheProfile,
) -> Result<(Value, u64), String> {
    use std::io::Read;
    let encoded = serde_json::to_vec(body).map_err(|_| "cache request encoding failed")?;
    let request_bytes = encoded.len() as u64;
    if encoded.len() > wire_bound(config) {
        return Err("cache request exceeds bound".into());
    }
    let response = service
        .client
        .post(format!(
            "{}/api/runs/{}/cache/{operation}",
            service.origin, binding.id
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            service.authorization.clone(),
        )
        .header(reqwest::header::ORIGIN, &service.origin)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(encoded)
        .send()
        .map_err(|_| "cache HTTPS request failed")?;
    if !response.status().is_success() {
        return Err("cache service rejected request".into());
    }
    let maximum = wire_bound(config).min(service.maximum_response_bytes);
    if response
        .content_length()
        .is_some_and(|n| n > maximum as u64)
    {
        return Err("cache response exceeds bound".into());
    }
    let mut bytes = Vec::new();
    response
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cache response read failed")?;
    if bytes.len() > maximum {
        return Err("cache response exceeds bound".into());
    }
    let transferred = request_bytes
        .checked_add(bytes.len() as u64)
        .ok_or("cache transfer byte overflow")?;
    Ok((
        serde_json::from_slice(&bytes).map_err(|_| "cache response invalid")?,
        transferred,
    ))
}

pub(super) fn restore(
    service: &ServiceClient,
    profile: &WorkerProfile,
    binding: &RunBinding,
    node_id: &str,
    config: &WorkerCacheProfile,
    checkout: &Path,
) -> Result<(CacheKey, CacheObservation), String> {
    let started = Instant::now();
    let key = local_key(&profile.project_id, node_id, config, checkout)?;
    let (response, transferred_bytes) = send(
        service,
        binding,
        "restore",
        &request(binding, &key, None, false),
        config,
    )?;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Response {
        entry: Option<CacheEnvelope>,
    }
    let response: Response =
        serde_json::from_value(response).map_err(|_| "cache restore response invalid")?;
    let outcome = if let Some(envelope) = response.entry {
        if envelope.entry.key != key {
            return Err("cache restored key conflict".into());
        }
        restore_local(checkout, &envelope, config)?;
        "hit"
    } else {
        "miss"
    };
    let observation = CacheObservation {
        key: key.identity()?,
        outcome: outcome.into(),
        transferred_bytes,
        duration_ms: elapsed(started),
    };
    Ok((key, observation))
}

/// Caller may only invoke after child success and admitted log/report/artifact
/// ingress. The explicit flag is the authenticated worker's observed evidence;
/// it is not a claim that the cache server independently ran the child.
pub(super) fn publish(
    service: &ServiceClient,
    binding: &RunBinding,
    node_id: &str,
    config: &WorkerCacheProfile,
    checkout: &Path,
    key: &CacheKey,
    observed_success: bool,
) -> Result<CacheObservation, String> {
    let started = Instant::now();
    config.admit()?;
    if !observed_success || key.node_id != node_id {
        return Err("cache publication requires successful admitted step".into());
    }
    let envelope = collect_local(checkout, key, config)?;
    let (response, transferred_bytes) = send(
        service,
        binding,
        "publish",
        &request(binding, key, Some(&envelope), true),
        config,
    )?;
    if response.get("published").and_then(Value::as_bool) != Some(true)
        || response.get("blake3").and_then(Value::as_str) != Some(envelope.blake3.as_str())
    {
        return Err("cache publication receipt conflict".into());
    }
    Ok(CacheObservation {
        key: key.identity()?,
        outcome: "published".into(),
        transferred_bytes,
        duration_ms: elapsed(started),
    })
}

fn directory_at(root: &Path, relative: &Path, create: bool) -> Result<OwnedFd, String> {
    let mut fd = super::files::directory(root)?;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("cache directory traversal denied".into());
        };
        let path = Path::new(name);
        if create {
            match nix::sys::stat::mkdirat(&fd, path, Mode::from_bits_truncate(0o700)) {
                Ok(()) | Err(nix::errno::Errno::EEXIST) => {}
                Err(_) => return Err("cache directory creation failed".into()),
            }
        }
        fd = openat(
            &fd,
            path,
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| "cache directory traversal denied")?;
    }
    Ok(fd)
}
fn collect(
    root: &Path,
    relative: &Path,
    config: &WorkerCacheProfile,
    files: &mut Vec<CacheFile>,
    total: &mut usize,
    visited: &mut usize,
) -> Result<(), String> {
    *visited = visited.checked_add(1).ok_or("cache traversal overflow")?;
    if *visited > config.maximum_files {
        return Err("cache traversal count exceeded".into());
    }
    if !safe_path(relative) {
        return Err("cache output path denied".into());
    }
    let parent = directory_at(root, relative.parent().unwrap_or(Path::new("")), false)?;
    let name = relative.file_name().ok_or("cache file name missing")?;
    let metadata = match fstatat(&parent, Path::new(name), AtFlags::AT_SYMLINK_NOFOLLOW) {
        Ok(v) => v,
        // A declared output absent after success is not a valid publication.
        Err(_) => return Err("cache declared output unavailable".into()),
    };
    let kind = SFlag::from_bits_truncate(metadata.st_mode) & SFlag::S_IFMT;
    if kind == SFlag::S_IFDIR {
        let fd = openat(
            &parent,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| "cache directory unavailable")?;
        let directory = PathBuf::from(format!("/proc/self/fd/{}", fd.as_raw_fd()));
        for (index, child) in std::fs::read_dir(directory)
            .map_err(|_| "cache directory listing failed")?
            .enumerate()
        {
            if index >= config.maximum_files || files.len() >= config.maximum_files {
                return Err("cache file count exceeded".into());
            }
            let child = child.map_err(|_| "cache directory listing failed")?;
            collect(
                root,
                &relative.join(child.file_name()),
                config,
                files,
                total,
                visited,
            )?;
        }
    } else if kind == SFlag::S_IFREG {
        if files.len() >= config.maximum_files {
            return Err("cache file count exceeded".into());
        }
        let bytes = super::files::artifact(root, relative, config.maximum_entry_bytes)?;
        *total = total
            .checked_add(bytes.len())
            .ok_or("cache output byte overflow")?;
        if *total > config.maximum_entry_bytes {
            return Err("cache output byte bound exceeded".into());
        }
        files.push(CacheFile {
            path: relative.into(),
            blake3: blake3::hash(&bytes).to_hex().to_string(),
            body_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        });
    } else {
        return Err("cache special file or symlink denied".into());
    }
    Ok(())
}
fn preflight_entry(root: &Path, entry: &CacheEntry) -> Result<(), String> {
    for file in &entry.files {
        let mut fd = super::files::directory(root)?;
        let mut missing = false;
        for part in file.path.parent().unwrap_or(Path::new("")).components() {
            let Component::Normal(name) = part else {
                return Err("cache restore path denied".into());
            };
            match openat(
                &fd,
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            ) {
                Ok(next) => fd = next,
                Err(nix::errno::Errno::ENOENT) => {
                    missing = true;
                    break;
                }
                Err(_) => return Err("cache restore parent denied".into()),
            }
        }
        if missing {
            continue;
        }
        let name = file.path.file_name().ok_or("cache restore name missing")?;
        match fstatat(&fd, Path::new(name), AtFlags::AT_SYMLINK_NOFOLLOW) {
            Ok(metadata) => {
                if SFlag::from_bits_truncate(metadata.st_mode) & SFlag::S_IFMT != SFlag::S_IFREG
                    || metadata.st_nlink != 1
                {
                    return Err("cache restore target denied".into());
                }
                let bytes = super::files::artifact(root, &file.path, file.body_base64.len())?;
                if blake3::hash(&bytes).to_hex().as_str() != file.blake3 {
                    return Err("cache restore existing content conflict".into());
                }
            }
            Err(nix::errno::Errno::ENOENT) => {}
            Err(_) => return Err("cache restore target unavailable".into()),
        }
    }
    Ok(())
}
fn restore_entry(root: &Path, entry: &CacheEntry) -> Result<(), String> {
    // verify() is always called first; no manifest path can bypass declaration.
    preflight_entry(root, entry)?;
    for file in &entry.files {
        let parent = directory_at(root, file.path.parent().unwrap_or(Path::new("")), true)?;
        let name = file.path.file_name().ok_or("cache restore name missing")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&file.body_base64)
            .map_err(|_| "cache restore encoding invalid")?;
        let target = Path::new(name);
        match openat(
            &parent,
            target,
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        ) {
            Ok(existing) => {
                use std::io::Read;
                use std::os::unix::fs::MetadataExt;
                let existing = File::from(existing);
                let metadata = existing
                    .metadata()
                    .map_err(|_| "cache restore target unavailable")?;
                if !metadata.is_file()
                    || metadata.nlink() != 1
                    || metadata.len() != bytes.len() as u64
                {
                    return Err("cache restore target conflict".into());
                }
                let mut previous = Vec::with_capacity(bytes.len());
                existing
                    .take(bytes.len() as u64 + 1)
                    .read_to_end(&mut previous)
                    .map_err(|_| "cache restore target read failed")?;
                if previous != bytes {
                    return Err("cache restore target conflict".into());
                }
            }
            Err(nix::errno::Errno::ENOENT) => {
                let directory = PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd()));
                let mut staged = tempfile::NamedTempFile::new_in(&directory)
                    .map_err(|_| "cache restore staging failed")?;
                staged
                    .write_all(&bytes)
                    .map_err(|_| "cache restore write failed")?;
                staged
                    .as_file()
                    .sync_all()
                    .map_err(|_| "cache restore sync failed")?;
                staged
                    .persist_noclobber(directory.join(name))
                    .map_err(|_| "cache restore atomic target conflict")?;
            }
            Err(_) => return Err("cache restore symlink or target denied".into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_output_inspection_rejects_symlink_ancestors_even_when_leaf_is_absent() {
        let root =
            tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture root: {error:?}"));
        let outside =
            tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture root: {error:?}"));
        let mut policy = config();
        policy.paths = vec!["parent/output".into()];
        assert!(require_absent_outputs(root.path(), &policy).is_ok());
        std::os::unix::fs::symlink(outside.path(), root.path().join("parent"))
            .unwrap_or_else(|error| unreachable!("fixture symlink: {error:?}"));
        assert!(require_absent_outputs(root.path(), &policy).is_err());
        assert!(!outside.path().join("output").exists());
    }
    fn config() -> WorkerCacheProfile {
        WorkerCacheProfile {
            producer: "compiler-v1".into(),
            toolchain: "reviewed-v1".into(),
            platform: "linux-x86_64".into(),
            inputs: vec!["input".into()],
            paths: vec!["cache-output".into()],
            maximum_entry_bytes: 1024,
            maximum_files: 8,
        }
    }
    fn entry(files: Vec<CacheFile>) -> CacheEntry {
        CacheEntry {
            key: CacheKey {
                project_id: "project".into(),
                node_id: "build".into(),
                producer: "compiler-v1".into(),
                toolchain: "reviewed-v1".into(),
                platform: "linux-x86_64".into(),
                inputs: BTreeMap::from([(
                    "input".into(),
                    blake3::hash(b"input").to_hex().to_string(),
                )]),
                paths: vec!["cache-output".into()],
            },
            files,
        }
    }
    fn file(path: &str, bytes: &[u8]) -> CacheFile {
        CacheFile {
            path: path.into(),
            blake3: blake3::hash(bytes).to_hex().to_string(),
            body_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        }
    }
    #[test]
    fn local_transfers_bind_current_source_and_verify_before_writes() {
        let producer =
            tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture root: {error:?}"));
        let consumer =
            tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture root: {error:?}"));
        for root in [producer.path(), consumer.path()] {
            std::fs::write(root.join("input"), b"source")
                .unwrap_or_else(|error| unreachable!("fixture input: {error:?}"));
        }
        std::fs::create_dir(producer.path().join("cache-output"))
            .unwrap_or_else(|error| unreachable!("fixture output: {error:?}"));
        std::fs::write(producer.path().join("cache-output/product"), b"product")
            .unwrap_or_else(|error| unreachable!("fixture product: {error:?}"));
        let key = local_key("project", "job", &config(), producer.path())
            .unwrap_or_else(|error| unreachable!("fixture key: {error:?}"));
        let envelope = collect_local(producer.path(), &key, &config())
            .unwrap_or_else(|error| unreachable!("fixture envelope: {error:?}"));
        std::fs::write(consumer.path().join("input"), b"changed")
            .unwrap_or_else(|error| unreachable!("fixture input: {error:?}"));
        assert!(restore_local(consumer.path(), &envelope, &config()).is_err());
        assert!(!consumer.path().join("cache-output").exists());
        std::fs::write(consumer.path().join("input"), b"source")
            .unwrap_or_else(|error| unreachable!("fixture input: {error:?}"));
        let mut corrupt = envelope.clone();
        corrupt.blake3 = "0".repeat(64);
        assert!(restore_local(consumer.path(), &corrupt, &config()).is_err());
        assert!(!consumer.path().join("cache-output").exists());
        restore_local(consumer.path(), &envelope, &config())
            .unwrap_or_else(|error| unreachable!("verified restoration: {error:?}"));
        assert_eq!(
            std::fs::read(consumer.path().join("cache-output/product"))
                .unwrap_or_else(|error| unreachable!("restored product: {error:?}")),
            b"product"
        );
    }
    #[test]
    fn compatible_workers_share_keys_but_changed_input_bytes_do_not() {
        let first = tempfile::tempdir()
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "first worker checkout", error));
        let second = tempfile::tempdir()
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "second worker checkout", error));
        std::fs::write(first.path().join("input"), b"same input")
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "first input", error));
        std::fs::write(second.path().join("input"), b"same input")
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "second input", error));
        let mut producer: WorkerProfile = serde_json::from_slice(include_bytes!(
            "../../../../fixtures/web-worker/profile.json"
        ))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "reviewed worker profile", error));
        producer.work_root = first.path().join("jobs");
        let mut consumer = producer.clone();
        consumer.work_root = second.path().join("jobs");
        consumer.repository_path = second.path().join("repository");
        consumer.token_file = second.path().join("worker.token");
        let producer_key = local_key(&producer.project_id, "build", &config(), first.path())
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "producer key", error));
        let consumer_key = local_key(&consumer.project_id, "build", &config(), second.path())
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "consumer key", error));
        assert_eq!(producer_key, consumer_key);
        std::fs::write(second.path().join("input"), b"changed input")
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "changed input", error));
        assert_ne!(
            producer_key,
            local_key(&consumer.project_id, "build", &config(), second.path())
                .unwrap_or_else(|error| unreachable!("changed input key: {error}")),
        );
    }
    #[test]
    fn two_independent_checkouts_exchange_verified_data_without_source_or_credential_copy() {
        let producer = tempfile::tempdir().unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        let consumer = tempfile::tempdir().unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        std::fs::create_dir(producer.path().join("cache-output")).unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        std::fs::write(
            producer.path().join("cache-output/object"),
            b"compiler object",
        )
        .unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        std::fs::write(producer.path().join("input"), b"not an output").unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        let mut files = Vec::new();
        let mut bytes = 0;
        let mut visited = 0;
        collect(
            producer.path(),
            Path::new("cache-output"),
            &config(),
            &mut files,
            &mut bytes,
            &mut visited,
        )
        .unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        let entry = entry(files);
        assert_eq!(
            entry
                .verify(1024, 8)
                .unwrap_or_else(|error| unreachable!("verified cache entry: {error}")),
            15
        );
        restore_entry(consumer.path(), &entry).unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        assert_eq!(
            std::fs::read(consumer.path().join("cache-output/object"))
                .unwrap_or_else(|error| unreachable!("restored cache file: {error}")),
            b"compiler object"
        );
        assert!(!consumer.path().join("input").exists());
        restore_entry(consumer.path(), &entry).unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
    }
    #[test]
    fn unsafe_publish_paths_links_and_total_size_are_rejected() {
        let root = tempfile::tempdir().unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        std::fs::create_dir(root.path().join("cache-output")).unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        let outside = tempfile::NamedTempFile::new().unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        std::os::unix::fs::symlink(outside.path(), root.path().join("cache-output/link"))
            .unwrap_or_else(|error| {
                unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
            });
        let mut files = Vec::new();
        let mut bytes = 0;
        let mut visited = 0;
        assert!(
            collect(
                root.path(),
                Path::new("cache-output"),
                &config(),
                &mut files,
                &mut bytes,
                &mut visited
            )
            .is_err()
        );
        std::fs::remove_file(root.path().join("cache-output/link")).unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        std::fs::write(root.path().join("cache-output/.env"), b"credentials").unwrap_or_else(
            |error| unreachable!("{}: {:?}", "cache fixture operation succeeds", error),
        );
        assert!(
            collect(
                root.path(),
                Path::new("cache-output/.env"),
                &config(),
                &mut files,
                &mut bytes,
                &mut visited
            )
            .is_err()
        );
        std::fs::write(root.path().join("cache-output/large"), vec![0u8; 1025]).unwrap_or_else(
            |error| unreachable!("{}: {:?}", "cache fixture operation succeeds", error),
        );
        assert!(
            collect(
                root.path(),
                Path::new("cache-output/large"),
                &config(),
                &mut files,
                &mut bytes,
                &mut visited
            )
            .is_err()
        );
    }
    #[test]
    fn preflight_rejects_later_symlink_or_conflict_before_any_restore_write() {
        let root = tempfile::tempdir().unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        std::fs::create_dir(root.path().join("cache-output")).unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        let outside = tempfile::tempdir().unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        std::os::unix::fs::symlink(outside.path(), root.path().join("cache-output/bad-parent"))
            .unwrap_or_else(|error| {
                unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
            });
        let entry = entry(vec![
            file("cache-output/first", b"valid"),
            file("cache-output/bad-parent/second", b"valid"),
        ]);
        entry.verify(1024, 8).unwrap_or_else(|error| {
            unreachable!("{}: {:?}", "cache fixture operation succeeds", error)
        });
        assert!(restore_entry(root.path(), &entry).is_err());
        assert!(!root.path().join("cache-output/first").exists());
        assert!(!outside.path().join("second").exists());
    }
}
