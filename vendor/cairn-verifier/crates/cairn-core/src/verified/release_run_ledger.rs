// r[impl cairn.nominal_evidence_identities.receipt_chains]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::evidence::canonical_json_hash;
use super::nominal_identity::{
    ArtifactDigestRole, Blake3Digest, DigestRoleMarker, LedgerDigestRole, PolicyDigestRole,
    StateDigestRole, RAW_BLAKE3_PROFILE,
};

pub const RELEASE_RUN_LEDGER_SCHEMA_V1: &str = "cairn.release-run-ledger.v1";
pub const RELEASE_RUN_LEDGER_BOUNDARY: &str = "release-run ledger validation proves ledger syntax, Cairn-owned identity linkage, transition validity, and non-claim preservation only; it does not publish artifacts, does not create tags, does not create GitHub releases, does not upload registry packages, does not sign artifacts, does not prove release eligibility, does not prove evidence truth, and does not trust external services";
pub const RELEASE_RUN_STATE_PENDING: &str = "pending";
pub const RELEASE_RUN_STATE_RUNNING_OBSERVED: &str = "running-observed";
pub const RELEASE_RUN_STATE_PASSED: &str = "passed";
pub const RELEASE_RUN_STATE_FAILED: &str = "failed";
pub const RELEASE_RUN_STATE_SKIPPED: &str = "skipped";
pub const RELEASE_RUN_STATE_BLOCKED: &str = "blocked";
pub const RELEASE_RUN_STATE_SUPERSEDED: &str = "superseded";

const BLAKE3_HEX_LENGTH: usize = 64;
const MAX_STEPS: usize = 256;
const MAX_ARTIFACTS: usize = 512;
const MAX_DIAGNOSTICS: usize = 256;
const MAX_BLOCKERS: usize = 256;
const MAX_RESUME_HINTS: usize = 256;
const REQUIRED_NON_CLAIMS: &[&str] = &[
    "does not publish artifacts",
    "does not create tags",
    "does not upload registry packages",
    "does not prove release eligibility",
    "does not trust external services",
];
const FORBIDDEN_CLAIMS: &[&str] = &[
    "publishes artifacts",
    "creates github releases",
    "proves release eligibility",
    "proves evidence truth",
    "trusts external services",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunCommandIdentity {
    pub name: String,
    pub argv_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunStep {
    pub id: String,
    pub from_state: String,
    pub state: String,
    pub artifact_refs: Vec<String>,
    pub diagnostic_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunArtifact {
    pub id: String,
    pub run_id: String,
    pub role: String,
    pub path: String,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunDiagnostic {
    pub id: String,
    pub step_id: Option<String>,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunBlocker {
    pub id: String,
    pub step_id: String,
    pub class: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunResumeHint {
    pub step_id: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunLedger {
    pub schema_version: String,
    pub run_id: String,
    pub lifecycle_root_identity: String,
    pub policy_hash: String,
    pub command_identity: ReleaseRunCommandIdentity,
    pub profile: String,
    pub steps: Vec<ReleaseRunStep>,
    pub artifacts: Vec<ReleaseRunArtifact>,
    pub diagnostics: Vec<ReleaseRunDiagnostic>,
    pub blockers: Vec<ReleaseRunBlocker>,
    pub resume_hints: Vec<ReleaseRunResumeHint>,
    pub non_claims: Vec<String>,
    pub ledger_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunLedgerIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunLedgerReport {
    pub valid: bool,
    pub run_id: String,
    pub ledger_hash: String,
    pub disposition: String,
    pub terminal_verdict: String,
    pub blockers: Vec<String>,
    pub resumable_steps: Vec<String>,
    pub missing_evidence: Vec<String>,
    pub stale_inputs: Vec<String>,
    pub issues: Vec<ReleaseRunLedgerIssue>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunStatusProjection {
    pub valid: bool,
    pub run_id: String,
    pub disposition: String,
    pub terminal_verdict: String,
    pub blockers: Vec<String>,
    pub resumable_steps: Vec<String>,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRunExplainProjection {
    pub run_id: String,
    pub blockers: Vec<String>,
    pub stale_inputs: Vec<String>,
    pub missing_evidence: Vec<String>,
    pub resumable_steps: Vec<String>,
    pub issues: Vec<ReleaseRunLedgerIssue>,
    pub boundary: String,
}

#[must_use]
pub fn release_run_identity(ledger: &ReleaseRunLedger) -> String {
    canonical_json_hash(&json!({
        "lifecycle_root_identity": ledger.lifecycle_root_identity,
        "policy_hash": ledger.policy_hash,
        "command_identity": ledger.command_identity,
        "profile": ledger.profile,
    }))
}

#[must_use]
pub fn release_run_ledger_hash(value: &Value) -> String {
    let mut material = value.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove("ledger_hash");
    }
    canonical_json_hash(&material)
}

#[must_use]
pub fn validate_release_run_ledger_value(
    value: &Value,
    expected_policy_hash: &str,
) -> ReleaseRunLedgerReport {
    let computed_hash = release_run_ledger_hash(value);
    let Ok(ledger) = serde_json::from_value::<ReleaseRunLedger>(value.clone()) else {
        return invalid_report(
            computed_hash,
            vec![issue(
                "release_run_ledger.malformed",
                "ledger",
                "ledger must match the release-run ledger schema",
            )],
        );
    };
    validate_release_run_ledger(&ledger, expected_policy_hash, computed_hash)
}

#[must_use]
pub fn release_run_status(report: &ReleaseRunLedgerReport) -> ReleaseRunStatusProjection {
    // r[impl cairn.release_evidence.durable_run_ledger.status_explain]
    ReleaseRunStatusProjection {
        valid: report.valid,
        run_id: report.run_id.clone(),
        disposition: report.disposition.clone(),
        terminal_verdict: report.terminal_verdict.clone(),
        blockers: report.blockers.clone(),
        resumable_steps: report.resumable_steps.clone(),
        next_action: report
            .resumable_steps
            .first()
            .map(|step| format!("resume local step `{step}` after resolving its blocker")),
    }
}

#[must_use]
pub fn release_run_explain(report: &ReleaseRunLedgerReport) -> ReleaseRunExplainProjection {
    // r[impl cairn.release_evidence.durable_run_ledger.status_explain]
    ReleaseRunExplainProjection {
        run_id: report.run_id.clone(),
        blockers: report.blockers.clone(),
        stale_inputs: report.stale_inputs.clone(),
        missing_evidence: report.missing_evidence.clone(),
        resumable_steps: report.resumable_steps.clone(),
        issues: report.issues.clone(),
        boundary: report.boundary.clone(),
    }
}

#[must_use]
pub fn positive_release_run_ledger_fixture(expected_policy_hash: &str) -> Value {
    let mut ledger = ReleaseRunLedger {
        schema_version: RELEASE_RUN_LEDGER_SCHEMA_V1.to_string(),
        run_id: String::new(),
        lifecycle_root_identity: fixture_hash('a'),
        policy_hash: expected_policy_hash.to_string(),
        command_identity: ReleaseRunCommandIdentity {
            name: "cairn release-readiness".to_string(),
            argv_blake3: fixture_hash('b'),
        },
        profile: "local-release-readiness".to_string(),
        steps: vec![ReleaseRunStep {
            id: "validate".to_string(),
            from_state: RELEASE_RUN_STATE_PENDING.to_string(),
            state: RELEASE_RUN_STATE_PASSED.to_string(),
            artifact_refs: vec!["validation-receipt".to_string()],
            diagnostic_refs: vec!["validation-summary".to_string()],
        }],
        artifacts: Vec::new(),
        diagnostics: vec![ReleaseRunDiagnostic {
            id: "validation-summary".to_string(),
            step_id: Some("validate".to_string()),
            severity: "info".to_string(),
            message: "local validation completed".to_string(),
        }],
        blockers: Vec::new(),
        resume_hints: Vec::new(),
        non_claims: vec![RELEASE_RUN_LEDGER_BOUNDARY.to_string()],
        ledger_hash: String::new(),
    };
    ledger.run_id = release_run_identity(&ledger);
    ledger.artifacts.push(ReleaseRunArtifact {
        id: "validation-receipt".to_string(),
        run_id: ledger.run_id.clone(),
        role: "validation-receipt".to_string(),
        path: "target/release-run/validation.json".to_string(),
        blake3: fixture_hash('c'),
    });
    let mut value = serde_json::to_value(ledger).expect("fixture serializes");
    value["ledger_hash"] = Value::String(release_run_ledger_hash(&value));
    value
}

fn validate_release_run_ledger(
    ledger: &ReleaseRunLedger,
    expected_policy_hash: &str,
    computed_hash: String,
) -> ReleaseRunLedgerReport {
    // r[impl cairn.release_evidence.durable_run_ledger.model]
    // r[impl cairn.release_evidence.durable_run_ledger.validation]
    let mut issues = Vec::new();
    if ledger.schema_version != RELEASE_RUN_LEDGER_SCHEMA_V1 {
        push_issue(
            &mut issues,
            "release_run_ledger.schema_unsupported",
            "schema_version",
            "schema version is unsupported",
        );
    }
    validate_hash(
        &ledger.lifecycle_root_identity,
        "lifecycle_root_identity",
        &mut issues,
    );
    validate_hash(&ledger.policy_hash, "policy_hash", &mut issues);
    if !same_digest::<PolicyDigestRole>(&ledger.policy_hash, expected_policy_hash) {
        push_issue(
            &mut issues,
            "release_run_ledger.policy_stale",
            "policy_hash",
            "ledger policy hash must match the current expected policy hash",
        );
    }
    validate_required_text(
        &ledger.command_identity.name,
        "command_identity.name",
        &mut issues,
    );
    validate_hash(
        &ledger.command_identity.argv_blake3,
        "command_identity.argv_blake3",
        &mut issues,
    );
    validate_required_text(&ledger.profile, "profile", &mut issues);
    let expected_run_id = release_run_identity(ledger);
    validate_hash(&ledger.run_id, "run_id", &mut issues);
    if !same_digest::<StateDigestRole>(&ledger.run_id, &expected_run_id) {
        push_issue(
            &mut issues,
            "release_run_ledger.run_identity_stale",
            "run_id",
            "run id must match canonical run identity material",
        );
    }
    let step_index = validate_steps(&ledger.steps, &mut issues);
    let artifact_index = validate_artifacts(ledger, &mut issues);
    let diagnostic_index = validate_diagnostics(&ledger.diagnostics, &step_index, &mut issues);
    validate_step_references(
        &ledger.steps,
        &artifact_index,
        &diagnostic_index,
        &mut issues,
    );
    validate_blockers_and_hints(ledger, &step_index, &mut issues);
    validate_non_claims(&ledger.non_claims, &mut issues);
    if !same_digest::<LedgerDigestRole>(&ledger.ledger_hash, &computed_hash) {
        push_issue(
            &mut issues,
            "release_run_ledger.hash_mismatch",
            "ledger_hash",
            "ledger hash must match canonical ledger material",
        );
    }
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
    let blockers = sorted_unique(
        ledger
            .blockers
            .iter()
            .map(|blocker| blocker.id.clone())
            .collect(),
    );
    let resumable_steps = sorted_unique(
        ledger
            .resume_hints
            .iter()
            .map(|hint| hint.step_id.clone())
            .collect(),
    );
    let missing_evidence = missing_evidence(&ledger.steps);
    let stale_inputs = stale_inputs(&issues);
    let terminal_verdict = terminal_verdict(&ledger.steps, &issues);
    let disposition = disposition(&terminal_verdict, &blockers, &resumable_steps);
    let report = ReleaseRunLedgerReport {
        valid: issues.is_empty(),
        run_id: ledger.run_id.clone(),
        ledger_hash: computed_hash,
        disposition,
        terminal_verdict,
        blockers,
        resumable_steps,
        missing_evidence,
        stale_inputs,
        issues,
        boundary: RELEASE_RUN_LEDGER_BOUNDARY.to_string(),
    };
    debug_assert_eq!(report.valid, report.issues.is_empty());
    report
}

fn validate_steps(
    steps: &[ReleaseRunStep],
    issues: &mut Vec<ReleaseRunLedgerIssue>,
) -> BTreeMap<String, ReleaseRunStep> {
    if steps.is_empty() {
        push_issue(
            issues,
            "release_run_ledger.steps_empty",
            "steps",
            "ledger must contain at least one step",
        );
    }
    validate_bound(steps.len(), MAX_STEPS, "steps", issues);
    let mut index = BTreeMap::new();
    for (position, step) in steps.iter().enumerate() {
        let field_path = format!("steps[{position}]");
        validate_required_text(&step.id, &format!("{field_path}.id"), issues);
        if index.insert(step.id.clone(), step.clone()).is_some() {
            push_issue(
                issues,
                "release_run_ledger.step_duplicate",
                &format!("{field_path}.id"),
                "step ids must be unique",
            );
        }
        if !valid_transition(&step.from_state, &step.state) {
            push_issue(
                issues,
                "release_run_ledger.transition_invalid",
                &format!("{field_path}.state"),
                "step transition is not allowed",
            );
        }
        if step.state == RELEASE_RUN_STATE_PASSED && step.artifact_refs.is_empty() {
            push_issue(
                issues,
                "release_run_ledger.artifact_missing",
                &format!("{field_path}.artifact_refs"),
                "passed steps must reference at least one artifact",
            );
        }
        validate_unique_texts(
            &step.artifact_refs,
            &format!("{field_path}.artifact_refs"),
            issues,
        );
        validate_unique_texts(
            &step.diagnostic_refs,
            &format!("{field_path}.diagnostic_refs"),
            issues,
        );
    }
    index
}

fn validate_artifacts(
    ledger: &ReleaseRunLedger,
    issues: &mut Vec<ReleaseRunLedgerIssue>,
) -> BTreeMap<String, ReleaseRunArtifact> {
    validate_bound(ledger.artifacts.len(), MAX_ARTIFACTS, "artifacts", issues);
    let mut index = BTreeMap::new();
    for (position, artifact) in ledger.artifacts.iter().enumerate() {
        let field_path = format!("artifacts[{position}]");
        validate_required_text(&artifact.id, &format!("{field_path}.id"), issues);
        if index
            .insert(artifact.id.clone(), artifact.clone())
            .is_some()
        {
            push_issue(
                issues,
                "release_run_ledger.artifact_duplicate",
                &format!("{field_path}.id"),
                "artifact ids must be unique",
            );
        }
        if !same_digest::<StateDigestRole>(&artifact.run_id, &ledger.run_id) {
            push_issue(
                issues,
                "release_run_ledger.artifact_run_substitution",
                &format!("{field_path}.run_id"),
                "artifact run id must match the ledger run id",
            );
        }
        validate_required_text(&artifact.role, &format!("{field_path}.role"), issues);
        validate_safe_relative_path(&artifact.path, &format!("{field_path}.path"), issues);
        validate_hash(&artifact.blake3, &format!("{field_path}.blake3"), issues);
    }
    index
}

fn validate_diagnostics(
    diagnostics: &[ReleaseRunDiagnostic],
    steps: &BTreeMap<String, ReleaseRunStep>,
    issues: &mut Vec<ReleaseRunLedgerIssue>,
) -> BTreeSet<String> {
    validate_bound(diagnostics.len(), MAX_DIAGNOSTICS, "diagnostics", issues);
    let mut ids = BTreeSet::new();
    for (position, diagnostic) in diagnostics.iter().enumerate() {
        let field_path = format!("diagnostics[{position}]");
        validate_required_text(&diagnostic.id, &format!("{field_path}.id"), issues);
        if !ids.insert(diagnostic.id.clone()) {
            push_issue(
                issues,
                "release_run_ledger.diagnostic_duplicate",
                &format!("{field_path}.id"),
                "diagnostic ids must be unique",
            );
        }
        if let Some(step_id) = &diagnostic.step_id {
            if !steps.contains_key(step_id) {
                push_issue(
                    issues,
                    "release_run_ledger.diagnostic_step_unknown",
                    &format!("{field_path}.step_id"),
                    "diagnostic step id must resolve to a ledger step",
                );
            }
        }
        if !matches!(diagnostic.severity.as_str(), "info" | "warning" | "error") {
            push_issue(
                issues,
                "release_run_ledger.diagnostic_severity_unsupported",
                &format!("{field_path}.severity"),
                "diagnostic severity must be info, warning, or error",
            );
        }
        validate_required_text(
            &diagnostic.message,
            &format!("{field_path}.message"),
            issues,
        );
    }
    ids
}

fn validate_step_references(
    steps: &[ReleaseRunStep],
    artifacts: &BTreeMap<String, ReleaseRunArtifact>,
    diagnostics: &BTreeSet<String>,
    issues: &mut Vec<ReleaseRunLedgerIssue>,
) {
    for (position, step) in steps.iter().enumerate() {
        for artifact in &step.artifact_refs {
            if !artifacts.contains_key(artifact) {
                push_issue(
                    issues,
                    "release_run_ledger.artifact_ref_missing",
                    &format!("steps[{position}].artifact_refs"),
                    "step artifact reference must resolve",
                );
            }
        }
        for diagnostic in &step.diagnostic_refs {
            if !diagnostics.contains(diagnostic) {
                push_issue(
                    issues,
                    "release_run_ledger.diagnostic_ref_missing",
                    &format!("steps[{position}].diagnostic_refs"),
                    "step diagnostic reference must resolve",
                );
            }
        }
    }
}

fn validate_blockers_and_hints(
    ledger: &ReleaseRunLedger,
    steps: &BTreeMap<String, ReleaseRunStep>,
    issues: &mut Vec<ReleaseRunLedgerIssue>,
) {
    validate_bound(ledger.blockers.len(), MAX_BLOCKERS, "blockers", issues);
    validate_bound(
        ledger.resume_hints.len(),
        MAX_RESUME_HINTS,
        "resume_hints",
        issues,
    );
    let mut blocker_ids = BTreeSet::new();
    let mut blocked_steps = BTreeSet::new();
    for (position, blocker) in ledger.blockers.iter().enumerate() {
        let field_path = format!("blockers[{position}]");
        validate_required_text(&blocker.id, &format!("{field_path}.id"), issues);
        if !blocker_ids.insert(blocker.id.as_str()) {
            push_issue(
                issues,
                "release_run_ledger.blocker_duplicate",
                &format!("{field_path}.id"),
                "blocker ids must be unique",
            );
        }
        match steps.get(&blocker.step_id) {
            Some(step)
                if matches!(
                    step.state.as_str(),
                    RELEASE_RUN_STATE_BLOCKED | RELEASE_RUN_STATE_FAILED
                ) =>
            {
                blocked_steps.insert(blocker.step_id.as_str());
            }
            _ => push_issue(
                issues,
                "release_run_ledger.blocker_step_invalid",
                &format!("{field_path}.step_id"),
                "blocker must name a blocked or failed step",
            ),
        }
        validate_required_text(&blocker.class, &format!("{field_path}.class"), issues);
        validate_required_text(&blocker.message, &format!("{field_path}.message"), issues);
    }
    let mut hinted_steps = BTreeSet::new();
    for (position, hint) in ledger.resume_hints.iter().enumerate() {
        let field_path = format!("resume_hints[{position}]");
        if !blocked_steps.contains(hint.step_id.as_str()) {
            push_issue(
                issues,
                "release_run_ledger.resume_step_invalid",
                &format!("{field_path}.step_id"),
                "resume hint must name a blocked or failed step with a blocker",
            );
        }
        if !hinted_steps.insert(hint.step_id.as_str()) {
            push_issue(
                issues,
                "release_run_ledger.resume_hint_duplicate",
                &format!("{field_path}.step_id"),
                "resume hints must be unique per step",
            );
        }
        validate_required_text(&hint.action, &format!("{field_path}.action"), issues);
    }
}

fn valid_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        (
            RELEASE_RUN_STATE_PENDING,
            RELEASE_RUN_STATE_RUNNING_OBSERVED
        ) | (RELEASE_RUN_STATE_PENDING, RELEASE_RUN_STATE_PASSED)
            | (RELEASE_RUN_STATE_PENDING, RELEASE_RUN_STATE_SKIPPED)
            | (RELEASE_RUN_STATE_PENDING, RELEASE_RUN_STATE_BLOCKED)
            | (RELEASE_RUN_STATE_RUNNING_OBSERVED, RELEASE_RUN_STATE_PASSED)
            | (RELEASE_RUN_STATE_RUNNING_OBSERVED, RELEASE_RUN_STATE_FAILED)
            | (
                RELEASE_RUN_STATE_RUNNING_OBSERVED,
                RELEASE_RUN_STATE_BLOCKED
            )
            | (
                RELEASE_RUN_STATE_BLOCKED,
                RELEASE_RUN_STATE_RUNNING_OBSERVED
            )
            | (RELEASE_RUN_STATE_BLOCKED, RELEASE_RUN_STATE_PASSED)
            | (RELEASE_RUN_STATE_FAILED, RELEASE_RUN_STATE_RUNNING_OBSERVED)
            | (RELEASE_RUN_STATE_FAILED, RELEASE_RUN_STATE_SUPERSEDED)
            | (RELEASE_RUN_STATE_PASSED, RELEASE_RUN_STATE_SUPERSEDED)
            | (RELEASE_RUN_STATE_SKIPPED, RELEASE_RUN_STATE_SUPERSEDED)
    )
}

fn missing_evidence(steps: &[ReleaseRunStep]) -> Vec<String> {
    sorted_unique(
        steps
            .iter()
            .filter(|step| step.state == RELEASE_RUN_STATE_PASSED && step.artifact_refs.is_empty())
            .map(|step| step.id.clone())
            .collect(),
    )
}

fn stale_inputs(issues: &[ReleaseRunLedgerIssue]) -> Vec<String> {
    sorted_unique(
        issues
            .iter()
            .filter(|issue| issue.code.contains("stale") || issue.code.contains("substitution"))
            .map(|issue| issue.field_path.clone())
            .collect(),
    )
}

fn terminal_verdict(steps: &[ReleaseRunStep], issues: &[ReleaseRunLedgerIssue]) -> String {
    if !issues.is_empty() {
        return "invalid".to_string();
    }
    if steps
        .iter()
        .any(|step| step.state == RELEASE_RUN_STATE_FAILED)
    {
        return "failed".to_string();
    }
    if steps
        .iter()
        .any(|step| step.state == RELEASE_RUN_STATE_BLOCKED)
    {
        return "blocked".to_string();
    }
    if steps.iter().all(|step| {
        matches!(
            step.state.as_str(),
            RELEASE_RUN_STATE_PASSED | RELEASE_RUN_STATE_SKIPPED | RELEASE_RUN_STATE_SUPERSEDED
        )
    }) {
        return "complete".to_string();
    }
    "in-progress".to_string()
}

fn disposition(terminal_verdict: &str, blockers: &[String], resumable_steps: &[String]) -> String {
    if terminal_verdict == "complete" {
        "complete".to_string()
    } else if !resumable_steps.is_empty() {
        "resumable".to_string()
    } else if !blockers.is_empty() {
        "blocked".to_string()
    } else {
        terminal_verdict.to_string()
    }
}

fn validate_non_claims(non_claims: &[String], issues: &mut Vec<ReleaseRunLedgerIssue>) {
    let joined = non_claims.join("; ");
    for required in REQUIRED_NON_CLAIMS {
        if !joined.contains(required) {
            push_issue(
                issues,
                "release_run_ledger.non_claim_missing",
                "non_claims",
                &format!("non-claims must include `{required}`"),
            );
        }
    }
    let normalized = joined.to_ascii_lowercase();
    for forbidden in FORBIDDEN_CLAIMS {
        if normalized.contains(forbidden) {
            push_issue(
                issues,
                "release_run_ledger.remote_publish_overclaim",
                "non_claims",
                "ledger contains an unsupported publish or correctness claim",
            );
        }
    }
}

fn validate_unique_texts(
    values: &[String],
    field_path: &str,
    issues: &mut Vec<ReleaseRunLedgerIssue>,
) {
    if values.iter().any(|value| value.trim().is_empty()) {
        push_issue(
            issues,
            "release_run_ledger.reference_empty",
            field_path,
            "references must be non-empty",
        );
    }
    let unique = values.iter().collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        push_issue(
            issues,
            "release_run_ledger.reference_duplicate",
            field_path,
            "references must be unique",
        );
    }
}

fn validate_hash(value: &str, field_path: &str, issues: &mut Vec<ReleaseRunLedgerIssue>) {
    let valid = if field_path == "policy_hash" {
        is_digest::<PolicyDigestRole>(value)
    } else if field_path == "ledger_hash" {
        is_digest::<LedgerDigestRole>(value)
    } else if matches!(field_path, "lifecycle_root_identity" | "run_id") {
        is_digest::<StateDigestRole>(value)
    } else {
        is_digest::<ArtifactDigestRole>(value)
    };
    if !valid {
        push_issue(
            issues,
            "release_run_ledger.hash_malformed",
            field_path,
            "hash must be 64 lowercase BLAKE3 hexadecimal characters",
        );
    }
}

fn is_digest<Role: DigestRoleMarker>(value: &str) -> bool {
    Blake3Digest::<Role>::parse(value, RAW_BLAKE3_PROFILE, "release_run.digest").is_ok()
}

fn same_digest<Role: DigestRoleMarker>(left: &str, right: &str) -> bool {
    let left = Blake3Digest::<Role>::parse(left, RAW_BLAKE3_PROFILE, "release_run.left_digest");
    let right = Blake3Digest::<Role>::parse(right, RAW_BLAKE3_PROFILE, "release_run.right_digest");
    matches!(
        (left, right),
        (Ok(left), Ok(right)) if left.as_hex() == right.as_hex()
    )
}

fn validate_required_text(value: &str, field_path: &str, issues: &mut Vec<ReleaseRunLedgerIssue>) {
    if value.trim().is_empty() {
        push_issue(
            issues,
            "release_run_ledger.field_empty",
            field_path,
            "field must be non-empty",
        );
    }
}

fn validate_safe_relative_path(
    path: &str,
    field_path: &str,
    issues: &mut Vec<ReleaseRunLedgerIssue>,
) {
    let safe = !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('~')
        && !path.contains('\\')
        && !path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..");
    if !safe {
        push_issue(
            issues,
            "release_run_ledger.path_unsafe",
            field_path,
            "path must be a safe repository-relative path",
        );
    }
}

fn validate_bound(
    actual: usize,
    maximum: usize,
    field_path: &str,
    issues: &mut Vec<ReleaseRunLedgerIssue>,
) {
    if actual > maximum {
        push_issue(
            issues,
            "release_run_ledger.bound_exceeded",
            field_path,
            "collection exceeds its reviewed bound",
        );
    }
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn invalid_report(
    ledger_hash: String,
    issues: Vec<ReleaseRunLedgerIssue>,
) -> ReleaseRunLedgerReport {
    ReleaseRunLedgerReport {
        valid: false,
        run_id: String::new(),
        ledger_hash,
        disposition: "invalid".to_string(),
        terminal_verdict: "invalid".to_string(),
        blockers: Vec::new(),
        resumable_steps: Vec::new(),
        missing_evidence: Vec::new(),
        stale_inputs: Vec::new(),
        issues,
        boundary: RELEASE_RUN_LEDGER_BOUNDARY.to_string(),
    }
}

fn fixture_hash(character: char) -> String {
    std::iter::repeat_n(character, BLAKE3_HEX_LENGTH).collect()
}

fn issue(code: &str, field_path: &str, message: &str) -> ReleaseRunLedgerIssue {
    ReleaseRunLedgerIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn push_issue(
    issues: &mut Vec<ReleaseRunLedgerIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(issue(code, field_path, message));
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_BLOCKER_COUNT: usize = 1;

    fn policy_hash() -> String {
        fixture_hash('d')
    }

    fn codes(report: &ReleaseRunLedgerReport) -> Vec<&str> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect()
    }

    fn rehash(value: &mut Value) {
        value["ledger_hash"] = Value::String(release_run_ledger_hash(value));
    }

    #[test]
    fn complete_release_run_validates() {
        // r[verify cairn.release_evidence.durable_run_ledger.fixtures.positive]
        // r[verify cairn.release_evidence.durable_run_ledger.final_validation]
        let policy_hash = policy_hash();
        let report = validate_release_run_ledger_value(
            &positive_release_run_ledger_fixture(&policy_hash),
            &policy_hash,
        );
        assert!(report.valid, "{:?}", report.issues);
        assert_eq!(report.terminal_verdict, "complete");
        assert_eq!(report.disposition, "complete");
    }

    #[test]
    fn blocked_release_run_is_valid_and_explainable() {
        // r[verify cairn.release_evidence.durable_run_ledger.status_explain]
        let policy_hash = policy_hash();
        let mut value = positive_release_run_ledger_fixture(&policy_hash);
        value["steps"][0]["state"] = Value::String(RELEASE_RUN_STATE_BLOCKED.to_string());
        value["steps"][0]["artifact_refs"] = Value::Array(Vec::new());
        value["blockers"] = json!([{
            "id": "missing-evidence",
            "step_id": "validate",
            "class": "missing-evidence",
            "message": "validation receipt is absent"
        }]);
        value["resume_hints"] = json!([{
            "step_id": "validate",
            "action": "run local validation and attach its receipt"
        }]);
        rehash(&mut value);
        let report = validate_release_run_ledger_value(&value, &policy_hash);
        assert!(report.valid, "{:?}", report.issues);
        assert_eq!(report.blockers.len(), EXPECTED_BLOCKER_COUNT);
        assert_eq!(report.disposition, "resumable");
        assert!(release_run_status(&report).next_action.is_some());
        assert_eq!(
            release_run_explain(&report).resumable_steps,
            vec!["validate"]
        );
    }

    #[test]
    fn blocked_release_run_without_resume_hint_remains_blocked() {
        // r[verify cairn.release_evidence.durable_run_ledger.fixtures.positive]
        let policy_hash = policy_hash();
        let mut value = positive_release_run_ledger_fixture(&policy_hash);
        value["steps"][0]["state"] = Value::String(RELEASE_RUN_STATE_BLOCKED.to_string());
        value["steps"][0]["artifact_refs"] = Value::Array(Vec::new());
        value["blockers"] = json!([{
            "id": "operator-decision",
            "step_id": "validate",
            "class": "operator-decision",
            "message": "operator review is required"
        }]);
        rehash(&mut value);
        let report = validate_release_run_ledger_value(&value, &policy_hash);
        assert!(report.valid, "{:?}", report.issues);
        assert_eq!(report.disposition, "blocked");
        assert!(report.resumable_steps.is_empty());
    }

    #[test]
    fn invalid_release_run_fixture_matrix_fails_closed() {
        // r[verify cairn.release_evidence.durable_run_ledger.fixtures.negative]
        let policy_hash = policy_hash();
        let cases = [
            ("stale-policy", "release_run_ledger.policy_stale"),
            ("missing-command", "release_run_ledger.field_empty"),
            (
                "invalid-transition",
                "release_run_ledger.transition_invalid",
            ),
            ("weakened-boundary", "release_run_ledger.non_claim_missing"),
            (
                "artifact-substitution",
                "release_run_ledger.artifact_run_substitution",
            ),
            (
                "publish-overclaim",
                "release_run_ledger.remote_publish_overclaim",
            ),
        ];
        for (case, expected) in cases {
            let mut value = positive_release_run_ledger_fixture(&policy_hash);
            match case {
                "stale-policy" => value["policy_hash"] = Value::String(fixture_hash('e')),
                "missing-command" => {
                    value["command_identity"]["name"] = Value::String(String::new())
                }
                "invalid-transition" => {
                    value["steps"][0]["from_state"] =
                        Value::String(RELEASE_RUN_STATE_PASSED.to_string());
                    value["steps"][0]["state"] =
                        Value::String(RELEASE_RUN_STATE_PENDING.to_string());
                }
                "weakened-boundary" => value["non_claims"] = json!(["proves everything"]),
                "artifact-substitution" => {
                    value["artifacts"][0]["run_id"] = Value::String(fixture_hash('f'))
                }
                "publish-overclaim" => value["non_claims"]
                    .as_array_mut()
                    .expect("fixture non-claims array")
                    .push(Value::String("publishes artifacts".to_string())),
                _ => unreachable!(),
            }
            rehash(&mut value);
            let report = validate_release_run_ledger_value(&value, &policy_hash);
            assert!(!report.valid, "case {case}");
            assert!(
                codes(&report).contains(&expected),
                "case {case}: {:?}",
                report.issues
            );
        }
    }

    #[test]
    fn ledger_hash_detects_tampering() {
        let policy_hash = policy_hash();
        let mut value = positive_release_run_ledger_fixture(&policy_hash);
        value["profile"] = Value::String("changed".to_string());
        let report = validate_release_run_ledger_value(&value, &policy_hash);
        assert!(!report.valid);
        assert!(codes(&report).contains(&"release_run_ledger.hash_mismatch"));
    }
}
