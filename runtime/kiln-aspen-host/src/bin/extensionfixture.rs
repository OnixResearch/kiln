#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the test-only binary mirrors the published extension process boundary"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln Aspen extension names preserve their protocol ownership"
)]
#![allow(
    unknown_lints,
    tigerstyle::ambient_env,
    reason = "the test-only composition shell captures explicit process arguments once"
)]

use std::io::{Read, Write};

use kiln_adapter_aspen::profile::{AspenRuntimeProfileDocument, admit_profile};
use kiln_aspen_extension::KilnAspenCallback;

const PROFILE_MAXIMUM_BYTES: u64 = 1_048_576;
const PROFILE_OVERFLOW_SENTINEL_BYTES: u64 = 1;
const READ_OVERFLOW_SENTINEL_BYTES: usize = 1;
const FAILURE_EXIT_CODE: u8 = 1;
const RUNTIME_PROFILE_ARGUMENT: &str = "--runtime-profile";
const RUNTIME_PROFILE_IDENTITY_ARGUMENT: &str = "--runtime-profile-identity";
const MAXIMUM_ARGUMENT_PAIRS: usize = 2;

#[derive(Debug, Default)]
struct ArgumentFields {
    runtime_profile: Option<std::path::PathBuf>,
    runtime_profile_identity: Option<String>,
}

fn main() -> std::process::ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            match writeln!(std::io::stderr().lock(), "{message}") {
                Ok(()) | Err(_) => {}
            }
            std::process::ExitCode::from(FAILURE_EXIT_CODE)
        }
    }
}

fn run(arguments: impl Iterator<Item = String>) -> Result<(), String> {
    let (runtime_profile, expected_identity) = parse_arguments(arguments)?;
    let file = std::fs::File::open(&runtime_profile)
        .map_err(|error| format!("open callback profile: {error}"))?;
    let read_limit = PROFILE_MAXIMUM_BYTES
        .checked_add(PROFILE_OVERFLOW_SENTINEL_BYTES)
        .ok_or_else(|| "callback profile bound overflowed".to_string())?;
    let mut profile_bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut profile_bytes)
        .map_err(|error| format!("read callback profile: {error}"))?;
    let maximum_profile_bytes = usize::try_from(PROFILE_MAXIMUM_BYTES)
        .map_err(|_| "callback profile bound does not fit usize".to_string())?;
    if profile_bytes.len() > maximum_profile_bytes {
        return Err("callback profile exceeds its named byte bound".to_string());
    }
    let document: AspenRuntimeProfileDocument = serde_json::from_slice(&profile_bytes)
        .map_err(|error| format!("decode callback profile: {error}"))?;
    let profile = document
        .into_runtime_profile()
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let admitted =
        admit_profile(&profile).map_err(|error| format!("{}: {}", error.code, error.message))?;
    if admitted.profile_identity.as_str() != expected_identity {
        return Err("callback profile identity differs from the host binding".to_string());
    }
    let input_limit = admitted
        .profile
        .bounds
        .max_callback_input_bytes
        .checked_add(READ_OVERFLOW_SENTINEL_BYTES)
        .ok_or_else(|| "callback input read bound overflowed".to_string())?;
    let input_limit = u64::try_from(input_limit)
        .map_err(|_| "callback input read bound does not fit u64".to_string())?;
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take(input_limit)
        .read_to_end(&mut input)
        .map_err(|error| format!("callback input read failed: {error}"))?;
    if input.len() > admitted.profile.bounds.max_callback_input_bytes {
        return Err("callback input exceeds its named byte bound".to_string());
    }
    let maximum_output_bytes = admitted.profile.bounds.max_callback_output_bytes;
    let output = KilnAspenCallback::new(admitted)
        .invoke_aspen_native(&input)
        .map_err(|error| format!("callback denied: {error}"))?;
    if output.len() > maximum_output_bytes {
        return Err("callback output exceeds its named byte bound".to_string());
    }
    std::io::stdout()
        .lock()
        .write_all(&output)
        .map_err(|error| format!("callback output write failed: {error}"))
}

fn parse_arguments(
    mut arguments: impl Iterator<Item = String>,
) -> Result<(std::path::PathBuf, String), String> {
    let mut fields = ArgumentFields::default();
    for _argument_index in 0..MAXIMUM_ARGUMENT_PAIRS {
        let Some(argument) = arguments.next() else {
            break;
        };
        let value = arguments
            .next()
            .ok_or_else(|| format!("{argument} requires a value"))?;
        match argument.as_str() {
            RUNTIME_PROFILE_ARGUMENT => set_once(
                &mut fields.runtime_profile,
                std::path::PathBuf::from(value),
                RUNTIME_PROFILE_ARGUMENT,
            )?,
            RUNTIME_PROFILE_IDENTITY_ARGUMENT => set_once(
                &mut fields.runtime_profile_identity,
                value,
                RUNTIME_PROFILE_IDENTITY_ARGUMENT,
            )?,
            _ => return Err(format!("unknown argument {argument}")),
        }
    }
    if arguments.next().is_some() {
        return Err("callback argument count exceeds its closed interface".to_string());
    }
    Ok((
        fields
            .runtime_profile
            .ok_or_else(|| format!("{RUNTIME_PROFILE_ARGUMENT} is required"))?,
        fields
            .runtime_profile_identity
            .ok_or_else(|| format!("{RUNTIME_PROFILE_IDENTITY_ARGUMENT} is required"))?,
    ))
}

fn set_once<T>(field: &mut Option<T>, value: T, label: &str) -> Result<(), String> {
    if field.replace(value).is_some() {
        return Err(format!("{label} was supplied more than once"));
    }
    Ok(())
}
