use super::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile;
use kiln_adapter_aspen::transport::{AspenServiceAck, AspenServiceIngress};

const CHILD_DEADLINE_SECONDS: u64 = 15;
const CHILD_POLL_MILLISECONDS: u64 = 5;
const PROVIDER_POLL_MILLISECONDS: u64 = 2;
const HOST_REJECTION_EXIT_CODE: i32 = 1;
const DURABLE_COMPONENTS: [&str; 4] = [
    "operation-ledger",
    "native-journal",
    "callback-values",
    "host-state.json",
];

pub(super) struct ProcessFixture {
    pub(super) directory: tempfile::TempDir,
    pub(super) paths: HostPaths,
    pub(super) profile: AdmittedAspenRuntimeProfile,
    pub(super) radicle_profile: PathBuf,
    pub(super) host_socket: PathBuf,
    pub(super) lattice_socket: PathBuf,
    pub(super) host_executable: PathBuf,
}

impl ProcessFixture {
    pub(super) fn new() -> Self {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary root"));
        let mut paths = host_paths(&directory);
        paths.extension_executable = qualification_executable(
            "KILN_ASPEN_HOST_QUALIFICATION_EXTENSION",
            env!("CARGO_BIN_EXE_kiln-aspen-host-extension-fixture"),
        );
        let profile = admit_profile(&profile_fixture())
            .unwrap_or_else(|_| unreachable!("admitted Aspen profile"));
        let lattice_socket = directory.path().join("lattice.sock");
        let radicle_profile = directory.path().join("radicle-profile.json");
        let config = radicle_config(
            lattice_socket.clone(),
            directory.path().join("replay.sqlite"),
            nix::unistd::getuid().as_raw(),
        );
        write_json(&radicle_profile, &config);
        let host_socket = directory.path().join("host.sock");
        let host_executable = qualification_executable(
            "KILN_ASPEN_HOST_QUALIFICATION_EXECUTABLE",
            env!("CARGO_BIN_EXE_kiln-aspen-host"),
        );
        Self {
            directory,
            paths,
            profile,
            radicle_profile,
            host_socket,
            lattice_socket,
            host_executable,
        }
    }

    pub(super) fn spawn(&self, maximum_requests: usize) -> HostChild {
        let stderr = tempfile::tempfile().unwrap_or_else(|_| unreachable!("child diagnostics"));
        self.record_launch(maximum_requests);
        let child = Command::new(&self.host_executable)
            .arg("--aspen-profile")
            .arg(&self.paths.runtime_profile)
            .arg("--radicle-profile")
            .arg(&self.radicle_profile)
            .arg("--socket")
            .arg(&self.host_socket)
            .arg("--extension")
            .arg(&self.paths.extension_executable)
            .arg("--state-root")
            .arg(&self.paths.workspace)
            .arg("--max-requests")
            .arg(maximum_requests.to_string())
            .arg("--timeout-ms")
            .arg(HOST_TIMEOUT_MILLISECONDS.to_string())
            .arg("--poll-interval-ms")
            .arg(TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(
                stderr
                    .try_clone()
                    .unwrap_or_else(|_| unreachable!("diagnostics clone")),
            )
            .spawn()
            .unwrap_or_else(|error| unreachable!("spawn native host: {error}"));
        HostChild { child, stderr }
    }

    pub(super) fn start(&self, maximum_requests: usize) -> HostChild {
        let mut child = self.spawn(maximum_requests);
        let deadline = Instant::now() + Duration::from_secs(CHILD_DEADLINE_SECONDS);
        while !self.host_socket.exists() {
            assert!(
                child
                    .child
                    .try_wait()
                    .unwrap_or_else(|_| unreachable!("host status"))
                    .is_none(),
                "host exited before ingress: {}",
                child.diagnostics()
            );
            assert!(
                Instant::now() < deadline,
                "host startup deadline: {}",
                child.diagnostics()
            );
            std::thread::park_timeout(Duration::from_millis(CHILD_POLL_MILLISECONDS));
        }
        child
    }

    pub(super) fn client(&self) -> UnixAspenServiceTransport {
        UnixAspenServiceTransport::new(
            self.host_socket.clone(),
            self.profile.profile.bounds.max_callback_input_bytes,
            Duration::from_millis(HOST_TIMEOUT_MILLISECONDS),
        )
        .unwrap_or_else(|_| unreachable!("native host client"))
    }

    pub(super) fn ingress(&self, index: usize) -> AspenServiceIngress {
        let mut request = trigger(false);
        request.repository = reference(
            "radicle",
            "repository",
            &format!("rad:qualification-{index}"),
        );
        build_ingress(&self.profile, IngressSource::Operator, request)
            .unwrap_or_else(|_| unreachable!("qualification ingress"))
    }

    pub(super) fn remove_crashed_socket(&self) {
        std::fs::remove_file(&self.host_socket)
            .unwrap_or_else(|_| unreachable!("remove owned crashed ingress socket"));
    }

    fn record_launch(&self, maximum_requests: usize) {
        println!(
            "{}",
            serde_json::json!({
                "schema": "kiln.aspen-host.qualification-child-launch.v1",
                "scope": "machine-local-native-host-with-closed-lattice-terminal-fixture",
                "host": file_identity(&self.host_executable),
                "extension": file_identity(&self.paths.extension_executable),
                "aspen_profile": file_identity(&self.paths.runtime_profile),
                "radicle_profile": file_identity(&self.radicle_profile),
                "aspen_profile_document": file_document(&self.paths.runtime_profile),
                "radicle_profile_document": file_document(&self.radicle_profile),
                "socket": self.host_socket,
                "state_root": self.paths.workspace,
                "maximum_requests": maximum_requests,
                "timeout_milliseconds": HOST_TIMEOUT_MILLISECONDS,
                "poll_interval_milliseconds": TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS,
                "child_deadline_seconds": CHILD_DEADLINE_SECONDS,
            })
        );
    }
}

pub(super) struct HostChild {
    child: Child,
    stderr: std::fs::File,
}

impl HostChild {
    fn diagnostics(&self) -> String {
        use std::os::unix::fs::FileExt;
        const MAXIMUM_DIAGNOSTIC_BYTES: usize = 16_384;
        let mut bytes = [0; MAXIMUM_DIAGNOSTIC_BYTES];
        let count = self
            .stderr
            .read_at(&mut bytes, 0)
            .unwrap_or_else(|_| unreachable!("read child diagnostics"));
        let retained = bytes
            .get(..count)
            .unwrap_or_else(|| unreachable!("child diagnostic read exceeded buffer"));
        String::from_utf8_lossy(retained).into_owned()
    }

    pub(super) fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(CHILD_DEADLINE_SECONDS);
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .unwrap_or_else(|_| unreachable!("wait child status"))
            {
                println!(
                    "{}",
                    serde_json::json!({
                        "schema": "kiln.aspen-host.qualification-child-exit.v1",
                        "pid": self.child.id(),
                        "status": status.to_string(),
                        "code": status.code(),
                        "diagnostics": self.diagnostics(),
                    })
                );
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "child exit deadline: {}",
                self.diagnostics()
            );
            std::thread::park_timeout(Duration::from_millis(CHILD_POLL_MILLISECONDS));
        }
    }

    pub(super) fn finish(mut self) {
        let status = self.wait();
        assert!(
            status.success(),
            "host exit {status}: {}",
            self.diagnostics()
        );
    }

    pub(super) fn reject(mut self, expected_diagnostic: &str) {
        let status = self.wait();
        assert_eq!(
            status.code(),
            Some(HOST_REJECTION_EXIT_CODE),
            "host rejection: {}",
            self.diagnostics()
        );
        assert!(
            self.diagnostics().contains(expected_diagnostic),
            "expected {expected_diagnostic:?}: {}",
            self.diagnostics()
        );
    }

    pub(super) fn crash(mut self) {
        use std::os::unix::process::ExitStatusExt;
        const SIGKILL: i32 = 9;
        self.child
            .kill()
            .unwrap_or_else(|_| unreachable!("kill owned host"));
        assert_eq!(self.wait().signal(), Some(SIGKILL));
    }
}

impl Drop for HostChild {
    fn drop(&mut self) {
        match self.child.kill() {
            Ok(()) | Err(_) => {}
        }
        match self.child.wait() {
            Ok(_) | Err(_) => {}
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ExchangeCounts {
    pub(super) dispatch: usize,
    pub(super) recover: usize,
    pub(super) observe: usize,
}

// This is the existing closed terminal transport fixture, not a real Lattice worker.
// Only Aspen's native service, extension execution and host persistence are real here.
struct CountedTerminalHandler {
    counts: Arc<std::sync::Mutex<ExchangeCounts>>,
    terminal: TerminalHandler,
    recovery_unavailable: Arc<AtomicBool>,
}

impl WorkflowExchangeHandler for CountedTerminalHandler {
    fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(|_| unreachable!("exchange counts"));
        match command {
            ExchangeCommand::Dispatch { .. } => counts.dispatch += 1,
            ExchangeCommand::RecoverDispatch { .. } => counts.recover += 1,
            ExchangeCommand::Observe { .. } => counts.observe += 1,
            _ => return Err(HandlerError::Rejected),
        }
        drop(counts);
        if self.recovery_unavailable.load(Ordering::SeqCst)
            && matches!(command, ExchangeCommand::RecoverDispatch { .. })
        {
            return Err(HandlerError::Unavailable);
        }
        self.terminal.handle(command)
    }
}

pub(super) struct FixtureLattice {
    stop: Arc<AtomicBool>,
    recovery_unavailable: Arc<AtomicBool>,
    counts: Arc<std::sync::Mutex<ExchangeCounts>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FixtureLattice {
    pub(super) fn start(fixture: &ProcessFixture, disconnect_first_dispatch: bool) -> Self {
        Self::with_recovery(fixture, disconnect_first_dispatch, false)
    }

    pub(super) fn unavailable_recovery(fixture: &ProcessFixture) -> Self {
        Self::with_recovery(fixture, true, true)
    }

    fn with_recovery(
        fixture: &ProcessFixture,
        disconnect_first_dispatch: bool,
        recovery_unavailable: bool,
    ) -> Self {
        let listener = UnixListener::bind(&fixture.lattice_socket)
            .unwrap_or_else(|_| unreachable!("fixture Lattice listener"));
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|_| unreachable!("nonblocking fixture"));
        let config = radicle_config(
            fixture.lattice_socket.clone(),
            fixture.directory.path().join("replay.sqlite"),
            nix::unistd::getuid().as_raw(),
        );
        let mut profile = admit_config(config)
            .unwrap_or_else(|_| unreachable!("fixture transport profile"))
            .transport;
        profile.limits.maximum_connections = 1;
        let counts = Arc::new(std::sync::Mutex::new(ExchangeCounts::default()));
        let recovery_unavailable = Arc::new(AtomicBool::new(recovery_unavailable));
        let handler = CountedTerminalHandler {
            counts: counts.clone(),
            recovery_unavailable: recovery_unavailable.clone(),
            terminal: TerminalHandler {
                pending_observations_remaining: 0,
            },
        };
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread_counts = counts.clone();
        let thread = std::thread::spawn(move || {
            let mut service = WorkflowExchangeService::new(handler);
            let mut disconnect = disconnect_first_dispatch;
            while !thread_stop.load(Ordering::SeqCst) {
                if disconnect {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            stream
                                .set_read_timeout(Some(Duration::from_millis(
                                    TEST_DEADLINE_MILLISECONDS,
                                )))
                                .unwrap_or_else(|_| unreachable!("fixture read deadline"));
                            let bytes = read_frame(&mut stream, TEST_MAXIMUM_FRAME_BYTES)
                                .unwrap_or_else(|_| unreachable!("uncertain dispatch frame"));
                            let request: ExchangeRequest = serde_json::from_slice(&bytes)
                                .unwrap_or_else(|_| unreachable!("uncertain dispatch command"));
                            assert!(matches!(request.command, ExchangeCommand::Dispatch { .. }));
                            thread_counts
                                .lock()
                                .unwrap_or_else(|_| unreachable!("dispatch count"))
                                .dispatch += 1;
                            disconnect = false;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(error) => unreachable!("accept uncertain dispatch: {error}"),
                    }
                } else {
                    match serve_listener(&listener, &profile, &mut service) {
                        Ok(report) => {
                            assert_eq!(report.completed_exchanges, 1);
                            assert_eq!(report.failed_exchanges, 0);
                            assert_eq!(report.rejected_peers, 0);
                        }
                        // The pinned transport maps a nonblocking accept's
                        // WouldBlock to AcceptFailed without consuming a frame.
                        Err(lattice_workflow_transport::ServerFailure::AcceptFailed) => {}
                        Err(error) => unreachable!("fixture Lattice exchange: {error:?}"),
                    }
                }
                std::thread::park_timeout(Duration::from_millis(PROVIDER_POLL_MILLISECONDS));
            }
        });
        Self {
            stop,
            recovery_unavailable,
            counts,
            thread: Some(thread),
        }
    }

    pub(super) fn restore_recovery(&self) {
        self.recovery_unavailable.store(false, Ordering::SeqCst);
    }

    pub(super) fn counts(&self) -> ExchangeCounts {
        self.counts
            .lock()
            .unwrap_or_else(|_| unreachable!("exchange counts"))
            .clone()
    }

    pub(super) fn finish(mut self) -> ExchangeCounts {
        self.join();
        let counts = self.counts();
        println!(
            "{}",
            serde_json::json!({
                "schema": "kiln.aspen-host.qualification-fixture-counts.v1",
                "dispatch": counts.dispatch,
                "recover_dispatch": counts.recover,
                "observe": counts.observe,
            })
        );
        counts
    }

    fn join(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .unwrap_or_else(|_| unreachable!("fixture Lattice join"));
        }
    }
}

impl Drop for FixtureLattice {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            match thread.join() {
                Ok(()) | Err(_) => {}
            }
        }
    }
}

pub(super) fn completed(outcome: ServiceTransportOutcome) -> AspenServiceAck {
    let ServiceTransportOutcome::Acknowledged(ack) = outcome else {
        unreachable!("terminal host acknowledgement: {outcome:?}");
    };
    assert_eq!(ack.decision, AspenServiceDecision::Completed);
    println!(
        "{}",
        serde_json::json!({
            "schema": "kiln.aspen-host.qualification-ack.v1", "acknowledgement": ack,
        })
    );
    ack
}

pub(super) fn unknown(outcome: ServiceTransportOutcome, operation: &ExternalRef) {
    let ServiceTransportOutcome::DisconnectedAfterAcceptance { operation_id } = outcome else {
        unreachable!("durably unknown operation: {outcome:?}");
    };
    assert_eq!(&operation_id, operation);
    println!(
        "{}",
        serde_json::json!({
            "schema": "kiln.aspen-host.qualification-unknown.v1", "operation_id": operation_id,
        })
    );
}

pub(super) fn write_json(path: &Path, value: &impl serde::Serialize) {
    std::fs::write(
        path,
        serde_json::to_vec(value).unwrap_or_else(|_| unreachable!("encode fixture JSON")),
    )
    .unwrap_or_else(|_| unreachable!("write fixture JSON"));
}

pub(super) fn state_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    for component in DURABLE_COMPONENTS {
        snapshot_entry(root, &root.join(component), &mut files);
    }
    let identities = files.iter().map(|(path, bytes)| {
        serde_json::json!({ "path": path, "blake3": blake3::hash(bytes).to_hex().to_string() })
    }).collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::json!({
            "schema": "kiln.aspen-host.qualification-quiescent-state.v1",
            "root": root,
            "marker": file_document(&root.join("host-state.json")),
            "files": identities,
        })
    );
    files
}

fn snapshot_entry(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
    let metadata = std::fs::symlink_metadata(path)
        .unwrap_or_else(|_| unreachable!("durable component metadata"));
    assert!(!metadata.file_type().is_symlink());
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path).unwrap_or_else(|_| unreachable!("durable directory")) {
            snapshot_entry(
                root,
                &entry
                    .unwrap_or_else(|_| unreachable!("durable entry"))
                    .path(),
                files,
            );
        }
    } else {
        assert!(metadata.is_file());
        files.insert(
            path.strip_prefix(root)
                .unwrap_or_else(|_| unreachable!("relative durable path"))
                .to_path_buf(),
            std::fs::read(path).unwrap_or_else(|_| unreachable!("durable bytes")),
        );
    }
}

pub(super) fn restore_state(root: &Path, snapshot: &BTreeMap<PathBuf, Vec<u8>>) {
    for component in DURABLE_COMPONENTS {
        let path = root.join(component);
        if path.is_dir() {
            std::fs::remove_dir_all(path)
                .unwrap_or_else(|_| unreachable!("replace owned durable directory"));
        } else if path.exists() {
            std::fs::remove_file(path)
                .unwrap_or_else(|_| unreachable!("replace owned durable file"));
        }
    }
    for component in &DURABLE_COMPONENTS[..3] {
        std::fs::create_dir(root.join(component))
            .unwrap_or_else(|_| unreachable!("restore durable directory"));
    }
    for (relative, bytes) in snapshot {
        let path = root.join(relative);
        let parent = path
            .parent()
            .unwrap_or_else(|| unreachable!("durable parent"));
        std::fs::create_dir_all(parent).unwrap_or_else(|_| unreachable!("restore durable parents"));
        std::fs::write(path, bytes).unwrap_or_else(|_| unreachable!("restore durable bytes"));
    }
    assert_eq!(&state_snapshot(root), snapshot);
}

#[allow(
    unknown_lints,
    tigerstyle::ambient_env,
    reason = "an explicit test-only override selects a frozen real host executable for qualification"
)]
fn qualification_executable(variable: &str, cargo_binary: &str) -> PathBuf {
    let path =
        std::env::var_os(variable).map_or_else(|| PathBuf::from(cargo_binary), PathBuf::from);
    assert!(
        path.is_absolute(),
        "qualification binary must be an exact absolute path"
    );
    std::fs::canonicalize(path).unwrap_or_else(|_| unreachable!("qualification executable path"))
}

fn file_identity(path: &Path) -> serde_json::Value {
    let bytes = std::fs::read(path).unwrap_or_else(|_| unreachable!("exact cohort input bytes"));
    serde_json::json!({ "path": path, "blake3": blake3::hash(&bytes).to_hex().to_string() })
}

fn file_document(path: &Path) -> serde_json::Value {
    let bytes = std::fs::read(path).unwrap_or_else(|_| unreachable!("cohort profile bytes"));
    serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!("cohort profile document"))
}
