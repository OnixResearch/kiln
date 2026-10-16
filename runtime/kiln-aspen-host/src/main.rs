#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the binary is the visible composition root for concrete runtime adapters"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln Aspen host names preserve their protocol ownership"
)]

use std::io::Read;

use kiln_adapter_aspen::profile::{
    AdmittedAspenRuntimeProfile, AspenRuntimeProfileDocument, admit_profile,
};
use kiln_adapter_lattice::{LatticeAdapter, UnixWorkflowExchange};
use kiln_adapter_radicle::{RadicleShellConfig, admit_config};
use kiln_aspen_host::cohort::{HostCohort, HostPaths};
use kiln_aspen_host::effect::{
    LatticeEffectPort, LatticeEffectPortOptions, ThreadObservationPacer,
};
use kiln_aspen_host::error::{HostBridgeError, Result};
use kiln_aspen_host::runtime::KilnAspenHostRuntime;
use kiln_aspen_host::server::{ServerConfig, serve};

#[path = "main/arguments.rs"]
mod arguments;

use arguments::parse_arguments;

const PROFILE_MAXIMUM_BYTES: u64 = 1_048_576;
const PROFILE_OVERFLOW_SENTINEL_BYTES: u64 = 1;
const INPUT_OUTPUT_EXIT_CODE: i32 = 1;

#[derive(Debug)]
struct RuntimeProfileBinding {
    path: std::path::PathBuf,
    profile: AdmittedAspenRuntimeProfile,
}

#[allow(
    unknown_lints,
    tigerstyle::ambient_env,
    reason = "the binary shell captures argv once and passes explicit values into orchestration"
)]
fn main() {
    let arguments = std::env::args().skip(1);
    if let Err(error) = run(arguments) {
        eprintln!("{error}");
        std::process::exit(INPUT_OUTPUT_EXIT_CODE);
    }
}

fn run(arguments: impl Iterator<Item = String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    let RuntimeProfileBinding {
        path: runtime_profile,
        profile,
    } = load_aspen_profile(&arguments.aspen_profile)?;
    if arguments.maximum_requests > profile.profile.bounds.max_records {
        return Err(HostBridgeError::Admission(
            "host request limit exceeds its uncertain-operation retention bound".to_string(),
        ));
    }
    let radicle_document: RadicleShellConfig = load_json(&arguments.radicle_profile)?;
    let radicle = admit_config(radicle_document)
        .map_err(|error| HostBridgeError::Admission(format!("Radicle profile: {error:?}")))?;
    let state_root = std::fs::canonicalize(&arguments.state_root)
        .map_err(|error| HostBridgeError::Io(format!("canonicalize state root: {error}")))?;
    if !state_root.is_dir() {
        return Err(HostBridgeError::Admission(
            "state root must be an existing directory".to_string(),
        ));
    }
    let maximum_records = radicle.shell.maximum_replay_records;
    let lattice_transport = UnixWorkflowExchange::new(
        radicle.shell.socket_path.clone(),
        radicle.transport,
        radicle.lattice.contract.clone(),
        maximum_records,
    )
    .map_err(|error| HostBridgeError::Admission(format!("Lattice transport: {error:?}")))?;
    let lattice = LatticeAdapter::new(radicle.lattice, lattice_transport)
        .map_err(|error| HostBridgeError::Admission(format!("Lattice adapter: {error:?}")))?;
    let cohort = HostCohort::build(
        profile.clone(),
        &HostPaths {
            extension_executable: arguments.extension,
            runtime_profile,
            workspace: state_root,
        },
    )?;
    let effects = LatticeEffectPort::new(LatticeEffectPortOptions {
        profile: profile.clone(),
        radicle: radicle.shell,
        lattice,
        values: cohort.values.clone(),
        ledger: cohort.ledger.clone(),
        observation_poll_interval: std::time::Duration::from_millis(
            arguments.poll_interval_milliseconds,
        ),
        observation_pacer: std::sync::Arc::new(ThreadObservationPacer),
    })?;
    let mut runtime = KilnAspenHostRuntime::start(cohort, effects)?;
    let maximum_frame_bytes = profile
        .profile
        .bounds
        .max_callback_input_bytes
        .max(profile.profile.bounds.max_callback_output_bytes);
    serve(
        &profile,
        &mut runtime,
        &ServerConfig {
            socket_path: arguments.socket,
            maximum_frame_bytes,
            maximum_requests: arguments.maximum_requests,
            timeout: std::time::Duration::from_millis(arguments.timeout_milliseconds),
        },
    )
}

fn load_aspen_profile(path: &std::path::Path) -> Result<RuntimeProfileBinding> {
    let path = std::fs::canonicalize(path)
        .map_err(|error| HostBridgeError::Io(format!("canonicalize Aspen profile: {error}")))?;
    if !path.is_file() {
        return Err(HostBridgeError::Admission(
            "Aspen profile must be a regular file".to_string(),
        ));
    }
    let document: AspenRuntimeProfileDocument = load_json(&path)?;
    let profile = document.into_runtime_profile().map_err(|error| {
        HostBridgeError::Admission(format!("{}: {}", error.code, error.message))
    })?;
    let profile = admit_profile(&profile).map_err(|error| {
        HostBridgeError::Admission(format!("{}: {}", error.code, error.message))
    })?;
    Ok(RuntimeProfileBinding { path, profile })
}

fn load_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T> {
    let file = std::fs::File::open(path)
        .map_err(|error| HostBridgeError::Io(format!("open profile: {error}")))?;
    let read_limit = PROFILE_MAXIMUM_BYTES
        .checked_add(PROFILE_OVERFLOW_SENTINEL_BYTES)
        .ok_or_else(|| HostBridgeError::Protocol("profile byte bound overflow".to_string()))?;
    let mut bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| HostBridgeError::Io(format!("read profile: {error}")))?;
    let maximum_bytes = usize::try_from(PROFILE_MAXIMUM_BYTES)
        .map_err(|_| HostBridgeError::Protocol("profile bound does not fit usize".to_string()))?;
    if bytes.len() > maximum_bytes {
        return Err(HostBridgeError::Protocol(
            "profile exceeds its named byte bound".to_string(),
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| HostBridgeError::Protocol(format!("decode profile JSON: {error}")))
}

#[cfg(test)]
#[path = "main/tests.rs"]
mod tests;
