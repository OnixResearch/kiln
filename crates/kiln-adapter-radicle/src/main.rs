use std::io::{Read, Write};
use std::path::PathBuf;

use kiln_adapter_lattice::{LatticeAdapter, UnixWorkflowExchange};
use kiln_adapter_radicle::defelo::{MAX_DEFELO_REQUEST_BYTES, run_defelo_protocol_with_policy};
use kiln_adapter_radicle::{
    MAX_TRIGGER_LINE_BYTES, RadicleShellConfig, RadicleWorkflowService, SqliteReplayStore,
    admit_config, run_protocol_with_policy,
};
use kiln_provider_api::{PortFailure, TriggerLifecyclePolicy, TriggerWaitMode};

const INPUT_OUTPUT_EXIT_CODE: i32 = 1;
const PROTOCOL_EXIT_CODE: i32 = 2;
const PROFILE_MAXIMUM_BYTES: u64 = 1_048_576;
const PROFILE_ARGUMENT: &str = "--profile";
const PROTOCOL_ARGUMENT: &str = "--protocol";
const DEFELO_PROTOCOL: &str = "defelo";
const NATIVE_PROTOCOL: &str = "native";
const BASE_ARGUMENT_COUNT: usize = 3;
const PROTOCOL_ARGUMENT_COUNT: usize = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Protocol {
    Defelo,
    Native,
}

#[derive(Debug)]
struct Arguments {
    profile: PathBuf,
    protocol: Protocol,
}

fn parse_arguments() -> Result<Arguments, &'static str> {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.len() != BASE_ARGUMENT_COUNT && arguments.len() != PROTOCOL_ARGUMENT_COUNT {
        return Err(
            "usage: kiln-adapter-radicle --profile PROFILE.json [--protocol defelo|native]",
        );
    }
    if arguments.get(1).map(String::as_str) != Some(PROFILE_ARGUMENT) {
        return Err("first argument must be --profile");
    }
    let profile = arguments
        .get(2)
        .map(PathBuf::from)
        .ok_or("profile path is missing")?;
    let protocol = if arguments.len() == BASE_ARGUMENT_COUNT {
        Protocol::Defelo
    } else {
        if arguments.get(3).map(String::as_str) != Some(PROTOCOL_ARGUMENT) {
            return Err("optional argument must be --protocol");
        }
        match arguments.get(4).map(String::as_str) {
            Some(DEFELO_PROTOCOL) => Protocol::Defelo,
            Some(NATIVE_PROTOCOL) => Protocol::Native,
            _ => return Err("protocol must be defelo or native"),
        }
    };
    Ok(Arguments { profile, protocol })
}

fn load_profile(path: &std::path::Path) -> Result<RadicleShellConfig, &'static str> {
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
    let config = load_profile(&arguments.profile).map_err(argument_error)?;
    let admitted = admit_config(config)?;
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
    let policy = TriggerLifecyclePolicy {
        mode: TriggerWaitMode::Wait,
        max_polls: admitted.shell.maximum_polls,
    };
    let maximum_input = match arguments.protocol {
        Protocol::Defelo => MAX_DEFELO_REQUEST_BYTES,
        Protocol::Native => MAX_TRIGGER_LINE_BYTES,
    };
    let input = read_stdin_bound(maximum_input).map_err(argument_error)?;
    let mut service = RadicleWorkflowService::new(lattice, replay, admitted.shell);
    match arguments.protocol {
        Protocol::Defelo => run_defelo_protocol_with_policy(&input, &mut service, policy),
        Protocol::Native => run_protocol_with_policy(&input, &mut service, policy),
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
