//! Consumer translation of the published Octet acceptance JSON contract.
//! Source: standards/receipts/contracts.ncl at OCTET_REVISION (AGPL-3.0-or-later).
//! No producer implementation is copied. Shape admission never implies deny-all.
use super::{ProducerIssue, MAX_CHECKS, MAX_TEXT_BYTES};
use crate::canonical_json_hash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const OCTET_REVISION: &str = "890f424cd7b1b2015f39c0eb5747f1bd1e576028";
const RECEIPT_SCHEMA: &str = "octet-acceptance-receipt/v1";
const PROVENANCE_SCHEMA: &str = "octet-provenance-record/v1";
const MAX_NATIVE_ROWS: usize = MAX_CHECKS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Passed,
    Failed,
    Blocked,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Digest {
    algorithm: String,
    value: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Stage {
    name: String,
    status: Status,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Input {
    path: String,
    blake3: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    path: String,
    blake3: String,
    bytes: u64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VerificationCommand {
    command: String,
    status: Status,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    schema_version: String,
    source_id: String,
    checker_id: String,
    evidence_tier: String,
    artifact_locator: String,
    blake3: String,
    checked_scope: String,
    #[serde(default)]
    verifier_identity: String,
    #[serde(default)]
    proof_artifact_identity: String,
    #[serde(default)]
    human_action: String,
    #[serde(default)]
    parent_artifacts: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: String,
    run_id: String,
    started_at: String,
    finished_at: String,
    tool: String,
    tool_version: String,
    git_revision: String,
    config_digest: String,
    config_digest_detail: Digest,
    inputs: Vec<Input>,
    stages: Vec<Stage>,
    artifacts: Vec<Artifact>,
    status: Status,
    warnings: Vec<String>,
    errors: Vec<String>,
    verification_commands: Vec<VerificationCommand>,
    verification_evidence: Vec<Input>,
    #[serde(default)]
    provenance_records: Vec<Provenance>,
    raw_log_path: String,
}

#[derive(Debug, Serialize)]
pub struct Admission {
    pub producer_revision: &'static str,
    pub native_status: Status,
    pub native_receipt_hash: String,
    pub warning_count: usize,
    pub error_count: usize,
    pub nonpassing_stage_count: usize,
    pub nonpassing_command_count: usize,
    pub evidence_tiers: Vec<String>,
    pub checked_scope_hashes: Vec<String>,
    pub completion_eligible: bool,
    pub non_claims: [&'static str; 3],
}

// r[impl cairn.check_evidence_producer.adapters]
pub fn admit_json(value: &Value) -> Result<Admission, ProducerIssue> {
    let r: Receipt =
        serde_json::from_value(value.clone()).map_err(|_| ProducerIssue::Observation)?;
    if r.schema_version != RECEIPT_SCHEMA
        || r.config_digest_detail.algorithm != "blake3"
        || ![
            &r.run_id,
            &r.started_at,
            &r.finished_at,
            &r.tool,
            &r.tool_version,
            &r.config_digest,
            &r.config_digest_detail.value,
        ]
        .into_iter()
        .all(|s| required(s))
        || ![&r.git_revision, &r.raw_log_path]
            .into_iter()
            .all(|s| bounded(s))
        || [
            r.inputs.len(),
            r.stages.len(),
            r.artifacts.len(),
            r.warnings.len(),
            r.errors.len(),
            r.verification_commands.len(),
            r.verification_evidence.len(),
            r.provenance_records.len(),
        ]
        .into_iter()
        .any(|n| n > MAX_NATIVE_ROWS)
        || r.inputs
            .iter()
            .chain(&r.verification_evidence)
            .any(|i| !required(&i.path) || !required(&i.blake3))
        || r.stages.iter().any(|s| !required(&s.name))
        || r.artifacts
            .iter()
            .any(|a| !required(&a.path) || !required(&a.blake3))
        || r.verification_commands
            .iter()
            .any(|c| !required(&c.command))
        || r.warnings.iter().chain(&r.errors).any(|s| !bounded(s))
        || r.provenance_records.iter().any(|p| !valid_provenance(p))
    {
        return Err(ProducerIssue::Observation);
    }
    Ok(Admission {
        producer_revision: OCTET_REVISION, native_status:r.status,
        native_receipt_hash: canonical_json_hash(value), warning_count:r.warnings.len(), error_count:r.errors.len(),
        nonpassing_stage_count:r.stages.iter().filter(|s| s.status != Status::Passed).count(),
        nonpassing_command_count:r.verification_commands.iter().filter(|s| s.status != Status::Passed).count(),
        evidence_tiers:r.provenance_records.iter().map(|p| p.evidence_tier.clone()).collect(),
        checked_scope_hashes:r.provenance_records.iter().map(|p| canonical_json_hash(&json!(p.checked_scope))).collect(),
        completion_eligible:false,
        non_claims:["published JSON shape admission does not establish full-catalog deny-all scope", "native statuses and diagnostic counts do not establish executed-test coverage", "scope hashes preserve native scope identity without disclosing raw diagnostics or granting authority"],
    })
}
fn bounded(s: &str) -> bool {
    s.len() <= MAX_TEXT_BYTES
}
fn required(s: &str) -> bool {
    !s.is_empty() && bounded(s)
}
fn valid_provenance(p: &Provenance) -> bool {
    p.schema_version == PROVENANCE_SCHEMA
        && [
            "static_lint",
            "policy_check",
            "ui_fixture",
            "autofix_verified",
            "transcript_assertion",
            "formal_proof",
            "manual_review_required",
        ]
        .contains(&p.evidence_tier.as_str())
        && [
            &p.source_id,
            &p.checker_id,
            &p.artifact_locator,
            &p.blake3,
            &p.checked_scope,
        ]
        .into_iter()
        .all(|s| required(s))
        && [
            &p.verifier_identity,
            &p.proof_artifact_identity,
            &p.human_action,
        ]
        .into_iter()
        .all(|s| bounded(s))
        && p.parent_artifacts.len() <= MAX_NATIVE_ROWS
        && p.parent_artifacts.iter().all(|s| bounded(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        json!({"schema_version":RECEIPT_SCHEMA,"run_id":"run","started_at":"start","finished_at":"finish",
            "tool":"octet","tool_version":"reviewed","git_revision":"revision","config_digest":"digest",
            "config_digest_detail":{"algorithm":"blake3","value":"digest"},"inputs":[],"stages":[{"name":"check","status":"passed"}],
            "artifacts":[],"status":"passed","warnings":[],"errors":[],"verification_commands":[],"verification_evidence":[],"raw_log_path":""})
    }
    #[test]
    // r[verify cairn.check_evidence_producer.adapters]
    fn pinned_shape_preserves_status_and_never_invents_strict_scope() {
        let current = fixture();
        assert_eq!(admit_json(&current).unwrap().native_status, Status::Passed);
        for status in ["passed", "failed", "blocked"] {
            let mut warning = current.clone();
            warning["status"] = json!(status);
            warning["warnings"] = json!(["secret-like native text"]);
            warning["stages"][0]["status"] = json!("failed");
            let admitted = admit_json(&warning).unwrap();
            assert!(!admitted.completion_eligible);
            assert_eq!(admitted.warning_count, 1);
            assert_eq!(admitted.nonpassing_stage_count, 1);
            assert!(!json!(admitted).to_string().contains("secret-like"));
        }
        for (field, bad) in [
            ("schema_version", json!("unknown")),
            ("status", json!("skipped")),
            ("tool", json!("")),
            ("artifacts", json!([{"path":"x","blake3":"h","bytes":-1}])),
            ("warnings", json!(vec!["warning"; MAX_NATIVE_ROWS + 1])),
        ] {
            let mut invalid = current.clone();
            invalid[field] = bad;
            assert!(admit_json(&invalid).is_err(), "admitted {field}");
        }
        let mut missing = current;
        missing.as_object_mut().unwrap().remove("stages");
        assert!(admit_json(&missing).is_err());
    }
}
