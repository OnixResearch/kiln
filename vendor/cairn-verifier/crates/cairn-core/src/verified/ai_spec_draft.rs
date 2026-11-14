use serde_json::Value;

use crate::verified::evidence::canonical_json_hash;

pub const AI_SPEC_DRAFT_SCHEMA_V1: &str = "cairn.ai_spec_draft_provenance.v1";

const ARTIFACT_KIND_LIFECYCLE_SPEC: &str = "lifecycle_spec";
const ARTIFACT_KIND_KANI_CONTRACT: &str = "kani_contract";
const ARTIFACT_KIND_HARNESS: &str = "harness";
const ARTIFACT_KIND_EVIDENCE_NOTE: &str = "evidence_note";
const ASSISTANT_CLASS_AGENTIC: &str = "agentic";
const ASSISTANT_CLASS_INTERACTIVE: &str = "interactive";
const ASSISTANT_CLASS_LOCAL: &str = "local";
const REDACTION_METADATA_ONLY: &str = "metadata_only";
const REDACTION_PROMPT_REDACTED: &str = "prompt_redacted";
const REDACTION_SUMMARY_ONLY: &str = "summary_only";
const REVIEW_DECISION_APPROVED: &str = "approved";
const REVIEW_DECISION_NEEDS_REVISION: &str = "needs_revision";
const REVIEW_DECISION_REJECTED: &str = "rejected";
const REVIEW_DECISION_NOT_REVIEWED: &str = "not_reviewed";
const DRAFT_STATUS_DRAFT: &str = "draft";
const DRAFT_STATUS_ACCEPTED: &str = "accepted";
const DRAFT_STATUS_REJECTED: &str = "rejected";
const DRAFT_STATUS_SUPERSEDED: &str = "superseded";
const ACCEPTANCE_PATH_MACHINE_CHECK: &str = "machine_check";
const ACCEPTANCE_PATH_HUMAN_REVIEW: &str = "human_review";
const ACCEPTANCE_PATH_DOCS_ONLY: &str = "docs_only";
const ACCEPTANCE_PATH_AI_ONLY: &str = "ai_only";
const VALIDATION_STATUS_PASSED: &str = "passed";
const VALIDATION_STATUS_FAILED: &str = "failed";
const VALIDATION_STATUS_NOT_RUN: &str = "not_run";
const EVIDENCE_ROLE_PROPERTY: &str = "property";
const EVIDENCE_ROLE_RECORDED_ONLY: &str = "recorded_only";
const EVIDENCE_ROLE_DOCS_ONLY: &str = "docs_only";
const UNSAFE_TEXT_MARKERS: &[&str] = &[
    "raw_prompt",
    "provider_payload",
    "system_prompt",
    "conversation_transcript",
];
const REQUIRED_FIELDS: &[&str] = &[
    "schema_version",
    "artifact_kind",
    "assistant_class",
    "redaction_class",
    "target_requirement",
    "validation_command",
    "review_decision",
    "draft_status",
    "acceptance_path",
    "validation_status",
    "evidence_role",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSpecDraftIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSpecDraftProvenance {
    pub artifact_kind: String,
    pub assistant_class: String,
    pub redaction_class: String,
    pub target_requirement: String,
    pub validation_command: String,
    pub review_decision: String,
    pub draft_status: String,
    pub acceptance_path: String,
    pub validation_status: String,
    pub evidence_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSpecDraftReport {
    pub valid: bool,
    pub accepted: bool,
    pub verification_evidence: bool,
    pub issues: Vec<AiSpecDraftIssue>,
    pub provenance: Option<AiSpecDraftProvenance>,
    pub receipt_hash: String,
}

pub fn ai_spec_draft_hash(value: &Value) -> String {
    canonical_json_hash(&without_receipt_hash(value))
}

pub fn validate_ai_spec_draft(value: &Value) -> AiSpecDraftReport {
    // r[impl cairn.ai_spec_drafts.provenance]
    // r[impl cairn.ai_spec_drafts.not_verification]
    // r[impl cairn.ai_spec_drafts.acceptance_paths]
    // r[impl cairn.ai_spec_drafts.gate_diagnostics]
    let mut issues = Vec::new();
    validate_required_fields(value, &mut issues);
    validate_known_fields(value, &mut issues);
    validate_secret_free_boundary(value, &mut issues);
    validate_acceptance_path(value, &mut issues);
    validate_ai_only_boundary(value, &mut issues);
    validate_rejected_reuse(value, &mut issues);
    validate_optional_receipt_hash(value, &mut issues);
    let accepted =
        issues.is_empty() && string_field(value, "draft_status") == Some(DRAFT_STATUS_ACCEPTED);
    AiSpecDraftReport {
        valid: issues.is_empty(),
        accepted,
        verification_evidence: false,
        issues,
        provenance: provenance_from_value(value),
        receipt_hash: ai_spec_draft_hash(value),
    }
}

fn validate_required_fields(value: &Value, issues: &mut Vec<AiSpecDraftIssue>) {
    match string_field(value, "schema_version") {
        Some(AI_SPEC_DRAFT_SCHEMA_V1) => {}
        Some(_) => push_issue(
            issues,
            "schema_mismatch",
            "schema_version",
            "unsupported AI draft schema",
        ),
        None => push_issue(
            issues,
            "missing_field",
            "schema_version",
            "missing AI draft provenance field",
        ),
    }
    for field in REQUIRED_FIELDS {
        if string_field(value, field).is_none() {
            push_issue(
                issues,
                "missing_field",
                field,
                "missing AI draft provenance field",
            );
        }
    }
}

fn validate_known_fields(value: &Value, issues: &mut Vec<AiSpecDraftIssue>) {
    validate_known_field(
        value,
        "artifact_kind",
        &[
            ARTIFACT_KIND_LIFECYCLE_SPEC,
            ARTIFACT_KIND_KANI_CONTRACT,
            ARTIFACT_KIND_HARNESS,
            ARTIFACT_KIND_EVIDENCE_NOTE,
        ],
        issues,
    );
    validate_known_field(
        value,
        "assistant_class",
        &[
            ASSISTANT_CLASS_AGENTIC,
            ASSISTANT_CLASS_INTERACTIVE,
            ASSISTANT_CLASS_LOCAL,
        ],
        issues,
    );
    validate_known_field(
        value,
        "redaction_class",
        &[
            REDACTION_METADATA_ONLY,
            REDACTION_PROMPT_REDACTED,
            REDACTION_SUMMARY_ONLY,
        ],
        issues,
    );
    validate_known_field(
        value,
        "review_decision",
        &[
            REVIEW_DECISION_APPROVED,
            REVIEW_DECISION_NEEDS_REVISION,
            REVIEW_DECISION_REJECTED,
            REVIEW_DECISION_NOT_REVIEWED,
        ],
        issues,
    );
    validate_known_field(
        value,
        "draft_status",
        &[
            DRAFT_STATUS_DRAFT,
            DRAFT_STATUS_ACCEPTED,
            DRAFT_STATUS_REJECTED,
            DRAFT_STATUS_SUPERSEDED,
        ],
        issues,
    );
    validate_known_field(
        value,
        "acceptance_path",
        &[
            ACCEPTANCE_PATH_MACHINE_CHECK,
            ACCEPTANCE_PATH_HUMAN_REVIEW,
            ACCEPTANCE_PATH_DOCS_ONLY,
            ACCEPTANCE_PATH_AI_ONLY,
        ],
        issues,
    );
    validate_known_field(
        value,
        "validation_status",
        &[
            VALIDATION_STATUS_PASSED,
            VALIDATION_STATUS_FAILED,
            VALIDATION_STATUS_NOT_RUN,
        ],
        issues,
    );
    validate_known_field(
        value,
        "evidence_role",
        &[
            EVIDENCE_ROLE_PROPERTY,
            EVIDENCE_ROLE_RECORDED_ONLY,
            EVIDENCE_ROLE_DOCS_ONLY,
        ],
        issues,
    );
}

fn validate_secret_free_boundary(value: &Value, issues: &mut Vec<AiSpecDraftIssue>) {
    for marker in UNSAFE_TEXT_MARKERS {
        if value.get(marker).is_some() {
            push_issue(
                issues,
                "unsafe_raw_prompt_storage",
                marker,
                "AI draft provenance must not store raw prompts or provider payloads",
            );
        }
    }
    let rendered = value.to_string();
    if rendered.contains("raw prompt:") || rendered.contains("system prompt:") {
        push_issue(
            issues,
            "unsafe_raw_prompt_storage",
            "$",
            "AI draft provenance text appears to contain raw prompt material",
        );
    }
}

fn validate_acceptance_path(value: &Value, issues: &mut Vec<AiSpecDraftIssue>) {
    match string_field(value, "acceptance_path") {
        Some(ACCEPTANCE_PATH_MACHINE_CHECK) => validate_machine_check_path(value, issues),
        Some(ACCEPTANCE_PATH_HUMAN_REVIEW) => validate_human_review_path(value, issues),
        Some(ACCEPTANCE_PATH_DOCS_ONLY) => validate_docs_only_path(value, issues),
        Some(ACCEPTANCE_PATH_AI_ONLY) => push_issue(
            issues,
            "ai_draft_not_verification",
            "acceptance_path",
            "AI draft provenance is not verification evidence",
        ),
        _ => {}
    }
}

fn validate_machine_check_path(value: &Value, issues: &mut Vec<AiSpecDraftIssue>) {
    if string_field(value, "validation_status") != Some(VALIDATION_STATUS_PASSED) {
        push_issue(
            issues,
            "missing_validation_status",
            "validation_status",
            "machine-check acceptance requires passing validation status",
        );
    }
    if string_field(value, "machine_receipt_status") != Some(VALIDATION_STATUS_PASSED) {
        push_issue(
            issues,
            "missing_machine_receipt",
            "machine_receipt_status",
            "machine-check acceptance requires a passing receipt status",
        );
    }
}

fn validate_human_review_path(value: &Value, issues: &mut Vec<AiSpecDraftIssue>) {
    if string_field(value, "review_decision") != Some(REVIEW_DECISION_APPROVED) {
        push_issue(
            issues,
            "missing_review_acceptance",
            "review_decision",
            "human-review acceptance requires an approved review decision",
        );
    }
}

fn validate_docs_only_path(value: &Value, issues: &mut Vec<AiSpecDraftIssue>) {
    if string_field(value, "docs_rationale").is_none() {
        push_issue(
            issues,
            "missing_docs_rationale",
            "docs_rationale",
            "docs-only acceptance requires explicit rationale",
        );
    }
    if string_field(value, "evidence_role") == Some(EVIDENCE_ROLE_PROPERTY) {
        push_issue(
            issues,
            "ai_draft_not_verification",
            "evidence_role",
            "docs-only AI draft provenance cannot be property evidence",
        );
    }
}

fn validate_ai_only_boundary(value: &Value, issues: &mut Vec<AiSpecDraftIssue>) {
    if string_field(value, "evidence_role") == Some(EVIDENCE_ROLE_PROPERTY)
        && string_field(value, "acceptance_path") == Some(ACCEPTANCE_PATH_AI_ONLY)
    {
        push_issue(
            issues,
            "ai_draft_not_verification",
            "evidence_role",
            "AI draft provenance alone cannot satisfy property evidence",
        );
    }
}

fn validate_rejected_reuse(value: &Value, issues: &mut Vec<AiSpecDraftIssue>) {
    let rejected_or_superseded = matches!(
        string_field(value, "draft_status"),
        Some(DRAFT_STATUS_REJECTED | DRAFT_STATUS_SUPERSEDED)
    );
    if rejected_or_superseded && bool_field(value, "cited_as_accepted") == Some(true) {
        push_issue(
            issues,
            "rejected_draft_reuse",
            "draft_status",
            "rejected or superseded AI drafts cannot be reused as accepted evidence",
        );
    }
}

fn validate_optional_receipt_hash(value: &Value, issues: &mut Vec<AiSpecDraftIssue>) {
    if let Some(actual) = string_field(value, "receipt_hash") {
        let expected = ai_spec_draft_hash(value);
        if actual != expected {
            push_issue(
                issues,
                "receipt_hash_mismatch",
                "receipt_hash",
                "AI draft receipt hash does not match canonical draft provenance",
            );
        }
    }
}

fn provenance_from_value(value: &Value) -> Option<AiSpecDraftProvenance> {
    Some(AiSpecDraftProvenance {
        artifact_kind: string_field(value, "artifact_kind")?.to_string(),
        assistant_class: string_field(value, "assistant_class")?.to_string(),
        redaction_class: string_field(value, "redaction_class")?.to_string(),
        target_requirement: string_field(value, "target_requirement")?.to_string(),
        validation_command: string_field(value, "validation_command")?.to_string(),
        review_decision: string_field(value, "review_decision")?.to_string(),
        draft_status: string_field(value, "draft_status")?.to_string(),
        acceptance_path: string_field(value, "acceptance_path")?.to_string(),
        validation_status: string_field(value, "validation_status")?.to_string(),
        evidence_role: string_field(value, "evidence_role")?.to_string(),
    })
}

fn validate_known_field(
    value: &Value,
    field: &'static str,
    allowed: &[&str],
    issues: &mut Vec<AiSpecDraftIssue>,
) {
    if let Some(actual) = string_field(value, field) {
        if !allowed.contains(&actual) {
            push_issue(
                issues,
                "invalid_field",
                field,
                "field has unsupported AI draft value",
            );
        }
    }
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn bool_field(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}

fn without_receipt_hash(value: &Value) -> Value {
    let mut canonical = value.clone();
    if let Some(object) = canonical.as_object_mut() {
        object.remove("receipt_hash");
    }
    canonical
}

fn push_issue(issues: &mut Vec<AiSpecDraftIssue>, code: &str, field_path: &str, message: &str) {
    issues.push(AiSpecDraftIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    const REQUIREMENT_ID: &str = "cairn.example.ai_draft";

    fn machine_checked_draft() -> Value {
        json!({
            "schema_version": AI_SPEC_DRAFT_SCHEMA_V1,
            "artifact_kind": ARTIFACT_KIND_KANI_CONTRACT,
            "assistant_class": ASSISTANT_CLASS_AGENTIC,
            "redaction_class": REDACTION_METADATA_ONLY,
            "target_requirement": REQUIREMENT_ID,
            "validation_command": "cairn kani-receipt validate target/kani-receipt.json",
            "review_decision": REVIEW_DECISION_NOT_REVIEWED,
            "draft_status": DRAFT_STATUS_ACCEPTED,
            "acceptance_path": ACCEPTANCE_PATH_MACHINE_CHECK,
            "validation_status": VALIDATION_STATUS_PASSED,
            "machine_receipt_status": VALIDATION_STATUS_PASSED,
            "evidence_role": EVIDENCE_ROLE_PROPERTY,
        })
    }

    fn reviewed_spec_draft() -> Value {
        json!({
            "schema_version": AI_SPEC_DRAFT_SCHEMA_V1,
            "artifact_kind": ARTIFACT_KIND_LIFECYCLE_SPEC,
            "assistant_class": ASSISTANT_CLASS_INTERACTIVE,
            "redaction_class": REDACTION_PROMPT_REDACTED,
            "target_requirement": REQUIREMENT_ID,
            "validation_command": "cairn validate --root .",
            "review_decision": REVIEW_DECISION_APPROVED,
            "draft_status": DRAFT_STATUS_ACCEPTED,
            "acceptance_path": ACCEPTANCE_PATH_HUMAN_REVIEW,
            "validation_status": VALIDATION_STATUS_PASSED,
            "evidence_role": EVIDENCE_ROLE_DOCS_ONLY,
        })
    }

    fn issue_codes(report: &AiSpecDraftReport) -> Vec<&str> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect()
    }

    #[test]
    fn ai_drafted_kani_contract_with_passing_receipt_is_accepted_metadata() {
        // r[verify cairn.ai_spec_drafts.positive_fixtures]
        // r[verify cairn.ai_spec_drafts.positive_fixtures.valid]
        // r[verify cairn.ai_spec_drafts.acceptance_paths.machine]
        // r[verify cairn.ai_spec_drafts.not_verification.checked]
        let report = validate_ai_spec_draft(&machine_checked_draft());

        assert!(report.valid, "{:?}", report.issues);
        assert!(report.accepted);
        assert!(!report.verification_evidence);
        assert_eq!(
            report.provenance.expect("provenance").target_requirement,
            REQUIREMENT_ID
        );
    }

    #[test]
    fn ai_drafted_spec_with_review_acceptance_is_accepted_metadata() {
        // r[verify cairn.ai_spec_drafts.positive_fixtures]
        // r[verify cairn.ai_spec_drafts.positive_fixtures.valid]
        // r[verify cairn.ai_spec_drafts.provenance.recorded]
        // r[verify cairn.ai_spec_drafts.acceptance_paths.review]
        let report = validate_ai_spec_draft(&reviewed_spec_draft());

        assert!(report.valid, "{:?}", report.issues);
        assert!(report.accepted);
        assert!(!report.verification_evidence);
    }

    #[test]
    fn ai_only_property_claim_fails_with_named_diagnostic() {
        // r[verify cairn.ai_spec_drafts.negative_fixtures]
        // r[verify cairn.ai_spec_drafts.not_verification.ai_only]
        // r[verify cairn.ai_spec_drafts.gate_diagnostics]
        // r[verify cairn.ai_spec_drafts.gate_diagnostics.named]
        let mut draft = machine_checked_draft();
        draft["acceptance_path"] = json!(ACCEPTANCE_PATH_AI_ONLY);
        draft["machine_receipt_status"] = Value::Null;
        let report = validate_ai_spec_draft(&draft);

        assert!(!report.valid);
        assert!(issue_codes(&report).contains(&"ai_draft_not_verification"));
    }

    #[test]
    fn missing_target_and_validation_status_fail_closed() {
        // r[verify cairn.ai_spec_drafts.negative_fixtures]
        // r[verify cairn.ai_spec_drafts.negative_fixtures.status]
        // r[verify cairn.ai_spec_drafts.provenance.target]
        let mut draft = machine_checked_draft();
        draft
            .as_object_mut()
            .expect("draft object")
            .remove("target_requirement");
        draft["validation_status"] = json!(VALIDATION_STATUS_FAILED);
        let report = validate_ai_spec_draft(&draft);
        let codes = issue_codes(&report);

        assert!(!report.valid);
        assert!(codes.contains(&"missing_field"));
        assert!(codes.contains(&"missing_validation_status"));
    }

    #[test]
    fn rejected_draft_reuse_fails_closed() {
        // r[verify cairn.ai_spec_drafts.negative_fixtures]
        // r[verify cairn.ai_spec_drafts.negative_fixtures.rejected]
        let mut draft = reviewed_spec_draft();
        draft["draft_status"] = json!(DRAFT_STATUS_REJECTED);
        draft["cited_as_accepted"] = json!(true);
        let report = validate_ai_spec_draft(&draft);

        assert!(!report.valid);
        assert!(issue_codes(&report).contains(&"rejected_draft_reuse"));
    }

    #[test]
    fn docs_only_property_and_raw_prompt_storage_fail_closed() {
        // r[verify cairn.ai_spec_drafts.negative_fixtures]
        // r[verify cairn.ai_spec_drafts.operator_docs]
        // r[verify cairn.ai_spec_drafts.operator_docs.visible]
        let mut draft = reviewed_spec_draft();
        draft["acceptance_path"] = json!(ACCEPTANCE_PATH_DOCS_ONLY);
        draft["evidence_role"] = json!(EVIDENCE_ROLE_PROPERTY);
        draft["docs_rationale"] = json!("reviewed docs only");
        draft["raw_prompt"] = json!("raw prompt: secret fixture");
        let report = validate_ai_spec_draft(&draft);
        let codes = issue_codes(&report);

        assert!(!report.valid);
        assert!(codes.contains(&"ai_draft_not_verification"));
        assert!(codes.contains(&"unsafe_raw_prompt_storage"));
    }

    #[test]
    fn receipt_hash_mismatch_fails_deterministically() {
        let mut draft = reviewed_spec_draft();
        draft["receipt_hash"] =
            json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let report = validate_ai_spec_draft(&draft);

        assert!(!report.valid);
        assert!(issue_codes(&report).contains(&"receipt_hash_mismatch"));
    }
}
