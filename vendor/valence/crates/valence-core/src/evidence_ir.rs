//! Evidence IR v1: canonical, serializable, type-checked intermediate
//! representation for evidence artifacts with span/provenance threading and
//! non-claim boundary enforcement.
//!
//! This module is the spine of the Valence Evidence IR. It defines the
//! unified canonical form that Function Objects, evidence index records, and
//! receipts can be carried in, plus a checker and a span explainer.
//!
//! # Design
//!
//! The IR is a typed Rust data model, not a language. It gives the stack
//! canonical-evidence-like discipline (canonical machine form + human projection +
//! span survival through the chain) without a compiler, without backends, and
//! without mechanized type-soundness.
//!
//! # Verification role
//!
//! Every artifact carries a [`VerificationRole`]. In Phase 0 the schema exists
//! and the checker validates it. In Phase 1, cairn `release-readiness` and
//! `evidence verify` count only [`VerificationRole::Property`] artifacts toward
//! "verified," so a "0 verified" receipt cannot be misread as "all good."
//!
//! # Non-claims
//!
//! Every artifact carries an explicit `non_claims` boundary. The checker
//! fails closed if the boundary is missing or weakened, mirroring the existing
//! [`crate::evidence_codebase_index::INDEX_BOUNDARY`] discipline.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::shared_identity::ArtifactIdentityDigest;
use crate::shared_identity::MIXED_BLAKE3_PROFILE;
use crate::shared_identity::SharedIdentityIssue;
use crate::shared_identity::canonical_json_bytes;

/// The Evidence IR v1 schema identifier.
pub const EVIDENCE_IR_SCHEMA: &str = "valence.evidence-ir.v1";

/// Required non-claim boundary text that every artifact must carry.
pub const REQUIRED_NON_CLAIM: &str =
    "does not claim behavioral correctness, semantic equivalence, or whole-program verification";

/// Kani contract evidence artifact kind.
pub const KANI_CONTRACT_ARTIFACT_KIND: &str = "kani-contract-evidence-v1";

/// Kani stub evidence artifact kind.
pub const KANI_STUB_ARTIFACT_KIND: &str = "kani-stub-evidence-v1";

/// Kani non-claim label: no whole-program correctness claim.
pub const KANI_NON_CLAIM_WHOLE_PROGRAM: &str = "kani.boundary.whole_program";

/// Kani non-claim label: no verifier, solver, CBMC, or Rust model soundness claim.
pub const KANI_NON_CLAIM_VERIFIER_SOUNDNESS: &str = "kani.boundary.verifier_soundness";

/// Kani non-claim label: aliasing assumptions remain explicit boundaries.
pub const KANI_NON_CLAIM_ALIASING: &str = "kani.boundary.aliasing";

/// Kani non-claim label: concurrency and async behavior remain explicit boundaries.
pub const KANI_NON_CLAIM_CONCURRENCY_ASYNC: &str = "kani.boundary.concurrency_async";

/// Kani non-claim label: FFI, plain stubs, and environmental models remain boundaries.
pub const KANI_NON_CLAIM_FFI_STUB: &str = "kani.boundary.ffi_stub";

/// Kani non-claim label: recursive termination requires separate complete evidence.
pub const KANI_NON_CLAIM_RECURSIVE_TERMINATION: &str = "kani.boundary.recursive_termination";

/// Kani non-claim label: bounded-only evidence is not unbounded verification.
pub const KANI_NON_CLAIM_BOUNDED_ONLY: &str = "kani.boundary.bounded_only";

const KANI_REQUIRED_NON_CLAIMS: &[&str] = &[
    KANI_NON_CLAIM_WHOLE_PROGRAM,
    KANI_NON_CLAIM_VERIFIER_SOUNDNESS,
    KANI_NON_CLAIM_ALIASING,
    KANI_NON_CLAIM_CONCURRENCY_ASYNC,
    KANI_NON_CLAIM_FFI_STUB,
    KANI_NON_CLAIM_RECURSIVE_TERMINATION,
    KANI_NON_CLAIM_BOUNDED_ONLY,
];

const KANI_CONTRACT_FACETS: &[&str] = &[
    "precondition",
    "postcondition",
    "loop_invariant",
    "loop_decreases",
    "quantifier",
    "contract_harness",
];

const KANI_CONTRACT_PASSING_CLASSIFICATION: &str = "passing_contract";
const KANI_CONTRACT_LOOP_CLASSIFICATION: &str = "partial_correctness_loop_contract";
const KANI_CONTRACT_BOUNDED_CLASSIFICATION: &str = "bounded_harness";
const KANI_RECEIPT_STATUS_PASSED: &str = "passed";
const KANI_RECEIPT_STATUS_FAILED: &str = "failed";
const KANI_RECEIPT_STATUS_BOUNDED_ONLY: &str = "bounded_only";
const KANI_STUB_KIND_VERIFIED_CONTRACT: &str = "verified_contract_stub";
const KANI_STUB_KIND_PLAIN: &str = "plain_stub";
const KANI_STUB_KIND_ENVIRONMENTAL: &str = "environmental_stub";
const KANI_TARGET_SCOPE_ALL_MONOMORPHIZATIONS: &str = "all_monomorphizations";

/// Overclaim strings the checker rejects if present in `non_claims` or `payload`.
const OVERCLAIM_MARKERS: &[&str] = &[
    "proves semantic equivalence",
    "proves whole-program correctness",
    "proves exhaustive coverage",
    "certified correct",
    "fully verified runtime",
    "whole-program Kani proof",
    "proves Kani verifier soundness",
    "proves solver soundness",
    "proves all Rust behavior",
    "covers all monomorphizations",
    "promotes bounded Kani evidence to unbounded verification",
    "plain stub verified",
];

/// The verification role of an evidence artifact.
///
/// Cairn role summaries and the applicable Valence/Cairn profile validators
/// count only `Property` artifacts as verified. `RecordedOnly` and `Boundary`
/// remain separate non-verification roles.
///
/// - `Property`: this artifact verifies a checkable predicate.
/// - `RecordedOnly`: this artifact records structure/identity only, proves nothing. A "0 passed, 0
///   failed" on recorded-only artifacts means "nothing was verified," not "all good."
/// - `Boundary`: non-claim text artifact, not a verification artifact at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationRole {
    Property,
    RecordedOnly,
    Boundary,
}

impl VerificationRole {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Property => "property",
            Self::RecordedOnly => "recorded_only",
            Self::Boundary => "boundary",
        }
    }
}

/// Typed Kani contract facets that Valence recognizes in contract evidence payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KaniContractFacet {
    Precondition,
    Postcondition,
    LoopInvariant,
    LoopDecreases,
    Quantifier,
    ContractHarness,
}

impl KaniContractFacet {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Precondition => "precondition",
            Self::Postcondition => "postcondition",
            Self::LoopInvariant => "loop_invariant",
            Self::LoopDecreases => "loop_decreases",
            Self::Quantifier => "quantifier",
            Self::ContractHarness => "contract_harness",
        }
    }
}

/// Typed Kani stub kinds that Valence recognizes in stub evidence payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KaniStubKind {
    VerifiedContractStub,
    PlainStub,
    EnvironmentalStub,
}

impl KaniStubKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VerifiedContractStub => KANI_STUB_KIND_VERIFIED_CONTRACT,
            Self::PlainStub => KANI_STUB_KIND_PLAIN,
            Self::EnvironmentalStub => KANI_STUB_KIND_ENVIRONMENTAL,
        }
    }
}

/// Extended source span carrying the stable identifier that threads through
/// the whole chain (source -> receipt -> binary).
///
/// The `source_id` is the stable identifier that
/// `valence explain --id <source_id>` resolves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSpan {
    /// Stable identifier threaded through the chain.
    pub source_id: String,
    /// Path to the source file.
    pub source_path: String,
    /// Zero-based byte offset of the span start.
    pub start_byte: u64,
    /// Zero-based byte offset of the span end (exclusive).
    pub end_byte: u64,
    /// One-based line number of the span start.
    pub start_line: u32,
    /// One-based column number of the span start.
    pub start_column: u32,
    /// One-based line number of the span end.
    pub end_line: u32,
    /// One-based column number of the span end.
    pub end_column: u32,
    /// Equation/requirement label if present.
    pub label: Option<String>,
}

/// Field-scoped structural issue for an evidence span.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SpanIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl SpanIssue {
    const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

/// Typed result of validating and searching a complete span manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplainSpanResult {
    Found(EvidenceSpan),
    NotFound,
    InvalidManifest(Vec<SpanIssue>),
}

/// Validate one span without reading its source file.
#[must_use]
pub fn validate_evidence_span(span: &EvidenceSpan) -> Vec<SpanIssue> {
    let mut issues = Vec::new();
    validate_span_text(&span.source_id, "source_id", &mut issues);
    validate_span_text(&span.source_path, "source_path", &mut issues);
    if let Some(label) = &span.label {
        validate_span_text(label, "label", &mut issues);
    }
    if span.start_byte > span.end_byte {
        issues.push(SpanIssue::new("byte_range", "span_bytes_reversed"));
    }
    for (field, value) in [
        ("start_line", span.start_line),
        ("start_column", span.start_column),
        ("end_line", span.end_line),
        ("end_column", span.end_column),
    ] {
        if value == 0 {
            issues.push(SpanIssue::new(field, "span_coordinate_zero"));
        }
    }
    if (span.end_line, span.end_column) < (span.start_line, span.start_column) {
        issues.push(SpanIssue::new("position_range", "span_position_reversed"));
    }
    issues.sort();
    issues.dedup();
    issues
}

fn validate_span_text(value: &str, field: &'static str, issues: &mut Vec<SpanIssue>) {
    if value.is_empty() {
        issues.push(SpanIssue::new(field, "span_text_empty"));
    } else if value.trim() != value {
        issues.push(SpanIssue::new(field, "span_text_padded"));
    }
    if value.chars().any(char::is_control) {
        issues.push(SpanIssue::new(field, "span_text_control_character"));
    }
}

/// A canonical Evidence IR artifact: the unified form for Function Objects,
/// evidence index records, and receipts.
///
/// The `canonical_hash` is computed over [`EvidenceArtifactHashPayload`], which
/// excludes `canonical_hash` and `source_span` so identity is stable across
/// path relocation and independent of its own hash output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceArtifact {
    /// Schema identifier, always [`EVIDENCE_IR_SCHEMA`].
    pub schema: String,
    /// Kind of artifact: "function-object-v1", "evidence-index-record", "receipt", etc.
    pub artifact_kind: String,
    /// The stable ID that threads through the chain.
    pub stable_id: String,
    /// Canonical BLAKE3 hash of the hash payload.
    pub canonical_hash: String,
    /// Provenance back to source, if known.
    pub source_span: Option<EvidenceSpan>,
    /// What this artifact verifies (or does not).
    pub verification_role: VerificationRole,
    /// Kind-specific payload.
    pub payload: serde_json::Value,
    /// Explicit non-claim boundary strings.
    pub non_claims: Vec<String>,
}

/// Reviewable typed hash material for Evidence IR artifact identity.
///
/// Exactly these fields are included in the canonical BLAKE3 hash. Display
/// fields (`source_span`) and the computed `canonical_hash` itself are
/// deliberately excluded so identity is stable across relocation.
#[derive(Debug, Clone, Serialize)]
struct EvidenceArtifactHashPayload<'a> {
    schema: &'a str,
    artifact_kind: &'a str,
    stable_id: &'a str,
    verification_role: &'a str,
    payload: &'a serde_json::Value,
    non_claims: &'a [String],
}

/// Check report returned by [`check_evidence_artifact`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceCheckReport {
    pub valid: bool,
    pub issue_codes: Vec<&'static str>,
    pub non_claims: Vec<String>,
}

/// Build a canonical Evidence IR artifact from typed inputs.
///
/// # Errors
///
/// Returns [`EvidenceIrError`] if the payload cannot be serialized into
/// canonical JSON for hashing.
pub fn build_evidence_artifact(
    artifact_kind: &str,
    stable_id: &str,
    source_span: Option<EvidenceSpan>,
    verification_role: VerificationRole,
    payload: serde_json::Value,
    non_claims: Vec<String>,
) -> Result<EvidenceArtifact, EvidenceIrError> {
    if let Some(span) = &source_span {
        let issues = validate_evidence_span(span);
        if !issues.is_empty() {
            return Err(EvidenceIrError::InvalidSourceSpan { issues });
        }
    }
    let mut artifact = EvidenceArtifact {
        schema: EVIDENCE_IR_SCHEMA.to_owned(),
        artifact_kind: artifact_kind.to_owned(),
        stable_id: stable_id.to_owned(),
        canonical_hash: String::new(),
        source_span,
        verification_role,
        payload,
        non_claims,
    };
    artifact.canonical_hash = canonical_hash(&artifact)?;
    Ok(artifact)
}

/// Computes the canonical BLAKE3 identity for an Evidence IR artifact.
///
/// # Errors
///
/// Returns [`EvidenceIrError`] if the hash payload cannot be serialized.
pub fn canonical_hash(artifact: &EvidenceArtifact) -> Result<String, EvidenceIrError> {
    let payload = EvidenceArtifactHashPayload {
        schema: &artifact.schema,
        artifact_kind: &artifact.artifact_kind,
        stable_id: &artifact.stable_id,
        verification_role: artifact.verification_role.as_str(),
        payload: &artifact.payload,
        non_claims: &artifact.non_claims,
    };
    let value = serde_json::to_value(&payload).map_err(|error| EvidenceIrError::CanonicalSerialization {
        detail: error.to_string(),
    })?;
    let bytes = canonical_json_bytes(&value).map_err(|error| EvidenceIrError::CanonicalSerialization {
        detail: format!("{}:{}", error.field, error.code),
    })?;
    Ok(format!("b3:{}", blake3::hash(&bytes).to_hex()))
}

/// Admit the public canonical-hash field into the typed artifact identity core.
pub fn artifact_identity(artifact: &EvidenceArtifact) -> Result<ArtifactIdentityDigest, SharedIdentityIssue> {
    ArtifactIdentityDigest::parse(&artifact.canonical_hash, MIXED_BLAKE3_PROFILE, "canonical_hash")
}

/// Validates a JSON-serialized Evidence IR artifact.
///
/// Checks: schema field, artifact_kind non-empty, stable_id non-empty,
/// canonical_hash matches recomputed hash, non_claims present and not
/// weakened, verification_role valid, no overclaim markers.
#[must_use]
pub fn check_evidence_artifact(json: &str) -> EvidenceCheckReport {
    let mut issue_codes = Vec::new();

    // r[impl valence.evidence_ir.complete_envelope.strict_decode]
    // r[impl valence.evidence_ir.complete_envelope.unknown_fields]
    let envelope: Value = match serde_json::from_str(json) {
        Ok(envelope) => envelope,
        Err(error) => {
            let code = if error.to_string().contains("trailing characters") {
                "trailing_json_value"
            } else {
                "malformed_json"
            };
            return invalid_evidence_report(code);
        }
    };
    inventory_closed_envelope(&envelope, &mut issue_codes);
    if !issue_codes.is_empty() {
        issue_codes.sort_unstable();
        issue_codes.dedup();
        return EvidenceCheckReport {
            valid: false,
            issue_codes,
            non_claims: Vec::new(),
        };
    }

    let artifact: EvidenceArtifact = match serde_json::from_str(json) {
        Ok(artifact) => artifact,
        Err(error) => {
            let code = if error.to_string().contains("duplicate field") {
                "duplicate_control_field"
            } else {
                "typed_field_mismatch"
            };
            return invalid_evidence_report(code);
        }
    };

    if artifact.schema != EVIDENCE_IR_SCHEMA {
        issue_codes.push("schema_mismatch");
    }
    if artifact.artifact_kind.trim().is_empty() {
        issue_codes.push("artifact_kind_empty");
    }
    if artifact.stable_id.trim().is_empty() {
        issue_codes.push("stable_id_empty");
    }
    if let Some(span) = &artifact.source_span {
        issue_codes.extend(validate_evidence_span(span).into_iter().map(|issue| issue.code));
    }

    let expected_hash = match canonical_hash(&artifact) {
        Ok(hash) => hash,
        Err(_) => {
            issue_codes.push("canonical_hash_compute_failed");
            String::new()
        }
    };
    if !expected_hash.is_empty() && artifact.canonical_hash != expected_hash {
        issue_codes.push("canonical_hash_mismatch");
    }

    if artifact.non_claims.is_empty() {
        issue_codes.push("non_claims_missing");
    } else {
        let joined = artifact.non_claims.join(" ");
        if !joined.contains(REQUIRED_NON_CLAIM) {
            issue_codes.push("non_claims_weakened");
        }
    }

    validate_kani_artifact(&artifact, &mut issue_codes);

    let full_json = serde_json::to_string(&artifact.payload).unwrap_or_default();
    let non_claims_joined = artifact.non_claims.join(" ");
    for marker in OVERCLAIM_MARKERS {
        if full_json.contains(marker) || non_claims_joined.contains(marker) {
            issue_codes.push("overclaim_detected");
            break;
        }
    }

    issue_codes.sort_unstable();
    issue_codes.dedup();
    EvidenceCheckReport {
        valid: issue_codes.is_empty(),
        issue_codes,
        non_claims: artifact.non_claims,
    }
}

fn invalid_evidence_report(code: &'static str) -> EvidenceCheckReport {
    EvidenceCheckReport {
        valid: false,
        issue_codes: vec![code],
        non_claims: Vec::new(),
    }
}

fn inventory_closed_envelope(envelope: &Value, issue_codes: &mut Vec<&'static str>) {
    const ARTIFACT_FIELDS: &[&str] = &[
        "schema",
        "artifact_kind",
        "stable_id",
        "canonical_hash",
        "source_span",
        "verification_role",
        "payload",
        "non_claims",
    ];
    const SPAN_FIELDS: &[&str] = &[
        "source_id",
        "source_path",
        "start_byte",
        "end_byte",
        "start_line",
        "start_column",
        "end_line",
        "end_column",
        "label",
    ];

    let Some(fields) = envelope.as_object() else {
        issue_codes.push("typed_field_mismatch");
        return;
    };
    if fields.keys().any(|field| !ARTIFACT_FIELDS.contains(&field.as_str())) {
        issue_codes.push("unknown_artifact_field");
    }
    if let Some(span) = fields.get("source_span").filter(|span| !span.is_null())
        && let Some(span_fields) = span.as_object()
        && span_fields.keys().any(|field| !SPAN_FIELDS.contains(&field.as_str()))
    {
        issue_codes.push("unknown_span_field");
    }
}

fn validate_kani_artifact(artifact: &EvidenceArtifact, issue_codes: &mut Vec<&'static str>) {
    match artifact.artifact_kind.as_str() {
        KANI_CONTRACT_ARTIFACT_KIND => {
            validate_kani_non_claims(artifact, issue_codes);
            validate_kani_contract_artifact(artifact, issue_codes);
        }
        KANI_STUB_ARTIFACT_KIND => {
            validate_kani_non_claims(artifact, issue_codes);
            validate_kani_stub_artifact(artifact, issue_codes);
        }
        _ => {}
    }
}

fn validate_kani_non_claims(artifact: &EvidenceArtifact, issue_codes: &mut Vec<&'static str>) {
    for required in KANI_REQUIRED_NON_CLAIMS {
        if !artifact.non_claims.iter().any(|claim| claim == required) {
            issue_codes.push("kani_non_claims_missing_required");
            break;
        }
    }
}

fn validate_kani_contract_artifact(artifact: &EvidenceArtifact, issue_codes: &mut Vec<&'static str>) {
    let payload = &artifact.payload;
    let facets = object_array_strings(payload, "contract_facets");
    if facets.is_empty() {
        issue_codes.push("kani_contract_facets_missing");
    }
    if facets.iter().any(|facet| !KANI_CONTRACT_FACETS.contains(facet)) {
        issue_codes.push("kani_contract_unknown_facet");
    }

    validate_kani_contract_scope(payload, issue_codes);
    validate_kani_contract_toolchain(payload, issue_codes);
    validate_kani_contract_receipt(artifact, payload, issue_codes);
    validate_kani_contract_classification(artifact, payload, &facets, issue_codes);
}

fn validate_kani_contract_scope(payload: &serde_json::Value, issue_codes: &mut Vec<&'static str>) {
    let Some(target) = payload.get("target") else {
        issue_codes.push("kani_contract_target_scope_missing");
        return;
    };
    let required_scope_fields = ["crate_target", "source_id", "target_scope", "monomorphization"];
    if required_scope_fields.iter().any(|field| object_string(target, field).is_none()) {
        issue_codes.push("kani_contract_target_scope_missing");
    }
    if object_string(target, "target_scope") == Some(KANI_TARGET_SCOPE_ALL_MONOMORPHIZATIONS) {
        issue_codes.push("kani_contract_all_monomorphization_overclaim");
    }
}

fn validate_kani_contract_toolchain(payload: &serde_json::Value, issue_codes: &mut Vec<&'static str>) {
    let Some(toolchain) = payload.get("toolchain") else {
        issue_codes.push("kani_contract_toolchain_missing");
        issue_codes.push("kani_contract_solver_missing");
        return;
    };
    let required_toolchain_fields = ["kani_version", "rust_toolchain", "cbmc_version"];
    if required_toolchain_fields.iter().any(|field| object_string(toolchain, field).is_none()) {
        issue_codes.push("kani_contract_toolchain_missing");
    }
    if object_string(toolchain, "solver").is_none() {
        issue_codes.push("kani_contract_solver_missing");
    }
}

fn validate_kani_contract_receipt(
    artifact: &EvidenceArtifact,
    payload: &serde_json::Value,
    issue_codes: &mut Vec<&'static str>,
) {
    let Some(receipt) = payload.get("receipt") else {
        issue_codes.push("kani_contract_receipt_missing");
        issue_codes.push("kani_contract_receipt_status_missing");
        return;
    };
    if object_string(receipt, "receipt_hash").is_none() {
        issue_codes.push("kani_contract_receipt_missing");
    }
    match object_string(receipt, "status") {
        Some(KANI_RECEIPT_STATUS_PASSED) => {}
        Some(KANI_RECEIPT_STATUS_FAILED) => {
            if artifact.verification_role == VerificationRole::Property {
                issue_codes.push("kani_contract_failed_property");
            }
        }
        Some(KANI_RECEIPT_STATUS_BOUNDED_ONLY) => {
            if artifact.verification_role == VerificationRole::Property {
                issue_codes.push("kani_contract_bounded_only_promoted");
            }
        }
        Some(_) => issue_codes.push("kani_contract_receipt_status_unknown"),
        None => issue_codes.push("kani_contract_receipt_status_missing"),
    }
}

fn validate_kani_contract_classification(
    artifact: &EvidenceArtifact,
    payload: &serde_json::Value,
    facets: &[&str],
    issue_codes: &mut Vec<&'static str>,
) {
    match object_string(payload, "claim_classification") {
        Some(KANI_CONTRACT_PASSING_CLASSIFICATION | KANI_CONTRACT_LOOP_CLASSIFICATION) => {}
        Some(KANI_CONTRACT_BOUNDED_CLASSIFICATION) => {
            if artifact.verification_role == VerificationRole::Property {
                issue_codes.push("kani_contract_bounded_only_promoted");
            }
        }
        Some(_) => issue_codes.push("kani_contract_claim_classification_unknown"),
        None => issue_codes.push("kani_contract_claim_classification_missing"),
    }
    if object_bool(payload, "termination_claimed") == Some(true) && !facets.contains(&"loop_decreases") {
        issue_codes.push("kani_contract_recursive_termination_overclaim");
    }
}

fn validate_kani_stub_artifact(artifact: &EvidenceArtifact, issue_codes: &mut Vec<&'static str>) {
    match object_string(&artifact.payload, "stub_kind") {
        Some(KANI_STUB_KIND_VERIFIED_CONTRACT) => validate_verified_kani_stub(artifact, issue_codes),
        Some(KANI_STUB_KIND_PLAIN) => validate_plain_kani_stub(artifact, issue_codes),
        Some(KANI_STUB_KIND_ENVIRONMENTAL) => validate_environmental_kani_stub(artifact, issue_codes),
        Some(_) => issue_codes.push("kani_stub_kind_unknown"),
        None => issue_codes.push("kani_stub_kind_missing"),
    }
}

fn validate_verified_kani_stub(artifact: &EvidenceArtifact, issue_codes: &mut Vec<&'static str>) {
    let payload = &artifact.payload;
    let Some(linked_contract) = payload.get("linked_contract") else {
        issue_codes.push("kani_stub_contract_link_missing");
        return;
    };
    let original_function = object_string(payload, "original_function");
    let source_scope = object_string(payload, "source_scope");
    if original_function.is_none() || source_scope.is_none() {
        issue_codes.push("kani_stub_contract_link_missing");
    }
    if original_function != object_string(linked_contract, "original_function") {
        issue_codes.push("kani_stub_contract_link_mismatch");
    }
    if source_scope != object_string(linked_contract, "source_scope") {
        issue_codes.push("kani_stub_contract_link_mismatch");
    }
    if object_string(payload, "stub_signature") != object_string(payload, "original_signature") {
        issue_codes.push("kani_stub_signature_mismatch");
    }
    if object_string(linked_contract, "artifact_hash") != object_string(linked_contract, "expected_artifact_hash") {
        issue_codes.push("kani_stub_stale_contract_hash");
    }
    if object_string(linked_contract, "status") != Some(KANI_RECEIPT_STATUS_PASSED) {
        issue_codes.push("kani_stub_contract_not_passed");
    }
}

fn validate_plain_kani_stub(artifact: &EvidenceArtifact, issue_codes: &mut Vec<&'static str>) {
    if artifact.verification_role == VerificationRole::Property {
        issue_codes.push("kani_stub_plain_stub_promoted");
    }
    if object_array_strings(&artifact.payload, "assumptions").is_empty() {
        issue_codes.push("kani_stub_assumptions_missing");
    }
}

fn validate_environmental_kani_stub(artifact: &EvidenceArtifact, issue_codes: &mut Vec<&'static str>) {
    if artifact.verification_role == VerificationRole::Property {
        issue_codes.push("kani_stub_environment_promoted");
    }
    if object_array_strings(&artifact.payload, "assumptions").is_empty() {
        issue_codes.push("kani_stub_environment_assumptions_missing");
    }
}

fn object_string<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str).map(str::trim).filter(|item| !item.is_empty())
}

fn object_bool(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(serde_json::Value::as_bool)
}

fn object_array_strings<'a>(value: &'a serde_json::Value, key: &str) -> Vec<&'a str> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect()
}

/// Validate every manifest row and resolve a unique stable source ID.
#[must_use]
pub fn explain_span(manifest_json: &str, source_id: &str) -> ExplainSpanResult {
    let spans: Vec<EvidenceSpan> = match serde_json::from_str(manifest_json) {
        Ok(spans) => spans,
        Err(_) => {
            return ExplainSpanResult::InvalidManifest(vec![SpanIssue::new("manifest", "manifest_malformed_json")]);
        }
    };
    let mut issues = spans.iter().flat_map(validate_evidence_span).collect::<Vec<_>>();
    let mut ids = std::collections::BTreeSet::new();
    for span in &spans {
        if !ids.insert(span.source_id.as_str()) {
            issues.push(SpanIssue::new("source_id", "manifest_duplicate_source_id"));
        }
    }
    issues.sort();
    issues.dedup();
    if !issues.is_empty() {
        return ExplainSpanResult::InvalidManifest(issues);
    }
    match spans.into_iter().find(|span| span.source_id == source_id) {
        Some(span) => ExplainSpanResult::Found(span),
        None => ExplainSpanResult::NotFound,
    }
}

/// Construction and validation errors for Evidence IR artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceIrError {
    CanonicalSerialization { detail: String },
    InvalidSourceSpan { issues: Vec<SpanIssue> },
}

impl std::fmt::Display for EvidenceIrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CanonicalSerialization { detail } => {
                write!(f, "canonicalize evidence artifact: {detail}")
            }
            Self::InvalidSourceSpan { issues } => {
                write!(f, "invalid evidence source span: {issues:?}")
            }
        }
    }
}

impl std::error::Error for EvidenceIrError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_span() -> EvidenceSpan {
        EvidenceSpan {
            source_id: "eq:bs_001".to_owned(),
            source_path: "src/pricing.rs".to_owned(),
            start_byte: 142,
            end_byte: 168,
            start_line: 7,
            start_column: 12,
            end_line: 7,
            end_column: 38,
            label: Some("eq:bs".to_owned()),
        }
    }

    fn sample_payload() -> serde_json::Value {
        serde_json::json!({
            "function": "black_scholes_call",
            "body_hash": "b3:abc123",
        })
    }

    fn sample_non_claims() -> Vec<String> {
        vec![
            "evidence artifact records identity and linkage only".to_owned(),
            REQUIRED_NON_CLAIM.to_owned(),
        ]
    }

    fn sample_artifact() -> EvidenceArtifact {
        build_evidence_artifact(
            "function-object-v1",
            "eq:bs_001",
            Some(sample_span()),
            VerificationRole::RecordedOnly,
            sample_payload(),
            sample_non_claims(),
        )
        .expect("build sample artifact")
    }

    fn sample_kani_non_claims() -> Vec<String> {
        let mut claims = sample_non_claims();
        claims.extend(KANI_REQUIRED_NON_CLAIMS.iter().map(ToString::to_string));
        claims
    }

    fn valid_contract_payload(facets: Vec<&str>, status: &str, classification: &str) -> serde_json::Value {
        serde_json::json!({
            "contract_facets": facets,
            "target": {
                "crate_target": "valence-core",
                "source_id": "src/lib.rs::checked_add",
                "target_scope": "fn checked_add::<u32>",
                "monomorphization": "u32",
            },
            "toolchain": {
                "kani_version": "kani fixture",
                "rust_toolchain": "nightly fixture",
                "cbmc_version": "cbmc fixture",
                "solver": "minisat fixture",
            },
            "receipt": {
                "status": status,
                "receipt_hash": "b3:kani-contract-receipt",
            },
            "claim_classification": classification,
            "termination_claimed": false,
        })
    }

    fn contract_artifact(payload: serde_json::Value, role: VerificationRole) -> EvidenceArtifact {
        build_evidence_artifact(
            KANI_CONTRACT_ARTIFACT_KIND,
            "kani:contract:checked_add",
            None,
            role,
            payload,
            sample_kani_non_claims(),
        )
        .expect("build Kani contract artifact")
    }

    fn valid_verified_stub_payload() -> serde_json::Value {
        serde_json::json!({
            "stub_kind": KaniStubKind::VerifiedContractStub.as_str(),
            "original_function": "crate::checked_add",
            "source_scope": "src/lib.rs",
            "stub_signature": "fn checked_add(u32, u32) -> Option<u32>",
            "original_signature": "fn checked_add(u32, u32) -> Option<u32>",
            "linked_contract": {
                "original_function": "crate::checked_add",
                "source_scope": "src/lib.rs",
                "artifact_hash": "b3:contract-artifact",
                "expected_artifact_hash": "b3:contract-artifact",
                "status": "passed",
            },
        })
    }

    fn stub_artifact(payload: serde_json::Value, role: VerificationRole) -> EvidenceArtifact {
        build_evidence_artifact(
            KANI_STUB_ARTIFACT_KIND,
            "kani:stub:checked_add",
            None,
            role,
            payload,
            sample_kani_non_claims(),
        )
        .expect("build Kani stub artifact")
    }

    fn report_for(artifact: EvidenceArtifact) -> EvidenceCheckReport {
        let json = serde_json::to_string(&artifact).expect("serialize artifact");
        check_evidence_artifact(&json)
    }

    #[test]
    fn artifact_hash_is_stable_for_same_core_inputs() {
        let one = sample_artifact();
        let two = sample_artifact();
        assert_eq!(one.canonical_hash, two.canonical_hash);
        assert_eq!(one.schema, EVIDENCE_IR_SCHEMA);
    }

    #[test]
    fn hash_excludes_display_fields() {
        let artifact = sample_artifact();

        // Moving the source span should not change the hash.
        let mut relocated = artifact.clone();
        relocated.source_span = Some(EvidenceSpan {
            source_path: "/elsewhere/src/pricing.rs".to_owned(),
            start_byte: 0,
            ..sample_span()
        });
        assert_eq!(
            canonical_hash(&artifact).unwrap(),
            canonical_hash(&relocated).unwrap(),
            "hash must be stable across path relocation"
        );

        // Changing the canonical_hash field itself must not change the recomputed hash.
        let mut tampered = artifact.clone();
        tampered.canonical_hash = "b3:deadbeef".to_owned();
        assert_eq!(
            canonical_hash(&artifact).unwrap(),
            canonical_hash(&tampered).unwrap(),
            "hash must be independent of its own output"
        );
    }

    #[test]
    fn check_passes_for_valid_artifact() {
        let artifact = sample_artifact();
        let json = serde_json::to_string(&artifact).unwrap();
        let report = check_evidence_artifact(&json);
        assert!(report.valid);
        assert!(report.issue_codes.is_empty());
    }

    #[test]
    fn strict_envelope_accepts_open_hash_bound_payload() {
        // r[verify valence.evidence_ir.complete_envelope.fixtures.positive]
        let artifact = build_evidence_artifact(
            "producer-extension",
            "extension:one",
            None,
            VerificationRole::RecordedOnly,
            serde_json::json!({"producer_extension": {"new_field": true}}),
            sample_non_claims(),
        )
        .expect("open payload builds");

        let report = check_evidence_artifact(&serde_json::to_string(&artifact).expect("serialize artifact"));

        assert!(report.valid, "unexpected issues: {:?}", report.issue_codes);
    }

    #[test]
    fn strict_envelope_rejects_unknown_duplicate_and_trailing_control_data() {
        // r[verify valence.evidence_ir.complete_envelope.fixtures.negative]
        let artifact = sample_artifact();
        let mut unknown = serde_json::to_value(&artifact).expect("artifact value");
        unknown
            .as_object_mut()
            .expect("artifact object")
            .insert("claims".to_owned(), serde_json::json!(["proves whole-program correctness"]));
        let unknown_report = check_evidence_artifact(&serde_json::to_string(&unknown).expect("unknown JSON"));
        assert_eq!(unknown_report.issue_codes, vec!["unknown_artifact_field"]);

        let mut unknown_span = serde_json::to_value(&artifact).expect("artifact value");
        unknown_span["source_span"]["display_hint"] = serde_json::json!("unbound");
        let span_report = check_evidence_artifact(&serde_json::to_string(&unknown_span).expect("span JSON"));
        assert_eq!(span_report.issue_codes, vec!["unknown_span_field"]);

        let json = serde_json::to_string(&artifact).expect("artifact JSON");
        let duplicate = json.replacen("{\"schema\":", "{\"schema\":\"valence.evidence-ir.v1\",\"schema\":", 1);
        let duplicate_report = check_evidence_artifact(&duplicate);
        assert_eq!(duplicate_report.issue_codes, vec!["duplicate_control_field"]);

        let trailing_report = check_evidence_artifact(&format!("{json} null"));
        assert_eq!(trailing_report.issue_codes, vec!["trailing_json_value"]);
    }

    #[test]
    fn strict_envelope_preserves_legacy_artifact_identity() {
        // r[verify valence.evidence_ir.complete_envelope.compatibility]
        const LEGACY_SAMPLE_HASH: &str = "b3:27f5aec990efd7f4e1e3956f6c7fae4e433596352ab78ff1c3b63a772cd4c0dd";
        let artifact = sample_artifact();
        assert_eq!(artifact.canonical_hash, LEGACY_SAMPLE_HASH);
        assert_eq!(
            artifact_identity(&artifact).expect("legacy canonical hash admits as an artifact identity").as_hex(),
            LEGACY_SAMPLE_HASH.trim_start_matches("b3:")
        );
    }

    #[test]
    fn typed_artifact_identity_rejects_malformed_wire_hash() {
        let mut artifact = sample_artifact();
        artifact.canonical_hash = "b3:not-a-blake3-digest".to_owned();
        let issue = artifact_identity(&artifact).expect_err("malformed artifact identity must fail");
        assert_eq!(issue.field, "canonical_hash");
        assert_eq!(issue.code, "malformed_blake3");
    }

    #[test]
    fn check_fails_closed_for_malformed_json() {
        let report = check_evidence_artifact("{ not json");
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"malformed_json"));
    }

    #[test]
    fn check_fails_closed_for_schema_mismatch() {
        let mut artifact = sample_artifact();
        artifact.schema = "wrong.schema".to_owned();
        let json = serde_json::to_string(&artifact).unwrap();
        let report = check_evidence_artifact(&json);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"schema_mismatch"));
    }

    #[test]
    fn check_fails_closed_for_empty_stable_id() {
        let artifact = build_evidence_artifact(
            "test",
            "",
            None,
            VerificationRole::RecordedOnly,
            serde_json::json!({}),
            sample_non_claims(),
        )
        .unwrap();
        let json = serde_json::to_string(&artifact).unwrap();
        let report = check_evidence_artifact(&json);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"stable_id_empty"));
    }

    #[test]
    fn check_fails_closed_for_hash_mismatch() {
        let mut artifact = sample_artifact();
        artifact.canonical_hash = "b3:deadbeef".to_owned();
        let json = serde_json::to_string(&artifact).unwrap();
        let report = check_evidence_artifact(&json);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"canonical_hash_mismatch"));
    }

    #[test]
    fn check_fails_closed_for_missing_non_claims() {
        let artifact =
            build_evidence_artifact("test", "id", None, VerificationRole::RecordedOnly, serde_json::json!({}), vec![])
                .unwrap();
        let json = serde_json::to_string(&artifact).unwrap();
        let report = check_evidence_artifact(&json);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"non_claims_missing"));
    }

    #[test]
    fn check_fails_closed_for_weakened_non_claims() {
        let artifact =
            build_evidence_artifact("test", "id", None, VerificationRole::RecordedOnly, serde_json::json!({}), vec![
                "this is fine".to_owned(),
            ])
            .unwrap();
        let json = serde_json::to_string(&artifact).unwrap();
        let report = check_evidence_artifact(&json);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"non_claims_weakened"));
    }

    #[test]
    fn check_rejects_overclaim_in_payload() {
        let artifact = build_evidence_artifact(
            "test",
            "id",
            None,
            VerificationRole::Property,
            serde_json::json!({"claim": "proves semantic equivalence"}),
            sample_non_claims(),
        )
        .unwrap();
        let json = serde_json::to_string(&artifact).unwrap();
        let report = check_evidence_artifact(&json);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"overclaim_detected"));
    }

    #[test]
    fn check_rejects_overclaim_in_non_claims() {
        let artifact =
            build_evidence_artifact("test", "id", None, VerificationRole::Property, serde_json::json!({}), vec![
                "proves whole-program correctness".to_owned(),
                REQUIRED_NON_CLAIM.to_owned(),
            ])
            .unwrap();
        let json = serde_json::to_string(&artifact).unwrap();
        let report = check_evidence_artifact(&json);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"overclaim_detected"));
    }

    #[test]
    fn verification_role_serializes_to_snake_case() {
        assert_eq!(VerificationRole::Property.as_str(), "property");
        assert_eq!(VerificationRole::RecordedOnly.as_str(), "recorded_only");
        assert_eq!(VerificationRole::Boundary.as_str(), "boundary");

        let json = serde_json::to_string(&VerificationRole::RecordedOnly).unwrap();
        assert_eq!(json, "\"recorded_only\"");
    }

    #[test]
    fn kani_contract_accepts_scoped_passing_function_contract() {
        let payload = valid_contract_payload(
            vec![
                KaniContractFacet::Precondition.as_str(),
                KaniContractFacet::Postcondition.as_str(),
                KaniContractFacet::ContractHarness.as_str(),
            ],
            KANI_RECEIPT_STATUS_PASSED,
            KANI_CONTRACT_PASSING_CLASSIFICATION,
        );
        let report = report_for(contract_artifact(payload, VerificationRole::Property));

        assert!(report.valid);
        assert!(report.issue_codes.is_empty());
        assert!(report.non_claims.contains(&KANI_NON_CLAIM_WHOLE_PROGRAM.to_owned()));
    }

    #[test]
    fn kani_contract_accepts_loop_decreases_for_termination_claim() {
        let mut payload = valid_contract_payload(
            vec![
                KaniContractFacet::LoopInvariant.as_str(),
                KaniContractFacet::LoopDecreases.as_str(),
                KaniContractFacet::ContractHarness.as_str(),
            ],
            KANI_RECEIPT_STATUS_PASSED,
            KANI_CONTRACT_LOOP_CLASSIFICATION,
        );
        payload["termination_claimed"] = serde_json::json!(true);
        let report = report_for(contract_artifact(payload, VerificationRole::Property));

        assert!(report.valid);
        assert!(report.issue_codes.is_empty());
    }

    #[test]
    fn kani_contract_keeps_bounded_harness_recorded_only() {
        let payload = valid_contract_payload(
            vec![KaniContractFacet::ContractHarness.as_str()],
            KANI_RECEIPT_STATUS_BOUNDED_ONLY,
            KANI_CONTRACT_BOUNDED_CLASSIFICATION,
        );
        let report = report_for(contract_artifact(payload, VerificationRole::RecordedOnly));

        assert!(report.valid);
        assert!(report.issue_codes.is_empty());
    }

    #[test]
    fn kani_contract_rejects_failed_receipt_as_property() {
        let payload = valid_contract_payload(
            vec![KaniContractFacet::Postcondition.as_str()],
            KANI_RECEIPT_STATUS_FAILED,
            KANI_CONTRACT_PASSING_CLASSIFICATION,
        );
        let report = report_for(contract_artifact(payload, VerificationRole::Property));

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"kani_contract_failed_property"));
    }

    #[test]
    fn kani_contract_rejects_missing_target_solver_and_receipt() {
        let payload = serde_json::json!({
            "contract_facets": [KaniContractFacet::Postcondition.as_str()],
            "toolchain": {"kani_version": "kani fixture"},
            "claim_classification": KANI_CONTRACT_PASSING_CLASSIFICATION,
        });
        let report = report_for(contract_artifact(payload, VerificationRole::Property));

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"kani_contract_target_scope_missing"));
        assert!(report.issue_codes.contains(&"kani_contract_solver_missing"));
        assert!(report.issue_codes.contains(&"kani_contract_receipt_missing"));
    }

    #[test]
    fn kani_contract_rejects_all_monomorphization_and_recursive_termination_overclaims() {
        let mut payload = valid_contract_payload(
            vec![KaniContractFacet::LoopInvariant.as_str()],
            KANI_RECEIPT_STATUS_PASSED,
            KANI_CONTRACT_LOOP_CLASSIFICATION,
        );
        payload["target"]["target_scope"] = serde_json::json!(KANI_TARGET_SCOPE_ALL_MONOMORPHIZATIONS);
        payload["termination_claimed"] = serde_json::json!(true);
        let report = report_for(contract_artifact(payload, VerificationRole::Property));

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"kani_contract_all_monomorphization_overclaim"));
        assert!(report.issue_codes.contains(&"kani_contract_recursive_termination_overclaim"));
    }

    #[test]
    fn kani_artifact_rejects_missing_required_non_claims() {
        let artifact = build_evidence_artifact(
            KANI_CONTRACT_ARTIFACT_KIND,
            "kani:contract:weak-boundary",
            None,
            VerificationRole::Property,
            valid_contract_payload(
                vec![KaniContractFacet::Postcondition.as_str()],
                KANI_RECEIPT_STATUS_PASSED,
                KANI_CONTRACT_PASSING_CLASSIFICATION,
            ),
            sample_non_claims(),
        )
        .expect("build weak-boundary Kani artifact");
        let report = report_for(artifact);

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"kani_non_claims_missing_required"));
    }

    #[test]
    fn kani_stub_accepts_verified_contract_linkage() {
        let report = report_for(stub_artifact(valid_verified_stub_payload(), VerificationRole::Property));

        assert!(report.valid);
        assert!(report.issue_codes.is_empty());
    }

    #[test]
    fn kani_stub_accepts_environmental_boundary_with_assumptions() {
        let payload = serde_json::json!({
            "stub_kind": KaniStubKind::EnvironmentalStub.as_str(),
            "assumptions": ["allocator returns aligned memory inside modeled range"],
        });
        let report = report_for(stub_artifact(payload, VerificationRole::Boundary));

        assert!(report.valid);
        assert!(report.issue_codes.is_empty());
    }

    #[test]
    fn kani_stub_rejects_missing_contract_link() {
        let payload = serde_json::json!({
            "stub_kind": KaniStubKind::VerifiedContractStub.as_str(),
            "original_function": "crate::checked_add",
            "source_scope": "src/lib.rs",
            "stub_signature": "fn checked_add(u32, u32) -> Option<u32>",
            "original_signature": "fn checked_add(u32, u32) -> Option<u32>",
        });
        let report = report_for(stub_artifact(payload, VerificationRole::Property));

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"kani_stub_contract_link_missing"));
    }

    #[test]
    fn kani_stub_rejects_stale_contract_hash_and_signature_mismatch() {
        let mut payload = valid_verified_stub_payload();
        payload["stub_signature"] = serde_json::json!("fn checked_add(u64, u64) -> Option<u64>");
        payload["linked_contract"]["artifact_hash"] = serde_json::json!("b3:stale-contract-artifact");
        let report = report_for(stub_artifact(payload, VerificationRole::Property));

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"kani_stub_signature_mismatch"));
        assert!(report.issue_codes.contains(&"kani_stub_stale_contract_hash"));
    }

    #[test]
    fn kani_stub_rejects_plain_stub_promotion_and_missing_assumptions() {
        let payload = serde_json::json!({
            "stub_kind": KANI_STUB_KIND_PLAIN,
        });
        let report = report_for(stub_artifact(payload, VerificationRole::Property));

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"kani_stub_plain_stub_promoted"));
        assert!(report.issue_codes.contains(&"kani_stub_assumptions_missing"));
    }

    #[test]
    fn explain_resolves_known_id() {
        let manifest = serde_json::to_string(&[sample_span()]).unwrap();
        let resolved = explain_span(&manifest, "eq:bs_001");
        let ExplainSpanResult::Found(span) = resolved else {
            panic!("expected found")
        };
        assert_eq!(span.source_path, "src/pricing.rs");
    }

    #[test]
    fn zero_width_span_is_valid() {
        let mut span = sample_span();
        span.end_byte = span.start_byte;
        span.end_line = span.start_line;
        span.end_column = span.start_column;
        assert!(validate_evidence_span(&span).is_empty());
    }

    #[test]
    fn impossible_span_coordinates_fail_closed() {
        let mut span = sample_span();
        span.source_id = " padded ".to_owned();
        span.source_path = "src/\0bad.rs".to_owned();
        span.start_byte = span.end_byte + 1;
        span.start_line = 0;
        span.label = Some(String::new());
        let issues = validate_evidence_span(&span);
        assert!(issues.iter().any(|issue| issue.code == "span_bytes_reversed"));
        assert!(issues.iter().any(|issue| issue.code == "span_coordinate_zero"));
        assert!(issues.iter().any(|issue| issue.code == "span_text_padded"));
        assert!(issues.iter().any(|issue| issue.code == "span_text_control_character"));
        assert!(issues.iter().any(|issue| issue.code == "span_text_empty"));
    }

    #[test]
    fn explain_distinguishes_not_found_from_invalid_manifest() {
        let manifest = serde_json::to_string(&[sample_span()]).unwrap();
        assert_eq!(explain_span(&manifest, "nonexistent"), ExplainSpanResult::NotFound);
        assert!(matches!(explain_span("{ not json", "eq:bs_001"), ExplainSpanResult::InvalidManifest(_)));
    }

    #[test]
    fn duplicate_manifest_id_never_selects_first_row() {
        let span = sample_span();
        let manifest = serde_json::to_string(&[span.clone(), span]).unwrap();
        let ExplainSpanResult::InvalidManifest(issues) = explain_span(&manifest, "eq:bs_001") else {
            panic!("duplicate manifest must be invalid")
        };
        assert!(issues.iter().any(|issue| issue.code == "manifest_duplicate_source_id"));
    }

    #[test]
    fn artifact_builder_and_checker_reject_invalid_span() {
        let mut span = sample_span();
        span.start_line = 0;
        let built = build_evidence_artifact(
            "receipt",
            "id",
            Some(span.clone()),
            VerificationRole::RecordedOnly,
            serde_json::json!({}),
            vec![REQUIRED_NON_CLAIM.to_owned()],
        );
        assert!(matches!(built, Err(EvidenceIrError::InvalidSourceSpan { .. })));

        let mut artifact = build_evidence_artifact(
            "receipt",
            "id",
            None,
            VerificationRole::RecordedOnly,
            serde_json::json!({}),
            vec![REQUIRED_NON_CLAIM.to_owned()],
        )
        .unwrap();
        artifact.source_span = Some(span);
        let report = check_evidence_artifact(&serde_json::to_string(&artifact).unwrap());
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"span_coordinate_zero"));
    }
}
