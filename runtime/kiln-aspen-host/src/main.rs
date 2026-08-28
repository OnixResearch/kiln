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

use kiln_adapter_aspen::profile::{AspenRuntimeProfileDocument, admit_profile};
use kiln_adapter_lattice::{LatticeAdapter, UnixWorkflowExchange};
use kiln_adapter_radicle::{RadicleShellConfig, admit_config};
use kiln_aspen_host::cohort::{HostCohort, HostPaths};
use kiln_aspen_host::effect::{
    LatticeEffectPort, LatticeEffectPortOptions, ThreadObservationPacer,
};
use kiln_aspen_host::error::{HostBridgeError, Result};
use kiln_aspen_host::runtime::KilnAspenHostRuntime;
use kiln_aspen_host::server::{ServerConfig, serve};

const PROFILE_MAXIMUM_BYTES: u64 = 1_048_576;
const PROFILE_OVERFLOW_SENTINEL_BYTES: u64 = 1;
const INPUT_OUTPUT_EXIT_CODE: i32 = 1;
const PROFILE_ARGUMENT: &str = "--aspen-profile";
const RADICLE_PROFILE_ARGUMENT: &str = "--radicle-profile";
const SOCKET_ARGUMENT: &str = "--socket";
const EXTENSION_ARGUMENT: &str = "--extension";
const STATE_ROOT_ARGUMENT: &str = "--state-root";
const MAXIMUM_REQUESTS_ARGUMENT: &str = "--max-requests";
const TIMEOUT_MILLISECONDS_ARGUMENT: &str = "--timeout-ms";
const POLL_INTERVAL_MILLISECONDS_ARGUMENT: &str = "--poll-interval-ms";
const MAXIMUM_ARGUMENT_PAIRS: usize = 8;

#[derive(Debug, Default)]
struct ArgumentFields {
    aspen_profile: Option<std::path::PathBuf>,
    radicle_profile: Option<std::path::PathBuf>,
    socket: Option<std::path::PathBuf>,
    extension: Option<std::path::PathBuf>,
    state_root: Option<std::path::PathBuf>,
    maximum_requests: Option<usize>,
    timeout_milliseconds: Option<u64>,
    poll_interval_milliseconds: Option<u64>,
}

#[derive(Debug)]
struct Arguments {
    aspen_profile: std::path::PathBuf,
    radicle_profile: std::path::PathBuf,
    socket: std::path::PathBuf,
    extension: std::path::PathBuf,
    state_root: std::path::PathBuf,
    maximum_requests: usize,
    timeout_milliseconds: u64,
    poll_interval_milliseconds: u64,
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
    let profile_document: AspenRuntimeProfileDocument = load_json(&arguments.aspen_profile)?;
    let profile = admit_profile(&profile_document.into_runtime_profile().map_err(|error| {
        HostBridgeError::Admission(format!("{}: {}", error.code, error.message))
    })?)
    .map_err(|error| HostBridgeError::Admission(format!("{}: {}", error.code, error.message)))?;
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

fn parse_arguments(mut arguments: impl Iterator<Item = String>) -> Result<Arguments> {
    let mut fields = ArgumentFields::default();
    for _argument_index in 0..MAXIMUM_ARGUMENT_PAIRS {
        let Some(argument) = arguments.next() else {
            break;
        };
        let value = arguments
            .next()
            .ok_or_else(|| HostBridgeError::Protocol(format!("{argument} requires a value")))?;
        match argument.as_str() {
            PROFILE_ARGUMENT => set_path(&mut fields.aspen_profile, value, PROFILE_ARGUMENT)?,
            RADICLE_PROFILE_ARGUMENT => {
                set_path(&mut fields.radicle_profile, value, RADICLE_PROFILE_ARGUMENT)?;
            }
            SOCKET_ARGUMENT => set_path(&mut fields.socket, value, SOCKET_ARGUMENT)?,
            EXTENSION_ARGUMENT => {
                set_path(&mut fields.extension, value, EXTENSION_ARGUMENT)?;
            }
            STATE_ROOT_ARGUMENT => {
                set_path(&mut fields.state_root, value, STATE_ROOT_ARGUMENT)?;
            }
            MAXIMUM_REQUESTS_ARGUMENT => {
                set_usize(
                    &mut fields.maximum_requests,
                    &value,
                    MAXIMUM_REQUESTS_ARGUMENT,
                )?;
            }
            TIMEOUT_MILLISECONDS_ARGUMENT => {
                set_u64(
                    &mut fields.timeout_milliseconds,
                    &value,
                    TIMEOUT_MILLISECONDS_ARGUMENT,
                )?;
            }
            POLL_INTERVAL_MILLISECONDS_ARGUMENT => {
                set_u64(
                    &mut fields.poll_interval_milliseconds,
                    &value,
                    POLL_INTERVAL_MILLISECONDS_ARGUMENT,
                )?;
            }
            _ => {
                return Err(HostBridgeError::Protocol(format!(
                    "unknown argument {argument}",
                )));
            }
        }
    }
    if arguments.next().is_some() {
        return Err(HostBridgeError::Protocol(
            "argument count exceeds the closed host interface".to_string(),
        ));
    }
    finish_arguments(fields)
}

fn finish_arguments(fields: ArgumentFields) -> Result<Arguments> {
    Ok(Arguments {
        aspen_profile: required(fields.aspen_profile, PROFILE_ARGUMENT)?,
        radicle_profile: required(fields.radicle_profile, RADICLE_PROFILE_ARGUMENT)?,
        socket: required(fields.socket, SOCKET_ARGUMENT)?,
        extension: required(fields.extension, EXTENSION_ARGUMENT)?,
        state_root: required(fields.state_root, STATE_ROOT_ARGUMENT)?,
        maximum_requests: required(fields.maximum_requests, MAXIMUM_REQUESTS_ARGUMENT)?,
        timeout_milliseconds: required(fields.timeout_milliseconds, TIMEOUT_MILLISECONDS_ARGUMENT)?,
        poll_interval_milliseconds: required(
            fields.poll_interval_milliseconds,
            POLL_INTERVAL_MILLISECONDS_ARGUMENT,
        )?,
    })
}

fn set_path(
    field: &mut Option<std::path::PathBuf>,
    value: String,
    label: &'static str,
) -> Result<()> {
    if field.is_some() {
        return Err(HostBridgeError::Protocol(format!("{label} is duplicated",)));
    }
    *field = Some(std::path::PathBuf::from(value));
    Ok(())
}

fn set_usize(field: &mut Option<usize>, value: &str, label: &'static str) -> Result<()> {
    if field.is_some() {
        return Err(HostBridgeError::Protocol(format!("{label} is duplicated",)));
    }
    let parsed = value
        .parse::<usize>()
        .map_err(|_| HostBridgeError::Protocol(format!("{label} is not a positive integer")))?;
    if parsed == 0 {
        return Err(HostBridgeError::Protocol(format!(
            "{label} must be positive",
        )));
    }
    *field = Some(parsed);
    Ok(())
}

fn set_u64(field: &mut Option<u64>, value: &str, label: &'static str) -> Result<()> {
    if field.is_some() {
        return Err(HostBridgeError::Protocol(format!("{label} is duplicated",)));
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|_| HostBridgeError::Protocol(format!("{label} is not a positive integer")))?;
    if parsed == 0 {
        return Err(HostBridgeError::Protocol(format!(
            "{label} must be positive",
        )));
    }
    *field = Some(parsed);
    Ok(())
}

fn required<T>(value: Option<T>, label: &'static str) -> Result<T> {
    value.ok_or_else(|| HostBridgeError::Protocol(format!("{label} is required")))
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
