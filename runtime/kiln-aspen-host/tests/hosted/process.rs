#![cfg(test)]

use super::*;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile;
use kiln_adapter_aspen::transport::{AspenServiceAck, AspenServiceIngress};

#[path = "process/child.rs"]
mod child;
#[path = "process/lattice.rs"]
mod lattice;
#[path = "process/state.rs"]
mod state;

pub(super) use child::HostChild;
pub(super) use lattice::{ExchangeCounts, FixtureLattice};
pub(super) use state::{restore_state, state_snapshot};

const CHILD_DEADLINE_SECONDS: u64 = 15;
const CHILD_POLL_MILLISECONDS: u64 = 5;
const HOST_REJECTION_EXIT_CODE: i32 = 1;

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
