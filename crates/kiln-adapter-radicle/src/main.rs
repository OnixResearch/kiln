use std::io::Read;
use std::io::Write;
use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;
use std::time::Duration;

use kiln_adapter_aspen::composition::{
    RuntimeComposition, RuntimeSelection, compose_application, exact_availability,
};
use kiln_adapter_aspen::profile::AspenRuntimeProfileDocument;
use kiln_adapter_aspen::transport::{AspenTriggerClient, IngressSource, UnixAspenServiceTransport};
use kiln_adapter_lattice::{LatticeAdapter, UnixWorkflowExchange};
use kiln_adapter_radicle::defelo::{MAX_DEFELO_REQUEST_BYTES, run_defelo_protocol_with_policy};
use kiln_adapter_radicle::{
    MAX_TRIGGER_LINE_BYTES, RadicleShellConfig, RadicleWorkflowService, SqliteReplayStore,
    admit_config, run_protocol_with_policy,
};
use kiln_provider_api::{PortFailure, TriggerLifecyclePolicy, TriggerWaitMode};
use serde::de::DeserializeOwned;

const INPUT_OUTPUT_EXIT_CODE: i32 = 1;
const PROTOCOL_EXIT_CODE: i32 = 2;
const PROFILE_MAXIMUM_BYTES: u64 = 1_048_576;
const PROFILE_ARGUMENT: &str = "--profile";
const PROTOCOL_ARGUMENT: &str = "--protocol";
const RUNTIME_ARGUMENT: &str = "--runtime";
const ASPEN_PROFILE_ARGUMENT: &str = "--aspen-profile";
const ASPEN_SOCKET_ARGUMENT: &str = "--aspen-socket";
const DEFELO_PROTOCOL: &str = "defelo";
const NATIVE_PROTOCOL: &str = "native";
const LATTICE_RUNTIME: &str = "lattice";
const ASPEN_RUNTIME: &str = "aspen";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Protocol {
    Defelo,
    Native,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Runtime {
    Lattice,
    Aspen,
}

#[derive(Debug)]
struct Arguments {
    profile: PathBuf,
    protocol: Protocol,
    runtime: Runtime,
    aspen_profile: Option<PathBuf>,
    aspen_socket: Option<PathBuf>,
}

fn parse_arguments() -> Result<Arguments, &'static str> {
    parse_argument_values(std::env::args().skip(1))
}

fn parse_argument_values(
    mut arguments: impl Iterator<Item = String>,
) -> Result<Arguments, &'static str> {
    let mut profile = None;
    let mut protocol = Protocol::Defelo;
    let mut runtime = None;
    let mut aspen_profile = None;
    let mut aspen_socket = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            PROFILE_ARGUMENT => set_path(&mut profile, arguments.next(), "profile")?,
            PROTOCOL_ARGUMENT => {
                protocol = match arguments.next().as_deref() {
                    Some(DEFELO_PROTOCOL) => Protocol::Defelo,
                    Some(NATIVE_PROTOCOL) => Protocol::Native,
                    _ => return Err("protocol must be defelo or native"),
                };
            }
            RUNTIME_ARGUMENT => {
                if runtime.is_some() {
                    return Err("runtime argument is duplicated");
                }
                runtime = Some(match arguments.next().as_deref() {
                    Some(LATTICE_RUNTIME) => Runtime::Lattice,
                    Some(ASPEN_RUNTIME) => Runtime::Aspen,
                    _ => return Err("runtime must be lattice or aspen"),
                });
            }
            ASPEN_PROFILE_ARGUMENT => {
                set_path(&mut aspen_profile, arguments.next(), "Aspen profile")?;
            }
            ASPEN_SOCKET_ARGUMENT => {
                set_path(&mut aspen_socket, arguments.next(), "Aspen socket")?;
            }
            _ => return Err("unknown argument"),
        }
    }
    let profile = profile.ok_or("--profile is required")?;
    let runtime = runtime.ok_or("--runtime is required")?;
    match runtime {
        Runtime::Lattice if aspen_profile.is_some() || aspen_socket.is_some() => {
            return Err("Aspen arguments are not allowed for the lattice runtime");
        }
        Runtime::Aspen if aspen_profile.is_none() || aspen_socket.is_none() => {
            return Err("Aspen runtime requires --aspen-profile and --aspen-socket");
        }
        Runtime::Lattice | Runtime::Aspen => {}
    }
    Ok(Arguments {
        profile,
        protocol,
        runtime,
        aspen_profile,
        aspen_socket,
    })
}

fn set_path(
    field: &mut Option<PathBuf>,
    value: Option<String>,
    label: &'static str,
) -> Result<(), &'static str> {
    if field.is_some() {
        return Err(match label {
            "profile" => "profile argument is duplicated",
            "Aspen profile" => "Aspen profile argument is duplicated",
            "Aspen socket" => "Aspen socket argument is duplicated",
            _ => "path argument is duplicated",
        });
    }
    *field = Some(PathBuf::from(
        value.ok_or("path argument value is missing")?,
    ));
    Ok(())
}

fn load_profile<T: DeserializeOwned>(path: &std::path::Path) -> Result<T, &'static str> {
    let file = std::fs::File::open(path).map_err(|_| "profile file could not be opened")?;
    let mut bytes = Vec::new();
    file.take(PROFILE_MAXIMUM_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| "profile file could not be read")?;
    let maximum = usize::try_from(PROFILE_MAXIMUM_BYTES)
        .map_err(|_| "profile bound does not fit this host")?;
    if bytes.len() > maximum {
        return Err("profile file exceeds its named byte bound");
    }
    serde_json::from_slice(&bytes).map_err(|_| "profile JSON is invalid")
}

fn read_stdin_bound(maximum_bytes: usize) -> Result<String, &'static str> {
    let limit = u64::try_from(maximum_bytes)
        .map_err(|_| "broker input bound does not fit the reader")?
        .saturating_add(1);
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_| "broker input read failed")?;
    if bytes.len() > maximum_bytes {
        return Err("broker input exceeds its named byte bound");
    }
    String::from_utf8(bytes).map_err(|_| "broker input is not UTF-8")
}

fn report_error(error: &PortFailure) {
    match error {
        PortFailure::Domain(diagnostic) => eprintln!("{}: {}", diagnostic.code, diagnostic.message),
        PortFailure::Infrastructure(failure)
        | PortFailure::Retryable(failure)
        | PortFailure::TimedOut(failure)
        | PortFailure::Cancelled(failure)
        | PortFailure::Unknown(failure) => eprintln!("{}: {}", failure.code, failure.message),
    }
}

fn run() -> Result<Vec<String>, PortFailure> {
    let arguments = parse_arguments().map_err(argument_error)?;
    let config: RadicleShellConfig = load_profile(&arguments.profile).map_err(argument_error)?;
    let admitted = admit_config(config)?;
    let policy = TriggerLifecyclePolicy {
        mode: TriggerWaitMode::Wait,
        max_polls: admitted.shell.maximum_polls,
    };
    let maximum_input = match arguments.protocol {
        Protocol::Defelo => MAX_DEFELO_REQUEST_BYTES,
        Protocol::Native => MAX_TRIGGER_LINE_BYTES,
    };
    let input = read_stdin_bound(maximum_input).map_err(argument_error)?;
    match arguments.runtime {
        Runtime::Lattice => run_lattice(arguments.protocol, &input, admitted, policy),
        Runtime::Aspen => run_aspen(arguments, &input, admitted, policy),
    }
}

fn run_lattice(
    protocol: Protocol,
    input: &str,
    admitted: kiln_adapter_radicle::AdmittedRadicleConfig,
    policy: TriggerLifecyclePolicy,
) -> Result<Vec<String>, PortFailure> {
    let maximum_records = admitted.shell.maximum_replay_records;
    let transport = UnixWorkflowExchange::new(
        admitted.shell.socket_path.clone(),
        admitted.transport,
        admitted.lattice.contract.clone(),
        maximum_records,
    )?;
    let lattice = LatticeAdapter::new(admitted.lattice, transport)?;
    let replay = SqliteReplayStore::open(
        &admitted.shell.replay_database_path,
        admitted.shell.maximum_replay_records,
    )?;
    let mut service = RadicleWorkflowService::new(lattice, replay, admitted.shell);
    run_protocol(protocol, input, &mut service, policy)
}

// r[impl kiln.aspen_runtime.boundary] r[impl kiln.aspen_runtime.composition]
fn run_aspen(
    arguments: Arguments,
    input: &str,
    _radicle: kiln_adapter_radicle::AdmittedRadicleConfig,
    policy: TriggerLifecyclePolicy,
) -> Result<Vec<String>, PortFailure> {
    let profile_path = arguments
        .aspen_profile
        .as_deref()
        .ok_or_else(|| argument_error("Aspen profile path is missing"))?;
    let socket_path = arguments
        .aspen_socket
        .ok_or_else(|| argument_error("Aspen socket path is missing"))?;
    let document: AspenRuntimeProfileDocument =
        load_profile(profile_path).map_err(argument_error)?;
    let profile = document
        .into_runtime_profile()
        .map_err(PortFailure::Domain)?;
    let mut availability = exact_availability();
    availability.endpoint_available = match std::fs::symlink_metadata(&socket_path) {
        Ok(metadata) => metadata.file_type().is_socket(),
        Err(_) => false,
    };
    let composition =
        compose_application(RuntimeSelection::AspenV2, Some(profile), Some(availability))
            .map_err(PortFailure::Domain)?;
    let RuntimeComposition::Aspen { profile } = composition.runtime else {
        return Err(argument_error("Aspen composition selected another runtime"));
    };
    let maximum_frame_bytes = profile
        .profile
        .bounds
        .max_callback_input_bytes
        .max(profile.profile.bounds.max_callback_output_bytes);
    let timeout = Duration::from_millis(profile.profile.bounds.callback_timeout_millis);
    let transport = UnixAspenServiceTransport::new(socket_path, maximum_frame_bytes, timeout)
        .map_err(argument_error)?;
    let mut service = AspenTriggerClient::new(*profile, IngressSource::Radicle, transport);
    run_protocol(arguments.protocol, input, &mut service, policy)
}

fn run_protocol(
    protocol: Protocol,
    input: &str,
    service: &mut impl kiln_provider_api::TriggerIntakePort,
    policy: TriggerLifecyclePolicy,
) -> Result<Vec<String>, PortFailure> {
    match protocol {
        Protocol::Defelo => run_defelo_protocol_with_policy(input, service, policy),
        Protocol::Native => run_protocol_with_policy(input, service, policy),
    }
}

fn argument_error(message: &'static str) -> PortFailure {
    PortFailure::Domain(kiln_core::types::Diagnostic::new(
        "radicle_arguments",
        "radicle.arguments",
        message,
    ))
}

fn main() {
    match run() {
        Ok(lines) => {
            let output = format!("{}\n", lines.join("\n"));
            if std::io::stdout().write_all(output.as_bytes()).is_err() {
                eprintln!("radicle broker output write failed");
                std::process::exit(INPUT_OUTPUT_EXIT_CODE);
            }
        }
        Err(error) => {
            report_error(&error);
            std::process::exit(PROTOCOL_EXIT_CODE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_selection_is_explicit_and_aspen_requires_exact_paths() {
        let lattice = parse_argument_values(
            ["--profile", "radicle.json", "--runtime", "lattice"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap_or_else(|_| unreachable!("lattice arguments"));
        assert_eq!(lattice.runtime, Runtime::Lattice);

        let aspen = parse_argument_values(
            [
                "--profile",
                "radicle.json",
                "--runtime",
                "aspen",
                "--aspen-profile",
                "aspen.json",
                "--aspen-socket",
                "/run/kiln/aspen.sock",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .unwrap_or_else(|_| unreachable!("Aspen arguments"));
        assert_eq!(aspen.runtime, Runtime::Aspen);
        assert!(aspen.aspen_profile.is_some());
        assert!(aspen.aspen_socket.is_some());

        assert!(
            parse_argument_values(
                ["--profile", "radicle.json", "--runtime", "aspen"]
                    .into_iter()
                    .map(str::to_string),
            )
            .is_err()
        );
        assert!(
            parse_argument_values(
                [
                    "--profile",
                    "radicle.json",
                    "--runtime",
                    "lattice",
                    "--aspen-profile",
                    "aspen.json",
                ]
                .into_iter()
                .map(str::to_string),
            )
            .is_err()
        );
    }
}
