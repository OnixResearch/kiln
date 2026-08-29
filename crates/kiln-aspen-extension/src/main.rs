use std::io::{Read, Write};
use std::path::{Component, PathBuf};
use std::process::ExitCode;

use kiln_adapter_aspen::profile::{
    AdmittedAspenRuntimeProfile, AspenRuntimeProfileDocument, admit_profile,
};
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
    runtime_profile: Option<PathBuf>,
    runtime_profile_identity: Option<String>,
}

#[derive(Debug)]
struct Arguments {
    runtime_profile: PathBuf,
    runtime_profile_identity: String,
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            match writeln!(std::io::stderr().lock(), "{message}") {
                Ok(()) | Err(_) => {}
            }
            ExitCode::from(FAILURE_EXIT_CODE)
        }
    }
}

fn run(arguments: impl Iterator<Item = String>) -> Result<(), String> {
    let arguments = parse_arguments(arguments)?;
    let admitted = load_runtime_profile(&arguments)?;
    let read_limit = admitted
        .profile
        .bounds
        .max_callback_input_bytes
        .checked_add(READ_OVERFLOW_SENTINEL_BYTES)
        .ok_or_else(|| "callback input read bound overflowed".to_string())?;
    let read_limit = u64::try_from(read_limit)
        .map_err(|_| "callback input read bound does not fit u64".to_string())?;
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take(read_limit)
        .read_to_end(&mut input)
        .map_err(|error| format!("callback input read failed: {error}"))?;
    if input.len() > admitted.profile.bounds.max_callback_input_bytes {
        return Err("callback input exceeds its named byte bound".to_string());
    }
    let maximum_output_bytes = admitted.profile.bounds.max_callback_output_bytes;
    let callback = KilnAspenCallback::new(admitted);
    let output = callback
        .invoke_aspen_native(&input)
        .map_err(|error| format!("callback denied: {error}"))?;
    if output.len() > maximum_output_bytes {
        return Err("callback output exceeds its named byte bound".to_string());
    }
    std::io::stdout()
        .lock()
        .write_all(&output)
        .map_err(|error| format!("callback output write failed: {error}"))?;
    Ok(())
}

fn parse_arguments(mut arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
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
                PathBuf::from(value),
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
        return Err("argument count exceeds the closed extension interface".to_string());
    }
    let runtime_profile = required(fields.runtime_profile, RUNTIME_PROFILE_ARGUMENT)?;
    let has_unsafe_component = runtime_profile
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir));
    if !runtime_profile.is_absolute()
        || runtime_profile.file_name().is_none()
        || has_unsafe_component
    {
        return Err("runtime profile requires a safe absolute file path".to_string());
    }
    let runtime_profile_identity = required(
        fields.runtime_profile_identity,
        RUNTIME_PROFILE_IDENTITY_ARGUMENT,
    )?;
    if runtime_profile_identity.is_empty() {
        return Err("runtime profile identity must not be empty".to_string());
    }
    Ok(Arguments {
        runtime_profile,
        runtime_profile_identity,
    })
}

// r[impl kiln.aspen_host.extension_profile_binding]
fn load_runtime_profile(arguments: &Arguments) -> Result<AdmittedAspenRuntimeProfile, String> {
    let canonical_path = std::fs::canonicalize(&arguments.runtime_profile)
        .map_err(|error| format!("canonicalize runtime profile: {error}"))?;
    if canonical_path != arguments.runtime_profile {
        return Err("runtime profile path is not canonical".to_string());
    }
    let metadata = std::fs::symlink_metadata(&arguments.runtime_profile)
        .map_err(|error| format!("inspect runtime profile: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("runtime profile is not an admitted regular file".to_string());
    }
    let document: AspenRuntimeProfileDocument = load_json(&arguments.runtime_profile)?;
    let profile = document
        .into_runtime_profile()
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let admitted =
        admit_profile(&profile).map_err(|error| format!("{}: {}", error.code, error.message))?;
    if admitted.profile_identity.as_str() != arguments.runtime_profile_identity {
        return Err("runtime profile identity differs from the host binding".to_string());
    }
    Ok(admitted)
}

fn load_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    let file =
        std::fs::File::open(path).map_err(|error| format!("open runtime profile: {error}"))?;
    let read_limit = PROFILE_MAXIMUM_BYTES
        .checked_add(PROFILE_OVERFLOW_SENTINEL_BYTES)
        .ok_or_else(|| "runtime profile byte bound overflow".to_string())?;
    let mut bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read runtime profile: {error}"))?;
    let maximum_bytes = usize::try_from(PROFILE_MAXIMUM_BYTES)
        .map_err(|_| "runtime profile bound does not fit usize".to_string())?;
    if bytes.len() > maximum_bytes {
        return Err("runtime profile exceeds its named byte bound".to_string());
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("decode runtime profile: {error}"))
}

fn set_once<T>(field: &mut Option<T>, value: T, label: &str) -> Result<(), String> {
    if field.replace(value).is_some() {
        return Err(format!("{label} was supplied more than once"));
    }
    Ok(())
}

fn required<T>(value: Option<T>, label: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("{label} is required"))
}

#[cfg(test)]
#[path = "main/tests.rs"]
mod tests;
