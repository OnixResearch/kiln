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

fn request(method: &str, path: &str, body: Option<Value>) -> Request {
    Request {
        method: method.to_owned(),
        path: path.to_owned(),
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
            "kiln_decisions"
        ]
    );
    assert!(
        tools
            .iter()
            .all(|tool| text(tool, "/description").contains("untrusted data"))
    );

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
