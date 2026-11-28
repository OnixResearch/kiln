//! Disposable, loopback-only exploration of the real Kiln console.
//! No installed profile, credential, repository or execution provider is used.
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kiln_web_service::access::Principal;
use kiln_web_service::engine::{ApiResponse, Role};
use serde_json::{Value, json};

#[path = "explore/automation.rs"]
mod automation;
#[path = "explore/campaign.rs"]
mod campaign;
#[path = "explore/ci.rs"]
mod ci;
#[path = "explore/feeds.rs"]
mod feeds;
#[path = "explore/inventory.rs"]
mod inventory;
#[path = "explore/receipt.rs"]
mod receipt;
#[path = "explore/review.rs"]
mod review;
#[path = "explore/work.rs"]
mod work;
#[path = "explore/workers.rs"]
mod workers;

type Result<T = (), E = Box<dyn std::error::Error>> = std::result::Result<T, E>;
const ACTOR: &str = "local:operator";
const DEFAULT_PORT: u16 = 8787;
const MAX_HEADER_BYTES: usize = 16_384;
const MAX_BODY_BYTES: usize = 524_288;
const REQUEST_DEADLINE: Duration = Duration::from_secs(3);
const ACCEPT_INTERVAL: Duration = Duration::from_millis(25);
const SSE_DISCONNECT_POLL_INTERVAL: Duration = Duration::from_millis(200);
const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SSE_FLEET_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(4);
const MAX_SSE_STREAMS: usize = 32;
const SYNTHETIC_PROJECT_REPOSITORY_LABEL: &str = "example/release-service";
const EVENT_HUB_SCHEMA: &str = "kiln.events.v1";
const ATTENTION_SCHEMA: &str = "kiln.implementation-attention.v1";
const ADMISSION_SCHEMA: &str = "kiln.implementation-admission.v1";
const CAMPAIGN_INDEX_SCHEMA: &str = "kiln.implementation-campaigns.v1";
const REPOSITORIES_SCHEMA: &str = "kiln.repositories.v1";
const GUEST_MOVE_OVERVIEW_SCHEMA: &str = "kiln.fleet-guest-move-overview.v1";
const CHANNEL_CI: usize = 0;
const CHANNEL_FLEET: usize = 1;
const CHANNEL_CAIRN: usize = 2;
const CHANNEL_ATTENTION: usize = 3;
const CLAIM_LEASE_MS: u64 = 4 * 60 * 60 * 1_000;
const EXPIRING_CLAIM_REMAINING_MS: u64 = 10 * 60 * 1_000;
const HEALTHY_CLAIM_REMAINING_MS: u64 = 120 * 60 * 1_000;
const EXPIRED_CLAIM_AGE_MS: u64 = 25 * 60 * 1_000;
const STALE_OBSERVATION_MS: u64 = 12 * 60 * 1_000;
const FOREIGN_STALE_AFTER_MS: u64 = 6 * 60 * 60 * 1_000;
const SERVING_VALIDITY_WINDOW_MS: u64 = 5 * 60 * 1_000;

#[derive(serde::Serialize, Clone)]
struct PresentationRequirements {
    theme: String,
    density: String,
    viewport: String,
    themes: Vec<String>,
    densities: Vec<String>,
    viewports: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
struct InteractionRequirements {
    keyboard: bool,
    narrow: bool,
    dark: bool,
}

#[derive(serde::Serialize, Clone)]
struct RecipeIdentity {
    id: String,
    version: String,
    revision: String,
}

#[derive(Clone)]
struct NamedCase {
    case_id: String,
    recipe: (String, String),
    presentation: PresentationRequirements,
    interaction: InteractionRequirements,
    subjects: Vec<(String, String)>,
    seeder_source: Vec<u8>,
}

#[derive(serde::Serialize, Clone)]
struct Example {
    #[serde(skip_serializing_if = "Option::is_none")]
    case_id: Option<String>,
    title: String,
    href: String,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    recipe: Option<RecipeIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presentation: Option<PresentationRequirements>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interaction: Option<InteractionRequirements>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    subjects: std::collections::BTreeMap<String, String>,
}

impl Example {
    fn new(title: impl Into<String>, href: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            case_id: None,
            title: title.into(),
            href: href.into(),
            detail: detail.into(),
            recipe: None,
            source_revision: None,
            presentation: None,
            interaction: None,
            subjects: std::collections::BTreeMap::new(),
        }
    }
    fn explorer_source_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(include_bytes!("explore.rs"));
        bytes.extend_from_slice(include_bytes!("explore/automation.rs"));
        bytes.extend_from_slice(include_bytes!("explore/campaign.rs"));
        bytes.extend_from_slice(include_bytes!("explore/ci.rs"));
        bytes.extend_from_slice(include_bytes!("explore/feeds.rs"));
        bytes.extend_from_slice(include_bytes!("explore/receipt.rs"));
        bytes.extend_from_slice(include_bytes!("explore/review.rs"));
        bytes.extend_from_slice(include_bytes!("explore/work.rs"));
        bytes.extend_from_slice(include_bytes!("explore/workers.rs"));
        bytes.extend_from_slice(include_bytes!("explore/index.html"));
        bytes.extend_from_slice(include_bytes!("explore/index.js"));
        bytes.extend_from_slice(include_bytes!("explore/example.css"));
        bytes
    }
    fn explorer_source_revision() -> String {
        blake3::hash(&Self::explorer_source_bytes())
            .to_hex()
            .to_string()
    }
    fn named(
        case: NamedCase,
        title: impl Into<String>,
        href: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        let title = title.into();
        let href = href.into();
        let detail = detail.into();
        let (recipe_id, recipe_version) = &case.recipe;
        let mut hasher = blake3::Hasher::new();
        let mut update_field = |bytes: &[u8]| {
            hasher.update(&(bytes.len() as u64).to_be_bytes());
            hasher.update(bytes);
        };
        update_field(case.case_id.as_bytes());
        update_field(recipe_id.as_bytes());
        update_field(recipe_version.as_bytes());
        update_field(case.presentation.theme.as_bytes());
        update_field(case.presentation.density.as_bytes());
        update_field(case.presentation.viewport.as_bytes());
        for value in &case.presentation.themes {
            update_field(value.as_bytes());
        }
        for value in &case.presentation.densities {
            update_field(value.as_bytes());
        }
        for value in &case.presentation.viewports {
            update_field(value.as_bytes());
        }
        update_field(if case.interaction.keyboard {
            b"1"
        } else {
            b"0"
        });
        update_field(if case.interaction.narrow { b"1" } else { b"0" });
        update_field(if case.interaction.dark { b"1" } else { b"0" });
        update_field(&case.seeder_source);
        let recipe_revision = hasher.finalize().to_hex().to_string();
        let mut subjects_map = std::collections::BTreeMap::new();
        for (k, v) in &case.subjects {
            subjects_map.insert(k.clone(), v.clone());
        }
        Self {
            case_id: Some(case.case_id.clone()),
            title,
            href,
            detail,
            recipe: Some(RecipeIdentity {
                id: recipe_id.clone(),
                version: recipe_version.clone(),
                revision: recipe_revision,
            }),
            source_revision: Some(Self::explorer_source_revision()),
            presentation: Some(case.presentation.clone()),
            interaction: Some(case.interaction.clone()),
            subjects: subjects_map,
        }
    }
}

fn now_ms() -> Result<u64> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

fn valid_case_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
}
fn valid_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

// r[impl kiln.web.exploration.cases.resolution] Unknown cases are 404; moved recipes and other workspaces are 409.
fn resolve_case(
    examples: &[Example],
    workspace_id: &str,
    case_id: &str,
    recipe: &str,
    workspace: &str,
) -> Result<Value, (u16, &'static str)> {
    if !valid_case_id(case_id) || !valid_hex64(recipe) || !valid_hex64(workspace) {
        return Err((400, "Malformed case resolution input"));
    }
    if workspace != workspace_id {
        return Err((409, "Example case belongs to a different workspace"));
    }
    let example = examples
        .iter()
        .find(|example| example.case_id.as_deref() == Some(case_id))
        .ok_or((404, "Unknown exploration case"))?;
    let recipe_identity = example
        .recipe
        .as_ref()
        .ok_or((404, "Exploration case has no recipe"))?;
    if recipe != recipe_identity.revision {
        return Err((409, "Recipe revision moved or removed"));
    }
    Ok(json!({
        "workspace": workspace_id,
        "case_id": example.case_id,
        "recipe": recipe_identity,
        "source_revision": example.source_revision,
        "subjects": example.subjects,
        "route": example.href,
        "synthetic": true,
    }))
}

fn accepted(response: ApiResponse) -> Result<Value> {
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "Example seed rejected ({}): {}",
            response.status,
            String::from_utf8_lossy(&response.body)
        )
        .into());
    }
    Ok(serde_json::from_slice(&response.body)?)
}
fn json(status: u16, value: Value) -> ApiResponse {
    ApiResponse {
        status,
        content_type: "application/json",
        body: value.to_string().into_bytes(),
        download_name: None,
        headers: Vec::new(),
    }
}
fn error(status: u16, message: &str) -> ApiResponse {
    json(
        status,
        json!({"code":"example_request_rejected","message":message}),
    )
}
fn bytes(content_type: &'static str, body: &[u8]) -> ApiResponse {
    ApiResponse {
        status: 200,
        content_type,
        body: body.to_vec(),
        download_name: None,
        headers: Vec::new(),
    }
}

#[derive(Debug)]
struct Request {
    method: String,
    target: String,
    body: Vec<u8>,
}
// r[impl kiln.web.exploration.scenario.admission]
fn request(stream: &TcpStream, authority: &str) -> std::result::Result<Request, ApiResponse> {
    stream
        .set_read_timeout(Some(REQUEST_DEADLINE))
        .map_err(|_| error(400, "Request deadline unavailable"))?;
    decode_request(BufReader::new(stream), authority)
}
fn decode_request<R: BufRead>(
    mut reader: R,
    authority: &str,
) -> std::result::Result<Request, ApiResponse> {
    let mut header = Vec::new();
    loop {
        let start = header.len();
        reader
            .by_ref()
            .take((MAX_HEADER_BYTES + 1 - start) as u64)
            .read_until(b'\n', &mut header)
            .map_err(|_| error(400, "Request header unavailable"))?;
        let line = header
            .get(start..)
            .ok_or_else(|| error(400, "Invalid header boundary"))?;
        if header.len() > MAX_HEADER_BYTES || line.is_empty() || !line.ends_with(b"\r\n") {
            return Err(error(
                400,
                "Request header is malformed or exceeds its bound",
            ));
        }
        if line == b"\r\n" {
            break;
        }
    }
    let text =
        std::str::from_utf8(&header).map_err(|_| error(400, "Request header must be text"))?;
    let mut lines = text.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(target), Some(protocol), None) =
        (first.next(), first.next(), first.next(), first.next())
    else {
        return Err(error(
            400,
            "Only bounded GET and POST requests are supported",
        ));
    };
    if !matches!(method, "GET" | "POST")
        || protocol != "HTTP/1.1"
        || !target.starts_with('/')
        || target.starts_with("//")
        || target.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(error(
            400,
            "Only bounded GET and POST requests are supported",
        ));
    }
    let mut headers = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| error(400, "Malformed header"))?;
        let name = name.to_ascii_lowercase();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || headers.insert(name, value.trim()).is_some()
        {
            return Err(error(
                400,
                "Duplicate or malformed headers are not admitted",
            ));
        }
    }
    let origin = format!("http://{authority}");
    if headers.get("host").copied() != Some(authority)
        || headers.get("origin").is_some_and(|value| *value != origin)
        || headers
            .get("sec-fetch-site")
            .is_some_and(|value| *value == "cross-site")
    {
        return Err(error(
            403,
            "This example accepts only its exact loopback Host and Origin",
        ));
    }
    if headers.contains_key("transfer-encoding") {
        return Err(error(400, "Transfer encoding is not supported"));
    }
    let length = match headers.get("content-length") {
        Some(value) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => {
            value
                .parse::<usize>()
                .map_err(|_| error(413, "Request body exceeds its bound"))?
        }
        Some(_) => return Err(error(400, "Malformed Content-Length")),
        None => 0,
    };
    if length > MAX_BODY_BYTES {
        return Err(error(413, "Request body exceeds its bound"));
    }
    if method == "POST"
        && (headers.get("origin").copied() != Some(origin.as_str())
            || headers.get("content-type").is_none_or(|value| {
                !matches!(
                    *value,
                    "application/json" | "application/json; charset=utf-8"
                )
            }))
    {
        return Err(error(
            403,
            "Local writes require this exact Origin and JSON content",
        ));
    }
    if method == "GET" && length != 0 {
        return Err(error(400, "GET bodies are not supported"));
    }
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .map_err(|_| error(400, "Request body is incomplete"))?;
    Ok(Request {
        method: method.into(),
        target: target.into(),
        body,
    })
}

fn hex_id(label: &str) -> String {
    blake3::hash(label.as_bytes()).to_hex().to_string()
}

fn is_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn parse_events_channels(target: &str) -> std::result::Result<[bool; 4], ()> {
    let Some((_, query)) = target.split_once('?') else {
        return Ok([true, true, true, true]);
    };
    let value = query.strip_prefix("channels=").ok_or(())?;
    if value.is_empty() || value.contains('&') {
        return Err(());
    }
    let mut channels = [false; 4];
    for item in value.split(',') {
        match item {
            "ci" => channels[CHANNEL_CI] = true,
            "fleet" => channels[CHANNEL_FLEET] = true,
            "cairn" => channels[CHANNEL_CAIRN] = true,
            "attention" => channels[CHANNEL_ATTENTION] = true,
            _ => return Err(()),
        }
    }
    Ok(channels)
}

fn parse_fleet_event_after(target: &str) -> std::result::Result<Option<String>, ()> {
    let Some((_, query)) = target.split_once('?') else {
        return Ok(None);
    };
    let mut after = None;
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').ok_or(())?;
        if name != "after" || after.is_some() || !is_hex64(value) {
            return Err(());
        }
        after = Some(value.to_owned());
    }
    Ok(after)
}

#[derive(Clone, Copy, Default)]
struct ClaimsInclude {
    foreign: bool,
    expired: bool,
}

fn parse_claims_include(target: &str) -> std::result::Result<ClaimsInclude, ()> {
    let Some((_, query)) = target.split_once('?') else {
        return Ok(ClaimsInclude::default());
    };
    let (key, value) = query.split_once('=').ok_or(())?;
    if key != "include" || value.is_empty() || value.contains('&') {
        return Err(());
    }
    let mut include = ClaimsInclude::default();
    for token in value.split(',') {
        match token {
            "foreign" if !include.foreign => include.foreign = true,
            "expired" if !include.expired => include.expired = true,
            _ => return Err(()),
        }
    }
    Ok(include)
}

fn synthetic_fleet_version() -> String {
    "e".repeat(64)
}

fn synthetic_machines_json(now: u64) -> Value {
    json!({"synthetic": true, "machines": [
        {
            "machine": "example-desktop",
            "placement_configured": true,
            "observed_at_ms": now,
            "last_seen_at_ms": now,
            "state": "reporting",
            "health": "available",
            "draining": false,
            "sessions": 2,
            "slots": {
                "agent": {"used": 1, "capacity": 4},
                "ci": {"used": 1, "capacity": 2}
            },
            "core": {
                "state": "reporting",
                "observed_at_ms": now,
                "last_seen_at_ms": now,
                "terminals": 2,
                "statuses": {"idle": 0, "working": 1, "done": 0, "blocked": 1, "error": 0, "unknown": 0}
            }
        },
        {
            "machine": "example-worker",
            "placement_configured": true,
            "observed_at_ms": now,
            "last_seen_at_ms": null,
            "state": "unreachable",
            "health": "unavailable",
            "draining": false,
            "sessions": null,
            "error": {"class": "HerdrFailure", "code": "unreachable", "detail": "Simulated unreachable host (no provider contacted)"},
            "core": {"state": "unreachable", "observed_at_ms": now, "last_seen_at_ms": null, "terminals": null, "statuses": null}
        },
        {
            "machine": "example-stale",
            "placement_configured": true,
            "observed_at_ms": now.saturating_sub(600_000),
            "last_seen_at_ms": now.saturating_sub(600_000),
            "state": "stale",
            "health": "available",
            "draining": false,
            "sessions": 0,
            "slots": {"agent": {"used": 0, "capacity": 2}},
            "core": {"state": "stale", "observed_at_ms": now, "last_seen_at_ms": now.saturating_sub(600_000), "terminals": null, "statuses": null}
        },
        {
            "machine": "example-peer",
            "placement_configured": false,
            "observed_at_ms": null,
            "last_seen_at_ms": null,
            "state": "not_reporting",
            "health": "available",
            "draining": false,
            "sessions": null
        }
    ]})
}

fn synthetic_fleet_health_json() -> Value {
    json!({"synthetic": true, "api": "v1", "machine": "example-desktop", "version": "example"})
}

fn synthetic_claims_payload(now: u64, include: ClaimsInclude) -> Value {
    let attempt_active = "d".repeat(64);
    let claim_expiring = hex_id("synthetic-fleet-claim-expiring-1");
    let claim_healthy = hex_id("synthetic-fleet-claim-healthy-2");
    let claim_expired = hex_id("synthetic-fleet-claim-expired-3");

    let claims = vec![
        json!({
            "identity": claim_expiring,
            "scope": {"target": {"repository": "example/release-service"}, "path": {"path": "crates/release"}},
            "holder": {"tool": "antigravity", "host": "example-desktop", "session": "synth-session-1"},
            "purpose": "Synthetic coordination lease expiring soon (disposable example)",
            "fence": 1,
            "initial_fence": 1,
            "granted_ms": now.saturating_sub(CLAIM_LEASE_MS.saturating_sub(EXPIRING_CLAIM_REMAINING_MS)),
            "expires_ms": now.saturating_add(EXPIRING_CLAIM_REMAINING_MS),
            "state": "open",
            "attempt": attempt_active,
            "claim_mirror": {"posted": {"message_id": "synth-claim-101"}},
            "release_mirror": "pending"
        }),
        json!({
            "identity": claim_healthy,
            "scope": {"target": {"repository": "example-service"}, "path": {"path": "src/lib.rs"}},
            "holder": {"tool": "codex", "host": "example-desktop", "session": "synth-session-2"},
            "purpose": "Synthetic healthy coordination lease (disposable example)",
            "fence": 2,
            "initial_fence": 2,
            "granted_ms": now.saturating_sub(900_000),
            "expires_ms": now.saturating_add(HEALTHY_CLAIM_REMAINING_MS),
            "state": "open",
            "attempt": null,
            "claim_mirror": {"posted": {"message_id": "synth-claim-102"}},
            "release_mirror": "pending"
        }),
    ];

    let mut payload = json!({"synthetic": true, "claims": claims});
    if let Some(obj) = payload.as_object_mut() {
        if include.foreign {
            let foreign_date = now.saturating_sub(420_000);
            obj.insert(
                "foreign_claims".into(),
                json!([{
                    "scope": {"target": {"repository": "example/release-service"}, "path": {"path": "crates/release"}},
                    "scope_text": "example/release-service crates/release",
                    "message_id": "synth-foreign-404",
                    "author": "peer-agent@swarm.example.invalid",
                    "holder": "peer-agent on example-peer, session synth-peer-7",
                    "date_unix_ms": foreign_date,
                    "age_ms": now.saturating_sub(foreign_date),
                    "stale_after_ms": FOREIGN_STALE_AFTER_MS,
                    "mirror_status": "foreign"
                }]),
            );
            obj.insert(
                "foreign_mirror".into(),
                json!({"configured": true, "status": "ok", "error": null}),
            );
        }
        if include.expired {
            let expired_at = now.saturating_sub(EXPIRED_CLAIM_AGE_MS);
            obj.insert(
                "expired_claims".into(),
                json!([{
                    "identity": claim_expired,
                    "scope": {"target": {"repository": "example/release-service"}, "path": {"path": "docs"}},
                    "holder": {"tool": "antigravity", "host": "example-stale", "session": "synth-session-0"},
                    "purpose": "Synthetic expired lease retained for inspection",
                    "fence": 1,
                    "initial_fence": 1,
                    "granted_ms": expired_at.saturating_sub(CLAIM_LEASE_MS),
                    "expires_ms": expired_at,
                    "state": {"expired": {"at_ms": expired_at}},
                    "attempt": null,
                    "claim_mirror": {"posted": {"message_id": "synth-claim-099"}},
                    "release_mirror": {"posted": {"message_id": "synth-rel-099"}}
                }]),
            );
            obj.insert("expired_truncated".into(), json!(false));
        }
    }
    payload
}

fn synthetic_dispatch_status_json(now: u64) -> Value {
    json!({
        "synthetic": true,
        "enabled": true,
        "state": "running",
        "paused": false,
        "paused_reason": null,
        "paused_at_ms": null,
        "paused_by": null,
        "draining_machines": [],
        "last_tick": {
            "ticked_at_ms": now.saturating_sub(5_000),
            "free_slots": 3,
            "planned_count": 1,
            "performed_count": 1,
            "deferred_count": 0,
            "deferred": [],
            "deferral_counts": {}
        }
    })
}

fn synthetic_work_json() -> Value {
    let work_ready = "a".repeat(64);
    let work_blocked = hex_id("synthetic-fleet-work-blocked-1");
    let source_ver = "b".repeat(64);
    let blocked_ver = hex_id("synthetic-fleet-work-blocked-src");
    json!({
        "synthetic": true,
        "ready_unclaimed": 1,
        "ranked": [{
            "identity": work_ready,
            "priority": 1,
            "repository": "example/release-service",
            "title": "Explain a failed test from its run (synthetic)",
            "permitted_paths": ["src/"],
            "interactive": true,
            "sources": [{
                "kind": "github_issue",
                "source": {
                    "contract_version": 1,
                    "provider_namespace": "github",
                    "native_kind": "issue",
                    "native_id": "example/release-service#42"
                },
                "native_id": "example/release-service#42",
                "version": source_ver
            }]
        }],
        "blocked": [{
            "identity": work_blocked,
            "repository": "example/release-service",
            "title": "Publish release notes after checks pass (synthetic)",
            "reasons": [{"state": "missing_dependency", "identity": work_ready}],
            "sources": [{
                "kind": "github_issue",
                "source": {
                    "contract_version": 1,
                    "provider_namespace": "github",
                    "native_kind": "issue",
                    "native_id": "example/release-service#43"
                },
                "native_id": "example/release-service#43",
                "version": blocked_ver
            }]
        }],
        "truncated": false
    })
}

fn synthetic_attempts_json(now: u64) -> Value {
    let attempt_active = "d".repeat(64);
    let attempt_stale = hex_id("synthetic-fleet-attempt-stale-2");
    let work_ready = "a".repeat(64);
    let work_stale = hex_id("synthetic-fleet-work-stale-2");
    let session_active = hex_id("synthetic-fleet-session-1");
    let session_stale = hex_id("synthetic-fleet-session-2");
    let source_ver = "b".repeat(64);
    json!({
        "synthetic": true,
        "attempts": [
            {
                "attempt": attempt_active,
                "fence": 1,
                "state": "executing",
                "lease_expires_ms": now.saturating_add(EXPIRING_CLAIM_REMAINING_MS),
                "held_by_fleet": true,
                "phase": "executing",
                "reservation": null,
                "queued_move_eligible": false,
                "work": work_ready,
                "repository": "example/release-service",
                "title": "Synthetic running implementation attempt",
                "observation_age_ms": 2_000,
                "observation_stale": false,
                "sources": [{
                    "kind": "github_issue",
                    "source": {"contract_version": 1, "provider_namespace": "github", "native_kind": "issue", "native_id": "example/release-service#42"},
                    "native_id": "example/release-service#42",
                    "version": source_ver
                }],
                "session": {
                    "identity": session_active,
                    "work": work_ready,
                    "attempt": attempt_active,
                    "fence": 1,
                    "machine": "example-desktop",
                    "agent_name": "antigravity",
                    "harness": "antigravity",
                    "state": "working",
                    "created_ms": now.saturating_sub(900_000),
                    "observed_ms": now.saturating_sub(2_000)
                }
            },
            {
                "attempt": attempt_stale,
                "fence": 1,
                "state": "claimed",
                "lease_expires_ms": now.saturating_add(1_200_000),
                "held_by_fleet": true,
                "phase": "executing",
                "reservation": null,
                "queued_move_eligible": false,
                "work": work_stale,
                "repository": "synthetic/disconnected-cairn",
                "title": "Synthetic stale-observation attempt",
                "observation_age_ms": STALE_OBSERVATION_MS,
                "observation_stale": true,
                "sources": [],
                "session": {
                    "identity": session_stale,
                    "work": work_stale,
                    "attempt": attempt_stale,
                    "fence": 1,
                    "machine": "example-stale",
                    "agent_name": "codex",
                    "harness": "codex",
                    "state": "blocked",
                    "created_ms": now.saturating_sub(1_200_000),
                    "observed_ms": now.saturating_sub(STALE_OBSERVATION_MS)
                }
            }
        ],
        "truncated": false
    })
}

fn synthetic_decisions_json(now: u64) -> Value {
    let decision_id = hex_id("synthetic-fleet-decision-open-1");
    let session_active = hex_id("synthetic-fleet-session-1");
    let attempt_active = "d".repeat(64);
    json!({
        "synthetic": true,
        "decisions": [{
            "identity": decision_id,
            "session": session_active,
            "attempt": attempt_active,
            "question": "[Synthetic] Confirm whether to keep the release-notes artifact minimal before rerun?",
            "raised_ms": now.saturating_sub(120_000),
            "answer": null,
            "closed_ms": null
        }]
    })
}

fn synthetic_guest_move_json() -> Value {
    json!({
        "synthetic": true,
        "schema": GUEST_MOVE_OVERVIEW_SCHEMA,
        "available": false,
        "reason": "Fail-closed synthetic example: running guest move prerequisites (Aspen #15 placement fencing, Lattice epoch reconnect, and two-host KVM qualification) are unavailable.",
        "prerequisites": [
            {
                "id": "fence-guest-placement-ownership",
                "owner": "Aspen (#15)",
                "satisfied": false,
                "detail": "Exclusive guest placement CAS and source resource fencing port unavailable in disposable example"
            },
            {
                "id": "reconnect-restored-guest-sessions",
                "owner": "Lattice",
                "satisfied": false,
                "detail": "Epoch-scoped restored guest session reconnect port unavailable in disposable example"
            },
            {
                "id": "two-host-kvm-qualification",
                "owner": "Operations",
                "satisfied": false,
                "detail": "Two-host compatible KVM qualification not retained"
            }
        ],
        "ordered_steps": ["pause", "checkpoint", "transfer", "restore", "reconnect", "retire_source"],
        "moves": []
    })
}

fn synthetic_swarm_run(now: u64) -> Value {
    let swarm_id = hex_id("synthetic-fleet-swarm-run-1");
    let claim_expiring = hex_id("synthetic-fleet-claim-expiring-1");
    let attempt_active = "d".repeat(64);
    let created = now.saturating_sub(1_800_000);
    json!({
        "identity": swarm_id,
        "config": {
            "title": "Synthetic release diagnostics swarm",
            "envelope": {
                "allowed_scopes": [
                    {"target": {"repository": "example/release-service"}, "path": {"path": "crates/release"}}
                ]
            },
            "max_concurrency": 3,
            "max_repair_rounds": 2,
            "budget_ceiling_units": 100,
            "stale_after_ms": 300_000
        },
        "budget": {
            "ceiling_units": 100,
            "drawn_units": 28
        },
        "phase": "active",
        "members": {
            "coordinator": {
                "name": "coordinator",
                "role": "coordinator",
                "state": {"state": "waiting", "since_seq": 3},
                "host": "example-desktop",
                "harness": "antigravity",
                "model": "synthetic-coordinator",
                "scope": null,
                "claim_identity": null,
                "claim_fence": null,
                "attempt_lineage": [],
                "budget_draw": 6,
                "last_read_seq": 3,
                "last_post_seq": 1,
                "spawned_ms": created,
                "observed_ms": now.saturating_sub(15_000),
                "failover_count": 0
            },
            "diagnostics-author": {
                "name": "diagnostics-author",
                "role": "author",
                "state": {"state": "working"},
                "host": "example-desktop",
                "harness": "antigravity",
                "model": "synthetic-worker",
                "scope": {"target": {"repository": "example/release-service"}, "path": {"path": "crates/release"}},
                "claim_identity": claim_expiring,
                "claim_fence": 1,
                "attempt_lineage": [attempt_active],
                "budget_draw": 16,
                "last_read_seq": 3,
                "last_post_seq": 2,
                "spawned_ms": created.saturating_add(5_000),
                "observed_ms": now.saturating_sub(2_000),
                "failover_count": 0
            },
            "verifier": {
                "name": "verifier",
                "role": "verifier",
                "state": {"state": "waiting", "since_seq": 3},
                "host": "example-desktop",
                "harness": "antigravity",
                "model": "synthetic-verifier",
                "scope": null,
                "claim_identity": null,
                "claim_fence": null,
                "attempt_lineage": [],
                "budget_draw": 6,
                "last_read_seq": 3,
                "last_post_seq": 3,
                "spawned_ms": created.saturating_add(10_000),
                "observed_ms": now.saturating_sub(10_000),
                "failover_count": 0
            }
        },
        "board": {
            "next_seq": 4,
            "posts": [
                {
                    "seq": 1,
                    "sender": "coordinator",
                    "sender_role": "coordinator",
                    "kind": "brief",
                    "mentions": [{"kind": "member", "target": "diagnostics-author"}],
                    "body": "Inspect synthetic release-version failure and prepare bounded fix inside crates/release.",
                    "posted_ms": created.saturating_add(5_000),
                    "in_reply_to": null,
                    "candidate_commit": null
                },
                {
                    "seq": 2,
                    "sender": "diagnostics-author",
                    "sender_role": "author",
                    "kind": "handoff",
                    "mentions": [{"kind": "member", "target": "verifier"}],
                    "body": "Prepared synthetic patch on candidate revision; ready for read-only verification.",
                    "posted_ms": now.saturating_sub(180_000),
                    "in_reply_to": 1,
                    "candidate_commit": null
                },
                {
                    "seq": 3,
                    "sender": "verifier",
                    "sender_role": "verifier",
                    "kind": "review",
                    "mentions": [{"kind": "member", "target": "diagnostics-author"}],
                    "body": "Verified synthetic failure explanation; requesting minimal release-notes confirmation.",
                    "posted_ms": now.saturating_sub(120_000),
                    "in_reply_to": 2,
                    "candidate_commit": null
                }
            ]
        },
        "verdicts": [],
        "repair_round": 0,
        "approved_commit": null,
        "escalation_reason": null,
        "created_ms": created,
        "updated_ms": now.saturating_sub(2_000)
    })
}

fn synthetic_swarm_graph(now: u64, up_to_seq: Option<u64>) -> Value {
    let head_seq = 3_u64;
    let cutoff = up_to_seq.unwrap_or(head_seq).min(head_seq);
    let mut edges = Vec::new();
    if cutoff >= 1 {
        edges.push(json!({
            "from": "coordinator",
            "to": "diagnostics-author",
            "weight": 1,
            "last_seq": 1,
            "kinds": ["brief"]
        }));
    }
    if cutoff >= 2 {
        edges.push(json!({
            "from": "diagnostics-author",
            "to": "verifier",
            "weight": 1,
            "last_seq": 2,
            "kinds": ["handoff"]
        }));
    }
    if cutoff >= 3 {
        edges.push(json!({
            "from": "verifier",
            "to": "diagnostics-author",
            "weight": 1,
            "last_seq": 3,
            "kinds": ["review"]
        }));
    }
    let _ = now;
    json!({
        "synthetic": true,
        "graph": {
            "up_to_seq": cutoff,
            "head_seq": head_seq,
            "nodes": [
                {"member": "coordinator", "role": "coordinator", "state": "waiting", "post_count": if cutoff >= 1 { 1 } else { 0 }, "mentions_in": 0, "mentions_out": if cutoff >= 1 { 1 } else { 0 }},
                {"member": "diagnostics-author", "role": "author", "state": "working", "post_count": if cutoff >= 2 { 1 } else { 0 }, "mentions_in": usize::from(cutoff >= 1) + usize::from(cutoff >= 3), "mentions_out": if cutoff >= 2 { 1 } else { 0 }},
                {"member": "verifier", "role": "verifier", "state": "waiting", "post_count": if cutoff >= 3 { 1 } else { 0 }, "mentions_in": usize::from(cutoff >= 2), "mentions_out": if cutoff >= 3 { 1 } else { 0 }}
            ],
            "edges": edges
        }
    })
}

fn synthetic_fleet_snapshot_json(now: u64) -> Value {
    json!({
        "synthetic": true,
        "version": synthetic_fleet_version(),
        "health": synthetic_fleet_health_json(),
        "claims": synthetic_claims_payload(now, ClaimsInclude { foreign: true, expired: true }),
        "work": synthetic_work_json(),
        "attempts": synthetic_attempts_json(now),
        "decisions": synthetic_decisions_json(now),
        "machines": synthetic_machines_json(now)
    })
}

fn synthetic_admission_json(now: u64) -> Value {
    let policy_id = hex_id("synthetic-admission-policy-v1");
    let root_account = hex_id("synthetic-admission-account-platform");
    let child_account = hex_id("synthetic-admission-account-cairn");
    let attempt_active = "d".repeat(64);
    let attempt_unrecorded = hex_id("synthetic-fleet-attempt-stale-2");
    let work_ready = "a".repeat(64);
    let work_stale = hex_id("synthetic-fleet-work-stale-2");
    let serving_scope = hex_id("synthetic-serving-scope-default");
    json!({
        "synthetic": true,
        "schema": ADMISSION_SCHEMA,
        "configured": true,
        "policy": {
            "identity": policy_id,
            "version": 1,
            "accounting_basis": "kiln_tokens_v1",
            "strategy": "drf",
            "strategy_weights": null,
            "bounds": {
                "max_items": 100,
                "max_bytes": 1_048_576,
                "max_active": 4,
                "max_children": 8
            },
            "shares": {
                "platform": 60,
                "release": 40
            },
            "group_limits": [],
            "serving": null
        },
        "accounts": [
            {
                "identity": root_account,
                "parent": null,
                "parent_attempt": null,
                "root": root_account,
                "depth": 0,
                "tenant": "platform",
                "campaign": "platform-root",
                "deadline_ms": now.saturating_add(86_400_000),
                "revoked": false,
                "allocated": {"tokens": 500_000, "attempts": 16},
                "transferred_out": {"tokens": 150_000, "attempts": 4},
                "available": {"tokens": 270_000, "attempts": 10},
                "held": {"tokens": 30_000, "attempts": 1},
                "tentative": {},
                "unknown": {},
                "consumed": {"tokens": 50_000, "attempts": 1},
                "settled_measured": {"tokens": 50_000},
                "settled_unknown": {},
                "children": 1
            },
            {
                "identity": child_account,
                "parent": root_account,
                "parent_attempt": null,
                "root": root_account,
                "depth": 1,
                "tenant": "platform",
                "campaign": campaign::CAMPAIGN_ID,
                "deadline_ms": now.saturating_add(43_200_000),
                "revoked": false,
                "allocated": {"tokens": 150_000, "attempts": 4},
                "transferred_out": {},
                "available": {"tokens": 108_000, "attempts": 2},
                "held": {"tokens": 22_000, "attempts": 1},
                "tentative": {},
                "unknown": {},
                "consumed": {"tokens": 20_000, "attempts": 1},
                "settled_measured": {"tokens": 18_500},
                "settled_unknown": {"tokens": 1_500},
                "children": 0
            }
        ],
        "serving": [
            {
                "provider": "synthetic-animus-pool",
                "scope": serving_scope,
                "bounds": {
                    "scope": "synthetic-pool",
                    "max_ongoing": 4,
                    "max_queued": 8
                },
                "readiness": "ready",
                "origin": "serving_owner",
                "remaining_slots": 3,
                "queued": 1,
                "observed_at_ms": now.saturating_sub(2_000),
                "valid_until_ms": now.saturating_add(SERVING_VALIDITY_WINDOW_MS),
                "source_generation": 7,
                "sequence": 12,
                "deployment_generation": 1
            }
        ],
        "queue_items": [],
        "attempt_metrics": [
            {
                "attempt_id": attempt_active,
                "fence": 1,
                "work_id": work_ready,
                "work_revision": 1,
                "phase": "executing",
                "open": true,
                "recorded": true,
                "telemetry_source": "metrics_receipt",
                "claimed_at_ms": now.saturating_sub(900_000),
                "observed_at_ms": now.saturating_sub(2_000),
                "ended_at_ms": null,
                "wall_time_ms": 898_000,
                "tokens": 18_500,
                "tokens_status": "observed",
                "tokens_absence_reason": null,
                "model_turns": 6,
                "model_turns_status": "observed",
                "model_turns_absence_reason": null,
                "auxiliary_calls": 14,
                "auxiliary_calls_status": "observed",
                "auxiliary_calls_absence_reason": null,
                "harness": "antigravity",
                "model": "synthetic-model",
                "worker_terminal": null,
                "verification_outcome": null
            },
            {
                "attempt_id": attempt_unrecorded,
                "fence": 1,
                "work_id": work_stale,
                "work_revision": 1,
                "phase": "executing",
                "open": true,
                "recorded": false,
                "telemetry_source": "not_recorded",
                "claimed_at_ms": now.saturating_sub(1_200_000),
                "observed_at_ms": null,
                "ended_at_ms": null,
                "wall_time_ms": null,
                "tokens": null,
                "tokens_status": "absent",
                "tokens_absence_reason": "not_reported",
                "model_turns": null,
                "model_turns_status": "absent",
                "model_turns_absence_reason": "not_reported",
                "auxiliary_calls": null,
                "auxiliary_calls_status": "absent",
                "auxiliary_calls_absence_reason": "not_instrumented",
                "harness": "codex",
                "model": null,
                "worker_terminal": null,
                "verification_outcome": null
            }
        ],
        "bounded": true,
        "truncated": false,
        "non_claims": [
            "Simulated fair-admission projection for website exploration only; confers no claim, reservation, or serving grant.",
            "Transfers reduce parent available quota and are never counted as consumption.",
            "Unrecorded AttemptMetrics telemetry stays null and is never zero-filled.",
            "Serving snapshots are wait evidence only and never replace owner request admission."
        ]
    })
}

fn synthetic_repositories_json(now: u64) -> Value {
    let release_id = hex_id("synthetic-repo-release-service");
    let service_id = hex_id("synthetic-repo-example-service");
    let cairn_id = blake3::hash(b"synthetic/disconnected-cairn")
        .to_hex()
        .to_string();
    let release_rev = hex_id("synthetic-rev-release-v3");
    let service_rev = hex_id("synthetic-rev-service-v3");
    let cairn_rev = hex_id("synthetic-rev-cairn-v1");
    let rad_rid = "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE";
    json!({
        "synthetic": true,
        "schema": REPOSITORIES_SCHEMA,
        "generated_at_ms": now,
        "repositories": [
            {
                "identity": service_id,
                "slug": "example-service",
                "label": "Example release service · synthetic",
                "canonical": {
                    "provider": "radicle",
                    "role": "canonical",
                    "url": rad_rid,
                    "native_id": rad_rid,
                    "normalized_key": format!("radicle:{rad_rid}"),
                    "default_ref": "main",
                    "observed_revision": service_rev,
                    "observed_at_ms": now.saturating_sub(30_000),
                    "status": "observed"
                },
                "remotes": [
                    {
                        "provider": "radicle",
                        "role": "canonical",
                        "url": rad_rid,
                        "native_id": rad_rid,
                        "normalized_key": format!("radicle:{rad_rid}"),
                        "default_ref": "main",
                        "observed_revision": service_rev,
                        "observed_at_ms": now.saturating_sub(30_000),
                        "status": "observed"
                    },
                    {
                        "provider": "github",
                        "role": "mirror",
                        "url": "https://github.com/example/service.git",
                        "native_id": "example/service",
                        "normalized_key": "github:example/service",
                        "default_ref": "main",
                        "observed_revision": service_rev,
                        "observed_at_ms": now.saturating_sub(30_000),
                        "status": "observed"
                    }
                ],
                "local_copies": [
                    {
                        "machine": "example-desktop",
                        "path": "/synthetic/workspace/example-service",
                        "vcs": "jj",
                        "head_commit": service_rev,
                        "branch_or_bookmark": "main",
                        "dirty": false,
                        "ahead": 0,
                        "behind": 0,
                        "remotes": [rad_rid, "https://github.com/example/service.git"],
                        "cairn_count": 2,
                        "observed_at_ms": now.saturating_sub(5_000),
                        "health": "available",
                        "error": null
                    }
                ],
                "sync_status": "synced",
                "admission": {
                    "catalog_imported": true,
                    "ci_configured": true,
                    "source_configured": true,
                    "fleet_enrolled": true
                },
                "total_cairns": 2,
                "non_claims": [
                    "Synthetic example repository record; no real checkout or Radicle node is contacted."
                ]
            },
            {
                "identity": release_id,
                "slug": "example/release-service",
                "label": "Release service · synthetic CI inputs",
                "canonical": {
                    "provider": "github",
                    "role": "canonical",
                    "url": "https://github.com/example/release-service.git",
                    "native_id": "example/release-service",
                    "normalized_key": "github:example/release-service",
                    "default_ref": "main",
                    "observed_revision": release_rev,
                    "observed_at_ms": now.saturating_sub(45_000),
                    "status": "observed"
                },
                "remotes": [
                    {
                        "provider": "github",
                        "role": "canonical",
                        "url": "https://github.com/example/release-service.git",
                        "native_id": "example/release-service",
                        "normalized_key": "github:example/release-service",
                        "default_ref": "main",
                        "observed_revision": release_rev,
                        "observed_at_ms": now.saturating_sub(45_000),
                        "status": "observed"
                    }
                ],
                "local_copies": [
                    {
                        "machine": "example-desktop",
                        "path": "/synthetic/workspace/release-service",
                        "vcs": "git",
                        "head_commit": release_rev,
                        "branch_or_bookmark": "main",
                        "dirty": true,
                        "ahead": 1,
                        "behind": 0,
                        "remotes": ["https://github.com/example/release-service.git"],
                        "cairn_count": 1,
                        "observed_at_ms": now.saturating_sub(5_000),
                        "health": "available",
                        "error": null
                    },
                    {
                        "machine": "example-worker",
                        "path": "/synthetic/worker/release-service",
                        "vcs": "git",
                        "head_commit": null,
                        "branch_or_bookmark": null,
                        "dirty": null,
                        "ahead": null,
                        "behind": null,
                        "remotes": [],
                        "cairn_count": null,
                        "observed_at_ms": now.saturating_sub(5_000),
                        "health": "unavailable",
                        "error": "Simulated unreachable host"
                    }
                ],
                "sync_status": "dirty",
                "admission": {
                    "catalog_imported": true,
                    "ci_configured": true,
                    "source_configured": true,
                    "fleet_enrolled": true
                },
                "total_cairns": 1,
                "non_claims": [
                    "Synthetic example repository record; no real checkout or remote is queried."
                ]
            },
            {
                "identity": cairn_id,
                "slug": "synthetic/disconnected-cairn",
                "label": "Disconnected Cairn workspace · synthetic",
                "canonical": null,
                "remotes": [],
                "local_copies": [
                    {
                        "machine": "example-desktop",
                        "path": "/synthetic/workspace/disconnected-cairn",
                        "vcs": "jj",
                        "head_commit": cairn_rev,
                        "branch_or_bookmark": "main",
                        "dirty": false,
                        "ahead": 0,
                        "behind": 0,
                        "remotes": [],
                        "cairn_count": 4,
                        "observed_at_ms": now.saturating_sub(10_000),
                        "health": "available",
                        "error": null
                    }
                ],
                "sync_status": "local_only",
                "admission": {
                    "catalog_imported": false,
                    "ci_configured": false,
                    "source_configured": false,
                    "fleet_enrolled": true
                },
                "total_cairns": 4,
                "non_claims": [
                    "Synthetic local-only Cairn checkout; no external remote is declared."
                ]
            }
        ],
        "hosts": [
            {
                "machine": "example-desktop",
                "health": "available",
                "copy_count": 3,
                "dirty_count": 1,
                "cairn_count": 7,
                "error": null,
                "observed_at_ms": now.saturating_sub(5_000)
            },
            {
                "machine": "example-worker",
                "health": "unavailable",
                "copy_count": 1,
                "dirty_count": 0,
                "cairn_count": 0,
                "error": "Simulated unreachable host",
                "observed_at_ms": now.saturating_sub(5_000)
            }
        ],
        "summary": {
            "total": 3,
            "synced": 1,
            "diverged": 0,
            "dirty": 1,
            "unknown": 0,
            "local_only": 1,
            "total_local_copies": 4,
            "total_cairns": 7
        },
        "sources": {
            "catalog": {"configured": true, "available": true, "count": 2, "error": null},
            "local_scan": {"configured": true, "available": true, "count": 3, "roots": ["/synthetic/workspace"], "error": null},
            "fleet": {"configured": true, "available": true, "count": 4, "error": null}
        },
        "bounded": true,
        "truncated": false,
        "non_claims": [
            "Synthetic example catalog; registration does not grant CI workflow execution or Fleet agent dispatch.",
            "Observed remotes and local copies are simulated snapshots and do not probe real upstreams.",
            "Radicle RIDs are recorded as read-only synthetic declarations; no Radicle command is run."
        ]
    })
}

fn synthetic_repositories_digest(now: u64) -> String {
    let bytes = serde_json::to_vec(&synthetic_repositories_json(now)).unwrap_or_default();
    blake3::hash(&bytes).to_hex().to_string()
}

fn rewrite_project_repository_labels(value: &mut Value) {
    let rewrite_row = |row: &mut Value| {
        if let Some(obj) = row.as_object_mut()
            && obj.contains_key("repository")
        {
            obj.insert(
                "repository".into(),
                json!(SYNTHETIC_PROJECT_REPOSITORY_LABEL),
            );
        }
    };
    if let Some(available) = value.get_mut("available").and_then(Value::as_array_mut) {
        for row in available {
            rewrite_row(row);
        }
    }
    if let Some(enrolled) = value.get_mut("enrolled").and_then(Value::as_array_mut) {
        for row in enrolled {
            rewrite_row(row);
        }
    }
    rewrite_row(value);
}

fn campaign_index_from_store(campaign_db: &Path) -> Result<Value> {
    let store = kiln_implementer::campaign::CampaignStore::open(campaign_db)?;
    let states = store.list()?;
    let mut summaries = Vec::with_capacity(states.len());
    for state in states {
        let id = state.contract.campaign_id.clone();
        let store_version = store.version(&id)?.unwrap_or(1);
        let complete = state.complete();
        let pending_decisions = state
            .decisions
            .values()
            .filter(|d| d.state == kiln_core::implementation::campaign::DecisionRequestState::Outstanding)
            .count();
        let unissued_decisions = state
            .decisions
            .values()
            .filter(|d| d.state == kiln_core::implementation::campaign::DecisionRequestState::Unissued)
            .count();
        let pending_effects = state
            .pending_effects
            .values()
            .filter(|e| !e.progress_applied)
            .count();
        let status = if state.cancelled {
            "cancelled"
        } else if complete {
            "complete"
        } else if state.paused {
            "paused"
        } else {
            match state.evaluate().decision.operation {
                kiln_core::implementation::campaign::CampaignOperation::Blocked => "blocked",
                kiln_core::implementation::campaign::CampaignOperation::Wait => "waiting",
                _ => "active",
            }
        };
        let updated_at = state.decisions.values().map(|d| d.created_ms).max();
        summaries.push(json!({
            "campaign_id": id,
            "status": status,
            "store_version": store_version,
            "revision": state.contract.revision,
            "repository": state.contract.repository,
            "repository_identity": kiln_web_service::inventory::campaign_projection::repository_identity(&state.contract.repository),
            "member_count": state.contract.members.len(),
            "pending_decisions": pending_decisions,
            "unissued_decisions": unissued_decisions,
            "pending_effects": pending_effects,
            "paused": state.paused,
            "cancelled": state.cancelled,
            "complete": complete,
            "updated_at": updated_at,
        }));
    }
    Ok(json!({
        "synthetic": true,
        "schema": CAMPAIGN_INDEX_SCHEMA,
        "campaigns": summaries,
        "bounded": true,
        "truncated": false
    }))
}

fn synthetic_attention_json(
    now: u64,
    ci_engine: &mut kiln_web_service::engine::Engine,
    ci_runs: &BTreeMap<String, String>,
    campaign_db: &Path,
    principal: &Principal,
) -> Result<Value> {
    let mut items: Vec<Value> = Vec::new();
    let mut counts = BTreeMap::from([
        ("UntrustedRunApproval", 0_usize),
        ("FleetSessionDecision", 0_usize),
        ("CampaignDecision", 0_usize),
        ("PlanReviewGate", 0_usize),
        ("UnknownTerminationOrDispatch", 0_usize),
        ("ExpiringOrConflictClaim", 0_usize),
    ]);

    if let Some(awaiting_id) = ci_runs.get("awaiting-approval")
        && let Ok(resp) = ci_engine.api_as("GET", &format!("/api/runs/{awaiting_id}"), &[], principal)
        && (200..300).contains(&resp.status)
        && let Ok(run) = serde_json::from_slice::<Value>(&resp.body)
    {
        let commit = run["source_revision"].as_str().unwrap_or_default().to_owned();
        let pr_label = run
            .pointer("/untrusted/pull_request")
            .and_then(Value::as_u64)
            .map_or_else(String::new, |n| format!(" (PR #{n})"));
        *counts.entry("UntrustedRunApproval").or_insert(0) += 1;
        items.push(json!({
            "id": format!("untrusted_run:{awaiting_id}"),
            "kind": "UntrustedRunApproval",
            "severity": "high",
            "title": format!("Untrusted run awaiting approval{pr_label}: {awaiting_id}"),
            "target": {
                "run_id": awaiting_id,
                "project_id": run["project_id"],
                "campaign_id": null,
                "cairn_change": null,
                "work_id": null,
                "attempt_id": null,
                "fence": null,
                "claim_id": null,
                "decision_id": null,
                "plan_id": null,
                "machine": null,
                "scope": null,
                "source_commit": commit
            },
            "context_preview": {
                "summary": "Synthetic untrusted pull-request run requires explicit operator approval before execution.",
                "project_id": run["project_id"],
                "source_revision": commit
            },
            "age_ms": Some(60_000),
            "ttl_remaining_ms": null,
            "allowed_actions": ["approve", "inspect"]
        }));
    }

    if let Ok(store) = kiln_implementer::campaign::CampaignStore::open(campaign_db)
        && let Ok(states) = store.list()
    {
        for state in states {
            let store_version = store.version(&state.contract.campaign_id).ok().flatten().unwrap_or(1);
            for decision in state.decisions.values() {
                if !matches!(
                    decision.state,
                    kiln_core::implementation::campaign::DecisionRequestState::Outstanding
                        | kiln_core::implementation::campaign::DecisionRequestState::Unissued
                ) {
                    continue;
                }
                let is_outstanding = decision.state
                    == kiln_core::implementation::campaign::DecisionRequestState::Outstanding;
                *counts.entry("CampaignDecision").or_insert(0) += 1;
                items.push(json!({
                    "id": format!("campaign_decision:{}:{}", state.contract.campaign_id, decision.identity.as_str()),
                    "kind": "CampaignDecision",
                    "severity": if is_outstanding { "high" } else { "medium" },
                    "title": format!("Campaign {} decision: {}", state.contract.campaign_id, decision.member),
                    "target": {
                        "run_id": null,
                        "project_id": null,
                        "campaign_id": state.contract.campaign_id,
                        "cairn_change": decision.member.split('/').next(),
                        "work_id": decision.work_identity.as_ref().map(|id| id.as_str().to_owned()),
                        "attempt_id": null,
                        "fence": null,
                        "claim_id": null,
                        "decision_id": decision.identity.as_str(),
                        "plan_id": null,
                        "machine": null,
                        "scope": null,
                        "source_commit": null
                    },
                    "context_preview": {
                        "summary": decision.question,
                        "question": decision.question,
                        "options": decision.options,
                        "recommendation": decision.recommendation,
                        "member": decision.member,
                        "category": decision.kind,
                        "state": decision.state,
                        "campaign_version": store_version
                    },
                    "age_ms": Some(now.saturating_sub(decision.created_ms)),
                    "ttl_remaining_ms": null,
                    "allowed_actions": if is_outstanding { vec!["resolve_decision", "wake_campaign", "inspect"] } else { vec!["issue_decision", "inspect"] }
                }));
            }
        }
    }

    let decision_id = hex_id("synthetic-fleet-decision-open-1");
    let attempt_active = "d".repeat(64);
    let attempt_short = attempt_active.get(..12).unwrap_or(attempt_active.as_str());
    let session_active = hex_id("synthetic-fleet-session-1");
    *counts.entry("FleetSessionDecision").or_insert(0) += 1;
    items.push(json!({
        "id": format!("fleet_decision:{decision_id}"),
        "kind": "FleetSessionDecision",
        "severity": "high",
        "title": format!("Agent question on attempt {attempt_short}"),
        "target": {
            "run_id": null,
            "project_id": null,
            "campaign_id": null,
            "cairn_change": null,
            "work_id": null,
            "attempt_id": attempt_active,
            "fence": 1,
            "claim_id": null,
            "decision_id": decision_id,
            "plan_id": null,
            "machine": "example-desktop",
            "scope": null,
            "source_commit": null
        },
        "context_preview": {
            "summary": "[Synthetic] Confirm whether to keep the release-notes artifact minimal before rerun?",
            "question": "[Synthetic] Confirm whether to keep the release-notes artifact minimal before rerun?",
            "session": session_active,
            "attempt": attempt_active,
            "raised_ms": now.saturating_sub(120_000),
            "stale_observation": false,
            "session_state": "working"
        },
        "age_ms": Some(120_000),
        "ttl_remaining_ms": null,
        "allowed_actions": ["answer_decision", "inspect_pane", "inspect"]
    }));

    let claim_expiring = hex_id("synthetic-fleet-claim-expiring-1");
    *counts.entry("ExpiringOrConflictClaim").or_insert(0) += 1;
    items.push(json!({
        "id": format!("expiring_claim:{claim_expiring}"),
        "kind": "ExpiringOrConflictClaim",
        "severity": "medium",
        "title": "Claim expiring soon: example/release-service crates/release",
        "target": {
            "run_id": null,
            "project_id": null,
            "campaign_id": null,
            "cairn_change": null,
            "work_id": null,
            "attempt_id": attempt_active,
            "fence": 1,
            "claim_id": claim_expiring,
            "decision_id": null,
            "plan_id": null,
            "machine": "example-desktop",
            "scope": "example/release-service crates/release",
            "source_commit": null
        },
        "context_preview": {
            "summary": "Simulated claim held by antigravity on example-desktop for example/release-service crates/release expires in 600s.",
            "subkind": "expiring_claim",
            "holder": "antigravity on example-desktop, session synth-session-1",
            "scope_text": "example/release-service crates/release",
            "purpose": "Synthetic coordination lease expiring soon (disposable example)",
            "expires_ms": now.saturating_add(EXPIRING_CLAIM_REMAINING_MS)
        },
        "age_ms": Some(CLAIM_LEASE_MS.saturating_sub(EXPIRING_CLAIM_REMAINING_MS)),
        "ttl_remaining_ms": Some(EXPIRING_CLAIM_REMAINING_MS),
        "allowed_actions": ["renew_claim", "release_claim", "inspect"]
    }));

    let total = items.len();
    Ok(json!({
        "synthetic": true,
        "schema": ATTENTION_SCHEMA,
        "generated_at_ms": now,
        "items": items,
        "total": total,
        "bounded": true,
        "truncated": false,
        "counts": counts,
        "sources": {
            "ci_engine": {"configured": true, "available": true, "error": null},
            "implementation_work": {"configured": true, "available": true, "error": null},
            "fleet": {"configured": true, "available": true, "error": null}
        }
    }))
}

fn ci_live_snapshot_payload(
    ci: &mut ci::Seed,
    principal: &Principal,
) -> Result<(u64, String)> {
    ci.poll()?;
    let resp = ci.engine.api_as("GET", "/api/live?after=0", &[], principal)?;
    if !(200..300).contains(&resp.status) {
        return Err("CI live snapshot unavailable".into());
    }
    let text = String::from_utf8(resp.body)?;
    let data = text
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .ok_or("CI live snapshot lacked data frame")?
        .to_owned();
    let parsed: Value = serde_json::from_str(&data)?;
    let version = parsed["version"].as_u64().unwrap_or(0);
    Ok((version, data))
}

fn build_events_initial_frames(
    workspace: &mut Workspace,
    channels: [bool; 4],
    principal: &Principal,
) -> Result<Vec<(&'static str, String)>> {
    let now = now_ms()?;
    let (ci_version, ci_data) = ci_live_snapshot_payload(&mut workspace.ci, principal)?;
    let fleet_version = synthetic_fleet_version();
    let channel_names: Vec<&'static str> = [
        channels[CHANNEL_CI].then_some("ci"),
        channels[CHANNEL_FLEET].then_some("fleet"),
        channels[CHANNEL_CAIRN].then_some("cairn"),
        channels[CHANNEL_ATTENTION].then_some("attention"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let ready = json!({
        "synthetic": true,
        "schema": EVENT_HUB_SCHEMA,
        "channels": channel_names,
        "fleet_configured": true,
        "cairn_configured": true,
        "ci_version": ci_version,
        "fleet_version": fleet_version,
    })
    .to_string();
    let mut frames = vec![("ready", ready)];
    if channels[CHANNEL_CI] {
        frames.push(("ci", ci_data));
    }
    if channels[CHANNEL_FLEET] {
        frames.push(("fleet", synthetic_fleet_snapshot_json(now).to_string()));
    }
    if channels[CHANNEL_CI] {
        let repos_event = json!({
            "synthetic": true,
            "invalidated": false,
            "version": synthetic_repositories_digest(now),
        })
        .to_string();
        frames.push(("repositories", repos_event));
    }
    Ok(frames)
}

fn format_sse_frames(frames: &[(&'static str, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (event, data) in frames {
        out.extend_from_slice(format!("event: {event}\ndata: {data}\n\n").as_bytes());
    }
    out
}

fn is_tcp_peer_disconnected(stream: &TcpStream) -> bool {
    if stream.set_nonblocking(true).is_err() {
        return false;
    }
    let mut peek_buf = [0u8; 1];
    let disconnected = match stream.peek(&mut peek_buf) {
        Ok(0) => true,
        Ok(_) => false,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) =>
        {
            false
        }
        Err(_) => true,
    };
    let _ = stream.set_nonblocking(false);
    disconnected
}

fn stream_sse(
    mut stream: TcpStream,
    initial_bytes: &[u8],
    keepalive_comment: &[u8],
    keepalive_interval: Duration,
    stop: &AtomicBool,
) -> std::io::Result<()> {
    let _ = stream.set_nonblocking(false);
    stream.set_write_timeout(Some(REQUEST_DEADLINE))?;
    write!(
        stream,
        "HTTP/1.1 200 Example\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\n\r\n"
    )?;
    stream.write_all(initial_bytes)?;
    stream.flush()?;

    let mut last_keepalive = std::time::Instant::now();
    while !stop.load(Ordering::Acquire) {
        if is_tcp_peer_disconnected(&stream) {
            break;
        }
        if last_keepalive.elapsed() >= keepalive_interval {
            if stream.set_write_timeout(Some(REQUEST_DEADLINE)).is_err()
                || stream.write_all(keepalive_comment).is_err()
                || stream.flush().is_err()
            {
                break;
            }
            last_keepalive = std::time::Instant::now();
        }
        std::thread::sleep(SSE_DISCONNECT_POLL_INTERVAL);
    }
    Ok(())
}

struct Workspace {
    ci: ci::Seed,
    work: work::Seed,
    review: review::Seed,
    feeds: feeds::Feeds,
    campaign: PathBuf,
    campaign_controls: kiln_web_service::campaign_controls::Controls,
    examples: Vec<Example>,
    workspace_id: String,
}
impl Workspace {
    // r[impl kiln.web.exploration] r[impl kiln.web.exploration.scenario.local]
    fn seed(root: &Path, port: u16) -> Result<Self> {
        let mut ci = ci::seed(root, port)?;
        let work = work::seed(root, &ci.engine.work_ci_facts(), &ci.runs)?;
        let review = review::seed(root, &mut ci.engine)?;
        let feeds = feeds::Feeds::seed(&format!("http://127.0.0.1:{port}"))?;
        let campaign = campaign::seed(root)?;
        let campaign_controls = campaign::controls(root, &campaign)?;
        let mut examples = std::mem::take(&mut ci.examples);
        let mut work = work;
        let mut review = review;
        examples.append(&mut work.examples);
        examples.append(&mut review.examples);
        examples.extend(feeds.examples());
        examples.extend([
            Example::new("Guide, keyboard reference and command palette", "/?page=guide", "Read the numbered register, state legend and shortcuts. Press Ctrl K to jump to any destination."),
            Example::new("Evidence entry point", "/?page=evidence", "Choose a retained synthetic run, then inspect its exact source, execution and evidence. Evidence is not acceptance authority."),
            Example::new("Appearance, density and session", "/?page=settings", "Switch light/dark/system and comfortable/compact density. Clear session removes private views; reload reconnects the synthetic local identity."),
            Example::new("Retained activity and audit decisions", "/?page=activity", "Inspect events from the seeded local commands, filter them and follow an exact run."),
            Example::new("Capacity and retention", "/?page=operations", "Inspect occupied slots, queued requests, retention limits and local state. No pruning or real worker operation occurs."),
            Example::new("Agents, worker instances, compute and serving", "/?page=workers", "Inspect four distinct synthetic observation families and exact-work readiness. Use the fictional all-a 64-hex work identity to explore a disconnected, non-authorizing explanation; no provider is contacted."),
            Example::new("Selected Cairn campaign: ready work beside planning, review and lifecycle waits", "/?page=workers&worker_campaign=synthetic-selected-cairn", "Inspect every selected member and the selection hold that keeps it waiting while an independent ready member is the proposed next action, plus partial enumeration, stale review/check evidence, distinct completion goals and unknown aggregate budget. This synthetic campaign uses the real canonical store and read-only projection, not live Cairn or worker evidence."),
            Example::new("Campaign decision inbox and admission controls", "/?page=workers&worker_campaign=synthetic-selected-cairn", "Answer the synthetic required review or lifecycle preference, issue the waiting experiment question once the outstanding bound frees, then pause, continue or cancel admission. Each command goes through the real canonical owner with a request ID; replays, stale versions and refusals stay visible. No worker or provider is contacted."),
            Example::new("Start a configured campaign from a frozen preview", "/?page=workers", "Open \"Start a configured campaign\" to compare a startable synthetic definition with one whose verified pull request goal its native no-PR route does not offer, then start the first and inspect its new campaign."),
        ]);
        examples.extend(campaign::examples());
        let mut random = [0u8; 16];
        getrandom::fill(&mut random)
            .map_err(|e| format!("random workspace id unavailable: {e}"))?;
        let workspace_id = blake3::hash(&random).to_hex().to_string();
        Ok(Self {
            ci,
            work,
            review,
            feeds,
            campaign,
            campaign_controls,
            examples,
            workspace_id,
        })
    }
    fn route(&mut self, req: &Request) -> Result<ApiResponse> {
        let path = req.target.split('?').next().unwrap_or_default();
        let principal = Principal {
            id: ACTOR.into(),
            label: "Synthetic example operator".into(),
            role: Role::Operator,
            agent: None,
        };
        if path == "/examples.json" && req.method == "GET" {
            return Ok(json(
                200,
                // r[impl kiln.web.exploration.cases.synthetic] Every catalog response is marked synthetic.
                json!({"synthetic":true,"workspace":self.workspace_id,"examples":self.examples}),
            ));
        }
        if let Some(rest) = path.strip_prefix("/examples/cases/") {
            if req.method != "GET" {
                return Ok(error(405, "This resource is read-only"));
            }
            let case_id = rest.trim_matches('/');
            let query = req.target.split('?').nth(1).unwrap_or_default();
            let mut recipe_param = None;
            let mut workspace_param = None;
            for pair in query.split('&') {
                let mut parts = pair.splitn(2, '=');
                let key = parts.next().unwrap_or_default();
                let value = parts.next().unwrap_or_default();
                match key {
                    "recipe" => recipe_param = Some(value.to_string()),
                    "workspace" => workspace_param = Some(value.to_string()),
                    _ => {}
                }
            }
            let recipe = recipe_param.unwrap_or_default();
            let workspace = workspace_param.unwrap_or_default();
            match resolve_case(
                &self.examples,
                &self.workspace_id,
                case_id,
                &recipe,
                &workspace,
            ) {
                Ok(value) => return Ok(json(200, value)),
                Err((status, message)) => return Ok(error(status, message)),
            }
        }
        if path == "/api/events" {
            if req.method != "GET" {
                return Ok(error(405, "Events stream is read-only"));
            }
            let Ok(channels) = parse_events_channels(&req.target) else {
                return Ok(error(400, "Invalid events channel query"));
            };
            let frames = build_events_initial_frames(self, channels, &principal)?;
            return Ok(ApiResponse {
                status: 200,
                content_type: "text/event-stream",
                body: format_sse_frames(&frames),
                download_name: None,
                headers: Vec::new(),
            });
        }
        if path == "/api/fleet/machines" {
            if req.method != "GET" {
                return Ok(error(405, "Fleet machines endpoint is read-only"));
            }
            if req.target != "/api/fleet/machines" {
                return Ok(error(400, "Invalid fleet machines query"));
            }
            return Ok(json(200, synthetic_machines_json(now_ms()?)));
        }
        if path == "/api/fleet/health" {
            if req.method != "GET" {
                return Ok(error(405, "Fleet health endpoint is read-only"));
            }
            return Ok(json(200, synthetic_fleet_health_json()));
        }
        if path == "/api/fleet/events" {
            if req.method != "GET" {
                return Ok(error(405, "Fleet events stream is read-only"));
            }
            let Ok(after) = parse_fleet_event_after(&req.target) else {
                return Ok(error(400, "Invalid fleet events cursor"));
            };
            let version = synthetic_fleet_version();
            if after.as_deref() == Some(version.as_str()) {
                return Ok(bytes("text/event-stream", b": heartbeat\n\n"));
            }
            let snapshot = synthetic_fleet_snapshot_json(now_ms()?);
            return Ok(bytes(
                "text/event-stream",
                format!("event: snapshot\ndata: {snapshot}\n\n").as_bytes(),
            ));
        }
        if path == "/api/fleet/claims" {
            if req.method != "GET" {
                return Ok(error(405, "Simulated fleet claims are read-only"));
            }
            let Ok(include) = parse_claims_include(&req.target) else {
                return Ok(error(400, "Invalid fleet claims include query"));
            };
            return Ok(json(200, synthetic_claims_payload(now_ms()?, include)));
        }
        if path == "/api/fleet/dispatch/status" {
            if req.method != "GET" {
                return Ok(error(405, "Fleet dispatch status is read-only"));
            }
            if req.target != "/api/fleet/dispatch/status" {
                return Ok(error(400, "Invalid fleet dispatch status query"));
            }
            return Ok(json(200, synthetic_dispatch_status_json(now_ms()?)));
        }
        if path == "/api/fleet/work" {
            if req.method != "GET" {
                return Ok(error(405, "Fleet work is read-only"));
            }
            if req.target != "/api/fleet/work" {
                return Ok(error(400, "Invalid fleet work query"));
            }
            return Ok(json(200, synthetic_work_json()));
        }
        if path == "/api/fleet/attempts" {
            if req.method != "GET" {
                return Ok(error(405, "Fleet attempts are read-only"));
            }
            if req.target != "/api/fleet/attempts" {
                return Ok(error(400, "Invalid fleet attempts query"));
            }
            return Ok(json(200, synthetic_attempts_json(now_ms()?)));
        }
        if let Some(attempt_id) = path
            .strip_prefix("/api/fleet/attempts/")
            .and_then(|rest| rest.strip_suffix("/pane"))
        {
            if req.method != "GET" {
                return Ok(error(405, "Fleet attempt pane is read-only"));
            }
            let attempt_active = "d".repeat(64);
            let attempt_stale = hex_id("synthetic-fleet-attempt-stale-2");
            if attempt_id != attempt_active && attempt_id != attempt_stale {
                return Ok(error(404, "Simulated fleet attempt not found"));
            }
            let text = "[synthetic example] No live agent process is attached.\nSimulated worker session output for website exploration.\n";
            return Ok(json(
                200,
                json!({
                    "synthetic": true,
                    "attempt": attempt_id,
                    "pane_id": "synth:p1",
                    "machine": if attempt_id == attempt_active { "example-desktop" } else { "example-stale" },
                    "session_state": if attempt_id == attempt_active { "working" } else { "blocked" },
                    "line_count": 2,
                    "truncated": false,
                    "text": text
                }),
            ));
        }
        if path == "/api/fleet/decisions" {
            if req.method != "GET" {
                return Ok(error(405, "Fleet decisions are read-only"));
            }
            return Ok(json(200, synthetic_decisions_json(now_ms()?)));
        }
        if path == "/api/fleet/placement/guest-move" {
            if req.method != "GET" {
                return Ok(error(405, "Guest move overview is read-only"));
            }
            if req.target != "/api/fleet/placement/guest-move" {
                return Ok(error(400, "Invalid guest move query"));
            }
            return Ok(json(200, synthetic_guest_move_json()));
        }
        if path == "/api/fleet/swarms" {
            if req.method != "GET" {
                return Ok(error(405, "Fleet swarms are read-only"));
            }
            if req.target != "/api/fleet/swarms" {
                return Ok(error(400, "Invalid fleet swarms query"));
            }
            let swarm = synthetic_swarm_run(now_ms()?);
            return Ok(json(200, json!({"synthetic": true, "swarms": [swarm]})));
        }
        if let Some(rest) = path.strip_prefix("/api/fleet/swarms/") {
            if req.method != "GET" {
                return Ok(error(405, "Fleet swarm inspection is read-only"));
            }
            let swarm_id = hex_id("synthetic-fleet-swarm-run-1");
            if let Some(id) = rest.strip_suffix("/graph") {
                if id != swarm_id {
                    return Ok(error(404, "Unknown synthetic swarm run"));
                }
                let up_to_seq = match req.target.split_once('?') {
                    None => None,
                    Some((_, query)) => {
                        let Some(raw) = query.strip_prefix("up_to_seq=") else {
                            return Ok(error(400, "Invalid swarm graph query"));
                        };
                        let Ok(seq) = raw.parse::<u64>() else {
                            return Ok(error(400, "Invalid swarm graph sequence"));
                        };
                        Some(seq)
                    }
                };
                return Ok(json(200, synthetic_swarm_graph(now_ms()?, up_to_seq)));
            }
            if rest == swarm_id {
                return Ok(json(
                    200,
                    json!({"synthetic": true, "swarm": synthetic_swarm_run(now_ms()?)}),
                ));
            }
            return Ok(error(404, "Unknown synthetic swarm run"));
        }
        // r[impl kiln.inventory.consumers]
        if path == "/api/inventory" {
            return inventory::response(&req.method, &req.target);
        }
        if path == "/api/implementation/attention" {
            if req.method != "GET" {
                return Ok(error(405, "Attention projection is read-only"));
            }
            if req.target != "/api/implementation/attention" {
                return Ok(error(400, "Invalid attention query"));
            }
            return Ok(json(
                200,
                synthetic_attention_json(
                    now_ms()?,
                    &mut self.ci.engine,
                    &self.ci.runs,
                    &self.campaign,
                    &principal,
                )?,
            ));
        }
        if path == "/api/implementation/admission" {
            if req.method != "GET" {
                return Ok(error(405, "Admission projection is read-only"));
            }
            if req.target != "/api/implementation/admission" {
                return Ok(error(400, "Invalid admission query"));
            }
            return Ok(json(200, synthetic_admission_json(now_ms()?)));
        }
        if path == "/api/implementation/campaigns" {
            if req.method != "GET" {
                return Ok(error(405, "Campaign projections are read-only"));
            }
            if req.target == "/api/implementation/campaigns" {
                return Ok(json(200, campaign_index_from_store(&self.campaign)?));
            }
            return campaign::response(&self.campaign, &req.method, &req.target);
        }
        if path == "/api/repositories" {
            if req.method != "GET" {
                return Ok(error(405, "Repository catalog list requires GET"));
            }
            if req.target != "/api/repositories" {
                return Ok(error(400, "Repository list takes no query"));
            }
            return Ok(json(200, synthetic_repositories_json(now_ms()?)));
        }
        if path == "/api/repositories/refresh" {
            if req.method != "POST" {
                return Ok(error(405, "Repository refresh requires POST"));
            }
            return Ok(json(200, synthetic_repositories_json(now_ms()?)));
        }
        if kiln_web_service::campaign_controls::ROUTES.contains(&path) {
            return Ok(self
                .campaign_controls
                .response(&kiln_web_service::campaign_controls::ControlRequest {
                    method: &req.method,
                    target: &req.target,
                    body: &req.body,
                    principal: ACTOR,
                    role: Role::Operator,
                    now_ms: now_ms()?,
                })
                .response);
        }
        // r[impl kiln.cairn.workspace.scenario.example] The real Cairn workspace shell over the synthetic stores.
        // r[impl kiln.cairn.workspace.agents.scenario.source] Its agents section reads the synthetic inventory.
        if kiln_web_service::cairn_workspace::http::ROUTES.contains(&path) {
            return Ok(kiln_web_service::cairn_workspace::http::response(
                Some(&self.campaign),
                &req.method,
                &req.target,
                Role::Operator,
                &inventory::cairn_observation,
            ));
        }
        if path.starts_with("/api/feeds") || path.starts_with("/feeds") {
            return self
                .feeds
                .response(&req.method, &req.target, &req.body, &mut self.ci.engine);
        }
        if path == "/api/work" || path.starts_with("/api/work/") {
            return Ok(self.work.workspace.response_with_ci(
                &req.method,
                &req.target,
                &req.body,
                Role::Operator,
                ACTOR,
                i64::try_from(now_ms()?)?,
                &self.ci.engine.work_ci_facts(),
            ));
        }
        if path == "/api/code-review" || path.starts_with("/api/code-review/") {
            let ci = review::ci_facts(&mut self.ci.engine)?;
            return Ok(self.review.workspace.response(
                &req.method,
                &req.target,
                &req.body,
                kiln_web_service::code_review::RequestContext {
                    role: Role::Operator,
                    principal: ACTOR,
                    now_ms: i64::try_from(now_ms()?)?,
                    ci: &ci,
                },
            ));
        }
        if path == "/api/sources" || path.starts_with("/api/sources/") {
            return Ok(self.review.sources.response(
                &req.method,
                &req.target,
                &req.body,
                Role::Operator,
            ));
        }
        if path == "/api/reviews" || path.starts_with("/api/reviews/") {
            return Ok(self.review.receipts.response(
                &req.method,
                &req.target,
                &req.body,
                Role::Operator,
            ));
        }
        if path == "/api/cobs" || path.starts_with("/api/cobs/") {
            return Ok(self.review.cobs.response(
                &req.method,
                &req.target,
                &req.body,
                Role::Operator,
            ));
        }
        if path.starts_with("/api/") {
            // Poll only the in-process simulator, never an execution transport.
            self.ci.poll()?;
            let mut response = self
                .ci
                .engine
                .api_as(&req.method, &req.target, &req.body, &principal)?;
            if path == "/api/session"
                && req.method == "GET"
                && req.body.is_empty()
                && (200..300).contains(&response.status)
            {
                if let Ok(mut parsed) = serde_json::from_slice::<Value>(&response.body) {
                    if let Some(object) = parsed.as_object_mut() {
                        object.insert("feeds_configured".to_owned(), Value::Bool(false));
                        object.insert(
                            "capabilities".to_owned(),
                            json!({
                                "feeds": false,
                                "runbooks": false,
                                "inventory": false,
                                "campaign_controls": false,
                                "automation": false,
                            }),
                        );
                        response.body = parsed.to_string().into_bytes();
                    }
                }
            }
            if path == "/api/projects" && (200..300).contains(&response.status) {
                if let Ok(mut parsed) = serde_json::from_slice::<Value>(&response.body) {
                    rewrite_project_repository_labels(&mut parsed);
                    response.body = parsed.to_string().into_bytes();
                }
            }
            return Ok(response);
        }
        if req.method != "GET" {
            return Ok(error(405, "This resource is read-only"));
        }
        Ok(asset(path))
    }
}

fn asset(path: &str) -> ApiResponse {
    const JS: &str = "text/javascript; charset=utf-8";
    const CSS: &str = "text/css; charset=utf-8";
    match path {
        "/" => {
            let html = include_str!("../src/web.html")
                .replace("</head>", "<link rel=\"stylesheet\" href=\"/example.css\"></head>")
                .replace("<span class=\"topbar-workspace\">Kiln</span>", "<a class=\"topbar-workspace example-link\" href=\"/examples\">Synthetic examples</a>")
                .replace("<main id=\"main\" class=\"app-main\" tabindex=\"-1\">", "<main id=\"main\" class=\"app-main\" tabindex=\"-1\"><aside class=\"example-boundary\"><strong>Disposable example workspace.</strong> Execution and feed observations are simulated; edits stay in this temporary workspace. <a href=\"/examples\">All examples &amp; what to try →</a></aside>")
                .replace("Authenticated live service. Actions affect the configured execution provider. Recorded evidence is not acceptance authority.", "Synthetic loopback example. No provider is contacted or source command executed. Local edits affect disposable stores only. Restart resets every example.");
            bytes("text/html; charset=utf-8", html.as_bytes())
        }
        "/examples" => {
            const START_HERE_STRIP: &str = r#"</header>
<section aria-labelledby="start-here-title">
<h2 id="start-here-title">Start here in 60 seconds</h2>
<div class="example-grid">
<article class="example-card">
<p class="example-eyebrow">1.1 · Live cockpit</p>
<h3><a href="/?page=overview">1.1 Factory</a></h3>
<p>Inspect attention escalations, the delivery spine, speculative merge trains, machine slot census, and expiring claims.</p>
<p class="example-meta"><a href="/?page=inbox">1.2 Attention inbox →</a></p>
</article>
<article class="example-card">
<p class="example-eyebrow">2.2 · Failed run logs &amp; CI</p>
<h3><a href="/?page=runs">2.2 Runs &amp; Evidence</a></h3>
<p>Browse retained synthetic runs, inspect failed stderr logs and test reports, or start a source-bound CI project run.</p>
<p class="example-meta"><a href="/?page=projects">CI projects →</a></p>
</article>
<article class="example-card">
<p class="example-eyebrow">2.3 · Diff &amp; merge queue</p>
<h3><a href="/?page=code-review">2.3 Code Review &amp; Queue</a></h3>
<p>Review exact revisions, interdiffs, inline threads, and the speculative merge train queue.</p>
<p class="example-meta"><a href="/?page=code-review&amp;review_tab=queue">Merge queue →</a></p>
</article>
</div>
</section>"#;
            let html = include_str!("explore/index.html").replace("</header>", START_HERE_STRIP);
            bytes("text/html; charset=utf-8", html.as_bytes())
        }
        "/example.css" => bytes(CSS, include_bytes!("explore/example.css")),
        "/example.js" => bytes(JS, include_bytes!("explore/index.js")),
        "/identity.css" => bytes(
            CSS,
            include_bytes!("../../kiln-dashboard/src/render/identity.css"),
        ),
        "/catalog.css" => bytes(
            CSS,
            include_bytes!("../../kiln-dashboard/src/render/catalog.css"),
        ),
        "/favicon.svg" => bytes("image/svg+xml", include_bytes!("../src/favicon.svg")),
        "/herdr-viewer.js" => bytes(JS, include_bytes!("../src/generated/herdr-viewer.js")),
        "/herdr-viewer.wasm.b64" => bytes(
            "text/plain; charset=utf-8",
            include_bytes!("../src/generated/herdr-viewer.wasm.b64"),
        ),
        "/herdr-viewer-notices.txt" => bytes(
            "text/plain; charset=utf-8",
            include_bytes!("../src/generated/herdr-viewer-notices.txt"),
        ),
        "/healthz" => json(200, json!({"status":"ok","synthetic":true})),
        name => {
            macro_rules! assets {
                ($($file:literal => $kind:expr),* $(,)?) => { match name { $(concat!("/", $file) => bytes($kind, include_bytes!(concat!("../src/", $file))),)* _ => error(404, "Only listed console assets are served; no filesystem paths are exposed") } };
            }
            assets! {
                "icons.js" => JS, "ui.js" => JS, "web.js" => JS, "web.css" => CSS,
                "work.js" => JS, "work.css" => CSS, "work-interaction.js" => JS,
                "work-interaction.css" => CSS, "work-evidence.js" => JS, "work-board.js" => JS,
                "code-review.js" => JS, "code-review.css" => CSS, "project.js" => JS, "project.css" => CSS,
                "source.js" => JS, "source.css" => CSS, "receipts.js" => JS, "receipts.css" => CSS,
                "workers.js" => JS, "workers.css" => CSS, "runbooks.js" => JS, "cairn.js" => JS, "cairn.css" => CSS,
                "fleet.js" => JS, "fleet.css" => CSS, "fleet-runtime.js" => JS,
                "fleet-capacity.js" => JS, "fleet-capacity.css" => CSS,
                "inventory.js" => JS, "inventory.css" => CSS, "preview-review.js" => JS,
            }
        }
    }
}
fn respond(stream: &mut TcpStream, response: ApiResponse) -> std::io::Result<()> {
    let _ = stream.set_nonblocking(false);
    stream.set_write_timeout(Some(REQUEST_DEADLINE))?;
    write!(
        stream,
        "HTTP/1.1 {} Example\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\n",
        response.status,
        response.content_type,
        response.body.len()
    )?;
    if let Some(name) = response.download_name {
        write!(
            stream,
            "Content-Disposition: attachment; filename=\"{}\"\r\n",
            name.replace(['\r', '\n', '"', '\\'], "_")
        )?;
    }
    for (name, value) in response.headers {
        if !name.contains(['\r', '\n']) && !value.contains(['\r', '\n']) {
            write!(stream, "{name}: {value}\r\n")?;
        }
    }
    stream.write_all(b"\r\n")?;
    stream.write_all(&response.body)
}

fn serve_listener(
    listener: TcpListener,
    authority: &str,
    workspace: &mut Workspace,
    stop: &Arc<AtomicBool>,
) -> Result {
    listener.set_nonblocking(true)?;
    let mut sse_threads: Vec<std::thread::JoinHandle<()>> = Vec::new();
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                match request(&stream, authority) {
                    Ok(req) => {
                        let req_path = req.target.split('?').next().unwrap_or_default();
                        let is_sse_route = req.method == "GET"
                            && matches!(req_path, "/api/events" | "/api/fleet/events");
                        match workspace.route(&req) {
                            Ok(response) if is_sse_route && response.status == 200 => {
                                sse_threads.retain(|handle| !handle.is_finished());
                                if sse_threads.len() >= MAX_SSE_STREAMS {
                                    let _ = respond(
                                        &mut stream,
                                        error(503, "Too many concurrent event streams"),
                                    );
                                    continue;
                                }
                                let (keepalive, interval) = if req_path == "/api/events" {
                                    (b": keepalive\n\n".as_slice(), SSE_KEEPALIVE_INTERVAL)
                                } else {
                                    (b": heartbeat\n\n".as_slice(), SSE_FLEET_HEARTBEAT_INTERVAL)
                                };
                                let stop_flag = Arc::clone(stop);
                                let initial = response.body;
                                sse_threads.push(std::thread::spawn(move || {
                                    let _ = stream_sse(
                                        stream,
                                        &initial,
                                        keepalive,
                                        interval,
                                        &stop_flag,
                                    );
                                }));
                            }
                            Ok(response) => {
                                if let Err(cause) = respond(&mut stream, response) {
                                    eprintln!("Example response disconnected: {cause}");
                                }
                            }
                            Err(cause) => {
                                eprintln!("Example request failed: {cause}");
                                let _ = respond(
                                    &mut stream,
                                    error(
                                        503,
                                        "Local example request failed; no fallback data was substituted",
                                    ),
                                );
                            }
                        }
                    }
                    Err(response) => {
                        if let Err(cause) = respond(&mut stream, response) {
                            eprintln!("Example response disconnected: {cause}");
                        }
                    }
                }
            }
            Err(cause) if cause.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_INTERVAL)
            }
            Err(cause) => return Err(cause.into()),
        }
    }
    for handle in sse_threads {
        let _ = handle.join();
    }
    Ok(())
}

// r[impl kiln.web.exploration.scenario.repeat]
fn main() -> Result {
    let mut port = DEFAULT_PORT;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => port = args.next().ok_or("--port needs a number")?.parse()?,
            "--help" | "-h" => {
                println!(
                    "cargo run --locked -p kiln-web-service --example explore -- [--port PORT]\nFresh synthetic workspace; IPv4 loopback only. Ctrl-C stops it. Restart resets all data."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument {arg}; use --help").into()),
        }
    }
    let listener = TcpListener::bind(SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, port))?;
    let port = listener.local_addr()?.port();
    let authority = format!("127.0.0.1:{port}");
    let root = tempfile::Builder::new()
        .prefix("kiln-website-examples-")
        .tempdir()?;
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))?;
    let mut workspace = Workspace::seed(root.path(), port)?;
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&stop))?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&stop))?;
    println!(
        "Synthetic examples; no provider is contacted or source command executed.\nExplore: http://{authority}/examples\nConsole: http://{authority}/\nRestart to reset. Ctrl-C removes the disposable workspace."
    );
    serve_listener(listener, &authority, &mut workspace, &stop)?;
    drop(workspace);
    root.close()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const HOST: &str = "127.0.0.1:18787";
    fn parse(raw: &str) -> std::result::Result<Request, ApiResponse> {
        decode_request(Cursor::new(raw.as_bytes()), HOST)
    }

    // r[verify kiln.web.exploration.scenario.admission]
    #[test]
    fn same_origin_reads_and_json_writes_are_admitted_but_foreign_authority_is_not() {
        let get = parse("GET /examples HTTP/1.1\r\nHost: 127.0.0.1:18787\r\n\r\n")
            .unwrap_or_else(|response| panic!("{}", response.status));
        assert_eq!(get.target, "/examples");
        let post = parse("POST /api/runs HTTP/1.1\r\nHost: 127.0.0.1:18787\r\nOrigin: http://127.0.0.1:18787\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}").unwrap_or_else(|response| panic!("{}", response.status));
        assert_eq!(post.method, "POST");
        assert_eq!(post.body, b"{}");
        for raw in [
            "GET /api/session HTTP/1.1\r\nHost: attacker.invalid\r\n\r\n",
            "GET /api/session HTTP/1.1\r\nHost: 127.0.0.1:18787\r\nOrigin: https://attacker.invalid\r\n\r\n",
            "GET /api/session HTTP/1.1\r\nHost: 127.0.0.1:18787\r\nSec-Fetch-Site: cross-site\r\n\r\n",
            "POST /api/runs HTTP/1.1\r\nHost: 127.0.0.1:18787\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
        ] {
            assert_eq!(parse(raw).unwrap_err().status, 403);
        }
    }

    #[test]
    fn ambiguous_framing_and_incomplete_bodies_are_rejected_before_routing() {
        for raw in [
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:18787\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:18787\r\nTransfer-Encoding: chunked\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:18787\r\nContent-Length: -1\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:18787\r\nContent-Length: 1\r\n\r\nx",
            "GET / HTTP/1.1\nHost: 127.0.0.1:18787\n\n",
            "POST /api/runs HTTP/1.1\r\nHost: 127.0.0.1:18787\r\nOrigin: http://127.0.0.1:18787\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{",
        ] {
            assert_eq!(parse(raw).unwrap_err().status, 400);
        }
        assert_eq!(
            parse(&format!(
                "GET / HTTP/1.1\r\nHost: {HOST}\r\nX-Fill: {}\r\n\r\n",
                "x".repeat(MAX_HEADER_BYTES)
            ))
            .unwrap_err()
            .status,
            400
        );
        assert_eq!(
            parse(&format!(
                "POST /api/runs HTTP/1.1\r\nHost: {HOST}\r\nContent-Length: {}\r\n\r\n",
                MAX_BODY_BYTES + 1
            ))
            .unwrap_err()
            .status,
            413
        );
    }

    #[test]
    fn static_admission_never_exposes_a_filesystem_path() {
        assert_eq!(asset("/examples").status, 200);
        for path in [
            "/../Cargo.toml",
            "/%2e%2e/Cargo.toml",
            "/examples/../../Cargo.toml",
            "/etc/passwd",
            "/ci-state/service.sqlite",
        ] {
            assert_eq!(asset(path).status, 404);
        }
    }

    // r[verify kiln.web.exploration.cases]
    #[test]
    fn plain_examples_have_no_case_identity() {
        let example = Example::new("Title", "/?page=overview", "detail");
        assert!(example.case_id.is_none());
        assert!(example.recipe.is_none());
    }

    // r[verify kiln.web.exploration.cases]
    #[test]
    fn recipe_revision_ignores_runtime_inputs() {
        let presentation = PresentationRequirements {
            theme: "system".into(),
            density: "comfortable".into(),
            viewport: "wide".into(),
            themes: vec!["light".into(), "dark".into(), "system".into()],
            densities: vec!["comfortable".into(), "compact".into()],
            viewports: vec!["narrow".into(), "wide".into()],
        };
        let interaction = InteractionRequirements {
            keyboard: true,
            narrow: true,
            dark: true,
        };
        let case = NamedCase {
            case_id: "case.test".into(),
            recipe: ("explore.case.test".into(), "v1".into()),
            presentation: presentation.clone(),
            interaction: interaction.clone(),
            subjects: vec![("run".into(), "runtime-a".into())],
            seeder_source: b"source-a".to_vec(),
        };
        let first = Example::named(case.clone(), "A", "/?page=runs&run=runtime-a", "detail");
        let case2 = NamedCase {
            subjects: vec![("run".into(), "runtime-b".into())],
            ..case
        };
        let second = Example::named(case2, "B", "/?page=runs&run=runtime-b", "other");
        assert_eq!(
            first.recipe.as_ref().unwrap().revision,
            second.recipe.as_ref().unwrap().revision
        );
    }

    // r[verify kiln.web.exploration.cases]
    #[test]
    fn recipe_revision_changes_with_source_or_requirements() {
        let presentation = PresentationRequirements {
            theme: "system".into(),
            density: "comfortable".into(),
            viewport: "wide".into(),
            themes: vec!["light".into(), "dark".into(), "system".into()],
            densities: vec!["comfortable".into(), "compact".into()],
            viewports: vec!["narrow".into(), "wide".into()],
        };
        let interaction = InteractionRequirements {
            keyboard: true,
            narrow: true,
            dark: true,
        };
        let base = NamedCase {
            case_id: "case.test".into(),
            recipe: ("explore.case.test".into(), "v1".into()),
            presentation: presentation.clone(),
            interaction: interaction.clone(),
            subjects: vec![],
            seeder_source: b"source-a".to_vec(),
        };
        let first = Example::named(base.clone(), "A", "/x", "d");
        let mut changed = base.clone();
        changed.seeder_source = b"source-b".to_vec();
        let second = Example::named(changed, "A", "/x", "d");
        assert_ne!(
            first.recipe.as_ref().unwrap().revision,
            second.recipe.as_ref().unwrap().revision
        );

        let mut changed2 = base.clone();
        changed2.presentation.theme = "dark".into();
        let third = Example::named(changed2, "A", "/x", "d");
        assert_ne!(
            first.recipe.as_ref().unwrap().revision,
            third.recipe.as_ref().unwrap().revision
        );
    }

    // r[verify kiln.web.exploration.cases.resolution]
    #[test]
    fn resolve_case_outcomes() {
        let presentation = PresentationRequirements {
            theme: "system".into(),
            density: "comfortable".into(),
            viewport: "wide".into(),
            themes: vec!["light".into(), "dark".into(), "system".into()],
            densities: vec!["comfortable".into(), "compact".into()],
            viewports: vec!["narrow".into(), "wide".into()],
        };
        let interaction = InteractionRequirements {
            keyboard: true,
            narrow: true,
            dark: true,
        };
        let case = NamedCase {
            case_id: "case.test".into(),
            recipe: ("explore.case.test".into(), "v1".into()),
            presentation,
            interaction,
            subjects: vec![("run".into(), "run-1".into())],
            seeder_source: b"source".to_vec(),
        };
        let example = Example::named(case, "A", "/?page=runs&run=run-1", "d");
        let examples = vec![example.clone()];
        let workspace = "a".repeat(64);
        let recipe = example.recipe.as_ref().unwrap().revision.clone();
        let ok = resolve_case(&examples, &workspace, "case.test", &recipe, &workspace).unwrap();
        assert_eq!(ok["workspace"], workspace);
        assert_eq!(ok["subjects"]["run"], "run-1");
        assert_eq!(
            resolve_case(&examples, &workspace, "case.missing", &recipe, &workspace)
                .unwrap_err()
                .0,
            404
        );
        let wrong_recipe = "0".repeat(64);
        assert_eq!(
            resolve_case(
                &examples,
                &workspace,
                "case.test",
                &wrong_recipe,
                &workspace
            )
            .unwrap_err()
            .0,
            409
        );
        let other_workspace = "1".repeat(64);
        assert_eq!(
            resolve_case(
                &examples,
                &workspace,
                "case.test",
                &recipe,
                &other_workspace
            )
            .unwrap_err()
            .0,
            409
        );
        assert_eq!(
            resolve_case(&examples, &workspace, "bad case", &recipe, &workspace)
                .unwrap_err()
                .0,
            400
        );
        let malformed_workspace = "w".repeat(64);
        assert_eq!(
            resolve_case(
                &examples,
                &workspace,
                "case.test",
                &recipe,
                &malformed_workspace
            )
            .unwrap_err()
            .0,
            400
        );
    }

    #[test]
    fn simulated_cockpit_fleet_admission_and_repo_routes_return_200_valid_payloads() -> Result {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir()?;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))?;
        let mut ws = Workspace::seed(root.path(), 18787)?;

        let get_req = |target: &str| Request {
            method: "GET".into(),
            target: target.into(),
            body: Vec::new(),
        };

        // 1. /examples HTML includes the 60-second Start Here strip and preserves /examples.json + case resolution.
        let examples_html = ws.route(&get_req("/examples"))?;
        assert_eq!(examples_html.status, 200);
        let html_str = String::from_utf8(examples_html.body)?;
        assert!(html_str.contains("Start here in 60 seconds"));
        assert!(html_str.contains("1.1 Factory"));
        assert!(html_str.contains("/?page=overview"));
        assert!(html_str.contains("2.2 Runs &amp; Evidence"));
        assert!(html_str.contains("/?page=runs"));
        assert!(html_str.contains("2.3 Code Review &amp; Queue"));
        assert!(html_str.contains("/?page=code-review"));

        let catalog_resp = ws.route(&get_req("/examples.json"))?;
        assert_eq!(catalog_resp.status, 200);
        let catalog: Value = serde_json::from_slice(&catalog_resp.body)?;
        assert_eq!(catalog["synthetic"], true);
        let workspace_id = catalog["workspace"]
            .as_str()
            .ok_or("catalog missing workspace")?;
        let named = catalog["examples"]
            .as_array()
            .ok_or("catalog missing examples")?
            .iter()
            .find(|ex| ex.get("case_id").and_then(Value::as_str) == Some("case.run.failed"))
            .ok_or("case.run.failed missing")?;
        let recipe_rev = named
            .pointer("/recipe/revision")
            .and_then(Value::as_str)
            .ok_or("missing recipe revision")?;
        let case_resp = ws.route(&get_req(&format!(
            "/examples/cases/case.run.failed?recipe={recipe_rev}&workspace={workspace_id}"
        )))?;
        assert_eq!(case_resp.status, 200);

        // 2. /api/projects uses a clean synthetic label rather than the tempdir path.
        let session_resp = ws.route(&get_req("/api/session"))?;
        assert_eq!(session_resp.status, 200);
        let session: Value = serde_json::from_slice(&session_resp.body)?;
        assert_eq!(session["feeds_configured"], false);

        let projects_resp = ws.route(&get_req("/api/projects"))?;
        assert_eq!(projects_resp.status, 200);
        let projects: Value = serde_json::from_slice(&projects_resp.body)?;
        let enrolled_repo = projects
            .pointer("/enrolled/0/repository")
            .and_then(Value::as_str)
            .ok_or("enrolled project missing repository")?;
        assert_eq!(enrolled_repo, SYNTHETIC_PROJECT_REPOSITORY_LABEL);
        assert!(!enrolled_repo.contains("/tmp/"));

        // 3. /api/events SSE initial frames (and negative channel query / method checks).
        let events_resp = ws.route(&get_req("/api/events"))?;
        assert_eq!(events_resp.status, 200);
        assert_eq!(events_resp.content_type, "text/event-stream");
        let events_text = String::from_utf8(events_resp.body)?;
        assert!(events_text.contains("event: ready\n"));
        assert!(events_text.contains("event: ci\n"));
        assert!(events_text.contains("event: fleet\n"));
        assert!(events_text.contains("event: repositories\n"));
        assert_eq!(ws.route(&get_req("/api/events?channels=bogus"))?.status, 400);
        assert_eq!(
            ws.route(&Request {
                method: "POST".into(),
                target: "/api/events".into(),
                body: Vec::new(),
            })?
            .status,
            405
        );

        // 4. /api/implementation/attention
        let attention_resp = ws.route(&get_req("/api/implementation/attention"))?;
        assert_eq!(attention_resp.status, 200);
        let attention: Value = serde_json::from_slice(&attention_resp.body)?;
        assert_eq!(attention["schema"], ATTENTION_SCHEMA);
        assert_eq!(attention["synthetic"], true);
        assert!(
            attention["items"]
                .as_array()
                .ok_or("attention items not array")?
                .len()
                >= 3
        );
        assert_eq!(
            ws.route(&get_req("/api/implementation/attention?bad=1"))?
                .status,
            400
        );

        // 5. /api/fleet/claims (with and without ?include=foreign,expired)
        let claims_resp = ws.route(&get_req("/api/fleet/claims?include=foreign,expired"))?;
        assert_eq!(claims_resp.status, 200);
        let claims: Value = serde_json::from_slice(&claims_resp.body)?;
        assert_eq!(claims["synthetic"], true);
        assert!(
            !claims["claims"]
                .as_array()
                .ok_or("claims not array")?
                .is_empty()
        );
        assert!(
            !claims["foreign_claims"]
                .as_array()
                .ok_or("foreign_claims not array")?
                .is_empty()
        );
        assert!(
            !claims["expired_claims"]
                .as_array()
                .ok_or("expired_claims not array")?
                .is_empty()
        );
        assert_eq!(ws.route(&get_req("/api/fleet/claims"))?.status, 200);
        assert_eq!(
            ws.route(&get_req("/api/fleet/claims?include=bogus"))?
                .status,
            400
        );

        // 6. /api/fleet/dispatch/status
        let dispatch_resp = ws.route(&get_req("/api/fleet/dispatch/status"))?;
        assert_eq!(dispatch_resp.status, 200);
        let dispatch: Value = serde_json::from_slice(&dispatch_resp.body)?;
        assert_eq!(dispatch["state"], "running");
        assert_eq!(dispatch["synthetic"], true);

        // 7. /api/fleet/attempts (+ pane + /api/fleet/decisions + /api/fleet/health)
        let attempts_resp = ws.route(&get_req("/api/fleet/attempts"))?;
        assert_eq!(attempts_resp.status, 200);
        let attempts: Value = serde_json::from_slice(&attempts_resp.body)?;
        let attempt_id = attempts
            .pointer("/attempts/0/attempt")
            .and_then(Value::as_str)
            .ok_or("attempt id missing")?;
        assert!(is_hex64(attempt_id));
        assert_eq!(
            ws.route(&get_req(&format!("/api/fleet/attempts/{attempt_id}/pane")))?
                .status,
            200
        );
        assert_eq!(ws.route(&get_req("/api/fleet/decisions"))?.status, 200);
        assert_eq!(ws.route(&get_req("/api/fleet/health"))?.status, 200);

        // 8. /api/fleet/work
        let work_resp = ws.route(&get_req("/api/fleet/work"))?;
        assert_eq!(work_resp.status, 200);
        let work: Value = serde_json::from_slice(&work_resp.body)?;
        assert_eq!(work["ready_unclaimed"], 1);
        assert_eq!(
            work.pointer("/ranked/0/identity").and_then(Value::as_str),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(
            work.pointer("/ranked/0/interactive"),
            Some(&Value::Bool(true))
        );

        // 9. /api/fleet/placement/guest-move (fail-closed)
        let gm_resp = ws.route(&get_req("/api/fleet/placement/guest-move"))?;
        assert_eq!(gm_resp.status, 200);
        let gm: Value = serde_json::from_slice(&gm_resp.body)?;
        assert_eq!(gm["schema"], GUEST_MOVE_OVERVIEW_SCHEMA);
        assert_eq!(gm["available"], false);

        // 10. /api/fleet/swarms, detail, graph, and /api/fleet/events
        let swarms_resp = ws.route(&get_req("/api/fleet/swarms"))?;
        assert_eq!(swarms_resp.status, 200);
        let swarms: Value = serde_json::from_slice(&swarms_resp.body)?;
        let swarm_id = swarms
            .pointer("/swarms/0/identity")
            .and_then(Value::as_str)
            .ok_or("swarm id missing")?;
        assert_eq!(
            ws.route(&get_req(&format!("/api/fleet/swarms/{swarm_id}")))?
                .status,
            200
        );
        let graph_resp = ws.route(&get_req(&format!(
            "/api/fleet/swarms/{swarm_id}/graph?up_to_seq=2"
        )))?;
        assert_eq!(graph_resp.status, 200);
        let graph: Value = serde_json::from_slice(&graph_resp.body)?;
        assert_eq!(graph.pointer("/graph/up_to_seq"), Some(&json!(2)));
        assert_eq!(ws.route(&get_req("/api/fleet/events"))?.status, 200);

        // 11. /api/implementation/admission
        let adm_resp = ws.route(&get_req("/api/implementation/admission"))?;
        assert_eq!(adm_resp.status, 200);
        let adm: Value = serde_json::from_slice(&adm_resp.body)?;
        assert_eq!(adm["schema"], ADMISSION_SCHEMA);
        assert_eq!(adm["configured"], true);
        assert!(
            !adm["accounts"]
                .as_array()
                .ok_or("accounts missing")?
                .is_empty()
        );

        // 12. /api/implementation/campaigns (index + detail)
        let camps_resp = ws.route(&get_req("/api/implementation/campaigns"))?;
        assert_eq!(camps_resp.status, 200);
        let camps: Value = serde_json::from_slice(&camps_resp.body)?;
        assert_eq!(camps["schema"], CAMPAIGN_INDEX_SCHEMA);
        assert_eq!(
            camps
                .pointer("/campaigns/0/campaign_id")
                .and_then(Value::as_str),
            Some(campaign::CAMPAIGN_ID)
        );
        let camp_detail = ws.route(&get_req(&format!(
            "/api/implementation/campaigns?campaign={}",
            campaign::CAMPAIGN_ID
        )))?;
        assert_eq!(camp_detail.status, 200);

        // 13. /api/repositories (+ refresh)
        let repos_resp = ws.route(&get_req("/api/repositories"))?;
        assert_eq!(repos_resp.status, 200);
        let repos: Value = serde_json::from_slice(&repos_resp.body)?;
        assert_eq!(repos["schema"], REPOSITORIES_SCHEMA);
        assert_eq!(repos.pointer("/summary/total"), Some(&json!(3)));
        assert_eq!(
            ws.route(&Request {
                method: "POST".into(),
                target: "/api/repositories/refresh".into(),
                body: Vec::new(),
            })?
            .status,
            200
        );

        Ok(())
    }

    #[test]
    fn live_sse_stream_stays_open_without_blocking_concurrent_requests() -> Result {
        use std::os::unix::fs::PermissionsExt as _;
        let listener = TcpListener::bind(SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let authority = format!("127.0.0.1:{port}");
        let root = tempfile::tempdir()?;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))?;
        let mut ws = Workspace::seed(root.path(), port)?;
        let stop = Arc::new(AtomicBool::new(false));
        let server_stop = Arc::clone(&stop);
        let server_authority = authority.clone();
        let server_handle = std::thread::spawn(move || {
            let _ = serve_listener(listener, &server_authority, &mut ws, &server_stop);
        });

        let mut sse = TcpStream::connect(&authority)?;
        sse.set_read_timeout(Some(Duration::from_secs(3)))?;
        write!(
            sse,
            "GET /api/events HTTP/1.1\r\nHost: {authority}\r\n\r\n"
        )?;
        sse.flush()?;

        let mut reader = BufReader::new(sse);
        let mut sse_head = String::new();
        for _ in 0..48 {
            let mut line = String::new();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }
            sse_head.push_str(&line);
            if sse_head.contains("event: repositories\n") && sse_head.ends_with("\n\n") {
                break;
            }
        }
        assert!(sse_head.starts_with("HTTP/1.1 200 Example\r\n"));
        assert!(sse_head.contains("Content-Type: text/event-stream\r\n"));
        assert!(sse_head.contains("event: ready\n"));
        assert!(sse_head.contains("event: ci\n"));
        assert!(sse_head.contains("event: fleet\n"));
        assert!(sse_head.contains("event: repositories\n"));

        // While the SSE stream remains open, regular API requests must still be served immediately.
        let mut api_sock = TcpStream::connect(&authority)?;
        api_sock.set_read_timeout(Some(Duration::from_secs(3)))?;
        write!(
            api_sock,
            "GET /api/implementation/attention HTTP/1.1\r\nHost: {authority}\r\n\r\n"
        )?;
        api_sock.flush()?;
        let mut api_resp = String::new();
        api_sock.read_to_string(&mut api_resp)?;
        assert!(api_resp.starts_with("HTTP/1.1 200 Example\r\n"));
        assert!(api_resp.contains(ATTENTION_SCHEMA));

        drop(reader);
        stop.store(true, Ordering::Release);
        server_handle
            .join()
            .map_err(|_| "SSE server thread panicked")?;
        Ok(())
    }
}
