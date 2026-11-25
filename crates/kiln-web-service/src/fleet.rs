//! Bounded console projection of one loopback fleet coordinator.
//!
//! The console never becomes the coordinator: it forwards only the reviewed
//! fleet API reads and operator commands over the coordinator's loopback
//! listener, with explicit request and response bounds. Fleet identities and
//! domain decisions stay with the fleet service.

use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Largest mutation body forwarded to the fleet coordinator.
pub const MAX_FLEET_REQUEST_BYTES: usize = 65_536;
pub const FLEET_PROFILE_SCHEMA: &str = "kiln.web-fleet.v1";
pub const MAX_FLEET_PROFILE_BYTES: usize = 4_096;
pub const MIN_FLEET_TIMEOUT_MS: u64 = 250;
pub const MAX_FLEET_TIMEOUT_MS: u64 = 10_000;
pub const MIN_FLEET_RESPONSE_BYTES: usize = 65_536;
pub const MAX_FLEET_RESPONSE_BYTES: usize = 4_194_304;

/// Runtime-only console projection policy. It is deliberately outside the
/// canonical service profile: adding an observation endpoint must not change
/// the durable CI profile binding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetProfile {
    pub schema: String,
    /// A loopback socket address such as `127.0.0.1:8095` or `[::1]:8095`.
    pub address: String,
    pub timeout_ms: u64,
    pub max_response_bytes: usize,
}

impl FleetProfile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, MAX_FLEET_PROFILE_BYTES, false)
            .map_err(|_| "fleet profile unavailable or unsafe")?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "fleet profile malformed")?;
        profile.admit()?;
        Ok(profile)
    }

    pub fn admit(&self) -> Result<(), String> {
        let address: std::net::SocketAddr = self
            .address
            .parse()
            .map_err(|_| "fleet address must be one loopback socket address")?;
        if self.schema != FLEET_PROFILE_SCHEMA
            || !address.ip().is_loopback()
            || address.port() == 0
            || !(MIN_FLEET_TIMEOUT_MS..=MAX_FLEET_TIMEOUT_MS).contains(&self.timeout_ms)
            || !(MIN_FLEET_RESPONSE_BYTES..=MAX_FLEET_RESPONSE_BYTES)
                .contains(&self.max_response_bytes)
        {
            return Err("fleet endpoint bounds rejected".into());
        }
        Ok(())
    }
}

pub(crate) struct Client {
    address: std::net::SocketAddr,
    http: reqwest::blocking::Client,
    max_response_bytes: usize,
}

pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) body: Vec<u8>,
}

pub(crate) enum EventFrame {
    Snapshot { version: String, data: Vec<u8> },
    Heartbeat,
}

/// Long-poll wait leaves room for five bounded snapshot reads under the
/// console's 15 s connection deadline.
const EVENT_WAIT_MS: u64 = 4_000;
const EVENT_POLL_MS: u64 = 400;

impl Client {
    pub(crate) fn new(profile: &FleetProfile) -> Result<Self, String> {
        profile.admit()?;
        let address = profile
            .address
            .parse()
            .map_err(|_| "fleet address rejected")?;
        let http = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_millis(profile.timeout_ms))
            .build()
            .map_err(|_| "fleet client unavailable")?;
        Ok(Self {
            address,
            http,
            max_response_bytes: profile.max_response_bytes,
        })
    }

    pub(crate) fn call(&self, method: &str, path: &str, body: Option<&[u8]>) -> Result<Reply, ()> {
        let url = format!("http://{}{path}", self.address);
        let mut request = self.http.request(method.parse().map_err(|_| ())?, &url);
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_vec());
        }
        let response = request.send().map_err(|_| ())?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        response
            .take((self.max_response_bytes + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| ())?;
        if bytes.len() > self.max_response_bytes {
            return Err(());
        }
        Ok(Reply {
            status,
            body: bytes,
        })
    }

    fn json(&self, path: &str) -> Result<Value, ()> {
        let reply = self.call("GET", path, None)?;
        if !(200..300).contains(&reply.status) {
            return Err(());
        }
        serde_json::from_slice(&reply.body).map_err(|_| ())
    }

    fn version(&self) -> Result<String, ()> {
        let value = self
            .json("/v1/version")?
            .get("version")
            .and_then(Value::as_str)
            .ok_or(())?
            .to_owned();
        if value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            Ok(value)
        } else {
            Err(())
        }
    }

    /// Wait for a change after `after`, then return one snapshot.
    ///
    /// The version is read after the five bounded reads. A continuously
    /// active coordinator can advance while the snapshot is assembled, so the
    /// version names the state observed no earlier than the data; the next
    /// long-poll covers any write that landed during assembly. The reads are
    /// independent observations, not one cross-endpoint transaction.
    pub(crate) fn events(&self, after: Option<String>) -> Result<EventFrame, ()> {
        self.events_with_wait(after, EVENT_WAIT_MS)
    }

    fn events_with_wait(&self, after: Option<String>, wait_ms: u64) -> Result<EventFrame, ()> {
        if let Some(after) = after {
            let deadline = Instant::now()
                .checked_add(Duration::from_millis(wait_ms))
                .ok_or(())?;
            loop {
                if self.version()? != after {
                    break;
                }
                if Instant::now() >= deadline {
                    return Ok(EventFrame::Heartbeat);
                }
                std::thread::sleep(Duration::from_millis(EVENT_POLL_MS.min(wait_ms)));
            }
        }
        let health = self.json("/v1/health")?;
        let claims = self.json("/v1/claims")?;
        let work = self.json("/v1/work")?;
        let attempts = self.json("/v1/attempts")?;
        let decisions = self.json("/v1/decisions")?;
        let version = self.version()?;
        let data = serde_json::to_vec(&json!({
            "version": version,
            "health": health,
            "claims": claims,
            "work": work,
            "attempts": attempts,
            "decisions": decisions,
        }))
        .map_err(|_| ())?;
        Ok(EventFrame::Snapshot { version, data })
    }
}

/// Map one console route to the fleet coordinator's coordinator API.
///
/// The mapping is an explicit allowlist: no caller-controlled suffix, query or
/// fragment reaches the fleet service.
pub fn upstream_path(method: &str, target: &str) -> Option<String> {
    let path = target.strip_prefix("/api/fleet/")?;
    if path.contains(['?', '#']) {
        return None;
    }
    let segments: Vec<&str> = path.split('/').collect();
    let identity = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    match (method, segments.as_slice()) {
        ("GET", ["health"]) => Some("/v1/health".into()),
        ("GET", ["machines"]) => Some("/v1/machines".into()),
        ("GET", ["claims"]) => Some("/v1/claims".into()),
        ("GET", ["work"]) => Some("/v1/work".into()),
        ("GET", ["attempts"]) => Some("/v1/attempts".into()),
        ("GET", ["decisions"]) => Some("/v1/decisions".into()),
        ("POST", ["claims"]) => Some("/v1/claims".into()),
        ("POST", ["claims", id, action @ ("renew" | "release")]) if identity(id) => {
            Some(format!("/v1/claims/{id}/{action}"))
        }
        ("POST", ["attempts", id, "cancel"]) if identity(id) => {
            Some(format!("/v1/attempts/{id}/cancel"))
        }
        ("POST", ["decisions", id, "answer"]) if identity(id) => {
            Some(format!("/v1/decisions/{id}/answer"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn routes_are_an_exact_allowlist() {
        assert_eq!(
            upstream_path("GET", "/api/fleet/claims").as_deref(),
            Some("/v1/claims")
        );
        assert_eq!(upstream_path("GET", "/api/fleet/machines").as_deref(), Some("/v1/machines"));
        assert_eq!(upstream_path("POST", "/api/fleet/machines"), None);
        assert_eq!(
            upstream_path("GET", "/api/fleet/decisions").as_deref(),
            Some("/v1/decisions")
        );
        assert_eq!(
            upstream_path("POST", "/api/fleet/claims").as_deref(),
            Some("/v1/claims")
        );
        assert_eq!(
            upstream_path("POST", &format!("/api/fleet/claims/{ID}/release")).as_deref(),
            Some(format!("/v1/claims/{ID}/release").as_str())
        );
        assert_eq!(
            upstream_path("POST", &format!("/api/fleet/attempts/{ID}/cancel")).as_deref(),
            Some(format!("/v1/attempts/{ID}/cancel").as_str())
        );
        assert_eq!(
            upstream_path("POST", &format!("/api/fleet/decisions/{ID}/answer")).as_deref(),
            Some(format!("/v1/decisions/{ID}/answer").as_str())
        );
        for target in [
            "/api/fleet/health?x=1",
            "/api/fleet/claims/../health",
            "/api/fleet/claims/not-an-identity/renew",
            "/api/fleet/attempts/0123/handoff",
            "/api/fleet/admin",
            "/api/inventory",
        ] {
            assert_eq!(upstream_path("POST", target), None, "{target}");
        }
        assert_eq!(upstream_path("DELETE", "/api/fleet/claims"), None);
        assert_eq!(upstream_path("POST", "/api/fleet/health"), None);
    }

    #[test]
    fn forwards_one_request_and_bounds_the_response() {
        let listener =
            TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!("listener"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|_| unreachable!("address"));
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap_or_else(|_| unreachable!("accept"));
            let mut reader =
                BufReader::new(stream.try_clone().unwrap_or_else(|_| unreachable!("clone")));
            let mut request = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader
                    .read_line(&mut line)
                    .unwrap_or_else(|_| unreachable!("read"));
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value
                        .trim()
                        .parse()
                        .unwrap_or_else(|_| unreachable!("length"));
                }
                request.push_str(&line);
            }
            let mut body = vec![0; length];
            reader
                .read_exact(&mut body)
                .unwrap_or_else(|_| unreachable!("body"));
            let response = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"attempts\":[]}";
            stream
                .try_clone()
                .unwrap_or_else(|_| unreachable!("response stream"))
                .write_all(response)
                .unwrap_or_else(|_| unreachable!("write"));
            (request, body)
        });
        let client = Client::new(&profile(address)).unwrap_or_else(|_| unreachable!("client"));
        let reply = client
            .call("POST", "/v1/claims", Some(br#"{"scope":"repo"}"#))
            .unwrap_or_else(|_| unreachable!("reply"));
        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, br#"{"attempts":[]}"#);
        let (request, body) = server.join().unwrap_or_else(|_| unreachable!("server"));
        assert!(request.starts_with("POST /v1/claims HTTP/1.1\r\n"));
        assert_eq!(body, br#"{"scope":"repo"}"#);
    }

    #[test]
    fn rejects_an_oversized_response() {
        let listener =
            TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!("listener"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|_| unreachable!("address"));
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!("accept"));
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 70000\r\n\r\n");
            let _ = stream.write_all(&vec![b'x'; 70_000]);
        });
        let client = Client::new(&profile(address)).unwrap_or_else(|_| unreachable!("client"));
        assert!(client.call("GET", "/v1/claims", None).is_err());
        server.join().unwrap_or_else(|_| unreachable!("server"));
    }

    fn profile(address: std::net::SocketAddr) -> FleetProfile {
        FleetProfile {
            schema: FLEET_PROFILE_SCHEMA.into(),
            address: address.to_string(),
            timeout_ms: 1_000,
            max_response_bytes: 65_536,
        }
    }

    fn event_server(expected: usize) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
        let listener =
            TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!("listener"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|_| unreachable!("address"));
        let server = std::thread::spawn(move || {
            for _ in 0..expected {
                let (stream, _) = listener.accept().unwrap_or_else(|_| unreachable!("accept"));
                let mut reader =
                    BufReader::new(stream.try_clone().unwrap_or_else(|_| unreachable!("clone")));
                let mut request = String::new();
                reader
                    .read_line(&mut request)
                    .unwrap_or_else(|_| unreachable!("request"));
                loop {
                    let mut line = String::new();
                    reader
                        .read_line(&mut line)
                        .unwrap_or_else(|_| unreachable!("header"));
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                }
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let body = match path {
                    "/v1/version" => {
                        r#"{"version":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#
                    }
                    "/v1/health" => r#"{"machine":"m","version":"0","api":"v1"}"#,
                    "/v1/claims" => r#"{"claims":[]}"#,
                    "/v1/work" => r#"{"ready_unclaimed":0,"ranked":[],"blocked":[]}"#,
                    "/v1/attempts" => r#"{"attempts":[]}"#,
                    "/v1/decisions" => r#"{"decisions":[]}"#,
                    _ => r#"{}"#,
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .try_clone()
                    .unwrap_or_else(|_| unreachable!("response"))
                    .write_all(response.as_bytes())
                    .unwrap_or_else(|_| unreachable!("write"));
            }
        });
        (address, server)
    }

    #[test]
    fn events_wait_for_a_change_then_return_one_consistent_snapshot() {
        let (address, server) = event_server(1);
        let client = Client::new(&profile(address)).unwrap_or_else(|_| unreachable!("client"));
        assert!(matches!(
            client.events_with_wait(Some("a".repeat(64)), 0),
            Ok(EventFrame::Heartbeat)
        ));
        server.join().unwrap_or_else(|_| unreachable!("server"));

        let (address, server) = event_server(6);
        let client = Client::new(&profile(address)).unwrap_or_else(|_| unreachable!("client"));
        let Ok(EventFrame::Snapshot { version, data }) = client.events_with_wait(None, 0) else {
            unreachable!("snapshot")
        };
        assert_eq!(version, "a".repeat(64));
        let value: Value = serde_json::from_slice(&data).unwrap_or_else(|_| unreachable!("json"));
        assert_eq!(value.pointer("/version"), Some(&json!("a".repeat(64))));
        assert_eq!(value.pointer("/claims/claims"), Some(&json!([])));
        assert_eq!(value.pointer("/work/ready_unclaimed"), Some(&json!(0)));
        server.join().unwrap_or_else(|_| unreachable!("server"));
    }

    #[test]
    fn profile_admission_is_loopback_only() {
        let mut profile = profile(
            "127.0.0.1:8095"
                .parse()
                .unwrap_or_else(|_| unreachable!("address")),
        );
        assert!(profile.admit().is_ok());
        profile.address = "10.0.0.1:8095".into();
        assert!(profile.admit().is_err());
        profile.address = "127.0.0.1:8095".into();
        profile.timeout_ms = 100;
        assert!(profile.admit().is_err());
        profile.timeout_ms = 2_000;
        profile.max_response_bytes = 1_024;
        assert!(profile.admit().is_err());
        profile.max_response_bytes = 1_048_576;
        profile.schema = "kiln.web-fleet.v2".into();
        assert!(profile.admit().is_err());
    }
}
