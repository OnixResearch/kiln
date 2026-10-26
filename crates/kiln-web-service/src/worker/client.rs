use std::collections::BTreeMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use base64::Engine as _;
use kiln_core::source::SourceResolution;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::files;
use super::profile::{WorkerProfile, hex};

const MAX_CA_BYTES: usize = 1_048_576;

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
pub struct LogUploads<'node> {
    cursors: BTreeMap<&'node str, [u64; 2]>,
    total_bytes: usize,
    chunks: usize,
}

impl LogUploads<'_> {
    pub fn reserve(
        &mut self,
        bounds: &LogBounds,
        output: &super::process::StepOutput,
    ) -> Result<(), String> {
        if bounds.maximum_chunk_bytes == 0 {
            return Err("worker log chunk bound invalid".into());
        }
        let mut total_bytes = self.total_bytes;
        let mut chunks = self.chunks;
        for capture in [&output.stdout, &output.stderr] {
            total_bytes = total_bytes
                .checked_add(capture.bytes.len())
                .ok_or("worker log byte total overflow")?;
            chunks = chunks
                .checked_add(
                    capture
                        .bytes
                        .len()
                        .div_ceil(bounds.maximum_chunk_bytes)
                        .max(1),
                )
                .ok_or("worker log chunk total overflow")?;
        }
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

#[derive(Deserialize)]
struct Runs {
    runs: Vec<Value>,
}

pub struct ServiceClient {
    client: reqwest::blocking::Client,
    origin: String,
    authorization: reqwest::header::HeaderValue,
    maximum_response_bytes: usize,
}

impl ServiceClient {
    pub fn new(profile: &WorkerProfile) -> Result<Self, String> {
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
            .no_proxy()
            .timeout(Duration::from_millis(profile.http_timeout_ms))
            .build()
            .map_err(|_| "worker HTTPS client unavailable")?;
        Ok(Self {
            client,
            origin: profile.service_origin.clone(),
            authorization,
            maximum_response_bytes: profile.maximum_response_bytes,
        })
    }

    fn response(&self, response: reqwest::blocking::Response) -> Result<Value, String> {
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

    pub fn lookup(
        &self,
        profile: &WorkerProfile,
        source: &SourceResolution,
    ) -> Result<Option<RunBinding>, String> {
        let response = self
            .client
            .get(format!("{}/api/runs", self.origin))
            .query(&[
                ("project_id", profile.project_id.as_str()),
                ("source_revision", source.exact_revision.native_id.as_str()),
            ])
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .send()
            .map_err(|_| "worker run lookup failed")?;
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

    pub fn await_binding(
        &self,
        profile: &WorkerProfile,
        source: &SourceResolution,
        cancellation: &AtomicBool,
    ) -> Result<RunBinding, String> {
        let start = Instant::now();
        let deadline = Duration::from_millis(profile.lookup_timeout_ms);
        // Only initial dispatch acceptance may lag behind job startup. Failures
        // are not retried and no mutation is retried.
        let maximum_attempts = profile.lookup_timeout_ms / profile.poll_interval_ms + 1;
        for _ in 0..maximum_attempts {
            if cancellation.load(Ordering::SeqCst) {
                return Err("worker cancelled before run binding".into());
            }
            if let Some(run) = self.lookup(profile, source)? {
                return Ok(run);
            }
            let remaining = deadline.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err("worker active run was not admitted before deadline".into());
            }
            std::thread::sleep(remaining.min(Duration::from_millis(profile.poll_interval_ms)));
        }
        Err("worker active run was not admitted before deadline".into())
    }

    pub fn upload(
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

    pub fn upload_logs<'node>(
        &self,
        profile: &WorkerProfile,
        source: &SourceResolution,
        binding: &RunBinding,
        node_id: &'node str,
        output: &super::process::StepOutput,
        progress: &mut LogUploads<'node>,
    ) -> Result<(), String> {
        progress.reserve(&binding.log_bounds, output)?;
        let current = self
            .lookup(profile, source)?
            .ok_or("worker active run missing")?;
        exact_binding(binding, &current)?;
        if !current.nodes.iter().any(|node| node.id == node_id) {
            return Err("worker log node binding invalid".into());
        }
        let [stdout_cursor, stderr_cursor] = progress.cursors.entry(node_id).or_default();
        for (stream, capture, cursor) in [
            ("stdout", &output.stdout, stdout_cursor),
            ("stderr", &output.stderr, stderr_cursor),
        ] {
            let mut remaining = capture.bytes.as_slice();
            loop {
                // The first iteration also records a truthfully empty stream.
                let (bytes, rest) =
                    remaining.split_at(remaining.len().min(binding.log_bounds.maximum_chunk_bytes));
                let length =
                    u64::try_from(bytes.len()).map_err(|_| "worker log cursor overflow")?;
                let next_cursor = cursor
                    .checked_add(length)
                    .ok_or("worker log cursor overflow")?;
                let request = LogChunk {
                    attempt_id: &binding.attempt_id,
                    execution: &binding.execution,
                    source_revision: &binding.source_revision,
                    node_id,
                    stream,
                    cursor: *cursor,
                    body_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                    blake3: blake3::hash(bytes).to_hex().to_string(),
                    truncated: capture.truncated && rest.is_empty(),
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
                *cursor = next_cursor;
                if rest.is_empty() {
                    break;
                }
                remaining = rest;
            }
        }
        Ok(())
    }
}

pub fn admit_binding(
    profile: &WorkerProfile,
    source: &SourceResolution,
    run: &RunBinding,
) -> Result<(), String> {
    let repository = serde_json::to_value(&source.repository)
        .map_err(|_| "worker repository encoding failed")?;
    let execution: kiln_core::types::ExternalRef = serde_json::from_value(run.execution.clone())
        .map_err(|_| "worker execution reference invalid")?;
    execution
        .validate()
        .map_err(|_| "worker execution reference invalid")?;
    if !super::profile::component(&run.id)
        || run.project_id != profile.project_id
        || run.repository != repository
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
        || profile
            .steps
            .iter()
            .any(|step| !run.nodes.iter().any(|node| node.id == step.node_id))
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
        || current.outcome != "Pending"
    {
        return Err("worker attempt or execution became stale".into());
    }
    Ok(())
}
