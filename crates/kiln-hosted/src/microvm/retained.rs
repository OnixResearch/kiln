//! Retained Cloud Hypervisor host effects using the existing runner, systemd
//! confinement and observed cleanup. This is not a placement authority.
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bounded_exec::{Completion, Input};
use kiln_core::types::ContentId;
use kiln_microvm::backend::{VmmBackend, cloud_hypervisor_restore};
use kiln_microvm::retained::{OwnerToken, RetainedSession, RetainedState};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{API_RESPONSE_BYTES, API_TIMEOUT, Runner, observe_confinement};
use crate::execution::{Backend, Config, cleanup_unit, manager_request, unit_command};

pub const SELECTED_MOLTEN_REVISION: &str = "22f8ded26ca1907c29948e08b53f35df23080733";
const CPU_FACT_BYTES: u64 = 1_048_576;
const MAX_DIAGNOSTIC_CHARS: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedProfile {
    pub identity: ContentId,
    pub facts: Value,
}

/// Observe actual immutable runner and stable host CPU/kernel facts. This
/// identity is not itself migration qualification or placement authority.
pub fn observed_profile(config: &Config, backend: &Backend) -> Result<ObservedProfile, String> {
    let runner = Runner::from_cloud_hypervisor(backend).ok_or("not an existing CH backend")?;
    let binary = runner.vmm.verify()?;
    runner.kernel.verify()?;
    runner.initrd.verify()?;
    let request = manager_request(
        config,
        binary,
        vec!["--version".into()],
        1_000,
        4_096,
        Input::Null,
    )?;
    let output = bounded_exec::run(request).map_err(|e| e.to_string())?;
    if output.completion != Completion::Exited
        || output.exit_code != Some(0)
        || output.stdout.truncated
    {
        return Err("actual Cloud Hypervisor version unavailable".into());
    }
    let version = std::str::from_utf8(&output.stdout.bytes)
        .map_err(|e| e.to_string())?
        .trim();
    if runner.version != Some(version) || !version.contains("v54.0") {
        return Err(
            "retained CH API requires exact declared v54.0 CLI build; not qualified".into(),
        );
    }
    let mut cpu = String::new();
    fs::File::open("/proc/cpuinfo")
        .map_err(|e| e.to_string())?
        .take(CPU_FACT_BYTES + 1)
        .read_to_string(&mut cpu)
        .map_err(|e| e.to_string())?;
    if cpu.len() as u64 > CPU_FACT_BYTES {
        return Err("CPU observation exceeds bound".into());
    }
    let mut stable = std::collections::BTreeSet::new();
    for line in cpu.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            if matches!(
                key,
                "vendor_id" | "cpu family" | "model" | "stepping" | "microcode" | "flags"
            ) {
                stable.insert(format!("{key}:{}", value.trim()));
            }
        }
    }
    if stable.is_empty() || !stable.iter().any(|line| line.starts_with("flags:")) {
        return Err("actual exposed Host CPU features unavailable".into());
    }
    let profile = json!({"schema":"kiln.retained-ch-profile.v1", "vmm":runner.vmm.blake3,
        "version":version, "kernel":runner.kernel.blake3,"initrd":runner.initrd.blake3,
        "cpu_profile":"Host", "cpu_facts":stable, "host_kernel":super::host_package(),
        "vcpus":runner.policy.vcpus, "memory_mib":runner.policy.memory_mib,
        "memory_backing":"anonymous-private", "hugepages":false, "mergeable":false,
        "network":"no-device", "disk_image_type":"Raw", "vsock":true, "passthrough":false});
    let bytes = serde_json::to_vec(&profile).map_err(|e| e.to_string())?;
    let identity = ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
        .map_err(|e| format!("{e:?}"))?;
    Ok(ObservedProfile {
        identity,
        facts: profile,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub directory: PathBuf,
    pub unit: String,
    pub workspace: PathBuf,
    pub disk_generation: u64,
    /// Original admitted wall deadline; relocation/restore may not extend it.
    pub deadline_unix_ms: u64,
    /// Exact VMM binary, kernel/initrd, exposed CPU/device and memory-backing
    /// qualification identity. It is not merely an ISA or host label.
    pub profile: ContentId,
}

fn remaining_lifetime(deadline: u64, now: u64, maximum: u64) -> Result<u64, String> {
    deadline
        .checked_sub(now)
        .map(|left| left.min(maximum))
        .filter(|left| *left != 0)
        .ok_or_else(|| {
            "retained original attempt deadline exhausted; no start/restore/resume authority".into()
        })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    ColdStart,
    Restore { checkpoint: PathBuf },
    Snapshot { checkpoint: PathBuf },
    Pause,
    Resume,
    Retire,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub operation: ContentId,
    pub session: RetainedSession,
    pub binding: Binding,
    pub action: Action,
}

/// The provider must atomically reserve the exact attempt/fence/current
/// placement AND enforce volume/external-effect exclusion until the callback
/// ends. Leases, local markers and authenticated network peers are insufficient.
/// The callback is FnOnce so a provider cannot replay a non-idempotent effect.
/// No Kiln-local implementation pretends to supply this distributed authority.
pub trait RetainedAuthority {
    fn with_reservation<T>(
        &mut self,
        operation: &Operation,
        effect: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String>;
}

/// Current selected Aspen service has trigger ingress/reconcile, not retained
/// placement CAS, volume fencing, or admitted migration streams. Fail BEFORE
/// journal, socket, process or cleanup effects; never degrade to a local lock.
#[derive(Debug, Default)]
pub struct SelectedAspenAuthority;
impl RetainedAuthority for SelectedAspenAuthority {
    fn with_reservation<T>(
        &mut self,
        _: &Operation,
        _: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        Err(format!(
            "Aspen {SELECTED_MOLTEN_REVISION} lacks qualified retained placement/resource fencing; retained effects refused"
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Observed(Box<RetainedSession>),
    /// Persisted intent without a conclusive host observation. Duplicate calls
    /// return this observation without redispatch, regardless of sender exit.
    ReconcileRequired,
}

fn durable_database(db: &Connection) -> Result<(), String> {
    let synchronous: u32 = db
        .pragma_query_value(None, "synchronous", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let journal: String = db
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if synchronous < 2 || matches!(journal.as_str(), "off" | "memory") {
        return Err(
            "retained intents require a file-backed SQLite journal with synchronous FULL or EXTRA"
                .into(),
        );
    }
    Ok(())
}

pub fn init(db: &Connection) -> Result<(), String> {
    durable_database(db)?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_retained_session(
        session TEXT PRIMARY KEY, host TEXT NOT NULL, slot INTEGER NOT NULL,
        occupied INTEGER NOT NULL, revision INTEGER NOT NULL, record TEXT NOT NULL, binding TEXT NOT NULL, checkpoint TEXT);
        CREATE UNIQUE INDEX IF NOT EXISTS hosted_retained_occupied_slot
        ON hosted_retained_session(host,slot) WHERE occupied=1;
        CREATE TABLE IF NOT EXISTS hosted_retained_operation(
        operation TEXT PRIMARY KEY, session TEXT NOT NULL, request TEXT NOT NULL,
        outcome TEXT, diagnostic TEXT, FOREIGN KEY(session) REFERENCES hosted_retained_session(session));
        CREATE TABLE IF NOT EXISTS hosted_retained_resource(
        resource TEXT PRIMARY KEY, session TEXT NOT NULL,
        FOREIGN KEY(session) REFERENCES hosted_retained_session(session));")
        .map_err(|e| e.to_string())
}

fn owner(session: &RetainedSession) -> OwnerToken {
    OwnerToken {
        session: session.identity.session.clone(),
        fence: session.identity.fence,
        host: session.placement.host.clone(),
        generation: session.placement.generation,
    }
}

fn validate_binding(operation: &Operation) -> Result<(), String> {
    operation
        .operation
        .validate()
        .map_err(|e| format!("{e:?}"))?;
    operation
        .binding
        .profile
        .validate()
        .map_err(|e| format!("{e:?}"))?;
    let binding = &operation.binding;
    let normalized = |path: &Path| {
        path.is_absolute()
            && path.components().all(|part| {
                matches!(
                    part,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
    };
    if !normalized(&binding.directory)
        || !normalized(&binding.workspace)
        || !crate::identifier(&binding.unit)
        || !binding.unit.starts_with("kiln-retained-")
        || binding.disk_generation == 0
        || binding.deadline_unix_ms == 0
        || !binding.workspace.starts_with(&binding.directory)
        || binding.directory.as_os_str().len() + 16 > super::MAX_SOCKET_PATH_BYTES
    {
        return Err("retained binding is not an exact owned private VM resource".into());
    }
    Ok(())
}

/// Register one already admitted provider reservation exactly once. The unique
/// occupied-slot index includes reconciliation/unknown rows. This ledger records
/// capacity evidence; it does not allocate distributed placement generations.
pub fn register<A: RetainedAuthority>(
    db: &mut Connection,
    authority: &mut A,
    operation: &Operation,
) -> Result<(), String> {
    validate_binding(operation)?;
    let admitted = RetainedSession::admit(
        operation.session.identity.clone(),
        operation.session.placement.clone(),
        operation.session.resources,
    )
    .map_err(|e| format!("{e:?}"))?;
    if admitted != operation.session {
        return Err("retained registration contains unadmitted lifecycle fields".into());
    }
    if operation.session.state != RetainedState::Receiving {
        return Err("new retained registration requires receiving state".into());
    }
    authority.with_reservation(operation, || {
        let session = &operation.session;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO hosted_retained_session(session,host,slot,occupied,revision,record,binding) VALUES(?1,?2,?3,1,0,?4,?5)", params![
            session.identity.session.as_str(), session.placement.host, session.resources.slot,
            serde_json::to_string(session).map_err(|e| e.to_string())?,
            serde_json::to_string(&operation.binding).map_err(|e| e.to_string())?
        ]).map_err(|e| e.to_string())?;
        for resource in [operation.binding.directory.to_string_lossy(),
            operation.binding.workspace.to_string_lossy(),std::borrow::Cow::Borrowed(operation.binding.unit.as_str())] {
            tx.execute("INSERT INTO hosted_retained_resource VALUES(?1,?2)",params![resource.as_ref(),session.identity.session.as_str()]).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Reacquire local capacity for a checkpoint captured while paused and never
/// resumed since. This cannot resurrect a migrated-away source, change original
/// identity/deadline/placement, or cold boot over accepted execution history.
pub fn reopen_retired_checkpoint<A: RetainedAuthority>(
    db: &mut Connection,
    authority: &mut A,
    operation: &Operation,
) -> Result<(), String> {
    validate_binding(operation)?;
    durable_database(db)?;
    let Action::Restore { checkpoint } = &operation.action else {
        return Err("retired execution may only restore its retained checkpoint".into());
    };
    let admitted = RetainedSession::admit(
        operation.session.identity.clone(),
        operation.session.placement.clone(),
        operation.session.resources,
    )
    .map_err(|e| format!("{e:?}"))?;
    if admitted != operation.session {
        return Err("restore reacquisition requires a fresh receiving state".into());
    }
    authority.with_reservation(operation,|| {
        let (record,binding,saved): (String,String,Option<String>) = db.query_row(
            "SELECT record,binding,checkpoint FROM hosted_retained_session WHERE session=?1",
            [operation.session.identity.session.as_str()],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(|e| e.to_string())?;
        let previous: RetainedSession = serde_json::from_str(&record).map_err(|e| e.to_string())?;
        if previous.state != RetainedState::Retired || previous.migration.is_some()
            || previous.identity != operation.session.identity || previous.placement != operation.session.placement
            || previous.resources != operation.session.resources
            || binding != serde_json::to_string(&operation.binding).map_err(|e| e.to_string())?
            || saved.as_deref() != checkpoint.to_str() {
            return Err("retired checkpoint was consumed, moved, changed or lacks observed source retirement".into());
        }
        remaining_lifetime(operation.binding.deadline_unix_ms,crate::now_ms()?,u64::MAX)?;
        super::paired::verify(operation,checkpoint)?;
        super::verify_pool_directory(&operation.binding.directory)?;
        let owner = fs::metadata("/proc/self").map_err(|e| e.to_string())?.uid();
        let sockets = [operation.binding.directory.join("api.sock"),operation.binding.directory.join("vsock.sock")];
        for path in &sockets {
            match fs::symlink_metadata(path) {
                Ok(metadata) if metadata.file_type().is_socket() && metadata.uid() == owner => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                _ => return Err("retired socket resource binding changed; reconcile before restore".into()),
            }
        }
        remaining_lifetime(operation.binding.deadline_unix_ms,crate::now_ms()?,u64::MAX)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
        let changed = tx.execute("UPDATE hosted_retained_session SET record=?2,occupied=1,revision=revision+1 WHERE session=?1 AND record=?3 AND occupied=0",
            params![operation.session.identity.session.as_str(),serde_json::to_string(&operation.session).map_err(|e| e.to_string())?,record]).map_err(|e| e.to_string())?;
        if changed != 1 { return Err("retired checkpoint reacquisition raced; reconcile".into()); }
        tx.commit().map_err(|e| e.to_string())?;
        // Observed retirement and the still-held provider resource reservation
        // permit removing only these old VMM socket inodes. Workspace/checkpoint
        // data and the host/guest gateway's separate listener are never deleted.
        let [api,vsock] = &sockets;
        super::remove_stale(&[api.as_path(),vsock.as_path()])?;
        fs::File::open(&operation.binding.directory).and_then(|file| file.sync_all()).map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Execute one current operation, durably journalling before every host effect.
/// An exact repeated operation reconciles its retained result, never repeats it.
/// A crash anywhere after journal commit leaves its capacity occupied.
// r[impl kiln.hosted.mobility.runtime.scenario.replay]
pub fn execute<A: RetainedAuthority>(
    db: &mut Connection,
    config: &Config,
    backend: &Backend,
    authority: &mut A,
    operation: &Operation,
) -> Result<Outcome, String> {
    execute_with_effect(db, authority, operation, || {
        host_effect(config, backend, operation)
    })
}

fn execute_with_effect<A: RetainedAuthority>(
    db: &mut Connection,
    authority: &mut A,
    operation: &Operation,
    effect: impl FnOnce() -> Result<(), String>,
) -> Result<Outcome, String> {
    validate_binding(operation)?;
    durable_database(db)?;
    let request = serde_json::to_string(operation).map_err(|e| e.to_string())?;
    authority.with_reservation(operation, || {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
        let previous: Option<(String, Option<String>)> = tx.query_row(
            "SELECT request,outcome FROM hosted_retained_operation WHERE operation=?1",
            [operation.operation.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional().map_err(|e| e.to_string())?;
        if let Some((original, outcome)) = previous {
            if original != request { return Err("retained operation ID reused with a changed body".into()); }
            return outcome.map_or(Ok(Outcome::ReconcileRequired), |value|
                serde_json::from_str(&value).map_err(|e| e.to_string()));
        }
        let (record, binding, checkpoint): (String, String, Option<String>) = tx.query_row(
            "SELECT record,binding,checkpoint FROM hosted_retained_session WHERE session=?1",
            [operation.session.identity.session.as_str()], |row| Ok((row.get(0)?, row.get(1)?,row.get(2)?)))
            .map_err(|e| e.to_string())?;
        let mut session: RetainedSession = serde_json::from_str(&record).map_err(|e| e.to_string())?;
        if session != operation.session || binding != serde_json::to_string(&operation.binding).map_err(|e| e.to_string())? {
            return Err("retained operation has stale state, owner, fence or local resource binding".into());
        }
        if session.state == RetainedState::Receiving && checkpoint.as_ref().is_some_and(|checkpoint|
            !matches!(&operation.action,Action::Restore {checkpoint: requested} if requested.to_str() == Some(checkpoint.as_str()))) {
            return Err("reopened execution must restore its exact non-rewound checkpoint, not cold start".into());
        }
        let pending: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM hosted_retained_operation WHERE session=?1 AND outcome IS NULL)",
            [session.identity.session.as_str()], |row| row.get(0)).map_err(|e| e.to_string())?;
        if pending { return Ok(Outcome::ReconcileRequired); }
        let token = owner(&session);
        match operation.action {
            Action::ColdStart | Action::Restore { .. } => { session.begin_restore(&token).map(|_| ()) },
            Action::Pause => session.pause(&token).map(|_| ()),
            Action::Resume => session.resume(&token).map(|_| ()),
            Action::Retire => session.begin_retire(&token).map(|_| ()),
            Action::Snapshot { .. } if session.state == RetainedState::Paused => Ok(()),
            Action::Snapshot { .. } => return Err("paired snapshot requires an already paused session".into()),
        }.map_err(|e| format!("{e:?}"))?;
        tx.execute("INSERT INTO hosted_retained_operation(operation,session,request,outcome) VALUES(?1,?2,?3,NULL)",
            params![operation.operation.as_str(), session.identity.session.as_str(), request]).map_err(|e| e.to_string())?;
        let consumes_checkpoint = matches!(operation.action,Action::Resume | Action::ColdStart | Action::Restore {..});
        tx.execute("UPDATE hosted_retained_session SET revision=revision+1,record=?2,checkpoint=CASE WHEN ?3 THEN NULL ELSE checkpoint END WHERE session=?1",
            params![session.identity.session.as_str(), serde_json::to_string(&session).map_err(|e| e.to_string())?,consumes_checkpoint])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        let effect = effect();
        let diagnostic = effect.as_ref().err().map(|error| error.chars().take(MAX_DIAGNOSTIC_CHARS)
            .map(|character| if character.is_control() {' '} else {character}).collect::<String>());
        match (&operation.action, effect) {
            (Action::ColdStart | Action::Restore { .. }, Ok(())) => { session.observe_restored(&token, true).map_err(|e| format!("{e:?}"))?; },
            (Action::Retire, Ok(())) => session.observe_retired(&token, true).map_err(|e| format!("{e:?}"))?,
            (_, Ok(())) => (),
            (_, Err(_)) => { session.require_reconciliation(&token, "host_effect_or_observation_unknown").map_err(|e| format!("{e:?}"))?; },
        }
        let outcome = if session.state == RetainedState::ReconcileRequired { Outcome::ReconcileRequired } else { Outcome::Observed(Box::new(session.clone())) };
        let checkpoint = if session.state == RetainedState::Paused {
            match &operation.action { Action::Snapshot {checkpoint} | Action::Restore {checkpoint} => checkpoint.to_str(), _ => None }
        } else { None };
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
        tx.execute("UPDATE hosted_retained_session SET revision=revision+1,record=?2,occupied=?3,checkpoint=COALESCE(?4,checkpoint) WHERE session=?1", params![
            session.identity.session.as_str(), serde_json::to_string(&session).map_err(|e| e.to_string())?, session.occupies_capacity(),checkpoint])
            .map_err(|e| e.to_string())?;
        tx.execute("UPDATE hosted_retained_operation SET outcome=?2,diagnostic=?3 WHERE operation=?1", params![
            operation.operation.as_str(), serde_json::to_string(&outcome).map_err(|e| e.to_string())?,diagnostic]).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(outcome)
    })
}

fn api(socket: &Path, pid: u32, method: &str, path: &str, body: &Value) -> Result<Value, String> {
    let deadline = Instant::now() + API_TIMEOUT;
    let metadata = loop {
        match fs::symlink_metadata(socket) {
            Ok(metadata) => break metadata,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && Instant::now() < deadline =>
            {
                std::thread::sleep(super::POLL)
            }
            Err(error) => return Err(error.to_string()),
        }
    };
    let owner = fs::metadata("/proc/self").map_err(|e| e.to_string())?.uid();
    if !metadata.file_type().is_socket() || metadata.uid() != owner || metadata.mode() & 0o077 != 0
    {
        return Err("retained API path is not an exact protected service-owned Unix socket".into());
    }
    let fd = rustix::net::socket_with(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::STREAM,
        rustix::net::SocketFlags::CLOEXEC | rustix::net::SocketFlags::NONBLOCK,
        None,
    )
    .map_err(|e| e.to_string())?;
    let address = rustix::net::SocketAddrUnix::new(socket).map_err(|e| e.to_string())?;
    loop {
        if Instant::now() >= deadline {
            return Err("retained API connect deadline".into());
        }
        match rustix::net::connect(&fd, &address) {
            Ok(()) | Err(rustix::io::Errno::ISCONN) => break,
            Err(
                rustix::io::Errno::AGAIN
                | rustix::io::Errno::INPROGRESS
                | rustix::io::Errno::ALREADY
                | rustix::io::Errno::INTR,
            ) => std::thread::sleep(super::POLL),
            Err(error) => return Err(error.to_string()),
        }
    }
    let mut stream = UnixStream::from(fd);
    let peer = rustix::net::sockopt::socket_peercred(&stream).map_err(|e| e.to_string())?;
    if peer.uid.as_raw() != owner || u32::try_from(peer.pid.as_raw_pid()).ok() != Some(pid) {
        return Err("retained API peer is not the exact confined VMM process".into());
    }
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or("retained API write deadline")?;
    stream
        .set_read_timeout(Some(remaining))
        .and_then(|()| stream.set_write_timeout(Some(remaining)))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(&kiln_microvm::plan::api_request(method, path, body))
        .map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        if kiln_microvm::plan::api_response_length(&response)
            .is_some_and(|length| response.len() >= length)
        {
            break;
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("retained API deadline")?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|e| e.to_string())?;
        let count = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        if response.len() + count > API_RESPONSE_BYTES {
            return Err("retained API response exceeds bound".into());
        }
        response.extend_from_slice(buffer.get(..count).ok_or("retained API read bound")?);
    }
    if kiln_microvm::plan::api_response_length(&response) != Some(response.len()) {
        return Err("retained API response was incomplete or ambiguously framed".into());
    }
    if !kiln_microvm::plan::api_status(&response).is_some_and(|status| (200..300).contains(&status))
    {
        return Err("retained API mutation/observation not confirmed; reconcile".into());
    }
    let body = response
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .and_then(|at| response.get(at + 4..))
        .unwrap_or_default();
    if body.is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_slice(body).map_err(|e| e.to_string())
    }
}

fn observe_state(socket: &Path, pid: u32, expected: &str) -> Result<(), String> {
    if api(socket, pid, "GET", "/api/v1/vm.info", &Value::Null)?
        .get("state")
        .and_then(Value::as_str)
        != Some(expected)
    {
        return Err("retained VMM state was not observed".into());
    }
    Ok(())
}

fn host_effect(config: &Config, backend: &Backend, operation: &Operation) -> Result<(), String> {
    if !matches!(operation.action, Action::Retire) {
        remaining_lifetime(
            operation.binding.deadline_unix_ms,
            crate::now_ms()?,
            config.maximum_timeout_ms,
        )?;
    }
    super::verify_pool_directory(&operation.binding.directory)?;
    super::admit_pool_path(&operation.binding.directory, &config.work_root)?;
    if fs::canonicalize(&operation.binding.directory).map_err(|e| e.to_string())?
        != operation.binding.directory
    {
        return Err("retained resource directory contains an indirect path".into());
    }
    let runner = Runner::from_cloud_hypervisor(backend)
        .ok_or("retained execution requires existing CH backend")?;
    let binary = runner.vmm.verify()?;
    let api_socket = operation.binding.directory.join("api.sock");
    let observed = observed_profile(config, backend)?;
    if observed.identity != operation.binding.profile
        || operation.session.resources.vcpus != u16::from(runner.policy.vcpus)
        || operation.session.resources.memory_mib != runner.policy.memory_mib
    {
        return Err("retained actual VMM/CPU/memory/image profile mismatch".into());
    }
    let profile_path = operation
        .binding
        .directory
        .join(format!("{}.profile.json", observed.identity.as_str()));
    let profile_bytes = serde_json::to_vec(&observed.facts).map_err(|e| e.to_string())?;
    if profile_bytes.len() as u64 > CPU_FACT_BYTES {
        return Err("runtime profile evidence exceeds bound".into());
    }
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&profile_path)
    {
        Ok(mut file) => {
            file.write_all(&profile_bytes)
                .and_then(|()| file.sync_all())
                .map_err(|e| e.to_string())?;
            fs::File::open(&operation.binding.directory)
                .and_then(|file| file.sync_all())
                .map_err(|e| e.to_string())?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut bytes = Vec::new();
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&profile_path)
                .map_err(|e| e.to_string())?;
            (&file)
                .take(CPU_FACT_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            fs::File::open(&operation.binding.directory)
                .and_then(|file| file.sync_all())
                .map_err(|e| e.to_string())?;
            if bytes != profile_bytes {
                return Err("retained runtime profile evidence changed or incomplete".into());
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    let token = &operation.binding.unit;
    if matches!(operation.action, Action::ColdStart | Action::Restore { .. }) {
        runner.kernel.verify()?;
        runner.initrd.verify()?;
        if api_socket.exists() {
            return Err("retained API resource already exists; reconcile before spawn".into());
        }
        let mut service = unit_command(config, token, config.maximum_timeout_ms, true)?;
        service.retain(|arg| arg != "--wait" && arg != "--pipe");
        service.insert(0, OsString::from("--property=Restart=no"));
        service.insert(0, OsString::from("--property=UMask=0077"));
        let mut args = match &operation.action {
            Action::Restore { checkpoint } => {
                if !checkpoint.is_absolute()
                    || !checkpoint.starts_with(&operation.binding.directory)
                {
                    return Err("restore checkpoint outside owned resource".into());
                }
                super::paired::verify(operation, checkpoint)?;
                cloud_hypervisor_restore(
                    super::text(&binary)?,
                    super::text(checkpoint)?,
                    super::text(&api_socket)?,
                )
            }
            _ => vec![
                super::text(&binary)?.into(),
                "--api-socket".into(),
                format!("path={}", super::text(&api_socket)?),
            ],
        };
        args.extend(["--seccomp".into(), "true".into()]);
        service.extend(args.into_iter().map(OsString::from));
        let lifetime = remaining_lifetime(
            operation.binding.deadline_unix_ms,
            crate::now_ms()?,
            config.maximum_timeout_ms,
        )?;
        service.retain(|arg| {
            !arg.to_str()
                .is_some_and(|arg| arg.starts_with("--property=RuntimeMaxSec="))
        });
        service.insert(0, format!("--property=RuntimeMaxSec={lifetime}ms").into());
        let request = manager_request(
            config,
            config.systemd_run.verify()?,
            service,
            runner.policy.boot_timeout_ms,
            super::CONSOLE_BYTES,
            Input::Null,
        )?;
        let output = bounded_exec::run(request).map_err(|e| e.to_string())?;
        if output.completion != Completion::Exited || output.exit_code != Some(0) {
            return Err("retained launch outcome unknown".into());
        }
        let confinement = observe_confinement(
            config,
            token,
            "cloud-hypervisor",
            &binary,
            &runner.vmm.blake3,
            true,
            Duration::from_millis(runner.policy.boot_timeout_ms),
        )?;
        let pid = confinement.pid;
        if matches!(operation.action, Action::ColdStart) {
            let workspace = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&operation.binding.workspace)
                .map_err(|e| e.to_string())?;
            if !workspace.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err("retained workspace must be a raw regular image".into());
            }
            api(
                &api_socket,
                pid,
                "PUT",
                "/api/v1/vm.create",
                &json!({
                    "cpus": {"boot_vcpus": runner.policy.vcpus, "max_vcpus": runner.policy.vcpus},
                    "memory": {"size": u64::from(runner.policy.memory_mib) * kiln_microvm::MIB,
                        "shared": false, "hugepages": false, "mergeable": false},
                    "payload": {"kernel": runner.kernel.path, "initramfs": runner.initrd.path,
                        "cmdline": kiln_microvm::backend::CloudHypervisor.boot_args()},
                    "disks": [{"path": operation.binding.workspace, "readonly": false, "image_type": "Raw"}],
                    "vsock": {"cid": kiln_microvm::GUEST_CID, "socket": operation.binding.directory.join("vsock.sock")},
                    "serial": {"mode": "Null"}, "console": {"mode": "Off"}
                }),
            )?;
            // Guest boot is not workflow dispatch. No host/guest job message is
            // sent by this controller; Lattice's admitted guest provider owns it.
            api(&api_socket, pid, "PUT", "/api/v1/vm.boot", &Value::Null)?;
            api(&api_socket, pid, "PUT", "/api/v1/vm.pause", &Value::Null)?;
            observe_confinement(
                config,
                token,
                "cloud-hypervisor",
                &binary,
                &runner.vmm.blake3,
                true,
                API_TIMEOUT,
            )?;
        }
        return observe_state(&api_socket, pid, "Paused");
    }
    let pid = observe_confinement(
        config,
        token,
        "cloud-hypervisor",
        &binary,
        &runner.vmm.blake3,
        true,
        API_TIMEOUT,
    )?
    .pid;
    match &operation.action {
        Action::Pause => {
            api(&api_socket, pid, "PUT", "/api/v1/vm.pause", &Value::Null)?;
            observe_state(&api_socket, pid, "Paused")
        }
        Action::Resume => {
            observe_state(&api_socket, pid, "Paused")?;
            remaining_lifetime(
                operation.binding.deadline_unix_ms,
                crate::now_ms()?,
                config.maximum_timeout_ms,
            )?;
            api(&api_socket, pid, "PUT", "/api/v1/vm.resume", &Value::Null)?;
            observe_state(&api_socket, pid, "Running")
        }
        Action::Snapshot { checkpoint } => {
            observe_state(&api_socket, pid, "Paused")?;
            if !checkpoint.starts_with(&operation.binding.directory)
                || checkpoint.components().any(|part| {
                    !matches!(
                        part,
                        std::path::Component::RootDir | std::path::Component::Normal(_)
                    )
                })
            {
                return Err("snapshot destination escapes the exact owned VM directory".into());
            }
            super::verify_pool_directory(checkpoint)?;
            if checkpoint.join("kiln-paired-checkpoint.json").exists() {
                return Err("checkpoint already consumed; reconcile".into());
            }
            api(
                &api_socket,
                pid,
                "PUT",
                "/api/v1/vm.snapshot",
                &json!({"destination_url":format!("file://{}", super::text(checkpoint)?)}),
            )?;
            observe_state(&api_socket, pid, "Paused")?;
            super::paired::publish(operation, checkpoint)
        }
        Action::Retire => {
            observe_state(&api_socket, pid, "Paused")?;
            if cleanup_unit(config, token) {
                Ok(())
            } else {
                Err("retained source cleanup unknown".into())
            }
        }
        _ => Err("unsupported retained host effect".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_microvm::retained::{Placement, RetainedResources, SessionIdentity};
    fn database() -> (tempfile::TempDir, Connection) {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let db = Connection::open(root.path().join("retained.sqlite"))
            .unwrap_or_else(|_| unreachable!());
        (root, db)
    }
    fn operation() -> Operation {
        let id = |c: char| {
            ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
        };
        Operation {
            operation: id('a'),
            session: RetainedSession::admit(
                SessionIdentity {
                    session: id('b'),
                    execution: id('f'),
                    vm: id('1'),
                    work: id('c'),
                    work_revision: 1,
                    attempt: id('d'),
                    fence: 1,
                },
                Placement {
                    host: "host-a".into(),
                    generation: 1,
                },
                RetainedResources {
                    vcpus: 1,
                    memory_mib: 512,
                    slot: 1,
                },
            )
            .unwrap_or_else(|_| unreachable!()),
            binding: Binding {
                directory: "/run/kiln/vm-a".into(),
                unit: "kiln-retained-a".into(),
                workspace: "/run/kiln/vm-a/workspace.raw".into(),
                disk_generation: 1,
                deadline_unix_ms: u64::MAX,
                profile: id('e'),
            },
            action: Action::ColdStart,
        }
    }

    #[test]
    fn restored_or_paused_guest_cannot_extend_original_wall_deadline() {
        assert_eq!(remaining_lifetime(100, 90, 50), Ok(10));
        assert_eq!(remaining_lifetime(200, 90, 50), Ok(50));
        assert!(remaining_lifetime(100, 100, 50).is_err());
        assert!(remaining_lifetime(90, 100, 50).is_err());
        assert!(remaining_lifetime(200, 90, 0).is_err());
    }
    // r[verify kiln.hosted.mobility.runtime]
    #[test]
    fn selected_aspen_fails_before_callback_and_capacity_mutation() {
        let mut authority = SelectedAspenAuthority;
        let mut called = false;
        let result: Result<(), String> = authority.with_reservation(&operation(), || {
            called = true;
            Ok(())
        });
        assert!(result.is_err());
        assert!(!called);
        let (_root, mut db) = database();
        init(&db).unwrap_or_else(|_| unreachable!());
        assert!(register(&mut db, &mut authority, &operation()).is_err());
        let count: u32 = db
            .query_row("SELECT COUNT(*) FROM hosted_retained_session", [], |row| {
                row.get(0)
            })
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(count, 0);
    }

    // Pure journal fixture only: this is not a distributed authority provider.
    struct FixtureReservation;
    impl RetainedAuthority for FixtureReservation {
        fn with_reservation<T>(
            &mut self,
            _: &Operation,
            effect: impl FnOnce() -> Result<T, String>,
        ) -> Result<T, String> {
            effect()
        }
    }

    #[test]
    fn exact_replay_stale_body_and_unknown_host_effect_never_redispatch() {
        let (_root, mut db) = database();
        init(&db).unwrap_or_else(|_| unreachable!());
        let mut authority = FixtureReservation;
        let operation = operation();
        register(&mut db, &mut authority, &operation).unwrap_or_else(|_| unreachable!());
        let mut effects = 0;
        let first = execute_with_effect(&mut db, &mut authority, &operation, || {
            effects += 1;
            Ok(())
        })
        .unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(&first, Outcome::Observed(session) if session.state == RetainedState::Paused)
        );
        let second = execute_with_effect(&mut db, &mut authority, &operation, || {
            effects += 1;
            Ok(())
        })
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(first, second);
        assert_eq!(effects, 1);
        let mut changed = operation.clone();
        changed.binding.disk_generation += 1;
        assert!(
            execute_with_effect(&mut db, &mut authority, &changed, || {
                effects += 1;
                Ok(())
            })
            .is_err()
        );
        let Outcome::Observed(session) = first else {
            unreachable!()
        };
        changed = operation.clone();
        changed.session = *session;
        changed.action = Action::Resume;
        changed.operation =
            ContentId::from_blake3_hex("f".repeat(64)).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &changed, || {
                effects += 1;
                Err("lost response".into())
            })
            .unwrap_or_else(|_| unreachable!()),
            Outcome::ReconcileRequired
        );
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &changed, || {
                effects += 1;
                Ok(())
            })
            .unwrap_or_else(|_| unreachable!()),
            Outcome::ReconcileRequired
        );
        assert_eq!(effects, 2);
        let occupied: u32 = db
            .query_row("SELECT occupied FROM hosted_retained_session", [], |row| {
                row.get(0)
            })
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(occupied, 1);
    }

    // r[verify kiln.hosted.mobility.runtime.scenario.replay]
    #[test]
    fn pending_crash_and_duplicate_slot_preserve_reservation() {
        let (root, mut db) = database();
        init(&db).unwrap_or_else(|_| unreachable!());
        let mut authority = FixtureReservation;
        let operation = operation();
        register(&mut db, &mut authority, &operation).unwrap_or_else(|_| unreachable!());
        assert!(register(&mut db, &mut authority, &operation).is_err());
        let request = serde_json::to_string(&operation).unwrap_or_else(|_| unreachable!());
        db.execute("INSERT INTO hosted_retained_operation(operation,session,request,outcome) VALUES(?1,?2,?3,NULL)",params![operation.operation.as_str(),operation.session.identity.session.as_str(),request]).unwrap_or_else(|_| unreachable!());
        drop(db);
        let mut db = Connection::open(root.path().join("retained.sqlite"))
            .unwrap_or_else(|_| unreachable!());
        let mut effects = 0;
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &operation, || {
                effects += 1;
                Ok(())
            })
            .unwrap_or_else(|_| unreachable!()),
            Outcome::ReconcileRequired
        );
        assert_eq!(effects, 0);
        assert!(operation.session.occupies_capacity());
    }
}

#[cfg(test)]
mod api_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    #[test]
    fn exact_local_peer_is_required_before_request_bytes_and_truncation_is_unknown() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = root.path().join("api.sock");
        let listener = UnixListener::bind(&path).unwrap_or_else(|_| unreachable!());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .unwrap_or_else(|_| unreachable!());
        let server = std::thread::spawn(move || {
            for index in 0..3 {
                let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!());
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap_or_else(|_| unreachable!());
                let mut request = [0_u8; 4096];
                let count = stream.read(&mut request).unwrap_or_else(|_| unreachable!());
                if index == 1 {
                    assert_eq!(
                        count, 0,
                        "wrong local VMM peer must receive no command bytes"
                    );
                    continue;
                }
                assert!(
                    request
                        .get(..count)
                        .unwrap_or_default()
                        .starts_with(b"PUT /api/v1/vm.pause ")
                );
                let response: &[u8] = if index == 0 {
                    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n"
                } else {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n{}"
                };
                stream
                    .write_all(response)
                    .unwrap_or_else(|_| unreachable!());
            }
        });
        let pid = std::process::id();
        assert_eq!(
            api(&path, pid, "PUT", "/api/v1/vm.pause", &Value::Null),
            Ok(Value::Null)
        );
        assert!(api(&path, pid + 1, "PUT", "/api/v1/vm.pause", &Value::Null).is_err());
        assert!(api(&path, pid, "PUT", "/api/v1/vm.pause", &Value::Null).is_err());
        server.join().unwrap_or_else(|_| unreachable!());
    }
}

#[cfg(test)]
mod checkpoint_reopen_tests {
    use super::*;
    use kiln_microvm::retained::{Placement, RetainedResources, SessionIdentity};
    use std::os::unix::fs::PermissionsExt;

    // Journal/file fixture, never a runtime authority or real VMM qualification.
    struct FixtureReservation;
    impl RetainedAuthority for FixtureReservation {
        fn with_reservation<T>(
            &mut self,
            _: &Operation,
            effect: impl FnOnce() -> Result<T, String>,
        ) -> Result<T, String> {
            effect()
        }
    }
    fn observed(outcome: Outcome) -> RetainedSession {
        match outcome {
            Outcome::Observed(session) => *session,
            Outcome::ReconcileRequired => unreachable!("fixture effect uncertain"),
        }
    }
    #[test]
    fn retired_checkpoint_reopens_once_but_any_resume_consumes_old_memory_authority() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|_| unreachable!());
        let mut db =
            Connection::open(root.path().join("journal.sqlite")).unwrap_or_else(|_| unreachable!());
        init(&db).unwrap_or_else(|_| unreachable!());
        let id = |c: char| {
            ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
        };
        let session = RetainedSession::admit(
            SessionIdentity {
                session: id('a'),
                execution: id('b'),
                vm: id('c'),
                work: id('d'),
                work_revision: 1,
                attempt: id('e'),
                fence: 1,
            },
            Placement {
                host: "host-a".into(),
                generation: 1,
            },
            RetainedResources {
                vcpus: 1,
                memory_mib: 512,
                slot: 1,
            },
        )
        .unwrap_or_else(|_| unreachable!());
        let mut operation = Operation {
            operation: id('1'),
            session,
            binding: Binding {
                directory: root.path().into(),
                unit: "kiln-retained-test".into(),
                workspace: root.path().join("workspace.raw"),
                disk_generation: 1,
                deadline_unix_ms: u64::MAX,
                profile: id('f'),
            },
            action: Action::ColdStart,
        };
        let mut authority = FixtureReservation;
        register(&mut db, &mut authority, &operation).unwrap_or_else(|_| unreachable!());
        operation.session = observed(
            execute_with_effect(&mut db, &mut authority, &operation, || Ok(()))
                .unwrap_or_else(|_| unreachable!()),
        );
        let checkpoint = root.path().join("checkpoint");
        fs::create_dir(&checkpoint).unwrap_or_else(|_| unreachable!());
        fs::set_permissions(&checkpoint, fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|_| unreachable!());
        for name in ["config.json", "state.json", "memory-ranges"] {
            fs::write(checkpoint.join(name), name).unwrap_or_else(|_| unreachable!());
        }
        fs::write(&operation.binding.workspace, b"counter=7\n").unwrap_or_else(|_| unreachable!());
        operation.operation = id('2');
        operation.action = Action::Snapshot {
            checkpoint: checkpoint.clone(),
        };
        operation.session = observed(
            execute_with_effect(&mut db, &mut authority, &operation, || {
                super::super::paired::publish(&operation, &checkpoint)
            })
            .unwrap_or_else(|_| unreachable!()),
        );
        operation.operation = id('3');
        operation.action = Action::Retire;
        operation.session = observed(
            execute_with_effect(&mut db, &mut authority, &operation, || Ok(()))
                .unwrap_or_else(|_| unreachable!()),
        );
        operation.session = RetainedSession::admit(
            operation.session.identity.clone(),
            operation.session.placement.clone(),
            operation.session.resources,
        )
        .unwrap_or_else(|_| unreachable!());
        operation.operation = id('4');
        operation.action = Action::Restore {
            checkpoint: checkpoint.clone(),
        };
        reopen_retired_checkpoint(&mut db, &mut authority, &operation)
            .unwrap_or_else(|_| unreachable!());
        assert!(reopen_retired_checkpoint(&mut db, &mut authority, &operation).is_err());
        operation.session = observed(
            execute_with_effect(&mut db, &mut authority, &operation, || Ok(()))
                .unwrap_or_else(|_| unreachable!()),
        );
        for (operation_id, action) in [
            ('5', Action::Resume),
            ('6', Action::Pause),
            ('7', Action::Retire),
        ] {
            operation.operation = id(operation_id);
            operation.action = action;
            operation.session = observed(
                execute_with_effect(&mut db, &mut authority, &operation, || Ok(()))
                    .unwrap_or_else(|_| unreachable!()),
            );
        }
        operation.session = RetainedSession::admit(
            operation.session.identity.clone(),
            operation.session.placement.clone(),
            operation.session.resources,
        )
        .unwrap_or_else(|_| unreachable!());
        operation.operation = id('8');
        operation.action = Action::Restore { checkpoint };
        assert!(
            reopen_retired_checkpoint(&mut db, &mut authority, &operation).is_err(),
            "unchanged workspace alone cannot justify rewinding memory after a resume"
        );
    }
}
