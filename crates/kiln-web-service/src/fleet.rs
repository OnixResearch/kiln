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
pub const MAX_MACHINE_LABEL_BYTES: usize = 63;
pub const MAX_PANE_READ_LINES: u32 = 1_000;

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

pub(crate) const JSON_CONTENT_TYPE: &str = "application/json; charset=utf-8";
pub(crate) const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";
pub(crate) const TEXT_CONTENT_TYPE: &str = "text/plain; charset=utf-8";

pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) content_type: &'static str,
    pub(crate) body: Vec<u8>,
}

pub(crate) fn classify_content_type(header: Option<&str>) -> &'static str {
    let Some(raw) = header else {
        return JSON_CONTENT_TYPE;
    };
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.starts_with("text/plain") {
        if normalized.contains("version=0.0.4") {
            PROMETHEUS_CONTENT_TYPE
        } else {
            TEXT_CONTENT_TYPE
        }
    } else {
        JSON_CONTENT_TYPE
    }
}

#[derive(Deserialize)]
struct ClaimsEnvelope {
    claims: Vec<kiln_core::fleet::FleetClaim>,
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
        let content_type = classify_content_type(
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
        );
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
            content_type,
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

    pub(crate) fn live_claims(&self) -> Result<Vec<kiln_core::fleet::FleetClaim>, ()> {
        let reply = self.call("GET", "/v1/claims", None)?;
        if !(200..300).contains(&reply.status) {
            return Err(());
        }
        let envelope: ClaimsEnvelope = serde_json::from_slice(&reply.body).map_err(|_| ())?;
        for claim in &envelope.claims {
            claim.scope.validate().map_err(|_| ())?;
            claim.holder.validate().map_err(|_| ())?;
        }
        Ok(envelope.claims)
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
/// The mapping is an explicit allowlist: no caller-controlled suffix, fragment
/// or unvalidated query reaches the fleet service. Swarm routes are strictly
/// read-only (`GET`).
// r[impl kiln.fleet.swarm.console]
// r[impl kiln.fleet.unified_console_surface]
// r[impl kiln.fleet.live_pane_and_machine_census]
pub fn upstream_path(method: &str, target: &str) -> Option<String> {
    let rest = target.strip_prefix("/api/fleet/")?;
    if rest.contains('#')
        || rest
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return None;
    }
    let (path, query) = match rest.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (rest, None),
    };
    let segments: Vec<&str> = path.split('/').collect();
    let identity = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    match (method, segments.as_slice()) {
        ("GET", ["health"]) if query.is_none() => Some("/v1/health".into()),
        ("GET", ["machines"]) if query.is_none() => Some("/v1/machines".into()),
        ("POST", ["machines", name, "drain"])
            if query.is_none() && machine_label(name) =>
        {
            Some(format!("/v1/machines/{name}/drain"))
        }
        ("GET", ["claims"]) => {
            let suffix = match query {
                None => String::new(),
                Some(raw) => format!("?{}", validated_claims_query(raw)?),
            };
            Some(format!("/v1/claims{suffix}"))
        }
        ("GET", ["work"]) if query.is_none() => Some("/v1/work".into()),
        ("GET", ["attempts"]) if query.is_none() => Some("/v1/attempts".into()),
        ("GET", ["attempts", id, "pane"]) if identity(id) => {
            let suffix = match query {
                None => String::new(),
                Some(raw) => format!("?{}", validated_pane_query(raw)?),
            };
            Some(format!("/v1/attempts/{id}/pane{suffix}"))
        }
        ("GET", ["decisions"]) if query.is_none() => Some("/v1/decisions".into()),
        ("GET", ["metrics"]) if query.is_none() => Some("/v1/metrics".into()),
        ("GET", ["version"]) if query.is_none() => Some("/v1/version".into()),
        ("GET", ["dispatch", "status"]) if query.is_none() => Some("/v1/dispatch/status".into()),
        ("POST", ["dispatch", action @ ("pause" | "resume")]) if query.is_none() => {
            Some(format!("/v1/dispatch/{action}"))
        }
        ("GET", ["swarms"]) if query.is_none() => Some("/v1/swarms".into()),
        ("GET", ["swarms", id]) if query.is_none() && identity(id) => {
            Some(format!("/v1/swarms/{id}"))
        }
        ("GET", ["swarms", id, "board"]) if identity(id) => {
            let suffix = match query {
                None => String::new(),
                Some(raw) => format!("?{}", validated_board_query(raw)?),
            };
            Some(format!("/v1/swarms/{id}/board{suffix}"))
        }
        ("GET", ["swarms", id, "graph"]) if identity(id) => {
            let suffix = match query {
                None => String::new(),
                Some(raw) => format!("?{}", validated_graph_query(raw)?),
            };
            Some(format!("/v1/swarms/{id}/graph{suffix}"))
        }
        ("POST", ["claims"]) if query.is_none() => Some("/v1/claims".into()),
        ("POST", ["claims", id, action @ ("renew" | "release")])
            if query.is_none() && identity(id) =>
        {
            Some(format!("/v1/claims/{id}/{action}"))
        }
        ("POST", ["attempts", id, action @ ("cancel" | "prompt" | "interrupt")])
            if query.is_none() && identity(id) =>
        {
            Some(format!("/v1/attempts/{id}/{action}"))
        }
        ("POST", ["decisions", id, "answer"]) if query.is_none() && identity(id) => {
            Some(format!("/v1/decisions/{id}/answer"))
        }
        _ => None,
    }
}

fn machine_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_MACHINE_LABEL_BYTES
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn validated_claims_query(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let (key, value) = raw.split_once('=')?;
    if key != "include" || value.is_empty() || value.contains('&') {
        return None;
    }
    let mut seen_foreign = false;
    let mut seen_expired = false;
    let mut tokens = Vec::new();
    for token in value.split(',') {
        match token {
            "foreign" if !seen_foreign => {
                seen_foreign = true;
                tokens.push("foreign");
            }
            "expired" if !seen_expired => {
                seen_expired = true;
                tokens.push("expired");
            }
            _ => return None,
        }
    }
    Some(format!("include={}", tokens.join(",")))
}

fn validated_pane_query(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let (key, value) = raw.split_once('=')?;
    if key != "lines"
        || value.is_empty()
        || value.contains('&')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let parsed = value.parse::<u32>().ok()?;
    if !(1..=MAX_PANE_READ_LINES).contains(&parsed) {
        return None;
    }
    Some(format!("lines={parsed}"))
}

fn validated_graph_query(raw: &str) -> Option<String> {
    let (key, value) = raw.split_once('=')?;
    if key != "up_to_seq" || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let parsed = value.parse::<u64>().ok()?;
    Some(format!("up_to_seq={parsed}"))
}

fn validated_board_query(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for pair in raw.split('&') {
        let (key, value) = pair.split_once('=')?;
        match key {
            "since" | "limit" => {
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                let parsed = value.parse::<u64>().ok()?;
                parts.push(format!("{key}={parsed}"));
            }
            "seqs" => {
                if value.is_empty() {
                    return None;
                }
                let mut seqs = Vec::new();
                for token in value.split(',') {
                    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) {
                        return None;
                    }
                    seqs.push(token.parse::<u64>().ok()?.to_string());
                }
                parts.push(format!("seqs={}", seqs.join(",")));
            }
            "member" => {
                let valid = !value.is_empty()
                    && value.len() <= 64
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
                if !valid {
                    return None;
                }
                parts.push(format!("member={value}"));
            }
            _ => return None,
        }
    }
    Some(parts.join("&"))
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
        // Metrics and version proxy allowlist (r[kiln.fleet.unified_console_surface]).
        assert_eq!(
            upstream_path("GET", "/api/fleet/metrics").as_deref(),
            Some("/v1/metrics")
        );
        assert_eq!(
            upstream_path("GET", "/api/fleet/version").as_deref(),
            Some("/v1/version")
        );
        for refused in [
            ("POST", "/api/fleet/metrics"),
            ("PUT", "/api/fleet/metrics"),
            ("DELETE", "/api/fleet/metrics"),
            ("POST", "/api/fleet/version"),
            ("DELETE", "/api/fleet/version"),
            ("GET", "/api/fleet/metrics/"),
            ("GET", "/api/fleet/metrics/extra"),
            ("GET", "/api/fleet/version/extra"),
            ("GET", "/api/fleet/metrics?x=1"),
            ("GET", "/api/fleet/metrics?"),
            ("GET", "/api/fleet/version?x=1"),
            ("GET", "/api/fleet/metrics#frag"),
            ("GET", "/api/fleet/version#frag"),
            ("GET", "/api/fleet/metrics/../claims"),
            ("GET", "/api/fleet/../metrics"),
            ("GET", "/api/fleet/%2e%2e/metrics"),
            ("GET", "/api/fleet/metrics%2f.."),
            ("GET", "/api/fleet/metrics%3fx=1"),
            ("GET", "/api/fleet/metrics\r\n"),
            ("GET", "/api/fleet//metrics"),
        ] {
            assert_eq!(upstream_path(refused.0, refused.1), None, "{refused:?}");
        }
        // Read-only swarm proxy allowlist.
        assert_eq!(
            upstream_path("GET", "/api/fleet/swarms").as_deref(),
            Some("/v1/swarms")
        );
        assert_eq!(
            upstream_path("GET", &format!("/api/fleet/swarms/{ID}")).as_deref(),
            Some(format!("/v1/swarms/{ID}").as_str())
        );
        assert_eq!(
            upstream_path(
                "GET",
                &format!("/api/fleet/swarms/{ID}/board?since=2&limit=20")
            )
            .as_deref(),
            Some(format!("/v1/swarms/{ID}/board?since=2&limit=20").as_str())
        );
        assert_eq!(
            upstream_path("GET", &format!("/api/fleet/swarms/{ID}/graph?up_to_seq=5")).as_deref(),
            Some(format!("/v1/swarms/{ID}/graph?up_to_seq=5").as_str())
        );
        for refused in [
            ("POST", "/api/fleet/swarms".to_string()),
            ("POST", format!("/api/fleet/swarms/{ID}/members")),
            ("POST", format!("/api/fleet/swarms/{ID}/board")),
            ("POST", format!("/api/fleet/swarms/{ID}/verdicts")),
            ("POST", format!("/api/fleet/swarms/{ID}/failover")),
            ("GET", format!("/api/fleet/swarms/{ID}/graph?up_to_seq=abc")),
            (
                "GET",
                format!("/api/fleet/swarms/{ID}/graph?up_to_seq=1&extra=2"),
            ),
            ("GET", format!("/api/fleet/swarms/{ID}/board?bogus=1")),
            ("GET", format!("/api/fleet/swarms/{ID}?x=1")),
        ] {
            assert_eq!(upstream_path(refused.0, &refused.1), None, "{refused:?}");
        }
    }

    #[test]
    // r[verify kiln.fleet.live_pane_and_machine_census]
    fn p1_control_plane_routes_are_strictly_allowlisted() {
        assert_eq!(
            upstream_path("POST", "/api/fleet/machines/aspen3/drain").as_deref(),
            Some("/v1/machines/aspen3/drain")
        );
        assert_eq!(
            upstream_path("GET", &format!("/api/fleet/attempts/{ID}/pane")).as_deref(),
            Some(format!("/v1/attempts/{ID}/pane").as_str())
        );
        assert_eq!(
            upstream_path("GET", &format!("/api/fleet/attempts/{ID}/pane?lines=200")).as_deref(),
            Some(format!("/v1/attempts/{ID}/pane?lines=200").as_str())
        );
        assert_eq!(
            upstream_path("GET", &format!("/api/fleet/attempts/{ID}/pane?lines=1000")).as_deref(),
            Some(format!("/v1/attempts/{ID}/pane?lines=1000").as_str())
        );
        assert_eq!(
            upstream_path("POST", &format!("/api/fleet/attempts/{ID}/prompt")).as_deref(),
            Some(format!("/v1/attempts/{ID}/prompt").as_str())
        );
        assert_eq!(
            upstream_path("POST", &format!("/api/fleet/attempts/{ID}/interrupt")).as_deref(),
            Some(format!("/v1/attempts/{ID}/interrupt").as_str())
        );
        assert_eq!(
            upstream_path("GET", "/api/fleet/claims?include=foreign").as_deref(),
            Some("/v1/claims?include=foreign")
        );
        assert_eq!(
            upstream_path("GET", "/api/fleet/claims?include=expired").as_deref(),
            Some("/v1/claims?include=expired")
        );
        assert_eq!(
            upstream_path("GET", "/api/fleet/claims?include=foreign,expired").as_deref(),
            Some("/v1/claims?include=foreign,expired")
        );
        assert_eq!(
            upstream_path("GET", "/api/fleet/dispatch/status").as_deref(),
            Some("/v1/dispatch/status")
        );
        assert_eq!(
            upstream_path("POST", "/api/fleet/dispatch/pause").as_deref(),
            Some("/v1/dispatch/pause")
        );
        assert_eq!(
            upstream_path("POST", "/api/fleet/dispatch/resume").as_deref(),
            Some("/v1/dispatch/resume")
        );

        for refused in [
            ("GET", "/api/fleet/machines/aspen3/drain".to_string()),
            ("POST", "/api/fleet/machines/Aspen3/drain".to_string()),
            ("POST", "/api/fleet/machines/../drain".to_string()),
            ("POST", "/api/fleet/machines/aspen3/drain?x=1".to_string()),
            ("POST", format!("/api/fleet/attempts/{ID}/pane")),
            ("GET", format!("/api/fleet/attempts/{ID}/pane?")),
            ("GET", format!("/api/fleet/attempts/{ID}/pane?lines=0")),
            ("GET", format!("/api/fleet/attempts/{ID}/pane?lines=1001")),
            ("GET", format!("/api/fleet/attempts/{ID}/pane?lines=abc")),
            ("GET", format!("/api/fleet/attempts/{ID}/pane?lines=10&extra=1")),
            ("GET", format!("/api/fleet/attempts/{ID}/pane?foo=1")),
            ("GET", format!("/api/fleet/attempts/not-hex/pane")),
            ("GET", format!("/api/fleet/attempts/{ID}/prompt")),
            ("POST", format!("/api/fleet/attempts/{ID}/prompt?x=1")),
            ("GET", format!("/api/fleet/attempts/{ID}/interrupt")),
            ("POST", format!("/api/fleet/attempts/{ID}/interrupt?x=1")),
            ("GET", "/api/fleet/claims?".to_string()),
            ("GET", "/api/fleet/claims?include=".to_string()),
            ("GET", "/api/fleet/claims?include=bogus".to_string()),
            ("GET", "/api/fleet/claims?include=foreign,foreign".to_string()),
            ("GET", "/api/fleet/claims?include=foreign,".to_string()),
            ("GET", "/api/fleet/claims?other=1".to_string()),
            ("POST", "/api/fleet/dispatch/status".to_string()),
            ("GET", "/api/fleet/dispatch/pause".to_string()),
            ("GET", "/api/fleet/dispatch/resume".to_string()),
            ("POST", "/api/fleet/dispatch/pause?x=1".to_string()),
            ("GET", "/api/fleet/dispatch/status?x=1".to_string()),
        ] {
            assert_eq!(upstream_path(refused.0, &refused.1), None, "{refused:?}");
        }
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
        assert_eq!(reply.content_type, JSON_CONTENT_TYPE);
        assert_eq!(reply.body, br#"{"attempts":[]}"#);
        let (request, body) = server.join().unwrap_or_else(|_| unreachable!("server"));
        assert!(request.starts_with("POST /v1/claims HTTP/1.1\r\n"));
        assert_eq!(body, br#"{"scope":"repo"}"#);
    }

    #[test]
    // r[verify kiln.fleet.unified_console_surface.scenario.proxy]
    fn preserves_prometheus_and_json_content_types_for_metrics_and_version() {
        let listener =
            TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!("listener"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|_| unreachable!("address"));
        let server = std::thread::spawn(move || {
            for _ in 0..3 {
                let (stream, _) = listener.accept().unwrap_or_else(|_| unreachable!("accept"));
                let mut reader =
                    BufReader::new(stream.try_clone().unwrap_or_else(|_| unreachable!("clone")));
                let mut request_line = String::new();
                reader
                    .read_line(&mut request_line)
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
                let path = request_line.split_whitespace().nth(1).unwrap_or("/");
                let (status, content_type, body) = match path {
                    "/v1/metrics" => (
                        "200 OK",
                        "text/plain; version=0.0.4; charset=utf-8",
                        "# TYPE kiln_fleet_claims gauge\nkiln_fleet_claims 2\n",
                    ),
                    "/v1/version" => (
                        "200 OK",
                        "application/json",
                        r#"{"machine":"m","version":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
                    ),
                    _ => ("404 Not Found", "application/json", r#"{"error":"nope"}"#),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .try_clone()
                    .unwrap_or_else(|_| unreachable!("response"))
                    .write_all(response.as_bytes())
                    .unwrap_or_else(|_| unreachable!("write"));
            }
        });
        let client = Client::new(&profile(address)).unwrap_or_else(|_| unreachable!("client"));
        let metrics = client
            .call("GET", "/v1/metrics", None)
            .unwrap_or_else(|_| unreachable!("metrics"));
        assert_eq!(metrics.status, 200);
        assert_eq!(metrics.content_type, PROMETHEUS_CONTENT_TYPE);
        assert!(String::from_utf8_lossy(&metrics.body).contains("kiln_fleet_claims 2"));

        let version = client
            .call("GET", "/v1/version", None)
            .unwrap_or_else(|_| unreachable!("version"));
        assert_eq!(version.status, 200);
        assert_eq!(version.content_type, JSON_CONTENT_TYPE);
        assert!(String::from_utf8_lossy(&version.body).contains("aaaaaaaaaaaaaaaa"));

        let missing = client
            .call("GET", "/v1/unknown", None)
            .unwrap_or_else(|_| unreachable!("missing"));
        assert_eq!(missing.status, 404);
        assert_eq!(missing.content_type, JSON_CONTENT_TYPE);
        server.join().unwrap_or_else(|_| unreachable!("server"));
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

    #[test]
    fn live_claims_rejects_uncanonical_and_malformed_claim_scopes() {
        use kiln_core::fleet::{
            ClaimState, FleetAgent, FleetClaim, FleetScope, FleetTarget, MirrorStatus, ScopePath,
            parse_scope_text,
        };
        use kiln_core::types::ContentId;

        let make_claim = |scope: FleetScope| FleetClaim {
            identity: ContentId::from_blake3_hex("f".repeat(64))
                .unwrap_or_else(|e| unreachable!("{e:?}")),
            scope,
            holder: FleetAgent {
                tool: "codex".into(),
                host: "aspen3".into(),
                session: "s-claim".into(),
            },
            purpose: "test".into(),
            fence: 1,
            granted_ms: 1_000,
            expires_ms: 600_000,
            state: ClaimState::Open,
            attempt: None,
            claim_mirror: MirrorStatus::Posted { message_id: None },
            release_mirror: MirrorStatus::Pending,
        };

        let cases = [
            // 1. Non-lowercase repository -> rejected
            (
                make_claim(FleetScope {
                    target: FleetTarget::Repository("OnixResearch/Kiln".into()),
                    path: ScopePath::Whole,
                }),
                false,
            ),
            // 2. Unnormalized path -> rejected
            (
                make_claim(FleetScope {
                    target: FleetTarget::Repository("onixresearch/kiln".into()),
                    path: ScopePath::Path("./crates/".into()),
                }),
                false,
            ),
            // 3. Traversal path -> rejected
            (
                make_claim(FleetScope {
                    target: FleetTarget::Repository("onixresearch/kiln".into()),
                    path: ScopePath::Path("crates/../secret".into()),
                }),
                false,
            ),
            // 4. Canonical claim -> accepted
            (
                make_claim(
                    parse_scope_text("OnixResearch/Kiln .")
                        .unwrap_or_else(|e| unreachable!("{e:?}")),
                ),
                true,
            ),
        ];

        for (claim, expect_ok) in cases {
            let body = serde_json::to_string(&json!({ "claims": [claim] }))
                .unwrap_or_else(|e| unreachable!("{e:?}"));
            let listener =
                TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!("listener"));
            let address = listener
                .local_addr()
                .unwrap_or_else(|_| unreachable!("address"));
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap_or_else(|_| unreachable!("accept"));
                let mut reader =
                    BufReader::new(stream.try_clone().unwrap_or_else(|_| unreachable!("clone")));
                loop {
                    let mut line = String::new();
                    reader
                        .read_line(&mut line)
                        .unwrap_or_else(|_| unreachable!("header"));
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .try_clone()
                    .unwrap_or_else(|_| unreachable!("response"))
                    .write_all(response.as_bytes())
                    .unwrap_or_else(|_| unreachable!("write"));
            });
            let client = Client::new(&profile(address)).unwrap_or_else(|_| unreachable!("client"));
            let res = client.live_claims();
            server.join().unwrap_or_else(|_| unreachable!("server"));
            if expect_ok {
                let claims = res.unwrap_or_else(|()| unreachable!("canonical claim should pass"));
                assert_eq!(claims.len(), 1);
            } else {
                assert!(res.is_err(), "non-canonical claim scope must fail closed");
            }
        }
    }
}
