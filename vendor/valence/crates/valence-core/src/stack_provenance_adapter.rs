//! Stack-provenance adapter for bounded Valence/Octet/Mantle traceability.
//!
//! The adapter turns verified Octet provenance exports into a Valence sidecar
//! that Mantle can bundle opaquely. Validation proves only identity and linkage:
//! artifact digests, evidence tiers, source/function/requirement IDs, and the
//! Mantle external-evidence digest. It does not prove behavioral correctness,
//! semantic equivalence, whole-program safety, or requirement satisfaction.
//!
//! Accepted stack-provenance traceability markers for the maintained adapter surface:
//! r[impl valence.stack_provenance_adapter]
//! r[impl valence.stack_provenance_adapter.boundary]
//! r[impl valence.stack_provenance_adapter.boundary.operator]
//! r[impl valence.stack_provenance_adapter.broken_chain_fixtures]
//! r[impl valence.stack_provenance_adapter.built_binary_thread]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.binary_digest]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.cli]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.cli.diagnostics]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.cli.verify]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.e2e]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.mantle_binary_identity]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.missing_mantle_binary]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.non_claims]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.octet_export]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.sidecar_digest]
//! r[impl valence.stack_provenance_adapter.built_binary_thread.stale_binary]
//! r[impl valence.stack_provenance_adapter.cli]
//! r[impl valence.stack_provenance_adapter.cli.build]
//! r[impl valence.stack_provenance_adapter.cli.verify]
//! r[impl valence.stack_provenance_adapter.cross_repo_fixtures]
//! r[impl valence.stack_provenance_adapter.cross_repo_fixtures.deep]
//! r[impl valence.stack_provenance_adapter.cross_repo_fixtures.fast]
//! r[impl valence.stack_provenance_adapter.cross_repo_fixtures.non_claims]
//! r[impl valence.stack_provenance_adapter.cross_repo_fixtures.positive]
//! r[impl valence.stack_provenance_adapter.deterministic]
//! r[impl valence.stack_provenance_adapter.graph_report.markdown]
//! r[impl valence.stack_provenance_adapter.mantle_external_evidence]
//! r[impl valence.stack_provenance_adapter.mantle_external_evidence.digest]
//! r[impl valence.stack_provenance_adapter.mantle_external_evidence.missing_role]
//! r[impl valence.stack_provenance_adapter.nickel_contracts]
//! r[impl valence.stack_provenance_adapter.nickel_contracts.cli]
//! r[impl valence.stack_provenance_adapter.nickel_contracts.docs]
//! r[impl valence.stack_provenance_adapter.nickel_contracts.fixtures]
//! r[impl valence.stack_provenance_adapter.nickel_contracts.gate]
//! r[impl valence.stack_provenance_adapter.nickel_contracts.metadata_inputs]
//! r[impl valence.stack_provenance_adapter.nickel_contracts.rust_boundary]
//! r[impl valence.stack_provenance_adapter.nickel_contracts.static_invariants]
//! r[impl valence.stack_provenance_adapter.nickel_contracts.thread_input]
//! r[impl valence.stack_provenance_adapter.preserves_values]
//! r[impl valence.stack_provenance_adapter.preserves_values.boundary]
//! r[impl valence.stack_provenance_adapter.preserves_values.corpus]
//! r[impl valence.stack_provenance_adapter.preserves_values.docs]
//! r[impl valence.stack_provenance_adapter.preserves_values.gate]
//! r[impl valence.stack_provenance_adapter.preserves_values.hashes]
//! r[impl valence.stack_provenance_adapter.preserves_values.metadata_surfaces]
//! r[impl valence.stack_provenance_adapter.preserves_values.receipts]
//! r[impl valence.stack_provenance_adapter.preserves_values.report_diff]
//! r[impl valence.stack_provenance_adapter.sidecar]
//! r[impl valence.stack_provenance_adapter.typed_boundary_values]
//! r[impl valence.stack_provenance_adapter.typed_boundary_values.boundary]
//! r[impl valence.stack_provenance_adapter.typed_boundary_values.digest_roles]
//! r[impl valence.stack_provenance_adapter.typed_boundary_values.docs]
//! r[impl valence.stack_provenance_adapter.typed_boundary_values.dto_parse]
//! r[impl valence.stack_provenance_adapter.typed_boundary_values.evidence_tiers]
//! r[impl valence.stack_provenance_adapter.typed_boundary_values.preserves]
//! r[impl valence.stack_provenance_adapter.typed_boundary_values.tests]
//! r[impl valence.stack_provenance_adapter.typed_boundary_values.validation]
//! r[impl valence.stack_provenance_adapter.validation]
//! r[impl valence.stack_provenance_adapter.validation.binary_mismatch]
//! r[impl valence.stack_provenance_adapter.validation.boundary]
//! r[impl valence.stack_provenance_adapter.validation.missing_parent]
//! r[impl valence.stack_provenance_adapter.validation.no_tier_promotion]

use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use crate::provenance::PROVENANCE_BOUNDARY;
use crate::provenance::ProvenanceChain;
use crate::provenance::ProvenanceLink;
use crate::provenance::check_provenance_chain;
use crate::shared_identity::AdmittedBinaryDigest;
use crate::shared_identity::ArtifactIdentityRole;
use crate::shared_identity::BinaryRole;
use crate::shared_identity::DigestRef;
use crate::shared_identity::MIXED_BLAKE3_PROFILE;
use crate::shared_identity::PreservesMetadataRole;
use crate::shared_identity::SidecarRole;

mod graph;
pub use graph::*;

/// Canonical stack-provenance sidecar schema.
pub const STACK_PROVENANCE_SIDECAR_SCHEMA: &str = "valence.stack-provenance-sidecar.v1";
/// Octet-to-Valence export schema consumed by this adapter.
pub const OCTET_VALENCE_EXPORT_SCHEMA: &str = "octet-valence-provenance-export/v1";
/// Mantle external evidence role used for the sidecar.
pub const MANTLE_STACK_PROVENANCE_ROLE: &str = "stack-provenance-trace";
/// Claim scope recorded in the sidecar and Mantle external evidence metadata.
pub const STACK_PROVENANCE_CLAIM_SCOPE: &str = "identity-linkage-sidecar";
/// Required sidecar non-claim.
pub const STACK_PROVENANCE_NON_CLAIM: &str = "stack provenance proves identity and linkage only; it does not prove behavioral correctness, semantic equivalence, whole-program safety, or requirement satisfaction";
/// Required built-binary thread non-claim.
pub const BUILT_BINARY_THREAD_NON_CLAIM: &str = "built-binary thread verification proves measured-byte identity and linkage only; it does not prove behavioral correctness, semantic equivalence, requirement satisfaction, release eligibility, verifier soundness, or whole-program safety";
const FORMAL_PROOF_TIER: &str = "formal_proof";
const MANUAL_REVIEW_TIER: &str = "manual_review_required";
const EXTERNAL_TRELLIS_ORIGIN: &str = "external_trellis_import";
const VALID_VERIFICATION_STATUS: &str = "valid";
const BUILT_BINARY_OVERCLAIM_PATTERNS: &[&str] = &[
    "proves behavioral correctness",
    "proves semantic equivalence",
    "proves requirement satisfaction",
    "proves release eligibility",
    "proves whole-program safety",
    "certifies release eligibility",
    "guarantees behavioral correctness",
    "guarantees whole-program safety",
];
const FIELD_STACK_DIGEST: &str = "digest";
const PRESERVES_STRING_QUOTE_OVERHEAD: usize = 2;
const PRESERVES_OPTION_NONE: &str = "<none>";
const PRESERVES_OPTION_SOME_TAG: &str = "some";
const PRESERVES_METADATA_NON_CLAIM: &str = "Preserves stack-provenance values prove canonical metadata identity only; they do not prove measured-byte identity, behavioral correctness, semantic equivalence, requirement satisfaction, release eligibility, verifier soundness, or whole-program safety";

/// Canonical text encoding label used for stack-provenance Preserves values.
pub const STACK_PROVENANCE_PRESERVES_ENCODING: &str = "valence.stack-provenance-preserves-text.v1";

/// Compatibility wrapper for a shared typed binary digest view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinaryDigest<'a>(DigestRef<'a, BinaryRole>);

impl<'a> BinaryDigest<'a> {
    /// Parse a string-compatible binary digest.
    #[must_use]
    pub fn parse(value: &'a str) -> Option<Self> {
        DigestRef::parse_compatible(value, MIXED_BLAKE3_PROFILE, FIELD_STACK_DIGEST).ok().map(Self)
    }

    /// Return the validated digest hex without its optional prefix.
    #[must_use]
    pub fn normalized(self) -> &'a str {
        self.0.as_hex()
    }

    #[must_use]
    pub fn matches(self, other: Self) -> bool {
        self.0.matches(other.0)
    }
}

/// Compatibility wrapper for a shared typed sidecar digest view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidecarDigest<'a>(DigestRef<'a, SidecarRole>);

impl<'a> SidecarDigest<'a> {
    /// Parse a string-compatible sidecar digest.
    #[must_use]
    pub fn parse(value: &'a str) -> Option<Self> {
        DigestRef::parse_compatible(value, MIXED_BLAKE3_PROFILE, FIELD_STACK_DIGEST).ok().map(Self)
    }

    #[must_use]
    pub fn normalized(self) -> &'a str {
        self.0.as_hex()
    }

    #[must_use]
    pub fn matches(self, other: Self) -> bool {
        self.0.matches(other.0)
    }
}

/// Compatibility wrapper for a shared typed artifact digest view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactDigest<'a>(DigestRef<'a, ArtifactIdentityRole>);

impl<'a> ArtifactDigest<'a> {
    /// Parse a string-compatible artifact digest.
    #[must_use]
    pub fn parse(value: &'a str) -> Option<Self> {
        DigestRef::parse_compatible(value, MIXED_BLAKE3_PROFILE, FIELD_STACK_DIGEST).ok().map(Self)
    }

    #[must_use]
    pub fn normalized(self) -> &'a str {
        self.0.as_hex()
    }
}

/// Compatibility wrapper for a shared typed Preserves metadata digest view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreservesMetadataDigest<'a>(DigestRef<'a, PreservesMetadataRole>);

impl<'a> PreservesMetadataDigest<'a> {
    /// Parse a string-compatible Preserves metadata digest.
    #[must_use]
    pub fn parse(value: &'a str) -> Option<Self> {
        DigestRef::parse_compatible(value, MIXED_BLAKE3_PROFILE, FIELD_STACK_DIGEST).ok().map(Self)
    }

    #[must_use]
    pub fn normalized(self) -> &'a str {
        self.0.as_hex()
    }
}

/// Stack sidecar schema versions accepted by this verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackProvenanceSidecarSchema {
    V1,
}

impl StackProvenanceSidecarSchema {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        if value == STACK_PROVENANCE_SIDECAR_SCHEMA {
            Some(Self::V1)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1 => STACK_PROVENANCE_SIDECAR_SCHEMA,
        }
    }
}

/// Octet export schema versions accepted by this verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OctetValenceExportSchema {
    V1,
}

impl OctetValenceExportSchema {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        if value == OCTET_VALENCE_EXPORT_SCHEMA {
            Some(Self::V1)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1 => OCTET_VALENCE_EXPORT_SCHEMA,
        }
    }
}

/// Verification statuses accepted by this verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationStatus {
    Valid,
}

impl VerificationStatus {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        if value == VALID_VERIFICATION_STATUS {
            Some(Self::Valid)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Valid => VALID_VERIFICATION_STATUS,
        }
    }
}

/// Sidecar claim scopes accepted by this verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackProvenanceClaimScope {
    IdentityLinkageSidecar,
}

impl StackProvenanceClaimScope {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        if value == STACK_PROVENANCE_CLAIM_SCOPE {
            Some(Self::IdentityLinkageSidecar)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IdentityLinkageSidecar => STACK_PROVENANCE_CLAIM_SCOPE,
        }
    }
}

/// Mantle external-evidence roles accepted by this verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MantleExternalEvidenceRole {
    StackProvenanceTrace,
}

impl MantleExternalEvidenceRole {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        if value == MANTLE_STACK_PROVENANCE_ROLE {
            Some(Self::StackProvenanceTrace)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StackProvenanceTrace => MANTLE_STACK_PROVENANCE_ROLE,
        }
    }
}

/// Evidence tiers accepted or explicitly preserved by this verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceTier<'a> {
    FormalProof,
    ManualReviewRequired,
    Unknown(&'a str),
}

impl<'a> EvidenceTier<'a> {
    #[must_use]
    pub fn parse(value: &'a str) -> Self {
        match value {
            FORMAL_PROOF_TIER => Self::FormalProof,
            MANUAL_REVIEW_TIER => Self::ManualReviewRequired,
            unknown => Self::Unknown(unknown),
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'a str {
        match self {
            Self::FormalProof => FORMAL_PROOF_TIER,
            Self::ManualReviewRequired => MANUAL_REVIEW_TIER,
            Self::Unknown(value) => value,
        }
    }

    #[must_use]
    pub fn is_formal_proof(self) -> bool {
        matches!(self, Self::FormalProof)
    }

    #[must_use]
    pub fn is_manual_review_required(self) -> bool {
        matches!(self, Self::ManualReviewRequired)
    }

    #[must_use]
    pub fn is_unknown(self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

/// Typed boundary values parsed from a stack sidecar DTO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackSidecarBoundaryValues<'a> {
    pub schema: StackProvenanceSidecarSchema,
    pub claim_scope: StackProvenanceClaimScope,
    pub binary_hash: BinaryDigest<'a>,
    pub parent_artifact_digests: Vec<ArtifactDigest<'a>>,
}

/// Typed boundary values parsed from an Octet export DTO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OctetExportBoundaryValues<'a> {
    pub schema_version: OctetValenceExportSchema,
    pub verification_status: VerificationStatus,
    pub artifact_digests: Vec<ArtifactDigest<'a>>,
    pub provenance_evidence_tiers: Vec<EvidenceTier<'a>>,
    pub proof_card_evidence_tiers: Vec<EvidenceTier<'a>>,
    pub proof_card_statuses: Vec<VerificationStatus>,
}

/// Typed boundary values parsed from Mantle external evidence metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MantleExternalEvidenceBoundaryValues<'a> {
    pub role: MantleExternalEvidenceRole,
    pub schema: StackProvenanceSidecarSchema,
    pub digest_blake3: SidecarDigest<'a>,
    pub claim_scope: StackProvenanceClaimScope,
}

/// Typed boundary values parsed from a Mantle release manifest view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MantleReleaseManifestBoundaryValues<'a> {
    pub binary_digests: Vec<BinaryDigest<'a>>,
    pub provenance_coverage_binary_hash: Option<BinaryDigest<'a>>,
    pub stack_sidecar_external_evidence: MantleExternalEvidenceBoundaryValues<'a>,
}

/// Typed boundary values parsed from a built-binary thread report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltBinaryThreadReportBoundaryValues<'a> {
    pub binary_hash: BinaryDigest<'a>,
    pub sidecar_digest_blake3: SidecarDigest<'a>,
    pub preserves_content_hash: Option<PreservesMetadataDigest<'a>>,
}

/// Preserves metadata surface identity and canonical value bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProvenancePreservesEvidence {
    pub surface: String,
    pub preserves_encoding: String,
    pub preserves_value: String,
    pub preserves_content_hash: String,
    pub non_claims: Vec<String>,
}

/// Semantic comparison between two built-binary thread reports by Preserves value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProvenancePreservesDiff {
    pub equal: bool,
    pub baseline_preserves_content_hash: String,
    pub candidate_preserves_content_hash: String,
    pub changed_fields: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Verified Octet export consumed by the adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OctetValenceProvenanceExport {
    pub schema_version: String,
    pub artifact_dir: String,
    pub verification_status: String,
    pub artifacts: Vec<StackArtifactIdentity>,
    pub provenance_records: Vec<OctetProvenanceRecord>,
    #[serde(default)]
    pub proof_cards: Vec<OctetProofCard>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub non_claims: Vec<String>,
}

/// Generic artifact identity used by Octet, Cairn, Trellis, and Mantle adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackArtifactIdentity {
    pub artifact: String,
    pub blake3: String,
}

/// Normalized Octet provenance record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OctetProvenanceRecord {
    pub source_id: String,
    pub checker_id: String,
    pub evidence_tier: String,
    pub artifact: String,
    pub blake3: String,
    pub checked_scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifier_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_artifact_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_action: Option<String>,
    #[serde(default)]
    pub parent_artifacts: Vec<String>,
}

/// Normalized proof-card summary from Octet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OctetProofCard {
    pub proof_or_property_id: String,
    pub source_requirement_or_policy_id: String,
    pub policy_id: String,
    pub rail_index: u32,
    pub receipt_path: String,
    pub artifact_blake3_identities: Vec<StackArtifactIdentity>,
    pub evidence_tier: String,
    pub evidence_origin: String,
    pub status: String,
    #[serde(default)]
    pub non_claims: Vec<String>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

/// Normalized Trellis evidence record for future adapter inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrellisProofEvidenceRecord {
    pub proof_id: String,
    pub repository_revision: String,
    pub artifact: StackArtifactIdentity,
    pub evidence_tier: String,
}

/// Normalized Cairn receipt identity for future adapter inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CairnReceiptRecord {
    pub requirement_id: String,
    pub receipt_hash: String,
    pub stage: String,
}

/// Mantle external evidence record parsed from release manifests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MantleExternalEvidenceRecord {
    pub role: String,
    pub schema: String,
    pub relative_path: String,
    pub digest_blake3: String,
    pub claim_scope: String,
    #[serde(default)]
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct MantleManifestView {
    #[serde(default)]
    external_evidence: Vec<MantleExternalEvidenceRecord>,
}

/// Minimal Mantle binary identity consumed by the Valence semantic verifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MantleBinaryIdentity {
    pub relative_path: String,
    pub digest_blake3: String,
}

/// Minimal Mantle provenance coverage view consumed by built-binary threading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MantleProvenanceCoverageView {
    pub binary_hash: String,
}

/// Minimal Mantle release manifest view for stack-specific Valence verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MantleReleaseManifestView {
    #[serde(default)]
    pub external_evidence: Vec<MantleExternalEvidenceRecord>,
    #[serde(default)]
    pub binaries: Vec<MantleBinaryIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_coverage: Option<MantleProvenanceCoverageView>,
}

/// Pure input for verifying that stack provenance reaches measured binary bytes.
pub struct BuiltBinaryThreadInput<'a> {
    pub binary_bytes: &'a [u8],
    pub octet_export: &'a OctetValenceProvenanceExport,
    pub sidecar: &'a StackProvenanceSidecar,
    pub sidecar_bytes: &'a [u8],
    pub mantle_manifest: &'a MantleReleaseManifestView,
}

/// Deterministic report for built-binary thread verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuiltBinaryThreadReport {
    pub valid: bool,
    pub issue_codes: Vec<String>,
    pub binary_hash: String,
    pub sidecar_digest_blake3: String,
    pub source_ids: Vec<String>,
    pub requirement_ids: Vec<String>,
    pub artifact_identities: Vec<StackArtifactIdentity>,
    pub non_claims: Vec<String>,
    #[serde(default)]
    pub preserves_content_hash: String,
}

/// Canonical Valence stack-provenance sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProvenanceSidecar {
    pub schema: String,
    pub claim_scope: String,
    pub binary_hash: String,
    pub octet_export_artifact_dir: String,
    pub chain: ProvenanceChain,
    pub parent_artifacts: Vec<StackArtifactIdentity>,
    pub diagnostics: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Validation report for stack-provenance sidecars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProvenanceReport {
    pub valid: bool,
    pub issue_codes: Vec<String>,
}

/// Parse typed boundary values from a stack sidecar DTO.
pub fn parse_stack_sidecar_boundary_values(
    sidecar: &StackProvenanceSidecar,
) -> Result<StackSidecarBoundaryValues<'_>, Vec<String>> {
    let mut issue_codes = Vec::new();
    let schema = StackProvenanceSidecarSchema::parse(&sidecar.schema);
    let claim_scope = StackProvenanceClaimScope::parse(&sidecar.claim_scope);
    let binary_hash = BinaryDigest::parse(&sidecar.binary_hash);
    let parent_artifact_digests = parse_artifact_identity_digests(&sidecar.parent_artifacts, &mut issue_codes);
    if schema.is_none() {
        issue_codes.push("schema_mismatch".to_owned());
    }
    if claim_scope.is_none() {
        issue_codes.push("claim_scope_mismatch".to_owned());
    }
    if binary_hash.is_none() {
        issue_codes.push("binary_hash_invalid".to_owned());
    }
    match (schema, claim_scope, binary_hash, issue_codes.is_empty()) {
        (Some(schema), Some(claim_scope), Some(binary_hash), true) => Ok(StackSidecarBoundaryValues {
            schema,
            claim_scope,
            binary_hash,
            parent_artifact_digests,
        }),
        _ => Err(sorted_unique(issue_codes)),
    }
}

/// Parse typed boundary values from an Octet Valence provenance export DTO.
pub fn parse_octet_export_boundary_values(
    export: &OctetValenceProvenanceExport,
) -> Result<OctetExportBoundaryValues<'_>, Vec<String>> {
    let mut issue_codes = Vec::new();
    let schema_version = OctetValenceExportSchema::parse(&export.schema_version);
    let verification_status = VerificationStatus::parse(&export.verification_status);
    let mut artifact_digests = parse_artifact_identity_digests(&export.artifacts, &mut issue_codes);
    for record in &export.provenance_records {
        if let Some(digest) = ArtifactDigest::parse(&record.blake3) {
            artifact_digests.push(digest);
        } else {
            issue_codes.push("artifact_hash_invalid".to_owned());
        }
    }
    let provenance_evidence_tiers = export
        .provenance_records
        .iter()
        .map(|record| EvidenceTier::parse(&record.evidence_tier))
        .collect::<Vec<_>>();
    let mut proof_card_evidence_tiers = Vec::new();
    let mut proof_card_statuses = Vec::new();
    for card in &export.proof_cards {
        artifact_digests.extend(parse_artifact_identity_digests(&card.artifact_blake3_identities, &mut issue_codes));
        proof_card_evidence_tiers.push(EvidenceTier::parse(&card.evidence_tier));
        match VerificationStatus::parse(&card.status) {
            Some(status) => proof_card_statuses.push(status),
            None => issue_codes.push("proof_card_status_invalid".to_owned()),
        }
    }
    if schema_version.is_none() {
        issue_codes.push("octet_schema_mismatch".to_owned());
    }
    if verification_status.is_none() {
        issue_codes.push("octet_verification_not_valid".to_owned());
    }
    match (schema_version, verification_status, issue_codes.is_empty()) {
        (Some(schema_version), Some(verification_status), true) => Ok(OctetExportBoundaryValues {
            schema_version,
            verification_status,
            artifact_digests,
            provenance_evidence_tiers,
            proof_card_evidence_tiers,
            proof_card_statuses,
        }),
        _ => Err(sorted_unique(issue_codes)),
    }
}

/// Parse typed boundary values from Mantle external evidence metadata.
pub fn parse_mantle_external_evidence_boundary_values(
    record: &MantleExternalEvidenceRecord,
) -> Result<MantleExternalEvidenceBoundaryValues<'_>, Vec<String>> {
    let mut issue_codes = Vec::new();
    let role = MantleExternalEvidenceRole::parse(&record.role);
    let schema = StackProvenanceSidecarSchema::parse(&record.schema);
    let digest_blake3 = SidecarDigest::parse(&record.digest_blake3);
    let claim_scope = StackProvenanceClaimScope::parse(&record.claim_scope);
    if role.is_none() {
        issue_codes.push("mantle_role_missing".to_owned());
    }
    if schema.is_none() {
        issue_codes.push("mantle_schema_mismatch".to_owned());
    }
    if digest_blake3.is_none() {
        issue_codes.push("sidecar_digest_invalid".to_owned());
    }
    if claim_scope.is_none() {
        issue_codes.push("mantle_claim_scope_mismatch".to_owned());
    }
    match (role, schema, digest_blake3, claim_scope, issue_codes.is_empty()) {
        (Some(role), Some(schema), Some(digest_blake3), Some(claim_scope), true) => {
            Ok(MantleExternalEvidenceBoundaryValues {
                role,
                schema,
                digest_blake3,
                claim_scope,
            })
        }
        _ => Err(sorted_unique(issue_codes)),
    }
}

/// Parse typed boundary values from a Mantle release manifest view.
pub fn parse_mantle_release_manifest_boundary_values(
    manifest: &MantleReleaseManifestView,
) -> Result<MantleReleaseManifestBoundaryValues<'_>, Vec<String>> {
    let mut issue_codes = Vec::new();
    let mut binary_digests = Vec::new();
    for binary in &manifest.binaries {
        match BinaryDigest::parse(&binary.digest_blake3) {
            Some(digest) => binary_digests.push(digest),
            None => issue_codes.push("mantle_binary_digest_invalid".to_owned()),
        }
    }
    let provenance_coverage_binary_hash = match manifest.provenance_coverage.as_ref() {
        Some(coverage) => match BinaryDigest::parse(&coverage.binary_hash) {
            Some(digest) => Some(digest),
            None => {
                issue_codes.push("provenance_coverage_binary_invalid".to_owned());
                None
            }
        },
        None => None,
    };
    let stack_sidecar_external_evidence = match manifest
        .external_evidence
        .iter()
        .find(|record| MantleExternalEvidenceRole::parse(&record.role).is_some())
    {
        Some(record) => match parse_mantle_external_evidence_boundary_values(record) {
            Ok(boundary) => Some(boundary),
            Err(record_issues) => {
                issue_codes.extend(record_issues);
                None
            }
        },
        None => {
            issue_codes.push("mantle_role_missing".to_owned());
            None
        }
    };
    match (stack_sidecar_external_evidence, issue_codes.is_empty()) {
        (Some(stack_sidecar_external_evidence), true) => Ok(MantleReleaseManifestBoundaryValues {
            binary_digests,
            provenance_coverage_binary_hash,
            stack_sidecar_external_evidence,
        }),
        _ => Err(sorted_unique(issue_codes)),
    }
}

/// Parse typed boundary values from a built-binary thread report DTO.
pub fn parse_built_binary_thread_report_boundary_values(
    report: &BuiltBinaryThreadReport,
) -> Result<BuiltBinaryThreadReportBoundaryValues<'_>, Vec<String>> {
    let mut issue_codes = Vec::new();
    let binary_hash = BinaryDigest::parse(&report.binary_hash);
    let sidecar_digest_blake3 = SidecarDigest::parse(&report.sidecar_digest_blake3);
    let preserves_content_hash = if report.preserves_content_hash.is_empty() {
        None
    } else {
        match PreservesMetadataDigest::parse(&report.preserves_content_hash) {
            Some(digest) => Some(digest),
            None => {
                issue_codes.push("preserves_content_hash_invalid".to_owned());
                None
            }
        }
    };
    if binary_hash.is_none() {
        issue_codes.push("binary_hash_invalid".to_owned());
    }
    if sidecar_digest_blake3.is_none() {
        issue_codes.push("sidecar_digest_invalid".to_owned());
    }
    match (binary_hash, sidecar_digest_blake3, issue_codes.is_empty()) {
        (Some(binary_hash), Some(sidecar_digest_blake3), true) => Ok(BuiltBinaryThreadReportBoundaryValues {
            binary_hash,
            sidecar_digest_blake3,
            preserves_content_hash,
        }),
        _ => Err(sorted_unique(issue_codes)),
    }
}

/// Build a sidecar from a verified Octet export and a release binary hash.
#[must_use]
pub fn build_stack_provenance_sidecar(
    octet_export: &OctetValenceProvenanceExport,
    binary_hash: &str,
) -> StackProvenanceSidecar {
    let chain = ProvenanceChain::from_links(
        octet_export
            .provenance_records
            .iter()
            .map(|record| provenance_link_from_octet(record, &octet_export.proof_cards, binary_hash))
            .collect(),
    );
    StackProvenanceSidecar {
        schema: STACK_PROVENANCE_SIDECAR_SCHEMA.to_owned(),
        claim_scope: STACK_PROVENANCE_CLAIM_SCOPE.to_owned(),
        binary_hash: binary_hash.to_owned(),
        octet_export_artifact_dir: octet_export.artifact_dir.clone(),
        chain,
        parent_artifacts: octet_export.artifacts.clone(),
        diagnostics: octet_export.diagnostics.clone(),
        non_claims: vec![STACK_PROVENANCE_NON_CLAIM.to_owned(), PROVENANCE_BOUNDARY.to_owned()],
    }
}

/// Validate a sidecar without any filesystem access.
#[must_use]
pub fn validate_stack_provenance_sidecar(sidecar: &StackProvenanceSidecar) -> StackProvenanceReport {
    let mut issue_codes = Vec::new();
    match parse_stack_sidecar_boundary_values(sidecar) {
        Ok(boundary) => issue_codes.extend(validate_chain(&sidecar.chain, boundary.binary_hash)),
        Err(parse_issues) => {
            issue_codes.extend(parse_issues);
            if let Some(binary_hash) = BinaryDigest::parse(&sidecar.binary_hash) {
                issue_codes.extend(validate_chain(&sidecar.chain, binary_hash));
            } else {
                issue_codes.extend(validate_chain_artifact_digests(&sidecar.chain));
            }
        }
    }
    if !sidecar.non_claims.iter().any(|claim| claim == STACK_PROVENANCE_NON_CLAIM) {
        issue_codes.push("non_claim_boundary_missing".to_owned());
    }
    if !sidecar.diagnostics.is_empty() {
        issue_codes.push("diagnostics_present".to_owned());
    }
    if sidecar.parent_artifacts.is_empty() {
        issue_codes.push("parent_artifact_missing".to_owned());
    }
    issue_codes = sorted_unique(issue_codes);
    StackProvenanceReport {
        valid: issue_codes.is_empty(),
        issue_codes,
    }
}

/// Validate an Octet export before building a sidecar.
#[must_use]
pub fn validate_octet_export(export: &OctetValenceProvenanceExport) -> StackProvenanceReport {
    let mut issue_codes = Vec::new();
    if let Err(parse_issues) = parse_octet_export_boundary_values(export) {
        issue_codes.extend(parse_issues);
    }
    if !export.diagnostics.is_empty() {
        issue_codes.push("octet_diagnostics_present".to_owned());
    }
    if export.provenance_records.is_empty() {
        issue_codes.push("octet_provenance_missing".to_owned());
    }
    issue_codes.extend(validate_parent_artifacts(export));
    issue_codes.extend(validate_evidence_tiers(export));
    issue_codes = sorted_unique(issue_codes);
    StackProvenanceReport {
        valid: issue_codes.is_empty(),
        issue_codes,
    }
}

/// Build a stack sidecar from an already admitted binary-role argument.
pub fn build_sidecar_json_from_typed_binary(
    octet_export_json: &str,
    binary_hash: &AdmittedBinaryDigest,
) -> Result<String, StackProvenanceReport> {
    build_sidecar_json_from_octet_export_json(octet_export_json, binary_hash.original())
}

/// Compatibility adapter that parses, builds, validates, and renders a stack sidecar.
pub fn build_sidecar_json_from_octet_export_json(
    octet_export_json: &str,
    binary_hash: &str,
) -> Result<String, StackProvenanceReport> {
    let export = match serde_json::from_str::<OctetValenceProvenanceExport>(octet_export_json) {
        Ok(export) => export,
        Err(_) => {
            return Err(StackProvenanceReport {
                valid: false,
                issue_codes: vec!["octet_export_malformed".to_owned()],
            });
        }
    };
    let export_report = validate_octet_export(&export);
    if !export_report.valid {
        return Err(export_report);
    }
    let sidecar = build_stack_provenance_sidecar(&export, binary_hash);
    let sidecar_report = validate_stack_provenance_sidecar(&sidecar);
    if !sidecar_report.valid {
        return Err(sidecar_report);
    }
    serde_json::to_string_pretty(&sidecar).map_err(|_| StackProvenanceReport {
        valid: false,
        issue_codes: vec!["sidecar_serialize_failed".to_owned()],
    })
}

/// Verify a sidecar against a Mantle release manifest external-evidence record.
#[must_use]
pub fn verify_mantle_external_evidence_sidecar(
    mantle_manifest_json: &str,
    sidecar_json: &str,
) -> StackProvenanceReport {
    let manifest = match serde_json::from_str::<MantleManifestView>(mantle_manifest_json) {
        Ok(manifest) => manifest,
        Err(_) => {
            return StackProvenanceReport {
                valid: false,
                issue_codes: vec!["mantle_manifest_malformed".to_owned()],
            };
        }
    };
    let sidecar = match serde_json::from_str::<StackProvenanceSidecar>(sidecar_json) {
        Ok(sidecar) => sidecar,
        Err(_) => {
            return StackProvenanceReport {
                valid: false,
                issue_codes: vec!["sidecar_malformed".to_owned()],
            };
        }
    };
    let mut issue_codes = validate_stack_provenance_sidecar(&sidecar).issue_codes;
    let digest = blake3_hex(sidecar_json.as_bytes());
    issue_codes.extend(validate_sidecar_external_evidence(&manifest.external_evidence, &digest));
    issue_codes = sorted_unique(issue_codes);
    StackProvenanceReport {
        valid: issue_codes.is_empty(),
        issue_codes,
    }
}

/// Parse local artifacts and verify that provenance reaches measured binary bytes.
#[must_use]
pub fn verify_built_binary_thread_json(
    binary_bytes: &[u8],
    octet_export_json: &str,
    sidecar_json: &str,
    mantle_manifest_json: &str,
) -> BuiltBinaryThreadReport {
    let binary_hash = blake3_hex(binary_bytes);
    let sidecar_digest_blake3 = blake3_hex(sidecar_json.as_bytes());
    let mut issue_codes = Vec::new();
    let octet_export = match serde_json::from_str::<OctetValenceProvenanceExport>(octet_export_json) {
        Ok(export) => export,
        Err(_) => {
            issue_codes.push("octet_export_malformed".to_owned());
            return built_binary_report(issue_codes, binary_hash, sidecar_digest_blake3, None, None);
        }
    };
    let sidecar = match serde_json::from_str::<StackProvenanceSidecar>(sidecar_json) {
        Ok(sidecar) => sidecar,
        Err(_) => {
            issue_codes.push("sidecar_malformed".to_owned());
            return built_binary_report(issue_codes, binary_hash, sidecar_digest_blake3, Some(&octet_export), None);
        }
    };
    let mantle_manifest = match serde_json::from_str::<MantleReleaseManifestView>(mantle_manifest_json) {
        Ok(manifest) => manifest,
        Err(_) => {
            issue_codes.push("mantle_manifest_malformed".to_owned());
            return built_binary_report(
                issue_codes,
                binary_hash,
                sidecar_digest_blake3,
                Some(&octet_export),
                Some(&sidecar),
            );
        }
    };
    verify_built_binary_thread(BuiltBinaryThreadInput {
        binary_bytes,
        octet_export: &octet_export,
        sidecar: &sidecar,
        sidecar_bytes: sidecar_json.as_bytes(),
        mantle_manifest: &mantle_manifest,
    })
}

/// Verify the measured built binary against Octet, Valence, and Mantle identities.
#[must_use]
pub fn verify_built_binary_thread(input: BuiltBinaryThreadInput<'_>) -> BuiltBinaryThreadReport {
    let binary_hash = blake3_hex(input.binary_bytes);
    let sidecar_digest_blake3 = blake3_hex(input.sidecar_bytes);
    let mut issue_codes = Vec::new();
    issue_codes.extend(validate_stack_provenance_sidecar(input.sidecar).issue_codes);
    issue_codes.extend(validate_octet_export(input.octet_export).issue_codes);
    if let Err(parse_issues) = parse_mantle_release_manifest_boundary_values(input.mantle_manifest) {
        issue_codes.extend(parse_issues);
    }
    issue_codes.extend(validate_sidecar_external_evidence(
        input.mantle_manifest.external_evidence.as_slice(),
        &sidecar_digest_blake3,
    ));
    issue_codes.extend(validate_measured_binary_identity(&binary_hash, input.sidecar, input.mantle_manifest));
    issue_codes.extend(validate_sidecar_matches_octet_export(input.octet_export, input.sidecar, &binary_hash));
    issue_codes.extend(validate_built_binary_claim_boundary(input.octet_export, input.sidecar));
    issue_codes.sort();
    issue_codes.dedup();
    built_binary_report(issue_codes, binary_hash, sidecar_digest_blake3, Some(input.octet_export), Some(input.sidecar))
}

/// Project an Octet export into canonical Preserves metadata evidence.
#[must_use]
pub fn preserves_evidence_for_octet_export(export: &OctetValenceProvenanceExport) -> StackProvenancePreservesEvidence {
    stack_preserves_evidence("octet-valence-provenance-export", preserves_octet_export_value(export))
}

/// Project a stack sidecar into canonical Preserves metadata evidence.
#[must_use]
pub fn preserves_evidence_for_stack_sidecar(sidecar: &StackProvenanceSidecar) -> StackProvenancePreservesEvidence {
    stack_preserves_evidence("stack-provenance-sidecar", preserves_sidecar_value(sidecar))
}

/// Project a Mantle release manifest view into canonical Preserves metadata evidence.
#[must_use]
pub fn preserves_evidence_for_mantle_manifest(
    manifest: &MantleReleaseManifestView,
) -> StackProvenancePreservesEvidence {
    stack_preserves_evidence("mantle-release-manifest-view", preserves_mantle_manifest_value(manifest))
}

/// Project a built-binary thread input envelope into canonical Preserves metadata evidence.
///
/// The projection intentionally ignores raw `binary_bytes` and `sidecar_bytes`; Rust byte hashing
/// remains the authority for those identities.
#[must_use]
pub fn preserves_evidence_for_built_binary_thread_input(
    input: &BuiltBinaryThreadInput<'_>,
) -> StackProvenancePreservesEvidence {
    stack_preserves_evidence(
        "built-binary-thread-input",
        preserves_record("built-binary-thread-input", &[
            ("binary_hash", preserves_binary_digest(&input.sidecar.binary_hash)),
            ("mantle_manifest", preserves_mantle_manifest_value(input.mantle_manifest)),
            ("octet_export", preserves_octet_export_value(input.octet_export)),
            ("sidecar", preserves_sidecar_value(input.sidecar)),
        ]),
    )
}

/// Project a built-binary thread verification report into canonical Preserves metadata evidence.
#[must_use]
pub fn preserves_evidence_for_built_binary_thread_report(
    report: &BuiltBinaryThreadReport,
) -> StackProvenancePreservesEvidence {
    stack_preserves_evidence("built-binary-thread-report", preserves_built_binary_thread_report_value(report))
}

/// Compute the BLAKE3 hash over canonical Preserves bytes for a built-binary thread report.
#[must_use]
pub fn preserves_content_hash_for_built_binary_thread_report(report: &BuiltBinaryThreadReport) -> String {
    blake3_hex(preserves_built_binary_thread_report_value(report).as_bytes())
}

/// Compare two built-binary thread reports by Preserves semantic value.
#[must_use]
pub fn compare_built_binary_thread_reports_by_preserves(
    baseline: &BuiltBinaryThreadReport,
    candidate: &BuiltBinaryThreadReport,
) -> StackProvenancePreservesDiff {
    let baseline_hash = preserves_content_hash_for_built_binary_thread_report(baseline);
    let candidate_hash = preserves_content_hash_for_built_binary_thread_report(candidate);
    let changed_fields = built_binary_report_changed_fields(baseline, candidate);
    StackProvenancePreservesDiff {
        equal: baseline_hash == candidate_hash,
        baseline_preserves_content_hash: baseline_hash,
        candidate_preserves_content_hash: candidate_hash,
        changed_fields,
        non_claims: vec![
            PRESERVES_METADATA_NON_CLAIM.to_owned(),
            BUILT_BINARY_THREAD_NON_CLAIM.to_owned(),
        ],
    }
}

fn stack_preserves_evidence(surface: &str, preserves_value: String) -> StackProvenancePreservesEvidence {
    StackProvenancePreservesEvidence {
        surface: surface.to_owned(),
        preserves_encoding: STACK_PROVENANCE_PRESERVES_ENCODING.to_owned(),
        preserves_content_hash: blake3_hex(preserves_value.as_bytes()),
        preserves_value,
        non_claims: vec![
            PRESERVES_METADATA_NON_CLAIM.to_owned(),
            STACK_PROVENANCE_NON_CLAIM.to_owned(),
        ],
    }
}

fn preserves_octet_export_value(export: &OctetValenceProvenanceExport) -> String {
    preserves_record("octet-valence-provenance-export", &[
        ("artifact_dir", preserves_string(&export.artifact_dir)),
        ("artifacts", preserves_artifact_identities(&export.artifacts)),
        ("diagnostics", preserves_string_list(&export.diagnostics)),
        ("non_claims", preserves_string_list(&export.non_claims)),
        ("proof_cards", preserves_proof_cards(&export.proof_cards)),
        ("provenance_records", preserves_octet_provenance_records(&export.provenance_records)),
        ("schema_version", preserves_octet_schema(&export.schema_version)),
        ("verification_status", preserves_verification_status(&export.verification_status)),
    ])
}

fn preserves_sidecar_value(sidecar: &StackProvenanceSidecar) -> String {
    preserves_record("stack-provenance-sidecar", &[
        ("binary_hash", preserves_binary_digest(&sidecar.binary_hash)),
        ("chain", preserves_chain(&sidecar.chain)),
        ("claim_scope", preserves_claim_scope(&sidecar.claim_scope)),
        ("diagnostics", preserves_string_list(&sidecar.diagnostics)),
        ("non_claims", preserves_string_list(&sidecar.non_claims)),
        ("octet_export_artifact_dir", preserves_string(&sidecar.octet_export_artifact_dir)),
        ("parent_artifacts", preserves_artifact_identities(&sidecar.parent_artifacts)),
        ("schema", preserves_stack_sidecar_schema(&sidecar.schema)),
    ])
}

fn preserves_mantle_manifest_value(manifest: &MantleReleaseManifestView) -> String {
    preserves_record("mantle-release-manifest-view", &[
        ("binaries", preserves_mantle_binaries(&manifest.binaries)),
        ("external_evidence", preserves_external_evidence(&manifest.external_evidence)),
        ("provenance_coverage", preserves_coverage(manifest.provenance_coverage.as_ref())),
    ])
}

fn preserves_built_binary_thread_report_value(report: &BuiltBinaryThreadReport) -> String {
    preserves_record("built-binary-thread-report", &[
        ("artifact_identities", preserves_artifact_identities(&report.artifact_identities)),
        ("binary_hash", preserves_binary_digest(&report.binary_hash)),
        ("issue_codes", preserves_string_list(&report.issue_codes)),
        ("non_claims", preserves_string_list(&report.non_claims)),
        ("requirement_ids", preserves_string_list(&report.requirement_ids)),
        ("sidecar_digest_blake3", preserves_sidecar_digest(&report.sidecar_digest_blake3)),
        ("source_ids", preserves_string_list(&report.source_ids)),
        ("valid", preserves_bool(report.valid)),
    ])
}

fn preserves_artifact_identities(artifacts: &[StackArtifactIdentity]) -> String {
    let mut ordered = artifacts.to_vec();
    ordered.sort_by(|left, right| (&left.artifact, &left.blake3).cmp(&(&right.artifact, &right.blake3)));
    preserves_list(
        ordered
            .iter()
            .map(|artifact| {
                preserves_record("artifact-identity", &[
                    ("artifact", preserves_string(&artifact.artifact)),
                    ("blake3", preserves_artifact_digest(&artifact.blake3)),
                ])
            })
            .collect(),
    )
}

fn preserves_octet_provenance_records(records: &[OctetProvenanceRecord]) -> String {
    let mut ordered = records.to_vec();
    ordered.sort_by(|left, right| {
        (&left.source_id, &left.checker_id, &left.artifact, &left.blake3).cmp(&(
            &right.source_id,
            &right.checker_id,
            &right.artifact,
            &right.blake3,
        ))
    });
    preserves_list(ordered.iter().map(preserves_octet_provenance_record).collect())
}

fn preserves_octet_provenance_record(record: &OctetProvenanceRecord) -> String {
    preserves_record("octet-provenance-record", &[
        ("artifact", preserves_string(&record.artifact)),
        ("blake3", preserves_artifact_digest(&record.blake3)),
        ("checked_scope", preserves_string(&record.checked_scope)),
        ("checker_id", preserves_string(&record.checker_id)),
        ("evidence_tier", preserves_evidence_tier(&record.evidence_tier)),
        ("human_action", preserves_optional_string(record.human_action.as_deref())),
        ("parent_artifacts", preserves_string_list(&record.parent_artifacts)),
        ("proof_artifact_identity", preserves_optional_string(record.proof_artifact_identity.as_deref())),
        ("source_id", preserves_string(&record.source_id)),
        ("verifier_identity", preserves_optional_string(record.verifier_identity.as_deref())),
    ])
}

fn preserves_proof_cards(cards: &[OctetProofCard]) -> String {
    let mut ordered = cards.to_vec();
    ordered.sort_by(|left, right| {
        (&left.proof_or_property_id, &left.policy_id, left.rail_index).cmp(&(
            &right.proof_or_property_id,
            &right.policy_id,
            right.rail_index,
        ))
    });
    preserves_list(ordered.iter().map(preserves_proof_card).collect())
}

fn preserves_proof_card(card: &OctetProofCard) -> String {
    preserves_record("octet-proof-card", &[
        ("artifact_blake3_identities", preserves_artifact_identities(&card.artifact_blake3_identities)),
        ("diagnostics", preserves_string_list(&card.diagnostics)),
        ("evidence_origin", preserves_string(&card.evidence_origin)),
        ("evidence_tier", preserves_evidence_tier(&card.evidence_tier)),
        ("non_claims", preserves_string_list(&card.non_claims)),
        ("policy_id", preserves_string(&card.policy_id)),
        ("proof_or_property_id", preserves_string(&card.proof_or_property_id)),
        ("rail_index", preserves_u32(card.rail_index)),
        ("receipt_path", preserves_string(&card.receipt_path)),
        ("source_requirement_or_policy_id", preserves_string(&card.source_requirement_or_policy_id)),
        ("status", preserves_verification_status(&card.status)),
    ])
}

fn preserves_chain(chain: &ProvenanceChain) -> String {
    preserves_record("provenance-chain", &[
        ("boundary", preserves_string(&chain.boundary)),
        ("links", preserves_links(&chain.links)),
        ("schema", preserves_string(&chain.schema)),
    ])
}

fn preserves_links(links: &[ProvenanceLink]) -> String {
    let mut ordered = links.to_vec();
    ordered.sort_by(|left, right| {
        (&left.source_id, &left.function_object_id).cmp(&(&right.source_id, &right.function_object_id))
    });
    preserves_list(ordered.iter().map(preserves_link).collect())
}

fn preserves_link(link: &ProvenanceLink) -> String {
    preserves_record("provenance-link", &[
        ("artifact_hash", preserves_optional_artifact_digest(link.artifact_hash.as_deref())),
        ("binary_hash", preserves_optional_binary_digest(link.binary_hash.as_deref())),
        ("function_object_id", preserves_string(&link.function_object_id)),
        ("requirement_ids", preserves_string_list(&link.requirement_ids)),
        ("source_id", preserves_string(&link.source_id)),
    ])
}

fn preserves_external_evidence(records: &[MantleExternalEvidenceRecord]) -> String {
    let mut ordered = records.to_vec();
    ordered.sort_by(|left, right| {
        (&left.role, &left.relative_path, &left.digest_blake3).cmp(&(
            &right.role,
            &right.relative_path,
            &right.digest_blake3,
        ))
    });
    preserves_list(ordered.iter().map(preserves_external_evidence_record).collect())
}

fn preserves_external_evidence_record(record: &MantleExternalEvidenceRecord) -> String {
    preserves_record("mantle-external-evidence", &[
        ("claim_scope", preserves_claim_scope(&record.claim_scope)),
        ("digest_blake3", preserves_sidecar_digest(&record.digest_blake3)),
        ("non_claims", preserves_string_list(&record.non_claims)),
        ("relative_path", preserves_string(&record.relative_path)),
        ("role", preserves_mantle_role(&record.role)),
        ("schema", preserves_stack_sidecar_schema(&record.schema)),
    ])
}

fn preserves_mantle_binaries(binaries: &[MantleBinaryIdentity]) -> String {
    let mut ordered = binaries.to_vec();
    ordered.sort_by(|left, right| {
        (&left.relative_path, &left.digest_blake3).cmp(&(&right.relative_path, &right.digest_blake3))
    });
    preserves_list(ordered.iter().map(preserves_mantle_binary).collect())
}

fn preserves_mantle_binary(binary: &MantleBinaryIdentity) -> String {
    preserves_record("mantle-binary-identity", &[
        ("digest_blake3", preserves_binary_digest(&binary.digest_blake3)),
        ("relative_path", preserves_string(&binary.relative_path)),
    ])
}

fn preserves_coverage(coverage: Option<&MantleProvenanceCoverageView>) -> String {
    match coverage {
        Some(coverage) => {
            preserves_record(PRESERVES_OPTION_SOME_TAG, &[("value", preserves_binary_digest(&coverage.binary_hash))])
        }
        None => PRESERVES_OPTION_NONE.to_owned(),
    }
}

fn preserves_binary_digest(value: &str) -> String {
    match BinaryDigest::parse(value) {
        Some(digest) => preserves_string(&digest.normalized().to_ascii_lowercase()),
        None => preserves_string(value),
    }
}

fn preserves_sidecar_digest(value: &str) -> String {
    match SidecarDigest::parse(value) {
        Some(digest) => preserves_string(&digest.normalized().to_ascii_lowercase()),
        None => preserves_string(value),
    }
}

fn preserves_artifact_digest(value: &str) -> String {
    match ArtifactDigest::parse(value) {
        Some(digest) => preserves_string(&digest.normalized().to_ascii_lowercase()),
        None => preserves_string(value),
    }
}

fn preserves_optional_binary_digest(value: Option<&str>) -> String {
    match value {
        Some(value) => preserves_record(PRESERVES_OPTION_SOME_TAG, &[("value", preserves_binary_digest(value))]),
        None => PRESERVES_OPTION_NONE.to_owned(),
    }
}

fn preserves_optional_artifact_digest(value: Option<&str>) -> String {
    match value {
        Some(value) => preserves_record(PRESERVES_OPTION_SOME_TAG, &[("value", preserves_artifact_digest(value))]),
        None => PRESERVES_OPTION_NONE.to_owned(),
    }
}

fn preserves_stack_sidecar_schema(value: &str) -> String {
    match StackProvenanceSidecarSchema::parse(value) {
        Some(schema) => preserves_string(schema.as_str()),
        None => preserves_string(value),
    }
}

fn preserves_octet_schema(value: &str) -> String {
    match OctetValenceExportSchema::parse(value) {
        Some(schema) => preserves_string(schema.as_str()),
        None => preserves_string(value),
    }
}

fn preserves_verification_status(value: &str) -> String {
    match VerificationStatus::parse(value) {
        Some(status) => preserves_string(status.as_str()),
        None => preserves_string(value),
    }
}

fn preserves_claim_scope(value: &str) -> String {
    match StackProvenanceClaimScope::parse(value) {
        Some(scope) => preserves_string(scope.as_str()),
        None => preserves_string(value),
    }
}

fn preserves_mantle_role(value: &str) -> String {
    match MantleExternalEvidenceRole::parse(value) {
        Some(role) => preserves_string(role.as_str()),
        None => preserves_string(value),
    }
}

fn preserves_evidence_tier(value: &str) -> String {
    preserves_string(EvidenceTier::parse(value).as_str())
}

fn preserves_optional_string(value: Option<&str>) -> String {
    match value {
        Some(value) => preserves_record(PRESERVES_OPTION_SOME_TAG, &[("value", preserves_string(value))]),
        None => PRESERVES_OPTION_NONE.to_owned(),
    }
}

fn preserves_bool(value: bool) -> String {
    if value { "#t".to_owned() } else { "#f".to_owned() }
}

fn preserves_u32(value: u32) -> String {
    value.to_string()
}

fn preserves_string_list(values: &[String]) -> String {
    let mut ordered = values.to_vec();
    ordered.sort();
    preserves_list(ordered.iter().map(|value| preserves_string(value)).collect())
}

fn preserves_list(values: Vec<String>) -> String {
    format!("[{}]", values.join(" "))
}

fn preserves_record(tag: &str, fields: &[(&str, String)]) -> String {
    let mut ordered_fields = fields.iter().map(|(field, value)| (*field, value.as_str())).collect::<Vec<_>>();
    ordered_fields.sort_by(|left, right| left.0.cmp(right.0));
    let rendered_fields =
        ordered_fields.iter().map(|(field, value)| format!("{}={}", field, value)).collect::<Vec<_>>();
    format!("<{} {}>", tag, rendered_fields.join(" "))
}

fn preserves_string(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len() + PRESERVES_STRING_QUOTE_OVERHEAD);
    rendered.push('"');
    for character in value.chars() {
        match character {
            '"' => rendered.push_str("\\\""),
            '\\' => rendered.push_str("\\\\"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            character if character.is_control() => rendered.push_str(&format!("\\u{{{:x}}}", character as u32)),
            character => rendered.push(character),
        }
    }
    rendered.push('"');
    rendered
}

fn built_binary_report_changed_fields(
    baseline: &BuiltBinaryThreadReport,
    candidate: &BuiltBinaryThreadReport,
) -> Vec<String> {
    let mut changed = Vec::new();
    push_changed_field(&mut changed, baseline.valid == candidate.valid, "valid");
    push_changed_field(&mut changed, baseline.issue_codes == candidate.issue_codes, "issue_codes");
    push_changed_field(
        &mut changed,
        binary_digest_values_equal(&baseline.binary_hash, &candidate.binary_hash),
        "binary_hash",
    );
    push_changed_field(
        &mut changed,
        sidecar_digest_values_equal(&baseline.sidecar_digest_blake3, &candidate.sidecar_digest_blake3),
        "sidecar_digest_blake3",
    );
    push_changed_field(&mut changed, baseline.source_ids == candidate.source_ids, "source_ids");
    push_changed_field(&mut changed, baseline.requirement_ids == candidate.requirement_ids, "requirement_ids");
    push_changed_field(
        &mut changed,
        baseline.artifact_identities == candidate.artifact_identities,
        "artifact_identities",
    );
    push_changed_field(&mut changed, baseline.non_claims == candidate.non_claims, "non_claims");
    changed
}

fn push_changed_field(changed: &mut Vec<String>, unchanged: bool, field: &str) {
    if !unchanged {
        changed.push(field.to_owned());
    }
}

fn binary_digest_values_equal(left: &str, right: &str) -> bool {
    match (BinaryDigest::parse(left), BinaryDigest::parse(right)) {
        (Some(left), Some(right)) => left.matches(right),
        _ => left == right,
    }
}

fn sidecar_digest_values_equal(left: &str, right: &str) -> bool {
    match (SidecarDigest::parse(left), SidecarDigest::parse(right)) {
        (Some(left), Some(right)) => left.matches(right),
        _ => left == right,
    }
}

fn artifact_digest_values_equal(left: &str, right: &str) -> bool {
    match (ArtifactDigest::parse(left), ArtifactDigest::parse(right)) {
        (Some(left), Some(right)) => left.normalized().eq_ignore_ascii_case(right.normalized()),
        _ => left == right,
    }
}

fn optional_binary_digest_values_equal(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => binary_digest_values_equal(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn optional_artifact_digest_values_equal(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => artifact_digest_values_equal(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn provenance_links_match_typed(expected: &[ProvenanceLink], actual: &[ProvenanceLink]) -> bool {
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual.iter())
            .all(|(expected, actual)| provenance_link_matches_typed(expected, actual))
}

fn provenance_link_matches_typed(expected: &ProvenanceLink, actual: &ProvenanceLink) -> bool {
    expected.source_id == actual.source_id
        && expected.function_object_id == actual.function_object_id
        && expected.requirement_ids == actual.requirement_ids
        && optional_artifact_digest_values_equal(expected.artifact_hash.as_deref(), actual.artifact_hash.as_deref())
        && optional_binary_digest_values_equal(expected.binary_hash.as_deref(), actual.binary_hash.as_deref())
}

fn built_binary_report(
    mut issue_codes: Vec<String>,
    binary_hash: String,
    sidecar_digest_blake3: String,
    octet_export: Option<&OctetValenceProvenanceExport>,
    sidecar: Option<&StackProvenanceSidecar>,
) -> BuiltBinaryThreadReport {
    issue_codes.sort();
    issue_codes.dedup();
    let source_ids = sidecar
        .map(|sidecar| {
            let mut ids = sidecar.chain.links.iter().map(|link| link.source_id.clone()).collect::<Vec<_>>();
            ids.sort();
            ids.dedup();
            ids
        })
        .unwrap_or_default();
    let requirement_ids = sidecar
        .map(|sidecar| {
            let mut ids =
                sidecar.chain.links.iter().flat_map(|link| link.requirement_ids.iter().cloned()).collect::<Vec<_>>();
            ids.sort();
            ids.dedup();
            ids
        })
        .unwrap_or_default();
    let artifact_identities = octet_export.map(|export| export.artifacts.clone()).unwrap_or_default();
    let mut report = BuiltBinaryThreadReport {
        valid: issue_codes.is_empty(),
        issue_codes,
        binary_hash,
        sidecar_digest_blake3,
        source_ids,
        requirement_ids,
        artifact_identities,
        non_claims: vec![
            BUILT_BINARY_THREAD_NON_CLAIM.to_owned(),
            STACK_PROVENANCE_NON_CLAIM.to_owned(),
        ],
        preserves_content_hash: String::new(),
    };
    report.preserves_content_hash = preserves_content_hash_for_built_binary_thread_report(&report);
    report
}

fn validate_sidecar_external_evidence(
    external_evidence: &[MantleExternalEvidenceRecord],
    sidecar_digest_blake3: &str,
) -> Vec<String> {
    let mut issues = Vec::new();
    let measured_sidecar_digest = SidecarDigest::parse(sidecar_digest_blake3);
    match external_evidence.iter().find(|record| MantleExternalEvidenceRole::parse(&record.role).is_some()) {
        Some(record) => {
            match (parse_mantle_external_evidence_boundary_values(record), measured_sidecar_digest) {
                (Ok(boundary), Some(measured_digest)) => {
                    if !boundary.digest_blake3.matches(measured_digest) {
                        issues.push("sidecar_digest_mismatch".to_owned());
                    }
                }
                (Ok(_), None) => issues.push("sidecar_digest_invalid".to_owned()),
                (Err(record_issues), _) => issues.extend(record_issues),
            }
            if !record.non_claims.iter().any(|claim| claim == STACK_PROVENANCE_NON_CLAIM) {
                issues.push("mantle_non_claim_missing".to_owned());
            }
        }
        None => issues.push("mantle_role_missing".to_owned()),
    }
    issues
}

fn validate_measured_binary_identity(
    binary_hash: &str,
    sidecar: &StackProvenanceSidecar,
    mantle_manifest: &MantleReleaseManifestView,
) -> Vec<String> {
    let mut issues = Vec::new();
    let measured_binary_hash = match BinaryDigest::parse(binary_hash) {
        Some(digest) => digest,
        None => {
            issues.push("binary_hash_invalid".to_owned());
            return issues;
        }
    };
    match BinaryDigest::parse(&sidecar.binary_hash) {
        Some(sidecar_binary_hash) if sidecar_binary_hash.matches(measured_binary_hash) => {}
        Some(_) | None => issues.push("binary_digest_mismatch".to_owned()),
    }
    for link in &sidecar.chain.links {
        match link.binary_hash.as_deref().and_then(BinaryDigest::parse) {
            Some(link_hash) if link_hash.matches(measured_binary_hash) => {}
            Some(_) | None => issues.push("provenance_link_binary_mismatch".to_owned()),
        }
    }
    if !mantle_manifest
        .binaries
        .iter()
        .filter_map(|binary| BinaryDigest::parse(&binary.digest_blake3))
        .any(|binary_digest| binary_digest.matches(measured_binary_hash))
    {
        issues.push("missing_mantle_binary_identity".to_owned());
    }
    if mantle_manifest.provenance_coverage.as_ref().is_some_and(|coverage| {
        BinaryDigest::parse(&coverage.binary_hash)
            .map(|coverage_hash| !coverage_hash.matches(measured_binary_hash))
            .unwrap_or(true)
    }) {
        issues.push("provenance_coverage_binary_mismatch".to_owned());
    }
    issues
}

fn validate_sidecar_matches_octet_export(
    export: &OctetValenceProvenanceExport,
    sidecar: &StackProvenanceSidecar,
    binary_hash: &str,
) -> Vec<String> {
    let mut issues = Vec::new();
    if sidecar.octet_export_artifact_dir != export.artifact_dir {
        issues.push("octet_export_artifact_dir_mismatch".to_owned());
    }
    if artifact_identity_set(&sidecar.parent_artifacts) != artifact_identity_set(&export.artifacts) {
        issues.push("octet_parent_artifact_mismatch".to_owned());
    }
    let expected = build_stack_provenance_sidecar(export, binary_hash);
    if !provenance_links_match_typed(&expected.chain.links, &sidecar.chain.links) {
        issues.push("sidecar_octet_link_mismatch".to_owned());
    }
    issues
}

fn validate_built_binary_claim_boundary(
    export: &OctetValenceProvenanceExport,
    sidecar: &StackProvenanceSidecar,
) -> Vec<String> {
    let overclaim_present = export
        .non_claims
        .iter()
        .chain(sidecar.non_claims.iter())
        .chain(export.proof_cards.iter().flat_map(|card| card.non_claims.iter()))
        .any(|claim| built_binary_overclaim_detected(claim));
    if overclaim_present {
        vec!["overclaim_detected".to_owned()]
    } else {
        Vec::new()
    }
}

fn built_binary_overclaim_detected(claim: &str) -> bool {
    let lower_claim = claim.to_ascii_lowercase();
    BUILT_BINARY_OVERCLAIM_PATTERNS.iter().any(|pattern| lower_claim.contains(pattern))
}

fn artifact_identity_set(artifacts: &[StackArtifactIdentity]) -> BTreeSet<(String, String)> {
    artifacts
        .iter()
        .map(|artifact| {
            let normalized_digest = ArtifactDigest::parse(&artifact.blake3)
                .map(ArtifactDigest::normalized)
                .unwrap_or(artifact.blake3.as_str());
            (artifact.artifact.clone(), normalized_digest.to_owned())
        })
        .collect()
}

fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn parse_artifact_identity_digests<'a>(
    artifacts: &'a [StackArtifactIdentity],
    issue_codes: &mut Vec<String>,
) -> Vec<ArtifactDigest<'a>> {
    let mut digests = Vec::new();
    for artifact in artifacts {
        match ArtifactDigest::parse(&artifact.blake3) {
            Some(digest) => digests.push(digest),
            None => issue_codes.push("artifact_hash_invalid".to_owned()),
        }
    }
    digests
}

fn sorted_unique(mut issue_codes: Vec<String>) -> Vec<String> {
    issue_codes.sort();
    issue_codes.dedup();
    issue_codes
}

fn provenance_link_from_octet(
    record: &OctetProvenanceRecord,
    proof_cards: &[OctetProofCard],
    binary_hash: &str,
) -> ProvenanceLink {
    ProvenanceLink {
        source_id: record.source_id.clone(),
        function_object_id: record.artifact.clone(),
        requirement_ids: requirement_ids_for_record(record, proof_cards),
        artifact_hash: Some(record.blake3.clone()),
        binary_hash: Some(binary_hash.to_owned()),
    }
}

fn requirement_ids_for_record(record: &OctetProvenanceRecord, proof_cards: &[OctetProofCard]) -> Vec<String> {
    let mut requirements = proof_cards
        .iter()
        .filter(|card| card.receipt_path == record.artifact || card.policy_id == record.source_id)
        .map(|card| card.source_requirement_or_policy_id.clone())
        .collect::<Vec<_>>();
    requirements.sort();
    requirements.dedup();
    requirements
}

fn validate_chain(chain: &ProvenanceChain, binary_hash: BinaryDigest<'_>) -> Vec<String> {
    let mut issues = validate_chain_artifact_digests(chain);
    for link in &chain.links {
        match link.binary_hash.as_deref().and_then(BinaryDigest::parse) {
            Some(link_binary_hash) if link_binary_hash.matches(binary_hash) => {}
            Some(_) | None => issues.push("binary_hash_mismatch".to_owned()),
        }
    }
    issues
}

fn validate_chain_artifact_digests(chain: &ProvenanceChain) -> Vec<String> {
    let mut issues = check_provenance_chain(chain)
        .issue_codes
        .into_iter()
        .map(|code| format!("chain_{code}"))
        .collect::<Vec<_>>();
    for link in &chain.links {
        if link.artifact_hash.as_deref().is_some_and(|hash| ArtifactDigest::parse(hash).is_none()) {
            issues.push("artifact_hash_invalid".to_owned());
        }
    }
    issues
}

fn validate_parent_artifacts(export: &OctetValenceProvenanceExport) -> Vec<String> {
    let known = export.artifacts.iter().map(|artifact| artifact.artifact.as_str()).collect::<BTreeSet<_>>();
    let mut issues = Vec::new();
    for record in &export.provenance_records {
        if record.artifact.trim().is_empty() || !known.contains(record.artifact.as_str()) {
            issues.push("parent_artifact_missing".to_owned());
        }
        for parent in &record.parent_artifacts {
            if !known.contains(parent.as_str()) {
                issues.push("parent_artifact_missing".to_owned());
            }
        }
    }
    issues
}

fn validate_evidence_tiers(export: &OctetValenceProvenanceExport) -> Vec<String> {
    let mut issues = Vec::new();
    for card in &export.proof_cards {
        let typed_tier = EvidenceTier::parse(&card.evidence_tier);
        if !card.diagnostics.is_empty() {
            issues.push("proof_card_diagnostics_present".to_owned());
        }
        if typed_tier.is_unknown() {
            issues.push("unknown_evidence_tier".to_owned());
        }
        if card.evidence_origin == EXTERNAL_TRELLIS_ORIGIN && typed_tier.is_formal_proof() {
            issues.push("manual_review_evidence_promoted".to_owned());
        }
    }
    for record in &export.provenance_records {
        let typed_tier = EvidenceTier::parse(&record.evidence_tier);
        if typed_tier.is_unknown() {
            issues.push("unknown_evidence_tier".to_owned());
        }
        if record.human_action.is_some() && typed_tier.is_formal_proof() {
            issues.push("manual_review_evidence_promoted".to_owned());
        }
        if typed_tier.is_manual_review_required() && record.human_action.as_deref().unwrap_or_default().is_empty() {
            issues.push("manual_review_action_missing".to_owned());
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    const BINARY_HASH: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const STATUS_HASH: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const RESULTS_HASH: &str = "3333333333333333333333333333333333333333333333333333333333333333";

    #[test]
    fn builds_and_verifies_minimal_release_trace() {
        let export = sample_export();
        let json = build_sidecar_json_from_octet_export_json(&serde_json::to_string(&export).unwrap(), BINARY_HASH)
            .expect("sidecar json");
        let digest = blake3::hash(json.as_bytes()).to_hex().to_string();
        let manifest = mantle_manifest(&digest);
        let report = verify_mantle_external_evidence_sidecar(&manifest, &json);

        assert!(report.valid, "unexpected issues: {:?}", report.issue_codes);
    }

    #[test]
    fn typed_binary_adapter_preserves_accepted_wire_spelling() {
        let export_json = serde_json::to_string(&sample_export()).expect("Octet export JSON");
        let prefixed_uppercase = format!("b3:{}", BINARY_HASH.to_ascii_uppercase());
        let admitted = AdmittedBinaryDigest::parse_compatible(&prefixed_uppercase, MIXED_BLAKE3_PROFILE, "binary_hash")
            .expect("compatible binary argument");
        let typed = build_sidecar_json_from_typed_binary(&export_json, &admitted).expect("typed sidecar JSON");
        let compatibility = build_sidecar_json_from_octet_export_json(&export_json, &prefixed_uppercase)
            .expect("compatibility sidecar JSON");
        let sidecar: StackProvenanceSidecar = serde_json::from_str(&typed).expect("typed sidecar projection");

        assert_eq!(typed, compatibility);
        assert_eq!(sidecar.binary_hash, prefixed_uppercase);
    }

    #[test]
    fn keeps_octet_proof_card_requirement_links() {
        let sidecar = build_stack_provenance_sidecar(&sample_export(), BINARY_HASH);
        let links = sidecar.chain.resolve_requirement("r[octet.policy.no_unwrap]");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].source_id, "octet.standards.formal-proof");
    }

    #[test]
    fn rejects_missing_mantle_role_digest_mismatch_and_binary_mismatch() {
        let mut sidecar = build_stack_provenance_sidecar(&sample_export(), BINARY_HASH);
        sidecar.chain.links[0].binary_hash = Some(STATUS_HASH.to_owned());
        let json = serde_json::to_string_pretty(&sidecar).unwrap();
        let digest = blake3::hash(b"different bytes").to_hex().to_string();
        let manifest = mantle_manifest(&digest);
        let report = verify_mantle_external_evidence_sidecar(&manifest, &json);

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"binary_hash_mismatch".to_owned()));
        assert!(report.issue_codes.contains(&"sidecar_digest_mismatch".to_owned()));

        let missing_role = r#"{"external_evidence":[]}"#;
        let missing_role_report = verify_mantle_external_evidence_sidecar(missing_role, &json);
        assert!(missing_role_report.issue_codes.contains(&"mantle_role_missing".to_owned()));
    }

    #[test]
    fn rejects_stale_parent_and_trellis_tier_promotion() {
        let mut export = sample_export();
        export.provenance_records[0].parent_artifacts.push("missing.json".to_owned());
        export.proof_cards[0].evidence_origin = EXTERNAL_TRELLIS_ORIGIN.to_owned();
        export.proof_cards[0].evidence_tier = FORMAL_PROOF_TIER.to_owned();
        let report = validate_octet_export(&export);

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"parent_artifact_missing".to_owned()));
        assert!(report.issue_codes.contains(&"manual_review_evidence_promoted".to_owned()));
    }

    #[test]
    fn rejects_malformed_octet_export() {
        let report = build_sidecar_json_from_octet_export_json("not json", BINARY_HASH).unwrap_err();

        assert!(!report.valid);
        assert_eq!(report.issue_codes, vec!["octet_export_malformed".to_owned()]);
    }

    #[test]
    fn typed_boundary_parsers_accept_valid_stack_metadata() {
        let binary_bytes = b"valence-built-binary-fixture";
        let binary_hash = blake3_hex(binary_bytes);
        let export = sample_export();
        let sidecar = build_stack_provenance_sidecar(&export, &binary_hash);
        let sidecar_json = serde_json::to_vec_pretty(&sidecar).unwrap();
        let sidecar_digest = blake3_hex(&sidecar_json);
        let manifest = mantle_release_manifest(&sidecar_digest, &binary_hash);
        let report = verify_built_binary_thread(BuiltBinaryThreadInput {
            binary_bytes,
            octet_export: &export,
            sidecar: &sidecar,
            sidecar_bytes: &sidecar_json,
            mantle_manifest: &manifest,
        });

        let export_values = parse_octet_export_boundary_values(&export).expect("typed octet export");
        assert_eq!(export_values.schema_version, OctetValenceExportSchema::V1);
        assert_eq!(export_values.verification_status, VerificationStatus::Valid);
        assert_eq!(export_values.provenance_evidence_tiers, vec![EvidenceTier::FormalProof]);
        assert_eq!(export_values.proof_card_statuses, vec![VerificationStatus::Valid]);

        let sidecar_values = parse_stack_sidecar_boundary_values(&sidecar).expect("typed sidecar");
        assert_eq!(sidecar_values.schema, StackProvenanceSidecarSchema::V1);
        assert_eq!(sidecar_values.claim_scope, StackProvenanceClaimScope::IdentityLinkageSidecar);
        assert!(sidecar_values.binary_hash.matches(BinaryDigest::parse(&binary_hash).unwrap()));

        let manifest_values = parse_mantle_release_manifest_boundary_values(&manifest).expect("typed manifest");
        assert_eq!(
            manifest_values.stack_sidecar_external_evidence.role,
            MantleExternalEvidenceRole::StackProvenanceTrace
        );
        assert!(
            manifest_values
                .binary_digests
                .iter()
                .any(|digest| digest.matches(BinaryDigest::parse(&binary_hash).unwrap()))
        );

        let report_values = parse_built_binary_thread_report_boundary_values(&report).expect("typed report");
        assert!(report_values.binary_hash.matches(BinaryDigest::parse(&report.binary_hash).unwrap()));
        assert!(
            report_values
                .sidecar_digest_blake3
                .matches(SidecarDigest::parse(&report.sidecar_digest_blake3).unwrap())
        );
        assert!(report_values.preserves_content_hash.is_some());
    }

    #[test]
    fn typed_boundary_parsers_reject_invalid_strings() {
        let mut export = sample_export();
        export.schema_version = "octet-valence-provenance-export/v0".to_owned();
        export.verification_status = "warning".to_owned();
        export.artifacts[0].blake3 = "not-a-blake3-digest".to_owned();
        export.provenance_records[0].evidence_tier = "experimental_machine_review".to_owned();
        export.proof_cards[0].status = "stale".to_owned();
        let report = validate_octet_export(&export);

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"artifact_hash_invalid".to_owned()));
        assert!(report.issue_codes.contains(&"octet_schema_mismatch".to_owned()));
        assert!(report.issue_codes.contains(&"octet_verification_not_valid".to_owned()));
        assert!(report.issue_codes.contains(&"proof_card_status_invalid".to_owned()));
        assert!(report.issue_codes.contains(&"unknown_evidence_tier".to_owned()));

        let mut sidecar = build_stack_provenance_sidecar(&sample_export(), BINARY_HASH);
        sidecar.schema = "valence.stack-provenance-sidecar.v0".to_owned();
        sidecar.claim_scope = "correctness-proof".to_owned();
        sidecar.binary_hash = "short".to_owned();
        let sidecar_issues = parse_stack_sidecar_boundary_values(&sidecar).unwrap_err();
        assert!(sidecar_issues.contains(&"binary_hash_invalid".to_owned()));
        assert!(sidecar_issues.contains(&"claim_scope_mismatch".to_owned()));
        assert!(sidecar_issues.contains(&"schema_mismatch".to_owned()));

        let bad_record = MantleExternalEvidenceRecord {
            role: "other-role".to_owned(),
            schema: "valence.stack-provenance-sidecar.v0".to_owned(),
            relative_path: "external-evidence/stack-provenance.json".to_owned(),
            digest_blake3: "not-a-digest".to_owned(),
            claim_scope: "correctness-proof".to_owned(),
            non_claims: vec![STACK_PROVENANCE_NON_CLAIM.to_owned()],
        };
        let mantle_issues = parse_mantle_external_evidence_boundary_values(&bad_record).unwrap_err();
        assert!(mantle_issues.contains(&"mantle_role_missing".to_owned()));
        assert!(mantle_issues.contains(&"mantle_schema_mismatch".to_owned()));
        assert!(mantle_issues.contains(&"sidecar_digest_invalid".to_owned()));
        assert!(mantle_issues.contains(&"mantle_claim_scope_mismatch".to_owned()));
    }

    #[test]
    fn unknown_evidence_tier_is_preserved_but_conservative() {
        let mut export = sample_export();
        export.provenance_records[0].evidence_tier = "experimental_machine_review".to_owned();
        let report = validate_octet_export(&export);
        let evidence = preserves_evidence_for_octet_export(&export);

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"unknown_evidence_tier".to_owned()));
        assert!(!report.issue_codes.contains(&"manual_review_evidence_promoted".to_owned()));
        assert!(evidence.preserves_value.contains("experimental_machine_review"));
    }

    #[test]
    fn typed_digest_roles_accept_prefixed_uppercase_hashes_without_json_shape_changes() {
        let binary_bytes = b"valence-built-binary-fixture";
        let binary_hash = blake3_hex(binary_bytes);
        let prefixed_binary_hash = format!("b3:{}", binary_hash.to_ascii_uppercase());
        let export = sample_export();
        let mut sidecar = build_stack_provenance_sidecar(&export, &binary_hash);
        sidecar.binary_hash = prefixed_binary_hash.clone();
        for link in &mut sidecar.chain.links {
            link.binary_hash = Some(prefixed_binary_hash.clone());
        }
        let sidecar_json = serde_json::to_vec_pretty(&sidecar).unwrap();
        let sidecar_digest = blake3_hex(&sidecar_json);
        let prefixed_sidecar_digest = format!("b3:{}", sidecar_digest.to_ascii_uppercase());
        let manifest = mantle_release_manifest(&prefixed_sidecar_digest, &prefixed_binary_hash);
        let report = verify_built_binary_thread(BuiltBinaryThreadInput {
            binary_bytes,
            octet_export: &export,
            sidecar: &sidecar,
            sidecar_bytes: &sidecar_json,
            mantle_manifest: &manifest,
        });

        assert!(report.valid, "unexpected issues: {:?}", report.issue_codes);
        assert_eq!(sidecar.binary_hash, prefixed_binary_hash);
    }

    #[test]
    fn built_binary_thread_accepts_measured_binary_identity() {
        let binary_bytes = b"valence-built-binary-fixture";
        let binary_hash = blake3_hex(binary_bytes);
        let export = sample_export();
        let sidecar = build_stack_provenance_sidecar(&export, &binary_hash);
        let sidecar_json = serde_json::to_vec_pretty(&sidecar).unwrap();
        let sidecar_digest = blake3_hex(&sidecar_json);
        let manifest = mantle_release_manifest(&sidecar_digest, &binary_hash);
        let report = verify_built_binary_thread(BuiltBinaryThreadInput {
            binary_bytes,
            octet_export: &export,
            sidecar: &sidecar,
            sidecar_bytes: &sidecar_json,
            mantle_manifest: &manifest,
        });

        assert!(report.valid, "unexpected issues: {:?}", report.issue_codes);
        assert_eq!(report.binary_hash, binary_hash);
        assert_eq!(report.source_ids, vec!["octet.standards.formal-proof".to_owned()]);
        assert!(report.non_claims.contains(&BUILT_BINARY_THREAD_NON_CLAIM.to_owned()));
    }

    // r[verify valence.stack_provenance_adapter.cross_repo_fixtures.negative]
    #[test]
    fn built_binary_thread_rejects_stale_bytes_and_missing_binary_identity() {
        let binary_bytes = b"valence-built-binary-fixture";
        let stale_binary_bytes = b"valence-built-binary-fixture-tampered";
        let binary_hash = blake3_hex(binary_bytes);
        let export = sample_export();
        let sidecar = build_stack_provenance_sidecar(&export, &binary_hash);
        let sidecar_json = serde_json::to_vec_pretty(&sidecar).unwrap();
        let sidecar_digest = blake3_hex(&sidecar_json);
        let manifest = mantle_release_manifest(&sidecar_digest, &binary_hash);
        let report = verify_built_binary_thread(BuiltBinaryThreadInput {
            binary_bytes: stale_binary_bytes,
            octet_export: &export,
            sidecar: &sidecar,
            sidecar_bytes: &sidecar_json,
            mantle_manifest: &manifest,
        });

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"binary_digest_mismatch".to_owned()));
        assert!(report.issue_codes.contains(&"provenance_link_binary_mismatch".to_owned()));
        assert!(report.issue_codes.contains(&"missing_mantle_binary_identity".to_owned()));
    }

    // r[verify valence.stack_provenance_adapter.cross_repo_fixtures.negative]
    #[test]
    fn built_binary_thread_rejects_stale_sidecar_digest_and_octet_mismatch() {
        let binary_bytes = b"valence-built-binary-fixture";
        let binary_hash = blake3_hex(binary_bytes);
        let mut export = sample_export();
        let sidecar = build_stack_provenance_sidecar(&export, &binary_hash);
        export.artifacts[0].blake3 = RESULTS_HASH.to_owned();
        export.provenance_records[0].blake3 = RESULTS_HASH.to_owned();
        let sidecar_json = serde_json::to_vec_pretty(&sidecar).unwrap();
        let manifest = mantle_release_manifest(STATUS_HASH, &binary_hash);
        let report = verify_built_binary_thread(BuiltBinaryThreadInput {
            binary_bytes,
            octet_export: &export,
            sidecar: &sidecar,
            sidecar_bytes: &sidecar_json,
            mantle_manifest: &manifest,
        });

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"sidecar_digest_mismatch".to_owned()));
        assert!(report.issue_codes.contains(&"octet_parent_artifact_mismatch".to_owned()));
        assert!(report.issue_codes.contains(&"sidecar_octet_link_mismatch".to_owned()));
    }

    // r[verify valence.stack_provenance_adapter.cross_repo_fixtures.negative]
    #[test]
    fn built_binary_thread_rejects_missing_parent_tier_promotion_and_overclaim() {
        let binary_bytes = b"valence-built-binary-fixture";
        let binary_hash = blake3_hex(binary_bytes);
        let mut export = sample_export();
        export.provenance_records[0].parent_artifacts.push("missing-parent.json".to_owned());
        export.proof_cards[0].evidence_origin = EXTERNAL_TRELLIS_ORIGIN.to_owned();
        export.proof_cards[0].evidence_tier = FORMAL_PROOF_TIER.to_owned();
        export.non_claims.push("proves semantic equivalence".to_owned());
        let sidecar = build_stack_provenance_sidecar(&export, &binary_hash);
        let sidecar_json = serde_json::to_vec_pretty(&sidecar).unwrap();
        let sidecar_digest = blake3_hex(&sidecar_json);
        let manifest = mantle_release_manifest(&sidecar_digest, &binary_hash);
        let report = verify_built_binary_thread(BuiltBinaryThreadInput {
            binary_bytes,
            octet_export: &export,
            sidecar: &sidecar,
            sidecar_bytes: &sidecar_json,
            mantle_manifest: &manifest,
        });

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"parent_artifact_missing".to_owned()));
        assert!(report.issue_codes.contains(&"manual_review_evidence_promoted".to_owned()));
        assert!(report.issue_codes.contains(&"overclaim_detected".to_owned()));
    }

    #[test]
    fn built_binary_thread_json_rejects_malformed_manifest() {
        let report = verify_built_binary_thread_json(b"binary", "not json", "not json", "not json");

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"octet_export_malformed".to_owned()));
    }

    #[derive(Debug, Deserialize)]
    struct PreservesCorpusFixture {
        schema: String,
        entries: Vec<StackProvenancePreservesEvidence>,
        drift_cases: Vec<PreservesDriftCaseFixture>,
    }

    #[derive(Debug, Deserialize)]
    struct PreservesDriftCaseFixture {
        id: String,
        baseline_report: BuiltBinaryThreadReport,
        candidate_report: BuiltBinaryThreadReport,
        expected_diff: StackProvenancePreservesDiff,
    }

    #[test]
    fn stack_provenance_preserves_corpus_is_fresh() {
        let corpus = preserves_corpus_fixture();
        assert_eq!(corpus.schema, "valence.stack-provenance-preserves-corpus.v1");
        assert_eq!(corpus.entries, sample_preserves_corpus_entries());
        for entry in &corpus.entries {
            assert_eq!(entry.preserves_encoding, STACK_PROVENANCE_PRESERVES_ENCODING);
            assert_eq!(entry.preserves_content_hash, blake3_hex(entry.preserves_value.as_bytes()));
            assert!(entry.non_claims.contains(&PRESERVES_METADATA_NON_CLAIM.to_owned()));
        }
    }

    #[test]
    fn stack_provenance_preserves_report_diff_fixture_is_fresh() {
        let corpus = preserves_corpus_fixture();
        assert_eq!(corpus.drift_cases.len(), 1);
        let case = &corpus.drift_cases[0];
        assert_eq!(case.id, "report-diagnostic-change");
        let actual_diff =
            compare_built_binary_thread_reports_by_preserves(&case.baseline_report, &case.candidate_report);
        assert_eq!(actual_diff, case.expected_diff);
        assert!(!actual_diff.equal);
        assert_eq!(actual_diff.changed_fields, vec!["valid".to_owned(), "issue_codes".to_owned()]);
    }

    #[test]
    fn stack_provenance_preserves_report_diff_ignores_json_field_order() {
        let report = sample_built_binary_thread_report();
        let reordered = serde_json::json!({
            "valid": report.valid,
            "source_ids": report.source_ids.clone(),
            "sidecar_digest_blake3": report.sidecar_digest_blake3.clone(),
            "requirement_ids": report.requirement_ids.clone(),
            "preserves_content_hash": report.preserves_content_hash.clone(),
            "non_claims": report.non_claims.clone(),
            "issue_codes": report.issue_codes.clone(),
            "binary_hash": report.binary_hash.clone(),
            "artifact_identities": report.artifact_identities.clone(),
        });
        let parsed: BuiltBinaryThreadReport = serde_json::from_value(reordered).unwrap();
        let diff = compare_built_binary_thread_reports_by_preserves(&report, &parsed);
        assert!(diff.equal, "unexpected diff: {diff:?}");
        assert!(diff.changed_fields.is_empty());
    }

    fn preserves_corpus_fixture() -> PreservesCorpusFixture {
        serde_json::from_str(include_str!("../../../contracts/stack-provenance/preserves-corpus/corpus.json"))
            .expect("parse preserves corpus")
    }

    fn sample_preserves_corpus_entries() -> Vec<StackProvenancePreservesEvidence> {
        let binary_bytes = b"valence-built-binary-fixture";
        let binary_hash = blake3_hex(binary_bytes);
        let export = sample_export();
        let sidecar = build_stack_provenance_sidecar(&export, &binary_hash);
        let sidecar_json = serde_json::to_vec_pretty(&sidecar).unwrap();
        let sidecar_digest = blake3_hex(&sidecar_json);
        let manifest = mantle_release_manifest(&sidecar_digest, &binary_hash);
        let input = BuiltBinaryThreadInput {
            binary_bytes,
            octet_export: &export,
            sidecar: &sidecar,
            sidecar_bytes: &sidecar_json,
            mantle_manifest: &manifest,
        };
        let input_evidence = preserves_evidence_for_built_binary_thread_input(&input);
        let report = verify_built_binary_thread(input);
        vec![
            preserves_evidence_for_octet_export(&export),
            preserves_evidence_for_stack_sidecar(&sidecar),
            preserves_evidence_for_mantle_manifest(&manifest),
            input_evidence,
            preserves_evidence_for_built_binary_thread_report(&report),
        ]
    }

    fn sample_built_binary_thread_report() -> BuiltBinaryThreadReport {
        let binary_bytes = b"valence-built-binary-fixture";
        let binary_hash = blake3_hex(binary_bytes);
        let export = sample_export();
        let sidecar = build_stack_provenance_sidecar(&export, &binary_hash);
        let sidecar_json = serde_json::to_vec_pretty(&sidecar).unwrap();
        let sidecar_digest = blake3_hex(&sidecar_json);
        let manifest = mantle_release_manifest(&sidecar_digest, &binary_hash);
        verify_built_binary_thread(BuiltBinaryThreadInput {
            binary_bytes,
            octet_export: &export,
            sidecar: &sidecar,
            sidecar_bytes: &sidecar_json,
            mantle_manifest: &manifest,
        })
    }

    fn sample_export() -> OctetValenceProvenanceExport {
        OctetValenceProvenanceExport {
            schema_version: OCTET_VALENCE_EXPORT_SCHEMA.to_owned(),
            artifact_dir: "target/octet-artifact".to_owned(),
            verification_status: VALID_VERIFICATION_STATUS.to_owned(),
            artifacts: vec![
                StackArtifactIdentity {
                    artifact: "status.json".to_owned(),
                    blake3: STATUS_HASH.to_owned(),
                },
                StackArtifactIdentity {
                    artifact: "results.jsonl".to_owned(),
                    blake3: RESULTS_HASH.to_owned(),
                },
            ],
            provenance_records: vec![OctetProvenanceRecord {
                source_id: "octet.standards.formal-proof".to_owned(),
                checker_id: "octet-artifact-verifier".to_owned(),
                evidence_tier: FORMAL_PROOF_TIER.to_owned(),
                artifact: "status.json".to_owned(),
                blake3: STATUS_HASH.to_owned(),
                checked_scope: "standards/proof-claims/catalog.ncl".to_owned(),
                verifier_identity: Some("octet-verus-rail".to_owned()),
                proof_artifact_identity: Some(RESULTS_HASH.to_owned()),
                human_action: None,
                parent_artifacts: vec!["results.jsonl".to_owned()],
            }],
            proof_cards: vec![OctetProofCard {
                proof_or_property_id: "octet.no-unwrap-proof".to_owned(),
                source_requirement_or_policy_id: "r[octet.policy.no_unwrap]".to_owned(),
                policy_id: "octet.standards.formal-proof".to_owned(),
                rail_index: 0,
                receipt_path: "status.json".to_owned(),
                artifact_blake3_identities: vec![StackArtifactIdentity {
                    artifact: "status.json".to_owned(),
                    blake3: STATUS_HASH.to_owned(),
                }],
                evidence_tier: FORMAL_PROOF_TIER.to_owned(),
                evidence_origin: "octet_verified_receipt".to_owned(),
                status: VALID_VERIFICATION_STATUS.to_owned(),
                non_claims: vec![STACK_PROVENANCE_NON_CLAIM.to_owned()],
                diagnostics: Vec::new(),
            }],
            diagnostics: Vec::new(),
            non_claims: vec![STACK_PROVENANCE_NON_CLAIM.to_owned()],
        }
    }

    fn mantle_manifest(digest: &str) -> String {
        serde_json::json!({
            "external_evidence": [
                {
                    "role": MANTLE_STACK_PROVENANCE_ROLE,
                    "schema": STACK_PROVENANCE_SIDECAR_SCHEMA,
                    "relative_path": "external-evidence/stack-provenance.json",
                    "digest_blake3": digest,
                    "claim_scope": STACK_PROVENANCE_CLAIM_SCOPE,
                    "non_claims": [STACK_PROVENANCE_NON_CLAIM]
                }
            ]
        })
        .to_string()
    }

    fn mantle_release_manifest(sidecar_digest: &str, binary_hash: &str) -> MantleReleaseManifestView {
        serde_json::from_value(serde_json::json!({
            "binaries": [
                {
                    "relative_path": "bin/valence-fixture",
                    "digest_blake3": binary_hash
                }
            ],
            "external_evidence": [
                {
                    "role": MANTLE_STACK_PROVENANCE_ROLE,
                    "schema": STACK_PROVENANCE_SIDECAR_SCHEMA,
                    "relative_path": "external-evidence/stack-provenance.json",
                    "digest_blake3": sidecar_digest,
                    "claim_scope": STACK_PROVENANCE_CLAIM_SCOPE,
                    "non_claims": [STACK_PROVENANCE_NON_CLAIM]
                }
            ],
            "provenance_coverage": {
                "binary_hash": binary_hash
            }
        }))
        .unwrap()
    }
}
