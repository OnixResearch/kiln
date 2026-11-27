//! Read-only observed implementation-provider graph and bounded logs from an
//! explicitly admitted exact-execution artifact route.
//!
//! No URL or query parameter supplies a filesystem path or provider route.
//! Canonical attempt, fence, work, and provider execution identity come from
//! the `--implementation-work` database; artifact roots and capabilities come
//! from the startup-admitted `--execution-artifacts` profile.
// r[impl kiln.fleet.operator_plane.scenario.stuck]

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};

use kiln_core::implementation::{AttemptLedger, ImplementationAttempt};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::observation::ImplementationObservation;
use kiln_provider_api::inventory::InventoryReadFailure;
use serde::{Deserialize, Serialize};

use crate::engine::{ApiResponse, Role, host_time_ms};

pub const ROUTE: &str = "/api/implementation/execution-artifacts";
pub const PROFILE_SCHEMA: &str = "kiln.execution-artifacts-profile.v1";
pub const OBSERVATION_SCHEMA: &str = "kiln.execution-artifacts-observation.v1";
pub const ARTIFACT_SCHEMA: &str = "kiln.execution-artifact-record.v1";

pub const MAX_PROFILE_BYTES: u64 = 16 * 1024;
pub const MAX_ADMITTED_ROUTES: usize = 16;
pub const MAX_ARTIFACT_FILE_BYTES_CAP: u64 = 2 * 1024 * 1024;
pub const DEFAULT_MAX_ARTIFACT_FILE_BYTES: u64 = 512 * 1024;
pub const MAX_GRAPH_NODES_CAP: usize = 256;
pub const DEFAULT_MAX_GRAPH_NODES: usize = 128;
pub const MAX_GRAPH_EDGES_CAP: usize = 1024;
pub const DEFAULT_MAX_GRAPH_EDGES: usize = 512;
pub const MAX_LOG_ENTRIES_CAP: usize = 512;
pub const DEFAULT_MAX_LOG_ENTRIES: usize = 128;
pub const MAX_LOG_BYTES_CAP: usize = 64 * 1024;
pub const DEFAULT_MAX_LOG_BYTES: usize = 16 * 1024;
pub const MAX_FRESHNESS_WINDOW_MS_CAP: u64 = 24 * 60 * 60 * 1000;
pub const DEFAULT_FRESHNESS_WINDOW_MS: u64 = 5 * 60 * 1000;
pub const MAX_RESPONSE_BYTES: usize = 128 * 1024;

const MAX_PROVIDER_TOKEN_BYTES: usize = 64;
const MAX_EXECUTION_ID_BYTES: usize = 128;
const MAX_LABEL_BYTES: usize = 128;
const MAX_DETAIL_BYTES: usize = 256;

const NON_CLAIMS: [&str; 2] = [
    "observed implementation-provider graph and logs are provider artifacts, not Kiln verification or completion authority",
    "retained execution artifacts do not prove physical host liveness or release readiness",
];

const NODE_STATES: [&str; 8] = [
    "pending",
    "running",
    "paused",
    "completed",
    "failed",
    "cancelled",
    "skipped",
    "unknown",
];

const LOG_STREAMS: [&str; 3] = ["stdout", "stderr", "diagnostic"];

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactBounds {
    pub max_artifact_bytes: u64,
    pub max_graph_nodes: usize,
    pub max_graph_edges: usize,
    pub max_log_entries: usize,
    pub max_log_bytes: usize,
    pub freshness_window_ms: u64,
}

impl Default for ArtifactBounds {
    fn default() -> Self {
        Self {
            max_artifact_bytes: DEFAULT_MAX_ARTIFACT_FILE_BYTES,
            max_graph_nodes: DEFAULT_MAX_GRAPH_NODES,
            max_graph_edges: DEFAULT_MAX_GRAPH_EDGES,
            max_log_entries: DEFAULT_MAX_LOG_ENTRIES,
            max_log_bytes: DEFAULT_MAX_LOG_BYTES,
            freshness_window_ms: DEFAULT_FRESHNESS_WINDOW_MS,
        }
    }
}

const fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedArtifactRoute {
    pub provider_namespace: String,
    pub native_kind: String,
    pub artifact_directory: PathBuf,
    #[serde(default = "default_true")]
    pub supported: bool,
    #[serde(default = "default_true")]
    pub supports_graph: bool,
    #[serde(default = "default_true")]
    pub supports_logs: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionArtifactsProfile {
    pub schema: String,
    pub implementation_database: PathBuf,
    pub routes: Vec<AdmittedArtifactRoute>,
    #[serde(default)]
    pub bounds: ArtifactBounds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactQuery {
    pub attempt: ContentId,
    pub fence: u64,
    pub work: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionError {
    pub status: u16,
    pub code: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteSelection<'a> {
    Unavailable {
        reason: &'static str,
    },
    Unsupported {
        execution: ExternalRef,
        reason: &'static str,
    },
    UnsafeIdentity,
    Admitted {
        route: &'a AdmittedArtifactRoute,
        execution: ExternalRef,
        filename: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionArtifactRecord {
    pub schema: String,
    pub work_identity: ContentId,
    pub attempt_identity: ContentId,
    pub fence: u64,
    pub execution: ExternalRef,
    pub observed_at_ms: u64,
    #[serde(default)]
    pub execution_state: Option<String>,
    #[serde(default)]
    pub graph: Option<ObservedGraphInput>,
    #[serde(default)]
    pub logs: Option<ObservedLogsInput>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedGraphInput {
    #[serde(default)]
    pub graph_identity: Option<String>,
    pub nodes: Vec<ObservedGraphNodeInput>,
    pub edges: Vec<ObservedGraphEdgeInput>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedGraphNodeInput {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub terminal: bool,
    pub successful: bool,
    #[serde(default)]
    pub adapter: Option<String>,
    #[serde(default)]
    pub receipt: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedGraphEdgeInput {
    pub source: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedLogsInput {
    #[serde(default)]
    pub redacted: bool,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub total_bytes: Option<u64>,
    pub entries: Vec<ObservedLogEntryInput>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedLogEntryInput {
    pub node_id: String,
    pub stream: String,
    pub cursor: u64,
    pub text: String,
    #[serde(default)]
    pub redacted: bool,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub observed_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ObservedGraphNode {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub terminal: bool,
    pub successful: bool,
    pub adapter: Option<String>,
    pub receipt: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ObservedGraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphProjection {
    pub status: &'static str,
    pub reason: Option<&'static str>,
    pub graph_identity: Option<String>,
    pub nodes: Vec<ObservedGraphNode>,
    pub edges: Vec<ObservedGraphEdge>,
}

impl GraphProjection {
    fn unavailable(reason: &'static str) -> Self {
        Self {
            status: "unavailable",
            reason: Some(reason),
            graph_identity: None,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    fn unsupported(reason: &'static str) -> Self {
        Self {
            status: "unsupported",
            reason: Some(reason),
            graph_identity: None,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ObservedLogEntry {
    pub node_id: String,
    pub stream: String,
    pub cursor: u64,
    pub text: String,
    pub redacted: bool,
    pub truncated: bool,
    pub observed_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LogsProjection {
    pub status: &'static str,
    pub reason: Option<&'static str>,
    pub redacted: bool,
    pub truncated: bool,
    pub retained_entries: usize,
    pub omitted_entries: usize,
    pub retained_bytes: usize,
    pub total_bytes: u64,
    pub entries: Vec<ObservedLogEntry>,
}

impl LogsProjection {
    fn unavailable(reason: &'static str) -> Self {
        Self {
            status: "unavailable",
            reason: Some(reason),
            redacted: false,
            truncated: false,
            retained_entries: 0,
            omitted_entries: 0,
            retained_bytes: 0,
            total_bytes: 0,
            entries: Vec::new(),
        }
    }

    fn unsupported(reason: &'static str) -> Self {
        Self {
            status: "unsupported",
            reason: Some(reason),
            redacted: false,
            truncated: false,
            retained_entries: 0,
            omitted_entries: 0,
            retained_bytes: 0,
            total_bytes: 0,
            entries: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutionArtifactsView {
    pub schema: &'static str,
    pub status: &'static str,
    pub reason: Option<&'static str>,
    pub attempt: ContentId,
    pub fence: u64,
    pub work_identity: ContentId,
    pub execution: Option<ExternalRef>,
    pub execution_state: Option<String>,
    pub observed_at_ms: Option<u64>,
    pub evaluated_at_ms: u64,
    pub freshness: &'static str,
    pub age_ms: Option<u64>,
    pub freshness_window_ms: u64,
    pub source: &'static str,
    pub graph: GraphProjection,
    pub logs: LogsProjection,
    pub non_claims: [&'static str; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    IdentityMismatch,
    Malformed,
    Oversized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadArtifactError {
    UnsafePath,
    Oversized,
    Io,
}

pub(crate) fn clean_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path != Path::new("/")
        && path
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
}

fn hex64_identity(value: &str) -> Option<ContentId> {
    if value.len() != blake3::OUT_LEN * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    ContentId::from_blake3_hex(value.to_owned()).ok()
}

fn provider_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROVIDER_TOKEN_BYTES
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

fn safe_execution_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_EXECUTION_ID_BYTES
        && !value.starts_with('.')
        && !value.contains("..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:@-".contains(&b))
}

fn safe_label(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && !value.chars().any(char::is_control)
}

fn safe_log_text(value: &str) -> bool {
    !value.chars().any(|ch| {
        ch == '\0' || (ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t')
    })
}

/// Pure query admission: requires exact ordered `attempt=<hex64>&fence=<u64>&work=<hex64>`.
pub fn parse_query(target: &str) -> Option<ArtifactQuery> {
    if target.contains('#')
        || target
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
    {
        return None;
    }
    let query = target.strip_prefix(ROUTE)?.strip_prefix('?')?;
    let (attempt_raw, rest) = query.strip_prefix("attempt=")?.split_once("&fence=")?;
    let (fence_raw, work_raw) = rest.split_once("&work=")?;
    if fence_raw.is_empty()
        || fence_raw.starts_with('0')
        || !fence_raw.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let fence = fence_raw.parse::<u64>().ok().filter(|value| *value > 0)?;
    Some(ArtifactQuery {
        attempt: hex64_identity(attempt_raw)?,
        fence,
        work: hex64_identity(work_raw)?,
    })
}

/// Pure HTTP request admission gate: enforces non-restricted operator role,
/// read-only GET with empty body, and exact query parameters.
pub fn admit_request(
    method: &str,
    body: &[u8],
    role: Role,
    restricted_projection: bool,
    target: &str,
) -> Result<ArtifactQuery, AdmissionError> {
    if restricted_projection || role != Role::Operator {
        return Err(AdmissionError {
            status: 403,
            code: "execution_artifacts_forbidden",
        });
    }
    if method != "GET" || !body.is_empty() {
        return Err(AdmissionError {
            status: 405,
            code: "execution_artifacts_method_invalid",
        });
    }
    parse_query(target).ok_or(AdmissionError {
        status: 400,
        code: "execution_artifacts_query_invalid",
    })
}

/// Pure startup profile admission: validates schema, path hygiene, unique route
/// bindings, and explicit bounds.
pub fn admit_profile(profile: &ExecutionArtifactsProfile) -> Result<(), String> {
    if profile.schema != PROFILE_SCHEMA {
        return Err("execution artifacts profile schema invalid".into());
    }
    if !clean_absolute_path(&profile.implementation_database) {
        return Err("execution artifacts database path must be clean and absolute".into());
    }
    if profile.routes.is_empty() || profile.routes.len() > MAX_ADMITTED_ROUTES {
        return Err("execution artifacts routes must contain 1..=16 entries".into());
    }
    let b = &profile.bounds;
    if b.max_artifact_bytes == 0
        || b.max_artifact_bytes > MAX_ARTIFACT_FILE_BYTES_CAP
        || b.max_graph_nodes == 0
        || b.max_graph_nodes > MAX_GRAPH_NODES_CAP
        || b.max_graph_edges == 0
        || b.max_graph_edges > MAX_GRAPH_EDGES_CAP
        || b.max_log_entries == 0
        || b.max_log_entries > MAX_LOG_ENTRIES_CAP
        || b.max_log_bytes == 0
        || b.max_log_bytes > MAX_LOG_BYTES_CAP
        || b.freshness_window_ms == 0
        || b.freshness_window_ms > MAX_FRESHNESS_WINDOW_MS_CAP
    {
        return Err("execution artifacts bounds exceed permitted caps".into());
    }
    let mut seen = BTreeSet::new();
    for route in &profile.routes {
        if !provider_token(&route.provider_namespace)
            || !provider_token(&route.native_kind)
            || !clean_absolute_path(&route.artifact_directory)
            || !seen.insert((
                route.provider_namespace.as_str(),
                route.native_kind.as_str(),
            ))
        {
            return Err("execution artifacts route declaration invalid or duplicated".into());
        }
    }
    Ok(())
}

/// Pure route selection: binds the attempt's recorded provider execution to an
/// explicitly admitted route without touching the filesystem.
#[must_use]
pub fn select_admitted_route<'a>(
    profile: &'a ExecutionArtifactsProfile,
    attempt: &ImplementationAttempt,
) -> RouteSelection<'a> {
    let Some(execution) = attempt.workflow.clone() else {
        return RouteSelection::Unavailable {
            reason: "execution_not_observed",
        };
    };
    let Some(route) = profile.routes.iter().find(|candidate| {
        candidate.provider_namespace == execution.provider_namespace.as_str()
            && candidate.native_kind == execution.native_kind.as_str()
    }) else {
        return RouteSelection::Unsupported {
            execution,
            reason: "artifact_route_not_admitted",
        };
    };
    if !route.supported {
        return RouteSelection::Unsupported {
            execution,
            reason: "provider_artifacts_unsupported",
        };
    }
    if !safe_execution_id(execution.native_id.as_str()) {
        return RouteSelection::UnsafeIdentity;
    }
    let filename = format!("execution-{}.json", execution.native_id.as_str());
    RouteSelection::Admitted {
        route,
        execution,
        filename,
    }
}

/// Pure log redaction for common credential shapes as defense-in-depth.
#[must_use]
pub fn redact_log_text(input: &str) -> (String, bool) {
    let mut out = String::with_capacity(input.len());
    let mut redacted = false;
    let lines: Vec<&str> = input.split('\n').collect();
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let (clean_line, line_redacted) = redact_line(line);
        redacted |= line_redacted;
        out.push_str(&clean_line);
    }
    (out, redacted)
}

fn redact_line(line: &str) -> (String, bool) {
    let mut redacted = false;
    let lower = line.to_ascii_lowercase();
    if let Some(pos) = lower.find("authorization:") {
        let prefix = line.get(..pos + "authorization:".len()).unwrap_or("");
        return (format!("{prefix} [REDACTED]"), true);
    }
    let mut words = Vec::new();
    let mut redact_next = false;
    for word in line.split(' ') {
        if redact_next {
            words.push("[REDACTED]".to_owned());
            redact_next = false;
            redacted = true;
            continue;
        }
        let word_lower = word.to_ascii_lowercase();
        if word_lower == "bearer" {
            words.push(word.to_owned());
            redact_next = true;
            continue;
        }
        if let Some(clean) = redact_kv_or_prefix(word) {
            words.push(clean);
            redacted = true;
        } else {
            words.push(word.to_owned());
        }
    }
    (words.join(" "), redacted)
}

fn redact_kv_or_prefix(token: &str) -> Option<String> {
    if token.starts_with("ghp_")
        || token.starts_with("github_pat_")
        || token.starts_with("sk-")
    {
        return Some("[REDACTED]".to_owned());
    }
    if let Some((key, val)) = token.split_once('=') {
        let key_lower = key.to_ascii_lowercase();
        if matches!(
            key_lower.as_str(),
            "token" | "password" | "secret" | "api_key" | "access_token"
        ) && !val.is_empty()
            && val != "[REDACTED]"
        {
            return Some(format!("{key}=[REDACTED]"));
        }
    }
    None
}

fn valid_node_flags(state: &str, terminal: bool, successful: bool) -> bool {
    match state {
        "pending" | "running" | "paused" | "unknown" => !terminal && !successful,
        "completed" => terminal,
        "failed" | "cancelled" => terminal && !successful,
        "skipped" => terminal,
        _ => false,
    }
}

fn project_graph(
    input: Option<ObservedGraphInput>,
    route: &AdmittedArtifactRoute,
    bounds: &ArtifactBounds,
) -> Result<GraphProjection, ProjectionError> {
    if !route.supports_graph {
        return Ok(GraphProjection::unsupported("graph_unsupported"));
    }
    let Some(graph) = input else {
        return Ok(GraphProjection::unavailable("graph_unobserved"));
    };
    if graph.nodes.len() > bounds.max_graph_nodes || graph.edges.len() > bounds.max_graph_edges {
        return Err(ProjectionError::Malformed);
    }
    if let Some(identity) = &graph.graph_identity
        && !safe_label(identity, MAX_LABEL_BYTES)
    {
        return Err(ProjectionError::Malformed);
    }
    let mut node_ids = BTreeSet::new();
    let mut nodes = Vec::with_capacity(graph.nodes.len());
    for node in graph.nodes {
        if !safe_label(&node.id, MAX_LABEL_BYTES)
            || !safe_label(&node.kind, MAX_LABEL_BYTES)
            || !NODE_STATES.contains(&node.state.as_str())
            || !valid_node_flags(&node.state, node.terminal, node.successful)
            || !node_ids.insert(node.id.clone())
            || node
                .adapter
                .as_deref()
                .is_some_and(|v| !safe_label(v, MAX_DETAIL_BYTES))
            || node
                .receipt
                .as_deref()
                .is_some_and(|v| !safe_label(v, MAX_DETAIL_BYTES))
        {
            return Err(ProjectionError::Malformed);
        }
        nodes.push(ObservedGraphNode {
            id: node.id,
            kind: node.kind,
            state: node.state,
            terminal: node.terminal,
            successful: node.successful,
            adapter: node.adapter,
            receipt: node.receipt,
        });
    }
    let mut edges = Vec::with_capacity(graph.edges.len());
    for edge in graph.edges {
        if edge.source == edge.target
            || !node_ids.contains(&edge.source)
            || !node_ids.contains(&edge.target)
        {
            return Err(ProjectionError::Malformed);
        }
        edges.push(ObservedGraphEdge {
            source: edge.source,
            target: edge.target,
        });
    }
    Ok(GraphProjection {
        status: "available",
        reason: None,
        graph_identity: graph.graph_identity,
        nodes,
        edges,
    })
}

fn floor_char_boundary(s: &str, max_bytes: usize) -> usize {
    if max_bytes >= s.len() {
        return s.len();
    }
    let mut idx = max_bytes;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn project_logs(
    input: Option<ObservedLogsInput>,
    route: &AdmittedArtifactRoute,
    bounds: &ArtifactBounds,
) -> Result<LogsProjection, ProjectionError> {
    if !route.supports_logs {
        return Ok(LogsProjection::unsupported("logs_unsupported"));
    }
    let Some(logs) = input else {
        return Ok(LogsProjection::unavailable("logs_unobserved"));
    };
    let mut raw_total_bytes: u64 = 0;
    for entry in &logs.entries {
        if !safe_label(&entry.node_id, MAX_LABEL_BYTES)
            || !LOG_STREAMS.contains(&entry.stream.as_str())
            || !safe_log_text(&entry.text)
            || entry.observed_at_ms == Some(0)
        {
            return Err(ProjectionError::Malformed);
        }
        raw_total_bytes = raw_total_bytes.saturating_add(entry.text.len() as u64);
    }
    let total_bytes = logs.total_bytes.unwrap_or(raw_total_bytes).max(raw_total_bytes);
    let mut any_redacted = logs.redacted;
    let mut any_truncated = logs.truncated;
    let mut retained_bytes = 0usize;
    let mut entries = Vec::new();
    let total_entries = logs.entries.len();

    for entry in logs.entries {
        if entries.len() >= bounds.max_log_entries || retained_bytes >= bounds.max_log_bytes {
            any_truncated = true;
            break;
        }
        let (clean_text, text_redacted) = redact_log_text(&entry.text);
        let entry_redacted = entry.redacted || text_redacted;
        any_redacted |= entry_redacted;

        let remaining_budget = bounds.max_log_bytes.saturating_sub(retained_bytes);
        let (kept_text, entry_truncated) = if clean_text.len() > remaining_budget {
            let cut = floor_char_boundary(&clean_text, remaining_budget);
            let slice = clean_text.get(..cut).unwrap_or("").to_owned();
            any_truncated = true;
            (slice, true)
        } else {
            (clean_text, entry.truncated)
        };
        any_truncated |= entry_truncated;
        retained_bytes = retained_bytes.saturating_add(kept_text.len());
        entries.push(ObservedLogEntry {
            node_id: entry.node_id,
            stream: entry.stream,
            cursor: entry.cursor,
            text: kept_text,
            redacted: entry_redacted,
            truncated: entry_truncated,
            observed_at_ms: entry.observed_at_ms,
        });
    }

    let retained_entries = entries.len();
    let omitted_entries = total_entries.saturating_sub(retained_entries);
    if omitted_entries > 0 {
        any_truncated = true;
    }

    Ok(LogsProjection {
        status: "available",
        reason: None,
        redacted: any_redacted,
        truncated: any_truncated,
        retained_entries,
        omitted_entries,
        retained_bytes,
        total_bytes,
        entries,
    })
}

/// Pure projection of raw provider artifact bytes into the versioned operator
/// response. Fails closed on identity mismatch, malformed structure, or response
/// byte overflow; truncates bounded logs explicitly with `truncated: true`.
pub fn project_artifact(
    raw_bytes: &[u8],
    attempt: &ImplementationAttempt,
    route: &AdmittedArtifactRoute,
    now_ms: u64,
    bounds: &ArtifactBounds,
) -> Result<ExecutionArtifactsView, ProjectionError> {
    let record: ExecutionArtifactRecord =
        serde_json::from_slice(raw_bytes).map_err(|_| ProjectionError::Malformed)?;
    if record.schema != ARTIFACT_SCHEMA || record.observed_at_ms == 0 {
        return Err(ProjectionError::Malformed);
    }
    if record.work_identity != attempt.work_identity
        || record.attempt_identity != attempt.identity
        || record.fence != attempt.fence
        || attempt.workflow.as_ref() != Some(&record.execution)
    {
        return Err(ProjectionError::IdentityMismatch);
    }
    if let Some(state) = &record.execution_state
        && (!safe_label(state, MAX_PROVIDER_TOKEN_BYTES)
            || !state
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-'))
    {
        return Err(ProjectionError::Malformed);
    }

    let age_ms = now_ms.checked_sub(record.observed_at_ms);
    let freshness = match age_ms {
        Some(age) if age <= bounds.freshness_window_ms => "fresh",
        _ => "stale",
    };

    let graph = project_graph(record.graph, route, bounds)?;
    let logs = project_logs(record.logs, route, bounds)?;

    let view = ExecutionArtifactsView {
        schema: OBSERVATION_SCHEMA,
        status: "available",
        reason: None,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        work_identity: attempt.work_identity.clone(),
        execution: Some(record.execution),
        execution_state: record.execution_state,
        observed_at_ms: Some(record.observed_at_ms),
        evaluated_at_ms: now_ms,
        freshness,
        age_ms,
        freshness_window_ms: bounds.freshness_window_ms,
        source: "admitted_execution_artifact",
        graph,
        logs,
        non_claims: NON_CLAIMS,
    };
    let encoded_len = serde_json::to_vec(&view)
        .map(|bytes| bytes.len())
        .map_err(|_| ProjectionError::Malformed)?;
    if encoded_len > MAX_RESPONSE_BYTES {
        return Err(ProjectionError::Oversized);
    }
    Ok(view)
}

fn unconfigured_or_unobserved_view(
    attempt: &ImplementationAttempt,
    status: &'static str,
    reason: &'static str,
    execution: Option<ExternalRef>,
    now_ms: u64,
    freshness_window_ms: u64,
) -> ExecutionArtifactsView {
    let (graph, logs) = if status == "unsupported" {
        (
            GraphProjection::unsupported(reason),
            LogsProjection::unsupported(reason),
        )
    } else {
        (
            GraphProjection::unavailable(reason),
            LogsProjection::unavailable(reason),
        )
    };
    ExecutionArtifactsView {
        schema: OBSERVATION_SCHEMA,
        status,
        reason: Some(reason),
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        work_identity: attempt.work_identity.clone(),
        execution,
        execution_state: None,
        observed_at_ms: None,
        evaluated_at_ms: now_ms,
        freshness: "unknown",
        age_ms: None,
        freshness_window_ms,
        source: "admitted_execution_artifact",
        graph,
        logs,
        non_claims: NON_CLAIMS,
    }
}

// --- Imperative shell ---

fn open_regular_nofollow(path: &Path, max_bytes: u64) -> Result<Vec<u8>, ReadArtifactError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|err| {
            if err.raw_os_error() == Some(libc::ELOOP) {
                ReadArtifactError::UnsafePath
            } else {
                ReadArtifactError::Io
            }
        })?;
    let meta = file.metadata().map_err(|_| ReadArtifactError::Io)?;
    if !meta.is_file() {
        return Err(ReadArtifactError::UnsafePath);
    }
    if meta.len() > max_bytes {
        return Err(ReadArtifactError::Oversized);
    }
    let canonical = std::fs::canonicalize(path).map_err(|_| ReadArtifactError::UnsafePath)?;
    if canonical != path {
        return Err(ReadArtifactError::UnsafePath);
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ReadArtifactError::Io)?;
    if bytes.len() as u64 > max_bytes {
        return Err(ReadArtifactError::Oversized);
    }
    Ok(bytes)
}

fn ensure_canonical_dir(path: &Path) -> Result<(), String> {
    if !clean_absolute_path(path) {
        return Err("execution artifacts directory must be clean and absolute".into());
    }
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| "execution artifacts directory unavailable".to_owned())?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err("execution artifacts directory must be a non-symlink directory".into());
    }
    let canonical = std::fs::canonicalize(path)
        .map_err(|_| "execution artifacts directory cannot be canonicalized".to_owned())?;
    if canonical != path {
        return Err("execution artifacts directory must be canonical".into());
    }
    Ok(())
}

impl ExecutionArtifactsProfile {
    /// Load and validate the startup-admitted execution artifacts profile.
    pub fn load(path: &Path) -> Result<Self, String> {
        if !clean_absolute_path(path) {
            return Err("execution artifacts profile path must be clean and absolute".into());
        }
        let bytes = open_regular_nofollow(path, MAX_PROFILE_BYTES)
            .map_err(|_| "execution artifacts profile unavailable or unsafe".to_owned())?;
        let profile: Self = serde_json::from_slice(&bytes)
            .map_err(|_| "execution artifacts profile malformed".to_owned())?;
        admit_profile(&profile)?;
        for route in &profile.routes {
            ensure_canonical_dir(&route.artifact_directory)?;
        }
        Ok(profile)
    }
}

fn read_bounded_artifact(
    directory: &Path,
    filename: &str,
    max_bytes: u64,
) -> Result<Option<Vec<u8>>, ReadArtifactError> {
    ensure_canonical_dir(directory).map_err(|_| ReadArtifactError::UnsafePath)?;
    let relative = Path::new(filename);
    let mut components = relative.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(ReadArtifactError::UnsafePath);
    }
    let path = directory.join(relative);
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ReadArtifactError::Io),
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(ReadArtifactError::UnsafePath);
    }
    if meta.len() > max_bytes {
        return Err(ReadArtifactError::Oversized);
    }
    let bytes = open_regular_nofollow(&path, max_bytes)?;
    if path.parent() != Some(directory) {
        return Err(ReadArtifactError::UnsafePath);
    }
    Ok(Some(bytes))
}

fn json_error(status: u16, code: &'static str, obs_status: Option<&'static str>) -> ApiResponse {
    let body = match obs_status {
        Some(obs) => serde_json::json!({
            "code": code,
            "status": obs,
            "message": "Request could not be admitted"
        }),
        None => serde_json::json!({
            "code": code,
            "message": "Request could not be admitted"
        }),
    };
    ApiResponse {
        status,
        content_type: "application/json",
        body: body.to_string().into_bytes(),
        download_name: None,
        headers: vec![("Cache-Control", "no-store".to_owned())],
    }
}

fn json_ok(view: &ExecutionArtifactsView) -> ApiResponse {
    match serde_json::to_vec(view) {
        Ok(body) if body.len() <= MAX_RESPONSE_BYTES => ApiResponse {
            status: 200,
            content_type: "application/json",
            body,
            download_name: None,
            headers: vec![("Cache-Control", "no-store".to_owned())],
        },
        Ok(_) => json_error(503, "execution_artifacts_oversized", Some("unavailable")),
        Err(_) => json_error(503, "execution_artifacts_malformed", Some("unavailable")),
    }
}

fn lookup_exact_attempt(
    ledger: &AttemptLedger,
    query: &ArtifactQuery,
) -> Result<ImplementationAttempt, AdmissionError> {
    let Some(attempt) = ledger.get(&query.attempt) else {
        return Err(AdmissionError {
            status: 404,
            code: "execution_artifacts_attempt_not_found",
        });
    };
    if attempt.fence != query.fence || attempt.work_identity != query.work {
        return Err(AdmissionError {
            status: 409,
            code: "execution_artifacts_identity_mismatch",
        });
    }
    Ok(attempt.clone())
}

/// Serve `GET /api/implementation/execution-artifacts?attempt=...&fence=...&work=...`.
/// Fail closed on unauthorized/restricted viewers, unadmitted/unsafe routes,
/// stale/mismatched identities, symlinks/traversal, and oversized/malformed payloads.
#[must_use]
pub fn response(
    database: Option<&Path>,
    profile: Option<&ExecutionArtifactsProfile>,
    method: &str,
    target: &str,
    role: Role,
    restricted_projection: bool,
    body: &[u8],
) -> ApiResponse {
    let query = match admit_request(method, body, role, restricted_projection, target) {
        Ok(query) => query,
        Err(err) => return json_error(err.status, err.code, None),
    };
    let (Some(database), Some(profile)) = (database, profile) else {
        return json_error(503, "execution_artifacts_not_configured", Some("unavailable"));
    };
    if profile.implementation_database.as_path() != database {
        return json_error(503, "execution_artifacts_database_mismatch", Some("unavailable"));
    }
    let Ok(now_ms) = host_time_ms() else {
        return json_error(503, "clock_unavailable", Some("unavailable"));
    };
    let (_, ledger) = match ImplementationObservation::open(database)
        .and_then(|obs| obs.dispatch_recovery_ledgers())
    {
        Ok((_, Some(ledger))) => ((), ledger),
        Ok((_, None)) => {
            return json_error(404, "execution_artifacts_attempt_not_found", None);
        }
        Err(InventoryReadFailure::MalformedObservation) => {
            return json_error(503, "execution_artifacts_malformed", Some("unavailable"));
        }
        Err(_) => return json_error(503, "execution_artifacts_unavailable", Some("unavailable")),
    };
    let attempt = match lookup_exact_attempt(&ledger, &query) {
        Ok(attempt) => attempt,
        Err(err) => return json_error(err.status, err.code, Some("unavailable")),
    };
    evaluate_for_attempt(&attempt, profile, now_ms)
}

fn evaluate_for_attempt(
    attempt: &ImplementationAttempt,
    profile: &ExecutionArtifactsProfile,
    now_ms: u64,
) -> ApiResponse {
    match select_admitted_route(profile, attempt) {
        RouteSelection::Unavailable { reason } => json_ok(&unconfigured_or_unobserved_view(
            attempt,
            "unavailable",
            reason,
            None,
            now_ms,
            profile.bounds.freshness_window_ms,
        )),
        RouteSelection::Unsupported { execution, reason } => json_ok(&unconfigured_or_unobserved_view(
            attempt,
            "unsupported",
            reason,
            Some(execution),
            now_ms,
            profile.bounds.freshness_window_ms,
        )),
        RouteSelection::UnsafeIdentity => {
            json_error(503, "execution_artifacts_unsafe_path", Some("unavailable"))
        }
        RouteSelection::Admitted {
            route,
            execution: _,
            filename,
        } => {
            let bytes = match read_bounded_artifact(
                &route.artifact_directory,
                &filename,
                profile.bounds.max_artifact_bytes,
            ) {
                Ok(Some(bytes)) => bytes,
                Ok(None) => {
                    return json_ok(&unconfigured_or_unobserved_view(
                        attempt,
                        "unavailable",
                        "artifact_absent",
                        attempt.workflow.clone(),
                        now_ms,
                        profile.bounds.freshness_window_ms,
                    ));
                }
                Err(ReadArtifactError::UnsafePath) => {
                    return json_error(503, "execution_artifacts_unsafe_path", Some("unavailable"));
                }
                Err(ReadArtifactError::Oversized) => {
                    return json_error(503, "execution_artifacts_oversized", Some("unavailable"));
                }
                Err(ReadArtifactError::Io) => {
                    return json_error(503, "execution_artifacts_unavailable", Some("unavailable"));
                }
            };
            match project_artifact(&bytes, attempt, route, now_ms, &profile.bounds) {
                Ok(view) => json_ok(&view),
                Err(ProjectionError::IdentityMismatch) => {
                    json_error(409, "execution_artifacts_identity_mismatch", Some("unavailable"))
                }
                Err(ProjectionError::Malformed) => {
                    json_error(503, "execution_artifacts_malformed", Some("unavailable"))
                }
                Err(ProjectionError::Oversized) => {
                    json_error(503, "execution_artifacts_oversized", Some("unavailable"))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::implementation::{
        AcceptanceObligation, DispatchControls, DispatchWorkspace, ImplementationWork, SourceRef,
        TerminalPolicy, WorkBudget, WorkSourceKind, WorkspaceIsolation,
    };
    use kiln_core::types::ProviderProfileId;
    use kiln_implementer::Implementer;
    use serde_json::Value;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn id(ch: char) -> ContentId {
        ContentId::from_blake3_hex(std::iter::repeat_n(ch, 64).collect::<String>())
            .unwrap_or_else(|_| unreachable!())
    }

    fn ext_ref(
        ns: &str,
        kind: &str,
        native_id: &str,
    ) -> Result<ExternalRef, Box<dyn std::error::Error>> {
        ExternalRef::new(ns, kind, native_id)
            .map_err(|err| Box::<dyn std::error::Error>::from(std::io::Error::other(err.message)))
    }

    fn sample_route(dir: &Path) -> AdmittedArtifactRoute {
        AdmittedArtifactRoute {
            provider_namespace: "lattice".into(),
            native_kind: "workflow".into(),
            artifact_directory: dir.to_path_buf(),
            supported: true,
            supports_graph: true,
            supports_logs: true,
        }
    }

    fn sample_attempt(
        work: &ContentId,
        attempt: &ContentId,
        fence: u64,
        workflow: Option<ExternalRef>,
    ) -> Result<ImplementationAttempt, Box<dyn std::error::Error>> {
        let value = serde_json::json!({
            "schema": "kiln.implementation-attempt.v2",
            "identity": attempt,
            "canonical_key": work,
            "work_identity": work,
            "work_revision": 1,
            "ordinal": 1,
            "fence": fence,
            "lease_until_ms": 1_000_000,
            "phase": "executing",
            "workflow": workflow,
            "candidate": null,
            "failure": null,
            "cause": null,
            "superseded_by": null
        });
        Ok(serde_json::from_value(value)?)
    }

    fn sample_profile(db: &Path, dir: &Path) -> ExecutionArtifactsProfile {
        ExecutionArtifactsProfile {
            schema: PROFILE_SCHEMA.into(),
            implementation_database: db.to_path_buf(),
            routes: vec![sample_route(dir)],
            bounds: ArtifactBounds::default(),
        }
    }

    fn seed_ledger_db(
        db_path: &Path,
        workflow: Option<&ExternalRef>,
    ) -> Result<(ContentId, ContentId, u64), Box<dyn std::error::Error>> {
        let enrollment = kiln_core::implementation::Enrollment {
            repositories: BTreeSet::from(["onix/kiln".to_owned()]),
            actors: BTreeSet::from(["local:operator".to_owned()]),
        };
        let mut implementer =
            Implementer::open_with_local_operator(db_path, enrollment, "local:operator")?;
        let work = ImplementationWork {
            schema: kiln_core::implementation::IMPLEMENTATION_WORK_SCHEMA.into(),
            revision: 1,
            actor: "local:operator".into(),
            repository: "onix/kiln".into(),
            project: "kiln".into(),
            base: Some(ext_ref("git", "commit", &"11".repeat(20))?),
            title: "Stuck execution".into(),
            requested_outcome: "deliver".into(),
            acceptance: vec![AcceptanceObligation {
                id: "ci".into(),
                statement: "verified".into(),
            }],
            dependencies: Vec::new(),
            aliases: vec![SourceRef {
                kind: WorkSourceKind::CairnTask,
                source: ext_ref("cairn", "task", "stuck/t1")?,
                native_id: "stuck/t1".into(),
                version: id('1'),
            }],
            permitted_paths: vec!["crates".into()],
            permitted_effects: BTreeSet::from(["write_source".to_owned()]),
            verification_profile: id('9'),
            budget: WorkBudget {
                attempts: 3,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
        };
        let work_id = work
            .identity()
            .map_err(|err| std::io::Error::other(err.message))?;
        implementer.admit(&work, None)?;
        let claim = implementer
            .claim_next(1_000, kiln_core::implementation::DEFAULT_LEASE_MS)?
            .ok_or("missing claim")?;
        let plan = kiln_core::implementation::ImplementationDispatchPlan {
            schema: kiln_core::implementation::DISPATCH_PLAN_SCHEMA.into(),
            work_identity: work_id.clone(),
            work_revision: 1,
            attempt_identity: claim.identity.clone(),
            fence: claim.fence,
            repository: "onix/kiln".into(),
            base: claim.base.clone(),
            workspace: DispatchWorkspace {
                identity: id('d'),
                isolation: WorkspaceIsolation::IsolatedSnapshot,
                network: false,
                credentials: false,
                projection: None,
            },
            provider_profile: ProviderProfileId::new("lattice-default")
                .map_err(|err| std::io::Error::other(err.message))?,
            permitted_paths: vec!["crates".into()],
            permitted_effects: BTreeSet::from(["write_source".to_owned()]),
            controls: DispatchControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            deadline_minutes: 30,
            token_budget: 100_000,
            invocation_budget: 50,
            expected_output: id('f'),
            execution: None,
        };
        let reservation =
            implementer.reserve_dispatch_plan(&work, &claim.identity, claim.fence, 1_100, &plan)?;
        implementer.bind_campaign_request(&work, &claim.identity, claim.fence, 1_150, b"{}")?;
        implementer.begin_dispatch_transport(
            &claim.identity,
            claim.fence,
            1_200,
            &reservation.plan,
            None,
        )?;
        if let Some(workflow) = workflow {
            implementer.start_execution(&claim.identity, claim.fence, 1_300, workflow)?;
        }
        Ok((work_id, claim.identity, claim.fence))
    }

    // r[verify kiln.fleet.operator_plane.scenario.stuck]
    #[test]
    fn admits_valid_observed_graph_bounded_logs_freshness_and_redaction() -> TestResult {
        let work = id('a');
        let attempt_id = id('b');
        let execution = ext_ref("lattice", "workflow", "exec-42")?;
        let attempt = sample_attempt(&work, &attempt_id, 3, Some(execution.clone()))?;
        let route = sample_route(Path::new("/var/lib/kiln/artifacts"));
        let bounds = ArtifactBounds {
            freshness_window_ms: 60_000,
            ..ArtifactBounds::default()
        };
        let payload = serde_json::to_vec(&serde_json::json!({
            "schema": ARTIFACT_SCHEMA,
            "work_identity": work,
            "attempt_identity": attempt_id,
            "fence": 3,
            "execution": execution,
            "observed_at_ms": 100_000,
            "execution_state": "running",
            "graph": {
                "graph_identity": "graph-42",
                "nodes": [
                    { "id": "plan", "kind": "agent", "state": "completed", "terminal": true, "successful": true, "adapter": "local", "receipt": "rcpt-1" },
                    { "id": "verify", "kind": "test", "state": "running", "terminal": false, "successful": false }
                ],
                "edges": [
                    { "source": "plan", "target": "verify" }
                ]
            },
            "logs": {
                "redacted": false,
                "truncated": false,
                "entries": [
                    { "node_id": "plan", "stream": "stdout", "cursor": 0, "text": "Bearer secret-token-123\nok\n" },
                    { "node_id": "verify", "stream": "stderr", "cursor": 0, "text": "running tests" }
                ]
            }
        }))?;

        let fresh = project_artifact(&payload, &attempt, &route, 120_000, &bounds)
            .map_err(|err| format!("unexpected projection error: {err:?}"))?;
        assert_eq!(fresh.status, "available");
        assert_eq!(fresh.freshness, "fresh");
        assert_eq!(fresh.age_ms, Some(20_000));
        assert_eq!(fresh.graph.status, "available");
        assert_eq!(fresh.graph.nodes.len(), 2);
        assert_eq!(fresh.graph.edges.len(), 1);
        assert_eq!(fresh.logs.status, "available");
        assert!(fresh.logs.redacted);
        assert!(!fresh.logs.truncated);
        let first_entry = fresh.logs.entries.first().ok_or("missing first log entry")?;
        assert_eq!(first_entry.text, "Bearer [REDACTED]\nok\n");
        assert!(first_entry.redacted);

        let stale = project_artifact(&payload, &attempt, &route, 200_000, &bounds)
            .map_err(|err| format!("unexpected stale projection error: {err:?}"))?;
        assert_eq!(stale.freshness, "stale");
        assert_eq!(stale.age_ms, Some(100_000));
        Ok(())
    }

    // r[verify kiln.fleet.operator_plane.scenario.stuck]
    #[test]
    fn truncates_oversized_logs_and_sets_truncation_indicators() -> TestResult {
        let work = id('a');
        let attempt_id = id('b');
        let execution = ext_ref("lattice", "workflow", "exec-trunc")?;
        let attempt = sample_attempt(&work, &attempt_id, 1, Some(execution.clone()))?;
        let route = sample_route(Path::new("/var/lib/kiln/artifacts"));
        let bounds = ArtifactBounds {
            max_log_entries: 3,
            max_log_bytes: 10,
            ..ArtifactBounds::default()
        };
        let payload = serde_json::to_vec(&serde_json::json!({
            "schema": ARTIFACT_SCHEMA,
            "work_identity": work,
            "attempt_identity": attempt_id,
            "fence": 1,
            "execution": execution,
            "observed_at_ms": 5_000,
            "logs": {
                "entries": [
                    { "node_id": "n1", "stream": "stdout", "cursor": 0, "text": "123456" },
                    { "node_id": "n1", "stream": "stdout", "cursor": 6, "text": "abcdef" },
                    { "node_id": "n1", "stream": "stderr", "cursor": 0, "text": "more" }
                ]
            }
        }))?;
        let view = project_artifact(&payload, &attempt, &route, 6_000, &bounds)
            .map_err(|err| format!("unexpected projection error: {err:?}"))?;
        assert!(view.logs.truncated);
        assert_eq!(view.logs.retained_bytes, 10);
        assert_eq!(view.logs.retained_entries, 2);
        assert_eq!(view.logs.omitted_entries, 1);
        assert_eq!(view.logs.total_bytes, 16);
        let second = view.logs.entries.get(1).ok_or("missing truncated second entry")?;
        assert_eq!(second.text, "abcd");
        assert!(second.truncated);
        Ok(())
    }

    // r[verify kiln.fleet.operator_plane.scenario.stuck]
    #[test]
    fn refuses_crossed_identity_malformed_graph_and_malformed_log_payloads() -> TestResult {
        let work = id('a');
        let attempt_id = id('b');
        let other = id('c');
        let execution = ext_ref("lattice", "workflow", "exec-1")?;
        let wrong_exec = ext_ref("lattice", "workflow", "exec-other")?;
        let attempt = sample_attempt(&work, &attempt_id, 4, Some(execution.clone()))?;
        let route = sample_route(Path::new("/var/lib/kiln/artifacts"));
        let bounds = ArtifactBounds::default();

        for bad in [
            // Crossed work identity
            serde_json::json!({
                "schema": ARTIFACT_SCHEMA, "work_identity": other, "attempt_identity": attempt_id,
                "fence": 4, "execution": execution, "observed_at_ms": 100
            }),
            // Crossed attempt identity
            serde_json::json!({
                "schema": ARTIFACT_SCHEMA, "work_identity": work, "attempt_identity": other,
                "fence": 4, "execution": execution, "observed_at_ms": 100
            }),
            // Stale fence
            serde_json::json!({
                "schema": ARTIFACT_SCHEMA, "work_identity": work, "attempt_identity": attempt_id,
                "fence": 3, "execution": execution, "observed_at_ms": 100
            }),
            // Crossed execution identity
            serde_json::json!({
                "schema": ARTIFACT_SCHEMA, "work_identity": work, "attempt_identity": attempt_id,
                "fence": 4, "execution": wrong_exec, "observed_at_ms": 100
            }),
        ] {
            let bytes = serde_json::to_vec(&bad)?;
            assert_eq!(
                project_artifact(&bytes, &attempt, &route, 200, &bounds),
                Err(ProjectionError::IdentityMismatch)
            );
        }

        for malformed in [
            // Wrong schema
            serde_json::json!({
                "schema": "wrong.v1", "work_identity": work, "attempt_identity": attempt_id,
                "fence": 4, "execution": execution, "observed_at_ms": 100
            }),
            // Dangling graph edge
            serde_json::json!({
                "schema": ARTIFACT_SCHEMA, "work_identity": work, "attempt_identity": attempt_id,
                "fence": 4, "execution": execution, "observed_at_ms": 100,
                "graph": {
                    "nodes": [{ "id": "a", "kind": "agent", "state": "completed", "terminal": true, "successful": true }],
                    "edges": [{ "source": "a", "target": "missing" }]
                }
            }),
            // Contradictory node state (running + terminal=true)
            serde_json::json!({
                "schema": ARTIFACT_SCHEMA, "work_identity": work, "attempt_identity": attempt_id,
                "fence": 4, "execution": execution, "observed_at_ms": 100,
                "graph": {
                    "nodes": [{ "id": "a", "kind": "agent", "state": "running", "terminal": true, "successful": false }],
                    "edges": []
                }
            }),
            // Invalid log stream
            serde_json::json!({
                "schema": ARTIFACT_SCHEMA, "work_identity": work, "attempt_identity": attempt_id,
                "fence": 4, "execution": execution, "observed_at_ms": 100,
                "logs": { "entries": [{ "node_id": "a", "stream": "shell", "cursor": 0, "text": "hi" }] }
            }),
            // NUL byte in log text
            serde_json::json!({
                "schema": ARTIFACT_SCHEMA, "work_identity": work, "attempt_identity": attempt_id,
                "fence": 4, "execution": execution, "observed_at_ms": 100,
                "logs": { "entries": [{ "node_id": "a", "stream": "stdout", "cursor": 0, "text": "bad\u{0000}byte" }] }
            }),
        ] {
            let bytes = serde_json::to_vec(&malformed)?;
            assert_eq!(
                project_artifact(&bytes, &attempt, &route, 200, &bounds),
                Err(ProjectionError::Malformed)
            );
        }
        Ok(())
    }

    // r[verify kiln.fleet.operator_plane.scenario.stuck]
    #[test]
    fn unauthorized_restricted_and_query_admission_fail_closed() {
        let a = "aa".repeat(32);
        let w = "bb".repeat(32);
        let target = format!("{ROUTE}?attempt={a}&fence=2&work={w}");
        assert!(admit_request("GET", &[], Role::Operator, false, &target).is_ok());

        // Restricted projection refused even for Operator.
        assert_eq!(
            admit_request("GET", &[], Role::Operator, true, &target),
            Err(AdmissionError {
                status: 403,
                code: "execution_artifacts_forbidden"
            })
        );
        // Viewer and Worker refused.
        for role in [Role::Viewer, Role::Worker] {
            assert_eq!(
                admit_request("GET", &[], role, false, &target),
                Err(AdmissionError {
                    status: 403,
                    code: "execution_artifacts_forbidden"
                })
            );
        }
        // Non-GET or non-empty body refused.
        assert_eq!(
            admit_request("POST", &[], Role::Operator, false, &target),
            Err(AdmissionError {
                status: 405,
                code: "execution_artifacts_method_invalid"
            })
        );
        assert_eq!(
            admit_request("GET", b"{}", Role::Operator, false, &target),
            Err(AdmissionError {
                status: 405,
                code: "execution_artifacts_method_invalid"
            })
        );
        // Invalid query forms refused.
        for bad in [
            format!("{ROUTE}?attempt={a}&fence=0&work={w}"),
            format!("{ROUTE}?attempt={a}&fence=02&work={w}"),
            format!("{ROUTE}?attempt={}&fence=2&work={w}", a.to_uppercase()),
            format!("{ROUTE}?work={w}&attempt={a}&fence=2"),
            format!("{ROUTE}?attempt={a}&fence=2&work={w}&path=/etc/passwd"),
            format!("{ROUTE}?attempt={a}&fence=2&work={w}#frag"),
        ] {
            assert_eq!(
                admit_request("GET", &[], Role::Operator, false, &bad),
                Err(AdmissionError {
                    status: 400,
                    code: "execution_artifacts_query_invalid"
                }),
                "{bad}"
            );
        }
    }

    // r[verify kiln.fleet.operator_plane.scenario.stuck]
    #[test]
    fn http_route_serves_admitted_artifacts_and_refuses_unadmitted_symlinks_and_mismatches()
    -> TestResult {
        let root = tempfile::tempdir()?;
        let db = root.path().join("owner.sqlite");
        let artifacts_dir = root.path().join("artifacts");
        std::fs::create_dir(&artifacts_dir)?;
        let exec = ext_ref("lattice", "workflow", "exec-real-1")?;
        let (work, attempt, fence) = seed_ledger_db(&db, Some(&exec))?;
        let profile = sample_profile(&db, &artifacts_dir);
        let profile_path = root.path().join("artifacts-profile.json");
        std::fs::write(&profile_path, serde_json::to_vec(&profile)?)?;
        let loaded = ExecutionArtifactsProfile::load(&profile_path)?;

        let target = format!(
            "{ROUTE}?attempt={}&fence={fence}&work={}",
            attempt.as_str(),
            work.as_str()
        );

        // 1. Unconfigured profile fails closed with 503.
        let unconfigured = response(Some(&db), None, "GET", &target, Role::Operator, false, &[]);
        assert_eq!(unconfigured.status, 503);

        // 2. Absent artifact file returns explicit unavailable state.
        let absent = response(
            Some(&db),
            Some(&loaded),
            "GET",
            &target,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(absent.status, 200);
        let absent_json: Value = serde_json::from_slice(&absent.body)?;
        assert_eq!(absent_json["status"], "unavailable");
        assert_eq!(absent_json["reason"], "artifact_absent");
        assert_eq!(absent_json["graph"]["status"], "unavailable");
        assert_eq!(absent_json["logs"]["status"], "unavailable");

        // 3. Symlinked artifact fails closed with 503 and leaks nothing.
        let outside = root.path().join("outside.json");
        std::fs::write(&outside, b"{}")?;
        let artifact_file = artifacts_dir.join("execution-exec-real-1.json");
        std::os::unix::fs::symlink(&outside, &artifact_file)?;
        let symlink_res = response(
            Some(&db),
            Some(&loaded),
            "GET",
            &target,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(symlink_res.status, 503);
        let symlink_json: Value = serde_json::from_slice(&symlink_res.body)?;
        assert_eq!(symlink_json["code"], "execution_artifacts_unsafe_path");
        std::fs::remove_file(&artifact_file)?;

        // 4. Oversized artifact fails closed with 503.
        let mut oversized_profile = loaded.clone();
        oversized_profile.bounds.max_artifact_bytes = 64;
        std::fs::write(&artifact_file, vec![b' '; 65])?;
        let oversized_res = response(
            Some(&db),
            Some(&oversized_profile),
            "GET",
            &target,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(oversized_res.status, 503);
        let oversized_json: Value = serde_json::from_slice(&oversized_res.body)?;
        assert_eq!(oversized_json["code"], "execution_artifacts_oversized");

        // 5. Malformed artifact JSON fails closed with 503.
        std::fs::write(&artifact_file, b"{not-json")?;
        let malformed_res = response(
            Some(&db),
            Some(&loaded),
            "GET",
            &target,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(malformed_res.status, 503);
        let malformed_json: Value = serde_json::from_slice(&malformed_res.body)?;
        assert_eq!(malformed_json["code"], "execution_artifacts_malformed");

        // 6. Stale/mismatched execution identity inside artifact fails closed with 409.
        let mismatched_payload = serde_json::to_vec(&serde_json::json!({
            "schema": ARTIFACT_SCHEMA,
            "work_identity": work,
            "attempt_identity": attempt,
            "fence": fence,
            "execution": ext_ref("lattice", "workflow", "exec-stale")?,
            "observed_at_ms": host_time_ms().unwrap_or(10_000),
        }))?;
        std::fs::write(&artifact_file, mismatched_payload)?;
        let mismatch_res = response(
            Some(&db),
            Some(&loaded),
            "GET",
            &target,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(mismatch_res.status, 409);

        // 7. Stale fence query fails closed with 409.
        let stale_target = format!(
            "{ROUTE}?attempt={}&fence={}&work={}",
            attempt.as_str(),
            fence + 1,
            work.as_str()
        );
        let stale_query_res = response(
            Some(&db),
            Some(&loaded),
            "GET",
            &stale_target,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(stale_query_res.status, 409);

        // 8. Valid observed graph + bounded logs returns 200 with exact data.
        let now = host_time_ms().unwrap_or(10_000);
        let valid_payload = serde_json::to_vec(&serde_json::json!({
            "schema": ARTIFACT_SCHEMA,
            "work_identity": work,
            "attempt_identity": attempt,
            "fence": fence,
            "execution": exec,
            "observed_at_ms": now,
            "execution_state": "failed",
            "graph": {
                "nodes": [
                    { "id": "build", "kind": "agent", "state": "failed", "terminal": true, "successful": false }
                ],
                "edges": []
            },
            "logs": {
                "entries": [
                    { "node_id": "build", "stream": "stderr", "cursor": 0, "text": "compiler failed" }
                ]
            }
        }))?;
        std::fs::write(&artifact_file, valid_payload)?;
        let ok_res = response(
            Some(&db),
            Some(&loaded),
            "GET",
            &target,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(ok_res.status, 200);
        let ok_json: Value = serde_json::from_slice(&ok_res.body)?;
        assert_eq!(ok_json["status"], "available");
        assert_eq!(ok_json["graph"]["status"], "available");
        assert_eq!(ok_json["graph"]["nodes"][0]["id"], "build");
        assert_eq!(ok_json["logs"]["status"], "available");
        assert_eq!(ok_json["logs"]["entries"][0]["text"], "compiler failed");

        // 9. Restricted projection refused with 403 even with valid artifact on disk.
        let restricted = response(
            Some(&db),
            Some(&loaded),
            "GET",
            &target,
            Role::Operator,
            true,
            &[],
        );
        assert_eq!(restricted.status, 403);

        // 10. Unadmitted provider route returns explicit unsupported state without reading disk.
        let db2 = root.path().join("owner2.sqlite");
        let other_exec = ext_ref("other-provider", "session", "sess-9")?;
        let (work2, attempt2, fence2) = seed_ledger_db(&db2, Some(&other_exec))?;
        let profile2 = sample_profile(&db2, &artifacts_dir);
        let target2 = format!(
            "{ROUTE}?attempt={}&fence={fence2}&work={}",
            attempt2.as_str(),
            work2.as_str()
        );
        let unsupported_res = response(
            Some(&db2),
            Some(&profile2),
            "GET",
            &target2,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(unsupported_res.status, 200);
        let unsupported_json: Value = serde_json::from_slice(&unsupported_res.body)?;
        assert_eq!(unsupported_json["status"], "unsupported");
        assert_eq!(unsupported_json["reason"], "artifact_route_not_admitted");
        assert_eq!(unsupported_json["graph"]["status"], "unsupported");
        assert_eq!(unsupported_json["logs"]["status"], "unsupported");

        // 11. Attempt without observed workflow returns explicit unavailable state.
        let db3 = root.path().join("owner3.sqlite");
        let (work3, attempt3, fence3) = seed_ledger_db(&db3, None)?;
        let profile3 = sample_profile(&db3, &artifacts_dir);
        let target3 = format!(
            "{ROUTE}?attempt={}&fence={fence3}&work={}",
            attempt3.as_str(),
            work3.as_str()
        );
        let unstarted_res = response(
            Some(&db3),
            Some(&profile3),
            "GET",
            &target3,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(unstarted_res.status, 200);
        let unstarted_json: Value = serde_json::from_slice(&unstarted_res.body)?;
        assert_eq!(unstarted_json["status"], "unavailable");
        assert_eq!(unstarted_json["reason"], "execution_not_observed");

        // 12. Unsafe execution.native_id with path traversal fails closed.
        let mut unsafe_attempt = sample_attempt(
            &work,
            &attempt,
            fence,
            Some(ext_ref("lattice", "workflow", "../etc/passwd")?),
        )?;
        unsafe_attempt.workflow = Some(ext_ref("lattice", "workflow", "../etc/passwd")?);
        assert_eq!(
            select_admitted_route(&loaded, &unsafe_attempt),
            RouteSelection::UnsafeIdentity
        );

        // 13. Route capabilities mark unsupported graph/logs explicitly without leaking them.
        let mut partial_profile = loaded.clone();
        if let Some(first) = partial_profile.routes.get_mut(0) {
            first.supports_graph = false;
            first.supports_logs = true;
        }
        let partial_res = response(
            Some(&db),
            Some(&partial_profile),
            "GET",
            &target,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(partial_res.status, 200);
        let partial_json: Value = serde_json::from_slice(&partial_res.body)?;
        assert_eq!(partial_json["status"], "available");
        assert_eq!(partial_json["graph"]["status"], "unsupported");
        assert_eq!(partial_json["graph"]["reason"], "graph_unsupported");
        assert_eq!(partial_json["graph"]["nodes"], serde_json::json!([]));
        assert_eq!(partial_json["logs"]["status"], "available");

        // 14. Database mismatch fails closed with 503.
        let wrong_db = root.path().join("other.sqlite");
        let db_mismatch = response(
            Some(&wrong_db),
            Some(&loaded),
            "GET",
            &target,
            Role::Operator,
            false,
            &[],
        );
        assert_eq!(db_mismatch.status, 503);
        Ok(())
    }
}
