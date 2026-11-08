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
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{Scope, ScopedJoinHandle};
use std::time::{Duration, Instant};

use bounded_exec::{Completion, ExecutionOutput, Input, RunError};
use fs2::FileExt;
use kiln_microvm::admission::MachinePolicy;
use kiln_microvm::archive::{self, EntryKind, SourceEntry};
use kiln_microvm::backend::{
    ColdBootPaths, VmmBackend, cloud_hypervisor_cold_boot, cloud_hypervisor_restore,
    cloud_hypervisor_snapshot_requests,
};
use kiln_microvm::confinement::{
    ConfinementFacts, ExpectedProcess, MAX_STATUS_BYTES, admit_status,
};
use kiln_microvm::debug_hold::{self as hold_policy, DebugHoldPolicy, Grants, HoldFacts};
use kiln_microvm::host::HostFacts;
use kiln_microvm::lifecycle::{Event, Lifecycle, State};
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
const MAX_HOST_FACT_BYTES: usize = 4096;

fn read_bounded_text(path: &str) -> Option<String> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take((MAX_HOST_FACT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_HOST_FACT_BYTES {
        return None;
    }
    String::from_utf8(bytes)
        .ok()
        .map(|text| text.trim().to_owned())
}

fn parse_sysctl(path: &str) -> Option<bool> {
    match read_bounded_text(path)?.as_str() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn host_package() -> String {
    if let Ok(package) = std::env::var("KILN_HOST_KERNEL_PACKAGE") {
        return format!("operator-declared:{package}");
    }
    // NixOS's booted kernel store path is an exact package identity. An OS
    // BUILD_ID identifies the distribution, not its booted kernel package.
    fs::canonicalize("/run/booted-system/kernel")
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".into())
}

fn mptcp_listener_count() -> Option<u32> {
    use bounded_exec::{
        CommandSpec, EnvironmentMode, ExecutionLimits, OutcomePolicy, RunRequest, TerminationScope,
    };
    let program = std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join("ss"))
        .find(|candidate| candidate.is_file())?;
    let output = bounded_exec::run(RunRequest {
        command: CommandSpec {
            program,
            args: ["-M", "-l", "-n", "-p", "-H"]
                .into_iter()
                .map(OsString::from)
                .collect(),
            current_dir: PathBuf::from("/"),
            environment_mode: EnvironmentMode::Clear,
            environment: Vec::new(),
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms: 1000,
            stdin_max_bytes: MAX_HOST_FACT_BYTES,
            stdout_max_bytes: MAX_HOST_FACT_BYTES,
            stderr_max_bytes: MAX_HOST_FACT_BYTES,
            poll_interval_ms: 10,
            teardown_timeout_ms: 500,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], true, true).ok()?,
    })
    .ok()?;
    if output.completion != Completion::Exited
        || output.exit_code != Some(0)
        || output.stdout.truncated
    {
        return None;
    }
    let text = std::str::from_utf8(&output.stdout.bytes).ok()?;
    u32::try_from(text.lines().filter(|line| !line.trim().is_empty()).count()).ok()
}

/// Confidential local-executor OS-instance bucket, independent of runner labels.
/// This is not hardware attestation and must never enter customer receipts.
pub(crate) fn physical_host_identity() -> String {
    read_bounded_text("/etc/machine-id").unwrap_or_else(|| "local-host-unknown".into())
}

fn security_files(directory: &str) -> std::collections::BTreeMap<String, String> {
    let mut facts = std::collections::BTreeMap::new();
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten().take(128) {
            if let Some(text) = entry.path().to_str().and_then(read_bounded_text) {
                facts.insert(entry.file_name().to_string_lossy().into_owned(), text);
            }
        }
    }
    facts
}

fn has_l3_cache_assignment(schemata: &str) -> bool {
    schemata
        .lines()
        .map(str::trim_start)
        .any(|line| line.starts_with("L3:") || line.starts_with("L3CODE:"))
}

/// Assign every VMM/vCPU thread to an operator-created CAT group before
/// guest job dispatch. Future threads inherit their parent's group.
fn assign_cache_partition(pid: u32, partition: &str) -> Result<Value, String> {
    if !crate::identifier(partition) {
        return Err("cache partition must be a single resctrl group name".into());
    }
    let group = Path::new("/sys/fs/resctrl").join(partition);
    let schemata = read_bounded(&group.join("schemata"), MAX_HOST_FACT_BYTES)?;
    let schemata = std::str::from_utf8(&schemata).map_err(|_| "CAT schemata is not UTF-8")?;
    if !has_l3_cache_assignment(schemata) {
        return Err("cache partition has no L3 CAT assignment".into());
    }
    let mut threads = Vec::new();
    for entry in fs::read_dir(format!("/proc/{pid}/task")).map_err(|_| "VMM threads unavailable")? {
        let entry = entry.map_err(|_| "VMM thread unreadable")?;
        let tid: u32 = entry
            .file_name()
            .to_string_lossy()
            .parse()
            .map_err(|_| "VMM thread ID invalid")?;
        fs::write(group.join("tasks"), tid.to_string())
            .map_err(|_| "resctrl thread assignment refused")?;
        threads.push(tid);
    }
    threads.sort_unstable();
    let assigned = read_bounded(&group.join("tasks"), 65536)?;
    let assigned = std::str::from_utf8(&assigned).map_err(|_| "resctrl tasks invalid")?;
    let assigned = assigned
        .lines()
        .map(|line| line.trim().parse::<u32>())
        .collect::<Result<std::collections::BTreeSet<_>, _>>()
        .map_err(|_| "resctrl thread ID invalid")?;
    if threads.iter().any(|tid| !assigned.contains(tid)) {
        return Err("resctrl thread assignment not retained".into());
    }
    Ok(
        json!({"group": partition, "schemata": schemata, "threads": threads, "bandwidth_partitioned": false}),
    )
}

/// Read bounded kernel, package, security and network exposure observations.
/// Missing kernel/SMT facts fail closed; absent KSM support is not running.
pub(crate) fn read_host_facts() -> HostFacts {
    let release =
        read_bounded_text("/proc/sys/kernel/osrelease").unwrap_or_else(|| "unknown".into());
    let smt_active = parse_sysctl("/sys/devices/system/cpu/smt/active").unwrap_or(true);
    let ksm_running = read_bounded_text("/sys/kernel/mm/ksm/run")
        .map(|value| value != "0")
        .unwrap_or_else(|| Path::new("/sys/kernel/mm/ksm/run").exists());
    let mptcp_enabled = parse_sysctl("/proc/sys/net/mptcp/enabled");
    let tcp_migrate_req = parse_sysctl("/proc/sys/net/ipv4/tcp_migrate_req");
    let mut facts = HostFacts::observed(
        release,
        host_package(),
        &std::collections::BTreeMap::new(),
        smt_active,
        ksm_running,
        mptcp_enabled,
        tcp_migrate_req,
        mptcp_listener_count(),
        read_bounded(Path::new("/proc/mounts"), 65536)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .is_some_and(|mounts| kiln_microvm::host::resctrl_at_expected_mountpoint(&mounts)),
    );
    facts.smt_control = read_bounded_text("/sys/devices/system/cpu/smt/control");
    facts.ksm_run = read_bounded_text("/sys/kernel/mm/ksm/run");
    facts.kernel_cmdline = read_bounded_text("/proc/cmdline");
    facts.vulnerabilities = security_files("/sys/devices/system/cpu/vulnerabilities");
    for resource in ["L3", "L3CODE", "L3DATA", "MB"] {
        for (name, value) in security_files(&format!("/sys/fs/resctrl/info/{resource}")) {
            facts
                .resctrl_capabilities
                .insert(format!("{resource}/{name}"), value);
        }
    }
    facts
}

pub(crate) struct Runner<'a> {
    pub(crate) vmm: &'a Executable,
    pub(crate) kind: &'static str,
    pub(crate) kernel: &'a Image,
    pub(crate) initrd: &'a Image,
    pub(crate) policy: MachinePolicy<'a>,
    pub(crate) warm_pool: Option<&'a WarmPool>,
    pub(crate) debug_hold: Option<&'a DebugHoldPolicy>,
    pub(crate) trust_domain: Option<&'a str>,
    pub(crate) host_identity: Option<&'a str>,
    pub(crate) cache_partition: Option<&'a str>,
    pub(crate) cpu_affinity: Option<&'a str>,
    pub(crate) version: Option<&'a str>,
}

impl<'a> Runner<'a> {
    pub(crate) fn from_backend(backend: &'a Backend) -> Option<Self> {
        let Backend::Firecracker {
            firecracker,
            kernel,
            initrd,
            warm_pool,
            debug_hold,
            trust_domain,
            host_identity,
            cache_partition,
            cpu_affinity,
            version,
            ..
        } = backend
        else {
            return None;
        };
        Some(Self {
            vmm: firecracker,
            kind: "firecracker",
            kernel,
            initrd,
            policy: machine_policy(backend)?,
            warm_pool: warm_pool.as_ref(),
            debug_hold: debug_hold.as_ref(),
            trust_domain: trust_domain.as_deref(),
            host_identity: host_identity.as_deref(),
            cache_partition: cache_partition.as_deref(),
            cpu_affinity: cpu_affinity.as_deref(),
            version: version.as_deref(),
        })
    }

    pub(crate) fn from_cloud_hypervisor(backend: &'a Backend) -> Option<Self> {
        let Backend::CloudHypervisor {
            cloud_hypervisor,
            kernel,
            initrd,
            warm_pool,
            cpu_affinity,
            version,
            ..
        } = backend
        else {
            return None;
        };
        Some(Self {
            vmm: cloud_hypervisor,
            kind: "cloud-hypervisor",
            kernel,
            initrd,
            policy: machine_policy(backend)?,
            warm_pool: warm_pool.as_ref(),
            debug_hold: None,
            trust_domain: None,
            host_identity: None,
            cache_partition: None,
            cpu_affinity: cpu_affinity.as_deref(),
            version: version.as_deref(),
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
        Backend::CloudHypervisor {
            guest_shell,
            vcpus,
            memory_mib,
            boot_timeout_ms,
            source_image_mib,
            ..
        } => Some(MachinePolicy {
            vcpus: *vcpus,
            memory_mib: *memory_mib,
            boot_timeout_ms: *boot_timeout_ms,
            source_image_mib: *source_image_mib,
            warm_slots: None,
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
    pub(crate) project: &'a str,
    pub(crate) attempt: &'a str,
    pub(crate) job: &'a str,
    pub(crate) runner: &'a str,
    pub(crate) workspace: &'a Path,
    pub(crate) steps: &'a [(String, GuestStep)],
    pub(crate) unit: &'a str,
    pub(crate) timeout_ms: u64,
    pub(crate) cache_image: Option<&'a Path>,
    pub(crate) cache_paths: Option<&'a [String]>,
    pub(crate) cache_max_bytes: usize,
    /// Credential and trust facts that rule out a debug hold.
    pub(crate) grants: Grants,
}

pub(crate) struct JobOutcome {
    pub(crate) steps: Vec<StepObservation>,
    pub(crate) vm: Value,
    pub(crate) cache_tar: Option<Vec<u8>>,
}

pub(crate) type Vmm<'scope> = ScopedJoinHandle<'scope, Result<ExecutionOutput, RunError>>;

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
pub(crate) fn source_image(workspace: &Path, image: &Path, capacity: u64) -> Result<u64, String> {
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

pub(crate) struct VmmBudget {
    pub timeout_ms: u64,
    pub capture_bytes: usize,
    pub timeout_margin_ms: u64,
    pub preboot_syscalls: Option<&'static str>,
}

pub(crate) fn spawn_vmm<'scope>(
    scope: &'scope Scope<'scope, '_>,
    config: &'scope Config,
    unit: &str,
    budget: VmmBudget,
    affinity: Option<&str>,
    arguments: Vec<String>,
    stop: &'scope AtomicBool,
) -> Result<Vmm<'scope>, String> {
    let VmmBudget {
        timeout_ms,
        capture_bytes,
        timeout_margin_ms,
        preboot_syscalls,
    } = budget;
    let mut service = unit_command(config, unit, timeout_ms, true)?;
    if let Some(syscalls) = preboot_syscalls {
        service.insert(
            0,
            format!("--property=SystemCallFilter={}", syscalls.trim()).into(),
        );
        service.insert(0, "--property=SystemCallErrorNumber=EPERM".into());
    }
    if let Some(affinity) = affinity {
        let taskset = config
            .taskset
            .as_ref()
            .ok_or("CPU affinity requires a configured taskset")?
            .verify()?;
        service.extend([
            taskset.into_os_string(),
            OsString::from("-c"),
            OsString::from(affinity),
        ]);
    }
    service.extend(arguments.into_iter().map(Into::into));
    let request = manager_request(
        config,
        config.systemd_run.verify()?,
        service,
        timeout_ms.saturating_add(timeout_margin_ms),
        capture_bytes,
        Input::Null,
    )?;
    Ok(scope.spawn(move || bounded_exec::run_with_cancellation(request, stop)))
}

pub(crate) struct ConfinementObservation {
    pub(crate) pid: u32,
    facts: ConfinementFacts,
    exe_blake3: String,
}

// r[impl kiln.microvm.confinement.receipt]
pub(crate) fn confinement_value(result: &Result<ConfinementObservation, String>) -> Value {
    match result {
        Ok(observed) => json!({
            "ok": true,
            "pid": observed.pid,
            "name": observed.facts.name,
            "uid": observed.facts.uid,
            "gid": observed.facts.gid,
            "cap_eff": format!("{:016x}", observed.facts.cap_eff),
            "no_new_privs": 1,
            "seccomp": observed.facts.seccomp,
            "exe": observed.facts.exe,
            "exe_blake3": observed.exe_blake3,
        }),
        Err(error) => json!({"ok": false, "error": error}),
    }
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|_| "confinement evidence is unreadable".to_string())?;
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "confinement evidence is unreadable".to_string())?;
    if bytes.len() > maximum {
        return Err("confinement evidence exceeds its bound".into());
    }
    Ok(bytes)
}

fn control_group(config: &Config, unit: &str, deadline: Instant) -> Result<String, String> {
    let output = crate::execution::systemctl_until(
        config,
        &["show", unit, "--property=ControlGroup"],
        deadline,
    )?;
    if output.completion != Completion::Exited || output.stdout.truncated {
        return Err("attempt cgroup is unavailable".into());
    }
    let text = std::str::from_utf8(&output.stdout.bytes)
        .map_err(|_| "attempt cgroup evidence is not UTF-8".to_string())?;
    let mut values = text
        .lines()
        .filter_map(|line| line.strip_prefix("ControlGroup="));
    let group = values
        .next()
        .ok_or_else(|| "attempt cgroup is missing".to_string())?;
    if values.next().is_some()
        || group.is_empty()
        || !group.starts_with('/')
        || group.len() > 4096
        || group.split('/').any(|part| part == "..")
    {
        return Err("attempt cgroup is malformed".into());
    }
    Ok(group.to_owned())
}

pub(crate) fn observe_confinement(
    config: &Config,
    unit: &str,
    expected_name: &str,
    expected_exe: &Path,
    exe_blake3: &str,
    allow_unfiltered_main: bool,
    wait: Duration,
) -> Result<ConfinementObservation, String> {
    let deadline = Instant::now() + wait;
    loop {
        match observe_confinement_once(
            config,
            unit,
            expected_name,
            expected_exe,
            exe_blake3,
            allow_unfiltered_main,
            deadline,
        ) {
            Ok(observation) => return Ok(observation),
            Err(error) if Instant::now() >= deadline => return Err(error),
            Err(_) => std::thread::sleep(
                Duration::from_millis(10).min(deadline.saturating_duration_since(Instant::now())),
            ),
        }
    }
}

// r[impl kiln.microvm.confinement]
fn observe_confinement_once(
    config: &Config,
    unit: &str,
    expected_name: &str,
    expected_exe: &Path,
    exe_blake3: &str,
    allow_unfiltered_main: bool,
    deadline: Instant,
) -> Result<ConfinementObservation, String> {
    let group = control_group(config, unit, deadline)?;
    let cgroup = Path::new("/sys/fs/cgroup").join(group.trim_start_matches('/'));
    let processes = read_bounded(&cgroup.join("cgroup.procs"), 4096)?;
    let processes = std::str::from_utf8(&processes)
        .map_err(|_| "attempt cgroup processes are not UTF-8".to_string())?;
    let mut pids = processes
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.parse::<u32>()
                .map_err(|_| "attempt cgroup process is malformed".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    pids.sort_unstable();
    pids.dedup();
    let [pid] = pids.as_slice() else {
        return Err("attempt cgroup must contain exactly one VMM process".into());
    };
    let status = read_bounded(
        &Path::new("/proc").join(pid.to_string()).join("status"),
        MAX_STATUS_BYTES,
    )?;
    let exe = fs::read_link(Path::new("/proc").join(pid.to_string()).join("exe"))
        .map_err(|_| "VMM executable evidence is unavailable".to_string())?;
    let exe = text(&exe)?.to_owned();
    let expected = text(expected_exe)?;
    // `/proc/self` is owned by the service user; no unsafe identity syscall is
    // needed under the workspace's `unsafe_code = "forbid"` policy.
    let identity = fs::metadata("/proc/self")
        .map_err(|_| "service process identity is unavailable".to_string())?;
    let (uid, gid) = (identity.uid(), identity.gid());
    let expected_process = ExpectedProcess {
        name: expected_name,
        uid,
        gid,
        exe: expected,
        allow_unfiltered_main,
        require_name: true,
    };
    let mut facts = admit_status(&status, &exe, expected_process).map_err(str::to_owned)?;
    let tasks = Path::new("/proc").join(pid.to_string()).join("task");
    let mut workers = 0_usize;
    for task in
        fs::read_dir(&tasks).map_err(|_| "VMM thread evidence is unavailable".to_string())?
    {
        let task = task.map_err(|_| "VMM thread evidence is unavailable".to_string())?;
        let tid = task
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<u32>().ok())
            .ok_or_else(|| "VMM thread identity is malformed".to_string())?;
        if tid == *pid {
            continue;
        }
        workers += 1;
        if workers > 128 {
            return Err("VMM thread count exceeds its bound".into());
        }
        let Ok(status) = read_bounded(
            &tasks.join(tid.to_string()).join("status"),
            MAX_STATUS_BYTES,
        ) else {
            continue;
        };
        let worker = match admit_status(
            &status,
            &exe,
            ExpectedProcess {
                allow_unfiltered_main: false,
                require_name: false,
                ..expected_process
            },
        ) {
            Ok(worker) => worker,
            Err(_) => continue,
        };
        facts.seccomp_filtered += worker.seccomp_filtered;
    }
    facts.threads += workers;
    if facts.seccomp_filtered == 0 {
        return Err("VMM has no seccomp-filtered thread".into());
    }
    Ok(ConfinementObservation {
        pid: *pid,
        facts,
        exe_blake3: exe_blake3.to_owned(),
    })
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
pub(crate) fn api(
    socket: &Path,
    method: &str,
    path: &str,
    body: &Value,
    vmm: &Vmm<'_>,
) -> Result<(), String> {
    api_until(
        socket,
        method,
        path,
        body,
        vmm,
        Instant::now() + API_TIMEOUT,
    )
}

pub(crate) fn api_until(
    socket: &Path,
    method: &str,
    path: &str,
    body: &Value,
    vmm: &Vmm<'_>,
    deadline: Instant,
) -> Result<(), String> {
    let remaining = || {
        deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
            .ok_or_else(|| "Firecracker API deadline exhausted".to_owned())
    };
    let mut stream = loop {
        remaining()?;
        match UnixStream::connect(socket) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline && !vmm.is_finished() => {
                std::thread::sleep(POLL.min(remaining()?))
            }
            Err(_) => return Err("Firecracker API socket unavailable".into()),
        }
    };
    stream
        .set_read_timeout(Some(remaining()?))
        .and_then(|()| {
            stream.set_write_timeout(Some(
                deadline
                    .saturating_duration_since(Instant::now())
                    .max(Duration::from_nanos(1)),
            ))
        })
        .and_then(|()| stream.write_all(&plan::api_request(method, path, body)))
        .map_err(|_| "Firecracker API write failed")?;
    let mut response = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        if plan::api_response_length(&response).is_some_and(|length| response.len() >= length) {
            break;
        }
        stream
            .set_read_timeout(Some(remaining()?))
            .map_err(|_| "Firecracker API read deadline failed")?;
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
    let base = plan::snapshot_identity(&plan::SnapshotInputs {
        vmm_kind: runner.kind,
        vmm_version: runner.version.unwrap_or("unknown"),
        vmm_blake3: &runner.vmm.blake3,
        kernel_blake3: &runner.kernel.blake3,
        initrd_blake3: &runner.initrd.blake3,
        vcpus: runner.policy.vcpus,
        memory_mib: runner.policy.memory_mib,
        source_image_mib: runner.policy.source_image_mib,
        slot: slot.index,
        slot_directory: text(&slot.directory)?,
    });
    crate::identity(&("kiln.hosted.snapshot.cache-free-drive.v2", base))
}

/// A slot snapshot is reusable only for the exact identity and state bytes.
fn snapshot_reusable(slot: &Slot, identity: &str, kind: &str, memory_bytes: u64) -> bool {
    let Ok(raw) = fs::read(slot.path("manifest.json")) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_slice::<SlotManifest>(&raw) else {
        return false;
    };
    if kind == "cloud-hypervisor" {
        return manifest.identity == identity && slot.path("state").is_dir();
    }
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
    vmm: PathBuf,
}

fn vm_config(
    paths: &Paths,
    image: &Path,
    cache_image: Option<&Path>,
    vsock: &Path,
    runner: &Runner<'_>,
) -> Result<Vec<u8>, String> {
    let document = plan::vm_config(
        &VmPaths {
            kernel: text(&paths.kernel)?,
            initrd: text(&paths.initrd)?,
            source_image: text(image)?,
            cache_image: cache_image.map(text).transpose()?,
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
fn build_cloud_hypervisor_snapshot(
    config: &Config,
    runner: &Runner<'_>,
    paths: &Paths,
    slot: &Slot,
    identity: &str,
    job: &Job<'_>,
    cancellation: &AtomicBool,
) -> Result<(), String> {
    invalidate(slot);
    let _ = fs::remove_dir_all(slot.path("state"));
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
    let listener = UnixListener::bind(&listener_path).map_err(|_| "guest listener bind failed")?;
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let mut arguments = cloud_hypervisor_cold_boot(
            text(&paths.vmm)?,
            &ColdBootPaths {
                kernel: text(&paths.kernel)?,
                initrd: text(&paths.initrd)?,
                source_image: text(&image)?,
                vsock: text(&vsock)?,
            },
            &runner.policy,
        );
        arguments.extend([
            "--api-socket".into(),
            format!("path={}", text(&api_socket)?),
        ]);
        let vmm = spawn_vmm(
            scope,
            config,
            job.unit,
            VmmBudget {
                timeout_ms: job.timeout_ms,
                capture_bytes: CONSOLE_BYTES,
                timeout_margin_ms: VMM_EXEC_MARGIN_MS,
                preboot_syscalls: None,
            },
            runner.cpu_affinity,
            arguments,
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
            fs::create_dir_all(slot.path("state"))
                .map_err(|_| "snapshot destination directory create failed".to_string())?;
            for request in cloud_hypervisor_snapshot_requests(text(&slot.path("state"))?) {
                api(
                    &api_socket,
                    request.method,
                    request.path,
                    &request.body,
                    &vmm,
                )?;
            }
            Ok(())
        })();
        let (cleaned, console) = destroy(config, job.unit, vmm, &stop);
        if !cleaned {
            return Err("snapshot VM teardown uncertain".to_owned());
        }
        built.map_err(|error| format!("{error}; console: {}", tail(&console)))
    })?;
    let manifest = SlotManifest {
        identity: identity.into(),
        state_blake3: String::new(),
        memory_bytes: 0,
    };
    write_private(
        &slot.path("manifest.json"),
        &serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
    )
}

fn build_snapshot(
    config: &Config,
    runner: &Runner<'_>,
    paths: &Paths,
    slot: &Slot,
    identity: &str,
    job: &Job<'_>,
    cancellation: &AtomicBool,
) -> Result<(), String> {
    if runner.kind == "cloud-hypervisor" {
        return build_cloud_hypervisor_snapshot(
            config,
            runner,
            paths,
            slot,
            identity,
            job,
            cancellation,
        );
    }
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
    // The snapshot contains only an empty read-only device. No tenant cache
    // bytes enter snapshot memory or its identity.
    let blank_cache = slot.path("cache-empty.img");
    write_private(&blank_cache, &[0_u8; 1024])?;
    OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&blank_cache)
        .and_then(|file| file.set_len(u64::from(runner.policy.source_image_mib) * MIB))
        .map_err(|_| "warm pool cache image size failed")?;
    let config_path = slot.path("vm.json");
    write_private(
        &config_path,
        &vm_config(paths, &image, Some(&blank_cache), &vsock, runner)?,
    )?;
    let listener = UnixListener::bind(&listener_path).map_err(|_| "guest listener bind failed")?;
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let vmm = spawn_vmm(
            scope,
            config,
            job.unit,
            VmmBudget {
                timeout_ms: job.timeout_ms,
                capture_bytes: CONSOLE_BYTES,
                timeout_margin_ms: VMM_EXEC_MARGIN_MS,
                preboot_syscalls: None,
            },
            runner.cpu_affinity,
            vec![
                text(&paths.vmm)?.into(),
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
    /// Cancellation as observed when a debug hold began; the journaled
    /// result never depends on a cancellation that arrives while held.
    cancel_at_hold: Option<bool>,
    debug_hold: Option<Value>,
    /// Observed host confinement, recorded before the guest session.
    confinement: Option<Value>,
    cache_tar: Option<Vec<u8>>,
}

/// The not-yet-connected guest of a VM that is starting or restoring.
struct PendingGuest<'a> {
    listener: UnixListener,
    listener_path: &'a Path,
    spawned: Instant,
    boot_deadline: Instant,
    job_deadline: Instant,
}

/// Serve the guest session of a VM that is starting or restoring. The
/// accepted stream is returned for a possible debug hold.
fn drive_guest(
    session: &mut Session,
    lifecycle: &mut Lifecycle,
    pending: PendingGuest<'_>,
    vmm: &Vmm<'_>,
    job: &Job<'_>,
    cancellation: &AtomicBool,
) -> Result<UnixStream, String> {
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
                let admitted = admission_error.is_none();
                session.admission_error = admission_error;
                session.session_ms = Some(millis(started));
                if admitted && let Some(paths) = job.cache_paths {
                    #[cfg(feature = "cache-fault-tests")]
                    crate::cache::receiving(job.unit);
                    write_host(
                        &mut stream,
                        HostMessage::Publish {
                            nonce: job.job.into(),
                            paths: paths.to_vec(),
                            maximum_bytes: job.cache_max_bytes,
                        },
                    )?;
                    match read_guest(&mut stream, job_deadline, cancellation)? {
                        GuestMessage::Cache { nonce, tar_hex } if nonce == job.job => {
                            if tar_hex.len() > job.cache_max_bytes.saturating_mul(2) {
                                return Err("guest cache return exceeds bound".into());
                            }
                            session.cache_tar = protocol::hex_bytes(&tar_hex);
                            if session.cache_tar.is_none() {
                                return Err("guest cache tar encoding rejected".into());
                            }
                        }
                        GuestMessage::Done {
                            nonce,
                            admission_error,
                        } if nonce == job.job => {
                            return Err(format!(
                                "guest cache rejected: {}",
                                admission_error.unwrap_or_else(|| "no reason".into())
                            ));
                        }
                        _ => return Err("unexpected guest cache message".into()),
                    }
                }
                return Ok(stream);
            }
            _ => return Err("unexpected guest message".into()),
        }
    }
}

/// The running VM and job a debug hold applies to.
struct HoldScope<'a, 'scope> {
    db: &'a Connection,
    policy: &'a DebugHoldPolicy,
    runner: &'a Runner<'a>,
    job: &'a Job<'a>,
    vmm: &'a Vmm<'scope>,
    cancellation: &'a AtomicBool,
}

// r[impl kiln.hosted.microvm.debug_hold.lifecycle]
// r[impl kiln.hosted.microvm.debug_hold.end]
// r[impl kiln.hosted.microvm.debug_hold.relay]
/// Debug hold of a journaled job whose runner enables it. The VM stays up,
/// and this thread relays queued debug sessions over the already accepted
/// guest stream until the pure hold decision ends it; the lifecycle is then
/// left in `Destroying` for normal destruction. Returns the receipt record.
fn hold(
    scope: &HoldScope<'_, '_>,
    guest: Option<&mut UnixStream>,
    lifecycle: &mut Lifecycle,
    session: &mut Session,
) -> Value {
    let HoldScope {
        db,
        policy,
        runner,
        job,
        vmm,
        cancellation,
    } = *scope;
    let held = crate::debug_hold::HeldJob {
        organization: job.organization,
        project: job.project,
        attempt: job.attempt,
        job: job.job,
        runner: job.runner,
    };
    let failed = hold_policy::job_failed(
        job.steps.len(),
        &session.reports,
        session.admission_error.as_deref(),
    );
    let facts = |current: u32| HoldFacts {
        enabled: policy.enabled,
        failed,
        grants: job.grants,
        held: current,
        max_concurrent: policy.max_concurrent,
    };
    let Some(stream) = guest else {
        let reason = hold_policy::eligibility(&facts(0))
            .err()
            .map_or("session_incomplete", hold_policy::Refusal::as_str);
        return json!({"outcome": "not_held", "reason": reason});
    };
    let cancel_at_hold = cancellation.load(Ordering::Acquire);
    let begun = crate::debug_hold::begin(db, &held, facts, policy.grace_ms(), |facts| {
        lifecycle.advance(Event::Hold(facts)).map(|_| ())
    });
    let (started_ms, expires_ms) = match begun {
        Ok(crate::debug_hold::Begin::Held {
            started_ms,
            expires_ms,
        }) => (started_ms, expires_ms),
        Ok(crate::debug_hold::Begin::Refused(reason)) => return not_held(lifecycle, reason),
        Err(error) => return not_held(lifecycle, &error),
    };
    session.cancel_at_hold = Some(cancel_at_hold);
    let mut signals = hold_policy::HoldSignals {
        storage_failed: journal(db, job, "held", None).is_err(),
        ..hold_policy::HoldSignals::default()
    };
    let mut sessions = 0_u64;
    let end = loop {
        let now = crate::now_ms().unwrap_or(u64::MAX);
        match crate::debug_hold::signals(db, &held, started_ms) {
            Ok((released, pressure)) => {
                signals.released = released;
                signals.capacity_pressure = pressure;
            }
            Err(_) => signals.storage_failed = true,
        }
        signals.vmm_exited = vmm.is_finished();
        signals.cancelled = cancellation.load(Ordering::Acquire);
        if let Some(end) = hold_policy::hold_end(now, expires_ms, signals) {
            break end;
        }
        match crate::debug_hold::next_request(db, &held) {
            Ok(Some((ordinal, command))) => {
                sessions += 1;
                match relay(
                    db,
                    stream,
                    runner.policy.guest_shell,
                    (ordinal, &command),
                    expires_ms,
                    cancellation,
                ) {
                    Ok(in_sync) => {
                        signals.guest_lost = !in_sync && !cancellation.load(Ordering::Acquire);
                    }
                    Err(_) => signals.storage_failed = true,
                }
            }
            Ok(None) => std::thread::sleep(crate::debug_hold::HOLD_POLL),
            Err(_) => signals.storage_failed = true,
        }
    };
    let recorded = crate::debug_hold::end(db, &held, end);
    let ended_ms = crate::now_ms().unwrap_or(expires_ms);
    // `Held` accepts `HoldEnded` unconditionally; the journaled result is
    // never touched after the hold began.
    let _destroying = lifecycle.advance(Event::HoldEnded);
    json!({
        "outcome": "held",
        "grace_seconds": policy.grace_seconds,
        "started_ms": started_ms,
        "expires_ms": expires_ms,
        "end_reason": end.as_str(),
        "held_ms": ended_ms.saturating_sub(started_ms),
        "sessions": crate::debug_hold::session_count(db, &held).unwrap_or(sessions),
        "record": if recorded.is_ok() { "durable" } else { "storage_failed" },
        "connect": hold_policy::connection_instructions(job.project, job.attempt, job.job, expires_ms),
    })
}

/// A refused hold; a lifecycle that already entered `Held` (the reservation
/// did not commit) ends it at once for normal destruction.
fn not_held(lifecycle: &mut Lifecycle, reason: &str) -> Value {
    if lifecycle.state == State::Held {
        // `Held` accepts `HoldEnded` unconditionally.
        let _destroying = lifecycle.advance(Event::HoldEnded);
    }
    json!({"outcome": "not_held", "reason": reason})
}

/// Relay one queued debug command and record its bounded report. Returns
/// whether the guest session is still in sync for another command; an error
/// means the session record could not be written.
fn relay(
    db: &Connection,
    stream: &mut UnixStream,
    shell: &str,
    (ordinal, command): (i64, &str),
    expires_ms: u64,
    cancellation: &AtomicBool,
) -> Result<bool, String> {
    let step = match hold_policy::debug_step(shell, command) {
        Ok(step) => step,
        Err(error) => {
            crate::debug_hold::finish_request(db, ordinal, false, &json!({"error": error}))?;
            return Ok(true);
        }
    };
    let deadline_ms =
        hold_policy::command_deadline_ms(crate::now_ms().unwrap_or(u64::MAX), expires_ms);
    let nonce = format!("debug-{ordinal}");
    let outcome = write_host(
        stream,
        HostMessage::Debug {
            nonce: nonce.clone(),
            deadline_ms,
            step,
        },
    )
    .and_then(|()| {
        read_guest(
            stream,
            Instant::now()
                + Duration::from_millis(
                    deadline_ms.saturating_add(crate::debug_hold::GUEST_REPLY_MARGIN_MS),
                ),
            cancellation,
        )
    })
    .and_then(|message| match message {
        GuestMessage::Debug {
            nonce: answered,
            report,
        } if answered == nonce => Ok(report),
        GuestMessage::Done {
            admission_error: Some(error),
            ..
        } => Err(error),
        _ => Err("unexpected guest message".into()),
    });
    let in_sync = outcome.is_ok();
    let result = match outcome {
        Ok(report) => json!(report),
        Err(error) => json!({"error": error}),
    };
    crate::debug_hold::finish_request(db, ordinal, in_sync, &result)?;
    Ok(in_sync)
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
    host: &HostFacts,
    host_preflight_override: Option<&str>,
    cancellation: &AtomicBool,
) -> JobOutcome {
    let started = Instant::now();
    let job_deadline = started + Duration::from_millis(job.timeout_ms);
    let mut lifecycle = Lifecycle::new(job.steps.len());
    let mut session = Session::default();
    // Zero unless the runner enables holds and the job could ever be held;
    // only then does the unit outlive the job deadline by the grace.
    let hold_reserve_ms = hold_policy::reserve_ms(runner.debug_hold, job.grants);
    let unit_timeout_ms = if hold_reserve_ms == 0 {
        job.timeout_ms
    } else {
        job.timeout_ms
            .saturating_add(hold_reserve_ms)
            .saturating_add(crate::debug_hold::HOLD_UNIT_MARGIN_MS)
    };
    // r[impl kiln.hosted.microvm.claims]
    let mut vm = json!({
        "backend": runner.kind,
        "backend_version": runner.version,
        "vmm_blake3": runner.vmm.blake3,
        "capabilities": kiln_microvm::backend::Firecracker.capabilities(),
        "vcpus": runner.policy.vcpus,
        "memory_mib": runner.policy.memory_mib,
        "network": "none",
        "jailer": false,
        "hostile_isolation_claim": false,
        "host": host,
    });
    if let (Some(fields), Ok(Value::Object(host_fields))) =
        (vm.as_object_mut(), serde_json::to_value(host))
    {
        fields.extend(host_fields);
    }
    let mut cleaned = true;
    let mut journaled = false;
    let mut console = String::new();
    let mut observe = |key: &str, value: Value| {
        if let Some(fields) = vm.as_object_mut() {
            fields.insert(key.into(), value);
        }
    };
    let untrusted_capable = matches!(
        config.runners.get(job.runner),
        Some(Backend::Firecracker {
            untrusted_capable: true,
            ..
        })
    );
    let observed_refusal = host.untrusted_refusal();
    let override_applied =
        untrusted_capable && observed_refusal.is_some() && host_preflight_override.is_some();
    observe(
        "host_admission",
        json!({"decision": "admitted", "untrusted_capable": untrusted_capable, "kernel_policy_source": "issue_28_fixed_release_table", "override_reason": host_preflight_override, "override_applied": override_applied, "observed_refusal": observed_refusal, "mptcp_exposure_reduced_partial": host.mptcp_exposure_reduced(), "hostile_isolation_claim": false}),
    );
    if let Some(reason) = host_preflight_override {
        observe("host_preflight_override", json!(reason));
    }
    if let Some(domain) = runner.trust_domain {
        observe("trust_domain", json!(domain));
    }
    if let Some(host) = runner.host_identity {
        observe("host_identity", json!(host));
    }
    if let Some(affinity) = runner.cpu_affinity {
        observe("cpu_affinity", json!(affinity));
    }
    if let Some(partition) = runner.cache_partition {
        observe("cache_partition", json!(partition));
    }
    if let Err(error) = journal(db, job, "starting", None) {
        session.failure = Some(error);
    } else {
        let outcome = (|| -> Result<(), String> {
            let verify_started = Instant::now();
            let paths = Paths {
                vmm: runner.vmm.verify()?,
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
                if snapshot_reusable(
                    held,
                    &identity,
                    runner.kind,
                    u64::from(runner.policy.memory_mib) * MIB,
                ) {
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
            let mut arguments = if runner.kind == "cloud-hypervisor" {
                if let Some(held) = &slot {
                    fs::rename(&image, held.path("source.img"))
                        .map_err(|_| "warm pool source image install failed")?;
                    cloud_hypervisor_restore(
                        text(&paths.vmm)?,
                        text(&held.path("state"))?,
                        text(&api_socket)?,
                    )
                } else {
                    cloud_hypervisor_cold_boot(
                        text(&paths.vmm)?,
                        &ColdBootPaths {
                            kernel: text(&paths.kernel)?,
                            initrd: text(&paths.initrd)?,
                            source_image: text(&image)?,
                            vsock: text(&vsock)?,
                        },
                        &runner.policy,
                    )
                }
            } else {
                vec![text(&paths.vmm)?.to_owned()]
            };
            if runner.kind == "cloud-hypervisor" {
                // Phase 1 is cold boot only; no API socket is exposed.
            } else if let Some(held) = &slot {
                fs::rename(&image, held.path("source.img"))
                    .map_err(|_| "warm pool source image install failed")?;
                arguments.extend(["--api-sock".into(), text(&api_socket)?.into()]);
            } else {
                let config_path = scratch.path().join("vm.json");
                write_private(
                    &config_path,
                    &vm_config(&paths, &image, job.cache_image, &vsock, runner)?,
                )?;
                arguments.extend([
                    "--no-api".into(),
                    "--config-file".into(),
                    text(&config_path)?.into(),
                ]);
            }
            if runner.kind != "cloud-hypervisor" {
                arguments.extend(["--level".into(), "Error".into()]);
            }
            let listener =
                UnixListener::bind(&listener_path).map_err(|_| "guest listener bind failed")?;
            let stop = AtomicBool::new(false);
            std::thread::scope(|scope| -> Result<(), String> {
                let spawned = Instant::now();
                let vmm = spawn_vmm(
                    scope,
                    config,
                    job.unit,
                    VmmBudget {
                        timeout_ms: unit_timeout_ms,
                        capture_bytes: CONSOLE_BYTES,
                        timeout_margin_ms: VMM_EXEC_MARGIN_MS,
                        preboot_syscalls: None,
                    },
                    runner.cpu_affinity,
                    arguments,
                    &stop,
                )?;
                lifecycle.advance(Event::Spawned)?;
                let served = (|| -> Result<UnixStream, String> {
                    if let Some(held) = &slot
                        && runner.kind != "cloud-hypervisor"
                    {
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
                        if let Some(cache) = job.cache_image {
                            api(
                                &api_socket,
                                "PATCH",
                                "/drives/cache",
                                &json!({
                                    "drive_id": plan::CACHE_DRIVE,
                                    "path_on_host": text(cache)?,
                                }),
                                &vmm,
                            )
                            .inspect_err(|_| invalidate(held))?;
                        }
                        session.restore_api_ms = Some(millis(spawned));
                    }
                    let confinement = observe_confinement(
                        config,
                        job.unit,
                        runner.kind,
                        &paths.vmm,
                        &runner.vmm.blake3,
                        runner.kind == "cloud-hypervisor",
                        Duration::from_millis(runner.policy.boot_timeout_ms.min(2_000)),
                    );
                    session.confinement = Some(confinement_value(&confinement));
                    match &confinement {
                        Ok(observation) => {
                            crate::placement::begin(
                                db,
                                job.organization,
                                job.attempt,
                                job.job,
                                observation.pid,
                            )?;
                            if let Some(partition) = runner.cache_partition {
                                observe(
                                    "cache_assignment",
                                    assign_cache_partition(observation.pid, partition)?,
                                );
                            }
                            lifecycle.advance(Event::ConfinementObserved { ok: true })?;
                            if runner.kind == "cloud-hypervisor" && slot.is_some() {
                                api(&api_socket, "PUT", "/api/v1/vm.resume", &Value::Null, &vmm)?;
                                session.restore_api_ms = Some(millis(spawned));
                            }
                        }
                        Err(error) => {
                            session.failure = Some(error.clone());
                            lifecycle.advance(Event::ConfinementObserved { ok: false })?;
                            return Err(error.clone());
                        }
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
                // Without a possible hold the session ends here, as before.
                let mut guest = match served {
                    Ok(stream) => (hold_reserve_ms > 0).then_some(stream),
                    Err(error) => {
                        session.failure = Some(error);
                        if !matches!(lifecycle.state, State::Collected { .. }) {
                            lifecycle.advance(Event::Failed)?;
                        }
                        None
                    }
                };
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
                if let Some(policy) = runner.debug_hold.filter(|policy| policy.enabled) {
                    let record = if lifecycle.state == State::Journaled {
                        let scope = HoldScope {
                            db,
                            policy,
                            runner,
                            job,
                            vmm: &vmm,
                            cancellation,
                        };
                        hold(&scope, guest.as_mut(), &mut lifecycle, &mut session)
                    } else {
                        json!({"outcome": "not_held", "reason": "not_journaled"})
                    };
                    session.debug_hold = Some(record);
                }
                drop(guest);
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
    match crate::placement::finish(db, job.organization, job.attempt, job.job, cleaned) {
        Ok(peers) => observe("co_residency", peers),
        Err(error) => {
            session.failure.get_or_insert(error);
        }
    }
    if let Some(confinement) = session.confinement.take() {
        observe("confinement", confinement);
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
    if let Some(debug_hold) = session.debug_hold.take() {
        // Last, so the job's VM record ends with how to connect.
        observe("debug_hold", debug_hold);
    }
    JobOutcome {
        steps: observations(job, &session, cleaned, cancellation),
        vm,
        cache_tar: publishable_cache(session.cache_tar, cleaned, journaled, lifecycle.state),
    }
}

fn publishable_cache(
    bytes: Option<Vec<u8>>,
    cleaned: bool,
    journaled: bool,
    state: State,
) -> Option<Vec<u8>> {
    if cleaned && journaled && matches!(state, State::Destroyed { journaled: true }) {
        bytes
    } else {
        None
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
        session
            .cancel_at_hold
            .unwrap_or_else(|| cancellation.load(Ordering::Acquire)),
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

    #[test]
    fn l3_cache_assignment_accepts_padded_kernel_rows_only_for_l3_resources() {
        for schemata in [
            "SMBA:0=4096;1=4096\n    MB:0=4096;1=4096\n    L3:0=ffff;1=ffff\n",
            "    L3CODE:0=ff\n  L3DATA:0=ff\n",
        ] {
            assert!(has_l3_cache_assignment(schemata));
        }
        for schemata in ["", "    MB:0=4096\n", "L3DATA:0=ff\n", "otherL3:0=ff\n"] {
            assert!(!has_l3_cache_assignment(schemata));
        }
    }

    // r[verify kiln.hosted.microvm.cache_integrity]
    #[test]
    fn unconfirmed_or_unrecorded_destruction_cannot_publish_guest_bytes() {
        assert_eq!(
            publishable_cache(
                Some(vec![7]),
                true,
                true,
                State::Destroyed { journaled: true }
            ),
            Some(vec![7])
        );
        for (cleaned, journaled, state) in [
            (false, true, State::Abandoned { journaled: true }),
            (false, true, State::Destroyed { journaled: true }),
            (true, false, State::Destroyed { journaled: true }),
            (true, true, State::Destroyed { journaled: false }),
            (true, true, State::Journaled),
        ] {
            assert_eq!(
                publishable_cache(Some(vec![7]), cleaned, journaled, state),
                None
            );
        }
    }

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
