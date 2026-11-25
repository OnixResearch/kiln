//! Pinned-TLS HTTPS client for dispatching reviewed runbooks (`#97`) and
//! fan-outs (`#98`) to `kiln-web-service`.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::sync::Arc;
use std::time::Duration;

use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::runbook_routing::{RoutingClaimPolicy, RoutingParamValue};
use crate::secret::ResolvedSecret;
use crate::{domain, infrastructure};

const MAX_RESPONSE_BYTES_LIMIT: usize = 16 * 1024 * 1024;
const MAX_TIMEOUT_MS: u64 = 120_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebDispatchConfig {
    pub service_origin: String,
    pub timeout_ms: u64,
    pub maximum_response_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunbookDispatchPayload {
    pub request_id: String,
    pub target: String,
    pub params: BTreeMap<String, RoutingParamValue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunbookFanoutPayload {
    pub request_id: String,
    pub targets: Vec<String>,
    pub target_globs: Vec<String>,
    pub params: BTreeMap<String, RoutingParamValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrency: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_cap: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_policy: Option<RoutingClaimPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct RunbookDispatchOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub runbook_id: String,
    pub target: String,
    pub source_revision: String,
    pub run_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct RunbookFanoutOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub fanout_id: String,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    #[serde(default)]
    code: Option<String>,
}

pub trait RunbookDispatchPort {
    fn dispatch_runbook(
        &mut self,
        runbook_id: &str,
        payload: &RunbookDispatchPayload,
    ) -> PortResult<RunbookDispatchOutcome>;

    fn dispatch_fanout(
        &mut self,
        runbook_id: &str,
        payload: &RunbookFanoutPayload,
    ) -> PortResult<RunbookFanoutOutcome>;
}

pub struct WebRunbookDispatchClient {
    service_origin: String,
    token: ResolvedSecret,
    maximum_response_bytes: usize,
    agent: ureq::Agent,
}

impl std::fmt::Debug for WebRunbookDispatchClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebRunbookDispatchClient")
            .field("service_origin", &self.service_origin)
            .field("maximum_response_bytes", &self.maximum_response_bytes)
            .field("token", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl WebRunbookDispatchClient {
    pub fn new(
        config: WebDispatchConfig,
        token: ResolvedSecret,
        ca_certificate_pem: &[u8],
    ) -> PortResult<Self> {
        validate_https_origin(&config.service_origin)?;
        validate_bounds(&config, &token)?;
        let certificates = rustls_pemfile::certs(&mut Cursor::new(ca_certificate_pem))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                domain(
                    "web_dispatch_ca",
                    "service_ca",
                    "service CA certificate PEM is invalid",
                )
            })?;
        if certificates.is_empty() {
            return Err(domain(
                "web_dispatch_ca",
                "service_ca",
                "service CA certificate PEM is empty",
            ));
        }
        let mut roots = rustls::RootCertStore::empty();
        for cert in certificates {
            roots.add(cert).map_err(|_| {
                domain(
                    "web_dispatch_ca",
                    "service_ca",
                    "service CA certificate could not be installed",
                )
            })?;
        }
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| {
            infrastructure(
                "web_dispatch_tls",
                "TLS client versions could not be configured",
            )
        })?
        .with_root_certificates(roots)
        .with_no_client_auth();
        let timeout = Duration::from_millis(config.timeout_ms);
        let agent = ureq::AgentBuilder::new()
            .https_only(true)
            .redirects(0)
            .try_proxy_from_env(false)
            .max_idle_connections(0)
            .timeout(timeout)
            .timeout_connect(timeout)
            .tls_config(Arc::new(tls))
            .build();
        Ok(Self {
            service_origin: config.service_origin,
            token,
            maximum_response_bytes: config.maximum_response_bytes,
            agent,
        })
    }

    #[cfg(any(test, feature = "protocol-fixtures"))]
    pub fn new_fixture(config: WebDispatchConfig, token: ResolvedSecret) -> PortResult<Self> {
        validate_bounds(&config, &token)?;
        let authority = config
            .service_origin
            .strip_prefix("http://")
            .ok_or_else(|| {
                domain(
                    "web_dispatch_origin",
                    "service_origin",
                    "fixture origin must be a loopback HTTP origin",
                )
            })?;
        let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
            domain(
                "web_dispatch_origin",
                "service_origin",
                "fixture origin must declare a loopback port",
            )
        })?;
        if !matches!(host, "127.0.0.1" | "[::1]" | "localhost")
            || port.parse::<u16>().ok().is_none_or(|p| p == 0)
        {
            return Err(domain(
                "web_dispatch_origin",
                "service_origin",
                "fixture origin must be a loopback HTTP origin",
            ));
        }
        let timeout = Duration::from_millis(config.timeout_ms);
        let agent = ureq::AgentBuilder::new()
            .https_only(false)
            .redirects(0)
            .try_proxy_from_env(false)
            .max_idle_connections(0)
            .timeout(timeout)
            .timeout_connect(timeout)
            .build();
        Ok(Self {
            service_origin: config.service_origin,
            token,
            maximum_response_bytes: config.maximum_response_bytes,
            agent,
        })
    }

    fn post_json(&self, path: &str, body: &[u8]) -> PortResult<Vec<u8>> {
        let authorization = Zeroizing::new(format!("Bearer {}", self.token.as_str()?));
        let url = format!("{}{path}", self.service_origin);
        let result = self
            .agent
            .post(&url)
            .set("Authorization", &authorization)
            .set("Origin", &self.service_origin)
            .set("Content-Type", "application/json")
            .send_bytes(body);
        let response = match result {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(_)) => {
                return Err(PortFailure::Unknown(InfrastructureFailure {
                    code: "web_dispatch_transport_unknown".to_string(),
                    message: "runbook dispatch transport outcome is unknown".to_string(),
                }));
            }
        };
        let status = response.status();
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(self.maximum_response_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                PortFailure::Unknown(InfrastructureFailure {
                    code: "web_dispatch_body_unknown".to_string(),
                    message: "runbook dispatch response body could not be read".to_string(),
                })
            })?;
        if bytes.len() > self.maximum_response_bytes {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "web_dispatch_body_bound".to_string(),
                message: "runbook dispatch response exceeded its byte bound".to_string(),
            }));
        }
        if (200..=202).contains(&status) {
            return Ok(bytes);
        }
        if status == 408 {
            return Err(PortFailure::Unknown(InfrastructureFailure {
                code: "web_dispatch_http_408".to_string(),
                message: "runbook dispatch request timed out; outcome is unknown".to_string(),
            }));
        }
        if status == 429 {
            return Err(PortFailure::Retryable(InfrastructureFailure {
                code: "web_dispatch_rate_limited".to_string(),
                message: "runbook dispatch request was rate limited".to_string(),
            }));
        }
        if (400..500).contains(&status) {
            let code = serde_json::from_slice::<ApiErrorBody>(&bytes)
                .ok()
                .and_then(|err| err.code)
                .filter(|code| valid_error_code(code))
                .unwrap_or_else(|| format!("web_dispatch_http_{status}"));
            return Err(domain(
                &code,
                "runbook_dispatch",
                &format!("runbook service refused dispatch with HTTP {status}"),
            ));
        }
        Err(PortFailure::Unknown(InfrastructureFailure {
            code: format!("web_dispatch_http_{status}"),
            message: format!("runbook service returned HTTP {status}"),
        }))
    }
}

impl RunbookDispatchPort for WebRunbookDispatchClient {
    fn dispatch_runbook(
        &mut self,
        runbook_id: &str,
        payload: &RunbookDispatchPayload,
    ) -> PortResult<RunbookDispatchOutcome> {
        if !crate::runbook_routing::valid_runbook_id(runbook_id) {
            return Err(domain(
                "runbook_id_invalid",
                "runbook_id",
                "runbook identifier is invalid",
            ));
        }
        let body = serde_json::to_vec(payload).map_err(|_| {
            infrastructure(
                "web_dispatch_encode",
                "runbook dispatch payload could not be encoded",
            )
        })?;
        let bytes = self.post_json(&format!("/api/runbooks/{runbook_id}/dispatch"), &body)?;
        serde_json::from_slice(&bytes).map_err(|_| {
            PortFailure::Unknown(InfrastructureFailure {
                code: "web_dispatch_decode_unknown".to_string(),
                message: "runbook dispatch response could not be decoded".to_string(),
            })
        })
    }

    fn dispatch_fanout(
        &mut self,
        runbook_id: &str,
        payload: &RunbookFanoutPayload,
    ) -> PortResult<RunbookFanoutOutcome> {
        if !crate::runbook_routing::valid_runbook_id(runbook_id) {
            return Err(domain(
                "runbook_id_invalid",
                "runbook_id",
                "runbook identifier is invalid",
            ));
        }
        let body = serde_json::to_vec(payload).map_err(|_| {
            infrastructure(
                "web_dispatch_encode",
                "fan-out dispatch payload could not be encoded",
            )
        })?;
        let bytes = self.post_json(&format!("/api/runbooks/{runbook_id}/fanout"), &body)?;
        serde_json::from_slice(&bytes).map_err(|_| {
            PortFailure::Unknown(InfrastructureFailure {
                code: "web_dispatch_decode_unknown".to_string(),
                message: "fan-out dispatch response could not be decoded".to_string(),
            })
        })
    }
}

pub(crate) fn validate_https_origin(origin: &str) -> PortResult<()> {
    let Some(authority) = origin.strip_prefix("https://") else {
        return Err(domain(
            "web_dispatch_origin",
            "service_origin",
            "service origin must use https://",
        ));
    };
    if authority.is_empty()
        || origin.len() > 512
        || origin.ends_with('/')
        || authority.contains(['/', '?', '#', '@'])
        || origin
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
    {
        return Err(domain(
            "web_dispatch_origin",
            "service_origin",
            "service origin is invalid",
        ));
    }
    Ok(())
}

fn validate_bounds(config: &WebDispatchConfig, token: &ResolvedSecret) -> PortResult<()> {
    if config.timeout_ms == 0 || config.timeout_ms > MAX_TIMEOUT_MS {
        return Err(domain(
            "web_dispatch_timeout",
            "timeout_ms",
            "service dispatch timeout is outside bounds",
        ));
    }
    if config.maximum_response_bytes == 0
        || config.maximum_response_bytes > MAX_RESPONSE_BYTES_LIMIT
    {
        return Err(domain(
            "web_dispatch_response_bound",
            "maximum_response_bytes",
            "service dispatch response bound is invalid",
        ));
    }
    if !token.as_bytes().iter().all(|b| (b'!'..=b'~').contains(b)) {
        return Err(domain(
            "web_dispatch_token",
            "service_token",
            "service bearer token is not valid header material",
        ));
    }
    Ok(())
}

fn valid_error_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 128
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::net::TcpListener;

    #[test]
    fn http_408_is_unknown_and_429_is_retryable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
        let addr = listener.local_addr().unwrap_or_else(|_| unreachable!());
        let handle = std::thread::spawn(move || {
            for status_line in ["408 Request Timeout", "429 Too Many Requests"] {
                let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!());
                let _ = crate::tests::read_http_request(&mut stream);
                let body = r#"{"code":"transient"}"#;
                let reply = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .write_all(reply.as_bytes())
                    .unwrap_or_else(|_| unreachable!());
            }
        });
        let token =
            ResolvedSecret::from_bytes(b"web-secret".to_vec()).unwrap_or_else(|_| unreachable!());
        let mut client = WebRunbookDispatchClient::new_fixture(
            WebDispatchConfig {
                service_origin: format!("http://{addr}"),
                timeout_ms: 5_000,
                maximum_response_bytes: 65_536,
            },
            token,
        )
        .unwrap_or_else(|_| unreachable!());
        let payload = RunbookDispatchPayload {
            request_id: "gh-rb-test".to_string(),
            target: "OnixResearch/kiln@refs/heads/main".to_string(),
            params: BTreeMap::new(),
        };
        let err_408 = client.dispatch_runbook("flake-update", &payload);
        assert!(
            matches!(err_408, Err(PortFailure::Unknown(ref f)) if f.code == "web_dispatch_http_408"),
            "expected Unknown for 408, got {err_408:?}"
        );
        let err_429 = client.dispatch_runbook("flake-update", &payload);
        assert!(
            matches!(err_429, Err(PortFailure::Retryable(ref f)) if f.code == "web_dispatch_rate_limited"),
            "expected Retryable for 429, got {err_429:?}"
        );
        handle.join().unwrap_or_else(|_| unreachable!());
    }
}
