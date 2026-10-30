//! Firecracker shell for `kind = "firecracker"` hosted runners.
//!
//! Each job gets its own microVM inside the attempt's systemd unit, with that
//! unit's limits and owned cleanup. The shell builds the read-only source image,
//! spawns `firecracker` (from a config file for a cold boot, or through the API
//! socket for a snapshot restore), serves exactly one guest vsock session, and
//! journals the job result before it stops the unit. Decisions come from the
//! pure `kiln_microvm` core; this module only performs the requested effects.
//!
//! The Firecracker jailer is not used: it needs root for chroot, cgroup and uid
//! changes. Firecracker runs unprivileged in the user-manager unit with its
//! built-in seccomp filters.
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{Scope, ScopedJoinHandle};
use std::time::{Duration, Instant};

use bounded_exec::{ExecutionOutput, Input, RunError};
use fs2::FileExt;
use kiln_microvm::admission::MachinePolicy;
use kiln_microvm::archive::{self, EntryKind, SourceEntry};
use kiln_microvm::lifecycle::{Event, Lifecycle};
use kiln_microvm::plan::{self, VmPaths};
use kiln_microvm::protocol::{
    self, GuestMessage, GuestStep, HostMessage, StepReport, decode_guest, encode, frame_length,
    host_frame,
};
use kiln_microvm::{MIB, archive::BLOCK_BYTES};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::execution::{
    Backend, Config, Image, ProcessFacts, StepObservation, cleanup_unit,
    infrastructure_failure_state, manager_request, step_observation, unit_command,
};
use crate::fleet::Executable;

/// Operator-owned snapshot/restore warm pool for one Firecracker runner.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarmPool {
    pub directory: PathBuf,
    pub slots: u32,
}

/// VMM stdout/stderr capture: serial console plus VMM errors.
const CONSOLE_BYTES: usize = 65_536;
/// Console bytes retained in a failed job's observation.
const CONSOLE_TAIL_BYTES: usize = 4096;
/// Poll interval for listener accept and API socket appearance.
const POLL: Duration = Duration::from_millis(1);
/// Read slice so cancellation and deadlines are observed while waiting.
const IO_SLICE: Duration = Duration::from_millis(20);
/// Bound for one Firecracker API exchange.
const API_TIMEOUT: Duration = Duration::from_secs(10);
const API_RESPONSE_BYTES: usize = 65_536;
/// Linux `sun_path` holds 108 bytes including the terminating NUL.
const MAX_SOCKET_PATH_BYTES: usize = 107;
/// The longest socket path below a pool directory: `/slot-NN/v.sock_10240`.
const POOL_SOCKET_SUFFIX_BYTES: usize = 22;
/// Bounded-exec deadline beyond the unit's own `RuntimeMaxSec`.
const VMM_EXEC_MARGIN_MS: u64 = 5_000;

/// A Firecracker runner's reviewed images and machine policy.
pub(crate) struct Runner<'a> {
    pub(crate) firecracker: &'a Executable,
    pub(crate) kernel: &'a Image,
    pub(crate) initrd: &'a Image,
    pub(crate) policy: MachinePolicy<'a>,
    pub(crate) warm_pool: Option<&'a WarmPool>,
}

impl<'a> Runner<'a> {
    pub(crate) fn from_backend(backend: &'a Backend) -> Option<Self> {
        let Backend::Firecracker {
            firecracker,
            kernel,
            initrd,
            warm_pool,
            ..
        } = backend
        else {
            return None;
        };
        Some(Self {
            firecracker,
            kernel,
            initrd,
            policy: machine_policy(backend)?,
            warm_pool: warm_pool.as_ref(),
        })
    }
}

pub(crate) fn machine_policy(backend: &Backend) -> Option<MachinePolicy<'_>> {
    match backend {
        Backend::Firecracker {
            guest_shell,
            vcpus,
            memory_mib,
            boot_timeout_ms,
            source_image_mib,
            warm_pool,
            ..
        } => Some(MachinePolicy {
            vcpus: *vcpus,
            memory_mib: *memory_mib,
            boot_timeout_ms: *boot_timeout_ms,
            source_image_mib: *source_image_mib,
            warm_slots: warm_pool.as_ref().map(|pool| pool.slots),
            guest_shell,
        }),
        _ => None,
    }
}

// r[impl kiln.hosted.microvm.warm_pool]
/// Pure pool path admission: absolute, normalized, short enough for every slot
/// socket, and disjoint from customer source and the immutable store.
pub(crate) fn admit_pool_path(directory: &Path, work_root: &Path) -> Result<(), String> {
    let normalized = directory.is_absolute()
        && directory.components().all(|c| {
            matches!(
                c,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        });
    let length = directory.as_os_str().len();
    if !normalized
        || directory == Path::new("/")
        || directory.starts_with("/nix/store")
        || directory.starts_with(work_root)
        || work_root.starts_with(directory)
        || length + POOL_SOCKET_SUFFIX_BYTES > MAX_SOCKET_PATH_BYTES
        || directory.to_str().is_none()
    {
        return Err(
            "warm pool directory must be a short private path outside work_root and /nix/store"
                .into(),
        );
    }
    Ok(())
}

/// Shell check: the pool directory exists, is a real directory owned by this
/// service user and is private (0700).
pub(crate) fn verify_pool_directory(directory: &Path) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(directory).map_err(|_| "warm pool directory unavailable")?;
    let owner = fs::metadata("/proc/self")
        .map_err(|_| "process owner unavailable")?
        .uid();
    if !metadata.is_dir() || metadata.uid() != owner || metadata.mode() & 0o777 != 0o700 {
        return Err(
            "warm pool directory must be a private 0700 directory owned by the service user".into(),
        );
    }
    Ok(())
}

/// One job to run in its own microVM.
pub(crate) struct Job<'a> {
    pub(crate) organization: &'a str,
    pub(crate) attempt: &'a str,
    pub(crate) job: &'a str,
    pub(crate) workspace: &'a Path,
    pub(crate) steps: &'a [(String, GuestStep)],
    pub(crate) unit: &'a str,
    pub(crate) timeout_ms: u64,
}

pub(crate) struct JobOutcome {
    pub(crate) steps: Vec<StepObservation>,
    pub(crate) vm: Value,
}

type Vmm<'scope> = ScopedJoinHandle<'scope, Result<ExecutionOutput, RunError>>;

fn millis(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn text(path: &Path) -> Result<&str, String> {
    let text = path.to_str().ok_or("microVM path is not UTF-8")?;
    if text.len() > 4096 || text.contains(',') {
        return Err("microVM path rejected".into());
    }
    Ok(text)
}

// r[impl kiln.hosted.microvm.lifecycle]
// r[impl kiln.hosted.microvm.lifecycle.scenario.journal]
/// Durable per-job VM record. The row exists before any VM starts, holds the
/// collected result before the unit is stopped, and finally records whether
/// destruction was observed.
fn journal(
    db: &Connection,
    job: &Job<'_>,
    lifecycle: &str,
    result: Option<&Value>,
) -> Result<(), String> {
    let result = result.map(Value::to_string);
    if lifecycle == "starting" {
        db.execute(
            "INSERT INTO hosted_microvm_job VALUES(?1,?2,?3,?4,NULL)",
            params![job.organization, job.attempt, job.job, lifecycle],
        )
    } else {
        db.execute(
            "UPDATE hosted_microvm_job SET lifecycle=?4,result=COALESCE(?5,result) WHERE org=?1 AND attempt=?2 AND job=?3",
            params![job.organization, job.attempt, job.job, lifecycle, result],
        )
    }
    .map_err(|e| e.to_string())
    .and_then(|rows| {
        if rows == 1 {
            Ok(())
        } else {
            Err("microVM journal row missing".into())
        }
    })
}

// r[impl kiln.hosted.microvm.plan]
/// Write the bounded deterministic source archive into a fresh private image
/// of exactly `capacity` bytes. Symlinks are archived, never followed.
fn source_image(workspace: &Path, image: &Path, capacity: u64) -> Result<u64, String> {
    let mut entries = Vec::new();
    let mut pending = vec![(workspace.to_path_buf(), String::new())];
    while let Some((directory, prefix)) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|_| "source snapshot read failed")? {
            if entries.len() >= archive::MAX_SOURCE_ENTRIES {
                return Err("source snapshot entry count exceeds bound".into());
            }
            let entry = entry.map_err(|_| "source snapshot read failed")?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "source snapshot path is not UTF-8")?;
            let path = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| "source snapshot stat failed")?;
            let kind = if metadata.file_type().is_symlink() {
                EntryKind::Symlink {
                    target: fs::read_link(entry.path())
                        .map_err(|_| "source snapshot link read failed")?
                        .into_os_string()
                        .into_string()
                        .map_err(|_| "source snapshot link is not UTF-8")?,
                }
            } else if metadata.is_dir() {
                pending.push((entry.path(), path.clone()));
                EntryKind::Directory
            } else if metadata.is_file() {
                EntryKind::File {
                    size: metadata.len(),
                    executable: metadata.mode() & 0o111 != 0,
                }
            } else {
                return Err("source snapshot contains an unsupported special file".into());
            };
            entries.push(SourceEntry { path, kind });
        }
    }
    let (entries, total) = archive::plan_archive(entries, capacity)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(image)
        .map_err(|_| "source image create failed")?;
    let mut writer = BufWriter::new(&file);
    let write_error = |_| "source image write failed".to_owned();
    for entry in &entries {
        writer
            .write_all(&archive::header(entry)?)
            .map_err(write_error)?;
        if let EntryKind::File { size, .. } = entry.kind {
            let mut source = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(workspace.join(&entry.path))
                .map_err(|_| "source snapshot file open failed")?;
            let metadata = source
                .metadata()
                .map_err(|_| "source snapshot stat failed")?;
            if !metadata.is_file() || metadata.len() != size {
                return Err("source changed while building the snapshot".into());
            }
            let copied =
                std::io::copy(&mut (&mut source).take(size), &mut writer).map_err(write_error)?;
            if copied != size {
                return Err("source changed while building the snapshot".into());
            }
            let padding = (BLOCK_BYTES - size % BLOCK_BYTES) % BLOCK_BYTES;
            writer
                .write_all(&vec![0; usize::try_from(padding).unwrap_or(0)])
                .map_err(write_error)?;
        }
    }
    writer
        .write_all(&[0; 1024])
        .and_then(|()| writer.flush())
        .map_err(write_error)?;
    drop(writer);
    file.set_len(capacity)
        .map_err(|_| "source image size failed")?;
    Ok(total)
}

fn spawn_vmm<'scope>(
    scope: &'scope Scope<'scope, '_>,
    config: &'scope Config,
    unit: &str,
    timeout_ms: u64,
    arguments: Vec<String>,
    stop: &'scope AtomicBool,
) -> Result<Vmm<'scope>, String> {
    let mut service = unit_command(config, unit, timeout_ms, true)?;
    service.extend(arguments.into_iter().map(Into::into));
    let request = manager_request(
        config,
        config.systemd_run.verify()?,
        service,
        timeout_ms.saturating_add(VMM_EXEC_MARGIN_MS),
        CONSOLE_BYTES,
        Input::Null,
    )?;
    Ok(scope.spawn(move || bounded_exec::run_with_cancellation(request, stop)))
}

/// Accept the guest agent's single connection, then remove the listener so
/// no later guest-initiated vsock connection can reach the host.
fn accept_guest(
    listener: UnixListener,
    listener_path: &Path,
    deadline: Instant,
    vmm: &Vmm<'_>,
    cancellation: &AtomicBool,
) -> Result<UnixStream, String> {
    listener
        .set_nonblocking(true)
        .map_err(|_| "guest listener setup failed")?;
    let accepted = loop {
        match listener.accept() {
            Ok((stream, _)) => break Ok(stream),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(_) => break Err("guest listener failed".to_owned()),
        }
        if cancellation.load(Ordering::Acquire) {
            break Err("cancelled before the guest connected".into());
        }
        if vmm.is_finished() {
            break Err("VMM exited before the guest connected".into());
        }
        if Instant::now() >= deadline {
            break Err("guest boot deadline elapsed".into());
        }
        std::thread::sleep(POLL);
    };
    drop(listener);
    let _ = fs::remove_file(listener_path);
    let stream = accepted?;
    stream
        .set_nonblocking(false)
        .and_then(|()| stream.set_read_timeout(Some(IO_SLICE)))
        .map_err(|_| "guest stream setup failed")?;
    Ok(stream)
}

fn read_exact_until(
    stream: &mut UnixStream,
    buffer: &mut [u8],
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<(), String> {
    let mut filled = 0;
    while filled < buffer.len() {
        if cancellation.load(Ordering::Acquire) {
            return Err("cancelled".into());
        }
        if Instant::now() >= deadline {
            return Err("job deadline elapsed in the guest".into());
        }
        match stream.read(buffer.get_mut(filled..).unwrap_or_default()) {
            Ok(0) => return Err("guest closed the session".into()),
            Ok(count) => filled += count,
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return Err("guest session read failed".into()),
        }
    }
    Ok(())
}

fn read_guest(
    stream: &mut UnixStream,
    deadline: Instant,
    cancellation: &AtomicBool,
) -> Result<GuestMessage, String> {
    let mut prefix = [0_u8; 4];
    read_exact_until(stream, &mut prefix, deadline, cancellation)?;
    let mut body = vec![0_u8; frame_length(prefix)?];
    read_exact_until(stream, &mut body, deadline, cancellation)?;
    Ok(decode_guest(&body)?.message)
}

fn write_host(stream: &mut UnixStream, message: HostMessage) -> Result<(), String> {
    stream
        .write_all(&encode(&host_frame(message))?)
        .map_err(|_| "guest session write failed".into())
}

/// One request on the Firecracker API socket, waiting for the socket to appear.
fn api(socket: &Path, method: &str, path: &str, body: &Value, vmm: &Vmm<'_>) -> Result<(), String> {
    let deadline = Instant::now() + API_TIMEOUT;
    let mut stream = loop {
        match UnixStream::connect(socket) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline && !vmm.is_finished() => std::thread::sleep(POLL),
            Err(_) => return Err("Firecracker API socket unavailable".into()),
        }
    };
    stream
        .set_read_timeout(Some(API_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(API_TIMEOUT)))
        .and_then(|()| stream.write_all(&plan::api_request(method, path, body)))
        .map_err(|_| "Firecracker API write failed")?;
    let mut response = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        if plan::api_response_length(&response).is_some_and(|length| response.len() >= length) {
            break;
        }
        let count = stream
            .read(&mut buffer)
            .map_err(|_| "Firecracker API read failed")?;
        if count == 0 {
            break;
        }
        response.extend_from_slice(buffer.get(..count).unwrap_or_default());
        if response.len() > API_RESPONSE_BYTES {
            return Err("Firecracker API response exceeds bound".into());
        }
    }
    match plan::api_status(&response) {
        Some(status) if (200..300).contains(&status) => Ok(()),
        Some(status) => Err(format!(
            "Firecracker API {method} {path} returned {status}: {}",
            String::from_utf8_lossy(&response)
                .chars()
                .take(512)
                .collect::<String>()
        )),
        None => Err("malformed Firecracker API response".into()),
    }
}

fn remove_stale(paths: &[&Path]) -> Result<(), String> {
    for path in paths {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err("stale microVM socket could not be removed".into()),
        }
    }
    Ok(())
}

fn private_directory(path: &Path) -> Result<(), String> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => verify_pool_directory(path),
        Err(_) => Err("warm pool slot directory create failed".into()),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotManifest {
    identity: String,
    state_blake3: String,
    memory_bytes: u64,
}

struct Slot {
    index: u32,
    directory: PathBuf,
    _lock: File,
}

impl Slot {
    fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }
}

/// Exclusive lock on the first free slot; `None` when every slot is busy.
fn acquire_slot(pool: &WarmPool) -> Result<Option<Slot>, String> {
    verify_pool_directory(&pool.directory)?;
    for index in 0..pool.slots {
        let directory = pool.directory.join(format!("slot-{index}"));
        private_directory(&directory)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(directory.join("lock"))
            .map_err(|_| "warm pool slot lock unavailable")?;
        if lock.try_lock_exclusive().is_ok() {
            return Ok(Some(Slot {
                index,
                directory,
                _lock: lock,
            }));
        }
    }
    Ok(None)
}

fn blake3_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|_| "snapshot file unavailable")?;
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut file, &mut hasher).map_err(|_| "snapshot file read failed")?;
    Ok(hasher.finalize().to_hex().to_string())
}

fn snapshot_identity(runner: &Runner<'_>, slot: &Slot) -> Result<String, String> {
    Ok(plan::snapshot_identity(&plan::SnapshotInputs {
        firecracker_blake3: &runner.firecracker.blake3,
        kernel_blake3: &runner.kernel.blake3,
        initrd_blake3: &runner.initrd.blake3,
        vcpus: runner.policy.vcpus,
        memory_mib: runner.policy.memory_mib,
        source_image_mib: runner.policy.source_image_mib,
        slot: slot.index,
        slot_directory: text(&slot.directory)?,
    }))
}

/// A slot snapshot is reusable only for the exact identity and state bytes.
fn snapshot_reusable(slot: &Slot, identity: &str, memory_bytes: u64) -> bool {
    let Ok(raw) = fs::read(slot.path("manifest.json")) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_slice::<SlotManifest>(&raw) else {
        return false;
    };
    manifest.identity == identity
        && manifest.memory_bytes == memory_bytes
        && fs::metadata(slot.path("memory")).is_ok_and(|m| m.len() == memory_bytes)
        && blake3_file(&slot.path("state")).is_ok_and(|hash| hash == manifest.state_blake3)
}

fn invalidate(slot: &Slot) {
    let _ = fs::remove_file(slot.path("manifest.json"));
}

struct Paths {
    kernel: PathBuf,
    initrd: PathBuf,
    firecracker: PathBuf,
}

fn vm_config(
    paths: &Paths,
    image: &Path,
    vsock: &Path,
    runner: &Runner<'_>,
) -> Result<Vec<u8>, String> {
    let document = plan::vm_config(
        &VmPaths {
            kernel: text(&paths.kernel)?,
            initrd: text(&paths.initrd)?,
            source_image: text(image)?,
            vsock: text(vsock)?,
        },
        &runner.policy,
    );
    serde_json::to_vec(&document).map_err(|e| e.to_string())
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|_| "microVM private file write failed".into())
}

/// Stop the unit holding the VMM, then reap the launcher thread.
fn destroy(config: &Config, unit: &str, vmm: Vmm<'_>, stop: &AtomicBool) -> (bool, String) {
    let cleaned = cleanup_unit(config, unit);
    stop.store(true, Ordering::Release);
    let console = match vmm.join() {
        Ok(Ok(output)) => String::from_utf8_lossy(&output.stdout.bytes).into_owned(),
        Ok(Err(error)) => error.to_string(),
        Err(_) => "VMM launcher thread panicked".into(),
    };
    (cleaned, console)
}

fn tail(console: &str) -> String {
    let skip = console.chars().count().saturating_sub(CONSOLE_TAIL_BYTES);
    console.chars().skip(skip).collect()
}

// r[impl kiln.hosted.microvm.warm_pool]
/// Cold boot into the slot paths with a blank source drive, wait for the
/// agent, pause, take a full snapshot and destroy that VM.
fn build_snapshot(
    config: &Config,
    runner: &Runner<'_>,
    paths: &Paths,
    slot: &Slot,
    identity: &str,
    job: &Job<'_>,
    cancellation: &AtomicBool,
) -> Result<(), String> {
    invalidate(slot);
    let vsock = slot.path("v.sock");
    let listener_path = PathBuf::from(plan::host_listener_path(text(&vsock)?));
    let api_socket = slot.path("api.sock");
    remove_stale(&[&vsock, &listener_path, &api_socket])?;
    let image = slot.path("source.img");
    let blank = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&image)
        .map_err(|_| "warm pool source image create failed")?;
    blank
        .set_len(u64::from(runner.policy.source_image_mib) * MIB)
        .map_err(|_| "warm pool source image size failed")?;
    let config_path = slot.path("vm.json");
    write_private(&config_path, &vm_config(paths, &image, &vsock, runner)?)?;
    let listener = UnixListener::bind(&listener_path).map_err(|_| "guest listener bind failed")?;
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let vmm = spawn_vmm(
            scope,
            config,
            job.unit,
            job.timeout_ms,
            vec![
                text(&paths.firecracker)?.into(),
                "--api-sock".into(),
                text(&api_socket)?.into(),
                "--config-file".into(),
                text(&config_path)?.into(),
                "--level".into(),
                "Error".into(),
            ],
            &stop,
        )?;
        let deadline = Instant::now() + Duration::from_millis(runner.policy.boot_timeout_ms);
        let built = (|| {
            let mut stream = accept_guest(listener, &listener_path, deadline, &vmm, cancellation)?;
            write_host(
                &mut stream,
                HostMessage::Ping {
                    nonce: identity.into(),
                },
            )?;
            match read_guest(&mut stream, deadline, cancellation)? {
                GuestMessage::Pong { nonce } if nonce == identity => {}
                _ => return Err("guest did not answer the snapshot readiness probe".to_owned()),
            }
            drop(stream);
            api(&api_socket, "PATCH", "/vm", &plan::pause(), &vmm)?;
            api(
                &api_socket,
                "PUT",
                "/snapshot/create",
                &plan::snapshot_create(
                    text(&slot.path("state.tmp"))?,
                    text(&slot.path("memory.tmp"))?,
                ),
                &vmm,
            )
        })();
        let (cleaned, console) = destroy(config, job.unit, vmm, &stop);
        if !cleaned {
            return Err("snapshot VM teardown uncertain".to_owned());
        }
        built.map_err(|error| format!("{error}; console: {}", tail(&console)))
    })?;
    fs::rename(slot.path("state.tmp"), slot.path("state"))
        .and_then(|()| fs::rename(slot.path("memory.tmp"), slot.path("memory")))
        .map_err(|_| "snapshot files could not be installed")?;
    let manifest = SlotManifest {
        identity: identity.into(),
        state_blake3: blake3_file(&slot.path("state"))?,
        memory_bytes: u64::from(runner.policy.memory_mib) * MIB,
    };
    write_private(
        &slot.path("manifest.json"),
        &serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
    )
}

#[derive(Default)]
struct Session {
    reports: Vec<StepReport>,
    failure: Option<String>,
    admission_error: Option<String>,
    ready_ms: Option<u64>,
    restore_api_ms: Option<u64>,
    session_ms: Option<u64>,
}

/// The not-yet-connected guest of a VM that is starting or restoring.
struct PendingGuest<'a> {
    listener: UnixListener,
    listener_path: &'a Path,
    spawned: Instant,
    boot_deadline: Instant,
    job_deadline: Instant,
}

/// Serve the guest session of a VM that is starting or restoring.
fn drive_guest(
    session: &mut Session,
    lifecycle: &mut Lifecycle,
    pending: PendingGuest<'_>,
    vmm: &Vmm<'_>,
    job: &Job<'_>,
    cancellation: &AtomicBool,
) -> Result<(), String> {
    let PendingGuest {
        listener,
        listener_path,
        spawned,
        boot_deadline,
        job_deadline,
    } = pending;
    let mut stream = accept_guest(listener, listener_path, boot_deadline, vmm, cancellation)?;
    session.ready_ms = Some(millis(spawned));
    lifecycle.advance(Event::GuestConnected)?;
    let mut entropy = [0_u8; protocol::ENTROPY_BYTES];
    File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut entropy))
        .map_err(|_| "host entropy unavailable")?;
    let started = Instant::now();
    write_host(
        &mut stream,
        HostMessage::Run {
            nonce: job.job.into(),
            clock_unix_ms: crate::now_ms()?,
            entropy_hex: protocol::hex(&entropy),
            deadline_ms: u64::try_from(
                job_deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis(),
            )
            .unwrap_or(u64::MAX),
            steps: job.steps.iter().map(|(_, step)| step.clone()).collect(),
        },
    )?;
    loop {
        match read_guest(&mut stream, job_deadline, cancellation)? {
            GuestMessage::Step(report) => {
                if report.index != session.reports.len() {
                    return Err("guest reported steps out of order".into());
                }
                lifecycle.advance(Event::StepReported)?;
                session.reports.push(report);
            }
            GuestMessage::Done {
                nonce,
                admission_error,
            } if nonce == job.job => {
                lifecycle.advance(Event::GuestDone)?;
                session.admission_error = admission_error;
                session.session_ms = Some(millis(started));
                return Ok(());
            }
            _ => return Err("unexpected guest message".into()),
        }
    }
}

// r[impl kiln.hosted.microvm.lifecycle]
// r[impl kiln.hosted.microvm.lifecycle.scenario.journal]
/// Run one job in its own microVM. Never returns before the VM's unit has been
/// stopped; an unproven teardown leaves every observation uncleaned.
pub(crate) fn run_job(
    db: &Connection,
    config: &Config,
    runner: &Runner<'_>,
    job: &Job<'_>,
    cancellation: &AtomicBool,
) -> JobOutcome {
    let started = Instant::now();
    let job_deadline = started + Duration::from_millis(job.timeout_ms);
    let mut lifecycle = Lifecycle::new(job.steps.len());
    let mut session = Session::default();
    // r[impl kiln.hosted.microvm.claims]
    let mut vm = json!({
        "backend": "firecracker",
        "vcpus": runner.policy.vcpus,
        "memory_mib": runner.policy.memory_mib,
        "network": "none",
        "jailer": false,
        "hostile_isolation_claim": false,
    });
    let mut cleaned = true;
    let mut journaled = false;
    let mut console = String::new();
    let mut observe = |key: &str, value: Value| {
        if let Some(fields) = vm.as_object_mut() {
            fields.insert(key.into(), value);
        }
    };
    if let Err(error) = journal(db, job, "starting", None) {
        session.failure = Some(error);
    } else {
        let outcome = (|| -> Result<(), String> {
            let verify_started = Instant::now();
            let paths = Paths {
                firecracker: runner.firecracker.verify()?,
                kernel: runner.kernel.verify()?,
                initrd: runner.initrd.verify()?,
            };
            observe("verify_ms", json!(millis(verify_started)));
            let scratch = tempfile::Builder::new()
                .prefix("kiln-hosted-vm-")
                .tempdir()
                .map_err(|_| "microVM scratch directory unavailable")?;
            fs::set_permissions(scratch.path(), fs::Permissions::from_mode(0o700))
                .map_err(|_| "microVM scratch directory permissions failed")?;
            let capacity = u64::from(runner.policy.source_image_mib) * MIB;
            let mut slot = match runner.warm_pool {
                Some(pool) => acquire_slot(pool)?,
                None => None,
            };
            observe(
                "warm_pool",
                json!(match (runner.warm_pool, &slot) {
                    (None, _) => "disabled",
                    (Some(_), None) => "exhausted",
                    (Some(_), Some(_)) => "slot",
                }),
            );
            if let Some(held) = &slot {
                observe("slot", json!(held.index));
                let identity = snapshot_identity(runner, held)?;
                if snapshot_reusable(held, &identity, u64::from(runner.policy.memory_mib) * MIB) {
                    observe("snapshot", json!("reused"));
                } else {
                    let build_started = Instant::now();
                    match build_snapshot(config, runner, &paths, held, &identity, job, cancellation)
                    {
                        Ok(()) => {
                            observe("snapshot", json!("rebuilt"));
                            observe("snapshot_build_ms", json!(millis(build_started)));
                        }
                        Err(error) if error.contains("teardown uncertain") => {
                            cleaned = false;
                            return Err(error);
                        }
                        Err(error) => {
                            // Fail closed to a cold boot; the slot stays invalid.
                            observe("snapshot", json!({"rebuild_failed": error}));
                            slot = None;
                        }
                    }
                }
            }
            let image_started = Instant::now();
            let (image, vsock, api_socket) = match &slot {
                Some(held) => (
                    held.path("source.img.next"),
                    held.path("v.sock"),
                    held.path("api.sock"),
                ),
                None => (
                    scratch.path().join("source.img"),
                    scratch.path().join("v.sock"),
                    scratch.path().join("api.sock"),
                ),
            };
            remove_stale(&[&image])?;
            let archive_bytes = source_image(job.workspace, &image, capacity)?;
            observe("source_image_ms", json!(millis(image_started)));
            observe("source_archive_bytes", json!(archive_bytes));
            let listener_path = PathBuf::from(plan::host_listener_path(text(&vsock)?));
            remove_stale(&[&vsock, &listener_path, &api_socket])?;
            let mut arguments = vec![text(&paths.firecracker)?.to_owned()];
            if let Some(held) = &slot {
                fs::rename(&image, held.path("source.img"))
                    .map_err(|_| "warm pool source image install failed")?;
                arguments.extend(["--api-sock".into(), text(&api_socket)?.into()]);
            } else {
                let config_path = scratch.path().join("vm.json");
                write_private(&config_path, &vm_config(&paths, &image, &vsock, runner)?)?;
                arguments.extend([
                    "--no-api".into(),
                    "--config-file".into(),
                    text(&config_path)?.into(),
                ]);
            }
            arguments.extend(["--level".into(), "Error".into()]);
            let listener =
                UnixListener::bind(&listener_path).map_err(|_| "guest listener bind failed")?;
            let stop = AtomicBool::new(false);
            std::thread::scope(|scope| -> Result<(), String> {
                let spawned = Instant::now();
                let vmm = spawn_vmm(scope, config, job.unit, job.timeout_ms, arguments, &stop)?;
                lifecycle.advance(Event::Spawned)?;
                let served = (|| -> Result<(), String> {
                    if let Some(held) = &slot {
                        api(
                            &api_socket,
                            "PUT",
                            "/snapshot/load",
                            &plan::snapshot_load(
                                text(&held.path("state"))?,
                                text(&held.path("memory"))?,
                            ),
                            &vmm,
                        )
                        .inspect_err(|_| invalidate(held))?;
                        session.restore_api_ms = Some(millis(spawned));
                    }
                    let boot_deadline = (spawned
                        + Duration::from_millis(runner.policy.boot_timeout_ms))
                    .min(job_deadline);
                    drive_guest(
                        &mut session,
                        &mut lifecycle,
                        PendingGuest {
                            listener,
                            listener_path: &listener_path,
                            spawned,
                            boot_deadline,
                            job_deadline,
                        },
                        &vmm,
                        job,
                        cancellation,
                    )
                })();
                if let Err(error) = served {
                    session.failure = Some(error);
                    lifecycle.advance(Event::Failed)?;
                }
                // Journal before destruction; the VM is stopped either way.
                let record = json!({
                    "reports": session.reports,
                    "failure": session.failure,
                    "admission_error": session.admission_error,
                });
                match journal(db, job, "journaled", Some(&record)) {
                    Ok(()) => {
                        journaled = true;
                        lifecycle.advance(Event::Journaled)?;
                    }
                    Err(error) => {
                        session.failure.get_or_insert(error);
                        lifecycle.advance(Event::JournalFailed)?;
                    }
                }
                let (clean, output) = destroy(config, job.unit, vmm, &stop);
                cleaned = clean;
                console = output;
                lifecycle.advance(Event::DestroyObserved { clean })?;
                Ok(())
            })?;
            observe(
                "boot",
                json!(if slot.is_some() { "restore" } else { "cold" }),
            );
            if !cleaned {
                // The scratch directory may still be referenced by the VMM.
                let _retained = scratch.keep();
            }
            Ok(())
        })();
        if let Err(error) = outcome {
            session.failure.get_or_insert(error);
        }
        let final_state = if cleaned {
            "destroyed"
        } else {
            "destroy_uncertain"
        };
        if journal(db, job, final_state, None).is_err() {
            journaled = false;
        }
    }
    observe("ready_ms", json!(session.ready_ms));
    observe("restore_api_ms", json!(session.restore_api_ms));
    observe("session_ms", json!(session.session_ms));
    observe("job_ms", json!(millis(started)));
    observe("lifecycle", json!(format!("{:?}", lifecycle.state)));
    observe("journaled", json!(journaled));
    observe("destroyed", json!(cleaned));
    if session.failure.is_some() || session.admission_error.is_some() {
        observe("failure", json!(session.failure));
        observe("admission_error", json!(session.admission_error));
        observe("console_tail", json!(tail(&console)));
    }
    JobOutcome {
        steps: observations(job, &session, cleaned, cancellation),
        vm,
    }
}

fn observations(
    job: &Job<'_>,
    session: &Session,
    cleaned: bool,
    cancellation: &AtomicBool,
) -> Vec<StepObservation> {
    observations_for_reports(
        job.steps,
        &session.reports,
        session
            .failure
            .as_deref()
            .or(session.admission_error.as_deref()),
        cleaned,
        cancellation.load(Ordering::Acquire),
    )
}

// r[impl kiln.hosted.microvm.equivalence]
/// Map guest reports through the same step receipt as every other backend.
/// An interrupted or refused job adds one infrastructure observation for the
/// step that was in flight.
pub(crate) fn observations_for_reports(
    steps: &[(String, GuestStep)],
    reports: &[StepReport],
    interruption: Option<&str>,
    cleaned: bool,
    cancel_requested: bool,
) -> Vec<StepObservation> {
    let mut observations: Vec<StepObservation> = reports
        .iter()
        .zip(steps)
        .map(|(report, (name, _))| {
            step_observation(
                ProcessFacts {
                    name,
                    exited: !report.timed_out && report.exit_code.is_some(),
                    timed_out: report.timed_out,
                    cancelled: false,
                    exit_code: report.exit_code,
                    signal: report.signal,
                    stdout: report.stdout.clone(),
                    stderr: report.stderr.clone(),
                    stdout_truncated: report.stdout_truncated,
                    stderr_truncated: report.stderr_truncated,
                    elapsed_ms: report.elapsed_ms,
                    cleaned,
                },
                true,
                None,
            )
        })
        .collect();
    if let (Some(reason), Some((name, _))) = (interruption, steps.get(reports.len())) {
        let (state, cancelled) = infrastructure_failure_state(cancel_requested, cleaned);
        observations.push(StepObservation {
            success: false,
            cancelled,
            cleaned,
            value: json!({"name":name,"state":state,"reason":reason,"cleaned":cleaned,"elapsed_ms":null}),
        });
    }
    observations
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify kiln.hosted.microvm.warm_pool]
    #[test]
    fn pool_paths_must_be_short_private_and_disjoint_from_source() {
        let work = Path::new("/srv/kiln/work");
        assert_eq!(admit_pool_path(Path::new("/srv/kiln/pool"), work), Ok(()));
        for rejected in [
            "relative/pool",
            "/",
            "/nix/store/pool",
            "/srv/kiln/work/pool",
            "/srv/kiln",
            "/srv/kiln/../pool",
        ] {
            assert!(
                admit_pool_path(Path::new(rejected), work).is_err(),
                "{rejected}"
            );
        }
        let long = format!(
            "/{}",
            "p".repeat(MAX_SOCKET_PATH_BYTES - POOL_SOCKET_SUFFIX_BYTES)
        );
        assert!(admit_pool_path(Path::new(&long), work).is_err());
        let fits = format!(
            "/{}",
            "p".repeat(MAX_SOCKET_PATH_BYTES - POOL_SOCKET_SUFFIX_BYTES - 1)
        );
        assert_eq!(admit_pool_path(Path::new(&fits), work), Ok(()));
        assert!(
            format!("{fits}/slot-15/v.sock_{}", kiln_microvm::AGENT_PORT).len()
                <= MAX_SOCKET_PATH_BYTES
        );
    }

    #[test]
    fn source_image_is_fixed_size_deterministic_and_never_follows_symlinks() {
        let root = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
        let source = root.path().join("source");
        fs::create_dir_all(source.join("nested")).unwrap_or_else(|e| unreachable!("{e}"));
        fs::write(source.join("nested/file.txt"), b"hello").unwrap_or_else(|e| unreachable!("{e}"));
        fs::write(root.path().join("secret"), b"host secret")
            .unwrap_or_else(|e| unreachable!("{e}"));
        std::os::unix::fs::symlink(root.path().join("secret"), source.join("link"))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let first = root.path().join("first.img");
        let second = root.path().join("second.img");
        let used = source_image(&source, &first, MIB).unwrap_or_else(|e| unreachable!("{e}"));
        source_image(&source, &second, MIB).unwrap_or_else(|e| unreachable!("{e}"));
        let bytes = fs::read(&first).unwrap_or_default();
        assert_eq!(bytes.len() as u64, MIB);
        assert_eq!(bytes, fs::read(&second).unwrap_or_default());
        assert!(used < MIB);
        let haystack = String::from_utf8_lossy(&bytes);
        assert!(haystack.contains("hello"));
        assert!(
            !haystack.contains("host secret"),
            "symlink target content leaked"
        );
        assert!(
            source_image(&source, &first, MIB).is_err(),
            "never overwrites"
        );
        assert!(source_image(&source, &root.path().join("tiny.img"), 2048).is_err());
    }
}
