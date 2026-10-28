//! Read-only retained Patchgate review receipts from one operator-declared directory.
//!
//! Receipts are producer records, not Kiln state: this surface lists and shows the
//! exact bytes an operator retained, and grants no review, merge, gate or release
//! authority.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engine::api::{error, json_response, query};
use crate::engine::{ApiResponse, Role};

const MAX_POLICY_BYTES: usize = 65_536;
const MAX_RECEIPTS: usize = 256;
const MAX_RECEIPT_BYTES: usize = 1_048_576;
const MAX_NAME_BYTES: usize = 128;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const RECEIPT_SCHEMA: &str = "kiln.patchgate.review.v1";

const NON_CLAIMS: [&str; 3] = [
    "A retained receipt proves only that this directory holds those bytes; Kiln did not re-run the review.",
    "Decisions are the producer's recorded verdicts, not review acceptance, merge authority or gate acceptance.",
    "Listing and reading neither mutate receipts nor grant CI, acceptance or release authority.",
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub label: String,
    pub directory: PathBuf,
}

impl Profile {
    // r[impl kiln.dashboard.reviews.view]
    pub fn admit(&self) -> Result<(), String> {
        if self.schema != "kiln.web-reviews.v1"
            || self.label.is_empty()
            || self.label.len() > 128
            || self.label.chars().any(char::is_control)
            || !crate::profile::absolute(&self.directory)
        {
            return Err("review receipt policy rejected".into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, MAX_POLICY_BYTES, false)
            .map_err(|_| "review policy unavailable or unsafe")?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "review policy malformed")?;
        profile.admit()?;
        Ok(profile)
    }
}

#[derive(Debug)]
pub struct Browser {
    profile: Option<Profile>,
}

impl Browser {
    pub fn new(profile: Option<Profile>) -> Result<Self, String> {
        if let Some(profile) = &profile {
            profile.admit()?;
        }
        Ok(Self { profile })
    }

    // r[impl kiln.dashboard.reviews.view]
    pub fn response(&self, method: &str, target: &str, body: &[u8], role: Role) -> ApiResponse {
        match self.read(method, target, body, role) {
            Ok(response) if response.body.len() <= MAX_RESPONSE_BYTES => response,
            _ => ApiResponse {
                status: 503,
                content_type: "application/json",
                body: br#"{"code":"review_read_unavailable","message":"Review receipt read exceeded its bounds or is unavailable"}"#.to_vec(),
                download_name: None,
            },
        }
    }

    fn read(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
    ) -> Result<ApiResponse, String> {
        if role == Role::Worker {
            return error(403, "worker_scope");
        }
        if method != "GET" {
            return error(405, "reviews_read_only");
        }
        if !body.is_empty() {
            return error(400, "unexpected_body");
        }
        let (path, parameters) = match query(target) {
            Ok(value) => value,
            Err(()) => return error(400, "query_invalid"),
        };
        if !parameters.is_empty() {
            return error(400, "query_invalid");
        }
        if path == "/api/reviews" {
            return self.inventory();
        }
        let Some(name) = path.strip_prefix("/api/reviews/") else {
            return error(404, "route_missing");
        };
        self.detail(name)
    }

    fn inventory(&self) -> Result<ApiResponse, String> {
        let Some(profile) = &self.profile else {
            return json_response(
                200,
                json!({"configured":false,"label":null,"receipts":[],"non_claims":NON_CLAIMS}),
            );
        };
        let directory = &profile.directory;
        let metadata =
            std::fs::symlink_metadata(directory).map_err(|_| "review directory missing")?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return error(503, "review_directory_unavailable");
        }
        let entries = std::fs::read_dir(directory).map_err(|_| "review directory unreadable")?;
        let mut receipts = Vec::new();
        let mut skipped = Vec::new();
        let mut total = 0usize;
        for entry in entries {
            let Ok(entry) = entry else {
                return error(503, "review_directory_unreadable");
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if !receipt_name(&name) {
                skipped.push(json!({"name":name,"reason":"not a .json receipt name"}));
                continue;
            }
            total = total.saturating_add(1);
            if total > MAX_RECEIPTS {
                return error(503, "review_receipt_bound");
            }
            match read_receipt(directory, &name) {
                Ok((bytes, document)) => {
                    receipts.push(summary(&name, bytes, document));
                }
                Err(reason) => skipped.push(json!({"name":name,"reason":reason})),
            }
        }
        receipts.sort_by(|left, right| {
            left.get("name")
                .and_then(Value::as_str)
                .cmp(&right.get("name").and_then(Value::as_str))
        });
        json_response(
            200,
            json!({
                "configured":true,
                "label":profile.label,
                "receipts":receipts,
                "skipped":skipped,
                "total":receipts.len(),
                "non_claims":NON_CLAIMS,
            }),
        )
    }

    fn detail(&self, name: &str) -> Result<ApiResponse, String> {
        let Some(profile) = &self.profile else {
            return error(404, "review_missing");
        };
        if !receipt_name(name) {
            return error(400, "review_name_invalid");
        }
        let directory = &profile.directory;
        match read_receipt(directory, name) {
            Ok((bytes, document)) => json_response(
                200,
                json!({
                    "name":name,
                    "bytes":bytes.len(),
                    "blake3":blake3(&bytes),
                    "document":document,
                    "non_claims":NON_CLAIMS,
                }),
            ),
            Err("missing") => error(404, "review_missing"),
            Err(_) => error(400, "review_invalid"),
        }
    }
}

fn receipt_name(name: &str) -> bool {
    name.len() <= MAX_NAME_BYTES
        && name.ends_with(".json")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn blake3(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn read_receipt(directory: &Path, name: &str) -> Result<(Vec<u8>, Value), &'static str> {
    let path = directory.join(name);
    let metadata = std::fs::symlink_metadata(&path).map_err(|_| "missing")?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("not a regular file");
    }
    if metadata.len() > MAX_RECEIPT_BYTES as u64 {
        return Err("exceeds the receipt byte bound");
    }
    let bytes = std::fs::read(&path).map_err(|_| "unreadable")?;
    let document: Value = serde_json::from_slice(&bytes).map_err(|_| "not JSON")?;
    if document.get("schema").and_then(Value::as_str) != Some(RECEIPT_SCHEMA) {
        return Err("not a kiln.patchgate.review.v1 document");
    }
    Ok((bytes, document))
}

fn summary(name: &str, bytes: Vec<u8>, document: Value) -> Value {
    let observation = document.get("observation").cloned().unwrap_or(Value::Null);
    let reviews = observation
        .get("reviews")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for row in &reviews {
        if let Some(decision) = row.get("decision").and_then(Value::as_str) {
            *counts.entry(decision.to_owned()).or_default() += 1;
        }
    }
    json!({
        "name":name,
        "bytes":bytes.len(),
        "blake3":blake3(&bytes),
        "effect_id":observation.get("effect_id"),
        "candidate_id":observation.get("candidate_id"),
        "pipeline_revision":observation.get("pipeline_revision"),
        "provider_profile":observation.get("provider_profile"),
        "report_identity":observation.get("report_identity"),
        "isolated":observation.get("isolated"),
        "authorizes_acceptance":observation.get("authorizes_acceptance"),
        "decision_counts":counts,
        "decisions":reviews,
        "native_report_bytes":document.get("native_report").and_then(Value::as_str).map(str::len),
    })
}
