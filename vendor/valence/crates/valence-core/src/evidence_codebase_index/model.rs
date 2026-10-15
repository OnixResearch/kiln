use std::collections::BTreeSet;

use iddqd::IdOrdItem;
use iddqd::id_upcast;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

pub const INDEX_SCHEMA_VERSION: &str = "octet-evidence-codebase-index/v1";
pub const NORMALIZATION_VERSION: &str = "octet-evidence-codebase-normalization/v1";
pub const INDEX_BOUNDARY: &str = "Octet evidence codebase index proves artifact identity and linkage only; it does not claim whole-program correctness, semantic equivalence, complete dependency closure, distributed execution correctness, remote code mobility, standalone Valence release ownership, or formal proof unless a linked formal-proof receipt scopes that claim.";

pub(crate) const PROVENANCE_SCHEMA_VERSION: &str = "octet-provenance-record/v1";
pub(crate) const PROOF_CARDS_SCHEMA_VERSION: &str = "octet-proof-cards/v1";
pub(crate) const EVIDENCE_LOG_MANIFEST_SCHEMA_VERSION: &str = "octet-evidence-log-manifest/v1";
pub(crate) const EVIDENCE_RAIL_RECEIPT_SCHEMA_VERSION: &str = "octet-evidence-rail-receipt/v1";
pub(crate) const REPLAY_CAPSULE_KIND: &str = "octet-replay-capsule";
pub(crate) const MANUAL_REVIEW_TIER: &str = "manual_review_required";
pub(crate) const TRANSCRIPT_ASSERTION_TIER: &str = "transcript_assertion";
pub(crate) const STATIC_LINT_TIER: &str = "static_lint";
pub(crate) const POLICY_CHECK_TIER: &str = "policy_check";
pub(crate) const UNSUPPORTED_FAMILY_CODE: &str = "unsupported_family";
pub(crate) const MALFORMED_JSON_CODE: &str = "malformed_json";
pub(crate) const HASH_PREFIX: &str = "b3:";
pub(crate) const COMMAND_PREFIX: &str = "$ ";
pub(crate) const EXPECTED_OUTPUT_PREFIX: &str = "=> ";

pub(crate) const LOCATOR_STABLE_MARKERS: &[&str] = &[
    "artifacts/",
    "cargo-octet/tests/fixtures/",
    "cairn/",
    "docs/",
    "openspec/",
    "standards/",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceArtifactInput {
    pub locator: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceIndexBuildMode {
    Strict,
    Advisory,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct EvidenceCodebaseIndex {
    pub schema_version: String,
    pub normalization_version: String,
    pub index_hash: String,
    pub boundary: String,
    pub source_artifact_count: usize,
    pub record_count: usize,
    pub records: Vec<EvidenceIndexRecord>,
    pub issues: Vec<EvidenceIndexIssue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct EvidenceIndexRecord {
    pub record_id: String,
    pub artifact_family: String,
    pub stable_id: String,
    pub artifact_locator: String,
    pub artifact_blake3: String,
    pub evidence_tier: String,
    pub source_ids: Vec<String>,
    pub requirement_ids: Vec<String>,
    pub lint_ids: Vec<String>,
    pub policy_ids: Vec<String>,
    pub fingerprints: Vec<String>,
    pub artifact_hashes: Vec<String>,
    pub provenance_source_ids: Vec<String>,
    pub replay_commands: Vec<String>,
    pub parent_links: Vec<String>,
    pub caveats: Vec<String>,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct EvidenceIndexIssue {
    pub severity: EvidenceIndexIssueSeverity,
    pub code: String,
    pub artifact_locator: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceIndexIssueSeverity {
    Error,
    Caveat,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct EvidenceIndexQueryReport {
    pub schema_version: String,
    pub query: String,
    pub match_count: usize,
    pub boundary: String,
    pub records: Vec<EvidenceIndexRecord>,
}
pub(crate) fn index_hash(index: &EvidenceCodebaseIndex) -> String {
    let payload = HashableIndex {
        schema_version: &index.schema_version,
        normalization_version: &index.normalization_version,
        boundary: &index.boundary,
        source_artifact_count: index.source_artifact_count,
        record_count: index.record_count,
        records: &index.records,
        issues: &index.issues,
    };
    let rendered = serde_json::to_string(&payload).unwrap_or_else(|error| format!("serialization-error:{error}"));
    blake3_hex(rendered.as_bytes())
}

pub(crate) fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub(crate) fn prefixed_hash(bytes: &[u8]) -> String {
    format!("{HASH_PREFIX}{}", blake3_hex(bytes))
}

pub(crate) fn normalize_locator(locator: &str) -> String {
    let replaced = locator.replace('\\', "/");
    let trimmed = replaced.trim_start_matches("./");
    for marker in LOCATOR_STABLE_MARKERS {
        if let Some(offset) = trimmed.find(marker) {
            return trimmed[offset..].to_owned();
        }
    }
    if trimmed.starts_with('/') {
        let mut components = trimmed.rsplit('/').filter(|component| !component.is_empty());
        let Some(file) = components.next() else {
            return "absolute-path".to_owned();
        };
        if let Some(parent) = components.next() {
            return format!("{parent}/{file}");
        }
        return file.to_owned();
    }
    trimmed.to_owned()
}

pub(crate) fn object_string(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned).filter(|item| !item.trim().is_empty())
}

pub(crate) fn object_strings(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|item| !item.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) fn strings_with_requirement_ids(value: &Value) -> Vec<String> {
    let mut ids = BTreeSet::new();
    collect_requirement_ids(value, &mut ids);
    ids.into_iter().collect()
}

fn collect_requirement_ids(value: &Value, ids: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            let mut remainder = text.as_str();
            while let Some(start) = remainder.find("r[") {
                let after_start = &remainder[start..];
                let Some(end) = after_start.find(']') else {
                    break;
                };
                ids.insert(after_start[..=end].to_owned());
                remainder = &after_start[end + 1..];
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_requirement_ids(item, ids);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_requirement_ids(item, ids);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

pub(crate) fn hash_values_from(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
        Some(Value::Object(map)) => map.values().filter_map(Value::as_str).map(str::to_owned).collect(),
        Some(Value::String(text)) => vec![text.clone()],
        Some(Value::Null | Value::Bool(_) | Value::Number(_)) | None => Vec::new(),
    }
}

pub(crate) fn command_from_json_array(items: &[Value]) -> String {
    items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")
}

pub(crate) fn looks_like_json(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[')
}

#[derive(Clone, Debug)]
pub(crate) struct NormalizedSource {
    pub(crate) locator: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) blake3: String,
}

impl IdOrdItem for NormalizedSource {
    type Key<'a> = &'a str;

    fn key(&self) -> Self::Key<'_> {
        &self.locator
    }

    id_upcast!();
}

pub(crate) struct IndexBuilder {
    pub(crate) mode: EvidenceIndexBuildMode,
    pub(crate) records: Vec<EvidenceIndexRecord>,
    pub(crate) issues: Vec<EvidenceIndexIssue>,
}

impl IndexBuilder {
    pub(crate) fn new(mode: EvidenceIndexBuildMode) -> Self {
        Self {
            mode,
            records: Vec::new(),
            issues: Vec::new(),
        }
    }

    pub(crate) fn record(&mut self, record: EvidenceIndexRecord) {
        self.records.push(record);
    }

    pub(crate) fn issue(&mut self, source: &NormalizedSource, code: &str, detail: &str) {
        self.issues.push(EvidenceIndexIssue {
            severity: if self.mode == EvidenceIndexBuildMode::Strict {
                EvidenceIndexIssueSeverity::Error
            } else {
                EvidenceIndexIssueSeverity::Caveat
            },
            code: code.to_owned(),
            artifact_locator: source.locator.clone(),
            detail: detail.to_owned(),
        });
    }

    pub(crate) fn caveat(&mut self, source: &NormalizedSource, code: &str, detail: &str) {
        self.issues.push(EvidenceIndexIssue {
            severity: EvidenceIndexIssueSeverity::Caveat,
            code: code.to_owned(),
            artifact_locator: source.locator.clone(),
            detail: detail.to_owned(),
        });
    }
}

pub(crate) struct RequiredFields<'a, 'b> {
    source: &'a NormalizedSource,
    builder: &'b mut IndexBuilder,
    family: &'static str,
}

impl<'a, 'b> RequiredFields<'a, 'b> {
    pub(crate) fn new(source: &'a NormalizedSource, builder: &'b mut IndexBuilder, family: &'static str) -> Self {
        Self {
            source,
            builder,
            family,
        }
    }

    pub(crate) fn string(&mut self, value: &Value, field: &str) -> Option<String> {
        let result = object_string(value, field);
        if result.is_none() {
            self.builder.issue(
                self.source,
                &format!("{}.missing_{}", self.family, field),
                &format!("missing required {field} field"),
            );
        }
        result
    }
}

pub(crate) struct EvidenceIndexRecordBuilder<'a> {
    artifact_family: &'static str,
    stable_id: &'a str,
    source: &'a NormalizedSource,
    evidence_tier: String,
    source_ids: BTreeSet<String>,
    requirement_ids: BTreeSet<String>,
    lint_ids: BTreeSet<String>,
    policy_ids: BTreeSet<String>,
    fingerprints: BTreeSet<String>,
    artifact_hashes: BTreeSet<String>,
    provenance_source_ids: BTreeSet<String>,
    replay_commands: BTreeSet<String>,
    parent_links: BTreeSet<String>,
    caveats: BTreeSet<String>,
    summary: String,
}

impl<'a> EvidenceIndexRecordBuilder<'a> {
    pub(crate) fn new(artifact_family: &'static str, stable_id: &'a str, source: &'a NormalizedSource) -> Self {
        let mut artifact_hashes = BTreeSet::new();
        artifact_hashes.insert(source.blake3.clone());
        Self {
            artifact_family,
            stable_id,
            source,
            evidence_tier: MANUAL_REVIEW_TIER.to_owned(),
            source_ids: BTreeSet::new(),
            requirement_ids: BTreeSet::new(),
            lint_ids: BTreeSet::new(),
            policy_ids: BTreeSet::new(),
            fingerprints: BTreeSet::new(),
            artifact_hashes,
            provenance_source_ids: BTreeSet::new(),
            replay_commands: BTreeSet::new(),
            parent_links: BTreeSet::new(),
            caveats: BTreeSet::new(),
            summary: String::new(),
        }
    }

    pub(crate) fn evidence_tier(mut self, value: &str) -> Self {
        value.clone_into(&mut self.evidence_tier);
        self
    }

    pub(crate) fn source_id(mut self, value: Option<String>) -> Self {
        insert_optional(&mut self.source_ids, value);
        self
    }

    pub(crate) fn requirement_id(mut self, value: Option<String>) -> Self {
        insert_optional(&mut self.requirement_ids, value);
        self
    }

    pub(crate) fn requirement_ids(mut self, values: Vec<String>) -> Self {
        self.requirement_ids.extend(values);
        self
    }

    pub(crate) fn lint_id(mut self, value: Option<String>) -> Self {
        insert_optional(&mut self.lint_ids, value);
        self
    }

    pub(crate) fn policy_id(mut self, value: Option<String>) -> Self {
        insert_optional(&mut self.policy_ids, value);
        self
    }

    pub(crate) fn fingerprint(mut self, value: Option<String>) -> Self {
        insert_optional(&mut self.fingerprints, value);
        self
    }

    pub(crate) fn artifact_hash(mut self, value: Option<String>) -> Self {
        insert_optional(&mut self.artifact_hashes, value);
        self
    }

    pub(crate) fn artifact_hashes(mut self, values: Vec<String>) -> Self {
        self.artifact_hashes.extend(values.into_iter().filter(|value| !value.trim().is_empty()));
        self
    }

    pub(crate) fn provenance_source_id(mut self, value: Option<String>) -> Self {
        insert_optional(&mut self.provenance_source_ids, value);
        self
    }

    pub(crate) fn replay_command(mut self, value: Option<String>) -> Self {
        insert_optional(&mut self.replay_commands, value);
        self
    }

    pub(crate) fn replay_commands(mut self, values: Vec<String>) -> Self {
        self.replay_commands.extend(values.into_iter().filter(|value| !value.trim().is_empty()));
        self
    }

    pub(crate) fn parent_link(mut self, value: Option<String>) -> Self {
        insert_optional(&mut self.parent_links, value.map(|item| normalize_locator(&item)));
        self
    }

    pub(crate) fn parent_links(mut self, values: Vec<String>) -> Self {
        self.parent_links.extend(values.into_iter().map(|item| normalize_locator(&item)));
        self
    }

    pub(crate) fn caveat(mut self, value: Option<String>) -> Self {
        insert_optional(&mut self.caveats, value);
        self
    }

    pub(crate) fn caveats(mut self, values: Vec<String>) -> Self {
        self.caveats.extend(values.into_iter().filter(|value| !value.trim().is_empty()));
        self
    }

    pub(crate) fn summary(mut self, value: String) -> Self {
        self.summary = value;
        self
    }

    pub(crate) fn build(self) -> EvidenceIndexRecord {
        let record_seed = format!(
            "family={}\nstable_id={}\nlocator={}\nartifact_blake3={}\n",
            self.artifact_family, self.stable_id, self.source.locator, self.source.blake3
        );
        EvidenceIndexRecord {
            record_id: format!("{}:{}", self.artifact_family, blake3_hex(record_seed.as_bytes())),
            artifact_family: self.artifact_family.to_owned(),
            stable_id: self.stable_id.to_owned(),
            artifact_locator: self.source.locator.clone(),
            artifact_blake3: self.source.blake3.clone(),
            evidence_tier: self.evidence_tier,
            source_ids: self.source_ids.into_iter().collect(),
            requirement_ids: self.requirement_ids.into_iter().collect(),
            lint_ids: self.lint_ids.into_iter().collect(),
            policy_ids: self.policy_ids.into_iter().collect(),
            fingerprints: self.fingerprints.into_iter().collect(),
            artifact_hashes: self.artifact_hashes.into_iter().collect(),
            provenance_source_ids: self.provenance_source_ids.into_iter().collect(),
            replay_commands: self.replay_commands.into_iter().collect(),
            parent_links: self.parent_links.into_iter().collect(),
            caveats: self.caveats.into_iter().collect(),
            summary: self.summary,
        }
    }
}

fn insert_optional(values: &mut BTreeSet<String>, value: Option<String>) {
    if let Some(value) = value.filter(|item| !item.trim().is_empty()) {
        values.insert(value);
    }
}

#[derive(Serialize)]
struct HashableIndex<'a> {
    schema_version: &'a str,
    normalization_version: &'a str,
    boundary: &'a str,
    source_artifact_count: usize,
    record_count: usize,
    records: &'a [EvidenceIndexRecord],
    issues: &'a [EvidenceIndexIssue],
}

impl PartialOrd for EvidenceIndexIssueSeverity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EvidenceIndexIssueSeverity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        issue_severity_rank(*self).cmp(&issue_severity_rank(*other))
    }
}

fn issue_severity_rank(severity: EvidenceIndexIssueSeverity) -> u8 {
    match severity {
        EvidenceIndexIssueSeverity::Error => 0,
        EvidenceIndexIssueSeverity::Caveat => 1,
    }
}
