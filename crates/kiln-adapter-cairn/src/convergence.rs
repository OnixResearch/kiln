//! Pure, bounded intake for `cairn.convergence.v1`.
//!
//! The shell reads bytes; this module decides. It performs no I/O, starts no
//! process and sets no gate, run, attempt, candidate or provider outcome. A
//! `current` row is not Kiln acceptance.

use std::collections::BTreeSet;

use blake3::Hasher;
use kiln_core::convergence::{EvidenceRow, EvidenceState};
use kiln_core::types::{ContentId, Diagnostic};
use serde::Deserialize;
use serde_json::Value;

/// The only admitted receipt schema.
pub const CONVERGENCE_RECEIPT_SCHEMA_V1: &str = "cairn.convergence.v1";
/// The only admitted receipt status.
pub const CONVERGENCE_RECEIPT_STATUS: &str = "evaluated";
/// Only Git revisions bind a Kiln candidate in this profile.
pub const CONVERGENCE_REVISION_KIND: &str = "git";

/// Largest receipt Kiln parses. The retained pilot is 231,695 bytes.
pub const MAX_CONVERGENCE_RECEIPT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_CONVERGENCE_ROWS: usize = 4096;
pub const MAX_CONVERGENCE_CONTRACTS: usize = 512;
pub const MAX_CONVERGENCE_ISSUES: usize = 4096;
pub const MAX_CONVERGENCE_NON_CLAIMS: usize = 64;
pub const MAX_CONVERGENCE_ID_BYTES: usize = 128;
pub const MAX_CONVERGENCE_TEXT_BYTES: usize = 1024;

pub const ISSUE_OVERSIZED: &str = "convergence_receipt.oversized";
pub const ISSUE_MALFORMED: &str = "convergence_receipt.malformed";
pub const ISSUE_UNKNOWN_SCHEMA: &str = "convergence_receipt.unknown_schema";
pub const ISSUE_UNKNOWN_STATUS: &str = "convergence_receipt.unknown_status";
pub const ISSUE_UNKNOWN_STATE: &str = "convergence_receipt.unknown_state";
pub const ISSUE_INVALID_IDENTITY: &str = "convergence_receipt.invalid_identity";
pub const ISSUE_INVALID_REVISION_KIND: &str = "convergence_receipt.invalid_revision_kind";
pub const ISSUE_DUPLICATE_REQUIREMENT: &str = "convergence_receipt.duplicate_requirement";
pub const ISSUE_ROW_BOUND: &str = "convergence_receipt.row_bound";
pub const ISSUE_TEXT_BOUND: &str = "convergence_receipt.text_bound";
pub const ISSUE_RECEIPT_IDENTITY_MISMATCH: &str = "convergence_receipt.receipt_identity_mismatch";
pub const ISSUE_BINDING_MISMATCH: &str = "convergence_receipt.binding_mismatch";

/// The caller's declared identity for one receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvergenceReceiptBinding {
    pub receipt_blake3: ContentId,
    pub revision_kind: String,
    pub revision: String,
    pub registry_blake3: ContentId,
    pub case_blake3: ContentId,
    pub policy_blake3: ContentId,
}

/// One binding issue Cairn could not consume.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvergenceUnboundIssue {
    pub code: String,
    pub field_path: String,
}

/// Admitted receipt facts and nothing more.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedConvergenceReceipt {
    pub receipt_blake3: ContentId,
    pub repository_id: String,
    pub revision_kind: String,
    pub revision: String,
    pub registry_blake3: ContentId,
    pub case_blake3: ContentId,
    pub policy_blake3: ContentId,
    pub targets_blake3: ContentId,
    pub generated_sources_blake3: ContentId,
    pub rows: Vec<EvidenceRow>,
    pub unbound_target_issues: Vec<ConvergenceUnboundIssue>,
    pub targets_current: bool,
    pub coverage_complete: bool,
    pub complete: bool,
    pub authority: String,
    pub non_claims: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireReceipt {
    schema_version: String,
    status: String,
    repository_id: String,
    revision: WireRevision,
    inputs: WireInputs,
    rows: Vec<WireRow>,
    unbound_target_issues: Vec<WireUnboundIssue>,
    targets_current: bool,
    coverage_complete: bool,
    complete: bool,
    authority: String,
    non_claims: Vec<String>,
    receipt_blake3: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRevision {
    kind: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireInputs {
    case_blake3: String,
    policy_blake3: String,
    registry_blake3: String,
    targets_blake3: String,
    generated_sources_blake3: String,
    contracts: Vec<WireContractInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireContractInput {
    contract_id: String,
    #[serde(default)]
    producer_output_locator: Option<String>,
    contract_blake3: String,
    decision_blake3: String,
    evidence_blake3: String,
    observation_blake3: String,
    job_blake3: String,
    receipt_blake3: String,
    #[serde(default)]
    realization_blake3: Option<String>,
    #[serde(default)]
    store_info_blake3: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRow {
    requirement_id: String,
    specification_path: String,
    row_blake3: String,
    state: String,
    #[serde(default)]
    assurance: Option<String>,
    #[serde(default)]
    contract_id: Option<String>,
    #[serde(default)]
    job_id: Option<String>,
    #[serde(default)]
    bound: Option<WireBound>,
    #[serde(default)]
    issues: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBound {
    #[serde(default)]
    node_id: Option<String>,
    #[serde(default)]
    producer_job_identity: Option<String>,
    #[serde(default)]
    realization_route: Option<String>,
    #[serde(default)]
    receipt_identity: Option<String>,
    #[serde(default)]
    root_derivation: Option<String>,
    #[serde(default)]
    signer: Option<String>,
    #[serde(default)]
    source_revision: Option<String>,
    #[serde(default)]
    target_derivation: Option<String>,
    #[serde(default)]
    target_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireUnboundIssue {
    code: String,
    field_path: String,
}

// r[impl kiln.convergence.intake]
/// Admit one bounded `cairn.convergence.v1` receipt.
pub fn admit_convergence_receipt(
    bytes: &[u8],
    binding: &ConvergenceReceiptBinding,
) -> Result<AdmittedConvergenceReceipt, Diagnostic> {
    if bytes.len() > MAX_CONVERGENCE_RECEIPT_BYTES {
        return Err(Diagnostic::new(
            ISSUE_OVERSIZED,
            "convergence.receipt.bytes",
            "convergence receipt exceeds its named byte bound",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| malformed())?;
    let schema = value
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(malformed)?;
    if schema != CONVERGENCE_RECEIPT_SCHEMA_V1 {
        return Err(Diagnostic::new(
            ISSUE_UNKNOWN_SCHEMA,
            "convergence.receipt.schema_version",
            "convergence receipt schema is not admitted",
        ));
    }
    let wire: WireReceipt = serde_json::from_value(value.clone()).map_err(|_| malformed())?;
    if wire.schema_version != CONVERGENCE_RECEIPT_SCHEMA_V1 {
        return Err(Diagnostic::new(
            ISSUE_UNKNOWN_SCHEMA,
            "convergence.receipt.schema_version",
            "convergence receipt schema is not admitted",
        ));
    }
    if wire.status != CONVERGENCE_RECEIPT_STATUS {
        return Err(Diagnostic::new(
            ISSUE_UNKNOWN_STATUS,
            "convergence.receipt.status",
            "convergence receipt status is not admitted",
        ));
    }
    bounded(
        &wire.repository_id,
        MAX_CONVERGENCE_ID_BYTES,
        "convergence.receipt.repository_id",
    )?;
    bounded(
        &wire.authority,
        MAX_CONVERGENCE_TEXT_BYTES * 2,
        "convergence.receipt.authority",
    )?;
    if binding.revision_kind != CONVERGENCE_REVISION_KIND {
        return Err(binding_mismatch("convergence.binding.revision_kind"));
    }
    if wire.revision.kind != CONVERGENCE_REVISION_KIND {
        return Err(Diagnostic::new(
            ISSUE_INVALID_REVISION_KIND,
            "convergence.receipt.revision.kind",
            "only a Git revision binds a Kiln candidate",
        ));
    }
    let declared = ContentId::from_blake3_hex(binding.receipt_blake3.as_str())
        .map_err(|_| invalid_identity("convergence.binding.receipt_blake3"))?;
    let receipt_identity = ContentId::from_blake3_hex(&wire.receipt_blake3)
        .map_err(|_| invalid_identity("convergence.receipt.receipt_blake3"))?;
    let computed = canonical_receipt_identity(&value)?;
    if computed != receipt_identity {
        return Err(Diagnostic::new(
            ISSUE_RECEIPT_IDENTITY_MISMATCH,
            "convergence.receipt.receipt_blake3",
            "recomputed receipt BLAKE3 differs from the receipt field",
        ));
    }
    if receipt_identity != declared {
        return Err(binding_mismatch("convergence.binding.receipt_blake3"));
    }
    let registry_blake3 = content(
        &wire.inputs.registry_blake3,
        "convergence.inputs.registry_blake3",
    )?;
    let case_blake3 = content(&wire.inputs.case_blake3, "convergence.inputs.case_blake3")?;
    let policy_blake3 = content(
        &wire.inputs.policy_blake3,
        "convergence.inputs.policy_blake3",
    )?;
    if wire.revision.value != binding.revision {
        return Err(binding_mismatch("convergence.receipt.revision.value"));
    }
    if registry_blake3 != binding.registry_blake3 {
        return Err(binding_mismatch("convergence.inputs.registry_blake3"));
    }
    if case_blake3 != binding.case_blake3 {
        return Err(binding_mismatch("convergence.inputs.case_blake3"));
    }
    if policy_blake3 != binding.policy_blake3 {
        return Err(binding_mismatch("convergence.inputs.policy_blake3"));
    }
    let targets_blake3 = content(
        &wire.inputs.targets_blake3,
        "convergence.inputs.targets_blake3",
    )?;
    let generated_sources_blake3 = content(
        &wire.inputs.generated_sources_blake3,
        "convergence.inputs.generated_sources_blake3",
    )?;
    if wire.inputs.contracts.len() > MAX_CONVERGENCE_CONTRACTS {
        return Err(Diagnostic::new(
            ISSUE_ROW_BOUND,
            "convergence.inputs.contracts",
            "convergence contract input count exceeds its named bound",
        ));
    }
    for contract in &wire.inputs.contracts {
        bounded(
            &contract.contract_id,
            MAX_CONVERGENCE_ID_BYTES,
            "convergence.inputs.contracts.contract_id",
        )?;
        for (identity, field) in [
            (&contract.contract_blake3, "contract_blake3"),
            (&contract.decision_blake3, "decision_blake3"),
            (&contract.evidence_blake3, "evidence_blake3"),
            (&contract.observation_blake3, "observation_blake3"),
            (&contract.job_blake3, "job_blake3"),
            (&contract.receipt_blake3, "receipt_blake3"),
        ] {
            content(identity, &format!("convergence.inputs.contracts.{field}"))?;
        }
        if let Some(identity) = &contract.realization_blake3 {
            content(identity, "convergence.inputs.contracts.realization_blake3")?;
        }
        if let Some(identity) = &contract.store_info_blake3 {
            content(identity, "convergence.inputs.contracts.store_info_blake3")?;
        }
        if let Some(locator) = &contract.producer_output_locator {
            bounded(
                locator,
                MAX_CONVERGENCE_TEXT_BYTES,
                "convergence.inputs.contracts.producer_output_locator",
            )?;
        }
    }

    if wire.rows.len() > MAX_CONVERGENCE_ROWS {
        return Err(Diagnostic::new(
            ISSUE_ROW_BOUND,
            "convergence.rows",
            "convergence row count exceeds its named bound",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut rows = Vec::with_capacity(wire.rows.len());
    for row in &wire.rows {
        let state = state(&row.state)?;
        bounded(
            &row.requirement_id,
            MAX_CONVERGENCE_ID_BYTES,
            "convergence.rows.requirement_id",
        )?;
        bounded(
            &row.specification_path,
            MAX_CONVERGENCE_TEXT_BYTES,
            "convergence.rows.specification_path",
        )?;
        if !seen.insert(row.requirement_id.as_str()) {
            return Err(Diagnostic::new(
                ISSUE_DUPLICATE_REQUIREMENT,
                "convergence.rows.requirement_id",
                "a requirement identity appears more than once",
            ));
        }
        if let Some(assurance) = &row.assurance {
            bounded(
                assurance,
                MAX_CONVERGENCE_ID_BYTES,
                "convergence.rows.assurance",
            )?;
        }
        if let Some(contract) = &row.contract_id {
            bounded(
                contract,
                MAX_CONVERGENCE_ID_BYTES,
                "convergence.rows.contract_id",
            )?;
        }
        if let Some(job) = &row.job_id {
            bounded(job, MAX_CONVERGENCE_ID_BYTES, "convergence.rows.job_id")?;
        }
        for issue in &row.issues {
            bounded(issue, MAX_CONVERGENCE_ID_BYTES, "convergence.rows.issues")?;
        }
        if row.issues.len() > MAX_CONVERGENCE_ISSUES {
            return Err(Diagnostic::new(
                ISSUE_ROW_BOUND,
                "convergence.rows.issues",
                "row issue count exceeds its named bound",
            ));
        }
        if let Some(bound) = &row.bound {
            validate_bound(bound)?;
        }
        rows.push(EvidenceRow {
            requirement_id: row.requirement_id.clone(),
            specification_path: row.specification_path.clone(),
            row_blake3: content(&row.row_blake3, "convergence.rows.row_blake3")?,
            state,
            contract_id: row.contract_id.clone(),
            job_id: row.job_id.clone(),
        });
    }
    if wire.unbound_target_issues.len() > MAX_CONVERGENCE_ISSUES {
        return Err(Diagnostic::new(
            ISSUE_ROW_BOUND,
            "convergence.unbound_target_issues",
            "unbound target issue count exceeds its named bound",
        ));
    }
    let mut unbound_target_issues = Vec::with_capacity(wire.unbound_target_issues.len());
    for issue in &wire.unbound_target_issues {
        bounded(
            &issue.code,
            MAX_CONVERGENCE_ID_BYTES,
            "convergence.unbound_target_issues.code",
        )?;
        bounded(
            &issue.field_path,
            MAX_CONVERGENCE_TEXT_BYTES,
            "convergence.unbound_target_issues.field_path",
        )?;
        unbound_target_issues.push(ConvergenceUnboundIssue {
            code: issue.code.clone(),
            field_path: issue.field_path.clone(),
        });
    }
    if wire.non_claims.len() > MAX_CONVERGENCE_NON_CLAIMS {
        return Err(Diagnostic::new(
            ISSUE_TEXT_BOUND,
            "convergence.receipt.non_claims",
            "non-claim count exceeds its named bound",
        ));
    }
    for claim in &wire.non_claims {
        bounded(
            claim,
            MAX_CONVERGENCE_TEXT_BYTES * 2,
            "convergence.receipt.non_claims",
        )?;
    }
    Ok(AdmittedConvergenceReceipt {
        receipt_blake3: receipt_identity,
        repository_id: wire.repository_id,
        revision_kind: wire.revision.kind,
        revision: wire.revision.value,
        registry_blake3,
        case_blake3,
        policy_blake3,
        targets_blake3,
        generated_sources_blake3,
        rows,
        unbound_target_issues,
        targets_current: wire.targets_current,
        coverage_complete: wire.coverage_complete,
        complete: wire.complete,
        authority: wire.authority,
        non_claims: wire.non_claims,
    })
}

fn validate_bound(bound: &WireBound) -> Result<(), Diagnostic> {
    for (value, field) in [
        (&bound.producer_job_identity, "producer_job_identity"),
        (&bound.receipt_identity, "receipt_identity"),
    ] {
        if let Some(identity) = value {
            prefixed_identity(identity, &format!("convergence.rows.bound.{field}"))?;
        }
    }
    for (value, field) in [
        (&bound.node_id, "node_id"),
        (&bound.realization_route, "realization_route"),
        (&bound.root_derivation, "root_derivation"),
        (&bound.signer, "signer"),
        (&bound.source_revision, "source_revision"),
        (&bound.target_derivation, "target_derivation"),
        (&bound.target_path, "target_path"),
    ] {
        if let Some(text) = value {
            bounded(
                text,
                MAX_CONVERGENCE_TEXT_BYTES,
                &format!("convergence.rows.bound.{field}"),
            )?;
        }
    }
    Ok(())
}

fn prefixed_identity(value: &str, field: &str) -> Result<ContentId, Diagnostic> {
    let hex = value
        .strip_prefix("b3:")
        .ok_or_else(|| invalid_identity(field))?;
    content(hex, field)
}

fn content(value: &str, field: &str) -> Result<ContentId, Diagnostic> {
    ContentId::from_blake3_hex(value).map_err(|_| invalid_identity(field))
}

fn state(value: &str) -> Result<EvidenceState, Diagnostic> {
    match value {
        "untargeted" => Ok(EvidenceState::Untargeted),
        "not_projected" => Ok(EvidenceState::NotProjected),
        "current" => Ok(EvidenceState::Current),
        "unrealized" => Ok(EvidenceState::Unrealized),
        "stale" => Ok(EvidenceState::Stale),
        "failed" => Ok(EvidenceState::Failed),
        "unsupported" => Ok(EvidenceState::Unsupported),
        "incomplete" => Ok(EvidenceState::Incomplete),
        _ => Err(Diagnostic::new(
            ISSUE_UNKNOWN_STATE,
            "convergence.rows.state",
            "convergence row state is not admitted",
        )),
    }
}

fn bounded(value: &str, max_bytes: usize, field: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(Diagnostic::new(
            ISSUE_TEXT_BOUND,
            field,
            "value is empty, oversized or contains control characters",
        ));
    }
    Ok(())
}

fn invalid_identity(field: &str) -> Diagnostic {
    Diagnostic::new(
        ISSUE_INVALID_IDENTITY,
        field,
        "identity must be 64 hexadecimal characters",
    )
}

fn binding_mismatch(field: &str) -> Diagnostic {
    Diagnostic::new(
        ISSUE_BINDING_MISMATCH,
        field,
        "receipt facts differ from the declared intake binding",
    )
}

fn malformed() -> Diagnostic {
    Diagnostic::new(
        ISSUE_MALFORMED,
        "convergence.receipt",
        "convergence receipt is malformed or has an unknown field",
    )
}

/// Recompute Cairn's sealed receipt identity.
fn canonical_receipt_identity(value: &Value) -> Result<ContentId, Diagnostic> {
    let Some(mut object) = value.as_object().cloned() else {
        return Err(malformed());
    };
    object.remove("receipt_blake3");
    let canonical = canonical_value(Value::Object(object));
    let text = serde_json::to_string(&canonical).map_err(|_| malformed())?;
    let mut hasher = Hasher::new();
    hasher.update(text.as_bytes());
    ContentId::from_blake3_hex(hasher.finalize().to_hex().as_str()).map_err(|_| malformed())
}

fn canonical_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let mut canonical = serde_json::Map::new();
            for key in keys {
                if let Some(child) = object.get(&key) {
                    canonical.insert(key, canonical_value(child.clone()));
                }
            }
            Value::Object(canonical)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(canonical_value).collect()),
        scalar => scalar,
    }
}

#[cfg(test)]
mod tests;
