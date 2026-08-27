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

const EXCHANGE_REQUEST_SCHEMA: &str = "kiln.aspen-runtime.unix-request.v2";
const EXCHANGE_RESPONSE_SCHEMA: &str = "kiln.aspen-runtime.unix-response.v2";
const READ_OVERFLOW_SENTINEL_BYTES: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExchangeOperation {
    Submit,
    Reconcile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ExchangeRequest {
    schema: String,
    operation: ExchangeOperation,
    profile_identity: ContentId,
    ingress: Option<AspenServiceIngress>,
    operation_id: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ExchangeResponse {
    schema: String,
    profile_identity: ContentId,
    operation_id: ExternalRef,
    outcome: ServiceTransportOutcome,
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
        request: &ExchangeRequest,
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
        let response: ExchangeResponse = match serde_json::from_slice(&response_bytes) {
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
            &ExchangeRequest {
                schema: EXCHANGE_REQUEST_SCHEMA.to_string(),
                operation: ExchangeOperation::Submit,
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
            &ExchangeRequest {
                schema: EXCHANGE_REQUEST_SCHEMA.to_string(),
                operation: ExchangeOperation::Reconcile,
                profile_identity: profile.profile_identity.clone(),
                ingress: None,
                operation_id: operation_id.clone(),
            },
        )
    }
}

fn unknown_after_write(request: &ExchangeRequest) -> ServiceTransportOutcome {
    if request.operation == ExchangeOperation::Submit {
        ServiceTransportOutcome::DisconnectedAfterAcceptance {
            operation_id: request.operation_id.clone(),
        }
    } else {
        ServiceTransportOutcome::Unavailable
    }
}

#[cfg(test)]
pub(crate) fn encode_exchange_response(
    profile_identity: ContentId,
    operation_id: ExternalRef,
    outcome: ServiceTransportOutcome,
) -> Vec<u8> {
    serde_json::to_vec(&ExchangeResponse {
        schema: EXCHANGE_RESPONSE_SCHEMA.to_string(),
        profile_identity,
        operation_id,
        outcome,
    })
    .unwrap_or_else(|_| unreachable!("static Aspen Unix response"))
}
