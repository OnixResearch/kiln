use kiln_aspen_host::error::{HostBridgeError, Result};

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
pub(super) struct Arguments {
    pub(super) aspen_profile: std::path::PathBuf,
    pub(super) radicle_profile: std::path::PathBuf,
    pub(super) socket: std::path::PathBuf,
    pub(super) extension: std::path::PathBuf,
    pub(super) state_root: std::path::PathBuf,
    pub(super) maximum_requests: usize,
    pub(super) timeout_milliseconds: u64,
    pub(super) poll_interval_milliseconds: u64,
}

pub(super) fn parse_arguments(mut arguments: impl Iterator<Item = String>) -> Result<Arguments> {
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
            MAXIMUM_REQUESTS_ARGUMENT => set_usize(
                &mut fields.maximum_requests,
                &value,
                MAXIMUM_REQUESTS_ARGUMENT,
            )?,
            TIMEOUT_MILLISECONDS_ARGUMENT => set_u64(
                &mut fields.timeout_milliseconds,
                &value,
                TIMEOUT_MILLISECONDS_ARGUMENT,
            )?,
            POLL_INTERVAL_MILLISECONDS_ARGUMENT => set_u64(
                &mut fields.poll_interval_milliseconds,
                &value,
                POLL_INTERVAL_MILLISECONDS_ARGUMENT,
            )?,
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
