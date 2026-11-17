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
use kiln_microvm::retained::{ControlKind, OwnerToken, RetainedSession, RetainedState};
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum EffectEvidence {
    VmmState(String),
    CleanupObserved,
    Rejected,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectMode {
    Dispatch,
    ObserveOnly,
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
    execute_with_effect(db, authority, operation, |mode| match mode {
        EffectMode::Dispatch => host_effect(config, backend, operation),
        EffectMode::ObserveOnly => host_control_observation(config, backend, operation),
    })
}

/// Pause and resume adopt their target state only from the exact unfinished
/// operation's native readback after the confinement barrier; any other reply
/// leaves the control uncertain or is refused before the host is touched.
// r[impl kiln.hosted.mobility.runtime.scenario.control] r[impl kiln.hosted.mobility.runtime.scenario.control-observed]
fn execute_with_effect<A: RetainedAuthority>(
    db: &mut Connection,
    authority: &mut A,
    operation: &Operation,
    effect: impl FnOnce(EffectMode) -> Result<EffectEvidence, String>,
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
            let old_outcome = outcome.map(|value| serde_json::from_str::<Outcome>(&value).map_err(|e| e.to_string())).transpose()?;
            if !matches!(operation.action, Action::Pause | Action::Resume) || old_outcome.as_ref().is_some_and(|value| *value != Outcome::ReconcileRequired) {
                return Ok(old_outcome.unwrap_or(Outcome::ReconcileRequired));
            }
            let (current, binding): (String, String) = tx.query_row("SELECT record,binding FROM hosted_retained_session WHERE session=?1", [operation.session.identity.session.as_str()], |row| Ok((row.get(0)?, row.get(1)?))).map_err(|e| e.to_string())?;
            if binding != serde_json::to_string(&operation.binding).map_err(|e| e.to_string())? { return Err("retained control binding changed during reconciliation".into()); }
            let mut session: RetainedSession = serde_json::from_str(&current).map_err(|e| e.to_string())?;
            if session.identity != operation.session.identity || session.placement != operation.session.placement || session.resources != operation.session.resources {
                return Err("late control reply cannot affect a successor owner or placement".into());
            }
            let control = session.control.as_ref().ok_or("unfinished control record is missing")?;
            if control.operation != operation.operation || !matches!((control.kind, &operation.action), (ControlKind::Pause, Action::Pause) | (ControlKind::Resume, Action::Resume)) {
                return Err("control reconciliation is crossed with another operation".into());
            }
            let control_kind = control.kind;
            drop(tx);
            let token = owner(&session);
            match effect(EffectMode::ObserveOnly) {
                Ok(EffectEvidence::VmmState(state)) => session.observe_control(&token, &operation.operation,
                    matches!((control_kind, state.as_str()), (ControlKind::Pause, "Paused") | (ControlKind::Resume, "Running"))).map_err(|e| format!("{e:?}"))?,
                Ok(EffectEvidence::Rejected) => session.reject_control(&token, &operation.operation).map_err(|e| format!("{e:?}"))?,
                _ => session.observe_control(&token, &operation.operation, false).map_err(|e| format!("{e:?}"))?,
            }
            let reconciled = if session.state == RetainedState::ReconcileRequired { Outcome::ReconcileRequired } else { Outcome::Observed(Box::new(session.clone())) };
            let update = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
            update.execute("UPDATE hosted_retained_session SET revision=revision+1,record=?2,occupied=?3 WHERE session=?1", params![session.identity.session.as_str(), serde_json::to_string(&session).map_err(|e| e.to_string())?, session.occupies_capacity()]).map_err(|e| e.to_string())?;
            update.execute("UPDATE hosted_retained_operation SET outcome=?2 WHERE operation=?1", params![operation.operation.as_str(), serde_json::to_string(&reconciled).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
            update.commit().map_err(|e| e.to_string())?;
            return Ok(reconciled);
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
            Action::ColdStart | Action::Restore { .. } => session.begin_restore(&token).map(|_| ()),
            Action::Pause => session.request_control(&token, operation.operation.clone(), ControlKind::Pause).map(|_| ()),
            Action::Resume => session.request_control(&token, operation.operation.clone(), ControlKind::Resume).map(|_| ()),
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
        if matches!(operation.action, Action::Pause | Action::Resume) {
            session.control_in_progress(&token, &operation.operation).map_err(|e| format!("{e:?}"))?;
            let progress = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
            progress.execute("UPDATE hosted_retained_session SET revision=revision+1,record=?2 WHERE session=?1",
                params![session.identity.session.as_str(), serde_json::to_string(&session).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
            progress.commit().map_err(|e| e.to_string())?;
        }
        let effect = effect(EffectMode::Dispatch);
        let diagnostic = effect.as_ref().err().map(|error| error.chars().take(MAX_DIAGNOSTIC_CHARS)
            .map(|character| if character.is_control() {' '} else {character}).collect::<String>());
        match (&operation.action, effect) {
            (Action::ColdStart | Action::Restore { .. }, Ok(EffectEvidence::VmmState(state))) if state == "Paused" => { session.observe_restored(&token, true).map_err(|e| format!("{e:?}"))?; },
            (Action::Pause, Ok(EffectEvidence::VmmState(state))) => { session.observe_control(&token, &operation.operation, state == "Paused").map_err(|e| format!("{e:?}"))?; },
            (Action::Resume, Ok(EffectEvidence::VmmState(state))) => { session.observe_control(&token, &operation.operation, state == "Running").map_err(|e| format!("{e:?}"))?; },
            (Action::Pause | Action::Resume, Ok(EffectEvidence::Rejected)) => session.reject_control(&token, &operation.operation).map_err(|e| format!("{e:?}"))?,
            (Action::Retire, Ok(EffectEvidence::CleanupObserved)) => session.observe_retired(&token, true).map_err(|e| format!("{e:?}"))?,
            (Action::Snapshot { .. }, Ok(EffectEvidence::VmmState(state))) if state == "Paused" => (),
            (_, Err(_)) if matches!(operation.action, Action::Pause | Action::Resume) => { session.observe_control(&token, &operation.operation, false).map_err(|e| format!("{e:?}"))?; },
            (_, Ok(_)) => { session.require_reconciliation(&token, "host_effect_observation_incomplete").map_err(|e| format!("{e:?}"))?; },
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
    if let Some(status) = kiln_microvm::plan::api_status(&response) {
        if !(200..300).contains(&status) {
            if (400..500).contains(&status) {
                return Err(format!("retained API rejected request with HTTP {status}"));
            }
            return Err("retained API mutation/observation not confirmed; reconcile".into());
        }
    } else {
        return Err("retained API response has no status".into());
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

fn observe_state(socket: &Path, pid: u32, expected: &str) -> Result<String, String> {
    let state = api(socket, pid, "GET", "/api/v1/vm.info", &Value::Null)?
        .get("state")
        .and_then(Value::as_str)
        .ok_or("retained VMM state is missing")?
        .to_string();
    if state != expected {
        return Err("retained VMM state was not observed".into());
    }
    Ok(state)
}

fn host_effect(
    config: &Config,
    backend: &Backend,
    operation: &Operation,
) -> Result<EffectEvidence, String> {
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
        return observe_state(&api_socket, pid, "Paused").map(EffectEvidence::VmmState);
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
        Action::Pause => match api(&api_socket, pid, "PUT", "/api/v1/vm.pause", &Value::Null) {
            Ok(_) => {
                let state = observe_state(&api_socket, pid, "Paused")?;
                observe_confinement(
                    config,
                    token,
                    "cloud-hypervisor",
                    &binary,
                    &runner.vmm.blake3,
                    true,
                    API_TIMEOUT,
                )?;
                Ok(EffectEvidence::VmmState(state))
            }
            Err(error) if error.starts_with("retained API rejected request") => {
                Ok(EffectEvidence::Rejected)
            }
            Err(error) => Err(error),
        },
        Action::Resume => {
            observe_state(&api_socket, pid, "Paused")?;
            remaining_lifetime(
                operation.binding.deadline_unix_ms,
                crate::now_ms()?,
                config.maximum_timeout_ms,
            )?;
            match api(&api_socket, pid, "PUT", "/api/v1/vm.resume", &Value::Null) {
                Ok(_) => {
                    let state = observe_state(&api_socket, pid, "Running")?;
                    observe_confinement(
                        config,
                        token,
                        "cloud-hypervisor",
                        &binary,
                        &runner.vmm.blake3,
                        true,
                        API_TIMEOUT,
                    )?;
                    Ok(EffectEvidence::VmmState(state))
                }
                Err(error) if error.starts_with("retained API rejected request") => {
                    Ok(EffectEvidence::Rejected)
                }
                Err(error) => Err(error),
            }
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
            super::paired::publish(operation, checkpoint)?;
            Ok(EffectEvidence::VmmState("Paused".into()))
        }
        Action::Retire => {
            observe_state(&api_socket, pid, "Paused")?;
            if cleanup_unit(config, token) {
                Ok(EffectEvidence::CleanupObserved)
            } else {
                Err("retained source cleanup unknown".into())
            }
        }
        _ => Err("unsupported retained host effect".into()),
    }
}

fn host_control_observation(
    config: &Config,
    backend: &Backend,
    operation: &Operation,
) -> Result<EffectEvidence, String> {
    if !matches!(operation.action, Action::Pause | Action::Resume) {
        return Err("observation-only reconciliation requires a pause/resume operation".into());
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
    let observed = observed_profile(config, backend)?;
    if observed.identity != operation.binding.profile
        || operation.session.resources.vcpus != u16::from(runner.policy.vcpus)
        || operation.session.resources.memory_mib != runner.policy.memory_mib
    {
        return Err("retained actual VMM/CPU/memory profile mismatch".into());
    }
    let pid = observe_confinement(
        config,
        &operation.binding.unit,
        "cloud-hypervisor",
        &binary,
        &runner.vmm.blake3,
        true,
        API_TIMEOUT,
    )?
    .pid;
    let state = api(
        &operation.binding.directory.join("api.sock"),
        pid,
        "GET",
        "/api/v1/vm.info",
        &Value::Null,
    )?
    .get("state")
    .and_then(Value::as_str)
    .ok_or("retained VMM state is missing")?
    .to_string();
    observe_confinement(
        config,
        &operation.binding.unit,
        "cloud-hypervisor",
        &binary,
        &runner.vmm.blake3,
        true,
        API_TIMEOUT,
    )?;
    Ok(EffectEvidence::VmmState(state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_microvm::retained::{ControlStatus, Placement, RetainedResources, SessionIdentity};
    fn id(c: char) -> ContentId {
        ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }
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
        let first = execute_with_effect(&mut db, &mut authority, &operation, |mode| {
            if mode == EffectMode::Dispatch {
                effects += 1;
            }
            Ok(EffectEvidence::VmmState("Paused".into()))
        })
        .unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(&first, Outcome::Observed(session) if session.state == RetainedState::Paused)
        );
        let second = execute_with_effect(&mut db, &mut authority, &operation, |_| {
            unreachable!("observed duplicate must not call host")
        })
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(first, second);
        assert_eq!(effects, 1);
        let mut changed = operation.clone();
        changed.binding.disk_generation += 1;
        assert!(
            execute_with_effect(&mut db, &mut authority, &changed, |_| {
                unreachable!("stale body must not call host")
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
            execute_with_effect(&mut db, &mut authority, &changed, |mode| {
                if mode == EffectMode::Dispatch {
                    effects += 1;
                }
                Err("lost response".into())
            })
            .unwrap_or_else(|_| unreachable!()),
            Outcome::ReconcileRequired
        );
        let recovered = execute_with_effect(&mut db, &mut authority, &changed, |mode| {
            assert_eq!(mode, EffectMode::ObserveOnly);
            Ok(EffectEvidence::VmmState("Running".into()))
        })
        .unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(recovered, Outcome::Observed(session) if session.state == RetainedState::Running)
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
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &operation, |_| {
                unreachable!("crashed non-control effect cannot be replayed")
            })
            .unwrap_or_else(|_| unreachable!()),
            Outcome::ReconcileRequired
        );
        assert!(operation.session.occupies_capacity());
    }
    #[test]
    fn serialized_pause_intent_reconciles_after_restart_without_redispatch() {
        let (root, mut db) = database();
        init(&db).unwrap_or_else(|_| unreachable!());
        let mut authority = FixtureReservation;
        let mut start = operation();
        register(&mut db, &mut authority, &start).unwrap_or_else(|_| unreachable!());
        start.session = match execute_with_effect(&mut db, &mut authority, &start, |_| {
            Ok(EffectEvidence::VmmState("Paused".into()))
        })
        .unwrap_or_else(|_| unreachable!())
        {
            Outcome::Observed(session) => *session,
            Outcome::ReconcileRequired => unreachable!(),
        };
        let id = |c: char| {
            ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
        };
        let mut resume = start.clone();
        resume.operation = id('c');
        resume.action = Action::Resume;
        resume.session = match execute_with_effect(&mut db, &mut authority, &resume, |_| {
            Ok(EffectEvidence::VmmState("Running".into()))
        })
        .unwrap_or_else(|_| unreachable!())
        {
            Outcome::Observed(session) => *session,
            Outcome::ReconcileRequired => unreachable!(),
        };
        let mut pause = resume.clone();
        pause.operation = id('d');
        pause.action = Action::Pause;
        let mut pending = pause.session.clone();
        let token = owner(&pending);
        pending
            .request_control(&token, pause.operation.clone(), ControlKind::Pause)
            .unwrap_or_else(|_| unreachable!());
        pending
            .control_in_progress(&token, &pause.operation)
            .unwrap_or_else(|_| unreachable!());
        let request = serde_json::to_string(&pause).unwrap_or_else(|_| unreachable!());
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap_or_else(|_| unreachable!());
        tx.execute("INSERT INTO hosted_retained_operation(operation,session,request,outcome) VALUES(?1,?2,?3,NULL)", params![pause.operation.as_str(), pause.session.identity.session.as_str(), request]).unwrap_or_else(|_| unreachable!());
        tx.execute(
            "UPDATE hosted_retained_session SET record=?2 WHERE session=?1",
            params![
                pending.identity.session.as_str(),
                serde_json::to_string(&pending).unwrap_or_else(|_| unreachable!())
            ],
        )
        .unwrap_or_else(|_| unreachable!());
        tx.commit().unwrap_or_else(|_| unreachable!());
        let mut snapshot = pause.clone();
        snapshot.operation = id('e');
        snapshot.session = pending.clone();
        snapshot.action = Action::Snapshot {
            checkpoint: root.path().join("pending-checkpoint"),
        };
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &snapshot, |_| unreachable!(
                "pending pause blocks checkpoint"
            ))
            .unwrap_or_else(|_| unreachable!()),
            Outcome::ReconcileRequired
        );
        drop(db);
        let mut db = Connection::open(root.path().join("retained.sqlite"))
            .unwrap_or_else(|_| unreachable!());
        let recovered = execute_with_effect(&mut db, &mut authority, &pause, |mode| {
            assert_eq!(mode, EffectMode::ObserveOnly);
            Ok(EffectEvidence::VmmState("Paused".into()))
        })
        .unwrap_or_else(|_| unreachable!());
        assert!(
            matches!(recovered, Outcome::Observed(session) if session.state == RetainedState::Paused)
        );
    }
    #[test]
    fn lost_retirement_ack_keeps_capacity_and_never_repeats_cleanup() {
        let (_root, mut db) = database();
        init(&db).unwrap_or_else(|_| unreachable!());
        let mut authority = FixtureReservation;
        let mut operation = operation();
        register(&mut db, &mut authority, &operation).unwrap_or_else(|_| unreachable!());
        operation.session = match execute_with_effect(&mut db, &mut authority, &operation, |_| {
            Ok(EffectEvidence::VmmState("Paused".into()))
        })
        .unwrap_or_else(|_| unreachable!())
        {
            Outcome::Observed(session) => *session,
            Outcome::ReconcileRequired => unreachable!(),
        };
        operation.operation =
            ContentId::from_blake3_hex("c".repeat(64)).unwrap_or_else(|_| unreachable!());
        operation.action = Action::Retire;
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &operation, |_| Err(
                "lost cleanup acknowledgement".into()
            ))
            .unwrap_or_else(|_| unreachable!()),
            Outcome::ReconcileRequired
        );
        let duplicate = execute_with_effect(&mut db, &mut authority, &operation, |_| {
            unreachable!("retirement is never replayed")
        })
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(duplicate, Outcome::ReconcileRequired);
        let occupied: u32 = db
            .query_row("SELECT occupied FROM hosted_retained_session", [], |row| {
                row.get(0)
            })
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(occupied, 1);
    }

    // r[verify kiln.hosted.mobility.runtime]
    #[test]
    fn concurrent_sessions_cannot_share_identity_slot_or_local_resource() {
        let (_root, mut db) = database();
        init(&db).unwrap_or_else(|_| unreachable!());
        let mut authority = FixtureReservation;
        let first = operation();
        register(&mut db, &mut authority, &first).unwrap_or_else(|_| unreachable!());
        let mut second = first.clone();
        second.session = RetainedSession::admit(
            SessionIdentity {
                session: id('2'),
                vm: id('3'),
                ..first.session.identity.clone()
            },
            first.session.placement.clone(),
            RetainedResources {
                slot: 2,
                ..first.session.resources
            },
        )
        .unwrap_or_else(|_| unreachable!());
        second.binding = Binding {
            directory: "/run/kiln/vm-b".into(),
            unit: "kiln-retained-b".into(),
            workspace: "/run/kiln/vm-b/workspace.raw".into(),
            ..first.binding.clone()
        };
        let collisions: [fn(&mut Operation); 4] = [
            |operation| operation.session.identity.session = id('b'),
            |operation| operation.session.resources.slot = 1,
            |operation| operation.binding.unit = "kiln-retained-a".into(),
            |operation| {
                operation.binding.directory = "/run/kiln/vm-a".into();
                operation.binding.workspace = "/run/kiln/vm-a/second.raw".into();
            },
        ];
        for collide in collisions {
            let mut colliding = second.clone();
            collide(&mut colliding);
            assert!(register(&mut db, &mut authority, &colliding).is_err());
        }
        let sessions: u32 = db
            .query_row("SELECT COUNT(*) FROM hosted_retained_session", [], |row| {
                row.get(0)
            })
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(sessions, 1);
        register(&mut db, &mut authority, &second).unwrap_or_else(|_| unreachable!());
        let occupied: u32 = db
            .query_row(
                "SELECT COUNT(*) FROM hosted_retained_session WHERE occupied=1",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(occupied, 2);
    }

    fn observed_session(outcome: Result<Outcome, String>) -> RetainedSession {
        match outcome {
            Ok(Outcome::Observed(session)) => *session,
            other => unreachable!("fixture effect was not observed: {other:?}"),
        }
    }

    /// Register, cold start and resume the fixture session; the returned
    /// template is bound to the observed running record.
    fn running(db: &mut Connection, authority: &mut FixtureReservation) -> Operation {
        let mut operation = operation();
        register(db, authority, &operation).unwrap_or_else(|_| unreachable!());
        operation.session =
            observed_session(execute_with_effect(db, authority, &operation, |_| {
                Ok(EffectEvidence::VmmState("Paused".into()))
            }));
        operation.operation = id('c');
        operation.action = Action::Resume;
        operation.session =
            observed_session(execute_with_effect(db, authority, &operation, |_| {
                Ok(EffectEvidence::VmmState("Running".into()))
            }));
        operation
    }

    fn stored(db: &Connection) -> (RetainedSession, bool) {
        let (record, occupied): (String, bool) = db
            .query_row(
                "SELECT record,occupied FROM hosted_retained_session",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or_else(|_| unreachable!());
        (
            serde_json::from_str(&record).unwrap_or_else(|_| unreachable!()),
            occupied,
        )
    }

    // A host that accepts the pause while its readback still reports Running
    // completed prematurely; the observer must catch it.
    // r[verify kiln.hosted.mobility.runtime.scenario.control]
    #[test]
    fn accepted_pause_without_paused_readback_stays_uncertain_and_blocks_checkpoint() {
        let (root, mut db) = database();
        init(&db).unwrap_or_else(|_| unreachable!());
        let mut authority = FixtureReservation;
        let mut pause = running(&mut db, &mut authority);
        pause.operation = id('d');
        pause.action = Action::Pause;
        let mut dispatched = 0;
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &pause, |mode| {
                assert_eq!(mode, EffectMode::Dispatch);
                dispatched += 1;
                Ok(EffectEvidence::VmmState("Running".into()))
            }),
            Ok(Outcome::ReconcileRequired)
        );
        let (uncertain, occupied) = stored(&db);
        assert_eq!(uncertain.state, RetainedState::ReconcileRequired);
        assert_eq!(
            uncertain
                .control
                .as_ref()
                .map(|control| (control.operation.clone(), control.status)),
            Some((pause.operation.clone(), ControlStatus::Uncertain))
        );
        assert!(occupied);
        let mut snapshot = pause.clone();
        snapshot.operation = id('e');
        snapshot.session = uncertain.clone();
        snapshot.action = Action::Snapshot {
            checkpoint: root.path().join("checkpoint"),
        };
        assert!(
            execute_with_effect(&mut db, &mut authority, &snapshot, |_| unreachable!(
                "an uncertain pause admits no checkpoint"
            ))
            .is_err()
        );
        let target = Placement {
            host: "host-b".into(),
            generation: 2,
        };
        assert!(
            kiln_microvm::handoff::PausedHandoff::begin(
                &uncertain,
                &owner(&uncertain),
                target,
                id('e')
            )
            .is_err()
        );
        // Reconciliation only observes, and Running still is not Paused.
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &pause, |mode| {
                assert_eq!(mode, EffectMode::ObserveOnly);
                Ok(EffectEvidence::VmmState("Running".into()))
            }),
            Ok(Outcome::ReconcileRequired)
        );
        assert_eq!(stored(&db).0.state, RetainedState::ReconcileRequired);
        // The healthy equivalent: the exact Paused readback admits a checkpoint.
        let paused = observed_session(execute_with_effect(
            &mut db,
            &mut authority,
            &pause,
            |mode| {
                assert_eq!(mode, EffectMode::ObserveOnly);
                Ok(EffectEvidence::VmmState("Paused".into()))
            },
        ));
        assert_eq!(paused.state, RetainedState::Paused);
        assert_eq!(dispatched, 1);
        snapshot.session = paused;
        let captured = execute_with_effect(&mut db, &mut authority, &snapshot, |mode| {
            assert_eq!(mode, EffectMode::Dispatch);
            Ok(EffectEvidence::VmmState("Paused".into()))
        });
        assert!(
            matches!(captured, Ok(Outcome::Observed(session)) if session.state == RetainedState::Paused)
        );
    }

    // r[verify kiln.hosted.mobility.runtime.scenario.control-observed]
    #[test]
    fn crossed_or_late_control_replies_never_adopt_state_for_another_operation_or_owner() {
        let (_root, mut db) = database();
        init(&db).unwrap_or_else(|_| unreachable!());
        let mut authority = FixtureReservation;
        let mut pause = running(&mut db, &mut authority);
        pause.operation = id('d');
        pause.action = Action::Pause;
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &pause, |_| Err(
                "lost pause response".into()
            )),
            Ok(Outcome::ReconcileRequired)
        );
        let (uncertain, _) = stored(&db);
        let mut crossed = pause.clone();
        crossed.operation = id('f');
        crossed.action = Action::Resume;
        crossed.session = uncertain.clone();
        assert!(
            execute_with_effect(&mut db, &mut authority, &crossed, |_| unreachable!(
                "a crossed control never reaches the host"
            ))
            .is_err()
        );
        assert_eq!(stored(&db).0, uncertain);
        // Only the pause's own target state is adopted from its readback.
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &pause, |_| Ok(
                EffectEvidence::VmmState("Running".into())
            )),
            Ok(Outcome::ReconcileRequired)
        );
        let paused = observed_session(execute_with_effect(&mut db, &mut authority, &pause, |_| {
            Ok(EffectEvidence::VmmState("Paused".into()))
        }));
        assert_eq!(paused.state, RetainedState::Paused);
        // A resume left unfinished, then a successor owner takes the record.
        let mut resume = pause.clone();
        resume.operation = id('7');
        resume.action = Action::Resume;
        resume.session = paused;
        assert_eq!(
            execute_with_effect(&mut db, &mut authority, &resume, |_| Err(
                "lost resume response".into()
            )),
            Ok(Outcome::ReconcileRequired)
        );
        let mut successor = stored(&db).0;
        successor.identity.fence += 1;
        successor.state = RetainedState::Paused;
        successor.control = None;
        successor.cause = None;
        db.execute(
            "UPDATE hosted_retained_session SET record=?1",
            [serde_json::to_string(&successor).unwrap_or_else(|_| unreachable!())],
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(
            execute_with_effect(&mut db, &mut authority, &resume, |_| unreachable!(
                "a late reply never reaches the host"
            ))
            .is_err()
        );
        assert_eq!(stored(&db).0, successor);
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

    #[test]
    fn accepted_pause_reply_requires_paused_readback() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = root.path().join("api.sock");
        let listener = UnixListener::bind(&path).unwrap_or_else(|_| unreachable!());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .unwrap_or_else(|_| unreachable!());
        let server = std::thread::spawn(move || {
            for index in 0..4 {
                let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!());
                let mut request = [0_u8; 1024];
                let count = stream.read(&mut request).unwrap_or_else(|_| unreachable!());
                let request = request.get(..count).unwrap_or_default();
                if index % 2 == 0 {
                    assert!(request.starts_with(b"PUT /api/v1/vm.pause "));
                } else {
                    assert!(request.starts_with(b"GET /api/v1/vm.info "));
                }
                let response = match index {
                    0 | 2 => "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_string(),
                    1 => "HTTP/1.1 200 OK\r\nContent-Length: 19\r\n\r\n{\"state\":\"Running\"}"
                        .to_string(),
                    _ => "HTTP/1.1 200 OK\r\nContent-Length: 18\r\n\r\n{\"state\":\"Paused\"}"
                        .to_string(),
                };
                stream
                    .write_all(response.as_bytes())
                    .unwrap_or_else(|_| unreachable!());
            }
        });
        let pid = std::process::id();
        assert_eq!(
            api(&path, pid, "PUT", "/api/v1/vm.pause", &Value::Null),
            Ok(Value::Null)
        );
        assert!(observe_state(&path, pid, "Paused").is_err());
        assert_eq!(
            api(&path, pid, "PUT", "/api/v1/vm.pause", &Value::Null),
            Ok(Value::Null)
        );
        assert_eq!(observe_state(&path, pid, "Paused"), Ok("Paused".into()));
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
            execute_with_effect(&mut db, &mut authority, &operation, |_| {
                Ok(EffectEvidence::VmmState("Paused".into()))
            })
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
            execute_with_effect(&mut db, &mut authority, &operation, |_| {
                super::super::paired::publish(&operation, &checkpoint)
                    .map(|()| EffectEvidence::VmmState("Paused".into()))
            })
            .unwrap_or_else(|_| unreachable!()),
        );
        operation.operation = id('3');
        operation.action = Action::Retire;
        operation.session = observed(
            execute_with_effect(&mut db, &mut authority, &operation, |_| {
                Ok(EffectEvidence::CleanupObserved)
            })
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
            execute_with_effect(&mut db, &mut authority, &operation, |_| {
                Ok(EffectEvidence::VmmState("Paused".into()))
            })
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
                execute_with_effect(&mut db, &mut authority, &operation, |_| {
                    Ok(match operation.action {
                        Action::Resume => EffectEvidence::VmmState("Running".into()),
                        Action::Pause => EffectEvidence::VmmState("Paused".into()),
                        _ => EffectEvidence::CleanupObserved,
                    })
                })
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
