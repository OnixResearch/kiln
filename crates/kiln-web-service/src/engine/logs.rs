use super::api::{error, json_response};
use super::model::{LogChunk, RunRecord};
use super::{ApiResponse, Engine, storage};
use base64::Engine as _;
use kiln_core::types::{AttemptId, ExternalRef};
use kiln_core::workflow::WorkflowState;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Upload {
    attempt_id: AttemptId,
    #[serde(deserialize_with = "super::api::deserialize_execution")]
    execution: ExternalRef,
    source_revision: String,
    node_id: String,
    stream: String,
    cursor: u64,
    body_base64: String,
    blake3: String,
    truncated: bool,
}
fn active_binding(run: &RunRecord, upload: &Upload) -> bool {
    run.plan.attempt_id == upload.attempt_id
        && run.source.exact_revision.native_id.as_str() == upload.source_revision
        && run.phase == "active"
        && run.error_code.is_none()
        && run.observation.as_ref().is_some_and(|observation| {
            observation.execution == upload.execution
                && !observation.terminal
                && observation.state != WorkflowState::Unknown
        })
        && run
            .plan
            .nodes
            .iter()
            .any(|node| node.id.as_str() == upload.node_id)
}
/// Validate metadata independently of storage, both before persistence and on recovery/read.
pub(super) fn validate(run: &RunRecord) -> Result<(), String> {
    let bounds = &run.plan.bounds;
    if run.logs.len() > bounds.max_log_references {
        return Err("log chunk retention exceeded".into());
    }
    let mut total = 0usize;
    let mut streams = std::collections::BTreeMap::<(&str, &str), (u64, bool)>::new();
    let mut ids = std::collections::BTreeSet::new();
    for chunk in &run.logs {
        if !storage::hex_identity(&chunk.id)
            || !storage::hex_identity(&chunk.blake3)
            || chunk.attempt_id != run.plan.attempt_id
            || chunk.source_revision != run.source.exact_revision.native_id.as_str()
            || run
                .observation
                .as_ref()
                .is_none_or(|o| o.execution != chunk.execution)
            || !run
                .plan
                .nodes
                .iter()
                .any(|n| n.id.as_str() == chunk.node_id)
            || !matches!(chunk.stream.as_str(), "stdout" | "stderr")
            || chunk.bytes > bounds.max_log_read_bytes
            || chunk.identity(&run.id)?.as_str() != chunk.id
            || !ids.insert(&chunk.id)
        {
            return Err("retained log identity conflict".into());
        }
        let (end, empty) = streams.entry((&chunk.node_id, &chunk.stream)).or_default();
        if chunk.cursor != *end || (chunk.bytes == 0 && *empty) {
            return Err("retained log cursor conflict".into());
        }
        *empty = chunk.bytes == 0;
        *end = end
            .checked_add(chunk.bytes as u64)
            .ok_or("log cursor overflow")?;
        total = total.checked_add(chunk.bytes).ok_or("log byte overflow")?;
    }
    if total > bounds.max_total_log_bytes {
        return Err("log byte retention exceeded".into());
    }
    Ok(())
}
impl Engine {
    // r[impl kiln.web.worker]
    pub(super) fn upload_log(&mut self, id: &str, body: &[u8]) -> Result<ApiResponse, String> {
        let upload: Upload = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return error(400, "log_invalid"),
        };
        if !matches!(upload.stream.as_str(), "stdout" | "stderr")
            || !storage::hex_identity(&upload.blake3)
        {
            return error(400, "log_identity_invalid");
        }
        // Reject stale callers before decoding bytes or contacting the provider.
        let run = self.state.runs.get(id).ok_or("run missing")?;
        if !active_binding(run, &upload) {
            return error(409, "log_stale_identity");
        }
        self.refresh(id)?;
        let run = self.state.runs.get(id).ok_or("run missing")?;
        if !active_binding(run, &upload) {
            return error(409, "log_stale_identity");
        }
        validate(run)?;
        let bounds = &run.plan.bounds;
        let max = bounds.max_log_read_bytes;
        if upload.body_base64.len() > max.saturating_add(2).saturating_div(3).saturating_mul(4) {
            return error(400, "log_byte_bound");
        }
        let bytes = match base64::engine::general_purpose::STANDARD.decode(&upload.body_base64) {
            Ok(bytes) if bytes.len() <= max => bytes,
            _ => return error(400, "log_encoding_or_bound"),
        };
        if blake3::hash(&bytes).to_hex().as_str() != upload.blake3 {
            return error(400, "log_hash_conflict");
        }
        let mut chunk = LogChunk {
            id: String::new(),
            attempt_id: upload.attempt_id,
            execution: upload.execution,
            source_revision: upload.source_revision,
            node_id: upload.node_id,
            stream: upload.stream,
            cursor: upload.cursor,
            bytes: bytes.len(),
            blake3: upload.blake3,
            truncated: upload.truncated,
        };
        chunk.id = chunk.identity(id)?.as_str().to_owned();
        let next = match chunk.cursor.checked_add(chunk.bytes as u64) {
            Some(next) => next,
            None => return error(409, "log_cursor_conflict"),
        };
        let directory = self.profile.state_directory.join("logs");
        if let Some(prior) = run.logs.iter().find(|prior| prior.id == chunk.id) {
            if storage::read_retained(&directory, &prior.id, prior.bytes, &prior.blake3).is_err() {
                return error(503, "log_content_conflict");
            }
            return json_response(200, json!({"next_cursor":next}));
        }
        let mut end = 0u64;
        let mut empty_at_end = false;
        let mut total = 0usize;
        for prior in &run.logs {
            total = total.checked_add(prior.bytes).ok_or("log byte overflow")?;
            if prior.node_id == chunk.node_id && prior.stream == chunk.stream {
                end = prior
                    .cursor
                    .checked_add(prior.bytes as u64)
                    .ok_or("log cursor overflow")?;
                empty_at_end = prior.bytes == 0;
            }
        }
        if chunk.cursor != end || (chunk.bytes == 0 && empty_at_end) {
            return error(409, "log_cursor_conflict");
        }
        if run.logs.len() >= bounds.max_log_references
            || total
                .checked_add(bytes.len())
                .is_none_or(|v| v > bounds.max_total_log_bytes)
        {
            return error(409, "log_retention");
        }
        if storage::publish_retained(&directory, &chunk.id, &bytes).is_err() {
            self.storage_failed = true;
            return error(503, "log_storage_unavailable");
        }
        self.state
            .runs
            .get_mut(id)
            .ok_or("run missing")?
            .logs
            .push(chunk);
        self.persist()?;
        json_response(201, json!({"next_cursor":next}))
    }
    pub(super) fn captured_logs(
        &self,
        id: &str,
        node: &str,
        stream: &str,
        cursor: u64,
        max: usize,
    ) -> Result<ApiResponse, String> {
        let run = self.state.runs.get(id).ok_or("run missing")?;
        if validate(run).is_err() {
            return error(503, "log_metadata_conflict");
        }
        let matching = || {
            run.logs
                .iter()
                .filter(|chunk| chunk.node_id == node && chunk.stream == stream)
        };
        let last = matching().next_back();
        let retained = last.is_some();
        let end = match last {
            Some(chunk) => chunk
                .cursor
                .checked_add(chunk.bytes as u64)
                .ok_or("log cursor overflow")?,
            None => 0,
        };
        if cursor > end {
            return error(400, "log_cursor_out_of_range");
        }
        let mut bytes =
            Vec::with_capacity(max.min(usize::try_from(end - cursor).unwrap_or(usize::MAX)));
        let mut truncated = false;
        let directory = self.profile.state_directory.join("logs");
        // Verify every retained chunk in this stream, including empty and non-returned chunks.
        for chunk in matching() {
            let retained =
                match storage::read_retained(&directory, &chunk.id, chunk.bytes, &chunk.blake3) {
                    Ok(bytes) => bytes,
                    Err(_) => return error(503, "log_content_conflict"),
                };
            truncated |= chunk.truncated;
            let offset = cursor.saturating_sub(chunk.cursor).min(chunk.bytes as u64) as usize;
            bytes.extend(
                retained
                    .iter()
                    .skip(offset)
                    .take(max.saturating_sub(bytes.len()))
                    .copied(),
            );
        }
        let next = cursor
            .checked_add(bytes.len() as u64)
            .ok_or("log cursor overflow")?;
        json_response(
            200,
            json!({"node_id":node,"stream":stream,"source":"worker-capture",
            "cursor":cursor,"next_cursor":next,"text":String::from_utf8_lossy(&bytes),
            "body_base64":base64::engine::general_purpose::STANDARD.encode(&bytes),
            "gap":false,"truncated":truncated || next < end,"retained":retained}),
        )
    }
    fn retained_stream(
        &self,
        id: &str,
        node: &str,
        stream: &str,
    ) -> Result<(Vec<u8>, bool, bool), String> {
        let run = self.state.runs.get(id).ok_or("run missing")?;
        validate(run)?;
        let matching: Vec<_> = run
            .logs
            .iter()
            .filter(|c| c.node_id == node && c.stream == stream)
            .collect();
        let size = matching
            .iter()
            .try_fold(0usize, |sum, c| sum.checked_add(c.bytes))
            .ok_or("log byte overflow")?;
        let mut bytes = Vec::with_capacity(size);
        let mut truncated = false;
        for chunk in &matching {
            bytes.extend(storage::read_retained(
                &self.profile.state_directory.join("logs"),
                &chunk.id,
                chunk.bytes,
                &chunk.blake3,
            )?);
            truncated |= chunk.truncated;
        }
        Ok((bytes, !matching.is_empty(), truncated))
    }
    pub(super) fn search_logs(
        &self,
        id: &str,
        node: &str,
        stream: &str,
        cursor: u64,
        max: usize,
        q: &str,
    ) -> Result<ApiResponse, String> {
        let (bytes, retained, truncated) = match self.retained_stream(id, node, stream) {
            Ok(v) => v,
            Err(_) => return error(503, "log_content_conflict"),
        };
        let start = match usize::try_from(cursor) {
            Ok(v) if v <= bytes.len() => v,
            _ => return error(400, "log_cursor_out_of_range"),
        };
        let needle = q.as_bytes();
        let found = bytes
            .get(start..)
            .ok_or("retained log cursor invalid")?
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|offset| start + offset);
        let Some(found) = found else {
            return json_response(
                200,
                json!({"node_id":node,"stream":stream,"source":"worker-capture","cursor":cursor,"next_cursor":bytes.len(),"text":"","matches":[],"gap":false,"truncated":truncated,"retained":retained,"searched_bytes":bytes.len().saturating_sub(start)}),
            );
        };
        let end = found.saturating_add(max).min(bytes.len());
        let window = bytes
            .get(found..end)
            .ok_or("retained log search range invalid")?;
        let matches: Vec<_> = window
            .windows(needle.len())
            .enumerate()
            .filter(|(_, w)| *w == needle)
            .take(100)
            .map(|(offset, _)| json!({"cursor":found+offset,"bytes":needle.len()}))
            .collect();
        json_response(
            200,
            json!({"node_id":node,"stream":stream,"source":"worker-capture","cursor":found,"next_cursor":end,"text":String::from_utf8_lossy(window),"matches":matches,"gap":false,"truncated":truncated,"retained":retained,"searched_bytes":found-start+needle.len()}),
        )
    }
    pub(super) fn download_logs(
        &self,
        id: &str,
        params: &std::collections::BTreeMap<&str, &str>,
    ) -> Result<ApiResponse, String> {
        if params
            .keys()
            .any(|k| !matches!(*k, "node_id" | "stream" | "metadata"))
            || !(2..=3).contains(&params.len())
        {
            return error(400, "log_query_invalid");
        }
        let node = match params.get("node_id").map(|v| super::api::decoded(v)) {
            Some(Ok(v)) => v,
            _ => return error(400, "node_invalid"),
        };
        let stream = match params.get("stream") {
            Some(v) if matches!(*v, "stdout" | "stderr") => *v,
            _ => return error(400, "stream_invalid"),
        };
        let run = self.state.runs.get(id).ok_or("run missing")?;
        if !run.plan.nodes.iter().any(|n| n.id.as_str() == node) {
            return error(400, "node_invalid");
        }
        let (bytes, retained, truncated) = match self.retained_stream(id, &node, stream) {
            Ok(v) => v,
            Err(_) => return error(503, "log_content_conflict"),
        };
        let safe_node: String = node
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .take(128)
            .collect();
        let filename = format!(
            "{id}-{safe_node}-{stream}{}.log",
            if truncated {
                "-retained-truncated"
            } else {
                "-retained"
            }
        );
        if let Some(metadata) = params.get("metadata") {
            if *metadata != "true" {
                return error(400, "log_query_invalid");
            }
            return json_response(
                200,
                json!({"retained":retained,"truncated":truncated,"bytes":bytes.len(),"blake3":blake3::hash(&bytes).to_hex().as_str(),"filename":filename,"download_path":format!("/api/runs/{id}/logs/download?node_id={}&stream={stream}",url_component(&node))}),
            );
        }
        if !retained {
            return error(404, "log_stream_not_retained");
        }
        Ok(ApiResponse {
            status: 200,
            content_type: "application/octet-stream",
            body: bytes,
            download_name: Some(filename),
        })
    }
}

fn url_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}
