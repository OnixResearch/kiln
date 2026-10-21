use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

use kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile;
use kiln_adapter_aspen::transport::{
    decode_aspen_unix_request, encode_aspen_unix_response, handle_aspen_unix_request,
};

use crate::error::{HostBridgeError, Result};
use crate::runtime::KilnAspenHostRuntime;

const FRAME_OVERFLOW_SENTINEL_BYTES: usize = 1;
const SOCKET_MODE: u32 = 0o660;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub socket_path: std::path::PathBuf,
    pub maximum_frame_bytes: usize,
    pub maximum_requests: usize,
    pub timeout: std::time::Duration,
}

impl ServerConfig {
    pub fn validate(&self) -> Result<()> {
        let has_unsafe_component = self.socket_path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        });
        if !self.socket_path.is_absolute()
            || self.socket_path.file_name().is_none()
            || has_unsafe_component
            || self.maximum_frame_bytes == 0
            || self.maximum_requests == 0
            || self.timeout.is_zero()
        {
            return Err(HostBridgeError::Admission(
                "Unix server requires a safe absolute path and positive bounds".to_string(),
            ));
        }
        Ok(())
    }
}

// r[impl kiln.aspen_host.transport]
pub fn serve(
    profile: &AdmittedAspenRuntimeProfile,
    runtime: &mut KilnAspenHostRuntime,
    config: &ServerConfig,
) -> Result<()> {
    config.validate()?;
    if std::fs::symlink_metadata(&config.socket_path).is_ok() {
        return Err(HostBridgeError::Io(
            "Aspen host socket path already exists".to_string(),
        ));
    }
    let listener = UnixListener::bind(&config.socket_path)
        .map_err(|error| HostBridgeError::Io(format!("bind Aspen host socket: {error}")))?;
    std::fs::set_permissions(
        &config.socket_path,
        std::fs::Permissions::from_mode(SOCKET_MODE),
    )
    .map_err(|error| HostBridgeError::Io(format!("set Aspen host socket permissions: {error}")))?;

    let service_result = serve_requests(profile, runtime, config, &listener);
    let shutdown_result = runtime.shutdown();
    let cleanup_result = std::fs::remove_file(&config.socket_path)
        .map_err(|error| HostBridgeError::Io(format!("remove Aspen host socket: {error}")));
    service_result.and(shutdown_result).and(cleanup_result)
}

fn serve_requests(
    profile: &AdmittedAspenRuntimeProfile,
    runtime: &mut KilnAspenHostRuntime,
    config: &ServerConfig,
    listener: &UnixListener,
) -> Result<()> {
    for _request_index in 0..config.maximum_requests {
        let (mut stream, _address) = listener.accept().map_err(|error| {
            HostBridgeError::Io(format!("accept Aspen host connection: {error}"))
        })?;
        if let Err(error) = stream.set_read_timeout(Some(config.timeout)) {
            eprintln!("Aspen host connection setup failed: {error}");
            continue;
        }
        if let Err(error) = stream.set_write_timeout(Some(config.timeout)) {
            eprintln!("Aspen host connection setup failed: {error}");
            continue;
        }
        let read_limit = config
            .maximum_frame_bytes
            .checked_add(FRAME_OVERFLOW_SENTINEL_BYTES)
            .ok_or_else(|| HostBridgeError::Protocol("request frame bound overflow".to_string()))?;
        let read_limit = u64::try_from(read_limit).map_err(|_| {
            HostBridgeError::Protocol("request frame bound does not fit u64".to_string())
        })?;
        let mut request_bytes = Vec::new();
        if let Err(error) = (&mut stream)
            .take(read_limit)
            .read_to_end(&mut request_bytes)
        {
            eprintln!("Aspen host request read failed: {error}");
            continue;
        }
        let request = match decode_aspen_unix_request(&request_bytes, config.maximum_frame_bytes) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("Aspen host request rejected: {error}");
                continue;
            }
        };
        let response = handle_aspen_unix_request(profile, runtime, request);
        let response_bytes = match encode_aspen_unix_response(&response, config.maximum_frame_bytes)
        {
            Ok(response_bytes) => response_bytes,
            Err(error) => {
                eprintln!("Aspen host response rejected: {error}");
                continue;
            }
        };
        if let Err(error) = stream.write_all(&response_bytes) {
            eprintln!("Aspen host response write failed: {error}");
            continue;
        }
    }
    Ok(())
}
