#![cfg(unix)]
//! Fleet API routing, authentication, MCP and the TCP server against a
//! temporary store and a scripted `tailscale`.

mod common;

use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use common::{COORDINATOR, PEER, TestResult, config, executable, write_enrollment};
use kiln_fleet::api::Service;
use kiln_fleet::client::ApiClient;
use kiln_fleet::config::FleetConfig;
use kiln_fleet::http::{Request, Response};
use kiln_fleet::identity::PeerIdentity;
use kiln_fleet::mcp::McpServer;
use serde_json::{Value, json};

const TAILSCALE: &str = r#"#!/bin/sh
printf '%s\n' "$3" >> "$0.log"
case "$3" in
  100.100.1.1) printf '{"Node":{"Name":"aspen3-1.bison-tailor.ts.net.","ID":1},"UserProfile":{"LoginName":"x"}}\n' ;;
  100.100.1.2) printf '{"Node":{"Name":"stranger.bison-tailor.ts.net."}}\n' ;;
  100.100.1.4) printf 'not json\n' ;;
  100.100.1.5) printf '{"Node":{"Name":"ASPEN1-1.bison-tailor.ts.net"}}\n' ;;
  *) echo "no such peer" >&2; exit 1 ;;
esac
"#;

static FAKE_TAILSCALE: LazyLock<PathBuf> = LazyLock::new(|| executable("tailscale", TAILSCALE));

struct Fixture {
    _directory: tempfile::TempDir,
    service: Arc<Service>,
}

fn fixture() -> TestResult<Fixture> {
    let directory = tempfile::tempdir()?;
    write_enrollment(directory.path())?;
    let document = config(directory.path(), &FAKE_TAILSCALE);
    let config = FleetConfig::from_json(&serde_json::to_vec(&document)?)?;
    let service = Arc::new(Service::new(config)?);
    Ok(Fixture {
        _directory: directory,
        service,
    })
}

fn request(method: &str, target: &str, body: Option<Value>) -> Request {
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_owned(), Some(q.to_owned())),
        None => (target.to_owned(), None),
    };
    Request {
        method: method.to_owned(),
        path,
        query,
        body: body
            .map(|body| body.to_string().into_bytes())
            .unwrap_or_default(),
    }
}

impl Fixture {
    fn call(&self, machine: &str, method: &str, path: &str, body: Option<Value>) -> Response {
        self.service.route(
            machine,
            &request(method, path, body),
            kiln_fleet::log::now_ms(),
        )
    }
}

fn claim_body(scope: &str, session: &str) -> Value {
    json!({ "scope": scope, "tool": "omp", "session": session, "purpose": "edit the fleet crate" })
}

fn text<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

#[test]
// r[verify kiln.fleet.api.scenario.identity]
fn peers_are_loopback_or_configured_tailnet_nodes() -> TestResult {
    let fixture = fixture()?;
    let identity = PeerIdentity::new(&fixture.service.config);
    let now = kiln_fleet::log::now_ms();
    let ip = |text: &str| text.parse::<IpAddr>().unwrap_or_else(|_| unreachable!());
    assert_eq!(identity.identify(ip("127.0.0.1"), now)?, COORDINATOR);
    assert_eq!(identity.identify(ip("::1"), now)?, COORDINATOR);
    assert_eq!(identity.identify(ip("::ffff:127.0.0.1"), now)?, COORDINATOR);
    assert_eq!(identity.identify(ip("100.100.1.1"), now)?, PEER);
    // Node names compare without case and without the trailing dot.
    assert_eq!(identity.identify(ip("100.100.1.5"), now)?, "aspen1");
    assert!(
        identity.identify(ip("100.100.1.2"), now).is_err(),
        "unknown node"
    );
    assert!(
        identity.identify(ip("100.100.1.3"), now).is_err(),
        "failed whois"
    );
    assert!(
        identity.identify(ip("100.100.1.4"), now).is_err(),
        "malformed whois"
    );
    assert!(
        identity.identify(ip("192.168.1.10"), now).is_err(),
        "off-tailnet"
    );
    // One lookup per address within the cache window, another after it.
    assert_eq!(identity.identify(ip("100.100.1.5"), now + 1)?, "aspen1");
    let log = std::fs::read_to_string(format!("{}.log", FAKE_TAILSCALE.display()))?;
    assert_eq!(log.lines().filter(|line| *line == "100.100.1.5").count(), 1);
    assert!(log.lines().all(|line| line != "192.168.1.10"));
    identity.identify(
        ip("100.100.1.5"),
        now + kiln_fleet::identity::WHOIS_CACHE_TTL_MS + 1,
    )?;
    let log = std::fs::read_to_string(format!("{}.log", FAKE_TAILSCALE.display()))?;
    assert_eq!(log.lines().filter(|line| *line == "100.100.1.5").count(), 2);
    // An unauthenticated peer never reaches a handler.
    let refused = fixture
        .service
        .handle(ip("100.100.1.2"), &request("GET", "/v1/health", None));
    assert_eq!(refused.status, 403);
    let health = fixture
        .service
        .handle(ip("100.100.1.1"), &request("GET", "/v1/health", None));
    assert_eq!(health.status, 200);
    assert_eq!(text(&health.body, "/peer"), PEER);
    assert_eq!(text(&health.body, "/machine"), COORDINATOR);
    let version = fixture.call(COORDINATOR, "GET", "/v1/version", None);
    assert_eq!(version.status, 200);
    assert!(
        text(&version.body, "/version")
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
    assert_eq!(text(&version.body, "/version").len(), 64);
    let metrics = fixture.call(COORDINATOR, "GET", "/v1/metrics", None);
    assert_eq!(metrics.status, 200);
    let text = metrics.text.as_deref().unwrap_or_default();
    assert!(text.contains("kiln_fleet_claims 0"));
    assert!(text.contains("kiln_fleet_decisions_open 0"));
    let mut bytes = Vec::new();
    kiln_fleet::http::write_response(&mut bytes, &metrics)?;
    assert!(String::from_utf8_lossy(&bytes).starts_with(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4; charset=utf-8\r\n"
    ));
    Ok(())
}

#[test]
// r[verify kiln.fleet.api] r[verify kiln.fleet.api.scenario.identity]
fn serves_the_operator_view_only_to_authenticated_peers() -> TestResult {
    let ip = |text: &str| text.parse::<IpAddr>().unwrap_or_else(|_| unreachable!());
    let fixture = fixture()?;
    let page = fixture
        .service
        .handle(ip("127.0.0.1"), &request("GET", "/", None));
    assert_eq!(page.status, 200);
    let document = page.html.as_deref().unwrap_or_default();
    assert!(document.contains("Kiln fleet"));
    assert!(document.contains("/v1/claims"));
    assert!(document.contains("/v1/decisions"));
    let mut bytes = Vec::new();
    kiln_fleet::http::write_response(&mut bytes, &page)?;
    let head = String::from_utf8_lossy(&bytes);
    assert!(head.starts_with("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n"));
    assert!(head.contains("Content-Security-Policy:"));
    let alias = fixture
        .service
        .handle(ip("127.0.0.1"), &request("GET", "/ui", None));
    assert_eq!(alias.status, 200);
    let refused = fixture
        .service
        .handle(ip("100.100.1.2"), &request("GET", "/", None));
    assert_eq!(refused.status, 403);
    Ok(())
}

#[test]
// r[verify kiln.fleet.api] r[verify kiln.fleet.api.scenario.identity] r[verify kiln.fleet.claims.scenario.conflict]
fn claims_bind_the_authenticated_host_and_map_statuses() -> TestResult {
    let fixture = fixture()?;
    let mut declared = claim_body("OnixResearch/kiln crates/kiln-fleet", "s1");
    if let Some(object) = declared.as_object_mut() {
        object.insert("host".into(), json!(COORDINATOR));
    }
    let granted = fixture.call(PEER, "POST", "/v1/claims", Some(declared.clone()));
    assert_eq!(granted.status, 201, "{}", granted.body);
    assert_eq!(text(&granted.body, "/claim/holder/host"), PEER);
    let identity = text(&granted.body, "/claim/identity").to_owned();
    let fence = granted
        .body
        .pointer("/claim/fence")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    assert_eq!(
        fixture
            .call(PEER, "POST", "/v1/claims", Some(declared))
            .status,
        200
    );
    let conflict = fixture.call(
        COORDINATOR,
        "POST",
        "/v1/claims",
        Some(claim_body("onixresearch/kiln crates", "s2")),
    );
    assert_eq!(conflict.status, 409);
    assert_eq!(text(&conflict.body, "/code"), "fleet_claim_conflict");
    assert_eq!(
        fixture
            .call(
                COORDINATOR,
                "POST",
                "/v1/claims",
                Some(claim_body("OnixResearch/kiln docs", "s2"))
            )
            .status,
        201
    );
    for (body, status) in [
        (claim_body("OnixResearch/kiln ../etc", "s3"), 400),
        (claim_body("OnixResearch/kiln", "s3"), 400),
        (
            json!({ "scope": "OnixResearch/kiln x", "tool": "omp" }),
            400,
        ),
        (
            json!({ "scope": "OnixResearch/kiln x", "tool": "omp", "session": "s", "purpose": "p", "extra": 1 }),
            400,
        ),
        (
            json!({ "scope": "OnixResearch/kiln y", "tool": "omp", "session": "s", "purpose": "p", "lease_minutes": 100_000 }),
            400,
        ),
    ] {
        assert_eq!(
            fixture.call(PEER, "POST", "/v1/claims", Some(body)).status,
            status
        );
    }
    let listed = fixture.call(COORDINATOR, "GET", "/v1/claims", None);
    assert_eq!(listed.status, 200);
    assert_eq!(
        listed
            .body
            .pointer("/claims")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );

    let renew = format!("/v1/claims/{identity}/renew");
    // Only the holder's machine (or an operator) changes a claim.
    assert_eq!(
        fixture
            .call("aspen1", "POST", &renew, Some(json!({ "fence": fence })))
            .status,
        403
    );
    assert_eq!(
        fixture
            .call(PEER, "POST", &renew, Some(json!({ "fence": fence + 9 })))
            .status,
        409
    );
    assert_eq!(
        fixture
            .call(
                PEER,
                "POST",
                &renew,
                Some(json!({ "fence": fence, "lease_minutes": 30 }))
            )
            .status,
        200
    );
    let unknown = format!("/v1/claims/{}/renew", "e".repeat(64));
    assert_eq!(
        fixture
            .call(PEER, "POST", &unknown, Some(json!({ "fence": 1 })))
            .status,
        404
    );
    assert_eq!(
        fixture
            .call(
                PEER,
                "POST",
                "/v1/claims/zz/renew",
                Some(json!({ "fence": 1 }))
            )
            .status,
        400
    );
    let release = format!("/v1/claims/{identity}/release");
    let released = fixture.call(
        PEER,
        "POST",
        &release,
        Some(json!({ "fence": fence, "note": "done" })),
    );
    assert_eq!(released.status, 200, "{}", released.body);
    assert_eq!(text(&released.body, "/outcome"), "released");
    assert_eq!(
        fixture
            .call(PEER, "POST", &release, Some(json!({ "fence": fence })))
            .status,
        409
    );

    assert_eq!(fixture.call(PEER, "GET", "/v1/nothing", None).status, 404);
    assert_eq!(fixture.call(PEER, "DELETE", "/v1/claims", None).status, 405);
    assert_eq!(fixture.call(PEER, "GET", "/v2/claims", None).status, 404);
    for path in ["/v1/work", "/v1/attempts", "/v1/decisions"] {
        assert_eq!(fixture.call(PEER, "GET", path, None).status, 200, "{path}");
    }
    Ok(())
}

#[test]
// r[verify kiln.fleet.api] r[verify kiln.fleet.decisions]
fn operator_routes_refuse_other_machines() -> TestResult {
    let fixture = fixture()?;
    let attempt = "a".repeat(64);
    let cancel = format!("/v1/attempts/{attempt}/cancel");
    let answer = format!("/v1/decisions/{attempt}/answer");
    let response = fixture.call(PEER, "POST", &cancel, Some(json!({ "reason": "stop" })));
    assert_eq!(
        (response.status, text(&response.body, "/code")),
        (403, "fleet_operator")
    );
    assert_eq!(
        fixture
            .call(PEER, "POST", &answer, Some(json!({ "text": "yes" })))
            .status,
        403
    );
    // Operators pass the gate and reach the store.
    assert_eq!(
        fixture
            .call(
                COORDINATOR,
                "POST",
                &cancel,
                Some(json!({ "reason": "stop" }))
            )
            .status,
        404
    );
    assert_eq!(
        fixture
            .call(COORDINATOR, "POST", &answer, Some(json!({ "text": "yes" })))
            .status,
        404
    );
    // Handoffs need a full commit and a live session.
    let handoff = format!("/v1/attempts/{attempt}/handoff");
    assert_eq!(
        fixture
            .call(
                PEER,
                "POST",
                &handoff,
                Some(json!({ "fence": 1, "commit": "abc123" }))
            )
            .status,
        400
    );
    assert_eq!(
        fixture
            .call(
                PEER,
                "POST",
                &handoff,
                Some(json!({ "fence": 1, "commit": "b".repeat(40) }))
            )
            .status,
        404
    );
    Ok(())
}

fn serve(fixture: &Fixture) -> TestResult<String> {
    let listeners = kiln_fleet::server::bind(&["127.0.0.1:0".parse()?])?;
    let address = listeners.first().ok_or("no listener")?.local_addr()?;
    kiln_fleet::server::spawn(listeners, &fixture.service)?;
    Ok(format!("http://{address}"))
}

fn raw(server: &str, bytes: &[u8]) -> TestResult<String> {
    let mut stream = TcpStream::connect(server.trim_start_matches("http://"))?;
    stream.write_all(bytes)?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

#[test]
// r[verify kiln.fleet.api.scenario.identity]
fn the_server_bounds_bodies_and_answers_json() -> TestResult {
    let fixture = fixture()?;
    let server = serve(&fixture)?;
    let client = ApiClient::new(&server)?;
    let (status, body) = client.request("GET", "/v1/health", None)?;
    assert_eq!((status, text(&body, "/machine")), (200, COORDINATOR));
    let oversized = format!(
        "POST /v1/claims HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
        kiln_fleet::http::MAX_BODY_BYTES + 1
    );
    assert!(raw(&server, oversized.as_bytes())?.starts_with("HTTP/1.1 413"));
    assert!(raw(&server, b"POST /v1/claims HTTP/1.1\r\n\r\n")?.starts_with("HTTP/1.1 411"));
    let (status, _) = client.request(
        "POST",
        "/v1/claims",
        Some(&claim_body("OnixResearch/kiln src", "s")),
    )?;
    assert_eq!(status, 201);
    Ok(())
}

fn mcp_call(server: &mut McpServer, id: u64, method: &str, params: Value) -> TestResult<Value> {
    let line =
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string();
    let reply = server.handle_line(&line).ok_or("no reply")?;
    Ok(serde_json::from_str(&reply)?)
}

#[test]
// r[verify kiln.fleet.api]
fn mcp_tools_call_the_api() -> TestResult {
    let fixture = fixture()?;
    let url = serve(&fixture)?;
    let mut server = McpServer::new(ApiClient::new(&url)?, None);
    let initialized = mcp_call(
        &mut server,
        1,
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "claude-code", "version": "1" } }),
    )?;
    assert_eq!(text(&initialized, "/result/serverInfo/name"), "kiln-fleet");
    assert_eq!(text(&initialized, "/result/protocolVersion"), "2025-06-18");
    assert_eq!(server.tool(), "claude-code");
    assert!(
        server
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none()
    );
    assert!(
        mcp_call(&mut server, 2, "ping", json!({}))?
            .get("result")
            .is_some()
    );
    let tools = mcp_call(&mut server, 3, "tools/list", json!({}))?;
    let tools = tools
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .ok_or("no tools")?;
    let names: Vec<&str> = tools.iter().map(|tool| text(tool, "/name")).collect();
    assert_eq!(
        names,
        [
            "kiln_claims",
            "kiln_claim",
            "kiln_renew",
            "kiln_release",
            "kiln_work",
            "kiln_attempts",
            "kiln_handoff",
            "kiln_decisions",
            "kiln_swarm_create",
            "kiln_swarm_spawn",
            "kiln_swarm_post",
            "kiln_swarm_read",
            "kiln_swarm_verdict",
            "kiln_swarm_watchdog",
            "kiln_swarm_failover",
            "kiln_swarm_status",
        ]
    );
    assert!(
        tools
            .iter()
            .all(|tool| text(tool, "/description").contains("untrusted data"))
    );
    let resources = mcp_call(&mut server, 9, "resources/list", json!({}))?;
    let resources = resources
        .pointer("/result/resources")
        .and_then(Value::as_array)
        .ok_or("no resources")?;
    let uris: Vec<&str> = resources.iter().map(|entry| text(entry, "/uri")).collect();
    assert_eq!(
        uris,
        [
            "kiln-fleet://claims",
            "kiln-fleet://work",
            "kiln-fleet://attempts",
            "kiln-fleet://decisions",
            "kiln-fleet://swarms",
        ]
    );
    let claims_resource = mcp_call(
        &mut server,
        10,
        "resources/read",
        json!({ "uri": "kiln-fleet://claims" }),
    )?;
    assert!(text(&claims_resource, "/result/contents/0/text").contains("claims"));
    let unknown_resource = mcp_call(
        &mut server,
        11,
        "resources/read",
        json!({ "uri": "kiln-fleet://nope" }),
    )?;
    assert!(unknown_resource.get("error").is_some());

    let claimed = mcp_call(
        &mut server,
        4,
        "tools/call",
        json!({ "name": "kiln_claim", "arguments": { "scope": "OnixResearch/kiln docs", "purpose": "docs", "session": "/home/x/kiln" } }),
    )?;
    assert_eq!(claimed.pointer("/result/isError"), Some(&json!(false)));
    let payload: Value = serde_json::from_str(text(&claimed, "/result/content/0/text"))?;
    assert_eq!(payload.pointer("/status"), Some(&json!(201)));
    assert_eq!(text(&payload, "/response/claim/holder/tool"), "claude-code");
    assert_eq!(text(&payload, "/response/claim/holder/host"), COORDINATOR);
    let conflict = mcp_call(
        &mut server,
        5,
        "tools/call",
        json!({ "name": "kiln_claim", "arguments": { "scope": "OnixResearch/kiln docs/a", "purpose": "docs", "session": "other" } }),
    )?;
    assert_eq!(conflict.pointer("/result/isError"), Some(&json!(true)));
    let listed = mcp_call(
        &mut server,
        6,
        "tools/call",
        json!({ "name": "kiln_claims", "arguments": {} }),
    )?;
    assert!(text(&listed, "/result/content/0/text").contains("docs"));
    let missing = mcp_call(
        &mut server,
        7,
        "tools/call",
        json!({ "name": "kiln_claim", "arguments": { "scope": "x y" } }),
    )?;
    assert!(missing.get("error").is_some());
    assert!(
        mcp_call(&mut server, 8, "nope", json!({}))?
            .get("error")
            .is_some()
    );
    let parse_error: Value = serde_json::from_str(&server.handle_line("{").ok_or("no reply")?)?;
    assert_eq!(parse_error.pointer("/error/code"), Some(&json!(-32_700)));

    // `--tool` overrides the client-derived identity.
    let explicit = McpServer::new(ApiClient::new(&url)?, Some("codex".into()));
    assert_eq!(explicit.tool(), "codex");
    Ok(())
}

#[test]
// r[verify kiln.fleet.swarm.surface]
// r[verify kiln.fleet.swarm.surface.scenario.http_and_mcp]
fn swarm_http_api_enforces_delegation_verdicts_watchdog_failover_and_graph() -> TestResult {
    let fixture = fixture()?;
    let created = fixture.call(
        COORDINATOR,
        "POST",
        "/v1/swarms",
        Some(json!({
            "title": "Kiln #118 control plane",
            "allowed_scopes": ["OnixResearch/kiln crates/kiln-core"],
            "max_concurrency": 3,
            "max_repair_rounds": 1,
            "budget_ceiling_units": 500,
            "stale_after_ms": 60000
        })),
    );
    assert_eq!(created.status, 201, "{}", created.body);
    let swarm_id = text(&created.body, "/swarm/identity").to_owned();
    assert_eq!(swarm_id.len(), 64);

    // List and get swarm.
    let listed = fixture.call(COORDINATOR, "GET", "/v1/swarms", None);
    assert_eq!(listed.status, 200);
    assert_eq!(
        listed
            .body
            .pointer("/swarms")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    let fetched = fixture.call(COORDINATOR, "GET", &format!("/v1/swarms/{swarm_id}"), None);
    assert_eq!(fetched.status, 200);

    // Scope widening -> 403 Forbidden.
    let widened = fixture.call(
        COORDINATOR,
        "POST",
        &format!("/v1/swarms/{swarm_id}/members"),
        Some(json!({
            "name": "bad-author",
            "role": "author",
            "harness": "omp",
            "model": "gpt-5",
            "scope": "OnixResearch/kiln .",
            "brief": "Try widening"
        })),
    );
    assert_eq!(widened.status, 403, "{}", widened.body);

    // Verifier with write scope -> 403 Forbidden.
    let bad_verifier = fixture.call(
        COORDINATOR,
        "POST",
        &format!("/v1/swarms/{swarm_id}/members"),
        Some(json!({
            "name": "bad-verifier",
            "role": "verifier",
            "harness": "claude-code",
            "model": "claude-opus-4-6",
            "scope": "OnixResearch/kiln crates/kiln-core/src/fleet",
            "brief": "Should fail"
        })),
    );
    assert_eq!(bad_verifier.status, 403, "{}", bad_verifier.body);

    // Non-operator peer cannot create swarms -> 403.
    let peer_create = fixture.call(
        PEER,
        "POST",
        "/v1/swarms",
        Some(json!({
            "title": "Unauthorized",
            "allowed_scopes": ["OnixResearch/kiln crates/kiln-core"]
        })),
    );
    assert_eq!(peer_create.status, 403, "{}", peer_create.body);

    // Non-operator peer cannot spawn with requester=None (operator privilege) -> 403.
    let peer_spawn = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/members"),
        Some(json!({
            "name": "rogue",
            "role": "author",
            "harness": "omp",
            "model": "gpt-5",
            "scope": "OnixResearch/kiln crates/kiln-core/src/fleet",
            "brief": "Unauthorized spawn"
        })),
    );
    assert_eq!(peer_spawn.status, 403, "{}", peer_spawn.body);

    // Operator dispatches valid author on PEER and read-only verifier on COORDINATOR.
    let author = fixture.call(
        COORDINATOR,
        "POST",
        &format!("/v1/swarms/{swarm_id}/members"),
        Some(json!({
            "name": "author-1",
            "role": "author",
            "host": PEER,
            "harness": "antigravity",
            "model": "gemini-3.1-pro",
            "scope": "OnixResearch/kiln crates/kiln-core/src/fleet",
            "brief": "Implement swarm core"
        })),
    );
    assert_eq!(author.status, 201, "{}", author.body);
    assert_eq!(text(&author.body, "/member/host"), PEER);

    // Overlapping second author -> 409 Conflict.
    let overlap = fixture.call(
        COORDINATOR,
        "POST",
        &format!("/v1/swarms/{swarm_id}/members"),
        Some(json!({
            "name": "author-2",
            "role": "author",
            "harness": "omp",
            "model": "gpt-5",
            "scope": "OnixResearch/kiln crates/kiln-core/src/fleet/swarm",
            "brief": "Overlaps author-1"
        })),
    );
    assert_eq!(overlap.status, 409, "{}", overlap.body);

    let verifier = fixture.call(
        COORDINATOR,
        "POST",
        &format!("/v1/swarms/{swarm_id}/members"),
        Some(json!({
            "name": "verifier-1",
            "role": "verifier",
            "harness": "claude-code",
            "model": "claude-opus-4-6",
            "brief": "Audit author-1"
        })),
    );
    assert_eq!(verifier.status, 201, "{}", verifier.body);

    // Raw privileged board post -> 403 Forbidden.
    let forged = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/board"),
        Some(json!({
            "sender": "author-1",
            "kind": "verdict",
            "mentions": [],
            "body": "APPROVE"
        })),
    );
    assert_eq!(forged.status, 403, "{}", forged.body);

    // Park verifier waiting at seq 2, then author posts handoff mentioning verifier.
    let parked = fixture.call(
        COORDINATOR,
        "POST",
        &format!("/v1/swarms/{swarm_id}/members"),
        Some(json!({
            "action": "update_state",
            "member": "verifier-1",
            "state": { "state": "waiting", "since_seq": 2 },
            "budget_draw_delta": 5
        })),
    );
    assert_eq!(parked.status, 200, "{}", parked.body);

    let commit_sha = "a".repeat(40);
    let handoff = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/board"),
        Some(json!({
            "sender": "author-1",
            "kind": "handoff",
            "mentions": [{ "kind": "member", "target": "verifier-1" }],
            "body": "Ready for audit",
            "candidate_commit": commit_sha
        })),
    );
    assert_eq!(handoff.status, 201, "{}", handoff.body);
    let handoff_seq = handoff
        .body
        .pointer("/post/seq")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    assert_eq!(handoff_seq, 3);

    // Watchdog wakes waiting verifier-1.
    let wd = fixture.call(
        COORDINATOR,
        "POST",
        &format!("/v1/swarms/{swarm_id}/watchdog"),
        Some(json!({ "apply": true })),
    );
    assert_eq!(wd.status, 200, "{}", wd.body);
    assert_eq!(text(&wd.body, "/actions/0/action"), "wake_member");
    assert_eq!(text(&wd.body, "/actions/0/member"), "verifier-1");

    // Non-verifier verdict -> 403 Forbidden.
    let bad_verdict = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/verdicts"),
        Some(json!({
            "verifier": "author-1",
            "author": "author-1",
            "commit": commit_sha,
            "verdict": "approve"
        })),
    );
    assert_eq!(bad_verdict.status, 403, "{}", bad_verdict.body);

    // Cross-host spoofing checks: PEER cannot spoof COORDINATOR's verifier-1 on verdicts,
    // board posts, or state updates, nor trigger watchdog(apply=true) or failover!
    let spoof_verdict = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/verdicts"),
        Some(json!({
            "verifier": "verifier-1",
            "author": "author-1",
            "commit": commit_sha,
            "verdict": "approve"
        })),
    );
    assert_eq!(spoof_verdict.status, 403, "{}", spoof_verdict.body);
    assert_eq!(
        text(&spoof_verdict.body, "/code"),
        "fleet_swarm_host_mismatch"
    );

    let spoof_post = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/board"),
        Some(json!({
            "sender": "verifier-1",
            "kind": "status",
            "mentions": [],
            "body": "spoofed verifier status"
        })),
    );
    assert_eq!(spoof_post.status, 403, "{}", spoof_post.body);
    assert_eq!(text(&spoof_post.body, "/code"), "fleet_swarm_host_mismatch");

    let spoof_state = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/members"),
        Some(json!({
            "action": "update_state",
            "member": "verifier-1",
            "state": { "state": "retired" }
        })),
    );
    assert_eq!(spoof_state.status, 403, "{}", spoof_state.body);
    assert_eq!(
        text(&spoof_state.body, "/code"),
        "fleet_swarm_host_mismatch"
    );

    let peer_wd_apply = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/watchdog"),
        Some(json!({ "apply": true })),
    );
    assert_eq!(peer_wd_apply.status, 403, "{}", peer_wd_apply.body);
    // Read-only watchdog evaluation (apply=false) is permitted for authenticated peers.
    let peer_wd_read = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/watchdog"),
        Some(json!({ "apply": false })),
    );
    assert_eq!(peer_wd_read.status, 200, "{}", peer_wd_read.body);

    let peer_failover = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/failover"),
        Some(json!({
            "member": "author-1",
            "replacement_model": "claude-opus-4-6"
        })),
    );
    assert_eq!(peer_failover.status, 403, "{}", peer_failover.body);

    // Pause author-1 on quota, then failover preserving board_cursor & checkpoint_commit.
    let paused = fixture.call(
        PEER,
        "POST",
        &format!("/v1/swarms/{swarm_id}/members"),
        Some(json!({
            "action": "update_state",
            "member": "author-1",
            "state": {
                "state": "paused_quota",
                "checkpoint_commit": commit_sha,
                "board_cursor": 3,
                "reason": "quota exhausted",
                "paused_ms": 1000
            },
            "budget_draw_delta": 20
        })),
    );
    assert_eq!(paused.status, 200, "{}", paused.body);

    let failover = fixture.call(
        COORDINATOR,
        "POST",
        &format!("/v1/swarms/{swarm_id}/failover"),
        Some(json!({
            "member": "author-1",
            "replacement_model": "claude-opus-4-6",
            "replacement_harness": "claude-code",
            "reason": "switched provider"
        })),
    );
    assert_eq!(failover.status, 200, "{}", failover.body);
    assert_eq!(text(&failover.body, "/member/model"), "claude-opus-4-6");
    assert_eq!(
        failover.body.pointer("/member/last_read_seq"),
        Some(&json!(3))
    );
    assert_eq!(
        text(&failover.body, "/post/candidate_commit"),
        commit_sha.as_str()
    );

    // Verifier approves -> ReadyForPublication.
    let approved = fixture.call(
        COORDINATOR,
        "POST",
        &format!("/v1/swarms/{swarm_id}/verdicts"),
        Some(json!({
            "verifier": "verifier-1",
            "author": "author-1",
            "commit": commit_sha,
            "verdict": "approve"
        })),
    );
    assert_eq!(approved.status, 201, "{}", approved.body);
    assert_eq!(
        text(&approved.body, "/outcome/outcome"),
        "ready_for_publication"
    );

    // Board read with query params & replayable mention graph with up_to_seq.
    let board = fixture.call(
        COORDINATOR,
        "GET",
        &format!("/v1/swarms/{swarm_id}/board?since=2&limit=10"),
        None,
    );
    assert_eq!(board.status, 200, "{}", board.body);
    assert!(
        board
            .body
            .pointer("/posts")
            .and_then(Value::as_array)
            .is_some_and(|posts| posts.len() >= 3)
    );

    let graph_early = fixture.call(
        COORDINATOR,
        "GET",
        &format!("/v1/swarms/{swarm_id}/graph?up_to_seq=2"),
        None,
    );
    assert_eq!(graph_early.status, 200, "{}", graph_early.body);
    assert_eq!(
        graph_early.body.pointer("/graph/up_to_seq"),
        Some(&json!(2))
    );
    let graph_after_handoff = fixture.call(
        COORDINATOR,
        "GET",
        &format!("/v1/swarms/{swarm_id}/graph?up_to_seq=3"),
        None,
    );
    assert_eq!(graph_after_handoff.status, 200);
    assert!(
        graph_after_handoff
            .body
            .pointer("/graph/edges")
            .and_then(Value::as_array)
            .is_some_and(|edges| edges.iter().any(|e| {
                text(e, "/from") == "author-1"
                    && text(e, "/to") == "verifier-1"
                    && e.pointer("/weight") == Some(&json!(1))
            }))
    );

    // Unsupported method -> 405.
    let bad_method = fixture.call(
        COORDINATOR,
        "DELETE",
        &format!("/v1/swarms/{swarm_id}"),
        None,
    );
    assert_eq!(bad_method.status, 405);
    Ok(())
}

#[test]
// r[verify kiln.fleet.swarm.surface.scenario.http_and_mcp]
fn mcp_swarm_tools_round_trip() -> TestResult {
    let fixture = fixture()?;
    let listeners = kiln_fleet::server::bind(&["127.0.0.1:0".parse()?])?;
    let address = listeners.first().ok_or("no listener")?.local_addr()?;
    let _threads = kiln_fleet::server::spawn(listeners, &fixture.service)?;
    let url = format!("http://{address}");
    let mut server = McpServer::new(ApiClient::new(&url)?, Some("antigravity".into()));

    let create = mcp_call(
        &mut server,
        1,
        "tools/call",
        json!({
            "name": "kiln_swarm_create",
            "arguments": {
                "title": "MCP Swarm",
                "allowed_scopes": ["OnixResearch/kiln crates/kiln-fleet"]
            }
        }),
    )?;
    assert_eq!(create.pointer("/result/isError"), Some(&json!(false)));
    let create_payload: Value = serde_json::from_str(text(&create, "/result/content/0/text"))?;
    let swarm_id = text(&create_payload, "/response/swarm/identity").to_owned();

    let spawn_author = mcp_call(
        &mut server,
        2,
        "tools/call",
        json!({
            "name": "kiln_swarm_spawn",
            "arguments": {
                "swarm": swarm_id,
                "name": "author",
                "role": "author",
                "model": "gemini-3.1-pro",
                "scope": "OnixResearch/kiln crates/kiln-fleet/src",
                "brief": "Implement API"
            }
        }),
    )?;
    assert_eq!(spawn_author.pointer("/result/isError"), Some(&json!(false)));

    let spawn_verifier = mcp_call(
        &mut server,
        3,
        "tools/call",
        json!({
            "name": "kiln_swarm_spawn",
            "arguments": {
                "swarm": swarm_id,
                "name": "verifier",
                "role": "verifier",
                "model": "claude-opus-4-6",
                "brief": "Review API"
            }
        }),
    )?;
    assert_eq!(
        spawn_verifier.pointer("/result/isError"),
        Some(&json!(false))
    );

    let post = mcp_call(
        &mut server,
        4,
        "tools/call",
        json!({
            "name": "kiln_swarm_post",
            "arguments": {
                "swarm": swarm_id,
                "sender": "author",
                "kind": "handoff",
                "mentions": [{ "kind": "member", "target": "verifier" }],
                "body": "Please audit"
            }
        }),
    )?;
    assert_eq!(post.pointer("/result/isError"), Some(&json!(false)));

    let read = mcp_call(
        &mut server,
        5,
        "tools/call",
        json!({
            "name": "kiln_swarm_read",
            "arguments": { "swarm": swarm_id, "since": 0, "member": "verifier" }
        }),
    )?;
    assert_eq!(read.pointer("/result/isError"), Some(&json!(false)));
    assert!(text(&read, "/result/content/0/text").contains("Please audit"));

    // Malformed/injected member in MCP kiln_swarm_read is rejected before hitting HTTP.
    let bad_member = mcp_call(
        &mut server,
        55,
        "tools/call",
        json!({
            "name": "kiln_swarm_read",
            "arguments": { "swarm": swarm_id, "member": "verifier&since=99\r\n" }
        }),
    )?;
    assert!(bad_member.get("error").is_some());

    let wd = mcp_call(
        &mut server,
        6,
        "tools/call",
        json!({
            "name": "kiln_swarm_watchdog",
            "arguments": { "swarm": swarm_id, "apply": false }
        }),
    )?;
    assert_eq!(wd.pointer("/result/isError"), Some(&json!(false)));

    let verdict = mcp_call(
        &mut server,
        7,
        "tools/call",
        json!({
            "name": "kiln_swarm_verdict",
            "arguments": {
                "swarm": swarm_id,
                "verifier": "verifier",
                "author": "author",
                "commit": "b".repeat(40),
                "verdict": "approve"
            }
        }),
    )?;
    assert_eq!(verdict.pointer("/result/isError"), Some(&json!(false)));

    let status = mcp_call(
        &mut server,
        8,
        "tools/call",
        json!({
            "name": "kiln_swarm_status",
            "arguments": { "swarm": swarm_id }
        }),
    )?;
    assert_eq!(status.pointer("/result/isError"), Some(&json!(false)));
    assert!(text(&status, "/result/content/0/text").contains("ready_for_publication"));

    let resource = mcp_call(
        &mut server,
        9,
        "resources/read",
        json!({ "uri": "kiln-fleet://swarms" }),
    )?;
    assert!(text(&resource, "/result/contents/0/text").contains("MCP Swarm"));
    Ok(())
}

#[test]
fn the_committed_positive_export_is_accepted() -> TestResult {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let committed = manifest.join("tests/fixtures/fleet.json");
    let config = FleetConfig::load(&committed)?;
    assert_eq!(config.machine, COORDINATOR);
    // When nickel is available, the committed export must equal a fresh one.
    let fixture = manifest.join("../../config/fixtures/fleet-positive.ncl");
    if let Ok(output) = std::process::Command::new("nickel")
        .args(["export", "--format", "json"])
        .arg(&fixture)
        .output()
        && output.status.success()
    {
        let fresh: Value = serde_json::from_slice(&output.stdout)?;
        let stored: Value = serde_json::from_slice(&std::fs::read(&committed)?)?;
        assert_eq!(fresh, stored, "tests/fixtures/fleet.json is stale");
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A fleet under the verified profile whose holder seed files do not exist:
/// a route that loads a holder seed cannot open the store.
fn verified_without_seeds() -> TestResult<Fixture> {
    use kiln_adapter_basalt::test_support;
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_enrollment(root)?;
    let proofs = root.join("proofs");
    std::fs::create_dir_all(&proofs)?;
    let feed = root.join("revocations.json");
    std::fs::write(
        &feed,
        json!({
            "schema": "kiln.implementation-authority-revocations.v1",
            "generation": 1,
            "observed_at_ms": kiln_fleet::log::now_ms(),
            "revoked": [],
        })
        .to_string(),
    )?;
    let issuer = test_support::did(test_support::ROOT_SEED);
    let authority = json!({
        "schema": "kiln.implementation-authority-config.v1",
        "profile": { "verified_ucan": {
            "audience": test_support::AUDIENCE,
            "generation": test_support::GENERATION,
            "policy": "packaged",
            "trusted_roots": [issuer],
            "keys": [{
                "did": issuer,
                "ed25519": hex(&test_support::public_key(test_support::ROOT_SEED)),
            }],
            "proofs_dir": proofs,
            "revocation": { "path": feed, "max_age_ms": 600_000 },
            "replay": { "retention_ms": 60_000 },
            "holders": {
                "coordinator": { "seed_path": root.join("absent-coordinator.seed") },
                "verifier": { "seed_path": root.join("absent-verifier.seed") },
                "publisher": { "seed_path": root.join("absent-publisher.seed") },
            },
            "projection_enabled": false,
        } },
    });
    std::fs::write(root.join("authority.json"), authority.to_string())?;
    let mut document = config(root, &FAKE_TAILSCALE);
    let object = document
        .as_object_mut()
        .ok_or("configuration is not an object")?;
    object.remove("authority_operator");
    object.insert(
        "authority_config".into(),
        json!(root.join("authority.json")),
    );
    let config = FleetConfig::from_json(&serde_json::to_vec(&document)?)?;
    let service = Arc::new(Service::new(config)?);
    Ok(Fixture {
        _directory: directory,
        service,
    })
}

#[test]
// r[verify kiln.implementation.authority.selection.scenario.scoped]
fn observation_routes_load_no_holder_seed() -> TestResult {
    let fixture = verified_without_seeds()?;
    for path in [
        "/v1/version",
        "/v1/claims",
        "/v1/work",
        "/v1/attempts",
        "/v1/decisions",
        "/v1/metrics",
    ] {
        let response = fixture.call(COORDINATOR, "GET", path, None);
        assert_eq!(response.status, 200, "{path}: {}", response.body);
    }
    // Cancel checks `attempt-cancel`, so it loads the coordinator seed, which
    // is absent: the store does not open and nothing is cancelled.
    let attempt = "a".repeat(64);
    let response = fixture.call(
        COORDINATOR,
        "POST",
        &format!("/v1/attempts/{attempt}/cancel"),
        Some(json!({ "reason": "operator stop" })),
    );
    assert_eq!(response.status, 500, "{}", response.body);
    Ok(())
}

#[test]
// r[verify kiln.fleet.api] r[verify kiln.fleet.dispatch]
fn attempts_and_metrics_expose_session_observation_age() -> TestResult {
    use kiln_core::fleet::{
        AgentStatus, DEFAULT_STALE_OBSERVATION_AFTER_MS, Observation, Placement, SessionWorkspace,
    };
    use kiln_core::implementation::{
        AcceptanceObligation, DEFAULT_LEASE_MS, IMPLEMENTATION_WORK_SCHEMA,
        INTERACTIVE_AGENT_EFFECT, ImplementationWork, SourceRef, TerminalPolicy, WorkBudget,
        WorkSourceKind,
    };
    use kiln_core::types::{ContentId, ExternalRef};
    use std::collections::BTreeSet;

    let fixture = fixture()?;
    let now: u64 = 10_000_000_000;
    let mut store = fixture.service.store.open_coordinator()?;
    let work = ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_owned(),
        revision: 1,
        actor: common::ACTOR.to_owned(),
        repository: common::REPOSITORY.to_owned(),
        project: "kiln".to_owned(),
        base: Some(
            ExternalRef::new("git", "commit", &"1".repeat(40))
                .map_err(|error| format!("{error:?}"))?,
        ),
        title: "Stale observation test".to_owned(),
        requested_outcome: "implement".to_owned(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_owned(),
            statement: "verified".to_owned(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".to_owned()],
        permitted_effects: BTreeSet::from([
            "write_source".to_owned(),
            INTERACTIVE_AGENT_EFFECT.to_owned(),
        ]),
        verification_profile: ContentId::from_blake3_hex("9".repeat(64))
            .map_err(|error| format!("{error:?}"))?,
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "OnixResearch/kiln#105")
                .map_err(|error| format!("{error:?}"))?,
            native_id: "OnixResearch/kiln#105".to_owned(),
            version: ContentId::from_blake3_hex("2".repeat(64))
                .map_err(|error| format!("{error:?}"))?,
        }],
    };
    store.admit(&work, None)?;
    let attempt = store
        .claim_next(now, DEFAULT_LEASE_MS)?
        .ok_or("work not claimed")?;
    let session = store.fleet_bind_session(
        Placement {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            work: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            machine: PEER.to_owned(),
            kind: "omp".to_owned(),
        },
        now,
    )?;
    store.fleet_observe(
        &session.identity,
        Observation::Started {
            agent_name: "kiln-omp-1".to_owned(),
            workspace: SessionWorkspace::new(
                "kiln-1",
                "pane-1",
                "/srv/worktrees/attempt",
                "kiln/attempt",
                &"a".repeat(40),
            )
            .map_err(|error| format!("{error:?}"))?,
        },
        now,
    )?;
    store.fleet_observe(&session.identity, Observation::Prompted, now)?;
    store.fleet_observe(
        &session.identity,
        Observation::Status {
            status: AgentStatus::Working,
        },
        now,
    )?;
    drop(store);

    let stale_now = now + DEFAULT_STALE_OBSERVATION_AFTER_MS + 60_000;
    let attempts_response = fixture.service.route(
        COORDINATOR,
        &request("GET", "/v1/attempts", None),
        stale_now,
    );
    assert_eq!(attempts_response.status, 200);
    assert_eq!(
        attempts_response
            .body
            .pointer("/attempts/0/observation_age_ms"),
        Some(&json!(DEFAULT_STALE_OBSERVATION_AFTER_MS + 60_000))
    );
    assert_eq!(
        attempts_response
            .body
            .pointer("/attempts/0/observation_stale"),
        Some(&json!(true))
    );

    let metrics_response =
        fixture
            .service
            .route(COORDINATOR, &request("GET", "/v1/metrics", None), stale_now);
    assert_eq!(metrics_response.status, 200);
    let metrics_text = metrics_response.text.as_deref().unwrap_or_default();
    assert!(
        metrics_text.contains(&format!(
            "kiln_fleet_session_max_observation_age_ms {}",
            DEFAULT_STALE_OBSERVATION_AFTER_MS + 60_000
        )),
        "{metrics_text}"
    );
    assert!(
        metrics_text.contains("kiln_fleet_sessions_stale 1"),
        "{metrics_text}"
    );

    let ui_response = fixture
        .service
        .route(COORDINATOR, &request("GET", "/ui", None), stale_now);
    assert_eq!(ui_response.status, 200);
    let ui_html = ui_response.html.as_deref().unwrap_or_default();
    assert!(ui_html.contains("observation_age_ms"));
    assert!(ui_html.contains("observation_stale"));
    Ok(())
}
