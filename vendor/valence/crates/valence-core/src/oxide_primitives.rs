//! Oxide-reference primitive intake for Valence Evidence IR.
//!
//! References shape fixture-backed primitive design only. This module does not
//! treat external repositories as implementation authority or proof of Evidence
//! IR correctness, release readiness, or attestation truth.
//!
//! r[impl valence.evidence_ir.oxide_primitives.reference_inventory]
//! r[impl valence.evidence_ir.oxide_primitives.typed_ids]
//! r[impl valence.evidence_ir.oxide_primitives.identity_maps]
//! r[impl valence.evidence_ir.oxide_primitives.semantic_diffs]
//! r[impl valence.evidence_ir.oxide_primitives.attestation_vocabulary]

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

use crate::shared_identity::DigestRole;
use crate::shared_identity::MIXED_BLAKE3_PROFILE;
use crate::shared_identity::contains_overclaim;
use crate::shared_identity::parse_digest;

const FIELD_BASELINE: &str = "baseline";
const FIELD_BOUNDARY: &str = "boundary";
const FIELD_DIGEST: &str = "digest";
const FIELD_IDENTIFIER: &str = "identifier";
const FIELD_ISSUER: &str = "issuer";
const FIELD_KEY: &str = "key";
const FIELD_LICENSE: &str = "license";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_REFERENCE: &str = "reference";
const FIELD_ROLE: &str = "role";
const FIELD_SCOPE: &str = "scope";
const FIELD_SUBJECT: &str = "subject";

pub const OXIDE_PRIMITIVES_REQUIRED_NON_CLAIM: &str = "Oxide primitive intake is reference and review evidence only";
const OXIDE_PRIMITIVES_SUPPORTED_CLAIM: &str = "Oxide primitive validation is reference and review evidence only; it does not prove Evidence IR correctness, release readiness, attestation truth, behavioral correctness, semantic equivalence, or verifier soundness";

const SUPPORTED_LICENSE_POSTURES: &[&str] = &["MIT OR Apache-2.0", "Apache-2.0", "MPL-2.0", "reference-only"];
const SUPPORTED_IDENTIFIER_ROLES: &[&str] = &["artifact", "receipt", "external_uuid", "attestation"];
const UUID_SEPARATOR: char = '-';
const UUID_TEXT_LENGTH: usize = 36;

/// Reference-only inventory row for an Oxide repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxideReferenceInventoryRow {
    pub repository: String,
    pub license_posture: String,
    pub msrv_posture: String,
    pub no_std_posture: String,
    pub intended_adaptation: String,
    pub non_adoption_boundaries: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Typed evidence identifier that keeps BLAKE3 canonical identity separate from external IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedEvidenceIdentifier {
    pub role: String,
    pub blake3_identity: String,
    pub external_uuid: Option<String>,
    pub non_claims: Vec<String>,
}

/// Record whose lookup key is derived from the value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedEvidenceRecord {
    pub derived_key: String,
    pub role: String,
    pub payload_identity: String,
}

/// Semantic diff review input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticEvidenceDiffInput {
    pub before_identity: String,
    pub current_before_identity: String,
    pub after_identity: String,
    pub changed_fields: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Bounded attestation-shaped metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationVocabularyRecord {
    pub issuer: String,
    pub subject: String,
    pub scope: String,
    pub digest: String,
    pub non_claims: Vec<String>,
}

/// Field-scoped deterministic primitive issue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct OxidePrimitiveIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl OxidePrimitiveIssue {
    #[must_use]
    pub const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

/// Validation report for Oxide-reference primitives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OxidePrimitiveReport {
    pub valid: bool,
    pub issues: Vec<OxidePrimitiveIssue>,
    pub supported_claim: String,
}

/// Validate reference-only inventory rows.
#[must_use]
pub fn validate_reference_inventory(rows: &[OxideReferenceInventoryRow]) -> OxidePrimitiveReport {
    let mut issues = Vec::new();
    for row in rows {
        if row.repository.trim().is_empty() {
            issues.push(OxidePrimitiveIssue::new(FIELD_REFERENCE, "repository_missing"));
        }
        if !SUPPORTED_LICENSE_POSTURES.contains(&row.license_posture.as_str()) {
            issues.push(OxidePrimitiveIssue::new(FIELD_LICENSE, "unsupported_license_posture"));
        }
        if row.msrv_posture.trim().is_empty() || row.no_std_posture.trim().is_empty() {
            issues.push(OxidePrimitiveIssue::new(FIELD_REFERENCE, "portability_posture_missing"));
        }
        if row.intended_adaptation.trim().is_empty() {
            issues.push(OxidePrimitiveIssue::new(FIELD_REFERENCE, "adaptation_missing"));
        }
        if row.non_adoption_boundaries.is_empty() {
            issues.push(OxidePrimitiveIssue::new(FIELD_BOUNDARY, "non_adoption_boundary_missing"));
        }
        validate_non_claims(&row.non_claims, &mut issues);
    }
    finish_report(issues)
}

/// Validate typed IDs and preserve BLAKE3 as canonical stack identity.
#[must_use]
pub fn validate_typed_evidence_identifier(identifier: &TypedEvidenceIdentifier) -> OxidePrimitiveReport {
    let mut issues = Vec::new();
    if !SUPPORTED_IDENTIFIER_ROLES.contains(&identifier.role.as_str()) {
        issues.push(OxidePrimitiveIssue::new(FIELD_ROLE, "unknown_identifier_role"));
    }
    validate_digest(&identifier.blake3_identity, DigestRole::ArtifactIdentity, &mut issues);
    if let Some(uuid) = &identifier.external_uuid
        && !looks_like_uuid(uuid)
    {
        issues.push(OxidePrimitiveIssue::new(FIELD_IDENTIFIER, "malformed_external_uuid"));
    }
    validate_non_claims(&identifier.non_claims, &mut issues);
    finish_report(issues)
}

/// Build a derived-key collection with duplicate-key rejection.
#[must_use]
pub fn validate_derived_key_collection(records: &[DerivedEvidenceRecord]) -> OxidePrimitiveReport {
    let mut issues = Vec::new();
    let mut by_key = BTreeMap::new();
    for record in records {
        if record.derived_key.trim().is_empty() {
            issues.push(OxidePrimitiveIssue::new(FIELD_KEY, "derived_key_missing"));
            continue;
        }
        if by_key.insert(record.derived_key.as_str(), record).is_some() {
            issues.push(OxidePrimitiveIssue::new(FIELD_KEY, "duplicate_derived_key"));
        }
        if !SUPPORTED_IDENTIFIER_ROLES.contains(&record.role.as_str()) {
            issues.push(OxidePrimitiveIssue::new(FIELD_ROLE, "role_mismatch"));
        }
        validate_digest(&record.payload_identity, DigestRole::ArtifactIdentity, &mut issues);
    }
    finish_report(issues)
}

/// Validate semantic diff review evidence without claiming semantic equivalence.
#[must_use]
pub fn validate_semantic_diff(input: &SemanticEvidenceDiffInput) -> OxidePrimitiveReport {
    let mut issues = Vec::new();
    validate_digest(&input.before_identity, DigestRole::ArtifactIdentity, &mut issues);
    validate_digest(&input.current_before_identity, DigestRole::ArtifactIdentity, &mut issues);
    validate_digest(&input.after_identity, DigestRole::ArtifactIdentity, &mut issues);
    if input.before_identity != input.current_before_identity {
        issues.push(OxidePrimitiveIssue::new(FIELD_BASELINE, "stale_baseline"));
    }
    if input.changed_fields.is_empty() {
        issues.push(OxidePrimitiveIssue::new("changed_fields", "changed_fields_missing"));
    }
    validate_non_claims(&input.non_claims, &mut issues);
    finish_report(issues)
}

/// Validate bounded attestation vocabulary metadata.
#[must_use]
pub fn validate_attestation_vocabulary(record: &AttestationVocabularyRecord) -> OxidePrimitiveReport {
    let mut issues = Vec::new();
    if record.issuer.trim().is_empty() {
        issues.push(OxidePrimitiveIssue::new(FIELD_ISSUER, "issuer_missing"));
    }
    if record.subject.trim().is_empty() {
        issues.push(OxidePrimitiveIssue::new(FIELD_SUBJECT, "subject_missing"));
    }
    if record.scope.trim().is_empty() {
        issues.push(OxidePrimitiveIssue::new(FIELD_SCOPE, "scope_missing"));
    }
    validate_digest(&record.digest, DigestRole::ArtifactIdentity, &mut issues);
    validate_non_claims(&record.non_claims, &mut issues);
    finish_report(issues)
}

fn validate_digest(value: &str, role: DigestRole, issues: &mut Vec<OxidePrimitiveIssue>) {
    if parse_digest(value, role, MIXED_BLAKE3_PROFILE, FIELD_DIGEST).is_err() {
        issues.push(OxidePrimitiveIssue::new(FIELD_DIGEST, "malformed_blake3"));
    }
}

fn validate_non_claims(non_claims: &[String], issues: &mut Vec<OxidePrimitiveIssue>) {
    if non_claims.is_empty() {
        issues.push(OxidePrimitiveIssue::new(FIELD_NON_CLAIMS, "non_claims_missing"));
        return;
    }
    if !non_claims.iter().any(|claim| claim.contains(OXIDE_PRIMITIVES_REQUIRED_NON_CLAIM)) {
        issues.push(OxidePrimitiveIssue::new(FIELD_NON_CLAIMS, "non_claims_weakened"));
    }
    if non_claims.iter().any(|claim| contains_overclaim(claim)) {
        issues.push(OxidePrimitiveIssue::new(FIELD_NON_CLAIMS, "overclaim_detected"));
    }
}

fn looks_like_uuid(uuid: &str) -> bool {
    uuid.len() == UUID_TEXT_LENGTH
        && uuid.chars().filter(|character| *character == UUID_SEPARATOR).count() == UUID_SEPARATOR_COUNT
}

const UUID_SEPARATOR_COUNT: usize = 4;

fn finish_report(mut issues: Vec<OxidePrimitiveIssue>) -> OxidePrimitiveReport {
    issues.sort();
    issues.dedup();
    OxidePrimitiveReport {
        valid: issues.is_empty(),
        issues,
        supported_claim: OXIDE_PRIMITIVES_SUPPORTED_CLAIM.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const OTHER_DIGEST: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    const UUID: &str = "123e4567-e89b-12d3-a456-426614174000";

    fn non_claims() -> Vec<String> {
        vec![OXIDE_PRIMITIVES_REQUIRED_NON_CLAIM.to_owned()]
    }

    #[test]
    fn reference_inventory_records_boundaries() {
        // r[verify valence.evidence_ir.oxide_primitives.reference_inventory]
        let row = OxideReferenceInventoryRow {
            repository: "oxidecomputer/iddqd".to_owned(),
            license_posture: "MIT OR Apache-2.0".to_owned(),
            msrv_posture: "Rust 1.85".to_owned(),
            no_std_posture: "alloc required; IdOrdMap uses std".to_owned(),
            intended_adaptation: "derived-key evidence collections".to_owned(),
            non_adoption_boundaries: vec!["reference only until fixture-backed adoption".to_owned()],
            non_claims: non_claims(),
        };

        let report = validate_reference_inventory(&[row]);

        assert!(report.valid);
        assert!(report.supported_claim.contains("reference and review evidence only"));
    }

    #[test]
    fn typed_ids_keep_blake3_canonical_and_uuid_external() {
        // r[verify valence.evidence_ir.oxide_primitives.typed_ids]
        let id = TypedEvidenceIdentifier {
            role: "artifact".to_owned(),
            blake3_identity: DIGEST.to_owned(),
            external_uuid: Some(UUID.to_owned()),
            non_claims: non_claims(),
        };

        let report = validate_typed_evidence_identifier(&id);

        assert!(report.valid);
    }

    #[test]
    fn typed_id_role_or_uuid_mismatch_fails_closed() {
        // r[verify valence.evidence_ir.oxide_primitives.validation.negative]
        let id = TypedEvidenceIdentifier {
            role: "binary".to_owned(),
            blake3_identity: "not-a-digest".to_owned(),
            external_uuid: Some("not-a-uuid".to_owned()),
            non_claims: Vec::new(),
        };

        let report = validate_typed_evidence_identifier(&id);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "unknown_identifier_role"));
        assert!(report.issues.iter().any(|issue| issue.code == "malformed_external_uuid"));
        assert!(report.issues.iter().any(|issue| issue.code == "malformed_blake3"));
        assert!(report.issues.iter().any(|issue| issue.code == "non_claims_missing"));
    }

    #[test]
    fn derived_key_collection_rejects_duplicates() {
        // r[verify valence.evidence_ir.oxide_primitives.identity_maps.duplicate_rejection]
        let records = vec![
            DerivedEvidenceRecord {
                derived_key: "artifact-a".to_owned(),
                role: "artifact".to_owned(),
                payload_identity: DIGEST.to_owned(),
            },
            DerivedEvidenceRecord {
                derived_key: "artifact-a".to_owned(),
                role: "receipt".to_owned(),
                payload_identity: OTHER_DIGEST.to_owned(),
            },
        ];

        let report = validate_derived_key_collection(&records);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "duplicate_derived_key"));
    }

    #[test]
    fn semantic_diff_rejects_stale_baseline() {
        // r[verify valence.evidence_ir.oxide_primitives.semantic_diffs]
        let input = SemanticEvidenceDiffInput {
            before_identity: DIGEST.to_owned(),
            current_before_identity: OTHER_DIGEST.to_owned(),
            after_identity: OTHER_DIGEST.to_owned(),
            changed_fields: vec!["payload.kind".to_owned()],
            non_claims: non_claims(),
        };

        let report = validate_semantic_diff(&input);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "stale_baseline"));
    }

    #[test]
    fn attestation_metadata_requires_explicit_scope_and_boundary() {
        // r[verify valence.evidence_ir.oxide_primitives.attestation_vocabulary]
        let valid = AttestationVocabularyRecord {
            issuer: "issuer".to_owned(),
            subject: "subject".to_owned(),
            scope: "measurement".to_owned(),
            digest: DIGEST.to_owned(),
            non_claims: non_claims(),
        };
        let invalid = AttestationVocabularyRecord {
            issuer: String::new(),
            subject: String::new(),
            scope: String::new(),
            digest: "bad".to_owned(),
            non_claims: vec!["proves evidence truth".to_owned()],
        };

        let pass = validate_attestation_vocabulary(&valid);
        let fail = validate_attestation_vocabulary(&invalid);

        assert!(pass.valid);
        assert!(!fail.valid);
        assert!(fail.issues.iter().any(|issue| issue.code == "issuer_missing"));
        assert!(fail.issues.iter().any(|issue| issue.code == "scope_missing"));
        assert!(fail.issues.iter().any(|issue| issue.code == "overclaim_detected"));
    }
}
