//! Pure adapter validators for external evidence rows that map into Valence
//! Evidence IR identity/linkage surfaces.
//!
//! These adapters validate parsed in-memory records only. Producers remain
//! responsible for measuring bytes, source analysis, replay/VMM semantics,
//! lifecycle policy, and release-readiness decisions.
//!
//! r[impl valence.evidence_ir.cairn_content_addressed_compat.manifest]
//! r[impl valence.evidence_ir.cairn_content_addressed_compat.validation]
//! r[impl valence.evidence_ir.chaoscontrol_export.contract]
//! r[impl valence.evidence_ir.chaoscontrol_export.validation]
//! r[impl valence.evidence_ir.state_machine_evidence.contract]
//! r[impl valence.evidence_ir.state_machine_evidence.validation]
//! r[impl valence.evidence_ir.fsm_default_profile.contract]
//! r[impl valence.evidence_ir.fsm_default_profile.validation]
//! r[impl valence.provenance.canonical_core_manifest]
//! r[impl valence.provenance.canonical_core_drift]

use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use crate::shared_identity::DigestRole;
use crate::shared_identity::MIXED_BLAKE3_PROFILE;
use crate::shared_identity::blake3_hex;
use crate::shared_identity::contains_overclaim;
use crate::shared_identity::parse_digest;

const FIELD_ARTIFACT_KIND: &str = "artifact_kind";
const FIELD_DIGEST: &str = "digest";
const FIELD_DISPOSITION: &str = "compatibility_disposition";
const FIELD_EVIDENCE_TIER: &str = "evidence_tier";
const FIELD_FAMILY: &str = "family";
const FIELD_HASH: &str = "hash";
const FIELD_HASH_MATERIAL: &str = "hash_material";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_POINTER_TARGET: &str = "pointer_target";
const FIELD_PROOF_LINK: &str = "proof_link";
const FIELD_PRODUCER_ROLE: &str = "producer_role";
const FIELD_PROFILE: &str = "profile";
const FIELD_PROFILE_VERSION: &str = "profile_version";
const FIELD_SCHEMA: &str = "schema";
const FIELD_SEMANTIC_CLAIMS: &str = "semantic_claims";
const FIELD_SOURCE_SPAN: &str = "source_span";
const FIELD_STATUS: &str = "status";
const FIELD_VERIFICATION_ROLE: &str = "verification_role";

pub const CAIRN_COMPAT_REQUIRED_NON_CLAIM: &str =
    "Cairn compatibility validation proves identity/linkage compatibility only";
pub const CHAOSCONTROL_EXPORT_REQUIRED_NON_CLAIM: &str =
    "ChaosControl export validation proves evidence identity/linkage only";
pub const STATE_MACHINE_REQUIRED_NON_CLAIM: &str =
    "state-machine evidence validation proves receipt shape, identity, linkage, and non-claims only";
pub const FSM_DEFAULT_REQUIRED_NON_CLAIM: &str = "FSM default-exhaustive evidence validation proves identity, linkage, roles, digest shape, coverage-record shape, and non-claims only";
pub const FSM_PROOF_LINK_REQUIRED_NON_CLAIM: &str =
    "FSM proof links remain separate evidence and do not promote the default-exhaustive receipt tier";
pub const CANONICAL_CORE_REQUIRED_NON_CLAIM: &str =
    "canonical core cutover proves ownership and identity compatibility only";

const CAIRN_COMPAT_SUPPORTED_CLAIM: &str = "Cairn compatibility validation proves identity/linkage compatibility only and does not prove Cairn lifecycle readiness, source-code correctness, verifier soundness, semantic equivalence, release eligibility, or whole-program safety";
const CHAOSCONTROL_SUPPORTED_CLAIM: &str = "ChaosControl export validation proves evidence identity/linkage only and does not prove universal determinism, hosted-product parity, replay completeness, or VMM correctness";
const STATE_MACHINE_SUPPORTED_CLAIM: &str = "state-machine evidence validation proves receipt shape, identity, linkage, and non-claims only, not Rust semantics, protocol completeness, liveness, verifier soundness, exhaustive Rust coverage, or release readiness";
const FSM_DEFAULT_SUPPORTED_CLAIM: &str = "FSM default-exhaustive evidence validation proves identity, linkage, roles, digest shape, coverage-record shape, and non-claims only, not Rust semantics, FSM semantics, behavioral correctness, protocol completeness, liveness, verifier soundness, exhaustive Rust coverage, release eligibility, or lifecycle readiness";
const CANONICAL_CORE_SUPPORTED_CLAIM: &str = "canonical core cutover proves ownership and identity compatibility only, not behavioral correctness, semantic equivalence, release eligibility, verifier soundness, or requirement satisfaction";

const COMPATIBLE_DISPOSITION: &str = "compatible";
const MIGRATION_EXCEPTION_DISPOSITION: &str = "migration-exception";
const CANONICAL_DISPOSITION: &str = "canonical";
const LEGACY_COMPATIBLE_DISPOSITION: &str = "legacy-compatible";
const RETIRED_DISPOSITION: &str = "retired";

const SUPPORTED_CAIRN_FAMILIES: &[&str] = &["function-object-v1", "name-pointer", "export-bundle"];

const SUPPORTED_CHAOSCONTROL_FAMILIES: &[&str] = &[
    "replay-verdict",
    "dogfood-receipt",
    "assertion-summary",
    "readiness-report",
];

const ALLOWED_CHAOSCONTROL_ROLES: &[&str] = &["recorded_only", "boundary"];
const ALLOWED_STATE_MACHINE_TIERS: &[&str] = &["source_shape", "bounded_source_shape"];
const ALLOWED_FSM_EVIDENCE_TIERS: &[&str] = &["source-shape", "bounded-source-shape"];
const ALLOWED_FSM_PRODUCER_ROLES: &[&str] = &[
    "octet-source-shape-producer",
    "kamacite-portable-fsm-producer",
    "valence-linkage-verifier",
];
const ALLOWED_FSM_ARTIFACT_ROLES: &[&str] = &[
    "octet-source-shape-receipt",
    "kamacite-fsm-spec",
    "kamacite-fsm-receipt",
    "state-universe",
    "event-universe",
    "variant-inventory",
    "transition-coverage",
    "denial-coverage",
    "parent-export",
    "formal-proof-receipt",
];
const FSM_PROFILE_NAME: &str = "fsm-default-exhaustive";
const FSM_PROFILE_VERSION: &str = "1";
const FSM_DOMAIN_ARTIFACT: &str = "artifact";
const FSM_DOMAIN_FSM_COVERAGE: &str = "fsm-coverage";
const FSM_DOMAIN_FSM_SPEC: &str = "fsm-spec";
const FSM_DOMAIN_FSM_UNIVERSE: &str = "fsm-universe";
const FSM_DOMAIN_PROJECTION: &str = "projection";
const FSM_DOMAIN_RECEIPT: &str = "receipt";
const FSM_OCTET_PRODUCER_ROLE: &str = "octet-source-shape-producer";
const FSM_KAMACITE_PRODUCER_ROLE: &str = "kamacite-portable-fsm-producer";
const FSM_VALENCE_VERIFIER_ROLE: &str = "valence-linkage-verifier";
const FSM_PROOF_TIER: &str = "formal-proof";
const FSM_OVERCLAIM_MARKERS: &[&str] = &[
    "proves protocol completeness",
    "proves liveness",
    "proves exhaustive rust coverage",
    "proves rust semantics",
    "proves fsm semantics",
];
const ALLOWED_STATUS_VALUES: &[&str] = &["pass", "fail", "skipped"];
const ALLOWED_CORE_DISPOSITIONS: &[&str] = &[
    CANONICAL_DISPOSITION,
    LEGACY_COMPATIBLE_DISPOSITION,
    MIGRATION_EXCEPTION_DISPOSITION,
    RETIRED_DISPOSITION,
];

/// Field-scoped deterministic adapter issue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct AdapterIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl AdapterIssue {
    #[must_use]
    pub const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

/// Generic adapter report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterReport {
    pub valid: bool,
    pub issues: Vec<AdapterIssue>,
    pub supported_claim: String,
    pub mapped_identity: Option<String>,
    pub migration_exception: bool,
}

/// Manifest row for Cairn content-addressed Rust evidence compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CairnContentAddressedManifestRow {
    pub family: String,
    pub schema: String,
    pub hash_field: String,
    pub hash_material_profile: String,
    pub valence_artifact_kind: String,
    pub verification_role: String,
    pub disposition: String,
    pub non_claims: Vec<String>,
}

/// Parsed Cairn content-addressed receipt candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CairnContentAddressedReceipt {
    pub family: String,
    pub schema: String,
    pub stable_id: String,
    pub declared_hash: String,
    pub hash_material: Vec<u8>,
    pub pointer_targets: Vec<String>,
    pub known_targets: Vec<String>,
    pub ambient_entropy_fields: Vec<String>,
    pub non_claims: Vec<String>,
}

/// ChaosControl export adapter row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChaosControlExportRow {
    pub family: String,
    pub schema: String,
    pub stable_id: String,
    pub canonical_hash: String,
    pub canonical_material: Vec<u8>,
    pub payload_digest: String,
    pub verification_role: String,
    pub non_claims: Vec<String>,
}

/// Octet-produced state-machine evidence receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMachineEvidenceReceipt {
    pub schema: String,
    pub transition_core_id: String,
    pub source_span_id: String,
    pub state_type_id: String,
    pub event_type_id: String,
    pub variant_inventory_digest: String,
    pub coverage_summary_digest: String,
    pub coverage_summary_material: Vec<u8>,
    pub status: String,
    pub diagnostics: Vec<String>,
    pub artifact_identity: String,
    pub parent_export_identity: String,
    pub evidence_tier: String,
    pub proof_links: Vec<String>,
    pub available_proof_receipts: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Identity reference used by the FSM default-exhaustive profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsmEvidenceIdentity {
    pub domain: String,
    pub digest: String,
}

/// Role-scoped artifact identity preserved in an FSM default-exhaustive record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsmArtifactIdentity {
    pub role: String,
    pub identity: FsmEvidenceIdentity,
}

/// Optional formal proof linkage that remains separate from the FSM receipt tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsmProofLink {
    pub verifier: String,
    pub proof_receipt_identity: FsmEvidenceIdentity,
    pub evidence_tier: String,
    pub non_claims: Vec<String>,
}

/// Valence-owned profile for default-exhaustive FSM evidence records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsmDefaultExhaustiveEvidence {
    pub profile: String,
    pub profile_version: String,
    pub machine_id: String,
    pub source_span_id: String,
    pub source_span_ids: Vec<String>,
    pub state_universe_identity: FsmEvidenceIdentity,
    pub event_universe_identity: FsmEvidenceIdentity,
    pub variant_inventory_identity: FsmEvidenceIdentity,
    pub transition_coverage_identity: FsmEvidenceIdentity,
    pub denial_coverage_identity: FsmEvidenceIdentity,
    pub octet_receipt_identity: FsmEvidenceIdentity,
    pub kamacite_spec_identity: Option<FsmEvidenceIdentity>,
    pub kamacite_receipt_identity: Option<FsmEvidenceIdentity>,
    pub artifact_identity: FsmEvidenceIdentity,
    pub parent_export_identity: FsmEvidenceIdentity,
    pub artifact_identities: Vec<FsmArtifactIdentity>,
    pub producer_roles: Vec<String>,
    pub evidence_tier: String,
    pub proof_links: Vec<FsmProofLink>,
    pub semantic_claims: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Canonical Valence core manifest row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalCoreManifestRow {
    pub module_or_schema: String,
    pub canonical_valence_path: String,
    pub legacy_octet_path: Option<String>,
    pub schema_identity: String,
    pub compatibility_disposition: String,
    pub fixture_refs: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Drift-check input for a canonical/legacy row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalCoreDriftRow {
    pub row_id: String,
    pub canonical_digest: String,
    pub observed_digest: String,
    pub compatibility_disposition: String,
    pub migration_exception_owner: Option<String>,
    pub non_claims: Vec<String>,
}

/// Validate Cairn content-addressed compatibility manifest rows.
#[must_use]
pub fn validate_cairn_content_addressed_manifest(rows: &[CairnContentAddressedManifestRow]) -> AdapterReport {
    let mut issues = Vec::new();
    for row in rows {
        if !SUPPORTED_CAIRN_FAMILIES.contains(&row.family.as_str()) {
            issues.push(AdapterIssue::new(FIELD_FAMILY, "unsupported_family"));
        }
        if row.schema.trim().is_empty() {
            issues.push(AdapterIssue::new(FIELD_SCHEMA, "schema_missing"));
        }
        if row.hash_field.trim().is_empty() || row.hash_material_profile.trim().is_empty() {
            issues.push(AdapterIssue::new(FIELD_HASH_MATERIAL, "hash_material_profile_missing"));
        }
        if row.valence_artifact_kind.trim().is_empty() {
            issues.push(AdapterIssue::new(FIELD_ARTIFACT_KIND, "artifact_kind_missing"));
        }
        if row.verification_role.trim().is_empty() {
            issues.push(AdapterIssue::new(FIELD_VERIFICATION_ROLE, "verification_role_missing"));
        }
        if row.disposition != COMPATIBLE_DISPOSITION && row.disposition != MIGRATION_EXCEPTION_DISPOSITION {
            issues.push(AdapterIssue::new(FIELD_DISPOSITION, "unsupported_disposition"));
        }
        validate_non_claims(&row.non_claims, CAIRN_COMPAT_REQUIRED_NON_CLAIM, &mut issues);
    }
    finish_report(issues, CAIRN_COMPAT_SUPPORTED_CLAIM, None, false)
}

/// Validate and map one Cairn content-addressed receipt into bounded Valence identity/linkage.
#[must_use]
pub fn validate_cairn_content_addressed_receipt(
    receipt: &CairnContentAddressedReceipt,
    manifest: &[CairnContentAddressedManifestRow],
) -> AdapterReport {
    let Some(row) = manifest.iter().find(|row| row.family == receipt.family) else {
        return finish_report(
            vec![AdapterIssue::new(FIELD_FAMILY, "unsupported_family")],
            CAIRN_COMPAT_SUPPORTED_CLAIM,
            None,
            false,
        );
    };

    let mut issues = Vec::new();
    if row.schema != receipt.schema {
        issues.push(AdapterIssue::new(FIELD_SCHEMA, "schema_drift"));
    }
    validate_hash(&receipt.declared_hash, &receipt.hash_material, DigestRole::ReceiptIdentity, &mut issues);
    validate_non_claims(&receipt.non_claims, CAIRN_COMPAT_REQUIRED_NON_CLAIM, &mut issues);
    if !receipt.ambient_entropy_fields.is_empty() {
        issues.push(AdapterIssue::new(FIELD_HASH_MATERIAL, "ambient_entropy_in_hash_material"));
    }
    let known_targets = receipt.known_targets.iter().collect::<BTreeSet<_>>();
    if receipt.pointer_targets.iter().any(|target| !known_targets.contains(target)) {
        issues.push(AdapterIssue::new(FIELD_POINTER_TARGET, "unknown_pointer_target"));
    }
    let migration_exception = row.disposition == MIGRATION_EXCEPTION_DISPOSITION;
    finish_report(issues, CAIRN_COMPAT_SUPPORTED_CLAIM, Some(receipt.stable_id.clone()), migration_exception)
}

/// Validate a ChaosControl export row without claiming VMM/replay semantics.
#[must_use]
pub fn validate_chaoscontrol_export(row: &ChaosControlExportRow) -> AdapterReport {
    let mut issues = Vec::new();
    if !SUPPORTED_CHAOSCONTROL_FAMILIES.contains(&row.family.as_str()) {
        issues.push(AdapterIssue::new(FIELD_FAMILY, "unsupported_family"));
    }
    if row.schema.trim().is_empty() {
        issues.push(AdapterIssue::new(FIELD_SCHEMA, "schema_missing"));
    }
    if row.stable_id.trim().is_empty() {
        issues.push(AdapterIssue::new("stable_id", "stable_id_missing"));
    }
    validate_hash(&row.canonical_hash, &row.canonical_material, DigestRole::ArtifactIdentity, &mut issues);
    validate_digest_syntax(&row.payload_digest, DigestRole::ArtifactIdentity, &mut issues);
    if !ALLOWED_CHAOSCONTROL_ROLES.contains(&row.verification_role.as_str()) {
        issues.push(AdapterIssue::new(FIELD_VERIFICATION_ROLE, "role_promotion"));
    }
    validate_non_claims(&row.non_claims, CHAOSCONTROL_EXPORT_REQUIRED_NON_CLAIM, &mut issues);
    finish_report(issues, CHAOSCONTROL_SUPPORTED_CLAIM, Some(row.stable_id.clone()), false)
}

/// Validate an Octet state-machine exhaustiveness receipt.
#[must_use]
pub fn validate_state_machine_evidence(receipt: &StateMachineEvidenceReceipt) -> AdapterReport {
    let mut issues = Vec::new();
    validate_required_text(&receipt.schema, FIELD_SCHEMA, "schema_missing", &mut issues);
    validate_required_text(&receipt.transition_core_id, "transition_core_id", "transition_core_missing", &mut issues);
    validate_required_text(&receipt.source_span_id, FIELD_SOURCE_SPAN, "source_span_missing", &mut issues);
    validate_required_text(&receipt.state_type_id, "state_type_id", "state_type_missing", &mut issues);
    validate_required_text(&receipt.event_type_id, "event_type_id", "event_type_missing", &mut issues);
    validate_digest_syntax(&receipt.variant_inventory_digest, DigestRole::ArtifactIdentity, &mut issues);
    validate_hash(
        &receipt.coverage_summary_digest,
        &receipt.coverage_summary_material,
        DigestRole::ReceiptIdentity,
        &mut issues,
    );
    validate_digest_syntax(&receipt.artifact_identity, DigestRole::ArtifactIdentity, &mut issues);
    validate_digest_syntax(&receipt.parent_export_identity, DigestRole::ArtifactIdentity, &mut issues);
    if !ALLOWED_STATUS_VALUES.contains(&receipt.status.as_str()) {
        issues.push(AdapterIssue::new(FIELD_STATUS, "unknown_status"));
    }
    if receipt.status == "pass" && !receipt.diagnostics.is_empty() {
        issues.push(AdapterIssue::new(FIELD_STATUS, "passing_receipt_has_diagnostics"));
    }
    if !ALLOWED_STATE_MACHINE_TIERS.contains(&receipt.evidence_tier.as_str()) {
        issues.push(AdapterIssue::new(FIELD_EVIDENCE_TIER, "unknown_or_promoted_tier"));
    }
    let available = receipt.available_proof_receipts.iter().collect::<BTreeSet<_>>();
    if receipt.proof_links.iter().any(|link| !available.contains(link)) {
        issues.push(AdapterIssue::new(FIELD_PROOF_LINK, "missing_linked_proof"));
    }
    validate_non_claims(&receipt.non_claims, STATE_MACHINE_REQUIRED_NON_CLAIM, &mut issues);

    finish_report(issues, STATE_MACHINE_SUPPORTED_CLAIM, Some(receipt.transition_core_id.clone()), false)
}

/// Validate an FSM default-exhaustive evidence record without claiming FSM semantics.
#[must_use]
pub fn validate_fsm_default_exhaustive_evidence(record: &FsmDefaultExhaustiveEvidence) -> AdapterReport {
    let mut issues = Vec::new();
    if record.profile != FSM_PROFILE_NAME {
        issues.push(AdapterIssue::new(FIELD_PROFILE, "unsupported_profile"));
    }
    if record.profile_version != FSM_PROFILE_VERSION {
        issues.push(AdapterIssue::new(FIELD_PROFILE_VERSION, "unsupported_profile_version"));
    }
    validate_required_text(&record.machine_id, "machine_id", "machine_id_missing", &mut issues);
    validate_required_text(&record.source_span_id, FIELD_SOURCE_SPAN, "source_span_missing", &mut issues);
    if record.source_span_ids.is_empty() {
        issues.push(AdapterIssue::new(FIELD_SOURCE_SPAN, "source_span_set_missing"));
    }
    validate_fsm_identity(
        &record.state_universe_identity,
        FSM_DOMAIN_FSM_UNIVERSE,
        "state_universe_identity",
        "state_universe_missing",
        &mut issues,
    );
    validate_fsm_identity(
        &record.event_universe_identity,
        FSM_DOMAIN_FSM_UNIVERSE,
        "event_universe_identity",
        "event_universe_missing",
        &mut issues,
    );
    validate_fsm_identity(
        &record.variant_inventory_identity,
        FSM_DOMAIN_ARTIFACT,
        "variant_inventory_identity",
        "variant_inventory_missing",
        &mut issues,
    );
    validate_fsm_identity(
        &record.transition_coverage_identity,
        FSM_DOMAIN_FSM_COVERAGE,
        "transition_coverage_identity",
        "transition_coverage_missing",
        &mut issues,
    );
    validate_fsm_identity(
        &record.denial_coverage_identity,
        FSM_DOMAIN_FSM_COVERAGE,
        "denial_coverage_identity",
        "denial_coverage_missing",
        &mut issues,
    );
    validate_fsm_identity(
        &record.octet_receipt_identity,
        FSM_DOMAIN_RECEIPT,
        "octet_receipt_identity",
        "octet_receipt_missing",
        &mut issues,
    );
    validate_optional_fsm_identity(
        &record.kamacite_spec_identity,
        FSM_DOMAIN_FSM_SPEC,
        "kamacite_spec_identity",
        &mut issues,
    );
    validate_optional_fsm_identity(
        &record.kamacite_receipt_identity,
        FSM_DOMAIN_RECEIPT,
        "kamacite_receipt_identity",
        &mut issues,
    );
    validate_fsm_identity(
        &record.artifact_identity,
        FSM_DOMAIN_ARTIFACT,
        "artifact_identity",
        "artifact_identity_missing",
        &mut issues,
    );
    validate_fsm_identity(
        &record.parent_export_identity,
        FSM_DOMAIN_PROJECTION,
        "parent_export_identity",
        "parent_export_missing",
        &mut issues,
    );
    validate_fsm_roles(record, &mut issues);
    validate_fsm_artifact_identities(record, &mut issues);
    validate_fsm_proof_links(record, &mut issues);
    validate_non_claims(&record.non_claims, FSM_DEFAULT_REQUIRED_NON_CLAIM, &mut issues);
    if record.non_claims.iter().any(|claim| contains_fsm_overclaim(claim)) {
        issues.push(AdapterIssue::new(FIELD_NON_CLAIMS, "overclaim_detected"));
    }
    if record.semantic_claims.iter().any(|claim| contains_fsm_overclaim(claim)) {
        issues.push(AdapterIssue::new(FIELD_SEMANTIC_CLAIMS, "overclaim_detected"));
    }

    finish_report(issues, FSM_DEFAULT_SUPPORTED_CLAIM, Some(record.machine_id.clone()), false)
}

fn contains_fsm_overclaim(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    contains_overclaim(text) || FSM_OVERCLAIM_MARKERS.iter().any(|marker| normalized.contains(marker))
}

fn validate_optional_fsm_identity(
    identity: &Option<FsmEvidenceIdentity>,
    expected_domain: &'static str,
    field: &'static str,
    issues: &mut Vec<AdapterIssue>,
) {
    if let Some(identity) = identity {
        validate_fsm_identity(identity, expected_domain, field, "identity_missing", issues);
    }
}

fn validate_fsm_identity(
    identity: &FsmEvidenceIdentity,
    expected_domain: &'static str,
    field: &'static str,
    missing_code: &'static str,
    issues: &mut Vec<AdapterIssue>,
) {
    if identity.domain.trim().is_empty() || identity.digest.trim().is_empty() {
        issues.push(AdapterIssue::new(field, missing_code));
        return;
    }
    if identity.domain != expected_domain {
        issues.push(AdapterIssue::new(field, "wrong_identity_domain"));
    }
    validate_digest_syntax(&identity.digest, digest_role_for_fsm_domain(expected_domain), issues);
}

fn digest_role_for_fsm_domain(domain: &str) -> DigestRole {
    match domain {
        FSM_DOMAIN_RECEIPT => DigestRole::ReceiptIdentity,
        _ => DigestRole::ArtifactIdentity,
    }
}

fn validate_fsm_roles(record: &FsmDefaultExhaustiveEvidence, issues: &mut Vec<AdapterIssue>) {
    if !ALLOWED_FSM_EVIDENCE_TIERS.contains(&record.evidence_tier.as_str()) {
        issues.push(AdapterIssue::new(FIELD_EVIDENCE_TIER, "unknown_or_promoted_tier"));
    }
    let roles = record.producer_roles.iter().collect::<BTreeSet<_>>();
    if !roles.contains(&FSM_OCTET_PRODUCER_ROLE.to_owned()) {
        issues.push(AdapterIssue::new(FIELD_PRODUCER_ROLE, "octet_role_missing"));
    }
    if !roles.contains(&FSM_VALENCE_VERIFIER_ROLE.to_owned()) {
        issues.push(AdapterIssue::new(FIELD_PRODUCER_ROLE, "valence_role_missing"));
    }
    if (record.kamacite_spec_identity.is_some() || record.kamacite_receipt_identity.is_some())
        && !roles.contains(&FSM_KAMACITE_PRODUCER_ROLE.to_owned())
    {
        issues.push(AdapterIssue::new(FIELD_PRODUCER_ROLE, "kamacite_role_missing"));
    }
    for role in &record.producer_roles {
        if !ALLOWED_FSM_PRODUCER_ROLES.contains(&role.as_str()) {
            issues.push(AdapterIssue::new(FIELD_PRODUCER_ROLE, "unknown_role"));
        }
    }
}

fn validate_fsm_artifact_identities(record: &FsmDefaultExhaustiveEvidence, issues: &mut Vec<AdapterIssue>) {
    if record.artifact_identities.is_empty() {
        issues.push(AdapterIssue::new("artifact_identities", "artifact_identities_missing"));
        return;
    }
    let mut roles = BTreeSet::new();
    for artifact in &record.artifact_identities {
        if !ALLOWED_FSM_ARTIFACT_ROLES.contains(&artifact.role.as_str()) {
            issues.push(AdapterIssue::new("artifact_identities", "unknown_role"));
        }
        if !roles.insert(artifact.role.clone()) {
            issues.push(AdapterIssue::new("artifact_identities", "duplicate_role"));
        }
        validate_fsm_identity(
            &artifact.identity,
            expected_domain_for_fsm_artifact_role(&artifact.role),
            "artifact_identities",
            "identity_missing",
            issues,
        );
    }
    for required in [
        "octet-source-shape-receipt",
        "state-universe",
        "event-universe",
        "variant-inventory",
        "transition-coverage",
        "denial-coverage",
        "parent-export",
    ] {
        if !roles.contains(required) {
            issues.push(AdapterIssue::new("artifact_identities", "required_role_missing"));
        }
    }
}

fn expected_domain_for_fsm_artifact_role(role: &str) -> &'static str {
    match role {
        "octet-source-shape-receipt" | "kamacite-fsm-receipt" | "formal-proof-receipt" => FSM_DOMAIN_RECEIPT,
        "kamacite-fsm-spec" => FSM_DOMAIN_FSM_SPEC,
        "state-universe" | "event-universe" => FSM_DOMAIN_FSM_UNIVERSE,
        "transition-coverage" | "denial-coverage" => FSM_DOMAIN_FSM_COVERAGE,
        "parent-export" => FSM_DOMAIN_PROJECTION,
        _ => FSM_DOMAIN_ARTIFACT,
    }
}

fn validate_fsm_proof_links(record: &FsmDefaultExhaustiveEvidence, issues: &mut Vec<AdapterIssue>) {
    for proof_link in &record.proof_links {
        validate_required_text(&proof_link.verifier, FIELD_PROOF_LINK, "verifier_missing", issues);
        validate_fsm_identity(
            &proof_link.proof_receipt_identity,
            FSM_DOMAIN_RECEIPT,
            FIELD_PROOF_LINK,
            "proof_receipt_missing",
            issues,
        );
        if proof_link.evidence_tier != FSM_PROOF_TIER {
            issues.push(AdapterIssue::new(FIELD_PROOF_LINK, "proof_tier_not_separate"));
        }
        validate_non_claims(&proof_link.non_claims, FSM_PROOF_LINK_REQUIRED_NON_CLAIM, issues);
    }
}

/// Validate canonical Valence core manifest rows.
#[must_use]
pub fn validate_canonical_core_manifest(rows: &[CanonicalCoreManifestRow]) -> AdapterReport {
    let mut issues = Vec::new();
    for row in rows {
        validate_required_text(&row.module_or_schema, "module_or_schema", "module_missing", &mut issues);
        validate_required_text(
            &row.canonical_valence_path,
            "canonical_valence_path",
            "canonical_path_missing",
            &mut issues,
        );
        validate_digest_syntax(&row.schema_identity, DigestRole::ArtifactIdentity, &mut issues);
        validate_core_disposition(&row.compatibility_disposition, &mut issues);
        if row.fixture_refs.is_empty() {
            issues.push(AdapterIssue::new("fixture_refs", "fixture_refs_missing"));
        }
        validate_non_claims(&row.non_claims, CANONICAL_CORE_REQUIRED_NON_CLAIM, &mut issues);
    }
    finish_report(issues, CANONICAL_CORE_SUPPORTED_CLAIM, None, false)
}

/// Validate canonical-vs-observed drift for legacy or consumer-visible rows.
#[must_use]
pub fn validate_canonical_core_drift(rows: &[CanonicalCoreDriftRow]) -> AdapterReport {
    let mut issues = Vec::new();
    for row in rows {
        validate_digest_syntax(&row.canonical_digest, DigestRole::ArtifactIdentity, &mut issues);
        validate_digest_syntax(&row.observed_digest, DigestRole::ArtifactIdentity, &mut issues);
        validate_core_disposition(&row.compatibility_disposition, &mut issues);
        validate_non_claims(&row.non_claims, CANONICAL_CORE_REQUIRED_NON_CLAIM, &mut issues);
        if row.canonical_digest != row.observed_digest {
            let has_exception = row.compatibility_disposition == MIGRATION_EXCEPTION_DISPOSITION
                && row.migration_exception_owner.as_deref().is_some_and(|owner| !owner.trim().is_empty());
            if !has_exception {
                issues.push(AdapterIssue::new(FIELD_DIGEST, "stale_copy"));
            }
        }
    }
    finish_report(issues, CANONICAL_CORE_SUPPORTED_CLAIM, None, false)
}

fn validate_core_disposition(disposition: &str, issues: &mut Vec<AdapterIssue>) {
    if !ALLOWED_CORE_DISPOSITIONS.contains(&disposition) {
        issues.push(AdapterIssue::new(FIELD_DISPOSITION, "unsupported_disposition"));
    }
}

fn validate_required_text(value: &str, field: &'static str, code: &'static str, issues: &mut Vec<AdapterIssue>) {
    if value.trim().is_empty() {
        issues.push(AdapterIssue::new(field, code));
    }
}

fn validate_hash(declared: &str, material: &[u8], role: DigestRole, issues: &mut Vec<AdapterIssue>) {
    validate_digest_syntax(declared, role, issues);
    let expected = blake3_hex(material);
    if parse_digest(declared, role, MIXED_BLAKE3_PROFILE, FIELD_HASH)
        .is_ok_and(|digest| digest.normalized_hex != expected)
    {
        issues.push(AdapterIssue::new(FIELD_HASH, "stale_hash"));
    }
}

fn validate_digest_syntax(declared: &str, role: DigestRole, issues: &mut Vec<AdapterIssue>) {
    if parse_digest(declared, role, MIXED_BLAKE3_PROFILE, FIELD_HASH).is_err() {
        issues.push(AdapterIssue::new(FIELD_HASH, "malformed_blake3"));
    }
}

fn validate_non_claims(non_claims: &[String], required: &str, issues: &mut Vec<AdapterIssue>) {
    if non_claims.is_empty() {
        issues.push(AdapterIssue::new(FIELD_NON_CLAIMS, "non_claims_missing"));
        return;
    }
    if !non_claims.iter().any(|claim| claim.contains(required)) {
        issues.push(AdapterIssue::new(FIELD_NON_CLAIMS, "non_claims_weakened"));
    }
    if non_claims.iter().any(|claim| contains_overclaim(claim)) {
        issues.push(AdapterIssue::new(FIELD_NON_CLAIMS, "overclaim_detected"));
    }
}

fn finish_report(
    mut issues: Vec<AdapterIssue>,
    supported_claim: &str,
    mapped_identity: Option<String>,
    migration_exception: bool,
) -> AdapterReport {
    issues.sort();
    issues.dedup();
    AdapterReport {
        valid: issues.is_empty(),
        issues,
        supported_claim: supported_claim.to_owned(),
        mapped_identity,
        migration_exception,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STABLE_ID: &str = "fixture.identity";
    const DIGEST: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    fn hash_for(bytes: &[u8]) -> String {
        blake3_hex(bytes)
    }

    fn cairn_non_claims() -> Vec<String> {
        vec![CAIRN_COMPAT_REQUIRED_NON_CLAIM.to_owned()]
    }

    fn chaos_non_claims() -> Vec<String> {
        vec![CHAOSCONTROL_EXPORT_REQUIRED_NON_CLAIM.to_owned()]
    }

    fn state_machine_non_claims() -> Vec<String> {
        vec![STATE_MACHINE_REQUIRED_NON_CLAIM.to_owned()]
    }

    fn canonical_core_non_claims() -> Vec<String> {
        vec![CANONICAL_CORE_REQUIRED_NON_CLAIM.to_owned()]
    }

    fn cairn_manifest() -> CairnContentAddressedManifestRow {
        CairnContentAddressedManifestRow {
            family: "function-object-v1".to_owned(),
            schema: "cairn.content-addressed.function-object.v1".to_owned(),
            hash_field: "object_hash".to_owned(),
            hash_material_profile: "function-object-v1".to_owned(),
            valence_artifact_kind: "function_object".to_owned(),
            verification_role: "recorded_only".to_owned(),
            disposition: COMPATIBLE_DISPOSITION.to_owned(),
            non_claims: cairn_non_claims(),
        }
    }

    #[test]
    fn cairn_manifest_and_receipt_positive_fixture_pass() {
        // r[verify valence.evidence_ir.cairn_content_addressed_compat.fixtures.positive]
        let material = b"function-object".to_vec();
        let receipt = CairnContentAddressedReceipt {
            family: "function-object-v1".to_owned(),
            schema: "cairn.content-addressed.function-object.v1".to_owned(),
            stable_id: STABLE_ID.to_owned(),
            declared_hash: hash_for(&material),
            hash_material: material,
            pointer_targets: vec!["target-a".to_owned()],
            known_targets: vec!["target-a".to_owned()],
            ambient_entropy_fields: Vec::new(),
            non_claims: cairn_non_claims(),
        };

        let manifest_report = validate_cairn_content_addressed_manifest(&[cairn_manifest()]);
        let receipt_report = validate_cairn_content_addressed_receipt(&receipt, &[cairn_manifest()]);

        assert!(manifest_report.valid);
        assert!(receipt_report.valid);
        assert_eq!(receipt_report.mapped_identity.as_deref(), Some(STABLE_ID));
    }

    #[test]
    fn cairn_receipt_negative_fixture_fails_closed() {
        // r[verify valence.evidence_ir.cairn_content_addressed_compat.fixtures.negative]
        let receipt = CairnContentAddressedReceipt {
            family: "function-object-v1".to_owned(),
            schema: "drift".to_owned(),
            stable_id: STABLE_ID.to_owned(),
            declared_hash: DIGEST.to_owned(),
            hash_material: b"different".to_vec(),
            pointer_targets: vec!["missing".to_owned()],
            known_targets: vec!["present".to_owned()],
            ambient_entropy_fields: vec!["hostname".to_owned()],
            non_claims: vec!["proves semantic equivalence".to_owned()],
        };

        let report = validate_cairn_content_addressed_receipt(&receipt, &[cairn_manifest()]);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "schema_drift"));
        assert!(report.issues.iter().any(|issue| issue.code == "stale_hash"));
        assert!(report.issues.iter().any(|issue| issue.code == "ambient_entropy_in_hash_material"));
        assert!(report.issues.iter().any(|issue| issue.code == "unknown_pointer_target"));
        assert!(report.issues.iter().any(|issue| issue.code == "overclaim_detected"));
    }

    #[test]
    fn chaoscontrol_export_positive_and_negative_rows() {
        // r[verify valence.evidence_ir.chaoscontrol_export.fixtures.positive]
        // r[verify valence.evidence_ir.chaoscontrol_export.fixtures.negative]
        let material = b"replay-verdict".to_vec();
        let valid = ChaosControlExportRow {
            family: "replay-verdict".to_owned(),
            schema: "chaoscontrol.replay-verdict.v1".to_owned(),
            stable_id: STABLE_ID.to_owned(),
            canonical_hash: hash_for(&material),
            canonical_material: material,
            payload_digest: DIGEST.to_owned(),
            verification_role: "recorded_only".to_owned(),
            non_claims: chaos_non_claims(),
        };
        let mut invalid = valid.clone();
        invalid.family = "vmm-proof".to_owned();
        invalid.verification_role = "property".to_owned();
        invalid.canonical_hash = DIGEST.to_owned();
        invalid.non_claims.clear();

        let pass = validate_chaoscontrol_export(&valid);
        let fail = validate_chaoscontrol_export(&invalid);

        assert!(pass.valid);
        assert!(!fail.valid);
        assert!(fail.issues.iter().any(|issue| issue.code == "unsupported_family"));
        assert!(fail.issues.iter().any(|issue| issue.code == "role_promotion"));
        assert!(fail.issues.iter().any(|issue| issue.code == "non_claims_missing"));
    }

    fn state_machine_receipt() -> StateMachineEvidenceReceipt {
        let coverage = b"state-event coverage".to_vec();
        StateMachineEvidenceReceipt {
            schema: "octet.state-machine-exhaustiveness.v1".to_owned(),
            transition_core_id: "transition.apply".to_owned(),
            source_span_id: "span:transition.apply".to_owned(),
            state_type_id: "State".to_owned(),
            event_type_id: "Event".to_owned(),
            variant_inventory_digest: DIGEST.to_owned(),
            coverage_summary_digest: hash_for(&coverage),
            coverage_summary_material: coverage,
            status: "pass".to_owned(),
            diagnostics: Vec::new(),
            artifact_identity: DIGEST.to_owned(),
            parent_export_identity: DIGEST.to_owned(),
            evidence_tier: "source_shape".to_owned(),
            proof_links: vec!["proof:transition-preserves".to_owned()],
            available_proof_receipts: vec!["proof:transition-preserves".to_owned()],
            non_claims: state_machine_non_claims(),
        }
    }

    #[test]
    fn state_machine_receipt_positive_fixture_passes() {
        // r[verify valence.evidence_ir.state_machine_evidence.fixtures.positive]
        let report = validate_state_machine_evidence(&state_machine_receipt());

        assert!(report.valid);
        assert!(report.supported_claim.contains("not Rust semantics"));
    }

    #[test]
    fn state_machine_receipt_negative_fixture_fails_closed() {
        // r[verify valence.evidence_ir.state_machine_evidence.fixtures.negative]
        let mut receipt = state_machine_receipt();
        receipt.state_type_id.clear();
        receipt.coverage_summary_digest = DIGEST.to_owned();
        receipt.evidence_tier = "formal_proof".to_owned();
        receipt.proof_links = vec!["missing".to_owned()];
        receipt.non_claims = vec!["proves whole-program safety".to_owned()];

        let report = validate_state_machine_evidence(&receipt);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "state_type_missing"));
        assert!(report.issues.iter().any(|issue| issue.code == "stale_hash"));
        assert!(report.issues.iter().any(|issue| issue.code == "unknown_or_promoted_tier"));
        assert!(report.issues.iter().any(|issue| issue.code == "missing_linked_proof"));
        assert!(report.issues.iter().any(|issue| issue.code == "overclaim_detected"));
    }

    fn fsm_identity(domain: &str, material: &[u8]) -> FsmEvidenceIdentity {
        FsmEvidenceIdentity {
            domain: domain.to_owned(),
            digest: hash_for(material),
        }
    }

    fn fsm_artifact(role: &str, identity: FsmEvidenceIdentity) -> FsmArtifactIdentity {
        FsmArtifactIdentity {
            role: role.to_owned(),
            identity,
        }
    }

    fn fsm_default_evidence() -> FsmDefaultExhaustiveEvidence {
        let state_universe = fsm_identity(FSM_DOMAIN_FSM_UNIVERSE, b"state-universe");
        let event_universe = fsm_identity(FSM_DOMAIN_FSM_UNIVERSE, b"event-universe");
        let variant_inventory = fsm_identity(FSM_DOMAIN_ARTIFACT, b"variant-inventory");
        let transition_coverage = fsm_identity(FSM_DOMAIN_FSM_COVERAGE, b"transition-coverage");
        let denial_coverage = fsm_identity(FSM_DOMAIN_FSM_COVERAGE, b"denial-coverage");
        let octet_receipt = fsm_identity(FSM_DOMAIN_RECEIPT, b"octet-receipt");
        let parent_export = fsm_identity(FSM_DOMAIN_PROJECTION, b"parent-export");
        FsmDefaultExhaustiveEvidence {
            profile: FSM_PROFILE_NAME.to_owned(),
            profile_version: FSM_PROFILE_VERSION.to_owned(),
            machine_id: "fixture.lifecycle".to_owned(),
            source_span_id: "span:transition.apply".to_owned(),
            source_span_ids: vec!["span:transition.apply".to_owned()],
            state_universe_identity: state_universe.clone(),
            event_universe_identity: event_universe.clone(),
            variant_inventory_identity: variant_inventory.clone(),
            transition_coverage_identity: transition_coverage.clone(),
            denial_coverage_identity: denial_coverage.clone(),
            octet_receipt_identity: octet_receipt.clone(),
            kamacite_spec_identity: None,
            kamacite_receipt_identity: None,
            artifact_identity: fsm_identity(FSM_DOMAIN_ARTIFACT, b"source-artifact"),
            parent_export_identity: parent_export.clone(),
            artifact_identities: vec![
                fsm_artifact("octet-source-shape-receipt", octet_receipt),
                fsm_artifact("state-universe", state_universe),
                fsm_artifact("event-universe", event_universe),
                fsm_artifact("variant-inventory", variant_inventory),
                fsm_artifact("transition-coverage", transition_coverage),
                fsm_artifact("denial-coverage", denial_coverage),
                fsm_artifact("parent-export", parent_export),
            ],
            producer_roles: vec![FSM_OCTET_PRODUCER_ROLE.to_owned(), FSM_VALENCE_VERIFIER_ROLE.to_owned()],
            evidence_tier: "source-shape".to_owned(),
            proof_links: Vec::new(),
            semantic_claims: Vec::new(),
            non_claims: vec![FSM_DEFAULT_REQUIRED_NON_CLAIM.to_owned()],
        }
    }

    #[test]
    fn fsm_default_exhaustive_octet_only_fixture_passes() {
        // r[verify valence.evidence_ir.fsm_default_profile.fixtures.positive.octet_only]
        let report = validate_fsm_default_exhaustive_evidence(&fsm_default_evidence());

        assert!(report.valid);
        assert!(report.supported_claim.contains("not Rust semantics"));
        assert_eq!(report.mapped_identity.as_deref(), Some("fixture.lifecycle"));
    }

    #[test]
    fn fsm_default_exhaustive_kamacite_and_proof_link_preserve_boundaries() {
        // r[verify valence.evidence_ir.fsm_default_profile.fixtures.positive.kamacite_link]
        // r[verify valence.evidence_ir.fsm_default_profile.proof_links.no_promotion]
        let mut record = fsm_default_evidence();
        let kamacite_spec = fsm_identity(FSM_DOMAIN_FSM_SPEC, b"kamacite-spec");
        let kamacite_receipt = fsm_identity(FSM_DOMAIN_RECEIPT, b"kamacite-receipt");
        let proof_receipt = fsm_identity(FSM_DOMAIN_RECEIPT, b"trellis-proof");
        record.kamacite_spec_identity = Some(kamacite_spec.clone());
        record.kamacite_receipt_identity = Some(kamacite_receipt.clone());
        record.producer_roles.push(FSM_KAMACITE_PRODUCER_ROLE.to_owned());
        record.artifact_identities.push(fsm_artifact("kamacite-fsm-spec", kamacite_spec));
        record.artifact_identities.push(fsm_artifact("kamacite-fsm-receipt", kamacite_receipt));
        record.artifact_identities.push(fsm_artifact("formal-proof-receipt", proof_receipt.clone()));
        record.proof_links.push(FsmProofLink {
            verifier: "trellis".to_owned(),
            proof_receipt_identity: proof_receipt,
            evidence_tier: FSM_PROOF_TIER.to_owned(),
            non_claims: vec![FSM_PROOF_LINK_REQUIRED_NON_CLAIM.to_owned()],
        });

        let report = validate_fsm_default_exhaustive_evidence(&record);

        assert!(report.valid);
        assert_eq!(record.evidence_tier, "source-shape");
    }

    #[test]
    fn fsm_default_exhaustive_negative_fixture_fails_closed() {
        // r[verify valence.evidence_ir.fsm_default_profile.fixtures.negative]
        let mut record = fsm_default_evidence();
        record.profile_version = "unsupported".to_owned();
        record.denial_coverage_identity.digest.clear();
        record.state_universe_identity.digest = "not-blake3".to_owned();
        record.producer_roles.push("runtime-correctness-producer".to_owned());
        record.evidence_tier = FSM_PROOF_TIER.to_owned();
        record.semantic_claims = vec!["proves protocol completeness and liveness".to_owned()];
        record.non_claims.clear();
        record
            .artifact_identities
            .push(fsm_artifact("unknown-role", fsm_identity(FSM_DOMAIN_RECEIPT, b"wrong-domain")));
        record.proof_links.push(FsmProofLink {
            verifier: String::new(),
            proof_receipt_identity: fsm_identity(FSM_DOMAIN_ARTIFACT, b"wrong-proof-domain"),
            evidence_tier: "source-shape".to_owned(),
            non_claims: vec!["proof promotes FSM receipt".to_owned()],
        });

        let report = validate_fsm_default_exhaustive_evidence(&record);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "unsupported_profile_version"));
        assert!(report.issues.iter().any(|issue| issue.code == "denial_coverage_missing"));
        assert!(report.issues.iter().any(|issue| issue.code == "malformed_blake3"));
        assert!(report.issues.iter().any(|issue| issue.code == "unknown_role"));
        assert!(report.issues.iter().any(|issue| issue.code == "unknown_or_promoted_tier"));
        assert!(report.issues.iter().any(|issue| issue.code == "non_claims_missing"));
        assert!(report.issues.iter().any(|issue| issue.code == "overclaim_detected"));
        assert!(report.issues.iter().any(|issue| issue.code == "proof_tier_not_separate"));
    }

    #[test]
    fn canonical_core_manifest_and_drift_checks_cover_pass_and_fail() {
        // r[verify valence.provenance.canonical_core_manifest.positive]
        // r[verify valence.provenance.canonical_core_manifest.negative]
        // r[verify valence.provenance.canonical_core_drift.match]
        // r[verify valence.provenance.canonical_core_drift.stale]
        let manifest = CanonicalCoreManifestRow {
            module_or_schema: "evidence_ir".to_owned(),
            canonical_valence_path: "crates/valence-core/src/evidence_ir.rs".to_owned(),
            legacy_octet_path: Some("octet/crates/valence-core/src/evidence_ir.rs".to_owned()),
            schema_identity: DIGEST.to_owned(),
            compatibility_disposition: LEGACY_COMPATIBLE_DISPOSITION.to_owned(),
            fixture_refs: vec!["fixtures/evidence_ir/pass.json".to_owned()],
            non_claims: canonical_core_non_claims(),
        };
        let stale = CanonicalCoreDriftRow {
            row_id: "evidence_ir".to_owned(),
            canonical_digest: DIGEST.to_owned(),
            observed_digest: hash_for(b"drift"),
            compatibility_disposition: LEGACY_COMPATIBLE_DISPOSITION.to_owned(),
            migration_exception_owner: None,
            non_claims: canonical_core_non_claims(),
        };
        let matching = CanonicalCoreDriftRow {
            observed_digest: DIGEST.to_owned(),
            ..stale.clone()
        };

        let manifest_report = validate_canonical_core_manifest(&[manifest]);
        let matching_report = validate_canonical_core_drift(&[matching]);
        let stale_report = validate_canonical_core_drift(&[stale]);

        assert!(manifest_report.valid);
        assert!(matching_report.valid);
        assert!(!stale_report.valid);
        assert!(stale_report.issues.iter().any(|issue| issue.code == "stale_copy"));
    }
}
