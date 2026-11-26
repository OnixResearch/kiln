#![cfg(unix)]
//! Fleet API routing, authentication, MCP and the TCP server against a
//! temporary store and a scripted `tailscale`.

mod common;

use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use common::{COORDINATOR, PEER, TestResult, config, executable, write_enrollment};
use kiln_core::implementation::{SourceRef, WorkSourceKind};
use kiln_core::types::ExternalRef;
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
// r[verify kiln.fleet.unified_console_surface]
// r[verify kiln.fleet.unified_console_surface.scenario.combined]
fn serves_the_unified_console_pointer_only_to_authenticated_peers() -> TestResult {
    let ip = |text: &str| text.parse::<IpAddr>().unwrap_or_else(|_| unreachable!());
    let fixture = fixture()?;
    let page = fixture
        .service
        .handle(ip("127.0.0.1"), &request("GET", "/", None));
    assert_eq!(page.status, 200);
    assert!(page.location.is_none());
    let document = page.html.as_deref().unwrap_or_default();
    assert!(document.contains("Factory Control Plane"));
    assert!(document.contains("/?page=fleet"));
    assert!(document.contains("console_url"));
    assert!(!document.contains("<script"));
    assert!(!document.contains("prompt("));
    assert!(!document.contains("alert("));
    assert!(!document.contains("setInterval"));
    let mut bytes = Vec::new();
    kiln_fleet::http::write_response(&mut bytes, &page)?;
    let head = String::from_utf8_lossy(&bytes);
    assert!(head.starts_with("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n"));
    assert!(head.contains("Content-Security-Policy:"));

    let alias = fixture
        .service
        .handle(ip("127.0.0.1"), &request("GET", "/ui", None));
    assert_eq!(alias.status, 200);
    assert_eq!(alias.html, page.html);

    let wrong_method = fixture
        .service
        .handle(ip("127.0.0.1"), &request("POST", "/", None));
    assert_eq!(wrong_method.status, 405);

    let refused = fixture
        .service
        .handle(ip("100.100.1.2"), &request("GET", "/", None));
    assert_eq!(refused.status, 403);
    Ok(())
}

#[test]
// r[verify kiln.fleet.unified_console_surface]
// r[verify kiln.fleet.unified_console_surface.scenario.combined]
fn configured_console_url_redirects_root_and_ui_to_unified_fleet_page() -> TestResult {
    let ip = |text: &str| text.parse::<IpAddr>().unwrap_or_else(|_| unreachable!());
    let directory = tempfile::tempdir()?;
    write_enrollment(directory.path())?;
    let mut document = config(directory.path(), &FAKE_TAILSCALE);
    if let Some(object) = document.as_object_mut() {
        object.insert(
            "console_url".into(),
            json!("https://britton-desktop.bison-tailor.ts.net:9443/"),
        );
    }
    let parsed = FleetConfig::from_json(&serde_json::to_vec(&document)?)?;
    let service = Service::new(parsed)?;

    for path in ["/", "/ui"] {
        let response = service.handle(ip("127.0.0.1"), &request("GET", path, None));
        assert_eq!(response.status, 303, "{path}");
        assert_eq!(
            response.location.as_deref(),
            Some("https://britton-desktop.bison-tailor.ts.net:9443/?page=fleet"),
            "{path}"
        );
        let html = response.html.as_deref().unwrap_or_default();
        assert!(html.contains("https://britton-desktop.bison-tailor.ts.net:9443/?page=fleet"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("prompt("));
        assert!(!html.contains("alert("));

        let mut wire = Vec::new();
        kiln_fleet::http::write_response(&mut wire, &response)?;
        let wire_text = String::from_utf8_lossy(&wire);
        assert!(wire_text.starts_with("HTTP/1.1 303 See Other\r\n"));
        assert!(wire_text.contains(
            "Location: https://britton-desktop.bison-tailor.ts.net:9443/?page=fleet\r\n"
        ));
    }

    // Unauthenticated peers remain rejected before any redirect is issued.
    let refused = service.handle(ip("100.100.1.2"), &request("GET", "/ui", None));
    assert_eq!(refused.status, 403);
    assert!(refused.location.is_none());

    // Malformed or injection-bearing console_url values fail closed at config and service init.
    for invalid in [
        "javascript:alert(1)",
        "https://console.example\r\nX-Injected: 1",
        "https://",
        "https://console.example/?x=1",
        "https://console.example/#frag",
    ] {
        let mut bad_doc = config(directory.path(), &FAKE_TAILSCALE);
        if let Some(object) = bad_doc.as_object_mut() {
            object.insert("console_url".into(), json!(invalid));
        }
        assert!(
            FleetConfig::from_json(&serde_json::to_vec(&bad_doc)?).is_err(),
            "{invalid:?}"
        );
        let mut bad_cfg = FleetConfig::from_json(&serde_json::to_vec(&config(
            directory.path(),
            &FAKE_TAILSCALE,
        ))?)?;
        bad_cfg.console_url = Some(invalid.to_owned());
        assert!(Service::new(bad_cfg).is_err(), "{invalid:?}");
    }
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
    assert_eq!(fixture.call(PEER, "POST", "/v1/machines", None).status, 405);
    for path in ["/v1/machines", "/v1/work", "/v1/attempts", "/v1/decisions"] {
        assert_eq!(fixture.call(PEER, "GET", path, None).status, 200, "{path}");
    }
    Ok(())
}

#[test]
fn work_rows_preserve_exact_native_source_references() -> TestResult {
    let fixture = fixture()?;
    let profile = common::content('b');
    let ready = common::work(&"a".repeat(40), &profile, &["src"])?;
    let mut blocked = ready.clone();
    // Identical titles aren't evidence of a source relationship.
    blocked.dependencies = vec![common::content('c')];
    blocked.aliases = vec![SourceRef {
        kind: WorkSourceKind::CairnTask,
        source: ExternalRef::new("cairn", "task", "change/task-1")
            .map_err(|error| error.message)?,
        native_id: "change/task-1".into(),
        version: common::content('2'),
    }];
    let mut store = fixture.service.store.open_coordinator()?;
    store.admit(&ready, None)?;
    store.admit(&blocked, None)?;

    let response = fixture.call(PEER, "GET", "/v1/work", None);
    assert_eq!(response.status, 200);
    let body = response.body;
    assert_eq!(body.pointer("/truncated"), Some(&json!(false)));
    assert_eq!(body.pointer("/ranked/0/repository"), Some(&json!(common::REPOSITORY)));
    assert_eq!(body.pointer("/blocked/0/repository"), Some(&json!(common::REPOSITORY)));
    assert_eq!(body.pointer("/ranked/0/sources"), Some(&json!([{
        "kind": "github_issue",
        "source": { "contract_version": 1, "provider_namespace": "github", "native_kind": "issue", "native_id": "onix/kiln#7" },
        "native_id": "onix/kiln#7",
        "version": "1".repeat(64),
    }])));
    assert_eq!(body.pointer("/blocked/0/sources"), Some(&json!([{
        "kind": "cairn_task",
        "source": { "contract_version": 1, "provider_namespace": "cairn", "native_kind": "task", "native_id": "change/task-1" },
        "native_id": "change/task-1",
        "version": "2".repeat(64),
    }])));
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
    assert!(ui_html.contains("Factory Control Plane"));
    assert!(ui_html.contains("/?page=fleet"));
    assert!(!ui_html.contains("<script"));
    Ok(())
}

#[derive(Clone, Debug)]
struct RecordedHoldRequest {
    at_ms: u64,
    method: String,
    path: String,
    body: Option<Value>,
}

struct FakeHoldClock {
    now_ms: Arc<std::sync::atomic::AtomicU64>,
    sleeps: Arc<std::sync::atomic::AtomicU64>,
}

impl FakeHoldClock {
    fn new(t0: u64) -> Self {
        Self {
            now_ms: Arc::new(std::sync::atomic::AtomicU64::new(t0)),
            sleeps: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    fn handle(&self) -> Self {
        Self {
            now_ms: Arc::clone(&self.now_ms),
            sleeps: Arc::clone(&self.sleeps),
        }
    }

    fn wait_sleep_advance(&self) -> TestResult {
        let baseline = self.sleeps.load(std::sync::atomic::Ordering::SeqCst);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if self.sleeps.load(std::sync::atomic::Ordering::SeqCst) > baseline {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Err("timed out waiting for hold loop sleep tick".into())
    }
}

impl kiln_fleet::hold::HoldClock for FakeHoldClock {
    fn now_ms(&self) -> u64 {
        self.now_ms.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn sleep_ms(&self, _ms: u64) {
        self.sleeps
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProxyMode {
    Forward,
    DropConnection,
    Return503,
    AdvanceClockOnRenew503(u64),
    AdvanceClockOnRenewDrop(u64),
}

type RecordedRequests = Arc<std::sync::Mutex<Vec<RecordedHoldRequest>>>;

fn spawn_recording_proxy(
    service: Arc<Service>,
    clock_ms: Arc<std::sync::atomic::AtomicU64>,
    mode: Arc<std::sync::Mutex<ProxyMode>>,
) -> TestResult<(String, RecordedRequests, Arc<std::sync::atomic::AtomicBool>)> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let log_thread = Arc::clone(&log);
    let stop_thread = Arc::clone(&stop);
    std::thread::spawn(move || {
        while !stop_thread.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
                    let current_mode = mode
                        .lock()
                        .map(|guard| *guard)
                        .unwrap_or(ProxyMode::Forward);
                    if current_mode == ProxyMode::DropConnection {
                        drop(stream);
                        continue;
                    }
                    let Ok(req) = kiln_fleet::http::read_request(&mut stream) else {
                        continue;
                    };
                    let body = if req.body.is_empty() {
                        None
                    } else {
                        serde_json::from_slice::<Value>(&req.body).ok()
                    };
                    let mut now_ms = clock_ms.load(std::sync::atomic::Ordering::SeqCst);
                    if req.path.ends_with("/renew") {
                        match current_mode {
                            ProxyMode::AdvanceClockOnRenew503(advanced_ms)
                            | ProxyMode::AdvanceClockOnRenewDrop(advanced_ms) => {
                                clock_ms.store(advanced_ms, std::sync::atomic::Ordering::SeqCst);
                                now_ms = advanced_ms;
                            }
                            ProxyMode::Forward
                            | ProxyMode::DropConnection
                            | ProxyMode::Return503 => {}
                        }
                    }
                    if let Ok(mut guard) = log_thread.lock() {
                        guard.push(RecordedHoldRequest {
                            at_ms: now_ms,
                            method: req.method.clone(),
                            path: req.path.clone(),
                            body,
                        });
                    }
                    match current_mode {
                        ProxyMode::AdvanceClockOnRenewDrop(_) if req.path.ends_with("/renew") => {
                            drop(stream);
                        }
                        ProxyMode::Return503 | ProxyMode::AdvanceClockOnRenew503(_) => {
                            let response =
                                Response::error(503, "fleet_unavailable", "proxy simulated 503");
                            let _ = kiln_fleet::http::write_response(&mut stream, &response);
                        }
                        ProxyMode::Forward
                        | ProxyMode::DropConnection
                        | ProxyMode::AdvanceClockOnRenewDrop(_) => {
                            let response = service.route(COORDINATOR, &req, now_ms);
                            let _ = kiln_fleet::http::write_response(&mut stream, &response);
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });
    Ok((format!("http://{addr}"), log, stop))
}

fn wait_for_recorded<F>(
    log: &Arc<std::sync::Mutex<Vec<RecordedHoldRequest>>>,
    predicate: F,
) -> TestResult
where
    F: Fn(&[RecordedHoldRequest]) -> bool,
{
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Ok(guard) = log.lock()
            && predicate(&guard)
        {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Err("timed out waiting for recorded hold request".into())
}

fn wait_for_path(path: &Path) -> TestResult {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Err(format!("timed out waiting for {}", path.display()).into())
}

fn wait_for_pid_file(path: &Path) -> TestResult<i32> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(path)
            && !text.trim().is_empty()
        {
            return Ok(text.trim().parse::<i32>()?);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Err(format!("timed out waiting for pid file {}", path.display()).into())
}

fn assert_pid_dead(raw_pid: i32) -> TestResult {
    let pid = nix::unistd::Pid::from_raw(raw_pid);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if matches!(
            nix::sys::signal::kill(pid, None),
            Err(nix::errno::Errno::ESRCH)
        ) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    Err(format!("process {raw_pid} is still alive").into())
}

#[test]
fn hold_fake_clock_renews_within_one_third_lease_and_tracks_fence() -> TestResult {
    use kiln_core::fleet::{ClaimEffect, ClaimState, MirrorStatus};
    use kiln_fleet::hold::{HoldSpec, run_with};

    // Part A: real loopback Service + FakeHoldClock with 3-minute lease (renew <= 60_000 ms).
    let fixture = fixture()?;
    let directory = tempfile::tempdir()?;
    let stop_file = directory.path().join("stop-part-a");
    let t0: u64 = 10_000_000_000;
    let fake_clock = FakeHoldClock::new(t0);
    let mode = Arc::new(std::sync::Mutex::new(ProxyMode::Forward));
    let (url, recorded, stop_proxy) = spawn_recording_proxy(
        Arc::clone(&fixture.service),
        Arc::clone(&fake_clock.now_ms),
        mode,
    )?;

    let client = ApiClient::new(&url)?;
    let spec = HoldSpec {
        scope: "OnixResearch/kiln crates/kiln-fleet".to_owned(),
        tool: "codex".to_owned(),
        session: "fake-clock-renew".to_owned(),
        purpose: "verify 1/3 lease renewal".to_owned(),
        lease_minutes: 3,
        command: vec![
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            "while [ ! -f \"$1\" ]; do sleep 0.02; done".to_owned(),
            "_".to_owned(),
            stop_file.to_string_lossy().into_owned(),
        ],
    };
    let clock_thread = fake_clock.handle();
    let hold_handle = std::thread::spawn(move || {
        let signals = kiln_fleet::hold::HoldSignals::new();
        run_with(&client, &spec, &clock_thread, &signals)
    });

    wait_for_recorded(&recorded, |reqs| {
        reqs.iter()
            .any(|req| req.method == "POST" && req.path == "/v1/claims")
    })?;
    fake_clock.wait_sleep_advance()?;

    let state = fixture.service.store.open_coordinator()?.fleet_state()?;
    let claim = state
        .claims
        .claims
        .values()
        .next()
        .ok_or("missing claim")?
        .clone();
    assert_eq!(claim.attempt, None);
    assert_eq!(claim.expires_ms, t0 + 180_000);
    let initial_fence = claim.fence;

    // Advance fake clock to exactly 1/3 of 3 minutes (60_000 ms).
    fake_clock
        .now_ms
        .store(t0 + 60_000, std::sync::atomic::Ordering::SeqCst);
    wait_for_recorded(&recorded, |reqs| {
        reqs.iter()
            .filter(|req| req.path.ends_with("/renew"))
            .count()
            >= 1
    })?;
    fake_clock.wait_sleep_advance()?;

    // Advance another 60_000 ms for a second renewal.
    fake_clock
        .now_ms
        .store(t0 + 120_000, std::sync::atomic::Ordering::SeqCst);
    wait_for_recorded(&recorded, |reqs| {
        reqs.iter()
            .filter(|req| req.path.ends_with("/renew"))
            .count()
            >= 2
    })?;
    fake_clock.wait_sleep_advance()?;

    std::fs::write(&stop_file, b"done")?;
    let exit_code = hold_handle.join().map_err(|_| "hold thread panicked")?;
    assert_eq!(exit_code, 0);
    stop_proxy.store(true, std::sync::atomic::Ordering::SeqCst);

    let reqs = recorded.lock().map_err(|_| "lock poisoned")?.clone();
    let renews: Vec<_> = reqs
        .iter()
        .filter(|req| req.path.ends_with("/renew"))
        .collect();
    assert!(renews.len() >= 2);
    let renew1 = renews.first().ok_or("missing renew 1")?;
    assert!(renew1.at_ms <= t0 + 60_000);
    assert_eq!(
        renew1
            .body
            .as_ref()
            .and_then(|body| body.get("lease_minutes")),
        Some(&json!(3))
    );
    assert_eq!(
        renew1.body.as_ref().and_then(|body| body.get("fence")),
        Some(&json!(initial_fence))
    );
    let releases: Vec<_> = reqs
        .iter()
        .filter(|req| req.path.ends_with("/release"))
        .collect();
    assert_eq!(releases.len(), 1);
    assert_eq!(
        releases
            .first()
            .and_then(|req| req.body.as_ref())
            .and_then(|body| body.get("note")),
        Some(&json!("exit 0"))
    );

    let mut store = fixture.service.store.open_coordinator()?;
    let final_state = store.fleet_state()?;
    let final_claim = final_state
        .claims
        .claims
        .get(&claim.identity)
        .ok_or("missing final claim")?;
    assert!(matches!(
        &final_claim.state,
        ClaimState::Released { note, .. } if note == "exit 0"
    ));
    assert_eq!(final_claim.release_mirror, MirrorStatus::Pending);
    let pending = store.fleet_pending_mirror(COORDINATOR)?;
    assert_eq!(
        pending
            .iter()
            .filter(|effect| matches!(effect, ClaimEffect::PostRelease { identity, .. } if identity == &claim.identity))
            .count(),
        1
    );

    // Part B: scripted fence-bumping coordinator (fence 1 -> 2 -> 3) verifies hold
    // tracks each updated fence returned by grant and renew.
    let stop_file_b = directory.path().join("stop-part-b");
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let scripted_addr = listener.local_addr()?;
    let scripted_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let scripted_stop_thread = Arc::clone(&scripted_stop);
    let scripted_log = Arc::new(std::sync::Mutex::new(Vec::<(String, Option<u64>)>::new()));
    let scripted_log_thread = Arc::clone(&scripted_log);
    let claim_id = "a".repeat(64);
    let claim_id_thread = claim_id.clone();

    std::thread::spawn(move || {
        let mut renew_count: u64 = 0;
        while !scripted_stop_thread.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let Ok(req) = kiln_fleet::http::read_request(&mut stream) else {
                        continue;
                    };
                    let body = serde_json::from_slice::<Value>(&req.body).ok();
                    let sent_fence = body
                        .as_ref()
                        .and_then(|v| v.get("fence"))
                        .and_then(Value::as_u64);
                    let response = if req.path == "/v1/claims" {
                        Response::new(
                            201,
                            json!({
                                "outcome": "granted",
                                "claim": {
                                    "identity": claim_id_thread,
                                    "fence": 1,
                                    "granted_ms": t0,
                                    "expires_ms": t0 + 180_000,
                                }
                            }),
                        )
                    } else if req.path.ends_with("/renew") {
                        renew_count += 1;
                        let next_fence = renew_count + 1;
                        Response::new(
                            200,
                            json!({
                                "outcome": "renewed",
                                "claim": {
                                    "identity": claim_id_thread,
                                    "fence": next_fence,
                                    "expires_ms": t0 + (renew_count + 3) * 60_000,
                                }
                            }),
                        )
                    } else {
                        Response::new(200, json!({ "outcome": "released" }))
                    };
                    let _ = kiln_fleet::http::write_response(&mut stream, &response);
                    if let Ok(mut guard) = scripted_log_thread.lock() {
                        guard.push((req.path.clone(), sent_fence));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });

    let clock_b = FakeHoldClock::new(t0);
    let client_b = ApiClient::new(&format!("http://{scripted_addr}"))?;
    let spec_b = HoldSpec {
        scope: "OnixResearch/kiln docs".to_owned(),
        tool: "codex".to_owned(),
        session: "fence-bump".to_owned(),
        purpose: "track bumped fence".to_owned(),
        lease_minutes: 3,
        command: vec![
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            "while [ ! -f \"$1\" ]; do sleep 0.02; done".to_owned(),
            "_".to_owned(),
            stop_file_b.to_string_lossy().into_owned(),
        ],
    };
    let clock_b_thread = clock_b.handle();
    let handle_b = std::thread::spawn(move || {
        let signals = kiln_fleet::hold::HoldSignals::new();
        run_with(&client_b, &spec_b, &clock_b_thread, &signals)
    });

    let wait_scripted = |count: usize| -> TestResult {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Ok(guard) = scripted_log.lock()
                && guard.len() >= count
            {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Err("timed out waiting for scripted request".into())
    };
    wait_scripted(1)?;
    clock_b.wait_sleep_advance()?;
    clock_b
        .now_ms
        .store(t0 + 60_000, std::sync::atomic::Ordering::SeqCst);
    wait_scripted(2)?;
    clock_b.wait_sleep_advance()?;
    clock_b
        .now_ms
        .store(t0 + 120_000, std::sync::atomic::Ordering::SeqCst);
    wait_scripted(3)?;
    clock_b.wait_sleep_advance()?;
    std::fs::write(&stop_file_b, b"done")?;
    assert_eq!(handle_b.join().map_err(|_| "panic")?, 0);
    scripted_stop.store(true, std::sync::atomic::Ordering::SeqCst);

    let fences: Vec<_> = scripted_log.lock().map_err(|_| "lock poisoned")?.clone();
    assert_eq!(
        fences,
        vec![
            ("/v1/claims".to_owned(), None),
            (format!("/v1/claims/{claim_id}/renew"), Some(1)),
            (format!("/v1/claims/{claim_id}/renew"), Some(2)),
            (format!("/v1/claims/{claim_id}/release"), Some(3)),
        ]
    );
    Ok(())
}

#[test]
fn hold_stale_fence_or_expired_renew_kills_child_and_exits_one_without_release() -> TestResult {
    use kiln_core::fleet::ClaimState;
    use kiln_fleet::hold::{HoldSpec, run_with};

    // Case A: claim externally released before renew -> 409 stale -> kills child, exits 1, sends NO release.
    {
        let fixture = fixture()?;
        let directory = tempfile::tempdir()?;
        let marker = directory.path().join("ready-a");
        let pid_file = directory.path().join("child-a.pid");
        let t0: u64 = 10_000_000_000;
        let fake_clock = FakeHoldClock::new(t0);
        let mode = Arc::new(std::sync::Mutex::new(ProxyMode::Forward));
        let (url, recorded, stop_proxy) = spawn_recording_proxy(
            Arc::clone(&fixture.service),
            Arc::clone(&fake_clock.now_ms),
            mode,
        )?;

        let client = ApiClient::new(&url)?;
        let spec = HoldSpec {
            scope: "OnixResearch/kiln crates/kiln-fleet".to_owned(),
            tool: "codex".to_owned(),
            session: "stale-fence".to_owned(),
            purpose: "stale fence stop".to_owned(),
            lease_minutes: 3,
            command: vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "echo $$ > \"$1\"; touch \"$2\"; sleep 60".to_owned(),
                "_".to_owned(),
                pid_file.to_string_lossy().into_owned(),
                marker.to_string_lossy().into_owned(),
            ],
        };
        let clock_thread = fake_clock.handle();
        let handle = std::thread::spawn(move || {
            let signals = kiln_fleet::hold::HoldSignals::new();
            run_with(&client, &spec, &clock_thread, &signals)
        });

        wait_for_path(&marker)?;
        let child_pid = wait_for_pid_file(&pid_file)?;
        fake_clock.wait_sleep_advance()?;
        let state = fixture.service.store.open_coordinator()?.fleet_state()?;
        let claim = state
            .claims
            .claims
            .values()
            .next()
            .ok_or("missing claim")?
            .clone();

        // Operator externally releases the claim, making hold's next renew 409 stale.
        let external_release = fixture.service.route(
            COORDINATOR,
            &request(
                "POST",
                &format!("/v1/claims/{}/release", claim.identity.as_str()),
                Some(json!({ "fence": claim.fence, "note": "operator revoked" })),
            ),
            t0 + 1_000,
        );
        assert_eq!(external_release.status, 200);

        fake_clock
            .now_ms
            .store(t0 + 60_000, std::sync::atomic::Ordering::SeqCst);
        let exit_code = handle.join().map_err(|_| "hold panicked")?;
        stop_proxy.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(exit_code, 1);
        assert_pid_dead(child_pid)?;

        let reqs = recorded.lock().map_err(|_| "lock poisoned")?.clone();
        assert_eq!(
            reqs.iter().filter(|r| r.path.ends_with("/renew")).count(),
            1
        );
        assert_eq!(
            reqs.iter().filter(|r| r.path.ends_with("/release")).count(),
            0
        );
        let final_state = fixture.service.store.open_coordinator()?.fleet_state()?;
        let final_claim = final_state
            .claims
            .claims
            .get(&claim.identity)
            .ok_or("missing claim")?;
        assert!(matches!(
            &final_claim.state,
            ClaimState::Released { note, .. } if note == "operator revoked"
        ));
    }

    // Case B: claim lease expired on coordinator -> 409 expired on renew -> kills child, exits 1, no release.
    {
        let fixture = fixture()?;
        let directory = tempfile::tempdir()?;
        let marker = directory.path().join("ready-b");
        let pid_file = directory.path().join("child-b.pid");
        let t0: u64 = 10_000_000_000;
        let fake_clock = FakeHoldClock::new(t0);
        let mode = Arc::new(std::sync::Mutex::new(ProxyMode::Forward));
        let (url, recorded, stop_proxy) = spawn_recording_proxy(
            Arc::clone(&fixture.service),
            Arc::clone(&fake_clock.now_ms),
            mode,
        )?;

        let client = ApiClient::new(&url)?;
        let spec = HoldSpec {
            scope: "OnixResearch/kiln docs".to_owned(),
            tool: "codex".to_owned(),
            session: "expired-renew".to_owned(),
            purpose: "expired renew stop".to_owned(),
            lease_minutes: 3,
            command: vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "echo $$ > \"$1\"; touch \"$2\"; sleep 60".to_owned(),
                "_".to_owned(),
                pid_file.to_string_lossy().into_owned(),
                marker.to_string_lossy().into_owned(),
            ],
        };
        let clock_thread = fake_clock.handle();
        let handle = std::thread::spawn(move || {
            let signals = kiln_fleet::hold::HoldSignals::new();
            run_with(&client, &spec, &clock_thread, &signals)
        });

        wait_for_path(&marker)?;
        let child_pid = wait_for_pid_file(&pid_file)?;
        fake_clock.wait_sleep_advance()?;
        // Jump past the 3-minute lease expiry (180_000 ms).
        fake_clock
            .now_ms
            .store(t0 + 180_001, std::sync::atomic::Ordering::SeqCst);
        let exit_code = handle.join().map_err(|_| "hold panicked")?;
        stop_proxy.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(exit_code, 1);
        assert_pid_dead(child_pid)?;

        let reqs = recorded.lock().map_err(|_| "lock poisoned")?.clone();
        assert_eq!(
            reqs.iter().filter(|r| r.path.ends_with("/release")).count(),
            0
        );
    }

    Ok(())
}

#[test]
fn hold_and_dispatched_attempt_claims_remain_independent() -> TestResult {
    use kiln_core::fleet::{
        AgentStatus, ClaimCommand, ClaimState, FleetAgent, Observation, Placement,
        SessionWorkspace, parse_scope_text,
    };
    use kiln_core::implementation::{
        AcceptanceObligation, IMPLEMENTATION_WORK_SCHEMA, INTERACTIVE_AGENT_EFFECT,
        ImplementationWork, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
    };
    use kiln_core::types::{ContentId, ExternalRef};
    use kiln_fleet::dispatch::Dispatcher;
    use kiln_fleet::hold::{HoldSpec, run_with};
    use std::collections::BTreeSet;

    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_enrollment(root)?;
    let state_dir = root.join("herdr-state");
    std::fs::create_dir_all(&state_dir)?;
    let herdr_script = executable(
        "herdr-independence",
        r#"#!/bin/sh
state="$HOME"
shift 2
group="$1"; op="$2"; shift 2
case "$group-$op" in
  agent-list)
    if [ -f "$state/live" ]; then
      printf '{"id":"cli","result":{"type":"agent_list","agents":[{"agent":"omp","agent_status":"working","name":"kiln-omp-1","pane_id":"w1:p1","workspace_id":"w1"}]}}\n'
    else
      printf '{"id":"cli","result":{"type":"agent_list","agents":[]}}\n'
    fi ;;
  agent-get)
    if [ -f "$state/live" ]; then
      printf '{"id":"cli","result":{"type":"agent_info","agent":{"agent":"omp","agent_status":"working","name":"kiln-omp-1","pane_id":"w1:p1","workspace_id":"w1"}}}\n'
    else
      echo '{"id":"cli","error":{"code":"agent_not_found","message":"gone"}}' >&2; exit 1
    fi ;;
  workspace-close) printf '{"id":"cli","result":{"type":"ok"}}\n' ;;
  *) printf '{"id":"cli","result":{"type":"ok"}}\n' ;;
esac
"#,
    );
    let find_program = |name: &str| -> TestResult<PathBuf> {
        let path = std::env::var_os("PATH").ok_or("PATH is unset")?;
        std::env::split_paths(&path)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| format!("{name} not found on PATH").into())
    };
    let worktrees = root.join("worktrees");
    let exports = root.join("exports");
    let profiles = root.join("profiles");
    let repo_checkout = root.join("checkout");
    for dir in [&worktrees, &exports, &profiles, &repo_checkout] {
        std::fs::create_dir_all(dir)?;
    }

    let inventory = json!({
        "schema": "kiln.fleet-inventory.v1",
        "executable": herdr_script,
        "session": "kiln-indep",
        "environment": { "home": state_dir, "path": "/bin:/usr/bin" },
        "timeout_ms": 5_000,
        "start_timeout_ms": 1_000,
        "machines": [{
            "label": COORDINATOR,
            "worktree_root": worktrees,
            "repositories": { common::REPOSITORY: repo_checkout },
            "agents": [{ "kind": "omp", "slots": 1 }]
        }]
    });
    std::fs::write(root.join("inventory.json"), serde_json::to_vec(&inventory)?)?;
    std::fs::write(state_dir.join("live"), b"1")?;

    let mut document = config(root, &FAKE_TAILSCALE);
    if let Some(obj) = document.as_object_mut() {
        obj.insert(
            "capture".into(),
            json!({
                "git": find_program("git")?,
                "tar": find_program("tar")?,
                "export_root": exports,
                "verification_profiles": profiles,
                "repositories": {},
            }),
        );
        obj.insert(
            "dispatch".into(),
            json!({ "enabled": true, "kinds": ["omp"] }),
        );
    }
    let service = Arc::new(Service::new(FleetConfig::from_json(&serde_json::to_vec(
        &document,
    )?)?)?);
    let mut dispatcher = Dispatcher::new(Arc::clone(&service))?;

    let t0: u64 = 10_000_000_000;
    let mut store = service.store.open_coordinator()?;
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
        title: "Independence test".to_owned(),
        requested_outcome: "implement".to_owned(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_owned(),
            statement: "verified".to_owned(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["src".to_owned()],
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
            source: ExternalRef::new("github", "issue", "OnixResearch/kiln#104")
                .map_err(|error| format!("{error:?}"))?,
            native_id: "OnixResearch/kiln#104".to_owned(),
            version: ContentId::from_blake3_hex("2".repeat(64))
                .map_err(|error| format!("{error:?}"))?,
        }],
    };
    store.admit(&work, None)?;
    // Short 2-minute attempt lease so renewal is immediately due at t0 + 30_000.
    let attempt = store.claim_next(t0, 120_000)?.ok_or("work not claimed")?;
    let session = store.fleet_bind_session(
        Placement {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            work: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            machine: COORDINATOR.to_owned(),
            kind: "omp".to_owned(),
        },
        t0,
    )?;
    store.fleet_observe(
        &session.identity,
        Observation::Started {
            agent_name: "kiln-omp-1".to_owned(),
            workspace: SessionWorkspace::new(
                "w1",
                "w1:p1",
                "/tmp/wt",
                "kiln/attempt",
                &"a".repeat(40),
            )
            .map_err(|error| format!("{error:?}"))?,
        },
        t0,
    )?;
    store.fleet_observe(&session.identity, Observation::Prompted, t0)?;
    store.fleet_observe(
        &session.identity,
        Observation::Status {
            status: AgentStatus::Working,
        },
        t0,
    )?;
    store.fleet_claim(
        ClaimCommand::Acquire {
            scope: parse_scope_text("OnixResearch/kiln src")
                .map_err(|error| format!("{error:?}"))?,
            holder: FleetAgent {
                tool: "kiln".to_owned(),
                host: COORDINATOR.to_owned(),
                session: session.identity.as_str().to_owned(),
            },
            purpose: "dispatched attempt".to_owned(),
            lease_ms: 120_000,
            attempt: Some(attempt.identity.clone()),
        },
        t0,
        COORDINATOR,
        &[],
    )?;
    drop(store);

    // Run `hold` on a disjoint scope (`attempt: None`).
    let stop_file = directory.path().join("stop-indep");
    let fake_clock = FakeHoldClock::new(t0);
    let mode = Arc::new(std::sync::Mutex::new(ProxyMode::Forward));
    let (url, recorded, stop_proxy) =
        spawn_recording_proxy(Arc::clone(&service), Arc::clone(&fake_clock.now_ms), mode)?;
    let client = ApiClient::new(&url)?;
    let spec = HoldSpec {
        scope: "OnixResearch/kiln docs".to_owned(),
        tool: "codex".to_owned(),
        session: "hold-indep".to_owned(),
        purpose: "independent held claim".to_owned(),
        lease_minutes: 3,
        command: vec![
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            "while [ ! -f \"$1\" ]; do sleep 0.02; done".to_owned(),
            "_".to_owned(),
            stop_file.to_string_lossy().into_owned(),
        ],
    };
    let clock_thread = fake_clock.handle();
    let hold_handle = std::thread::spawn(move || {
        let signals = kiln_fleet::hold::HoldSignals::new();
        run_with(&client, &spec, &clock_thread, &signals)
    });

    wait_for_recorded(&recorded, |reqs| {
        reqs.iter().any(|r| r.path == "/v1/claims")
    })?;
    fake_clock.wait_sleep_advance()?;

    let claims_before = service.store.open_coordinator()?.fleet_state()?.claims;
    let attempt_claim_before = claims_before
        .claims
        .values()
        .find(|c| c.attempt.as_ref() == Some(&attempt.identity))
        .ok_or("missing attempt claim")?
        .clone();
    let held_claim_before = claims_before
        .claims
        .values()
        .find(|c| c.attempt.is_none())
        .ok_or("missing held claim")?
        .clone();
    assert_eq!(held_claim_before.expires_ms, t0 + 180_000);

    // Tick dispatcher at t0 + 30_000: renews attempt claim, never touches held claim.
    dispatcher.tick(t0 + 30_000)?;
    let claims_after_tick = service.store.open_coordinator()?.fleet_state()?.claims;
    let attempt_claim_after_tick = claims_after_tick
        .claims
        .get(&attempt_claim_before.identity)
        .ok_or("missing attempt claim after tick")?;
    let held_claim_after_tick = claims_after_tick
        .claims
        .get(&held_claim_before.identity)
        .ok_or("missing held claim after tick")?;
    assert!(attempt_claim_after_tick.expires_ms > attempt_claim_before.expires_ms);
    assert_eq!(
        held_claim_after_tick.expires_ms,
        held_claim_before.expires_ms
    );

    // Advance hold clock so hold renews its own claim; attempt claim expires_ms stays unchanged.
    fake_clock
        .now_ms
        .store(t0 + 60_000, std::sync::atomic::Ordering::SeqCst);
    wait_for_recorded(&recorded, |reqs| {
        reqs.iter().any(|r| r.path.ends_with("/renew"))
    })?;
    fake_clock.wait_sleep_advance()?;
    let claims_after_hold_renew = service.store.open_coordinator()?.fleet_state()?.claims;
    assert_eq!(
        claims_after_hold_renew
            .claims
            .get(&attempt_claim_before.identity)
            .ok_or("missing attempt claim")?
            .expires_ms,
        attempt_claim_after_tick.expires_ms
    );
    assert_eq!(
        claims_after_hold_renew
            .claims
            .get(&held_claim_before.identity)
            .ok_or("missing held claim")?
            .expires_ms,
        t0 + 60_000 + 180_000
    );

    // Remove agent so dispatcher stops the session -> releases ONLY attempt claim, NOT held claim.
    std::fs::remove_file(state_dir.join("live"))?;
    dispatcher.tick(t0 + 65_000)?;
    let claims_after_stop = service.store.open_coordinator()?.fleet_state()?.claims;
    assert!(matches!(
        claims_after_stop
            .claims
            .get(&attempt_claim_before.identity)
            .ok_or("missing attempt claim")?
            .state,
        ClaimState::Released { .. }
    ));
    assert_eq!(
        claims_after_stop
            .claims
            .get(&held_claim_before.identity)
            .ok_or("missing held claim")?
            .state,
        ClaimState::Open
    );

    std::fs::write(&stop_file, b"done")?;
    assert_eq!(hold_handle.join().map_err(|_| "hold panicked")?, 0);
    stop_proxy.store(true, std::sync::atomic::Ordering::SeqCst);

    let final_claims = service.store.open_coordinator()?.fleet_state()?.claims;
    assert!(matches!(
        &final_claims
            .claims
            .get(&held_claim_before.identity)
            .ok_or("missing held claim")?
            .state,
        ClaimState::Released { note, .. } if note == "exit 0"
    ));
    Ok(())
}

#[test]
fn hold_unreachable_coordinator_keeps_running_and_warns_until_lease_expires() -> TestResult {
    use kiln_core::fleet::ClaimState;
    use kiln_fleet::hold::{HoldSpec, run_with};

    // Subcase A: transient outage (DropConnection + 503) keeps child running and recovers before expires_ms.
    {
        let fixture = fixture()?;
        let directory = tempfile::tempdir()?;
        let stop_file = directory.path().join("stop-recover");
        let t0: u64 = 10_000_000_000;
        let fake_clock = FakeHoldClock::new(t0);
        let mode = Arc::new(std::sync::Mutex::new(ProxyMode::Forward));
        let (url, recorded, stop_proxy) = spawn_recording_proxy(
            Arc::clone(&fixture.service),
            Arc::clone(&fake_clock.now_ms),
            Arc::clone(&mode),
        )?;

        let client = ApiClient::new(&url)?;
        let spec = HoldSpec {
            scope: "OnixResearch/kiln crates/kiln-fleet".to_owned(),
            tool: "codex".to_owned(),
            session: "unreachable-recover".to_owned(),
            purpose: "transient partition".to_owned(),
            lease_minutes: 3,
            command: vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "while [ ! -f \"$1\" ]; do sleep 0.02; done".to_owned(),
                "_".to_owned(),
                stop_file.to_string_lossy().into_owned(),
            ],
        };
        let clock_thread = fake_clock.handle();
        let handle = std::thread::spawn(move || {
            let signals = kiln_fleet::hold::HoldSignals::new();
            run_with(&client, &spec, &clock_thread, &signals)
        });

        wait_for_recorded(&recorded, |reqs| {
            reqs.iter().any(|r| r.path == "/v1/claims")
        })?;
        fake_clock.wait_sleep_advance()?;

        // Simulate 503 from coordinator at 1/3 lease, then transport drop on retry.
        if let Ok(mut guard) = mode.lock() {
            *guard = ProxyMode::Return503;
        }
        fake_clock
            .now_ms
            .store(t0 + 60_000, std::sync::atomic::Ordering::SeqCst);
        wait_for_recorded(&recorded, |reqs| {
            reqs.iter().filter(|r| r.path.ends_with("/renew")).count() >= 1
        })?;
        fake_clock.wait_sleep_advance()?;
        assert!(
            !handle.is_finished(),
            "child must keep running before expires_ms"
        );

        if let Ok(mut guard) = mode.lock() {
            *guard = ProxyMode::DropConnection;
        }
        fake_clock
            .now_ms
            .store(t0 + 65_000, std::sync::atomic::Ordering::SeqCst);
        // Two sleep ticks confirm hold tried DropConnection, warned, updated next_renew_ms, and stayed alive.
        fake_clock.wait_sleep_advance()?;
        fake_clock.wait_sleep_advance()?;
        assert!(
            !handle.is_finished(),
            "child must keep running on transport error before expires_ms"
        );

        // Coordinator recovers before expires_ms (t0 + 180_000).
        if let Ok(mut guard) = mode.lock() {
            *guard = ProxyMode::Forward;
        }
        fake_clock
            .now_ms
            .store(t0 + 70_000, std::sync::atomic::Ordering::SeqCst);
        wait_for_recorded(&recorded, |reqs| {
            reqs.iter().filter(|r| r.path.ends_with("/renew")).count() >= 2
        })?;
        fake_clock.wait_sleep_advance()?;

        std::fs::write(&stop_file, b"done")?;
        let exit_code = handle.join().map_err(|_| "hold panicked")?;
        stop_proxy.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(exit_code, 0);

        let state = fixture.service.store.open_coordinator()?.fleet_state()?;
        let claim = state
            .claims
            .claims
            .values()
            .next()
            .ok_or("missing recovered claim")?;
        assert!(matches!(
            &claim.state,
            ClaimState::Released { note, .. } if note == "exit 0"
        ));
    }

    // Subcase B: coordinator remains unreachable past expires_ms -> kills child, exits 1, sends NO release.
    {
        let fixture = fixture()?;
        let directory = tempfile::tempdir()?;
        let marker = directory.path().join("ready-unreach");
        let pid_file = directory.path().join("child-unreach.pid");
        let t0: u64 = 10_000_000_000;
        let fake_clock = FakeHoldClock::new(t0);
        let mode = Arc::new(std::sync::Mutex::new(ProxyMode::Forward));
        let (url, recorded, stop_proxy) = spawn_recording_proxy(
            Arc::clone(&fixture.service),
            Arc::clone(&fake_clock.now_ms),
            Arc::clone(&mode),
        )?;

        let client = ApiClient::new(&url)?;
        let spec = HoldSpec {
            scope: "OnixResearch/kiln docs".to_owned(),
            tool: "codex".to_owned(),
            session: "unreachable-expire".to_owned(),
            purpose: "prolonged partition".to_owned(),
            lease_minutes: 3,
            command: vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "echo $$ > \"$1\"; touch \"$2\"; sleep 60".to_owned(),
                "_".to_owned(),
                pid_file.to_string_lossy().into_owned(),
                marker.to_string_lossy().into_owned(),
            ],
        };
        let clock_thread = fake_clock.handle();
        let handle = std::thread::spawn(move || {
            let signals = kiln_fleet::hold::HoldSignals::new();
            run_with(&client, &spec, &clock_thread, &signals)
        });

        wait_for_path(&marker)?;
        let child_pid = wait_for_pid_file(&pid_file)?;
        wait_for_recorded(&recorded, |reqs| {
            reqs.iter().any(|r| r.path == "/v1/claims")
        })?;
        fake_clock.wait_sleep_advance()?;

        // Shut down proxy listener completely so TCP connect fails with ConnectionRefused.
        stop_proxy.store(true, std::sync::atomic::Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(30));

        // Before expires_ms (t0 + 60_000): warns and keeps running.
        fake_clock
            .now_ms
            .store(t0 + 60_000, std::sync::atomic::Ordering::SeqCst);
        fake_clock.wait_sleep_advance()?;
        fake_clock.wait_sleep_advance()?;
        assert!(
            !handle.is_finished(),
            "child must keep running before expires_ms"
        );

        // At expires_ms (t0 + 180_000): stops child, exits 1, sends no release.
        fake_clock
            .now_ms
            .store(t0 + 180_000, std::sync::atomic::Ordering::SeqCst);
        let exit_code = handle.join().map_err(|_| "hold panicked")?;
        assert_eq!(exit_code, 1);
        assert_pid_dead(child_pid)?;

        let state = fixture.service.store.open_coordinator()?.fleet_state()?;
        let claim = state
            .claims
            .claims
            .values()
            .next()
            .ok_or("missing expired-unreachable claim")?;
        assert_eq!(claim.state, ClaimState::Open);
    }

    Ok(())
}

#[test]
fn hold_refuses_existing_same_holder_claim_without_collision_or_release() -> TestResult {
    use kiln_core::fleet::ClaimState;
    use kiln_fleet::hold::{HoldSpec, run_with};

    let fixture = fixture()?;
    let directory = tempfile::tempdir()?;
    let ready = directory.path().join("ready");
    let stop_file = directory.path().join("stop");
    let sentinel = directory.path().join("duplicate-should-not-exist");
    let t0: u64 = 10_000_000_000;
    let fake_clock = FakeHoldClock::new(t0);
    let mode = Arc::new(std::sync::Mutex::new(ProxyMode::Forward));
    let (url, recorded, stop_proxy) = spawn_recording_proxy(
        Arc::clone(&fixture.service),
        Arc::clone(&fake_clock.now_ms),
        mode,
    )?;

    let client_one = ApiClient::new(&url)?;
    let spec_one = HoldSpec {
        scope: "OnixResearch/kiln crates/kiln-fleet".to_owned(),
        tool: "codex".to_owned(),
        session: "same-session".to_owned(),
        purpose: "primary holder".to_owned(),
        lease_minutes: 3,
        command: vec![
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            "touch \"$1\"; while [ ! -f \"$2\" ]; do sleep 0.02; done".to_owned(),
            "_".to_owned(),
            ready.to_string_lossy().into_owned(),
            stop_file.to_string_lossy().into_owned(),
        ],
    };
    let clock_one = fake_clock.handle();
    let handle_one = std::thread::spawn(move || {
        let signals = kiln_fleet::hold::HoldSignals::new();
        run_with(&client_one, &spec_one, &clock_one, &signals)
    });

    wait_for_path(&ready)?;
    wait_for_recorded(&recorded, |reqs| {
        reqs.iter()
            .any(|req| req.method == "POST" && req.path == "/v1/claims")
    })?;
    fake_clock.wait_sleep_advance()?;

    let initial_state = fixture.service.store.open_coordinator()?.fleet_state()?;
    assert_eq!(initial_state.claims.claims.len(), 1);
    let claim_id = initial_state
        .claims
        .claims
        .values()
        .next()
        .ok_or("missing primary claim")?
        .identity
        .clone();

    let client_two = ApiClient::new(&url)?;
    let spec_two = HoldSpec {
        scope: "OnixResearch/kiln crates/kiln-fleet".to_owned(),
        tool: "codex".to_owned(),
        session: "same-session".to_owned(),
        purpose: "duplicate holder".to_owned(),
        lease_minutes: 3,
        command: vec![
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            "touch \"$1\"".to_owned(),
            "_".to_owned(),
            sentinel.to_string_lossy().into_owned(),
        ],
    };
    let signals_two = kiln_fleet::hold::HoldSignals::new();
    let exit_two = run_with(&client_two, &spec_two, &fake_clock, &signals_two);
    assert_eq!(exit_two, 1);
    assert!(
        !sentinel.exists(),
        "second hold with same (scope, tool, session) must not spawn child"
    );

    let reqs_after_two = recorded.lock().map_err(|_| "lock poisoned")?.clone();
    assert_eq!(
        reqs_after_two
            .iter()
            .filter(|req| req.method == "POST" && req.path == "/v1/claims")
            .count(),
        2
    );
    assert_eq!(
        reqs_after_two
            .iter()
            .filter(|req| req.path.ends_with("/release"))
            .count(),
        0
    );
    assert_eq!(
        reqs_after_two
            .iter()
            .filter(|req| req.path.ends_with("/renew"))
            .count(),
        0
    );

    let state_during = fixture.service.store.open_coordinator()?.fleet_state()?;
    assert_eq!(state_during.claims.claims.len(), 1);
    assert_eq!(
        state_during
            .claims
            .claims
            .get(&claim_id)
            .ok_or("missing primary claim after refusal")?
            .state,
        ClaimState::Open
    );

    // First hold continues renewing and releases cleanly on exit.
    fake_clock
        .now_ms
        .store(t0 + 60_000, std::sync::atomic::Ordering::SeqCst);
    wait_for_recorded(&recorded, |reqs| {
        reqs.iter()
            .filter(|req| req.path.ends_with("/renew"))
            .count()
            >= 1
    })?;
    fake_clock.wait_sleep_advance()?;

    std::fs::write(&stop_file, b"done")?;
    let exit_one = handle_one.join().map_err(|_| "hold one panicked")?;
    stop_proxy.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(exit_one, 0);

    let final_state = fixture.service.store.open_coordinator()?.fleet_state()?;
    let final_claim = final_state
        .claims
        .claims
        .get(&claim_id)
        .ok_or("missing primary claim at end")?;
    assert!(matches!(
        &final_claim.state,
        ClaimState::Released { note, .. } if note == "exit 0"
    ));
    Ok(())
}

#[test]
fn hold_slow_renew_crossing_deadline_stops_child_without_second_renew() -> TestResult {
    use kiln_core::fleet::ClaimState;
    use kiln_fleet::hold::{HoldSpec, run_with};

    for (label, renew_mode) in [
        (
            "delayed-503",
            ProxyMode::AdvanceClockOnRenew503(10_000_065_000),
        ),
        (
            "delayed-drop",
            ProxyMode::AdvanceClockOnRenewDrop(10_000_065_000),
        ),
    ] {
        let fixture = fixture()?;
        let directory = tempfile::tempdir()?;
        let marker = directory.path().join(format!("ready-{label}"));
        let pid_file = directory.path().join(format!("child-{label}.pid"));
        let t0: u64 = 10_000_000_000;
        let fake_clock = FakeHoldClock::new(t0);
        let mode = Arc::new(std::sync::Mutex::new(ProxyMode::Forward));
        let (url, recorded, stop_proxy) = spawn_recording_proxy(
            Arc::clone(&fixture.service),
            Arc::clone(&fake_clock.now_ms),
            Arc::clone(&mode),
        )?;

        let client = ApiClient::new(&url)?;
        let spec = HoldSpec {
            scope: "OnixResearch/kiln crates/kiln-fleet".to_owned(),
            tool: "codex".to_owned(),
            session: format!("slow-renew-{label}"),
            purpose: "verify post-request clock re-sampling".to_owned(),
            lease_minutes: 1,
            command: vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                "echo $$ > \"$1\"; touch \"$2\"; sleep 60".to_owned(),
                "_".to_owned(),
                pid_file.to_string_lossy().into_owned(),
                marker.to_string_lossy().into_owned(),
            ],
        };
        let clock_thread = fake_clock.handle();
        let handle = std::thread::spawn(move || {
            let signals = kiln_fleet::hold::HoldSignals::new();
            run_with(&client, &spec, &clock_thread, &signals)
        });

        wait_for_path(&marker)?;
        let child_pid = wait_for_pid_file(&pid_file)?;
        wait_for_recorded(&recorded, |reqs| {
            reqs.iter()
                .any(|r| r.method == "POST" && r.path == "/v1/claims")
        })?;
        fake_clock.wait_sleep_advance()?;

        // Arm the proxy to advance FakeHoldClock past lease_deadline_ms (t0 + 60_000)
        // during the upcoming renew call, then trigger renew at t0 + 58_000.
        if let Ok(mut guard) = mode.lock() {
            *guard = renew_mode;
        }
        fake_clock
            .now_ms
            .store(t0 + 58_000, std::sync::atomic::Ordering::SeqCst);

        let exit_code = handle.join().map_err(|_| "hold panicked")?;
        stop_proxy.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(exit_code, 1, "{label}");
        assert_pid_dead(child_pid)?;

        let reqs = recorded.lock().map_err(|_| "lock poisoned")?.clone();
        assert_eq!(
            reqs.iter().filter(|r| r.path.ends_with("/renew")).count(),
            1,
            "{label}: must not issue a second renew after post-request clock passes lease_deadline_ms"
        );
        assert_eq!(
            reqs.iter().filter(|r| r.path.ends_with("/release")).count(),
            0,
            "{label}: must not release after expired unreachable renew"
        );

        let state = fixture.service.store.open_coordinator()?.fleet_state()?;
        let claim = state.claims.claims.values().next().ok_or("missing claim")?;
        assert_eq!(claim.state, ClaimState::Open);
    }

    Ok(())
}
