//! Status publication (shell).
//!
//! Tangled spindles publish status by serving `getPipeline`; no status webhook
//! exists. Kiln additionally posts each changed `#pipeline` view to an
//! operator-declared endpoint, signed with the Tangled webhook HMAC header
//! format. `X-Tangled-Delivery` carries the view's BLAKE3 request identity, so
//! a receiver can de-duplicate re-sent deliveries.

use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::time::Duration;

use kiln_core::types::ContentId;
use kiln_provider_api::PortResult;

use crate::auth::webhook_signature;
use crate::error::{domain, infrastructure, retryable, unknown};
use crate::nsid;

/// Secrets longer than this are refused.
pub const MAXIMUM_SECRET_BYTES: usize = 4096;
/// Response bytes read from the status endpoint.
const MAXIMUM_RESPONSE_BYTES: u64 = 4096;
const SECRET_MODE_MASK: u32 = 0o077;
const HTTP_SUCCESS: std::ops::Range<u16> = 200..300;
const HTTP_CLIENT_ERROR: std::ops::Range<u16> = 400..500;
const USER_AGENT: &str = "kiln-adapter-tangled";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusDelivery {
    pub pipeline_id: String,
    pub request_identity: ContentId,
    pub body: Vec<u8>,
}

/// Status publication port.
pub trait StatusPort {
    /// Deliver one view. `Unknown` when the endpoint may have received it.
    fn publish(&mut self, delivery: &StatusDelivery) -> PortResult<()>;
}

/// Read an HMAC secret file that only the service user can read.
pub fn read_secret(path: &Path) -> PortResult<Vec<u8>> {
    let failure = || {
        domain(
            "tangled_secret",
            "secret_path",
            "Secret file is missing, shared or unbounded",
        )
    };
    let metadata = fs::metadata(path).map_err(|_| failure())?;
    // The secret must belong to the service user and be private to it.
    let owner_ok = metadata.uid() == own_uid();
    if !metadata.is_file() || metadata.permissions().mode() & SECRET_MODE_MASK != 0 || !owner_ok {
        return Err(failure());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| failure())?
        .take(MAXIMUM_SECRET_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| failure())?;
    while bytes.last().is_some_and(|byte| *byte == b'\n') {
        bytes.pop();
    }
    if bytes.is_empty() || bytes.len() > MAXIMUM_SECRET_BYTES {
        return Err(failure());
    }
    Ok(bytes)
}

fn own_uid() -> u32 {
    // The uid owning /proc/self is the process's effective uid.
    fs::metadata("/proc/self")
        .map(|metadata| metadata.uid())
        .unwrap_or(u32::MAX)
}

pub struct HttpStatusPublisher {
    endpoint: String,
    secret: Vec<u8>,
    agent: ureq::Agent,
}

impl std::fmt::Debug for HttpStatusPublisher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpStatusPublisher")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl HttpStatusPublisher {
    // r[impl kiln.tangled.status]
    pub fn new(endpoint: &str, secret: Vec<u8>, timeout_ms: u64) -> PortResult<Self> {
        if !endpoint.starts_with("https://") {
            return Err(domain(
                "tangled_status_endpoint",
                "status.endpoint",
                "Status endpoint must be HTTPS",
            ));
        }
        Self::build(endpoint, secret, timeout_ms)
    }

    /// Fixture publisher for a loopback `http://127.0.0.1` endpoint. Tests only.
    #[cfg(test)]
    pub(crate) fn fixture(endpoint: &str, secret: Vec<u8>, timeout_ms: u64) -> PortResult<Self> {
        if !endpoint.starts_with("http://127.0.0.1:") {
            return Err(domain(
                "tangled_status_endpoint",
                "status.endpoint",
                "Fixture endpoint must be loopback",
            ));
        }
        Self::build(endpoint, secret, timeout_ms)
    }

    fn build(endpoint: &str, secret: Vec<u8>, timeout_ms: u64) -> PortResult<Self> {
        if secret.is_empty() || secret.len() > MAXIMUM_SECRET_BYTES || timeout_ms == 0 {
            return Err(domain(
                "tangled_status_secret",
                "status",
                "Status secret or timeout is invalid",
            ));
        }
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_millis(timeout_ms))
            .redirects(0)
            .user_agent(USER_AGENT)
            .build();
        Ok(Self {
            endpoint: endpoint.to_string(),
            secret,
            agent,
        })
    }
}

impl StatusPort for HttpStatusPublisher {
    // r[impl kiln.tangled.status]
    fn publish(&mut self, delivery: &StatusDelivery) -> PortResult<()> {
        let signature = webhook_signature(&delivery.body, &self.secret).ok_or_else(|| {
            infrastructure(
                "tangled_status_signature",
                "Status signature could not be computed",
            )
        })?;
        let response = self
            .agent
            .post(&self.endpoint)
            .set("Content-Type", "application/json")
            .set("X-Tangled-Event", nsid::STATUS_EVENT)
            .set("X-Tangled-Delivery", delivery.request_identity.as_str())
            .set("X-Tangled-Pipeline", &delivery.pipeline_id)
            .set("X-Tangled-Signature-256", &signature)
            .send_bytes(&delivery.body);
        match response {
            Ok(response) if HTTP_SUCCESS.contains(&response.status()) => {
                let mut sink = Vec::new();
                // The answer carries no information; read it within a bound.
                let _ = response
                    .into_reader()
                    .take(MAXIMUM_RESPONSE_BYTES)
                    .read_to_end(&mut sink);
                Ok(())
            }
            Ok(_) => Err(unknown(
                "tangled_status_response",
                "Status endpoint answered outside 2xx",
            )),
            Err(ureq::Error::Status(code, _)) if HTTP_CLIENT_ERROR.contains(&code) => Err(domain(
                "tangled_status_refused",
                "status",
                "Status endpoint refused the delivery",
            )),
            Err(ureq::Error::Status(..)) => Err(unknown(
                "tangled_status_server",
                "Status endpoint failed after receiving the delivery",
            )),
            Err(ureq::Error::Transport(transport)) => match transport.kind() {
                // Nothing reached the endpoint: safe to retry.
                ureq::ErrorKind::Dns
                | ureq::ErrorKind::ConnectionFailed
                | ureq::ErrorKind::InvalidUrl => Err(retryable(
                    "tangled_status_unreachable",
                    "Status endpoint was not reached",
                )),
                // The request may have been received; the outcome is uncertain.
                _ => Err(unknown(
                    "tangled_status_transport",
                    "Status delivery outcome is unknown",
                )),
            },
        }
    }
}
