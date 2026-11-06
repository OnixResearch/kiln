use std::collections::BTreeMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use base64::Engine as _;
use kiln_core::source::SourceResolution;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::files;
use super::profile::{hex, WorkerProfile};

const MAX_CA_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConnection {
    pub service_origin: String,
    pub tls_ca_path: std::path::PathBuf,
    pub token_file: std::path::PathBuf,
    pub http_timeout_ms: u64,
    pub maximum_response_bytes: usize,
}

impl ServiceConnection {
    pub fn admit(&self) -> Result<(), String> {
        let url = reqwest::Url::parse(&self.service_origin).map_err(|_| "worker origin invalid")?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || self.service_origin.ends_with('/')
            || [&self.tls_ca_path, &self.token_file].iter().any(|path| {
                !path.is_absolute()
                    || path.components().any(|part| {
                        matches!(
                            part,
                            std::path::Component::ParentDir | std::path::Component::CurDir
                        )
                    })
            })
            || self.http_timeout_ms == 0
            || self.http_timeout_ms > 86_400_000
            || self.maximum_response_bytes == 0
            || self.maximum_response_bytes > super::profile::MAX_PROFILE_BYTES
        {
            return Err("worker connection outside admitted bounds".into());
        }
        Ok(())
    }
}

/// Read-only projection of the persisted Web execution authority.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionBinding {
    pub plan: kiln_core::workflow::WorkflowEffectPlan,
    pub binding: kiln_provider_api::WorkflowDispatchBinding,
    pub execution: Option<kiln_core::types::ExternalRef>,
    pub phase: String,
    pub outcome: kiln_core::domain::Outcome,
    /// Durable interactive cancellation intent for this exact attempt. Intent is
    /// not completed cancellation; the backend must still prove teardown.
    #[serde(default)]
    pub cancel_requested: bool,
    pub log_bounds: LogBounds,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunBinding {
    pub id: String,
    pub project_id: String,
    pub repository: Value,
    pub source_revision: String,
    pub canonical_tree: String,
    pub attempt_id: String,
    pub execution: Value,
    pub outcome: String,
    pub nodes: Vec<Node>,
    pub log_bounds: LogBounds,
    /// Scoped project root jobs start in; absent for unscoped projects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    /// Untrusted pull-request gate projection; absent for trusted runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untrusted: Option<Value>,
}
impl RunBinding {
    // r[impl kiln.untrusted.execution]
    /// Untrusted bindings run only on declared workers and never restore or
    /// publish cache entries.
    pub fn is_untrusted(&self) -> bool {
        self.untrusted.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogBounds {
    pub maximum_chunk_bytes: usize,
    pub maximum_total_bytes: usize,
    pub maximum_chunks: usize,
}

#[derive(Default)]
pub(super) struct LogUploads<'node> {
    cursors: BTreeMap<&'node str, [Option<u64>; 2]>,
    total_bytes: usize,
    chunks: usize,
}

impl LogUploads<'_> {
    pub(super) fn reserve(&mut self, bounds: &LogBounds, bytes: usize) -> Result<(), String> {
        if bounds.maximum_chunk_bytes == 0 {
            return Err("worker log chunk bound invalid".into());
        }
        if bytes > bounds.maximum_chunk_bytes {
            return Err("worker log chunk exceeds bound".into());
        }
        let total_bytes = self
            .total_bytes
            .checked_add(bytes)
            .ok_or("worker log byte total overflow")?;
        let chunks = self
            .chunks
            .checked_add(1)
            .ok_or("worker log chunk total overflow")?;
        if total_bytes > bounds.maximum_total_bytes || chunks > bounds.maximum_chunks {
            return Err("worker captured logs exceed run bounds".into());
        }
        self.total_bytes = total_bytes;
        self.chunks = chunks;
        Ok(())
    }
}

#[derive(Serialize)]
struct LogChunk<'a> {
    attempt_id: &'a str,
    execution: &'a Value,
    source_revision: &'a str,
    node_id: &'a str,
    stream: &'a str,
    cursor: u64,
    body_base64: String,
    blake3: String,
    truncated: bool,
}

impl ExecutionBinding {
    pub(crate) fn maximum_upload_chunk_bytes(
        &self,
        maximum_request_bytes: usize,
    ) -> Result<usize, String> {
        let execution = serde_json::to_value(&self.execution)
            .map_err(|_| "worker execution encoding failed")?;
        let mut envelope = Vec::new();
        let mut maximum_envelope_bytes = 0;
        // Both stream names have the same wire length. Reserve the largest
        // cursor and boolean encodings; base64 itself needs no JSON escaping.
        let mut chunk = LogChunk {
            attempt_id: self.plan.attempt_id.as_str(),
            execution: &execution,
            source_revision: self.binding.source.exact_revision.native_id.as_str(),
            node_id: "",
            stream: "stdout",
            cursor: u64::MAX,
            body_base64: String::new(),
            blake3: blake3::hash(b"").to_hex().to_string(),
            truncated: false,
        };
        for node in &self.plan.nodes {
            envelope.clear();
            chunk.node_id = node.id.as_str();
            serde_json::to_writer(&mut envelope, &chunk)
                .map_err(|_| "worker log envelope encoding failed")?;
            maximum_envelope_bytes = maximum_envelope_bytes.max(envelope.len());
        }
        Ok(self
            .plan
            .bounds
            .max_log_read_bytes
            .min(maximum_request_bytes.saturating_sub(maximum_envelope_bytes) / 4 * 3))
    }
}

#[derive(Deserialize)]
struct Runs {
    runs: Vec<Value>,
}

pub struct ServiceClient {
    pub(super) client: reqwest::blocking::Client,
    pub(super) origin: String,
    pub(super) authorization: reqwest::header::HeaderValue,
    pub(super) maximum_response_bytes: usize,
    http_timeout: Duration,
}
impl std::fmt::Debug for ServiceClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServiceClient")
            .field("origin", &self.origin)
            .field("maximum_response_bytes", &self.maximum_response_bytes)
            .finish_non_exhaustive()
    }
}

impl ServiceClient {
    pub(super) fn new(profile: &WorkerProfile) -> Result<Self, String> {
        Self::connect(&ServiceConnection {
            service_origin: profile.service_origin.clone(),
            tls_ca_path: profile.tls_ca_path.clone(),
            token_file: profile.token_file.clone(),
            http_timeout_ms: profile.http_timeout_ms,
            maximum_response_bytes: profile.maximum_response_bytes,
        })
    }

    pub fn connect(profile: &ServiceConnection) -> Result<Self, String> {
        profile.admit()?;
        let ca = files::regular(&profile.tls_ca_path, MAX_CA_BYTES, false)?;
        let ca = reqwest::Certificate::from_pem(&ca).map_err(|_| "worker CA invalid")?;
        let token = files::regular(&profile.token_file, 64, true)?;
        let token = std::str::from_utf8(&token).map_err(|_| "worker credential invalid")?;
        if !hex(token, 32) {
            return Err("worker credential invalid".into());
        }
        let mut authorization = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| "worker credential invalid")?;
        authorization.set_sensitive(true);
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .tls_built_in_root_certs(false)
            .add_root_certificate(ca)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .timeout(Duration::from_millis(profile.http_timeout_ms))
            .build()
            .map_err(|_| "worker HTTPS client unavailable")?;
        Ok(Self {
            client,
            origin: profile.service_origin.clone(),
            authorization,
            maximum_response_bytes: profile.maximum_response_bytes,
            http_timeout: Duration::from_millis(profile.http_timeout_ms),
        })
    }

    pub fn execution_binding(&self, run_id: &str) -> Result<ExecutionBinding, String> {
        if !super::profile::component(run_id) {
            return Err("worker run identity invalid".into());
        }
        let response = self
            .client
            .get(format!(
                "{}/api/runs/{run_id}/execution-binding",
                self.origin
            ))
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .send()
            .map_err(|error| transport_error(&error, "worker execution binding lookup"))?;
        let bound: ExecutionBinding = serde_json::from_value(self.response(response)?)
            .map_err(|_| "worker execution binding invalid")?;
        if bound.binding.run_id != run_id {
            return Err("worker execution binding identity mismatch".into());
        }
        Ok(bound)
    }

    pub fn lookup_execution_binding(
        &self,
        project_id: &str,
        context: &super::source::SourceContext,
        node_id: &str,
        timeout: Duration,
    ) -> Result<Option<ExecutionBinding>, String> {
        let started = Instant::now();
        if timeout.is_zero() {
            return Err("worker execution lookup deadline expired".into());
        }
        if !super::profile::component(project_id)
            || !super::profile::component(node_id)
            || context.provider != "git"
            || !hex(&context.repository, 32)
            || !hex(&context.revision, 20)
            || context.revision.bytes().all(|b| b == b'0')
        {
            return Err("worker source context invalid".into());
        }
        let response = self
            .client
            .get(format!("{}/api/runs", self.origin))
            .query(&[
                ("project_id", project_id),
                ("source_revision", context.revision.as_str()),
            ])
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .timeout(timeout.min(self.http_timeout))
            .send()
            .map_err(|error| transport_error(&error, "worker run lookup"))?;
        let runs: Runs = serde_json::from_value(self.response(response)?)
            .map_err(|_| "worker run lookup invalid")?;
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err("worker execution lookup deadline expired".into());
        }
        if runs.runs.len() > 1 {
            return Err("worker run identity is ambiguous".into());
        }
        let Some(run) = runs.runs.first() else {
            return Ok(None);
        };
        let id = run
            .get("id")
            .and_then(Value::as_str)
            .ok_or("worker run identity missing")?;
        if !super::profile::component(id) {
            return Err("worker run identity invalid".into());
        }
        let response = self
            .client
            .get(format!("{}/api/runs/{id}/execution-binding", self.origin))
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .timeout(remaining.min(self.http_timeout))
            .send()
            .map_err(|error| transport_error(&error, "worker execution binding lookup"))?;
        let bound: ExecutionBinding = serde_json::from_value(self.response(response)?)
            .map_err(|_| "worker execution binding invalid")?;
        if started.elapsed() >= timeout {
            return Err("worker execution lookup deadline expired".into());
        }
        if bound.binding.run_id != id {
            return Err("worker execution binding identity mismatch".into());
        }
        if bound.execution.is_none() {
            return Ok(None);
        }
        admit_execution_binding(&bound, node_id)?;
        let source = bound
            .plan
            .source
            .as_ref()
            .ok_or("worker execution source missing")?;
        if bound.binding.project_id != project_id
            || source.provider.as_str() != context.provider
            || source.repository.provider_namespace.as_str() != "git"
            || source.repository.native_kind.as_str() != "repository"
            || source.repository.native_id.as_str() != context.repository
            || source.revision.native_id.as_str() != context.revision
            || run.get("execution")
                != Some(
                    &serde_json::to_value(&bound.execution)
                        .map_err(|_| "worker execution encoding failed")?,
                )
        {
            return Err("worker active run source or execution binding mismatch".into());
        }
        Ok(Some(bound))
    }

    pub fn upload_execution_logs(
        &self,
        bound: &ExecutionBinding,
        node_id: &str,
        stdout: &[u8],
        stderr: &[u8],
        stdout_truncated: bool,
        stderr_truncated: bool,
    ) -> Result<(), String> {
        admit_execution_binding(bound, node_id)?;
        let chunks = execution_log_chunks(
            &bound.log_bounds,
            stdout,
            stderr,
            stdout_truncated,
            stderr_truncated,
        )?;
        let current = self.execution_binding(&bound.binding.run_id)?;
        if &current != bound {
            return Err("worker attempt or execution became stale".into());
        }
        let execution = serde_json::to_value(&bound.execution)
            .map_err(|_| "worker execution encoding failed")?;
        for ((stream, bytes, truncated), count) in [
            ("stdout", stdout, stdout_truncated),
            ("stderr", stderr, stderr_truncated),
        ]
        .into_iter()
        .zip(chunks)
        {
            for index in 0..count {
                let start = index
                    .checked_mul(bound.log_bounds.maximum_chunk_bytes)
                    .ok_or("worker log cursor overflow")?;
                let end = start
                    .saturating_add(bound.log_bounds.maximum_chunk_bytes)
                    .min(bytes.len());
                let bytes = bytes.get(start..end).ok_or("worker log chunk invalid")?;
                let cursor = u64::try_from(start).map_err(|_| "worker log cursor overflow")?;
                let next_cursor = u64::try_from(end).map_err(|_| "worker log cursor overflow")?;
                let request = LogChunk {
                    attempt_id: bound.plan.attempt_id.as_str(),
                    execution: &execution,
                    source_revision: bound.binding.source.exact_revision.native_id.as_str(),
                    node_id,
                    stream,
                    cursor,
                    body_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                    blake3: blake3::hash(bytes).to_hex().to_string(),
                    truncated: truncated && index + 1 == count,
                };
                let response = self
                    .client
                    .post(format!(
                        "{}/api/runs/{}/logs",
                        self.origin, bound.binding.run_id
                    ))
                    .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
                    .header(reqwest::header::ORIGIN, &self.origin)
                    .json(&request)
                    .send()
                    .map_err(|_| "worker log upload failed")?;
                if self
                    .response(response)?
                    .get("next_cursor")
                    .and_then(Value::as_u64)
                    != Some(next_cursor)
                {
                    return Err("worker log upload cursor mismatch".into());
                }
            }
        }
        Ok(())
    }

    pub(super) fn response(&self, response: reqwest::blocking::Response) -> Result<Value, String> {
        if !response.status().is_success() {
            return Err("worker service rejected request".into());
        }
        if response
            .content_length()
            .is_some_and(|n| n > self.maximum_response_bytes as u64)
        {
            return Err("worker service response exceeds bound".into());
        }
        let mut bytes = Vec::new();
        response
            .take(self.maximum_response_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "worker service response failed")?;
        if bytes.len() > self.maximum_response_bytes {
            return Err("worker service response exceeds bound".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "worker service response invalid".into())
    }

    pub(super) fn lookup(
        &self,
        profile: &WorkerProfile,
        source: &SourceResolution,
    ) -> Result<Option<RunBinding>, String> {
        self.lookup_with_timeout(
            profile,
            source,
            Duration::from_millis(profile.http_timeout_ms),
        )
    }

    fn lookup_with_timeout(
        &self,
        profile: &WorkerProfile,
        source: &SourceResolution,
        timeout: Duration,
    ) -> Result<Option<RunBinding>, String> {
        let response = self
            .client
            .get(format!("{}/api/runs", self.origin))
            .query(&[
                ("project_id", profile.project_id.as_str()),
                ("source_revision", source.exact_revision.native_id.as_str()),
            ])
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .timeout(timeout)
            .send()
            .map_err(|error| transport_error(&error, "worker run lookup"))?;
        let runs: Runs = serde_json::from_value(self.response(response)?)
            .map_err(|_| "worker run lookup invalid")?;
        if runs.runs.len() > 1 {
            return Err("worker run identity is ambiguous".into());
        }
        let Some(value) = runs.runs.into_iter().next() else {
            return Ok(None);
        };
        if value.get("execution").is_none_or(Value::is_null)
            || value.get("attempt_id").is_none_or(Value::is_null)
        {
            return Ok(None);
        }
        let run: RunBinding =
            serde_json::from_value(value).map_err(|_| "worker run binding invalid")?;
        admit_binding(profile, source, &run)?;
        Ok(Some(run))
    }

    pub(super) fn await_binding(
        &self,
        profile: &WorkerProfile,
        source: &SourceResolution,
        cancellation: &AtomicBool,
    ) -> Result<RunBinding, String> {
        let deadline = Instant::now() + Duration::from_millis(profile.lookup_timeout_ms);
        // Only initial dispatch acceptance may lag behind job startup. Failures
        // are not retried and no mutation is retried.
        let maximum_attempts = profile.lookup_timeout_ms / profile.poll_interval_ms + 1;
        for _ in 0..maximum_attempts {
            if cancellation.load(Ordering::SeqCst) {
                return Err("worker cancelled before run binding".into());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("worker active run was not admitted before deadline".into());
            }
            let result = self.lookup_with_timeout(
                profile,
                source,
                remaining.min(Duration::from_millis(profile.http_timeout_ms)),
            );
            if cancellation.load(Ordering::SeqCst) {
                return Err("worker cancelled before run binding".into());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("worker active run was not admitted before deadline".into());
            }
            if let Some(run) = result? {
                return Ok(run);
            }
            std::thread::sleep(remaining.min(Duration::from_millis(profile.poll_interval_ms)));
        }
        Err("worker active run was not admitted before deadline".into())
    }

    pub(super) fn upload(
        &self,
        profile: &WorkerProfile,
        source: &SourceResolution,
        binding: &RunBinding,
        node_id: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        let current = self
            .lookup(profile, source)?
            .ok_or("worker active run missing")?;
        exact_binding(binding, &current)?;
        if !current.nodes.iter().any(|node| node.id == node_id) {
            return Err("worker artifact node binding invalid".into());
        }
        let request = serde_json::json!({
            "attempt_id": binding.attempt_id, "execution": binding.execution,
            "source_revision": binding.source_revision, "node_id": node_id,
            "name": name, "body_base64": base64::engine::general_purpose::STANDARD.encode(bytes),
            "blake3": blake3::hash(bytes).to_hex().to_string(),
        });
        let response = self
            .client
            .post(format!("{}/api/runs/{}/artifacts", self.origin, binding.id))
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .header(reqwest::header::ORIGIN, &self.origin)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&request).map_err(|_| "worker artifact encoding failed")?)
            .send()
            .map_err(|_| "worker artifact upload failed")?;
        self.response(response)?;
        Ok(())
    }

    pub(super) fn upload_log_chunk<'node>(
        &self,
        profile: &WorkerProfile,
        source: &SourceResolution,
        binding: &RunBinding,
        node_id: &'node str,
        chunk: &super::streaming::Chunk,
        progress: &mut LogUploads<'node>,
    ) -> Result<(), String> {
        if chunk.bytes.is_empty()
            && !chunk.truncated
            && progress
                .cursors
                .get(node_id)
                .is_some_and(|[stdout, stderr]| match chunk.stream {
                    super::streaming::Stream::Stdout => stdout.is_some(),
                    super::streaming::Stream::Stderr => stderr.is_some(),
                })
        {
            return Ok(());
        }
        progress.reserve(&binding.log_bounds, chunk.bytes.len())?;
        let current = self
            .lookup(profile, source)?
            .ok_or("worker active run missing")?;
        exact_binding(binding, &current)?;
        if !current.nodes.iter().any(|node| node.id == node_id) {
            return Err("worker log node binding invalid".into());
        }
        let [stdout, stderr] = progress.cursors.entry(node_id).or_default();
        let cursor = match chunk.stream {
            super::streaming::Stream::Stdout => stdout,
            super::streaming::Stream::Stderr => stderr,
        };
        let next_cursor = cursor
            .unwrap_or_default()
            .checked_add(
                u64::try_from(chunk.bytes.len()).map_err(|_| "worker log cursor overflow")?,
            )
            .ok_or("worker log cursor overflow")?;
        let request = LogChunk {
            attempt_id: &binding.attempt_id,
            execution: &binding.execution,
            source_revision: &binding.source_revision,
            node_id,
            stream: chunk.stream.name(),
            cursor: cursor.unwrap_or_default(),
            body_base64: base64::engine::general_purpose::STANDARD.encode(&chunk.bytes),
            blake3: blake3::hash(&chunk.bytes).to_hex().to_string(),
            truncated: chunk.truncated,
        };
        let response = self
            .client
            .post(format!("{}/api/runs/{}/logs", self.origin, binding.id))
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .header(reqwest::header::ORIGIN, &self.origin)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&request).map_err(|_| "worker log encoding failed")?)
            .send()
            .map_err(|_| "worker log upload failed")?;
        let response = self.response(response)?;
        if response.get("next_cursor").and_then(Value::as_u64) != Some(next_cursor) {
            return Err("worker log upload cursor mismatch".into());
        }
        *cursor = Some(next_cursor);
        Ok(())
    }
}

fn transport_error(error: &reqwest::Error, operation: &'static str) -> String {
    if error.is_timeout() {
        format!("{operation} timed out")
    } else if error.is_connect() {
        format!("{operation} connect failed")
    } else {
        format!("{operation} failed")
    }
}

fn admit_execution_binding(bound: &ExecutionBinding, node_id: &str) -> Result<(), String> {
    kiln_core::workflow::validate_workflow_plan(&bound.plan)
        .map_err(|_| "worker workflow plan invalid")?;
    kiln_core::source::validate_resolution(&bound.binding.source)
        .map_err(|_| "worker persisted source binding invalid")?;
    let source = bound
        .plan
        .source
        .as_ref()
        .ok_or("worker execution source missing")?;
    let execution = bound
        .execution
        .as_ref()
        .ok_or("worker native execution missing")?;
    execution
        .validate()
        .map_err(|_| "worker native execution invalid")?;
    if bound.phase != "active"
        || bound.cancel_requested
        || bound.outcome != kiln_core::domain::Outcome::Pending
        || execution.provider_namespace.as_str() != "lattice"
        || !super::profile::component(&bound.binding.run_id)
        || !super::profile::component(&bound.binding.project_id)
        || source.revision.provider_namespace
            != bound.binding.source.exact_revision.provider_namespace
        || source.revision.native_id != bound.binding.source.exact_revision.native_id
        || bound.binding.source.exact_revision.native_kind.as_str() != "commit"
        || source.repository != bound.binding.source.repository
        || !bound
            .plan
            .nodes
            .iter()
            .any(|node| node.id.as_str() == node_id)
    {
        return Err("worker active execution binding mismatch".into());
    }
    Ok(())
}

fn execution_log_chunks(
    bounds: &LogBounds,
    stdout: &[u8],
    stderr: &[u8],
    stdout_truncated: bool,
    stderr_truncated: bool,
) -> Result<[usize; 2], String> {
    if bounds.maximum_chunk_bytes == 0
        || bounds.maximum_total_bytes == 0
        || bounds.maximum_chunks == 0
        || bounds.maximum_chunk_bytes > bounds.maximum_total_bytes
    {
        return Err("worker log bounds invalid".into());
    }
    let total = stdout
        .len()
        .checked_add(stderr.len())
        .ok_or("worker log byte total overflow")?;
    // Empty streams need no reference unless they carry a truncation marker.
    // An absent chunk is not a claim that a stream or execution completed.
    let stdout_chunks = stdout
        .len()
        .div_ceil(bounds.maximum_chunk_bytes)
        .max(usize::from(stdout_truncated));
    let stderr_chunks = stderr
        .len()
        .div_ceil(bounds.maximum_chunk_bytes)
        .max(usize::from(stderr_truncated));
    if total > bounds.maximum_total_bytes
        || stdout_chunks
            .checked_add(stderr_chunks)
            .is_none_or(|count| count > bounds.maximum_chunks)
    {
        return Err("worker captured logs exceed run bounds".into());
    }
    Ok([stdout_chunks, stderr_chunks])
}

// r[impl kiln.web.remote_workers.source]
// r[impl kiln.web.remote_workers.source.scenario.denied]
pub fn admit_binding(
    profile: &WorkerProfile,
    source: &SourceResolution,
    run: &RunBinding,
) -> Result<(), String> {
    if run
        .repository
        .as_object()
        .is_none_or(|fields| fields.len() != 4)
    {
        return Err("worker repository reference invalid".into());
    }
    let repository = kiln_core::types::ExternalRef::deserialize(&run.repository)
        .map_err(|_| "worker repository reference invalid")?;
    repository
        .validate()
        .map_err(|_| "worker repository reference invalid")?;
    let execution = kiln_core::types::ExternalRef::deserialize(&run.execution)
        .map_err(|_| "worker execution reference invalid")?;
    execution
        .validate()
        .map_err(|_| "worker execution reference invalid")?;
    if !super::profile::component(&run.id)
        || run.project_id != profile.project_id
        || repository.provider_namespace.as_str() != "git"
        || repository.native_kind.as_str() != "repository"
        || repository.native_id.as_str() != profile.source_repository
        || run.source_revision != source.exact_revision.native_id.as_str()
        || run.canonical_tree != source.canonical_tree.as_str()
        || run.attempt_id.is_empty()
        || run.attempt_id.len() > 256
        || run.outcome != "Pending"
        || run.log_bounds.maximum_chunk_bytes == 0
        || run.log_bounds.maximum_total_bytes == 0
        || run.log_bounds.maximum_chunks == 0
        || run.log_bounds.maximum_chunk_bytes > run.log_bounds.maximum_total_bytes
        || execution.provider_namespace.as_str() != "lattice"
        || run.nodes.is_empty()
        || run
            .nodes
            .iter()
            .any(|node| !super::profile::component(&node.id))
        || !run
            .nodes
            .iter()
            .any(|node| profile.steps.iter().any(|step| step.node_id == node.id))
        || run.nodes.iter().enumerate().any(|(index, node)| {
            run.nodes
                .iter()
                .take(index)
                .any(|prior| prior.id == node.id)
        })
    {
        return Err("worker active run source or node binding mismatch".into());
    }
    Ok(())
}

pub fn exact_binding(expected: &RunBinding, current: &RunBinding) -> Result<(), String> {
    if current.id != expected.id
        || current.attempt_id != expected.attempt_id
        || current.execution != expected.execution
        || current.source_revision != expected.source_revision
        || current.repository != expected.repository
        || current.canonical_tree != expected.canonical_tree
        || current.project_id != expected.project_id
        || current.log_bounds != expected.log_bounds
        || current.nodes != expected.nodes
        || current.project_root != expected.project_root
        || current.outcome != "Pending"
    {
        return Err("worker attempt or execution became stale".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "client/deadline_tests.rs"]
mod deadline_tests;
