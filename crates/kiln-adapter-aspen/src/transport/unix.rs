use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Component;
use std::path::PathBuf;
use std::time::Duration;

use kiln_core::types::{ContentId, ExternalRef};
use serde::{Deserialize, Serialize};

use super::{AspenServiceIngress, AspenServiceTransport, ServiceTransportOutcome};
use crate::profile::AdmittedAspenRuntimeProfile;

pub const EXCHANGE_REQUEST_SCHEMA: &str = "kiln.aspen-runtime.unix-request.v2";
pub const EXCHANGE_RESPONSE_SCHEMA: &str = "kiln.aspen-runtime.unix-response.v2";
const READ_OVERFLOW_SENTINEL_BYTES: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspenUnixOperation {
    Submit,
    Reconcile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AspenUnixRequest {
    pub schema: String,
    pub operation: AspenUnixOperation,
    pub profile_identity: ContentId,
    pub ingress: Option<AspenServiceIngress>,
    pub operation_id: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AspenUnixResponse {
    pub schema: String,
    pub profile_identity: ContentId,
    pub operation_id: ExternalRef,
    pub outcome: ServiceTransportOutcome,
}

// r[impl kiln.aspen_host.transport]
pub fn decode_aspen_unix_request(
    bytes: &[u8],
    maximum_frame_bytes: usize,
) -> Result<AspenUnixRequest, &'static str> {
    if maximum_frame_bytes == 0 || bytes.len() > maximum_frame_bytes {
        return Err("Aspen Unix request exceeds its positive frame bound");
    }
    let request: AspenUnixRequest =
        serde_json::from_slice(bytes).map_err(|_| "Aspen Unix request JSON is invalid")?;
    if request.schema != EXCHANGE_REQUEST_SCHEMA || request.operation_id.validate().is_err() {
        return Err("Aspen Unix request schema or operation identity is invalid");
    }
    match (&request.operation, &request.ingress) {
        (AspenUnixOperation::Submit, Some(ingress))
            if ingress.operation_id == request.operation_id
                && ingress.profile_identity == request.profile_identity => {}
        (AspenUnixOperation::Reconcile, None) => {}
        (AspenUnixOperation::Submit | AspenUnixOperation::Reconcile, _) => {
            return Err("Aspen Unix request operation and body do not match");
        }
    }
    Ok(request)
}

// r[impl kiln.aspen_host.transport]
pub fn encode_aspen_unix_response(
    response: &AspenUnixResponse,
    maximum_frame_bytes: usize,
) -> Result<Vec<u8>, &'static str> {
    if maximum_frame_bytes == 0
        || response.schema != EXCHANGE_RESPONSE_SCHEMA
        || response.operation_id.validate().is_err()
    {
        return Err("Aspen Unix response schema, identity, or frame bound is invalid");
    }
    let bytes =
        serde_json::to_vec(response).map_err(|_| "Aspen Unix response JSON encoding failed")?;
    if bytes.len() > maximum_frame_bytes {
        return Err("Aspen Unix response exceeds its frame bound");
    }
    Ok(bytes)
}

#[derive(Clone, Debug)]
pub struct UnixAspenServiceTransport {
    socket_path: PathBuf,
    maximum_frame_bytes: usize,
    timeout: Duration,
}

impl UnixAspenServiceTransport {
    pub fn new(
        socket_path: PathBuf,
        maximum_frame_bytes: usize,
        timeout: Duration,
    ) -> Result<Self, &'static str> {
        if maximum_frame_bytes == 0 || timeout.is_zero() {
            return Err("Aspen Unix transport requires positive frame and timeout bounds");
        }
        let has_unsafe_component = socket_path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir));
        if !socket_path.is_absolute() || socket_path.file_name().is_none() || has_unsafe_component {
            return Err("Aspen Unix transport requires an admitted absolute socket path");
        }
        Ok(Self {
            socket_path,
            maximum_frame_bytes,
            timeout,
        })
    }

    #[must_use]
    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }

    fn exchange(
        &self,
        profile: &AdmittedAspenRuntimeProfile,
        request: &AspenUnixRequest,
    ) -> ServiceTransportOutcome {
        let request_bytes = match serde_json::to_vec(request) {
            Ok(bytes) if bytes.len() <= self.maximum_frame_bytes => bytes,
            Ok(_) | Err(_) => return ServiceTransportOutcome::Malformed,
        };
        let mut stream = match UnixStream::connect(&self.socket_path) {
            Ok(stream) => stream,
            Err(_) => return ServiceTransportOutcome::Unavailable,
        };
        if stream.set_read_timeout(Some(self.timeout)).is_err()
            || stream.set_write_timeout(Some(self.timeout)).is_err()
        {
            return ServiceTransportOutcome::Unavailable;
        }
        if stream.write_all(&request_bytes).is_err()
            || stream.shutdown(std::net::Shutdown::Write).is_err()
        {
            return unknown_after_write(request);
        }
        let Some(read_limit) = self
            .maximum_frame_bytes
            .checked_add(READ_OVERFLOW_SENTINEL_BYTES)
        else {
            return ServiceTransportOutcome::Malformed;
        };
        let Ok(read_limit) = u64::try_from(read_limit) else {
            return ServiceTransportOutcome::Malformed;
        };
        let mut response_bytes = Vec::new();
        if stream
            .take(read_limit)
            .read_to_end(&mut response_bytes)
            .is_err()
        {
            return unknown_after_write(request);
        }
        if response_bytes.len() > self.maximum_frame_bytes {
            return ServiceTransportOutcome::Malformed;
        }
        let response: AspenUnixResponse = match serde_json::from_slice(&response_bytes) {
            Ok(response) => response,
            Err(_) => return ServiceTransportOutcome::Malformed,
        };
        if response.schema != EXCHANGE_RESPONSE_SCHEMA
            || response.profile_identity != profile.profile_identity
            || response.operation_id != request.operation_id
        {
            return ServiceTransportOutcome::Malformed;
        }
        response.outcome
    }
}

impl AspenServiceTransport for UnixAspenServiceTransport {
    // r[impl kiln.aspen_runtime.ingress] r[impl kiln.aspen_runtime.composition]
    fn submit(
        &mut self,
        profile: &AdmittedAspenRuntimeProfile,
        request: &AspenServiceIngress,
    ) -> ServiceTransportOutcome {
        self.exchange(
            profile,
            &AspenUnixRequest {
                schema: EXCHANGE_REQUEST_SCHEMA.to_string(),
                operation: AspenUnixOperation::Submit,
                profile_identity: profile.profile_identity.clone(),
                ingress: Some(request.clone()),
                operation_id: request.operation_id.clone(),
            },
        )
    }

    fn reconcile(
        &mut self,
        profile: &AdmittedAspenRuntimeProfile,
        operation_id: &ExternalRef,
    ) -> ServiceTransportOutcome {
        self.exchange(
            profile,
            &AspenUnixRequest {
                schema: EXCHANGE_REQUEST_SCHEMA.to_string(),
                operation: AspenUnixOperation::Reconcile,
                profile_identity: profile.profile_identity.clone(),
                ingress: None,
                operation_id: operation_id.clone(),
            },
        )
    }
}

fn unknown_after_write(request: &AspenUnixRequest) -> ServiceTransportOutcome {
    if request.operation == AspenUnixOperation::Submit {
        ServiceTransportOutcome::DisconnectedAfterAcceptance {
            operation_id: request.operation_id.clone(),
        }
    } else {
        ServiceTransportOutcome::Unavailable
    }
}
