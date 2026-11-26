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
        if req.method == "GET" && path == "/api/fleet/machines" {
            let now = now_ms()?;
            return Ok(json(
                200,
                json!({"synthetic": true, "machines": [
                    {"machine": "example-desktop", "placement_configured": true, "observed_at_ms": now, "last_seen_at_ms": now, "state": "reporting", "sessions": 2, "core": {"state":"reporting","observed_at_ms":now,"last_seen_at_ms":now,"terminals":2,"statuses":{"idle":0,"working":0,"done":0,"blocked":1,"error":0,"unknown":1}}},
                    {"machine": "example-worker", "placement_configured": true, "observed_at_ms": now, "last_seen_at_ms": null, "state": "unreachable", "sessions": null, "core":{"state":"unreachable","observed_at_ms":now,"last_seen_at_ms":null,"terminals":null,"statuses":null}},
                    {"machine": "example-stale", "placement_configured": true, "observed_at_ms": now.saturating_sub(600_000), "last_seen_at_ms": now.saturating_sub(600_000), "state": "stale", "sessions": null, "core":{"state":"stale","observed_at_ms":now,"last_seen_at_ms":now.saturating_sub(600_000),"terminals":null,"statuses":null}},
                    {"machine": "example-peer", "placement_configured": false, "observed_at_ms": null, "last_seen_at_ms": null, "state": "not_reporting", "sessions": null}
                ]}),
            ));
        }
        if req.method == "GET" && path == "/api/fleet/events" {
            let version = "e".repeat(64);
            if req.target.ends_with(&format!("?after={version}")) {
                return Ok(bytes("text/event-stream", b": heartbeat\n\n"));
            }
            let attempt = "d".repeat(64);
            let work = "a".repeat(64);
            let snapshot = json!({
                "version": version,
                "health": {"machine": "example-desktop", "version": "example", "api": "v1"},
                "claims": {"claims": []},
                "work": {"ready_unclaimed": 1, "ranked": [{"identity": work, "repository": "example/release-service", "title": "Synthetic issue work", "sources": [{"kind": "github_issue", "source": {"contract_version": 1, "provider_namespace": "github", "native_kind": "issue", "native_id": "example/release-service#42"}, "native_id": "example/release-service#42", "version": "b".repeat(64)}]}], "blocked": [], "truncated": false},
                "attempts": {"attempts": [{"attempt": attempt, "work": work, "title": "Synthetic running attempt", "state": "running", "fence": 1, "session": {"machine": "example-desktop"}}]},
                "decisions": {"decisions": []}
            });
            return Ok(bytes(
                "text/event-stream",
                format!("event: snapshot\ndata: {snapshot}\n\n").as_bytes(),
            ));
        }
        // r[impl kiln.inventory.consumers]
        if path == "/api/inventory" {
            return inventory::response(&req.method, &req.target);
        }
        if path == "/api/implementation/campaigns" {
            return campaign::response(&self.campaign, &req.method, &req.target);
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
            return Ok(self
                .ci
                .engine
                .api_as(&req.method, &req.target, &req.body, &principal)?);
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
        "/examples" => bytes(
            "text/html; charset=utf-8",
            include_bytes!("explore/index.html"),
        ),
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
    stream.set_write_timeout(Some(REQUEST_DEADLINE))?;
    write!(
        stream,
        "HTTP/1.1 {} Example\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\n",
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
    listener.set_nonblocking(true)?;
    println!(
        "Synthetic examples; no provider is contacted or source command executed.\nExplore: http://{authority}/examples\nConsole: http://{authority}/\nRestart to reset. Ctrl-C removes the disposable workspace."
    );
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let response = match request(&stream, &authority) {
                    Ok(req) => match workspace.route(&req) {
                        Ok(response) => response,
                        Err(cause) => {
                            eprintln!("Example request failed: {cause}");
                            error(
                                503,
                                "Local example request failed; no fallback data was substituted",
                            )
                        }
                    },
                    Err(response) => response,
                };
                if let Err(cause) = respond(&mut stream, response) {
                    eprintln!("Example response disconnected: {cause}");
                }
            }
            Err(cause) if cause.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_INTERVAL)
            }
            Err(cause) => return Err(cause.into()),
        }
    }
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
}
