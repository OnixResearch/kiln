use std::collections::BTreeSet;

use serde_json::Value;

use super::evidence::canonical_json_hash;
use super::nominal_identity::{
    ArtifactDigestRole, Blake3Digest, ChainDigestRole, DigestRoleMarker, InputDigestRole,
    PlanDigestRole, PolicyDigestRole, ReceiptDigestRole, StateDigestRole, RAW_BLAKE3_PROFILE,
};

pub const LIFECYCLE_RECEIPT_CORE_SCHEMA_V1: &str = "cairn.lifecycle-receipt-core.v1";
pub const LIFECYCLE_RECEIPT_CORE_BOUNDARY: &str = "lifecycle receipt core validation proves only receipt envelope syntax, hash consistency, diagnostics shape, and non-claim presence; it does not prove release eligibility, lifecycle readiness, policy satisfaction, evidence truth, source-code correctness, verifier soundness, or downstream behavioral correctness";

pub const LIFECYCLE_RECEIPT_HASH_PROFILE_CORE_V1: &str =
    "cairn.lifecycle-receipt-core.hash-profile.v1";
pub const LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH: &str = "receipt_hash";
pub const LIFECYCLE_RECEIPT_HASH_FIELD_INPUT_HASH: &str = "input_hash";
pub const LIFECYCLE_RECEIPT_HASH_FIELD_POLICY_HASH: &str = "policy_hash";

pub const LIFECYCLE_RECEIPT_ISSUE_UNSUPPORTED_VERDICT: &str =
    "lifecycle_receipt.unsupported_verdict";
pub const LIFECYCLE_RECEIPT_ISSUE_MISSING_DIAGNOSTIC: &str = "lifecycle_receipt.missing_diagnostic";
pub const LIFECYCLE_RECEIPT_ISSUE_MISSING_FIELD: &str = "lifecycle_receipt.missing_field";
pub const LIFECYCLE_RECEIPT_ISSUE_MISSING_HASH: &str = "lifecycle_receipt.missing_hash";
pub const LIFECYCLE_RECEIPT_ISSUE_MALFORMED_HASH: &str = "lifecycle_receipt.malformed_hash";
pub const LIFECYCLE_RECEIPT_ISSUE_STALE_HASH: &str = "lifecycle_receipt.stale_hash";
pub const LIFECYCLE_RECEIPT_ISSUE_HASH_PROFILE: &str = "lifecycle_receipt.hash_profile";
pub const LIFECYCLE_RECEIPT_ISSUE_SELF_HASH: &str = "lifecycle_receipt.self_hash";
pub const LIFECYCLE_RECEIPT_ISSUE_WEAK_NON_CLAIM: &str = "lifecycle_receipt.weak_non_claim";
pub const LIFECYCLE_RECEIPT_ISSUE_OVERCLAIM: &str = "lifecycle_receipt.overclaim";

const SUPPORTED_VERDICTS: &[&str] = &[
    "PASS", "WARN", "FAIL", "SKIP", "pass", "warn", "fail", "skip",
];
const DIAGNOSTIC_VERDICTS: &[&str] = &["WARN", "FAIL", "warn", "fail"];
const SUPPORTED_SEVERITIES: &[&str] = &["info", "warning", "error", "advisory"];
const HASH_FIELD_SUFFIX: &str = "_hash";
const DEFAULT_TOOL_ID: &str = "cairn";
const CAIRN_GATE_FAMILY: &str = "cairn.gate";
const CAIRN_VALIDATE_FAMILY: &str = "cairn.validate";
const CAIRN_SYNC_ARCHIVE_FAMILY: &str = "cairn.sync_archive";
const COMMAND_GATE: &str = "gate";
const COMMAND_VALIDATE: &str = "validate";
const COMMAND_SYNC_ARCHIVE: &str = "sync_or_archive";
const FIELD_VERDICT: &str = "verdict";
const FIELD_VALID: &str = "valid";
const FIELD_BLOCKED: &str = "blocked";
const FIELD_ISSUES: &str = "issues";
const FIELD_CHANGE_ISSUES: &str = "change_issues";
const FIELD_SPEC_ISSUES: &str = "spec_issues";
const FIELD_REASONS: &str = "reasons";
const FIELD_DIAGNOSTICS: &str = "diagnostics";
const FIELD_WARNINGS: &str = "warnings";
const FIELD_ACTIONS: &str = "actions";
const FIELD_ARTIFACTS: &str = "artifacts";
const FIELD_COMMAND: &str = "command";
const FIELD_CODE: &str = "code";
const FIELD_MESSAGE: &str = "message";
const FIELD_DESCRIPTION: &str = "description";
const FIELD_FIELD_PATH: &str = "field_path";
const FIELD_PATH: &str = "path";
const FIELD_SCOPE: &str = "scope";
const FIELD_SEVERITY: &str = "severity";
const FIELD_COUNT_CHANGES: &str = "changes";
const FIELD_COUNT_SPECS_VALIDATED: &str = "specs_validated";

const REQUIRED_NON_CLAIM_FRAGMENTS: &[&str] = &[
    "release eligibility",
    "lifecycle readiness",
    "policy satisfaction",
    "evidence truth",
    "source-code correctness",
    "verifier soundness",
    "downstream behavioral correctness",
];

const OVERCLAIM_PHRASES: &[&str] = &[
    "proves release eligibility",
    "proves lifecycle readiness",
    "proves policy satisfaction",
    "proves evidence truth",
    "proves source-code correctness",
    "proves verifier soundness",
    "proves downstream behavioral correctness",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleReceiptCommandIdentity {
    pub tool: String,
    pub family: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleReceiptDiagnostic {
    pub code: String,
    pub message: String,
    pub scope: String,
    pub field_path: String,
    pub severity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleReceiptEvidenceSummary {
    pub family: String,
    pub checked_count: usize,
    pub diagnostic_count: usize,
    pub artifact_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleReceiptHashField {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleReceiptHashProfile {
    pub id: String,
    pub required_hash_fields: Vec<String>,
    pub self_hash_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleReceiptEnvelope {
    pub schema_version: String,
    pub family: String,
    pub command: LifecycleReceiptCommandIdentity,
    pub verdict: String,
    pub diagnostics: Vec<LifecycleReceiptDiagnostic>,
    pub evidence_summary: LifecycleReceiptEvidenceSummary,
    pub hash_profile_id: String,
    pub hash_fields: Vec<LifecycleReceiptHashField>,
    pub receipt_hash: String,
    pub non_claims: Vec<String>,
    pub claim: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LifecycleReceiptValidationInput {
    pub envelope: LifecycleReceiptEnvelope,
    pub hash_profile: LifecycleReceiptHashProfile,
    pub hash_material: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleReceiptIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleReceiptValidationReport {
    pub valid: bool,
    pub verdict: String,
    pub family: String,
    pub issues: Vec<LifecycleReceiptIssue>,
    pub normalized_receipt_hash: String,
    pub boundary: String,
}

pub fn default_lifecycle_receipt_hash_profile() -> LifecycleReceiptHashProfile {
    // r[impl cairn.release_evidence.lifecycle_receipt_core.contract]
    // r[impl cairn.release_evidence.lifecycle_receipt_core.hash_profiles]
    LifecycleReceiptHashProfile {
        id: LIFECYCLE_RECEIPT_HASH_PROFILE_CORE_V1.to_string(),
        required_hash_fields: vec![LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH.to_string()],
        self_hash_fields: vec![LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH.to_string()],
    }
}

pub fn default_lifecycle_receipt_non_claims() -> Vec<String> {
    REQUIRED_NON_CLAIM_FRAGMENTS
        .iter()
        .map(|fragment| format!("does not prove {fragment}"))
        .collect()
}

pub fn lifecycle_receipt_hash_material(
    receipt: &Value,
    profile: &LifecycleReceiptHashProfile,
) -> Value {
    // r[impl cairn.release_evidence.lifecycle_receipt_core.hash_profiles]
    // r[impl cairn.release_evidence.lifecycle_receipt_core.hash_profiles.self_hash]
    let mut material = receipt.clone();
    if let Some(object) = material.as_object_mut() {
        for field in &profile.self_hash_fields {
            object.remove(field);
        }
    }
    material
}

pub fn lifecycle_receipt_hash(receipt: &Value, profile: &LifecycleReceiptHashProfile) -> String {
    let material = lifecycle_receipt_hash_material(receipt, profile);
    canonical_json_hash(&material)
}

pub fn validate_lifecycle_receipt_envelope(
    input: &LifecycleReceiptValidationInput,
) -> LifecycleReceiptValidationReport {
    // r[impl cairn.release_evidence.lifecycle_receipt_core.validation]
    // r[impl cairn.release_evidence.lifecycle_receipt_core.validation.shell]
    let mut issues = Vec::new();
    validate_hash_profile(input, &mut issues);
    validate_command_identity(&input.envelope.command, &mut issues);
    validate_supported_verdict(&input.envelope.verdict, &mut issues);
    validate_evidence_summary(&input.envelope, &mut issues);
    validate_diagnostics(&input.envelope, &mut issues);
    validate_hash_fields(input, &mut issues);
    validate_non_claims(&input.envelope, &mut issues);
    sort_lifecycle_receipt_issues(&mut issues);

    let valid = issues.is_empty();
    LifecycleReceiptValidationReport {
        valid,
        verdict: if valid { "PASS" } else { "FAIL" }.to_string(),
        family: input.envelope.family.clone(),
        issues,
        normalized_receipt_hash: lifecycle_receipt_hash(&input.hash_material, &input.hash_profile),
        boundary: LIFECYCLE_RECEIPT_CORE_BOUNDARY.to_string(),
    }
}

pub fn adapt_cairn_gate_receipt(receipt: &Value) -> LifecycleReceiptValidationReport {
    // r[impl cairn.release_evidence.lifecycle_receipt_core.adapters]
    // r[impl cairn.release_evidence.lifecycle_receipt_core.adapters.family]
    adapt_lifecycle_receipt_value(CAIRN_GATE_FAMILY, COMMAND_GATE, DEFAULT_TOOL_ID, receipt)
}

pub fn adapt_cairn_validate_receipt(receipt: &Value) -> LifecycleReceiptValidationReport {
    // r[impl cairn.release_evidence.lifecycle_receipt_core.adapters]
    adapt_lifecycle_receipt_value(
        CAIRN_VALIDATE_FAMILY,
        COMMAND_VALIDATE,
        DEFAULT_TOOL_ID,
        receipt,
    )
}

pub fn adapt_cairn_sync_archive_receipt(receipt: &Value) -> LifecycleReceiptValidationReport {
    // r[impl cairn.release_evidence.lifecycle_receipt_core.adapters]
    adapt_lifecycle_receipt_value(
        CAIRN_SYNC_ARCHIVE_FAMILY,
        COMMAND_SYNC_ARCHIVE,
        DEFAULT_TOOL_ID,
        receipt,
    )
}

pub fn adapt_downstream_lifecycle_receipt(
    family: &str,
    receipt: &Value,
) -> LifecycleReceiptValidationReport {
    // r[impl cairn.release_evidence.lifecycle_receipt_core.adapters]
    let tool = family.split('.').next().unwrap_or("downstream");
    let command = receipt
        .get(FIELD_COMMAND)
        .and_then(Value::as_str)
        .unwrap_or("receipt");
    adapt_lifecycle_receipt_value(family, command, tool, receipt)
}

pub fn adapt_lifecycle_receipt_value(
    family: &str,
    command: &str,
    tool: &str,
    receipt: &Value,
) -> LifecycleReceiptValidationReport {
    let profile = default_lifecycle_receipt_hash_profile();
    let envelope = lifecycle_receipt_envelope_from_value(family, command, tool, receipt, &profile);
    let input = LifecycleReceiptValidationInput {
        envelope,
        hash_profile: profile,
        hash_material: receipt.clone(),
    };
    validate_lifecycle_receipt_envelope(&input)
}

pub fn lifecycle_receipt_envelope_from_value(
    family: &str,
    command: &str,
    tool: &str,
    receipt: &Value,
    profile: &LifecycleReceiptHashProfile,
) -> LifecycleReceiptEnvelope {
    let receipt_hash = receipt
        .get(LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| lifecycle_receipt_hash(receipt, profile));
    let diagnostics = lifecycle_receipt_diagnostics_from_value(receipt);
    let artifact_count =
        array_count(receipt, FIELD_ACTIONS) + array_count(receipt, FIELD_ARTIFACTS);
    LifecycleReceiptEnvelope {
        schema_version: LIFECYCLE_RECEIPT_CORE_SCHEMA_V1.to_string(),
        family: family.to_string(),
        command: LifecycleReceiptCommandIdentity {
            tool: tool.to_string(),
            family: family.to_string(),
            command: command.to_string(),
        },
        verdict: receipt_verdict(receipt),
        diagnostics: diagnostics.clone(),
        evidence_summary: LifecycleReceiptEvidenceSummary {
            family: family.to_string(),
            checked_count: checked_count(receipt),
            diagnostic_count: diagnostics.len(),
            artifact_count,
        },
        hash_profile_id: profile.id.clone(),
        hash_fields: lifecycle_receipt_hash_fields_from_value(receipt, &receipt_hash),
        receipt_hash,
        non_claims: default_lifecycle_receipt_non_claims(),
        claim: LIFECYCLE_RECEIPT_CORE_BOUNDARY.to_string(),
    }
}

fn validate_hash_profile(
    input: &LifecycleReceiptValidationInput,
    issues: &mut Vec<LifecycleReceiptIssue>,
) {
    if input.hash_profile.id.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_HASH_PROFILE,
            "hash_profile.id",
            "receipt hash profile id is required",
        );
    }
    if input.envelope.hash_profile_id != input.hash_profile.id {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_HASH_PROFILE,
            "hash_profile_id",
            "receipt envelope hash profile id does not match selected profile",
        );
    }
    if receipt_has_self_hash(&input.hash_material)
        && !input
            .hash_profile
            .self_hash_fields
            .iter()
            .any(|field| field == LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH)
    {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_SELF_HASH,
            LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH,
            "receipt hash profile must explicitly exclude its own receipt_hash field",
        );
    }
}

fn validate_command_identity(
    command: &LifecycleReceiptCommandIdentity,
    issues: &mut Vec<LifecycleReceiptIssue>,
) {
    if command.tool.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_FIELD,
            "command.tool",
            "receipt command identity requires a tool",
        );
    }
    if command.family.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_FIELD,
            "command.family",
            "receipt command identity requires a family",
        );
    }
    if command.command.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_FIELD,
            "command.command",
            "receipt command identity requires a command",
        );
    }
}

fn validate_supported_verdict(verdict: &str, issues: &mut Vec<LifecycleReceiptIssue>) {
    if !SUPPORTED_VERDICTS.contains(&verdict) {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_UNSUPPORTED_VERDICT,
            FIELD_VERDICT,
            "receipt verdict is outside the lifecycle receipt vocabulary",
        );
    }
}

fn validate_evidence_summary(
    envelope: &LifecycleReceiptEnvelope,
    issues: &mut Vec<LifecycleReceiptIssue>,
) {
    if envelope.evidence_summary.family != envelope.family {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_FIELD,
            "evidence_summary.family",
            "receipt evidence summary family must match receipt family",
        );
    }
    if envelope.evidence_summary.diagnostic_count != envelope.diagnostics.len() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_FIELD,
            "evidence_summary.diagnostic_count",
            "receipt evidence summary diagnostic count must match diagnostics",
        );
    }
}

fn validate_diagnostics(
    envelope: &LifecycleReceiptEnvelope,
    issues: &mut Vec<LifecycleReceiptIssue>,
) {
    if DIAGNOSTIC_VERDICTS.contains(&envelope.verdict.as_str()) && envelope.diagnostics.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_DIAGNOSTIC,
            FIELD_DIAGNOSTICS,
            "warning and failure receipts require at least one diagnostic row",
        );
    }
    for diagnostic in &envelope.diagnostics {
        validate_diagnostic_row(diagnostic, issues);
    }
}

fn validate_diagnostic_row(
    diagnostic: &LifecycleReceiptDiagnostic,
    issues: &mut Vec<LifecycleReceiptIssue>,
) {
    if diagnostic.code.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_DIAGNOSTIC,
            "diagnostics.code",
            "diagnostic rows require a code",
        );
    }
    if diagnostic.message.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_DIAGNOSTIC,
            "diagnostics.message",
            "diagnostic rows require a message",
        );
    }
    if diagnostic.scope.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_DIAGNOSTIC,
            "diagnostics.scope",
            "diagnostic rows require a scope",
        );
    }
    if diagnostic.field_path.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_DIAGNOSTIC,
            "diagnostics.field_path",
            "diagnostic rows require a field path",
        );
    }
    if !SUPPORTED_SEVERITIES.contains(&diagnostic.severity.as_str()) {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_DIAGNOSTIC,
            "diagnostics.severity",
            "diagnostic rows require a supported severity",
        );
    }
}

fn validate_hash_fields(
    input: &LifecycleReceiptValidationInput,
    issues: &mut Vec<LifecycleReceiptIssue>,
) {
    let field_names = input
        .envelope
        .hash_fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<BTreeSet<_>>();
    for required in &input.hash_profile.required_hash_fields {
        if !field_names.contains(required.as_str()) {
            push_lifecycle_receipt_issue(
                issues,
                LIFECYCLE_RECEIPT_ISSUE_MISSING_HASH,
                required,
                "required receipt hash field is missing",
            );
        }
    }
    for field in &input.envelope.hash_fields {
        if !is_typed_hash_field(&field.name, &field.value) {
            push_lifecycle_receipt_issue(
                issues,
                LIFECYCLE_RECEIPT_ISSUE_MALFORMED_HASH,
                &field.name,
                "receipt hash fields must be lowercase BLAKE3 hex strings",
            );
        }
    }
    if input.envelope.receipt_hash.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MISSING_HASH,
            LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH,
            "receipt_hash is required",
        );
        return;
    }
    if !is_digest::<ReceiptDigestRole>(&input.envelope.receipt_hash) {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_MALFORMED_HASH,
            LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH,
            "receipt_hash must be a lowercase BLAKE3 hex string",
        );
        return;
    }
    let expected = lifecycle_receipt_hash(&input.hash_material, &input.hash_profile);
    if !same_digest::<ReceiptDigestRole>(&input.envelope.receipt_hash, &expected) {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_STALE_HASH,
            LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH,
            "receipt_hash does not match the selected profile's canonical material",
        );
    }
}

fn validate_non_claims(
    envelope: &LifecycleReceiptEnvelope,
    issues: &mut Vec<LifecycleReceiptIssue>,
) {
    let joined = envelope.non_claims.join("; ").to_ascii_lowercase();
    if envelope.non_claims.is_empty() {
        push_lifecycle_receipt_issue(
            issues,
            LIFECYCLE_RECEIPT_ISSUE_WEAK_NON_CLAIM,
            "non_claims",
            "lifecycle receipt validation requires explicit non-claims",
        );
        return;
    }
    for fragment in REQUIRED_NON_CLAIM_FRAGMENTS {
        let does_not_prove = format!("does not prove {fragment}");
        let not_fragment = format!("not {fragment}");
        if !joined.contains(&does_not_prove) && !joined.contains(&not_fragment) {
            push_lifecycle_receipt_issue(
                issues,
                LIFECYCLE_RECEIPT_ISSUE_WEAK_NON_CLAIM,
                "non_claims",
                &format!("non-claims must state that validation does not prove {fragment}"),
            );
        }
    }
    let claim = envelope.claim.to_ascii_lowercase();
    for phrase in OVERCLAIM_PHRASES {
        if claim.contains(phrase) {
            push_lifecycle_receipt_issue(
                issues,
                LIFECYCLE_RECEIPT_ISSUE_OVERCLAIM,
                "claim",
                "receipt claim text promotes envelope validation beyond its boundary",
            );
        }
    }
}

fn lifecycle_receipt_diagnostics_from_value(receipt: &Value) -> Vec<LifecycleReceiptDiagnostic> {
    let mut diagnostics = Vec::new();
    collect_diagnostics_from_field(receipt, FIELD_ISSUES, &mut diagnostics);
    collect_diagnostics_from_field(receipt, FIELD_CHANGE_ISSUES, &mut diagnostics);
    collect_diagnostics_from_field(receipt, FIELD_SPEC_ISSUES, &mut diagnostics);
    collect_diagnostics_from_field(receipt, FIELD_REASONS, &mut diagnostics);
    collect_diagnostics_from_field(receipt, FIELD_DIAGNOSTICS, &mut diagnostics);
    collect_diagnostics_from_field(receipt, FIELD_WARNINGS, &mut diagnostics);
    diagnostics.sort_by(|left, right| {
        left.scope
            .cmp(&right.scope)
            .then(left.field_path.cmp(&right.field_path))
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    diagnostics
}

fn collect_diagnostics_from_field(
    receipt: &Value,
    field: &str,
    diagnostics: &mut Vec<LifecycleReceiptDiagnostic>,
) {
    let Some(items) = receipt.get(field).and_then(Value::as_array) else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let default_path = format!("/{field}/{index}");
        diagnostics.push(match item {
            Value::Object(object) => LifecycleReceiptDiagnostic {
                code: object
                    .get(FIELD_CODE)
                    .and_then(Value::as_str)
                    .unwrap_or(field)
                    .to_string(),
                message: object
                    .get(FIELD_MESSAGE)
                    .or_else(|| object.get(FIELD_DESCRIPTION))
                    .and_then(Value::as_str)
                    .unwrap_or(field)
                    .to_string(),
                scope: object
                    .get(FIELD_SCOPE)
                    .and_then(Value::as_str)
                    .unwrap_or(field)
                    .to_string(),
                field_path: object
                    .get(FIELD_FIELD_PATH)
                    .or_else(|| object.get(FIELD_PATH))
                    .and_then(Value::as_str)
                    .unwrap_or(&default_path)
                    .to_string(),
                severity: object
                    .get(FIELD_SEVERITY)
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| default_severity(receipt))
                    .to_string(),
            },
            Value::String(message) => LifecycleReceiptDiagnostic {
                code: field.to_string(),
                message: message.clone(),
                scope: field.to_string(),
                field_path: default_path,
                severity: default_severity(receipt).to_string(),
            },
            _ => LifecycleReceiptDiagnostic {
                code: field.to_string(),
                message: item.to_string(),
                scope: field.to_string(),
                field_path: default_path,
                severity: default_severity(receipt).to_string(),
            },
        });
    }
}

fn lifecycle_receipt_hash_fields_from_value(
    receipt: &Value,
    receipt_hash: &str,
) -> Vec<LifecycleReceiptHashField> {
    let mut fields = Vec::new();
    if let Some(object) = receipt.as_object() {
        for (name, value) in object {
            if is_hash_field_name(name) {
                if let Some(hash) = value.as_str() {
                    fields.push(LifecycleReceiptHashField {
                        name: name.clone(),
                        value: hash.to_string(),
                    });
                }
            }
        }
    }
    if !fields
        .iter()
        .any(|field| field.name == LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH)
    {
        fields.push(LifecycleReceiptHashField {
            name: LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH.to_string(),
            value: receipt_hash.to_string(),
        });
    }
    fields.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.value.cmp(&right.value))
    });
    fields
}

fn receipt_verdict(receipt: &Value) -> String {
    if let Some(verdict) = receipt.get(FIELD_VERDICT).and_then(Value::as_str) {
        return verdict.to_string();
    }
    if let Some(valid) = receipt.get(FIELD_VALID).and_then(Value::as_bool) {
        return if valid { "PASS" } else { "FAIL" }.to_string();
    }
    if let Some(blocked) = receipt.get(FIELD_BLOCKED).and_then(Value::as_bool) {
        return if blocked { "FAIL" } else { "PASS" }.to_string();
    }
    String::new()
}

fn checked_count(receipt: &Value) -> usize {
    integer_count(receipt, FIELD_COUNT_CHANGES)
        + integer_count(receipt, FIELD_COUNT_SPECS_VALIDATED)
        + array_count(receipt, FIELD_ACTIONS)
        + array_count(receipt, FIELD_ARTIFACTS)
}

fn integer_count(receipt: &Value, field: &str) -> usize {
    receipt
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_default()
}

fn array_count(receipt: &Value, field: &str) -> usize {
    receipt
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default()
}

fn default_severity(receipt: &Value) -> &'static str {
    match receipt_verdict(receipt).as_str() {
        "WARN" | "warn" => "warning",
        "FAIL" | "fail" => "error",
        _ => "info",
    }
}

fn receipt_has_self_hash(receipt: &Value) -> bool {
    receipt
        .as_object()
        .map(|object| object.contains_key(LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH))
        .unwrap_or(false)
}

fn is_hash_field_name(name: &str) -> bool {
    name == LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH || name.ends_with(HASH_FIELD_SUFFIX)
}

fn is_typed_hash_field(name: &str, value: &str) -> bool {
    match name {
        LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH => is_digest::<ReceiptDigestRole>(value),
        LIFECYCLE_RECEIPT_HASH_FIELD_INPUT_HASH => is_digest::<InputDigestRole>(value),
        LIFECYCLE_RECEIPT_HASH_FIELD_POLICY_HASH => is_digest::<PolicyDigestRole>(value),
        "plan_hash" => is_digest::<PlanDigestRole>(value),
        "chain_hash" => is_digest::<ChainDigestRole>(value),
        "state_hash" => is_digest::<StateDigestRole>(value),
        _ => is_digest::<ArtifactDigestRole>(value),
    }
}

fn is_digest<Role: DigestRoleMarker>(value: &str) -> bool {
    Blake3Digest::<Role>::parse(value, RAW_BLAKE3_PROFILE, "lifecycle_receipt.digest").is_ok()
}

fn same_digest<Role: DigestRoleMarker>(left: &str, right: &str) -> bool {
    let left = Blake3Digest::<Role>::parse(left, RAW_BLAKE3_PROFILE, "lifecycle_receipt.left");
    let right = Blake3Digest::<Role>::parse(right, RAW_BLAKE3_PROFILE, "lifecycle_receipt.right");
    matches!(
        (left, right),
        (Ok(left), Ok(right)) if left.as_hex() == right.as_hex()
    )
}

fn push_lifecycle_receipt_issue(
    issues: &mut Vec<LifecycleReceiptIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(LifecycleReceiptIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    });
}

fn sort_lifecycle_receipt_issues(issues: &mut [LifecycleReceiptIssue]) {
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const BLAKE3_HEX_LENGTH: usize = 64;

    const EXPECTED_GATE_DIAGNOSTICS: usize = 1;

    fn hash_for(label: &str) -> String {
        canonical_json_hash(&json!({"label": label}))
    }

    fn valid_gate_receipt() -> Value {
        let profile = default_lifecycle_receipt_hash_profile();
        let mut receipt = json!({
            "change": "demo",
            "stage": "tasks",
            "valid": true,
            "verdict": "PASS",
            "issues": [],
            "input_hash": hash_for("gate-input"),
            "policy_hash": hash_for("gate-policy"),
            "policy": "cairn-default",
            "layout": "cairn"
        });
        let receipt_hash = lifecycle_receipt_hash(&receipt, &profile);
        receipt[LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH] = json!(receipt_hash);
        receipt
    }

    fn valid_sync_receipt() -> Value {
        let profile = default_lifecycle_receipt_hash_profile();
        let mut receipt = json!({
            "actions": [
                {"kind": "archive_change", "path": "./.cairn/archive/demo", "description": "archive demo"}
            ],
            "blocked": false,
            "change": "demo",
            "dry_run": true,
            "input_hash": hash_for("sync-input"),
            "layout": "cairn",
            "mutated": false,
            "mutation_manifest": null,
            "plan_hash": hash_for("sync-plan"),
            "policy": "cairn-default",
            "policy_hash": hash_for("sync-policy"),
            "reasons": []
        });
        let receipt_hash = lifecycle_receipt_hash(&receipt, &profile);
        receipt[LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH] = json!(receipt_hash);
        receipt
    }

    fn valid_downstream_receipt() -> Value {
        let profile = default_lifecycle_receipt_hash_profile();
        let mut receipt = json!({
            "command": "policy-check",
            "family": "octet.policy",
            "valid": true,
            "verdict": "WARN",
            "issues": [
                {
                    "code": "advisory_only",
                    "message": "receipt is advisory",
                    "scope": "policy",
                    "field_path": "/issues/0",
                    "severity": "warning"
                }
            ],
            "input_hash": hash_for("octet-input"),
            "policy_hash": hash_for("octet-policy")
        });
        let receipt_hash = lifecycle_receipt_hash(&receipt, &profile);
        receipt[LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH] = json!(receipt_hash);
        receipt
    }

    fn valid_envelope_input() -> LifecycleReceiptValidationInput {
        let profile = default_lifecycle_receipt_hash_profile();
        let receipt = valid_gate_receipt();
        LifecycleReceiptValidationInput {
            envelope: lifecycle_receipt_envelope_from_value(
                CAIRN_GATE_FAMILY,
                COMMAND_GATE,
                DEFAULT_TOOL_ID,
                &receipt,
                &profile,
            ),
            hash_profile: profile,
            hash_material: receipt,
        }
    }

    fn issue_codes(report: &LifecycleReceiptValidationReport) -> Vec<String> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect()
    }

    #[test]
    fn lifecycle_receipt_core_accepts_valid_cairn_and_downstream_envelopes() {
        // r[verify cairn.release_evidence.lifecycle_receipt_core.contract]
        // r[verify cairn.release_evidence.lifecycle_receipt_core.fixtures.positive]
        let gate_report = adapt_cairn_gate_receipt(&valid_gate_receipt());
        assert!(
            gate_report.valid,
            "unexpected gate issues: {:?}",
            gate_report.issues
        );
        assert_eq!(gate_report.family, CAIRN_GATE_FAMILY);
        assert_eq!(gate_report.normalized_receipt_hash.len(), BLAKE3_HEX_LENGTH);

        let validate_report = adapt_cairn_validate_receipt(&json!({
            "change_issues": [],
            "changes": 1,
            "issues": [],
            "layout": "cairn",
            "policy": "cairn-default",
            "spec_issues": [],
            "specs_validated": 1,
            "valid": true
        }));
        assert!(
            validate_report.valid,
            "unexpected validate issues: {:?}",
            validate_report.issues
        );
        assert_eq!(validate_report.family, CAIRN_VALIDATE_FAMILY);

        let downstream_report =
            adapt_downstream_lifecycle_receipt("octet.policy", &valid_downstream_receipt());
        assert!(
            downstream_report.valid,
            "unexpected downstream issues: {:?}",
            downstream_report.issues
        );
        assert_eq!(downstream_report.family, "octet.policy");
    }

    #[test]
    fn lifecycle_receipt_core_rejects_malformed_envelopes_and_overclaims() {
        // r[verify cairn.release_evidence.lifecycle_receipt_core.fixtures.negative]
        let mut input = valid_envelope_input();
        input.envelope.verdict = "MAYBE".to_string();
        input.envelope.diagnostics = Vec::new();
        input.envelope.receipt_hash = String::new();
        input.envelope.hash_fields = Vec::new();
        input.envelope.non_claims = vec!["proves release eligibility".to_string()];
        input.envelope.claim = "validating this envelope proves lifecycle readiness".to_string();
        input.hash_profile.self_hash_fields = Vec::new();
        let report = validate_lifecycle_receipt_envelope(&input);
        let codes = issue_codes(&report);

        assert!(!report.valid);
        assert!(codes.contains(&LIFECYCLE_RECEIPT_ISSUE_UNSUPPORTED_VERDICT.to_string()));
        assert!(codes.contains(&LIFECYCLE_RECEIPT_ISSUE_MISSING_HASH.to_string()));
        assert!(codes.contains(&LIFECYCLE_RECEIPT_ISSUE_SELF_HASH.to_string()));
        assert!(codes.contains(&LIFECYCLE_RECEIPT_ISSUE_WEAK_NON_CLAIM.to_string()));
        assert!(codes.contains(&LIFECYCLE_RECEIPT_ISSUE_OVERCLAIM.to_string()));
    }

    #[test]
    fn lifecycle_receipt_core_reports_stale_hashes_and_missing_failure_diagnostics() {
        // r[verify cairn.release_evidence.lifecycle_receipt_core.validation]
        let mut stale = valid_gate_receipt();
        stale[LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH] = json!("0".repeat(BLAKE3_HEX_LENGTH));
        let stale_report = adapt_cairn_gate_receipt(&stale);
        assert!(!stale_report.valid);
        assert!(
            issue_codes(&stale_report).contains(&LIFECYCLE_RECEIPT_ISSUE_STALE_HASH.to_string())
        );

        let profile = default_lifecycle_receipt_hash_profile();
        let mut failure = json!({
            "valid": false,
            "verdict": "FAIL",
            "issues": [],
            "input_hash": hash_for("failure-input"),
            "policy_hash": hash_for("failure-policy")
        });
        let receipt_hash = lifecycle_receipt_hash(&failure, &profile);
        failure[LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH] = json!(receipt_hash);
        let failure_report = adapt_cairn_gate_receipt(&failure);
        assert!(!failure_report.valid);
        assert!(issue_codes(&failure_report)
            .contains(&LIFECYCLE_RECEIPT_ISSUE_MISSING_DIAGNOSTIC.to_string()));
    }

    #[test]
    fn lifecycle_receipt_hash_profiles_exclude_self_hash_fields() {
        // r[verify cairn.release_evidence.lifecycle_receipt_core.hash_profiles]
        // r[verify cairn.release_evidence.lifecycle_receipt_core.hash_profiles.self_hash]
        let profile = default_lifecycle_receipt_hash_profile();
        let mut left = json!({
            "valid": true,
            "verdict": "PASS",
            "issues": [],
            "input_hash": hash_for("left")
        });
        left[LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH] = json!("a".repeat(BLAKE3_HEX_LENGTH));
        let mut right = left.clone();
        right[LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH] = json!("b".repeat(BLAKE3_HEX_LENGTH));

        assert_eq!(
            lifecycle_receipt_hash(&left, &profile),
            lifecycle_receipt_hash(&right, &profile)
        );
    }

    #[test]
    fn lifecycle_receipt_adapters_preserve_existing_family_shapes() {
        // r[verify cairn.release_evidence.lifecycle_receipt_core.adapters]
        // r[verify cairn.release_evidence.lifecycle_receipt_core.adapters.family]
        let sync_report = adapt_cairn_sync_archive_receipt(&valid_sync_receipt());
        assert!(
            sync_report.valid,
            "unexpected sync issues: {:?}",
            sync_report.issues
        );
        assert_eq!(sync_report.family, CAIRN_SYNC_ARCHIVE_FAMILY);

        let downstream =
            adapt_downstream_lifecycle_receipt("valence.evidence", &valid_downstream_receipt());
        assert!(
            downstream.valid,
            "unexpected downstream issues: {:?}",
            downstream.issues
        );
        assert_eq!(downstream.family, "valence.evidence");
    }

    #[test]
    fn lifecycle_receipt_core_boundary_is_visible_and_bounded() {
        // r[verify cairn.release_evidence.lifecycle_receipt_core.docs]
        // r[verify cairn.release_evidence.lifecycle_receipt_core.validation.shell]
        // r[verify cairn.release_evidence.lifecycle_receipt_core.final_validation]
        let report = adapt_cairn_gate_receipt(&valid_gate_receipt());

        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert!(report.boundary.contains("hash consistency"));
        assert!(report
            .boundary
            .contains("does not prove release eligibility"));
        assert!(report
            .boundary
            .contains("downstream behavioral correctness"));
    }

    #[test]
    fn lifecycle_receipt_diagnostics_are_normalized_from_public_issue_shapes() {
        let profile = default_lifecycle_receipt_hash_profile();
        let mut receipt = json!({
            "valid": false,
            "verdict": "FAIL",
            "issues": ["plain issue text"],
            "reasons": [
                {"code": "blocked", "message": "gate blocked", "path": "/blocked", "severity": "error"}
            ],
            "input_hash": hash_for("issue-input"),
            "policy_hash": hash_for("issue-policy")
        });
        let receipt_hash = lifecycle_receipt_hash(&receipt, &profile);
        receipt[LIFECYCLE_RECEIPT_HASH_FIELD_RECEIPT_HASH] = json!(receipt_hash);
        let envelope = lifecycle_receipt_envelope_from_value(
            CAIRN_GATE_FAMILY,
            COMMAND_GATE,
            DEFAULT_TOOL_ID,
            &receipt,
            &profile,
        );

        assert_eq!(envelope.diagnostics.len(), EXPECTED_GATE_DIAGNOSTICS + 1);
        assert!(envelope
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "blocked"));
    }
}
