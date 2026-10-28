use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use super::client::ServiceClient;
use super::profile::{WorkerProfile, component, hex};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartbeatProfile {
    pub worker_id: String,
    pub target_id: String,
    pub interval_ms: u64,
    pub ttl_ms: u64,
    pub slots: usize,
}
impl HeartbeatProfile {
    pub fn admit(&self) -> Result<(), String> {
        if !component(&self.worker_id)
            || !hex(&self.target_id, 32)
            || self.interval_ms < 100
            || self.interval_ms > 30_000
            || self.ttl_ms < self.interval_ms.saturating_mul(3)
            || self.ttl_ms > 120_000
            || self.slots == 0
            || self.slots > 256
        {
            return Err("worker heartbeat profile invalid".into());
        }
        Ok(())
    }
}

pub struct SlotGuard {
    _file: File,
}

fn private_directory(path: &Path) -> Result<(), String> {
    match std::fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(_) => return Err("worker ledger directory unavailable".into()),
    }
    super::files::owned_directory(path)
}

fn private_file(path: &Path) -> Result<File, String> {
    let parent = path.parent().ok_or("worker ledger parent missing")?;
    super::files::owned_directory(parent)?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "worker ledger file unavailable")?;
    let metadata = file
        .metadata()
        .map_err(|_| "worker ledger metadata unavailable")?;
    if !metadata.is_file()
        || metadata.uid() != nix::unistd::geteuid().as_raw()
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
    {
        return Err("worker ledger file must be private owned regular file".into());
    }
    Ok(file)
}

fn ledger(profile: &WorkerProfile) -> Result<Option<(std::path::PathBuf, usize)>, String> {
    let Some(heartbeat) = &profile.heartbeat else {
        return Ok(None);
    };
    heartbeat.admit()?;
    private_directory(&profile.work_root)?;
    let root = profile.work_root.join("worker-slots");
    private_directory(&root)?;
    let root = root.join(&heartbeat.worker_id);
    private_directory(&root)?;
    let cpus = std::thread::available_parallelism()
        .map_err(|_| "observed worker CPU availability unavailable")?
        .get();
    Ok(Some((root, heartbeat.slots.min(cpus))))
}

/// One lock is held for the entire job. Process death releases it in the kernel,
/// so cancellation/restart cannot leave durable invented active capacity.
pub fn acquire_slot(profile: &WorkerProfile) -> Result<Option<SlotGuard>, String> {
    let Some((root, slots)) = ledger(profile)? else {
        return Ok(None);
    };
    for slot in 0..slots {
        let file = private_file(&root.join(slot.to_string()))?;
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(Some(SlotGuard { _file: file })),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(_) => return Err("worker capacity ledger lock failed".into()),
        }
    }
    Err("worker observed capacity exhausted".into())
}

fn capacity(profile: &WorkerProfile) -> Result<(usize, usize), String> {
    let (root, slots) = ledger(profile)?.ok_or("worker heartbeat not configured")?;
    let mut active = 0;
    for slot in 0..slots {
        let file = private_file(&root.join(slot.to_string()))?;
        match file.try_lock_exclusive() {
            Ok(()) => FileExt::unlock(&file).map_err(|_| "worker capacity ledger unlock failed")?,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => active += 1,
            Err(_) => return Err("worker capacity observation failed".into()),
        }
    }
    Ok((slots, active))
}

fn now_ms() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "worker clock invalid")?
        .as_millis()
        .try_into()
        .map_err(|_| "worker clock exceeds bounds".into())
}

pub fn run(profile: &WorkerProfile, cancellation: &AtomicBool) -> Result<(), String> {
    profile.admit()?;
    let heartbeat = profile
        .heartbeat
        .as_ref()
        .ok_or("worker heartbeat not configured")?;
    if let Some(policy) = &profile.execution {
        crate::execution::preflight(policy)?;
    }
    let service = ServiceClient::new(profile)?;
    let (root, _) = ledger(profile)?.ok_or("worker heartbeat not configured")?;
    let owner = private_file(&root.join("heartbeat.lock"))?;
    owner
        .try_lock_exclusive()
        .map_err(|_| "worker heartbeat already has an owner")?;
    let mut random = [0u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut random))
        .map_err(|_| "worker session randomness unavailable")?;
    let session_id = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let session_started_at_ms = now_ms()?;
    let mut sequence = 0u64;
    while !cancellation.load(Ordering::Acquire) {
        let (observed_slots, active_slots) = capacity(profile)?;
        let body = serde_json::json!({"schema":"kiln.worker-heartbeat.v1", "worker_id":heartbeat.worker_id,"project_id":profile.project_id,"target_id":heartbeat.target_id,"session_id":session_id,"session_started_at_ms":session_started_at_ms,"sequence":sequence,"sent_at_ms":now_ms()?,"ttl_ms":heartbeat.ttl_ms,"observed_slots":observed_slots,"active_slots":active_slots});
        // Each attempt is a new replaceable observation. A lost response may
        // already have committed; never reuse its sequence with different facts.
        sequence = sequence
            .checked_add(1)
            .ok_or("worker heartbeat sequence exhausted")?;
        let response = service
            .client
            .post(format!("{}/api/workers/heartbeat", service.origin))
            .header(
                reqwest::header::AUTHORIZATION,
                service.authorization.clone(),
            )
            .header(reqwest::header::ORIGIN, service.origin.as_str())
            .json(&body)
            .send();
        if let Ok(response) = response {
            // The status is the entire acknowledgement contract; no response
            // body is needed. A policy/identity HTTP rejection is terminal,
            // unlike unavailable transport, which leaves the daemon observing.
            if !response.status().is_success() {
                return Err("worker service rejected heartbeat".into());
            }
        }
        let deadline = std::time::Instant::now() + Duration::from_millis(heartbeat.interval_ms);
        while !cancellation.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
            std::thread::sleep(
                Duration::from_millis(25)
                    .min(deadline.saturating_duration_since(std::time::Instant::now())),
            );
        }
    }
    Ok(())
}

pub fn cli_main(mut arguments: impl Iterator<Item = String>) -> i32 {
    let parsed = (
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
    );
    let (Some(a), Some(path), Some(b), Some(identity), None) = parsed else {
        return 2;
    };
    if a != "--profile" || b != "--identity" {
        return 2;
    }
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let mut signals = Vec::new();
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        match signal_hook::flag::register(signal, cancelled.clone()) {
            Ok(id) => signals.push(id),
            Err(_) => {
                for id in signals {
                    signal_hook::low_level::unregister(id);
                }
                return 1;
            }
        }
    }
    let result = super::load_profile(Path::new(&path), &identity)
        .and_then(|profile| run(&profile, &cancelled));
    for id in signals {
        signal_hook::low_level::unregister(id);
    }
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("kiln worker: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ttl_requires_multiple_heartbeat_intervals() {
        let mut p = HeartbeatProfile {
            worker_id: "local".into(),
            target_id: "a".repeat(64),
            interval_ms: 1000,
            ttl_ms: 3000,
            slots: 1,
        };
        assert!(p.admit().is_ok());
        p.ttl_ms = 2999;
        assert!(p.admit().is_err());
    }
}
