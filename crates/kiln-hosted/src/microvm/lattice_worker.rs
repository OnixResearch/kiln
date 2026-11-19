//! Hosted shell for the pinned Lattice worker guest (Kiln #51/#52, M1).
//!
//! It verifies the Nix-built guest manifest, derives the existing Cloud
//! Hypervisor backend from it, creates a never-used per-attempt workspace image
//! and dispatches one admitted attempt to a retained session that the existing
//! `retained` shell has observed `Running`. Dispatch intent is journalled before
//! the frame is sent and is never resent. This module is not a VMM supervisor,
//! placement authority or Lattice workflow-exchange gateway.
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kiln_core::types::ContentId;
use kiln_microvm::lattice_worker::{
    self, AttemptRequest, GuestManifest, GuestMessage, HostMessage, LATTICE_WORKER_PORT, Report,
};
use kiln_microvm::retained::{RetainedSession, RetainedState};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use super::retained::Operation;
use super::{POLL, Runner, observe_confinement};
use crate::execution::{Backend, Config, Image};
use crate::fleet::Executable;

/// The manifest is a few KiB; refuse anything far larger before parsing.
const MAX_MANIFEST_BYTES: u64 = 1_048_576;
/// Confinement must already hold for a running retained VMM.
const CONFINEMENT_WAIT: Duration = Duration::from_secs(5);
/// Per-job backend fields the retained CH route requires but does not use.
const GUEST_SHELL: &str = "/bin/sh";

/// Read, validate and re-hash every immutable object of a guest manifest.
/// Returns the manifest and its image identity.
// r[impl kiln.lattice.worker.guest.scenario.identity]
pub fn load_manifest(path: &Path) -> Result<(GuestManifest, ContentId), String> {
    let file = fs::File::open(path).map_err(|e| format!("guest manifest unavailable: {e}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("guest manifest exceeds bound".into());
    }
    let manifest: GuestManifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("guest manifest invalid: {e}"))?;
    let identity = manifest
        .identity()
        .map_err(|e| format!("guest manifest refused: {}", e.code))?;
    for object in [&manifest.kernel, &manifest.initrd, &manifest.vmm.binary] {
        Executable {
            path: PathBuf::from(&object.path),
            blake3: object.blake3.as_str().to_string(),
        }
        .verify()
        .map_err(|e| format!("guest object {}: {e}", object.path))?;
    }
    Ok((manifest, identity))
}

/// The existing CH backend for a verified manifest; no new runner kind.
#[must_use]
pub fn backend(manifest: &GuestManifest, boot_timeout_ms: u64) -> Backend {
    Backend::CloudHypervisor {
        cloud_hypervisor: Executable {
            path: PathBuf::from(&manifest.vmm.binary.path),
            blake3: manifest.vmm.binary.blake3.as_str().to_string(),
        },
        kernel: Image {
            path: PathBuf::from(&manifest.kernel.path),
            blake3: manifest.kernel.blake3.as_str().to_string(),
        },
        initrd: Image {
            path: PathBuf::from(&manifest.initrd.path),
            blake3: manifest.initrd.blake3.as_str().to_string(),
        },
        guest_shell: GUEST_SHELL.into(),
        vcpus: manifest.resources.vcpus,
        memory_mib: manifest.resources.memory_mib,
        boot_timeout_ms,
        source_image_mib: manifest.resources.workspace_mib,
        qualification_identity: None,
        warm_pool: None,
        network: None,
        version: Some(manifest.vmm.version.clone()),
        cpu_affinity: None,
    }
}

/// Create the attempt's blank sparse raw workspace image exactly once.
pub fn create_workspace(workspace: &Path, manifest: &GuestManifest) -> Result<(), String> {
    let parent = workspace
        .parent()
        .ok_or("workspace image needs a private parent directory")?;
    super::verify_pool_directory(parent)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(workspace)
        .map_err(|e| format!("workspace image not created: {e}"))?;
    file.set_len(u64::from(manifest.resources.workspace_mib) * kiln_microvm::MIB)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|e| e.to_string())
}

pub fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS hosted_lattice_worker_dispatch(
        attempt TEXT PRIMARY KEY, session TEXT NOT NULL, request TEXT NOT NULL, outcome TEXT,
        FOREIGN KEY(session) REFERENCES hosted_retained_session(session));",
    )
    .map_err(|e| e.to_string())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchOutcome {
    /// The guest's own report for this exact request.
    Reported(Box<Report>),
    /// Intent was journalled but no report was recorded; never resent.
    ReconcileRequired,
}

/// Exact guest announcement plus its reply to a ping, observed over a fresh
/// connection (for example after snapshot restore).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestObservation {
    pub environment: ContentId,
    pub attempt: Option<ContentId>,
}

fn current_session(db: &Connection, operation: &Operation) -> Result<RetainedSession, String> {
    let (record, binding): (String, String) = db
        .query_row(
            "SELECT record,binding FROM hosted_retained_session WHERE session=?1",
            [operation.session.identity.session.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("retained session unavailable: {e}"))?;
    let session: RetainedSession = serde_json::from_str(&record).map_err(|e| e.to_string())?;
    if session != operation.session
        || binding != serde_json::to_string(&operation.binding).map_err(|e| e.to_string())?
    {
        return Err("dispatch names a stale retained owner, placement or binding".into());
    }
    Ok(session)
}

/// One accepted connection from the exact confined VMM on the worker port.
struct Channel {
    stream: UnixStream,
    path: PathBuf,
}

impl Drop for Channel {
    fn drop(&mut self) {
        // The listener is closed; its inode is this controller's own resource.
        let _ = fs::remove_file(&self.path);
    }
}

fn accept(
    config: &Config,
    backend: &Backend,
    operation: &Operation,
    deadline: Instant,
) -> Result<Channel, String> {
    let runner = Runner::from_cloud_hypervisor(backend)
        .ok_or("Lattice worker requires the existing CH backend")?;
    let binary = runner.vmm.verify()?;
    let pid = observe_confinement(
        config,
        &operation.binding.unit,
        "cloud-hypervisor",
        &binary,
        &runner.vmm.blake3,
        true,
        CONFINEMENT_WAIT,
    )?
    .pid;
    super::verify_pool_directory(&operation.binding.directory)?;
    let path = operation
        .binding
        .directory
        .join(format!("vsock.sock_{LATTICE_WORKER_PORT}"));
    let owner = fs::metadata("/proc/self").map_err(|e| e.to_string())?.uid();
    match fs::symlink_metadata(&path) {
        // Only a previous listener inode of this service in the private VM
        // directory is replaced; anything else needs reconciliation.
        Ok(metadata) if metadata.file_type().is_socket() && metadata.uid() == owner => {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(_) => return Err("worker listener path is not this service's socket".into()),
        Err(error) if error.kind() == ErrorKind::NotFound => (),
        Err(error) => return Err(error.to_string()),
    }
    let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
    let channel_path = path.clone();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(POLL)
            }
            Err(error) => {
                let _ = fs::remove_file(&path);
                return Err(format!("Lattice worker guest did not connect: {error}"));
            }
        }
    };
    let channel = Channel {
        stream,
        path: channel_path,
    };
    let peer = rustix::net::sockopt::socket_peercred(&channel.stream).map_err(|e| e.to_string())?;
    if peer.uid.as_raw() != owner || u32::try_from(peer.pid.as_raw_pid()).ok() != Some(pid) {
        return Err("worker connection is not from the exact confined VMM".into());
    }
    channel
        .stream
        .set_nonblocking(false)
        .map_err(|e| e.to_string())?;
    Ok(channel)
}

fn send(channel: &mut Channel, message: HostMessage) -> Result<(), String> {
    channel
        .stream
        .write_all(&lattice_worker::encode(message)?)
        .map_err(|e| e.to_string())
}

fn receive(channel: &mut Channel, deadline: Instant) -> Result<GuestMessage, String> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or("Lattice worker deadline")?;
    channel
        .stream
        .set_read_timeout(Some(remaining))
        .map_err(|e| e.to_string())?;
    let mut prefix = [0_u8; 4];
    channel
        .stream
        .read_exact(&mut prefix)
        .map_err(|e| e.to_string())?;
    let mut body = vec![0_u8; lattice_worker::frame_length(prefix)?];
    channel
        .stream
        .read_exact(&mut body)
        .map_err(|e| e.to_string())?;
    lattice_worker::decode(&body)
}

fn hello(
    channel: &mut Channel,
    manifest: &GuestManifest,
    deadline: Instant,
) -> Result<Option<ContentId>, String> {
    match receive(channel, deadline)? {
        GuestMessage::Hello {
            environment,
            attempt,
        } if environment == manifest.environment_identity => Ok(attempt),
        GuestMessage::Hello { .. } => Err("guest announced a different environment".into()),
        _ => Err("guest did not announce itself".into()),
    }
}

/// Observe the guest over a fresh connection without dispatching anything.
pub fn observe(
    db: &Connection,
    config: &Config,
    backend: &Backend,
    operation: &Operation,
    manifest: &GuestManifest,
    wait: Duration,
) -> Result<GuestObservation, String> {
    if current_session(db, operation)?.state != RetainedState::Running {
        return Err("guest observation requires an observed running session".into());
    }
    let deadline = Instant::now() + wait;
    let mut channel = accept(config, backend, operation, deadline)?;
    let announced = hello(&mut channel, manifest, deadline)?;
    let nonce = crate::now_ms()?.to_string();
    send(
        &mut channel,
        HostMessage::Ping {
            nonce: nonce.clone(),
        },
    )?;
    match receive(&mut channel, deadline)? {
        GuestMessage::Pong {
            nonce: echoed,
            attempt,
        } if echoed == nonce && attempt == announced => Ok(GuestObservation {
            environment: manifest.environment_identity.clone(),
            attempt,
        }),
        _ => Err("guest ping reply was crossed or inconsistent".into()),
    }
}

/// Dispatch one admitted attempt at most once. Admission and session checks
/// precede the journal; the journal precedes the frame; any failure after the
/// journal is recorded as reconciliation and never resent.
// r[impl kiln.lattice.worker.session] r[impl kiln.lattice.worker.session.scenario.dispatch-once]
pub fn dispatch(
    db: &mut Connection,
    config: &Config,
    backend: &Backend,
    operation: &Operation,
    manifest: &GuestManifest,
    request: &AttemptRequest,
) -> Result<DispatchOutcome, String> {
    dispatch_with(db, operation, manifest, request, |request| {
        let deadline = Instant::now()
            + Duration::from_millis(
                request
                    .deadline_unix_ms
                    .saturating_sub(crate::now_ms()?)
                    .min(config.maximum_timeout_ms),
            );
        let mut channel = accept(config, backend, operation, deadline)?;
        if hello(&mut channel, manifest, deadline)?.is_some() {
            return Err("guest incarnation already runs an attempt".into());
        }
        let mut entropy = [0_u8; lattice_worker::ENTROPY_BYTES];
        fs::File::open("/dev/urandom")
            .and_then(|mut source| source.read_exact(&mut entropy))
            .map_err(|e| e.to_string())?;
        send(
            &mut channel,
            HostMessage::Dispatch {
                request: Box::new(request.clone()),
                clock_unix_ms: crate::now_ms()?,
                entropy_hex: kiln_microvm::protocol::hex(&entropy),
            },
        )?;
        match receive(&mut channel, deadline)? {
            GuestMessage::Report(report) => Ok(report),
            _ => Err("guest answered dispatch with another message".into()),
        }
    })
}

fn dispatch_with(
    db: &mut Connection,
    operation: &Operation,
    manifest: &GuestManifest,
    request: &AttemptRequest,
    effect: impl FnOnce(&AttemptRequest) -> Result<Report, String>,
) -> Result<DispatchOutcome, String> {
    lattice_worker::admit_attempt(manifest, request, crate::now_ms()?)
        .map_err(|e| format!("attempt refused before dispatch: {}", e.code))?;
    let identity = &operation.session.identity;
    if request.attempt != identity.attempt || request.execution != identity.execution {
        return Err("attempt request is not bound to this retained session".into());
    }
    let encoded = serde_json::to_string(request).map_err(|e| e.to_string())?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let previous: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT request,outcome FROM hosted_lattice_worker_dispatch WHERE attempt=?1",
            [request.attempt.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some((original, outcome)) = previous {
        if original != encoded {
            return Err("attempt dispatch reused with a changed request".into());
        }
        return match outcome {
            Some(outcome) => serde_json::from_str(&outcome).map_err(|e| e.to_string()),
            None => Ok(DispatchOutcome::ReconcileRequired),
        };
    }
    if current_session(&tx, operation)?.state != RetainedState::Running {
        return Err("dispatch requires an observed running retained session".into());
    }
    tx.execute(
        "INSERT INTO hosted_lattice_worker_dispatch(attempt,session,request,outcome) VALUES(?1,?2,?3,NULL)",
        params![request.attempt.as_str(), identity.session.as_str(), encoded],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    let outcome = match effect(request) {
        Ok(report)
            if report.attempt == request.attempt
                && report.guest_identity == request.guest_identity =>
        {
            DispatchOutcome::Reported(Box::new(report))
        }
        _ => DispatchOutcome::ReconcileRequired,
    };
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE hosted_lattice_worker_dispatch SET outcome=?2 WHERE attempt=?1 AND outcome IS NULL",
        params![
            request.attempt.as_str(),
            serde_json::to_string(&outcome).map_err(|e| e.to_string())?
        ],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_microvm::lattice_worker::{
        AUTHORITY_SCHEMA, AdmittedWorkflow, DeviceProfile, ENVIRONMENT_SCHEMA, EffectRequirements,
        GuestEnvironment, GuestResources, LatticeBuild, MANIFEST_SCHEMA, NETWORK_NONE,
        ReportOutcome, StoreObject, Vmm, WORKSPACE_DISK,
    };
    use kiln_microvm::retained::{Placement, RetainedResources, SessionIdentity};
    use std::cell::Cell;
    use std::collections::BTreeMap;

    use super::super::retained::{Action, Binding};

    fn id(c: char) -> ContentId {
        ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }

    fn object(c: char) -> StoreObject {
        StoreObject {
            path: format!("/nix/store/{}-object", c.to_string().repeat(32)),
            blake3: id(c),
        }
    }

    fn manifest() -> GuestManifest {
        let mut workflows = BTreeMap::new();
        workflows.insert(
            "log-triage".to_string(),
            AdmittedWorkflow {
                definition: id('d'),
                requirements: EffectRequirements {
                    schema: AUTHORITY_SCHEMA.into(),
                    workflow_revision: "blake3:7".into(),
                    machine_identity: "blake3:9".into(),
                    policy_ref: "blake3:8".into(),
                    grants: vec![lattice_worker::Grant {
                        node_id: 0,
                        operation: "lattice/shell".into(),
                        resource: "lattice://process/0".into(),
                        ability: "process/execute".into(),
                    }],
                },
            },
        );
        let environment = GuestEnvironment {
            schema: ENVIRONMENT_SCHEMA.into(),
            architecture: "x86_64".into(),
            lattice: LatticeBuild {
                revision: "661499d282c2e79ebefbd0f9b35beb5f8d7795cd".into(),
                binary: id('a'),
            },
            guest_shell: id('b'),
            busybox: id('c'),
            workflows,
        };
        GuestManifest {
            schema: MANIFEST_SCHEMA.into(),
            architecture: "x86_64".into(),
            kernel: object('1'),
            initrd: object('2'),
            vmm: Vmm {
                kind: "cloud-hypervisor".into(),
                version: "cloud-hypervisor v54.0.0".into(),
                binary: object('3'),
            },
            environment_identity: environment.identity().unwrap_or_else(|_| unreachable!()),
            environment,
            devices: DeviceProfile {
                workspace_disk: WORKSPACE_DISK.into(),
                vsock_port: LATTICE_WORKER_PORT,
                network: NETWORK_NONE.into(),
            },
            resources: GuestResources {
                vcpus: 1,
                memory_mib: 768,
                workspace_mib: 64,
            },
        }
    }

    fn database(state: RetainedState) -> (tempfile::TempDir, Connection, Operation) {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let mut db =
            Connection::open(directory.path().join("db.sqlite")).unwrap_or_else(|_| unreachable!());
        db.pragma_update(None, "synchronous", "FULL")
            .unwrap_or_else(|_| unreachable!());
        super::super::retained::init(&db).unwrap_or_else(|_| unreachable!());
        init(&db).unwrap_or_else(|_| unreachable!());
        let mut session = RetainedSession::admit(
            SessionIdentity {
                session: id('6'),
                execution: id('5'),
                vm: id('7'),
                work: id('8'),
                work_revision: 1,
                attempt: id('4'),
                fence: 1,
            },
            Placement {
                host: "host-a".into(),
                generation: 1,
            },
            RetainedResources {
                vcpus: 1,
                memory_mib: 768,
                slot: 1,
            },
        )
        .unwrap_or_else(|_| unreachable!());
        session.state = state;
        let operation = Operation {
            operation: id('9'),
            session,
            binding: Binding {
                directory: PathBuf::from("/tmp/klw-unit"),
                unit: "kiln-retained-unit".into(),
                workspace: PathBuf::from("/tmp/klw-unit/workspace.img"),
                disk_generation: 1,
                deadline_unix_ms: u64::MAX,
                profile: id('e'),
            },
            action: Action::Resume,
        };
        let tx = db.transaction().unwrap_or_else(|_| unreachable!());
        tx.execute(
            "INSERT INTO hosted_retained_session(session,host,slot,occupied,revision,record,binding) VALUES(?1,'host-a',0,1,0,?2,?3)",
            params![
                operation.session.identity.session.as_str(),
                serde_json::to_string(&operation.session).unwrap_or_default(),
                serde_json::to_string(&operation.binding).unwrap_or_default()
            ],
        )
        .unwrap_or_else(|_| unreachable!());
        tx.commit().unwrap_or_else(|_| unreachable!());
        (directory, db, operation)
    }

    fn attempt(manifest: &GuestManifest) -> AttemptRequest {
        lattice_worker::request(manifest, id('4'), id('5'), "log-triage", 1, u64::MAX)
            .unwrap_or_else(|_| unreachable!())
    }

    fn report(request: &AttemptRequest) -> Report {
        Report {
            attempt: request.attempt.clone(),
            guest_identity: request.guest_identity.clone(),
            outcome: ReportOutcome::Refused {
                code: "fixture".into(),
                field: "fixture".into(),
            },
        }
    }

    // r[verify kiln.lattice.worker.session.scenario.dispatch-once]
    #[test]
    fn dispatch_is_journalled_once_and_never_resent() {
        let manifest = manifest();
        let request = attempt(&manifest);
        let (_directory, mut db, operation) = database(RetainedState::Running);
        let sent = Cell::new(0);
        let first = dispatch_with(&mut db, &operation, &manifest, &request, |request| {
            sent.set(sent.get() + 1);
            Ok(report(request))
        });
        assert!(matches!(first, Ok(DispatchOutcome::Reported(_))));
        let again = dispatch_with(&mut db, &operation, &manifest, &request, |request| {
            sent.set(sent.get() + 1);
            Ok(report(request))
        });
        assert_eq!(first, again);
        assert_eq!(sent.get(), 1);
        let mut changed = request.clone();
        changed.deadline_unix_ms -= 1;
        assert!(
            dispatch_with(&mut db, &operation, &manifest, &changed, |_| unreachable!()).is_err()
        );
    }

    #[test]
    fn lost_report_and_crash_after_intent_require_reconciliation() {
        let manifest = manifest();
        let request = attempt(&manifest);
        let (_directory, mut db, operation) = database(RetainedState::Running);
        let lost = dispatch_with(&mut db, &operation, &manifest, &request, |_| {
            Err("connection reset after write".into())
        });
        assert_eq!(lost, Ok(DispatchOutcome::ReconcileRequired));
        let repeated = dispatch_with(&mut db, &operation, &manifest, &request, |_| unreachable!());
        assert_eq!(repeated, Ok(DispatchOutcome::ReconcileRequired));
        // A crash after the intent commit leaves no outcome: still no resend.
        db.execute("UPDATE hosted_lattice_worker_dispatch SET outcome=NULL", [])
            .unwrap_or_else(|_| unreachable!());
        let crashed = dispatch_with(&mut db, &operation, &manifest, &request, |_| unreachable!());
        assert_eq!(crashed, Ok(DispatchOutcome::ReconcileRequired));
    }

    #[test]
    fn crossed_report_is_not_adopted() {
        let manifest = manifest();
        let request = attempt(&manifest);
        let (_directory, mut db, operation) = database(RetainedState::Running);
        let crossed = dispatch_with(&mut db, &operation, &manifest, &request, |request| {
            let mut other = report(request);
            other.guest_identity = "0".repeat(32);
            Ok(other)
        });
        assert_eq!(crossed, Ok(DispatchOutcome::ReconcileRequired));
    }

    #[test]
    fn dispatch_refuses_before_journal_when_not_running_stale_or_crossed() {
        let manifest = manifest();
        let request = attempt(&manifest);
        let (_directory, mut paused_db, paused) = database(RetainedState::Paused);
        assert!(
            dispatch_with(
                &mut paused_db,
                &paused,
                &manifest,
                &request,
                |_| unreachable!()
            )
            .is_err()
        );
        let (_directory, mut db, operation) = database(RetainedState::Running);
        let mut stale = operation.clone();
        stale.session.placement.generation = 2;
        assert!(dispatch_with(&mut db, &stale, &manifest, &request, |_| unreachable!()).is_err());
        let mut other = operation.clone();
        other.session.identity.attempt = id('f');
        assert!(dispatch_with(&mut db, &other, &manifest, &request, |_| unreachable!()).is_err());
        let mut wrong_image = request.clone();
        wrong_image.image = id('f');
        assert!(
            dispatch_with(
                &mut db,
                &operation,
                &manifest,
                &wrong_image,
                |_| unreachable!()
            )
            .is_err()
        );
        let rows: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM hosted_lattice_worker_dispatch",
                [],
                |row| row.get(0),
            )
            .unwrap_or(-1);
        assert_eq!(rows, 0);
    }

    #[test]
    fn workspace_image_is_created_once_in_a_private_directory() {
        let manifest = manifest();
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|_| unreachable!());
        let image = directory.path().join("workspace.img");
        assert!(create_workspace(&image, &manifest).is_ok());
        let metadata = fs::metadata(&image).unwrap_or_else(|_| unreachable!());
        assert_eq!(metadata.mode() & 0o777, 0o600);
        assert_eq!(metadata.len(), 64 * kiln_microvm::MIB);
        assert!(create_workspace(&image, &manifest).is_err());
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|_| unreachable!());
        assert!(create_workspace(&directory.path().join("other.img"), &manifest).is_err());
    }

    #[test]
    fn manifest_with_changed_object_bytes_is_refused() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = directory.path().join("manifest.json");
        fs::write(&path, serde_json::to_vec(&manifest()).unwrap_or_default())
            .unwrap_or_else(|_| unreachable!());
        // Fixture objects are not in the store: verification must refuse.
        assert!(load_manifest(&path).is_err());
        fs::write(&path, b"{\"schema\":\"kiln.lattice-worker-guest.v0\"}")
            .unwrap_or_else(|_| unreachable!());
        assert!(load_manifest(&path).is_err());
    }
}
