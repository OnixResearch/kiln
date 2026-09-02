//! Function-address Evidence IR profile.
//!
//! Valence validates identity domains, evidence roles, spans, graph edges, and
//! non-claim boundaries for Octet/Kamacite function-address evidence. It does
//! not parse Rust or claim semantic correctness.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use crate::evidence_ir::EvidenceArtifact;
use crate::evidence_ir::EvidenceIrError;
use crate::evidence_ir::EvidenceSpan;
use crate::evidence_ir::VerificationRole;
use crate::evidence_ir::build_evidence_artifact;
use crate::shared_identity::ArtifactIdentityDigest;
use crate::shared_identity::BinaryDigest;
use crate::shared_identity::CompatibilityDigest;
use crate::shared_identity::DigestProfile;
use crate::shared_identity::GraphDigest;
use crate::shared_identity::MIXED_BLAKE3_PROFILE;
use crate::shared_identity::PolicyDigest;
use crate::shared_identity::RAW_BLAKE3_PROFILE;
use crate::shared_identity::ReceiptDigest;
use crate::shared_identity::STRICT_B3_PROFILE;
use crate::shared_identity::SidecarDigest;
use crate::shared_identity::SourceDigest;
use crate::shared_identity::blake3_hex;
use crate::shared_identity::canonical_json_bytes;

pub const FUNCTION_ADDRESS_PROFILE_VERSION: &str = "valence.function-address-evidence.v1";
pub const FUNCTION_ADDRESS_ARTIFACT_KIND: &str = "function-address-evidence-v1";
pub const FUNCTION_ADDRESS_GRAPH_SCHEMA: &str = "valence.function-address-graph.v1";
pub const FUNCTION_ADDRESS_BUNDLE_SCHEMA: &str = "valence.function-address-publication.v1";
pub const FUNCTION_ADDRESS_COMPATIBILITY_RECEIPT_SCHEMA: &str = "valence.function-address-compatibility-receipt.v1";
pub const FUNCTION_ADDRESS_COMPATIBILITY_GRAPH_SCHEMA: &str = "valence.function-address-compatibility-graph.v1";
pub const FUNCTION_ADDRESS_PUBLICATION_NON_CLAIM: &str = "function-address publication proves validated identity and linkage coherence only, not Rust semantics, correctness, proof status, or release eligibility";
const REQUIRED_NON_CLAIM_SOURCE_SHAPE: &str = "function-address evidence records source-shape identity only";
const REQUIRED_NON_CLAIM_NO_SEMANTICS: &str = "not Rust semantic correctness";
const REQUIRED_NON_CLAIM_NO_RELEASE: &str = "not release eligibility";
const REQUIRED_NON_CLAIMS: &[&str] = &[
    REQUIRED_NON_CLAIM_SOURCE_SHAPE,
    REQUIRED_NON_CLAIM_NO_SEMANTICS,
    REQUIRED_NON_CLAIM_NO_RELEASE,
];
const FORBIDDEN_CLAIM_MARKERS: &[&str] = &[
    "semantic correctness",
    "semantic equivalence",
    "whole-program safety",
    "verifier soundness",
    "release eligibility",
    "formal proof",
];
const EDGE_DEPENDS_ON: &str = "function-depends-on";
const EDGE_BACKED_BY_KAMACITE: &str = "backed-by-kamacite-receipt";
const EDGE_PROJECTED_BY_VALENCE: &str = "projected-by-valence";
const EDGE_BOUND_TO_MANTLE_SIDECAR: &str = "bound-to-mantle-sidecar";
const EDGE_BUNDLE_LOCAL_RELEASE_CONTEXT: &str = "bundle-local-release-context";
const POLICY_MATERIAL_KIND: &str = "valence-function-address-profile-policy-material-v1";
const OCTET_ROLE: &str = "octet-rust-source-shape-producer";
const KAMACITE_ROLE: &str = "kamacite-portable-receipt";
const VALENCE_PROJECTION_NODE: &str = "valence-function-address-projection";
const MANTLE_SIDECAR_NODE: &str = "mantle-release-sidecar";
const MANTLE_RELEASE_BINARY_NODE: &str = "mantle-release-binary";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FunctionAddressDigestDomain {
    SourceEnvelope,
    FunctionAddress,
    DependencyClosure,
    KamaciteReceipt,
    ValenceProjection,
    SidecarDigest,
    ReleaseBinary,
    OctetProducerReceipt,
    PolicyProfile,
}

impl FunctionAddressDigestDomain {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SourceEnvelope => "source-envelope",
            Self::FunctionAddress => "function-address",
            Self::DependencyClosure => "dependency-closure",
            Self::KamaciteReceipt => "kamacite-receipt",
            Self::ValenceProjection => "valence-projection",
            Self::SidecarDigest => "sidecar-digest",
            Self::ReleaseBinary => "release-binary",
            Self::OctetProducerReceipt => "octet-producer-receipt",
            Self::PolicyProfile => "policy-profile",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionAddressDigest {
    pub domain: FunctionAddressDigestDomain,
    pub digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FunctionAddressDigestBoundary {
    OctetB3,
    KamaciteBare,
    MantleBare,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionAddressDigestSpellingInput {
    pub boundary: FunctionAddressDigestBoundary,
    pub domain: FunctionAddressDigestDomain,
    pub expected_domain: Option<FunctionAddressDigestDomain>,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FunctionAddressDigestSpellingReport {
    pub valid: bool,
    pub normalized: Option<FunctionAddressDigest>,
    pub canonical_spelling: Option<String>,
    pub issues: Vec<FunctionAddressEvidenceIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FunctionAddressPolicyMaterial {
    pub material_kind: &'static str,
    pub profile_version: &'static str,
    pub artifact_kind: &'static str,
    pub accepted_digest_domains: Vec<&'static str>,
    pub accepted_roles: Vec<&'static str>,
    pub required_non_claims: Vec<&'static str>,
    pub graph_edge_kinds: Vec<&'static str>,
    pub boundary: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FunctionAddressEvidenceRole {
    OctetProducer,
    KamaciteReceipt,
    ValenceAdapter,
}

impl FunctionAddressEvidenceRole {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OctetProducer => OCTET_ROLE,
            Self::KamaciteReceipt => KAMACITE_ROLE,
            Self::ValenceAdapter => "valence-evidence-linkage-adapter",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionAddressEvidenceRecord {
    pub locator: String,
    pub role: FunctionAddressEvidenceRole,
    pub function_address: FunctionAddressDigest,
    pub source_identity: FunctionAddressDigest,
    pub dependency_closure: Option<FunctionAddressDigest>,
    pub source_span: Option<EvidenceSpan>,
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionAddressEvidenceEdge {
    pub from_locator: String,
    pub to_locator: String,
    pub edge_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionAddressEvidenceInput {
    pub profile_version: String,
    pub stable_id: String,
    pub verification_role: VerificationRole,
    pub records: Vec<FunctionAddressEvidenceRecord>,
    pub dependency_edges: Vec<FunctionAddressEvidenceEdge>,
    pub declared_roles: Vec<String>,
    pub octet_producer_receipt_identity: Option<FunctionAddressDigest>,
    pub kamacite_receipt_identity: Option<FunctionAddressDigest>,
    pub supplied_kamacite_receipt_identity: Option<FunctionAddressDigest>,
    pub valence_projection_identity: Option<FunctionAddressDigest>,
    pub mantle_sidecar_digest: Option<FunctionAddressDigest>,
    pub release_binary_identity: Option<FunctionAddressDigest>,
    pub policy_identity: Option<FunctionAddressDigest>,
    pub semantic_claims: Vec<String>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FunctionAddressEvidenceIssue {
    pub field: &'static str,
    pub code: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionAddressEvidenceReport {
    pub valid: bool,
    pub issues: Vec<FunctionAddressEvidenceIssue>,
    pub artifact: Option<EvidenceArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionAddressGraphRow {
    pub from: String,
    pub to: String,
    pub edge_kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionAddressGraphReport {
    pub schema: &'static str,
    pub policy_identity: FunctionAddressDigest,
    pub artifact_identity: String,
    pub valid: bool,
    pub row_count: usize,
    pub dependency_edge_count: usize,
    pub rows: Vec<FunctionAddressGraphRow>,
    pub non_claims: Vec<String>,
    pub graph_identity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionAddressPublicationBundle {
    pub schema: &'static str,
    pub publication_identity: String,
    pub valid: bool,
    pub receipt: EvidenceArtifact,
    pub graph: FunctionAddressGraphReport,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionAddressCompatibilityReceipt {
    pub schema: &'static str,
    pub publication_identity: String,
    pub policy_identity: FunctionAddressDigest,
    pub artifact_identity: String,
    pub valid: bool,
    pub receipt: EvidenceArtifact,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionAddressCompatibilityGraph {
    pub schema: &'static str,
    pub publication_identity: String,
    pub policy_identity: FunctionAddressDigest,
    pub artifact_identity: String,
    pub valid: bool,
    pub graph: FunctionAddressGraphReport,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionAddressPublication {
    pub bundle: FunctionAddressPublicationBundle,
    pub compatibility_receipt: FunctionAddressCompatibilityReceipt,
    pub compatibility_graph: FunctionAddressCompatibilityGraph,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionAddressPublicationDecision {
    pub valid: bool,
    pub issues: Vec<FunctionAddressEvidenceIssue>,
    pub publication: Option<FunctionAddressPublication>,
}

#[derive(Serialize)]
struct FunctionAddressGraphHashMaterial<'a> {
    schema: &'static str,
    policy_identity: &'a FunctionAddressDigest,
    artifact_identity: &'a str,
    valid: bool,
    row_count: usize,
    dependency_edge_count: usize,
    rows: &'a [FunctionAddressGraphRow],
    non_claims: &'a [String],
}

#[derive(Serialize)]
struct FunctionAddressPublicationHashMaterial<'a> {
    schema: &'static str,
    policy_identity: &'a FunctionAddressDigest,
    artifact_identity: &'a str,
    graph_identity: &'a str,
    valid: bool,
    non_claims: &'a [String],
}

#[derive(Debug, Clone, Serialize)]
struct FunctionAddressEvidencePayload<'a> {
    profile_version: &'a str,
    records: &'a [FunctionAddressEvidenceRecord],
    dependency_edges: &'a [FunctionAddressEvidenceEdge],
    declared_roles: &'a [String],
    octet_producer_receipt_identity: &'a Option<FunctionAddressDigest>,
    kamacite_receipt_identity: &'a Option<FunctionAddressDigest>,
    supplied_kamacite_receipt_identity: &'a Option<FunctionAddressDigest>,
    valence_projection_identity: &'a Option<FunctionAddressDigest>,
    mantle_sidecar_digest: &'a Option<FunctionAddressDigest>,
    release_binary_identity: &'a Option<FunctionAddressDigest>,
    policy_identity: &'a Option<FunctionAddressDigest>,
    evidence_roles: Vec<&'static str>,
}

#[must_use]
pub fn function_address_policy_material() -> FunctionAddressPolicyMaterial {
    let mut accepted_digest_domains = vec![
        FunctionAddressDigestDomain::SourceEnvelope.as_str(),
        FunctionAddressDigestDomain::FunctionAddress.as_str(),
        FunctionAddressDigestDomain::DependencyClosure.as_str(),
        FunctionAddressDigestDomain::KamaciteReceipt.as_str(),
        FunctionAddressDigestDomain::ValenceProjection.as_str(),
        FunctionAddressDigestDomain::SidecarDigest.as_str(),
        FunctionAddressDigestDomain::ReleaseBinary.as_str(),
        FunctionAddressDigestDomain::OctetProducerReceipt.as_str(),
        FunctionAddressDigestDomain::PolicyProfile.as_str(),
    ];
    accepted_digest_domains.sort();
    let mut accepted_roles = vec![
        OCTET_ROLE,
        KAMACITE_ROLE,
        FunctionAddressEvidenceRole::ValenceAdapter.as_str(),
    ];
    accepted_roles.sort();
    let mut graph_edge_kinds = vec![
        EDGE_DEPENDS_ON,
        EDGE_BACKED_BY_KAMACITE,
        EDGE_PROJECTED_BY_VALENCE,
        EDGE_BOUND_TO_MANTLE_SIDECAR,
        EDGE_BUNDLE_LOCAL_RELEASE_CONTEXT,
    ];
    graph_edge_kinds.sort();
    FunctionAddressPolicyMaterial {
        material_kind: POLICY_MATERIAL_KIND,
        profile_version: FUNCTION_ADDRESS_PROFILE_VERSION,
        artifact_kind: FUNCTION_ADDRESS_ARTIFACT_KIND,
        accepted_digest_domains,
        accepted_roles,
        required_non_claims: REQUIRED_NON_CLAIMS.to_vec(),
        graph_edge_kinds,
        boundary: "recorded-only source-shape function-address evidence; no Rust semantics, verifier soundness, release eligibility, or formal-proof promotion",
    }
}

#[must_use]
pub fn function_address_policy_identity() -> FunctionAddressDigest {
    let material = function_address_policy_material();
    let value = serde_json::to_value(&material).expect("function-address policy material serializes");
    let bytes = canonical_json_bytes(&value).expect("function-address policy material canonicalizes");
    let raw_digest = blake3_hex(&bytes);
    let digest = PolicyDigest::parse(&raw_digest, RAW_BLAKE3_PROFILE, "policy_identity")
        .expect("BLAKE3 output is a valid policy digest");
    FunctionAddressDigest {
        domain: FunctionAddressDigestDomain::PolicyProfile,
        digest: digest.as_hex().to_owned(),
    }
}

#[must_use]
pub fn normalize_function_address_digest_spelling(
    input: &FunctionAddressDigestSpellingInput,
) -> FunctionAddressDigestSpellingReport {
    let parsed = CompatibilityDigest::parse(&input.digest, digest_boundary_profile(input.boundary), "digest");
    let mut issues = parsed
        .as_ref()
        .err()
        .map(|issue| FunctionAddressEvidenceIssue::new(issue.field, issue.code))
        .into_iter()
        .collect::<Vec<_>>();
    if let Some(expected_domain) = input.expected_domain
        && expected_domain != input.domain
    {
        issues.push(FunctionAddressEvidenceIssue::new("domain", "domain_substitution"));
    }
    issues.sort_by_key(|issue| (issue.field, issue.code));
    issues.dedup();
    let normalized = parsed.ok().map(|digest| FunctionAddressDigest {
        domain: input.domain,
        digest: digest.as_hex().to_owned(),
    });
    let canonical_spelling = normalized.as_ref().map(|digest| format!("b3:{}", digest.digest));
    FunctionAddressDigestSpellingReport {
        valid: issues.is_empty() && normalized.is_some(),
        normalized,
        canonical_spelling,
        issues,
    }
}

fn digest_boundary_profile(boundary: FunctionAddressDigestBoundary) -> DigestProfile {
    match boundary {
        FunctionAddressDigestBoundary::OctetB3 => STRICT_B3_PROFILE,
        FunctionAddressDigestBoundary::KamaciteBare | FunctionAddressDigestBoundary::MantleBare => RAW_BLAKE3_PROFILE,
    }
}

#[must_use]
pub fn validate_function_address_evidence(input: &FunctionAddressEvidenceInput) -> FunctionAddressEvidenceReport {
    let mut issues = Vec::new();
    validate_profile_version(input, &mut issues);
    validate_verification_role(input, &mut issues);
    validate_non_claims(input, &mut issues);
    validate_claims(input, &mut issues);
    validate_declared_roles(input, &mut issues);
    validate_records(input, &mut issues);
    validate_edges(input, &mut issues);
    validate_external_identities(input, &mut issues);
    validate_policy_identity(input, &mut issues);
    issues.sort_by_key(|issue| (issue.field, issue.code));
    issues.dedup();
    if !issues.is_empty() {
        return FunctionAddressEvidenceReport {
            valid: false,
            issues,
            artifact: None,
        };
    }
    match build_function_address_artifact(input) {
        Ok(artifact) => FunctionAddressEvidenceReport {
            valid: true,
            issues,
            artifact: Some(artifact),
        },
        Err(_error) => FunctionAddressEvidenceReport {
            valid: false,
            issues: vec![FunctionAddressEvidenceIssue::new(
                "artifact",
                "canonical_serialization_failed",
            )],
            artifact: None,
        },
    }
}

pub fn build_function_address_artifact(
    input: &FunctionAddressEvidenceInput,
) -> Result<EvidenceArtifact, EvidenceIrError> {
    let payload = FunctionAddressEvidencePayload {
        profile_version: &input.profile_version,
        records: &input.records,
        dependency_edges: &input.dependency_edges,
        declared_roles: &input.declared_roles,
        octet_producer_receipt_identity: &input.octet_producer_receipt_identity,
        kamacite_receipt_identity: &input.kamacite_receipt_identity,
        supplied_kamacite_receipt_identity: &input.supplied_kamacite_receipt_identity,
        valence_projection_identity: &input.valence_projection_identity,
        mantle_sidecar_digest: &input.mantle_sidecar_digest,
        release_binary_identity: &input.release_binary_identity,
        policy_identity: &input.policy_identity,
        evidence_roles: evidence_roles(input),
    };
    let payload = serde_json::to_value(payload).map_err(|error| EvidenceIrError::CanonicalSerialization {
        detail: error.to_string(),
    })?;
    let source_span = input.records.iter().find_map(|record| record.source_span.clone());
    build_evidence_artifact(
        FUNCTION_ADDRESS_ARTIFACT_KIND,
        &input.stable_id,
        source_span,
        input.verification_role,
        payload,
        input.non_claims.clone(),
    )
}

#[must_use]
pub fn function_address_graph_rows(input: &FunctionAddressEvidenceInput) -> Vec<FunctionAddressGraphRow> {
    let mut rows = input
        .dependency_edges
        .iter()
        .map(|edge| FunctionAddressGraphRow {
            from: edge.from_locator.clone(),
            to: edge.to_locator.clone(),
            edge_kind: edge.edge_kind.clone(),
        })
        .collect::<Vec<_>>();
    if input.kamacite_receipt_identity.is_some() {
        for record in &input.records {
            rows.push(FunctionAddressGraphRow {
                from: record.locator.clone(),
                to: KAMACITE_ROLE.to_string(),
                edge_kind: EDGE_BACKED_BY_KAMACITE.to_string(),
            });
        }
    }
    if input.valence_projection_identity.is_some() {
        for record in &input.records {
            rows.push(FunctionAddressGraphRow {
                from: record.locator.clone(),
                to: VALENCE_PROJECTION_NODE.to_string(),
                edge_kind: EDGE_PROJECTED_BY_VALENCE.to_string(),
            });
        }
    }
    if input.mantle_sidecar_digest.is_some() {
        for record in &input.records {
            rows.push(FunctionAddressGraphRow {
                from: record.locator.clone(),
                to: MANTLE_SIDECAR_NODE.to_string(),
                edge_kind: EDGE_BOUND_TO_MANTLE_SIDECAR.to_string(),
            });
        }
    }
    if input.release_binary_identity.is_some() {
        rows.push(FunctionAddressGraphRow {
            from: MANTLE_SIDECAR_NODE.to_string(),
            to: MANTLE_RELEASE_BINARY_NODE.to_string(),
            edge_kind: EDGE_BUNDLE_LOCAL_RELEASE_CONTEXT.to_string(),
        });
    }
    rows.sort_by(|left, right| {
        left.from.cmp(&right.from).then(left.to.cmp(&right.to)).then(left.edge_kind.cmp(&right.edge_kind))
    });
    rows
}

/// Plan publishable output only from a passing typed validation report.
///
/// r[impl valence.evidence_ir.function_address_publication.validity_gate]
/// r[impl valence.evidence_ir.function_address_publication.canonical_graph]
#[must_use]
pub fn plan_function_address_publication(input: &FunctionAddressEvidenceInput) -> FunctionAddressPublicationDecision {
    let validation = validate_function_address_evidence(input);
    if !validation.valid {
        return FunctionAddressPublicationDecision {
            valid: false,
            issues: validation.issues,
            publication: None,
        };
    }
    let Some(artifact) = validation.artifact else {
        return publication_failure("artifact", "validated_artifact_missing");
    };
    let policy_identity = function_address_policy_identity();
    let rows = function_address_graph_rows(input);
    let dependency_edge_count = rows.iter().filter(|row| row.edge_kind == EDGE_DEPENDS_ON).count();
    let mut non_claims = input.non_claims.clone();
    non_claims.push(FUNCTION_ADDRESS_PUBLICATION_NON_CLAIM.to_owned());
    non_claims.sort();
    non_claims.dedup();
    let graph_material = FunctionAddressGraphHashMaterial {
        schema: FUNCTION_ADDRESS_GRAPH_SCHEMA,
        policy_identity: &policy_identity,
        artifact_identity: &artifact.canonical_hash,
        valid: true,
        row_count: rows.len(),
        dependency_edge_count,
        rows: &rows,
        non_claims: &non_claims,
    };
    let Some(graph_identity_text) = hash_serializable(&graph_material) else {
        return publication_failure("graph", "canonical_serialization_failed");
    };
    let Ok(graph_identity) = GraphDigest::parse(&graph_identity_text, RAW_BLAKE3_PROFILE, "graph_identity") else {
        return publication_failure("graph", "generated_graph_identity_invalid");
    };
    let graph = FunctionAddressGraphReport {
        schema: FUNCTION_ADDRESS_GRAPH_SCHEMA,
        policy_identity: policy_identity.clone(),
        artifact_identity: artifact.canonical_hash.clone(),
        valid: true,
        row_count: rows.len(),
        dependency_edge_count,
        rows,
        non_claims: non_claims.clone(),
        graph_identity: graph_identity.as_hex().to_owned(),
    };
    let publication_material = FunctionAddressPublicationHashMaterial {
        schema: FUNCTION_ADDRESS_BUNDLE_SCHEMA,
        policy_identity: &policy_identity,
        artifact_identity: &artifact.canonical_hash,
        graph_identity: &graph.graph_identity,
        valid: true,
        non_claims: &non_claims,
    };
    let Some(publication_identity) = hash_serializable(&publication_material) else {
        return publication_failure("publication", "canonical_serialization_failed");
    };
    let bundle = FunctionAddressPublicationBundle {
        schema: FUNCTION_ADDRESS_BUNDLE_SCHEMA,
        publication_identity: publication_identity.clone(),
        valid: true,
        receipt: artifact.clone(),
        graph: graph.clone(),
        non_claims: non_claims.clone(),
    };
    let compatibility_receipt = FunctionAddressCompatibilityReceipt {
        schema: FUNCTION_ADDRESS_COMPATIBILITY_RECEIPT_SCHEMA,
        publication_identity: publication_identity.clone(),
        policy_identity: policy_identity.clone(),
        artifact_identity: artifact.canonical_hash.clone(),
        valid: true,
        receipt: artifact,
        non_claims: non_claims.clone(),
    };
    let compatibility_graph = FunctionAddressCompatibilityGraph {
        schema: FUNCTION_ADDRESS_COMPATIBILITY_GRAPH_SCHEMA,
        publication_identity,
        policy_identity,
        artifact_identity: graph.artifact_identity.clone(),
        valid: true,
        graph,
        non_claims,
    };
    FunctionAddressPublicationDecision {
        valid: true,
        issues: Vec::new(),
        publication: Some(FunctionAddressPublication {
            bundle,
            compatibility_receipt,
            compatibility_graph,
        }),
    }
}

fn publication_failure(field: &'static str, code: &'static str) -> FunctionAddressPublicationDecision {
    FunctionAddressPublicationDecision {
        valid: false,
        issues: vec![FunctionAddressEvidenceIssue::new(field, code)],
        publication: None,
    }
}

fn hash_serializable<T: Serialize>(value: &T) -> Option<String> {
    let value = serde_json::to_value(value).ok()?;
    canonical_json_bytes(&value).ok().map(|bytes| blake3_hex(&bytes))
}

impl FunctionAddressEvidenceIssue {
    #[must_use]
    pub const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

fn validate_profile_version(input: &FunctionAddressEvidenceInput, issues: &mut Vec<FunctionAddressEvidenceIssue>) {
    if input.profile_version != FUNCTION_ADDRESS_PROFILE_VERSION {
        issues.push(FunctionAddressEvidenceIssue::new("profile_version", "unsupported_profile_version"));
    }
    if input.stable_id.is_empty() {
        issues.push(FunctionAddressEvidenceIssue::new("stable_id", "missing_stable_id"));
    }
}

fn validate_verification_role(input: &FunctionAddressEvidenceInput, issues: &mut Vec<FunctionAddressEvidenceIssue>) {
    if input.verification_role != VerificationRole::RecordedOnly {
        issues.push(FunctionAddressEvidenceIssue::new("verification_role", "formal_proof_promotion_denied"));
    }
}

fn validate_non_claims(input: &FunctionAddressEvidenceInput, issues: &mut Vec<FunctionAddressEvidenceIssue>) {
    for required in REQUIRED_NON_CLAIMS {
        if !input.non_claims.iter().any(|claim| claim == required) {
            issues.push(FunctionAddressEvidenceIssue::new("non_claims", "missing_non_claim"));
        }
    }
}

fn validate_claims(input: &FunctionAddressEvidenceInput, issues: &mut Vec<FunctionAddressEvidenceIssue>) {
    for claim in &input.semantic_claims {
        let lowered = claim.to_ascii_lowercase();
        if FORBIDDEN_CLAIM_MARKERS.iter().any(|marker| lowered.contains(marker)) {
            issues.push(FunctionAddressEvidenceIssue::new("semantic_claims", "overclaim"));
        }
    }
    for claim in &input.non_claims {
        let lowered = claim.to_ascii_lowercase();
        if lowered.contains("proves ") {
            issues.push(FunctionAddressEvidenceIssue::new("non_claims", "weakened_non_claim"));
        }
    }
}

fn validate_declared_roles(input: &FunctionAddressEvidenceInput, issues: &mut Vec<FunctionAddressEvidenceIssue>) {
    for role in &input.declared_roles {
        if !known_role(role) {
            issues.push(FunctionAddressEvidenceIssue::new("declared_roles", "unknown_role"));
        }
    }
}

fn validate_records(input: &FunctionAddressEvidenceInput, issues: &mut Vec<FunctionAddressEvidenceIssue>) {
    if input.records.is_empty() {
        issues.push(FunctionAddressEvidenceIssue::new("records", "missing_records"));
        return;
    }
    let mut locators = BTreeSet::new();
    for record in &input.records {
        if !locators.insert(record.locator.clone()) {
            issues.push(FunctionAddressEvidenceIssue::new("records.locator", "duplicate_locator"));
        }
        validate_digest_field(
            &record.function_address,
            FunctionAddressDigestDomain::FunctionAddress,
            "records.function_address",
            issues,
        );
        validate_digest_field(
            &record.source_identity,
            FunctionAddressDigestDomain::SourceEnvelope,
            "records.source_identity",
            issues,
        );
        if let Some(dependency_closure) = &record.dependency_closure {
            validate_digest_field(
                dependency_closure,
                FunctionAddressDigestDomain::DependencyClosure,
                "records.dependency_closure",
                issues,
            );
        }
        let Some(span) = &record.source_span else {
            issues.push(FunctionAddressEvidenceIssue::new("records.source_span", "missing_source_span"));
            continue;
        };
        if span.source_id != record.source_identity.digest {
            issues.push(FunctionAddressEvidenceIssue::new("records.source_span", "stale_span_source_identity"));
        }
    }
}

fn validate_edges(input: &FunctionAddressEvidenceInput, issues: &mut Vec<FunctionAddressEvidenceIssue>) {
    let locators = input.records.iter().map(|record| record.locator.as_str()).collect::<BTreeSet<_>>();
    let mut edges = BTreeSet::new();
    for edge in &input.dependency_edges {
        if edge.from_locator.trim().is_empty() || edge.to_locator.trim().is_empty() {
            issues.push(FunctionAddressEvidenceIssue::new("dependency_edges.locator", "empty_locator"));
        }
        if !edges.insert((&edge.from_locator, &edge.to_locator, &edge.edge_kind)) {
            issues.push(FunctionAddressEvidenceIssue::new("dependency_edges", "duplicate_edge"));
        }
        if edge.edge_kind != EDGE_DEPENDS_ON {
            issues.push(FunctionAddressEvidenceIssue::new("dependency_edges.edge_kind", "unknown_edge_kind"));
        }
        if !locators.contains(edge.from_locator.as_str()) {
            issues.push(FunctionAddressEvidenceIssue::new("dependency_edges.from_locator", "missing_locator"));
        }
        if !locators.contains(edge.to_locator.as_str()) {
            issues.push(FunctionAddressEvidenceIssue::new("dependency_edges.to_locator", "missing_locator"));
        }
    }
}

fn validate_external_identities(input: &FunctionAddressEvidenceInput, issues: &mut Vec<FunctionAddressEvidenceIssue>) {
    validate_optional_digest_field(
        &input.octet_producer_receipt_identity,
        FunctionAddressDigestDomain::OctetProducerReceipt,
        "octet_producer_receipt_identity",
        issues,
    );
    validate_optional_digest_field(
        &input.kamacite_receipt_identity,
        FunctionAddressDigestDomain::KamaciteReceipt,
        "kamacite_receipt_identity",
        issues,
    );
    validate_optional_digest_field(
        &input.supplied_kamacite_receipt_identity,
        FunctionAddressDigestDomain::KamaciteReceipt,
        "supplied_kamacite_receipt_identity",
        issues,
    );
    validate_optional_digest_field(
        &input.valence_projection_identity,
        FunctionAddressDigestDomain::ValenceProjection,
        "valence_projection_identity",
        issues,
    );
    validate_optional_digest_field(
        &input.mantle_sidecar_digest,
        FunctionAddressDigestDomain::SidecarDigest,
        "mantle_sidecar_digest",
        issues,
    );
    validate_optional_digest_field(
        &input.release_binary_identity,
        FunctionAddressDigestDomain::ReleaseBinary,
        "release_binary_identity",
        issues,
    );
    if let (Some(declared), Some(supplied)) =
        (&input.kamacite_receipt_identity, &input.supplied_kamacite_receipt_identity)
        && declared.digest != supplied.digest
    {
        issues.push(FunctionAddressEvidenceIssue::new("kamacite_receipt_identity", "stale_receipt_identity"));
    }
}

fn validate_policy_identity(input: &FunctionAddressEvidenceInput, issues: &mut Vec<FunctionAddressEvidenceIssue>) {
    let expected = function_address_policy_identity();
    let Some(policy_identity) = &input.policy_identity else {
        issues.push(FunctionAddressEvidenceIssue::new("policy_identity", "missing_policy_identity"));
        return;
    };
    if policy_identity.domain != FunctionAddressDigestDomain::PolicyProfile {
        issues.push(FunctionAddressEvidenceIssue::new("policy_identity", "identity_domain_substitution"));
        return;
    }
    if PolicyDigest::parse(&policy_identity.digest, MIXED_BLAKE3_PROFILE, "digest").is_err() {
        issues.push(FunctionAddressEvidenceIssue::new("policy_identity", "malformed_policy_identity"));
        return;
    }
    if policy_identity.digest != expected.digest {
        issues.push(FunctionAddressEvidenceIssue::new("policy_identity", "stale_policy_identity"));
    }
}

fn validate_optional_digest_field(
    identity: &Option<FunctionAddressDigest>,
    expected: FunctionAddressDigestDomain,
    field: &'static str,
    issues: &mut Vec<FunctionAddressEvidenceIssue>,
) {
    if let Some(identity) = identity {
        validate_digest_field(identity, expected, field, issues);
    }
}

fn validate_digest_field(
    identity: &FunctionAddressDigest,
    expected: FunctionAddressDigestDomain,
    field: &'static str,
    issues: &mut Vec<FunctionAddressEvidenceIssue>,
) {
    if identity.domain != expected {
        issues.push(FunctionAddressEvidenceIssue::new(field, "identity_domain_substitution"));
        return;
    }
    if !digest_matches_domain(&identity.digest, expected) {
        issues.push(FunctionAddressEvidenceIssue::new(field, "malformed_digest"));
    }
}

fn digest_matches_domain(digest: &str, domain: FunctionAddressDigestDomain) -> bool {
    match domain {
        FunctionAddressDigestDomain::SourceEnvelope => {
            SourceDigest::parse(digest, MIXED_BLAKE3_PROFILE, "digest").is_ok()
        }
        FunctionAddressDigestDomain::SidecarDigest => {
            SidecarDigest::parse(digest, MIXED_BLAKE3_PROFILE, "digest").is_ok()
        }
        FunctionAddressDigestDomain::ReleaseBinary => {
            BinaryDigest::parse(digest, MIXED_BLAKE3_PROFILE, "digest").is_ok()
        }
        FunctionAddressDigestDomain::KamaciteReceipt | FunctionAddressDigestDomain::OctetProducerReceipt => {
            ReceiptDigest::parse(digest, MIXED_BLAKE3_PROFILE, "digest").is_ok()
        }
        FunctionAddressDigestDomain::PolicyProfile => {
            PolicyDigest::parse(digest, MIXED_BLAKE3_PROFILE, "digest").is_ok()
        }
        FunctionAddressDigestDomain::FunctionAddress
        | FunctionAddressDigestDomain::DependencyClosure
        | FunctionAddressDigestDomain::ValenceProjection => {
            ArtifactIdentityDigest::parse(digest, MIXED_BLAKE3_PROFILE, "digest").is_ok()
        }
    }
}

fn evidence_roles(input: &FunctionAddressEvidenceInput) -> Vec<&'static str> {
    let mut roles = input.records.iter().map(|record| record.role.as_str()).collect::<Vec<_>>();
    if input.kamacite_receipt_identity.is_some() {
        roles.push(KAMACITE_ROLE);
    }
    roles.sort();
    roles.dedup();
    roles
}

fn known_role(role: &str) -> bool {
    role == OCTET_ROLE || role == KAMACITE_ROLE || role == FunctionAddressEvidenceRole::ValenceAdapter.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_DIGEST: &str = "b3:1111111111111111111111111111111111111111111111111111111111111111";
    const FUNCTION_DIGEST: &str = "b3:2222222222222222222222222222222222222222222222222222222222222222";
    const FUNCTION_TWO_DIGEST: &str = "b3:3333333333333333333333333333333333333333333333333333333333333333";
    const DEPENDENCY_DIGEST: &str = "b3:4444444444444444444444444444444444444444444444444444444444444444";
    const KAMACITE_DIGEST: &str = "b3:5555555555555555555555555555555555555555555555555555555555555555";
    const OCTET_RECEIPT_DIGEST: &str = "b3:6666666666666666666666666666666666666666666666666666666666666666";
    const VALENCE_PROJECTION_DIGEST: &str = "b3:7777777777777777777777777777777777777777777777777777777777777777";
    const MANTLE_SIDECAR_DIGEST: &str = "b3:8888888888888888888888888888888888888888888888888888888888888888";
    const RELEASE_BINARY_DIGEST: &str = "b3:9999999999999999999999999999999999999999999999999999999999999999";
    const STALE_KAMACITE_DIGEST: &str = "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const RAW_KAMACITE_DIGEST: &str = "5555555555555555555555555555555555555555555555555555555555555555";
    const RAW_MANTLE_DIGEST: &str = "8888888888888888888888888888888888888888888888888888888888888888";
    const MALFORMED_DIGEST: &str = "b3:abc";
    const WRONG_PREFIX_DIGEST: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    const STABLE_ID: &str = "function-address-profile-fixture";
    const FUNCTION_LOCATOR: &str = "src/lib.rs::f";
    const FUNCTION_TWO_LOCATOR: &str = "src/lib.rs::g";
    const START_BYTE: u64 = 0;
    const END_BYTE: u64 = 12;
    const START_LINE: u32 = 1;
    const START_COLUMN: u32 = 1;
    const END_LINE: u32 = 1;
    const END_COLUMN: u32 = 13;

    fn digest(domain: FunctionAddressDigestDomain, value: &str) -> FunctionAddressDigest {
        FunctionAddressDigest {
            domain,
            digest: value.to_string(),
        }
    }

    fn valid_input(with_kamacite: bool) -> FunctionAddressEvidenceInput {
        FunctionAddressEvidenceInput {
            profile_version: FUNCTION_ADDRESS_PROFILE_VERSION.to_string(),
            stable_id: STABLE_ID.to_string(),
            verification_role: VerificationRole::RecordedOnly,
            records: vec![
                FunctionAddressEvidenceRecord {
                    locator: FUNCTION_LOCATOR.to_string(),
                    role: FunctionAddressEvidenceRole::OctetProducer,
                    function_address: digest(FunctionAddressDigestDomain::FunctionAddress, FUNCTION_DIGEST),
                    source_identity: digest(FunctionAddressDigestDomain::SourceEnvelope, SOURCE_DIGEST),
                    dependency_closure: Some(digest(FunctionAddressDigestDomain::DependencyClosure, DEPENDENCY_DIGEST)),
                    source_span: Some(EvidenceSpan {
                        source_id: SOURCE_DIGEST.to_string(),
                        source_path: "src/lib.rs".to_string(),
                        start_byte: START_BYTE,
                        end_byte: END_BYTE,
                        start_line: START_LINE,
                        start_column: START_COLUMN,
                        end_line: END_LINE,
                        end_column: END_COLUMN,
                        label: Some(FUNCTION_LOCATOR.to_string()),
                    }),
                    caveats: Vec::new(),
                },
                FunctionAddressEvidenceRecord {
                    locator: FUNCTION_TWO_LOCATOR.to_string(),
                    role: FunctionAddressEvidenceRole::OctetProducer,
                    function_address: digest(FunctionAddressDigestDomain::FunctionAddress, FUNCTION_TWO_DIGEST),
                    source_identity: digest(FunctionAddressDigestDomain::SourceEnvelope, SOURCE_DIGEST),
                    dependency_closure: None,
                    source_span: Some(EvidenceSpan {
                        source_id: SOURCE_DIGEST.to_string(),
                        source_path: "src/lib.rs".to_string(),
                        start_byte: START_BYTE,
                        end_byte: END_BYTE,
                        start_line: START_LINE,
                        start_column: START_COLUMN,
                        end_line: END_LINE,
                        end_column: END_COLUMN,
                        label: Some(FUNCTION_TWO_LOCATOR.to_string()),
                    }),
                    caveats: vec!["dependency closure absent".to_string()],
                },
            ],
            dependency_edges: vec![FunctionAddressEvidenceEdge {
                from_locator: FUNCTION_TWO_LOCATOR.to_string(),
                to_locator: FUNCTION_LOCATOR.to_string(),
                edge_kind: EDGE_DEPENDS_ON.to_string(),
            }],
            declared_roles: vec![
                OCTET_ROLE.to_string(),
                FunctionAddressEvidenceRole::ValenceAdapter.as_str().to_string(),
            ],
            octet_producer_receipt_identity: Some(digest(
                FunctionAddressDigestDomain::OctetProducerReceipt,
                OCTET_RECEIPT_DIGEST,
            )),
            kamacite_receipt_identity: with_kamacite
                .then(|| digest(FunctionAddressDigestDomain::KamaciteReceipt, KAMACITE_DIGEST)),
            supplied_kamacite_receipt_identity: with_kamacite
                .then(|| digest(FunctionAddressDigestDomain::KamaciteReceipt, KAMACITE_DIGEST)),
            valence_projection_identity: Some(digest(
                FunctionAddressDigestDomain::ValenceProjection,
                VALENCE_PROJECTION_DIGEST,
            )),
            mantle_sidecar_digest: with_kamacite
                .then(|| digest(FunctionAddressDigestDomain::SidecarDigest, MANTLE_SIDECAR_DIGEST)),
            release_binary_identity: with_kamacite
                .then(|| digest(FunctionAddressDigestDomain::ReleaseBinary, RELEASE_BINARY_DIGEST)),
            policy_identity: Some(function_address_policy_identity()),
            semantic_claims: Vec::new(),
            non_claims: REQUIRED_NON_CLAIMS.iter().map(|claim| (*claim).to_string()).collect(),
        }
    }

    #[test]
    fn policy_identity_is_deterministic_and_embedded_in_valid_artifacts() {
        let material = function_address_policy_material();
        let identity = function_address_policy_identity();
        let input = valid_input(true);
        let report = validate_function_address_evidence(&input);

        assert_eq!(material.profile_version, FUNCTION_ADDRESS_PROFILE_VERSION);
        assert_eq!(identity.domain, FunctionAddressDigestDomain::PolicyProfile);
        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert_eq!(input.policy_identity.as_ref(), Some(&identity));
    }

    #[test]
    fn policy_identity_rejects_missing_malformed_stale_and_substituted_values() {
        let mut missing = valid_input(false);
        missing.policy_identity = None;
        let mut malformed = valid_input(false);
        malformed.policy_identity = Some(digest(FunctionAddressDigestDomain::PolicyProfile, "b3:abc"));
        let mut stale = valid_input(false);
        stale.policy_identity = Some(digest(
            FunctionAddressDigestDomain::PolicyProfile,
            "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ));
        let mut substituted = valid_input(false);
        substituted.policy_identity =
            Some(digest(FunctionAddressDigestDomain::ValenceProjection, &function_address_policy_identity().digest));

        let missing_report = validate_function_address_evidence(&missing);
        let malformed_report = validate_function_address_evidence(&malformed);
        let stale_report = validate_function_address_evidence(&stale);
        let substituted_report = validate_function_address_evidence(&substituted);

        assert!(missing_report.issues.iter().any(|issue| issue.code == "missing_policy_identity"));
        assert!(malformed_report.issues.iter().any(|issue| issue.code == "malformed_policy_identity"));
        assert!(stale_report.issues.iter().any(|issue| issue.code == "stale_policy_identity"));
        assert!(substituted_report.issues.iter().any(|issue| issue.code == "identity_domain_substitution"));
    }

    #[test]
    fn digest_spelling_adapters_normalize_octet_kamacite_and_mantle_boundaries() {
        let octet = normalize_function_address_digest_spelling(&FunctionAddressDigestSpellingInput {
            boundary: FunctionAddressDigestBoundary::OctetB3,
            domain: FunctionAddressDigestDomain::FunctionAddress,
            expected_domain: Some(FunctionAddressDigestDomain::FunctionAddress),
            digest: FUNCTION_DIGEST.to_string(),
        });
        let kamacite = normalize_function_address_digest_spelling(&FunctionAddressDigestSpellingInput {
            boundary: FunctionAddressDigestBoundary::KamaciteBare,
            domain: FunctionAddressDigestDomain::KamaciteReceipt,
            expected_domain: Some(FunctionAddressDigestDomain::KamaciteReceipt),
            digest: RAW_KAMACITE_DIGEST.to_string(),
        });
        let mantle = normalize_function_address_digest_spelling(&FunctionAddressDigestSpellingInput {
            boundary: FunctionAddressDigestBoundary::MantleBare,
            domain: FunctionAddressDigestDomain::SidecarDigest,
            expected_domain: Some(FunctionAddressDigestDomain::SidecarDigest),
            digest: RAW_MANTLE_DIGEST.to_string(),
        });

        assert!(octet.valid, "unexpected issues: {:?}", octet.issues);
        assert!(kamacite.valid, "unexpected issues: {:?}", kamacite.issues);
        assert!(mantle.valid, "unexpected issues: {:?}", mantle.issues);
        assert_eq!(octet.normalized.as_ref().expect("normalized").domain, FunctionAddressDigestDomain::FunctionAddress);
        assert_eq!(kamacite.canonical_spelling.as_deref(), Some(KAMACITE_DIGEST));
        assert_eq!(mantle.canonical_spelling.as_deref(), Some(MANTLE_SIDECAR_DIGEST));
    }

    #[test]
    fn digest_spelling_adapters_reject_malformed_prefixes_and_cross_domain_substitution() {
        let malformed = normalize_function_address_digest_spelling(&FunctionAddressDigestSpellingInput {
            boundary: FunctionAddressDigestBoundary::OctetB3,
            domain: FunctionAddressDigestDomain::FunctionAddress,
            expected_domain: Some(FunctionAddressDigestDomain::FunctionAddress),
            digest: MALFORMED_DIGEST.to_string(),
        });
        let wrong_prefix = normalize_function_address_digest_spelling(&FunctionAddressDigestSpellingInput {
            boundary: FunctionAddressDigestBoundary::OctetB3,
            domain: FunctionAddressDigestDomain::FunctionAddress,
            expected_domain: Some(FunctionAddressDigestDomain::FunctionAddress),
            digest: WRONG_PREFIX_DIGEST.to_string(),
        });
        let substituted = normalize_function_address_digest_spelling(&FunctionAddressDigestSpellingInput {
            boundary: FunctionAddressDigestBoundary::KamaciteBare,
            domain: FunctionAddressDigestDomain::SourceEnvelope,
            expected_domain: Some(FunctionAddressDigestDomain::KamaciteReceipt),
            digest: RAW_KAMACITE_DIGEST.to_string(),
        });

        assert!(!malformed.valid);
        assert!(malformed.issues.iter().any(|issue| issue.code == "malformed_blake3"));
        assert!(!wrong_prefix.valid);
        assert!(wrong_prefix.issues.iter().any(|issue| issue.code == "unsupported_digest_spelling"));
        assert!(!substituted.valid);
        assert!(substituted.issues.iter().any(|issue| issue.code == "domain_substitution"));
    }

    #[test]
    fn octet_only_profile_builds_recorded_only_evidence_artifact() {
        let input = valid_input(false);

        let report = validate_function_address_evidence(&input);

        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        let artifact = report.artifact.expect("artifact");
        assert_eq!(artifact.artifact_kind, FUNCTION_ADDRESS_ARTIFACT_KIND);
        assert_eq!(artifact.verification_role, VerificationRole::RecordedOnly);
        assert!(artifact.non_claims.iter().any(|claim| claim == REQUIRED_NON_CLAIM_NO_SEMANTICS));
    }

    #[test]
    fn octet_plus_kamacite_profile_exports_graph_rows() {
        let input = valid_input(true);

        let report = validate_function_address_evidence(&input);
        let rows = function_address_graph_rows(&input);

        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert!(rows.iter().any(|row| row.edge_kind == EDGE_DEPENDS_ON));
        assert!(rows.iter().any(|row| row.edge_kind == EDGE_BACKED_BY_KAMACITE));
        assert!(rows.iter().any(|row| row.edge_kind == EDGE_BOUND_TO_MANTLE_SIDECAR));
        assert!(rows.iter().any(|row| row.edge_kind == EDGE_BUNDLE_LOCAL_RELEASE_CONTEXT));
    }

    #[test]
    fn rejects_profile_version_roles_domains_and_overclaims() {
        let mut input = valid_input(true);
        input.profile_version = "valence.function-address-evidence.v0".to_string();
        input.verification_role = VerificationRole::Property;
        input.records[0].function_address.domain = FunctionAddressDigestDomain::SourceEnvelope;
        input.declared_roles.push("unknown-role".to_string());
        input.semantic_claims.push("proves semantic correctness".to_string());
        input.non_claims.clear();

        let report = validate_function_address_evidence(&input);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "unsupported_profile_version"));
        assert!(report.issues.iter().any(|issue| issue.code == "formal_proof_promotion_denied"));
        assert!(report.issues.iter().any(|issue| issue.code == "identity_domain_substitution"));
        assert!(report.issues.iter().any(|issue| issue.code == "unknown_role"));
        assert!(report.issues.iter().any(|issue| issue.code == "missing_non_claim"));
        assert!(report.issues.iter().any(|issue| issue.code == "overclaim"));
    }

    #[test]
    fn rejects_duplicate_locator_stale_span_and_unknown_edge() {
        let mut input = valid_input(false);
        input.records[1].locator = FUNCTION_LOCATOR.to_string();
        input.records[0].source_span.as_mut().expect("span").source_id = FUNCTION_TWO_DIGEST.to_string();
        input.dependency_edges[0].edge_kind = "semantic-equivalence".to_string();
        input.dependency_edges[0].to_locator = "missing".to_string();

        let report = validate_function_address_evidence(&input);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "duplicate_locator"));
        assert!(report.issues.iter().any(|issue| issue.code == "stale_span_source_identity"));
        assert!(report.issues.iter().any(|issue| issue.code == "unknown_edge_kind"));
        assert!(report.issues.iter().any(|issue| issue.code == "missing_locator"));
    }

    #[test]
    fn rejects_malformed_digest_text() {
        let mut input = valid_input(false);
        input.records[0].function_address.digest = "b3:abc".to_string();

        let report = validate_function_address_evidence(&input);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "malformed_digest"));
    }

    #[test]
    fn publication_is_validated_order_stable_and_identity_bound() {
        // r[verify valence.evidence_ir.function_address_publication.fixtures.positive]
        let input = valid_input(true);
        let mut reordered = input.clone();
        reordered.dependency_edges.reverse();

        let first = plan_function_address_publication(&input);
        let second = plan_function_address_publication(&reordered);
        let first_publication = first.publication.expect("valid publication");
        let second_publication = second.publication.expect("valid publication");

        assert!(first.valid);
        assert_eq!(first_publication.bundle.publication_identity, second_publication.bundle.publication_identity,);
        assert_eq!(first_publication.bundle.graph.graph_identity, second_publication.bundle.graph.graph_identity);
        assert_eq!(
            first_publication.compatibility_receipt.publication_identity,
            first_publication.compatibility_graph.publication_identity,
        );
        assert!(first_publication.bundle.valid);
        assert!(first_publication.bundle.graph.valid);
    }

    #[test]
    fn invalid_duplicate_and_empty_edges_never_produce_publication() {
        // r[verify valence.evidence_ir.function_address_publication.fixtures.negative]
        let mut promoted = valid_input(true);
        promoted.verification_role = VerificationRole::Property;
        let promoted_decision = plan_function_address_publication(&promoted);
        assert!(!promoted_decision.valid);
        assert!(promoted_decision.publication.is_none());

        let mut duplicate = valid_input(false);
        duplicate.dependency_edges.push(duplicate.dependency_edges[0].clone());
        let duplicate_decision = plan_function_address_publication(&duplicate);
        assert!(duplicate_decision.issues.iter().any(|issue| issue.code == "duplicate_edge"));
        assert!(duplicate_decision.publication.is_none());

        let mut empty = valid_input(false);
        empty.dependency_edges[0].from_locator.clear();
        let empty_decision = plan_function_address_publication(&empty);
        assert!(empty_decision.issues.iter().any(|issue| issue.code == "empty_locator"));
        assert!(empty_decision.publication.is_none());
    }

    #[test]
    fn rejects_stale_receipt_identity_and_missing_span() {
        let mut input = valid_input(true);
        input.supplied_kamacite_receipt_identity =
            Some(digest(FunctionAddressDigestDomain::KamaciteReceipt, STALE_KAMACITE_DIGEST));
        input.records[0].source_span = None;

        let report = validate_function_address_evidence(&input);

        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "stale_receipt_identity"));
        assert!(report.issues.iter().any(|issue| issue.code == "missing_source_span"));
    }
}
