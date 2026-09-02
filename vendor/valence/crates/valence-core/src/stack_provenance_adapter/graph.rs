use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;

use super::STACK_PROVENANCE_NON_CLAIM;
use super::blake3_hex;

pub const STACK_PROVENANCE_GRAPH_INPUT_SCHEMA: &str = "valence.stack-provenance-graph-input.v1";
pub const STACK_PROVENANCE_GRAPH_REPORT_SCHEMA: &str = "valence.stack-provenance-graph-report.v1";
pub const STACK_PROVENANCE_GRAPH_NON_CLAIM: &str = "stack provenance graph verifies identity, linkage, role, digest, and non-claim policy only; it does not prove behavioral correctness, verifier soundness, release eligibility, or requirement satisfaction";
pub const STACK_PROVENANCE_GRAPH_SUPPORTED_CLAIM: &str =
    "identity, linkage, role, digest, and bounded lifecycle-policy conformance only";
pub const CAIRN_GATE_CLAIM_SCOPE: &str = "stack_provenance.v1.lifecycle_policy_conformance";
pub const IDENTITY_LINKAGE_CLAIM_SCOPE: &str = "stack_provenance.v1.identity_linkage_only";
pub const MANUAL_REVIEW_CLAIM_SCOPE: &str = "stack_provenance.v1.manual_review_input";

const ARTIFACT_KIND_REQUIREMENT: &str = "requirement";
const ARTIFACT_KIND_SPAN: &str = "span";
const ARTIFACT_KIND_RECEIPT: &str = "receipt";
const ARTIFACT_KIND_PROOF_ARTIFACT: &str = "proof_artifact";
const ARTIFACT_KIND_SIDECAR: &str = "sidecar";
const ARTIFACT_KIND_RELEASE_BUNDLE: &str = "release_bundle";
const ARTIFACT_KIND_LIFECYCLE_GATE: &str = "lifecycle_gate";
const EDGE_KIND_PROVES_LOCAL_PROPERTY: &str = "proves_local_property";
const EDGE_KIND_CHECKS_SOURCE_SHAPE: &str = "checks_source_shape";
const EDGE_KIND_EXPORTS_PROVENANCE: &str = "exports_provenance";
const EDGE_KIND_BUNDLED_INTO_RELEASE: &str = "bundled_into_release";
const EDGE_KIND_SATISFIES_GATE: &str = "satisfies_gate";
const EDGE_KIND_MANUAL_REVIEW_INPUT: &str = "manual_review_input";
const PRODUCER_ROLE_TRELLIS: &str = "trellis";
const PRODUCER_ROLE_OCTET: &str = "octet";
const PRODUCER_ROLE_VALENCE: &str = "valence";
const PRODUCER_ROLE_MANTLE: &str = "mantle";
const PRODUCER_ROLE_CAIRN: &str = "cairn";
const PRODUCER_ROLE_MANUAL_REVIEW: &str = "manual_review";
const VERIFICATION_ROLE_PROPERTY: &str = "property";
const VERIFICATION_ROLE_RECORDED_ONLY: &str = "recorded_only";
const VERIFICATION_ROLE_BOUNDARY: &str = "boundary";
const EVIDENCE_TIER_FORMAL_PROOF: &str = "formal_proof";
const EVIDENCE_TIER_SOURCE_SHAPE_CHECK: &str = "source_shape_check";
const EVIDENCE_TIER_PROVENANCE_EXPORT: &str = "provenance_export";
const EVIDENCE_TIER_RELEASE_BUNDLE: &str = "release_bundle";
const EVIDENCE_TIER_LIFECYCLE_GATE: &str = "lifecycle_gate";
const EVIDENCE_TIER_MANUAL_REVIEW_REQUIRED: &str = "manual_review_required";
const GRAPH_ARTIFACT_ID_PREFIX: &str = "b3:";
const PAYLOAD_STATED_SOURCE_HASH_FIELD: &str = "stated_source_hash";
const PAYLOAD_SOURCE_CONTENT_HASH_FIELD: &str = "source_content_hash";
const REPORT_HASH_PLACEHOLDER: &str = "";
const MANTLE_OPAQUE_BOUNDARY: &str = "Mantle release-bundle nodes record path, digest, role, schema, and non-claims only; Valence owns stack semantic verification";
const CAIRN_GATE_BOUNDARY: &str =
    "Cairn lifecycle-gate nodes prove lifecycle policy conformance only, not source-code behavioral correctness";
const MANUAL_REVIEW_BOUNDARY: &str =
    "Manual-review nodes are recorded inputs only unless a downstream verifier receipt establishes a stronger property";
const ISSUE_GRAPH_INPUT_MALFORMED: &str = "graph_input_malformed";
const ISSUE_GRAPH_SCHEMA_MISMATCH: &str = "graph_schema_mismatch";
const ISSUE_ARTIFACT_KIND_UNSUPPORTED: &str = "unsupported_artifact_kind";
const ISSUE_ARTIFACT_ID_MISMATCH: &str = "artifact_id_mismatch";
const ISSUE_STALE_CONTENT: &str = "stale_content";
const ISSUE_SOURCE_CONTENT_DRIFT: &str = "span_source_content_drift";
const ISSUE_UNSUPPORTED_PRODUCER_ROLE: &str = "unsupported_producer_role";
const ISSUE_UNSUPPORTED_VERIFICATION_ROLE: &str = "unsupported_verification_role";
const ISSUE_NON_CLAIM_BOUNDARY_MISSING: &str = "non_claim_boundary_missing";
const ISSUE_UNSUPPORTED_EDGE_KIND: &str = "unsupported_edge_kind";
const ISSUE_EDGE_ID_MISMATCH: &str = "edge_id_mismatch";
const ISSUE_MISSING_PARENT: &str = "missing_parent";
const ISSUE_MANUAL_REVIEW_TIER_PROMOTED: &str = "manual_review_tier_promoted";
const ISSUE_MANTLE_SEMANTIC_OVERCLAIM: &str = "mantle_semantic_overclaim";
const ISSUE_CAIRN_GATE_OVERCLAIM: &str = "cairn_gate_overclaim";
const ISSUE_EDGE_NON_CLAIM_MISSING: &str = "edge_non_claim_boundary_missing";
const ISSUE_GRAPH_NON_CLAIM_MISSING: &str = "graph_non_claim_boundary_missing";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProvenanceGraphInput {
    pub schema: String,
    pub artifacts: Vec<StackProvenanceGraphArtifact>,
    pub edges: Vec<StackProvenanceGraphEdge>,
    #[serde(default)]
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProvenanceGraphArtifact {
    pub artifact_id: String,
    pub artifact_kind: String,
    pub producer_role: String,
    pub verification_role: String,
    pub evidence_tier: String,
    pub canonical_hash: String,
    #[serde(default)]
    pub requirement_ids: Vec<String>,
    #[serde(default)]
    pub span_ids: Vec<String>,
    #[serde(default)]
    pub parent_artifact_ids: Vec<String>,
    #[serde(default)]
    pub display_path: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProvenanceGraphEdge {
    pub edge_id: String,
    pub edge_kind: String,
    pub source_artifact_id: String,
    pub target_artifact_id: String,
    pub producer_role: String,
    pub consumer_role: String,
    pub claim_scope: String,
    pub evidence_tier: String,
    #[serde(default)]
    pub required_non_claims: Vec<String>,
    #[serde(default)]
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProvenanceGraphArtifactSummary {
    pub artifact_id: String,
    pub artifact_kind: String,
    pub producer_role: String,
    pub verification_role: String,
    pub evidence_tier: String,
    pub canonical_hash: String,
    pub requirement_ids: Vec<String>,
    pub span_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProvenanceGraphEdgeSummary {
    pub edge_id: String,
    pub edge_kind: String,
    pub source_artifact_id: String,
    pub target_artifact_id: String,
    pub producer_role: String,
    pub consumer_role: String,
    pub claim_scope: String,
    pub evidence_tier: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackProvenanceGraphReport {
    pub schema: String,
    pub valid: bool,
    pub artifact_count: u64,
    pub edge_count: u64,
    pub artifacts: Vec<StackProvenanceGraphArtifactSummary>,
    pub edges: Vec<StackProvenanceGraphEdgeSummary>,
    pub trust_boundaries: Vec<String>,
    pub manual_nodes: Vec<String>,
    pub unresolved_links: Vec<String>,
    pub diagnostics: Vec<String>,
    pub supported_claim: String,
    pub non_claims: Vec<String>,
    pub report_hash: String,
}

#[must_use]
pub fn stack_provenance_graph_report_from_json(input_json: &str) -> StackProvenanceGraphReport {
    match serde_json::from_str::<StackProvenanceGraphInput>(input_json) {
        Ok(input) => verify_stack_provenance_graph(&input),
        Err(_) => finalize_graph_report(GraphReportParts {
            artifact_count: 0,
            edge_count: 0,
            artifacts: Vec::new(),
            edges: Vec::new(),
            trust_boundaries: default_trust_boundaries(),
            manual_nodes: Vec::new(),
            unresolved_links: Vec::new(),
            diagnostics: vec![ISSUE_GRAPH_INPUT_MALFORMED.to_owned()],
            non_claims: vec![STACK_PROVENANCE_GRAPH_NON_CLAIM.to_owned()],
        }),
    }
}

#[must_use]
pub fn verify_stack_provenance_graph(input: &StackProvenanceGraphInput) -> StackProvenanceGraphReport {
    // r[impl valence.stack_provenance_adapter.canonical_chain_identity]
    // r[impl valence.stack_provenance_adapter.typed_edges]
    // r[impl valence.stack_provenance_adapter.non_claim_policy]
    // r[impl valence.stack_provenance_adapter.graph_report]
    let mut diagnostics = Vec::new();
    let mut unresolved_links = Vec::new();
    let mut trust_boundaries = default_trust_boundaries();
    let mut manual_nodes = Vec::new();
    if input.schema != STACK_PROVENANCE_GRAPH_INPUT_SCHEMA {
        diagnostics.push(ISSUE_GRAPH_SCHEMA_MISMATCH.to_owned());
    }
    if !contains_boundary(&input.non_claims) {
        diagnostics.push(ISSUE_GRAPH_NON_CLAIM_MISSING.to_owned());
    }
    let known_artifacts = artifact_id_set(&input.artifacts);
    for artifact in &input.artifacts {
        diagnostics.extend(validate_graph_artifact(artifact));
        collect_artifact_boundaries(artifact, &mut trust_boundaries, &mut manual_nodes);
    }
    for edge in &input.edges {
        diagnostics.extend(validate_graph_edge(edge, &known_artifacts, &mut unresolved_links));
        collect_edge_boundaries(edge, &mut trust_boundaries, &mut manual_nodes);
    }
    finalize_graph_report(GraphReportParts {
        artifact_count: input.artifacts.len() as u64,
        edge_count: input.edges.len() as u64,
        artifacts: graph_artifact_summaries(&input.artifacts),
        edges: graph_edge_summaries(&input.edges),
        trust_boundaries: sorted_unique(trust_boundaries),
        manual_nodes: sorted_unique(manual_nodes),
        unresolved_links: sorted_unique(unresolved_links),
        diagnostics: sorted_unique(diagnostics),
        non_claims: sorted_unique(input.non_claims.clone()),
    })
}

#[must_use]
pub fn stack_graph_artifact_hash(artifact: &StackProvenanceGraphArtifact) -> String {
    let canonical = canonical_graph_artifact_value(artifact);
    hash_json_value(&canonical)
}

#[must_use]
pub fn stack_graph_artifact_id(artifact: &StackProvenanceGraphArtifact) -> String {
    format!("{GRAPH_ARTIFACT_ID_PREFIX}{}", stack_graph_artifact_hash(artifact))
}

#[must_use]
pub fn stack_graph_edge_hash(edge: &StackProvenanceGraphEdge) -> String {
    let canonical = canonical_graph_edge_value(edge);
    hash_json_value(&canonical)
}

#[must_use]
pub fn stack_graph_edge_id(edge: &StackProvenanceGraphEdge) -> String {
    format!("{GRAPH_ARTIFACT_ID_PREFIX}{}", stack_graph_edge_hash(edge))
}

struct GraphReportParts {
    artifact_count: u64,
    edge_count: u64,
    artifacts: Vec<StackProvenanceGraphArtifactSummary>,
    edges: Vec<StackProvenanceGraphEdgeSummary>,
    trust_boundaries: Vec<String>,
    manual_nodes: Vec<String>,
    unresolved_links: Vec<String>,
    diagnostics: Vec<String>,
    non_claims: Vec<String>,
}

fn finalize_graph_report(parts: GraphReportParts) -> StackProvenanceGraphReport {
    let mut report = StackProvenanceGraphReport {
        schema: STACK_PROVENANCE_GRAPH_REPORT_SCHEMA.to_owned(),
        valid: parts.diagnostics.is_empty() && parts.unresolved_links.is_empty(),
        artifact_count: parts.artifact_count,
        edge_count: parts.edge_count,
        artifacts: parts.artifacts,
        edges: parts.edges,
        trust_boundaries: sorted_unique(parts.trust_boundaries),
        manual_nodes: sorted_unique(parts.manual_nodes),
        unresolved_links: sorted_unique(parts.unresolved_links),
        diagnostics: sorted_unique(parts.diagnostics),
        supported_claim: STACK_PROVENANCE_GRAPH_SUPPORTED_CLAIM.to_owned(),
        non_claims: sorted_unique(parts.non_claims),
        report_hash: REPORT_HASH_PLACEHOLDER.to_owned(),
    };
    report.report_hash = graph_report_hash(&report);
    report
}

fn validate_graph_artifact(artifact: &StackProvenanceGraphArtifact) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if !supported_artifact_kind(&artifact.artifact_kind) {
        diagnostics.push(ISSUE_ARTIFACT_KIND_UNSUPPORTED.to_owned());
    }
    if !supported_producer_role(&artifact.producer_role) {
        diagnostics.push(ISSUE_UNSUPPORTED_PRODUCER_ROLE.to_owned());
    }
    if !supported_verification_role(&artifact.verification_role) {
        diagnostics.push(ISSUE_UNSUPPORTED_VERIFICATION_ROLE.to_owned());
    }
    if !supported_evidence_tier(&artifact.evidence_tier) {
        diagnostics.push(ISSUE_MANUAL_REVIEW_TIER_PROMOTED.to_owned());
    }
    let computed_hash = stack_graph_artifact_hash(artifact);
    if artifact.canonical_hash != computed_hash {
        diagnostics.push(ISSUE_STALE_CONTENT.to_owned());
    }
    if artifact.artifact_id != format!("{GRAPH_ARTIFACT_ID_PREFIX}{computed_hash}") {
        diagnostics.push(ISSUE_ARTIFACT_ID_MISMATCH.to_owned());
    }
    if !contains_boundary(&artifact.non_claims) {
        diagnostics.push(ISSUE_NON_CLAIM_BOUNDARY_MISSING.to_owned());
    }
    if source_content_drifted(&artifact.payload) {
        diagnostics.push(ISSUE_SOURCE_CONTENT_DRIFT.to_owned());
    }
    diagnostics
}

fn validate_graph_edge(
    edge: &StackProvenanceGraphEdge,
    known_artifacts: &BTreeSet<String>,
    unresolved_links: &mut Vec<String>,
) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if !supported_edge_kind(&edge.edge_kind) {
        diagnostics.push(ISSUE_UNSUPPORTED_EDGE_KIND.to_owned());
    }
    if !supported_producer_role(&edge.producer_role) || !supported_producer_role(&edge.consumer_role) {
        diagnostics.push(ISSUE_UNSUPPORTED_PRODUCER_ROLE.to_owned());
    }
    if !supported_evidence_tier(&edge.evidence_tier) {
        diagnostics.push(ISSUE_MANUAL_REVIEW_TIER_PROMOTED.to_owned());
    }
    let computed_hash = stack_graph_edge_hash(edge);
    if edge.edge_id != format!("{GRAPH_ARTIFACT_ID_PREFIX}{computed_hash}") {
        diagnostics.push(ISSUE_EDGE_ID_MISMATCH.to_owned());
    }
    if !known_artifacts.contains(&edge.source_artifact_id) {
        diagnostics.push(ISSUE_MISSING_PARENT.to_owned());
        unresolved_links.push(edge.source_artifact_id.clone());
    }
    if !known_artifacts.contains(&edge.target_artifact_id) {
        diagnostics.push(ISSUE_MISSING_PARENT.to_owned());
        unresolved_links.push(edge.target_artifact_id.clone());
    }
    if edge_non_claims_missing(edge) {
        diagnostics.push(ISSUE_EDGE_NON_CLAIM_MISSING.to_owned());
    }
    diagnostics.extend(validate_edge_non_claim_policy(edge));
    diagnostics
}

fn edge_non_claims_missing(edge: &StackProvenanceGraphEdge) -> bool {
    !contains_boundary(&edge.non_claims)
        || edge.required_non_claims.iter().any(|claim| !edge.non_claims.contains(claim))
}

fn validate_edge_non_claim_policy(edge: &StackProvenanceGraphEdge) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if edge.evidence_tier == EVIDENCE_TIER_MANUAL_REVIEW_REQUIRED && edge.edge_kind != EDGE_KIND_MANUAL_REVIEW_INPUT {
        diagnostics.push(ISSUE_MANUAL_REVIEW_TIER_PROMOTED.to_owned());
    }
    if edge.edge_kind == EDGE_KIND_PROVES_LOCAL_PROPERTY && edge.claim_scope == MANUAL_REVIEW_CLAIM_SCOPE {
        diagnostics.push(ISSUE_MANUAL_REVIEW_TIER_PROMOTED.to_owned());
    }
    if edge.edge_kind == EDGE_KIND_BUNDLED_INTO_RELEASE && edge.claim_scope != IDENTITY_LINKAGE_CLAIM_SCOPE {
        diagnostics.push(ISSUE_MANTLE_SEMANTIC_OVERCLAIM.to_owned());
    }
    if edge.edge_kind == EDGE_KIND_SATISFIES_GATE && edge.claim_scope != CAIRN_GATE_CLAIM_SCOPE {
        diagnostics.push(ISSUE_CAIRN_GATE_OVERCLAIM.to_owned());
    }
    diagnostics
}

fn collect_artifact_boundaries(
    artifact: &StackProvenanceGraphArtifact,
    trust_boundaries: &mut Vec<String>,
    manual_nodes: &mut Vec<String>,
) {
    if artifact.producer_role == PRODUCER_ROLE_MANTLE || artifact.artifact_kind == ARTIFACT_KIND_RELEASE_BUNDLE {
        trust_boundaries.push(MANTLE_OPAQUE_BOUNDARY.to_owned());
    }
    if artifact.producer_role == PRODUCER_ROLE_CAIRN || artifact.artifact_kind == ARTIFACT_KIND_LIFECYCLE_GATE {
        trust_boundaries.push(CAIRN_GATE_BOUNDARY.to_owned());
    }
    if artifact.evidence_tier == EVIDENCE_TIER_MANUAL_REVIEW_REQUIRED {
        trust_boundaries.push(MANUAL_REVIEW_BOUNDARY.to_owned());
        manual_nodes.push(artifact.artifact_id.clone());
    }
}

fn collect_edge_boundaries(
    edge: &StackProvenanceGraphEdge,
    trust_boundaries: &mut Vec<String>,
    manual_nodes: &mut Vec<String>,
) {
    if edge.edge_kind == EDGE_KIND_MANUAL_REVIEW_INPUT || edge.evidence_tier == EVIDENCE_TIER_MANUAL_REVIEW_REQUIRED {
        trust_boundaries.push(MANUAL_REVIEW_BOUNDARY.to_owned());
        manual_nodes.push(edge.source_artifact_id.clone());
    }
    if edge.edge_kind == EDGE_KIND_BUNDLED_INTO_RELEASE {
        trust_boundaries.push(MANTLE_OPAQUE_BOUNDARY.to_owned());
    }
    if edge.edge_kind == EDGE_KIND_SATISFIES_GATE {
        trust_boundaries.push(CAIRN_GATE_BOUNDARY.to_owned());
    }
}

fn graph_artifact_summaries(artifacts: &[StackProvenanceGraphArtifact]) -> Vec<StackProvenanceGraphArtifactSummary> {
    let mut summaries = artifacts
        .iter()
        .map(|artifact| StackProvenanceGraphArtifactSummary {
            artifact_id: artifact.artifact_id.clone(),
            artifact_kind: artifact.artifact_kind.clone(),
            producer_role: artifact.producer_role.clone(),
            verification_role: artifact.verification_role.clone(),
            evidence_tier: artifact.evidence_tier.clone(),
            canonical_hash: artifact.canonical_hash.clone(),
            requirement_ids: sorted_unique(artifact.requirement_ids.clone()),
            span_ids: sorted_unique(artifact.span_ids.clone()),
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    summaries
}

fn graph_edge_summaries(edges: &[StackProvenanceGraphEdge]) -> Vec<StackProvenanceGraphEdgeSummary> {
    let mut summaries = edges
        .iter()
        .map(|edge| StackProvenanceGraphEdgeSummary {
            edge_id: edge.edge_id.clone(),
            edge_kind: edge.edge_kind.clone(),
            source_artifact_id: edge.source_artifact_id.clone(),
            target_artifact_id: edge.target_artifact_id.clone(),
            producer_role: edge.producer_role.clone(),
            consumer_role: edge.consumer_role.clone(),
            claim_scope: edge.claim_scope.clone(),
            evidence_tier: edge.evidence_tier.clone(),
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    summaries
}

fn artifact_id_set(artifacts: &[StackProvenanceGraphArtifact]) -> BTreeSet<String> {
    artifacts.iter().map(|artifact| artifact.artifact_id.clone()).collect()
}

fn canonical_graph_artifact_value(artifact: &StackProvenanceGraphArtifact) -> Value {
    json!({
        "schema": STACK_PROVENANCE_GRAPH_INPUT_SCHEMA,
        "artifact_kind": artifact.artifact_kind,
        "producer_role": artifact.producer_role,
        "verification_role": artifact.verification_role,
        "evidence_tier": artifact.evidence_tier,
        "requirement_ids": sorted_unique(artifact.requirement_ids.clone()),
        "span_ids": sorted_unique(artifact.span_ids.clone()),
        "parent_artifact_ids": sorted_unique(artifact.parent_artifact_ids.clone()),
        "payload": canonical_json_value(&artifact.payload),
        "non_claims": sorted_unique(artifact.non_claims.clone()),
    })
}

fn canonical_graph_edge_value(edge: &StackProvenanceGraphEdge) -> Value {
    json!({
        "schema": STACK_PROVENANCE_GRAPH_INPUT_SCHEMA,
        "edge_kind": edge.edge_kind,
        "source_artifact_id": edge.source_artifact_id,
        "target_artifact_id": edge.target_artifact_id,
        "producer_role": edge.producer_role,
        "consumer_role": edge.consumer_role,
        "claim_scope": edge.claim_scope,
        "evidence_tier": edge.evidence_tier,
        "required_non_claims": sorted_unique(edge.required_non_claims.clone()),
        "non_claims": sorted_unique(edge.non_claims.clone()),
    })
}

fn graph_report_hash(report: &StackProvenanceGraphReport) -> String {
    let material = json!({
        "schema": report.schema,
        "valid": report.valid,
        "artifact_count": report.artifact_count,
        "edge_count": report.edge_count,
        "artifacts": report.artifacts,
        "edges": report.edges,
        "trust_boundaries": report.trust_boundaries,
        "manual_nodes": report.manual_nodes,
        "unresolved_links": report.unresolved_links,
        "diagnostics": report.diagnostics,
        "supported_claim": report.supported_claim,
        "non_claims": report.non_claims,
    });
    hash_json_value(&material)
}

fn hash_json_value(value: &Value) -> String {
    let canonical = canonical_json_value(value);
    match serde_json::to_vec(&canonical) {
        Ok(bytes) => blake3_hex(&bytes),
        Err(error) => blake3_hex(error.to_string().as_bytes()),
    }
}

fn canonical_json_value(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonical_json_value).collect()),
        Value::Object(fields) => {
            let mut ordered = Map::new();
            let mut keys = fields.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(value) = fields.get(key) {
                    ordered.insert(key.clone(), canonical_json_value(value));
                }
            }
            Value::Object(ordered)
        }
        scalar => scalar.clone(),
    }
}

fn default_trust_boundaries() -> Vec<String> {
    vec![
        STACK_PROVENANCE_GRAPH_NON_CLAIM.to_owned(),
        STACK_PROVENANCE_NON_CLAIM.to_owned(),
        MANTLE_OPAQUE_BOUNDARY.to_owned(),
        CAIRN_GATE_BOUNDARY.to_owned(),
    ]
}

fn source_content_drifted(payload: &Value) -> bool {
    let stated = payload.get(PAYLOAD_STATED_SOURCE_HASH_FIELD).and_then(Value::as_str);
    let actual = payload.get(PAYLOAD_SOURCE_CONTENT_HASH_FIELD).and_then(Value::as_str);
    matches!((stated, actual), (Some(stated), Some(actual)) if stated != actual)
}

fn contains_boundary(non_claims: &[String]) -> bool {
    non_claims
        .iter()
        .any(|claim| claim == STACK_PROVENANCE_GRAPH_NON_CLAIM || claim == STACK_PROVENANCE_NON_CLAIM)
}

fn supported_artifact_kind(kind: &str) -> bool {
    matches!(
        kind,
        ARTIFACT_KIND_REQUIREMENT
            | ARTIFACT_KIND_SPAN
            | ARTIFACT_KIND_RECEIPT
            | ARTIFACT_KIND_PROOF_ARTIFACT
            | ARTIFACT_KIND_SIDECAR
            | ARTIFACT_KIND_RELEASE_BUNDLE
            | ARTIFACT_KIND_LIFECYCLE_GATE
    )
}

fn supported_edge_kind(kind: &str) -> bool {
    matches!(
        kind,
        EDGE_KIND_PROVES_LOCAL_PROPERTY
            | EDGE_KIND_CHECKS_SOURCE_SHAPE
            | EDGE_KIND_EXPORTS_PROVENANCE
            | EDGE_KIND_BUNDLED_INTO_RELEASE
            | EDGE_KIND_SATISFIES_GATE
            | EDGE_KIND_MANUAL_REVIEW_INPUT
    )
}

fn supported_producer_role(role: &str) -> bool {
    matches!(
        role,
        PRODUCER_ROLE_TRELLIS
            | PRODUCER_ROLE_OCTET
            | PRODUCER_ROLE_VALENCE
            | PRODUCER_ROLE_MANTLE
            | PRODUCER_ROLE_CAIRN
            | PRODUCER_ROLE_MANUAL_REVIEW
    )
}

fn supported_verification_role(role: &str) -> bool {
    matches!(role, VERIFICATION_ROLE_PROPERTY | VERIFICATION_ROLE_RECORDED_ONLY | VERIFICATION_ROLE_BOUNDARY)
}

fn supported_evidence_tier(tier: &str) -> bool {
    matches!(
        tier,
        EVIDENCE_TIER_FORMAL_PROOF
            | EVIDENCE_TIER_SOURCE_SHAPE_CHECK
            | EVIDENCE_TIER_PROVENANCE_EXPORT
            | EVIDENCE_TIER_RELEASE_BUNDLE
            | EVIDENCE_TIER_LIFECYCLE_GATE
            | EVIDENCE_TIER_MANUAL_REVIEW_REQUIRED
    )
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_ids_ignore_field_order_and_paths() {
        // r[verify valence.stack_provenance_adapter.canonical_chain_identity.deterministic]
        // r[verify valence.stack_provenance_adapter.canonical_chain_identity.paths_metadata]
        let first = graph_artifact("requirement", ARTIFACT_KIND_REQUIREMENT, PRODUCER_ROLE_VALENCE);
        let mut second = first.clone();
        second.display_path = "different/path.md".to_owned();
        second.requirement_ids.reverse();
        assert_eq!(stack_graph_artifact_hash(&first), stack_graph_artifact_hash(&second));
        assert_eq!(stack_graph_artifact_id(&first), stack_graph_artifact_id(&second));
    }

    #[test]
    fn graph_report_accepts_complete_fixture() {
        // r[verify valence.stack_provenance_adapter.typed_edges.supported]
        // r[verify valence.stack_provenance_adapter.non_claim_policy.mantle_opaque]
        // r[verify valence.stack_provenance_adapter.non_claim_policy.cairn_gate]
        // r[verify valence.stack_provenance_adapter.graph_report.json]
        // r[verify valence.stack_provenance_adapter.broken_chain_fixtures.positive]
        let report = stack_provenance_graph_report_from_json(include_str!(
            "../../../../fixtures/stack-provenance-chain/positive/complete.json"
        ));
        assert!(report.valid, "unexpected diagnostics: {:?}", report.diagnostics);
        assert!(report.trust_boundaries.contains(&MANTLE_OPAQUE_BOUNDARY.to_owned()));
        assert!(report.trust_boundaries.contains(&CAIRN_GATE_BOUNDARY.to_owned()));
        assert_eq!(report.edge_count, 6);
        assert!(!report.report_hash.is_empty());
    }

    #[test]
    fn graph_report_rejects_broken_fixtures() {
        // r[verify valence.stack_provenance_adapter.canonical_chain_identity.stale]
        // r[verify valence.stack_provenance_adapter.typed_edges.unsupported]
        // r[verify valence.stack_provenance_adapter.typed_edges.missing_parent]
        // r[verify valence.stack_provenance_adapter.non_claim_policy.manual_review]
        // r[verify valence.stack_provenance_adapter.non_claim_policy.weakened]
        // r[verify valence.stack_provenance_adapter.broken_chain_fixtures.negative]
        let cases = [
            ("missing-parent.json", ISSUE_MISSING_PARENT),
            ("digest-mismatch.json", ISSUE_STALE_CONTENT),
            ("stale-policy.json", ISSUE_CAIRN_GATE_OVERCLAIM),
            ("unsupported-edge.json", ISSUE_UNSUPPORTED_EDGE_KIND),
            ("tier-promotion.json", ISSUE_MANUAL_REVIEW_TIER_PROMOTED),
            ("weakened-non-claim.json", ISSUE_EDGE_NON_CLAIM_MISSING),
            ("span-source-drift.json", ISSUE_SOURCE_CONTENT_DRIFT),
        ];
        for (fixture, issue) in cases {
            let input = fixture_json(fixture);
            let report = stack_provenance_graph_report_from_json(&input);
            assert!(!report.valid, "{fixture} unexpectedly passed");
            assert!(report.diagnostics.contains(&issue.to_owned()), "{fixture}: {:?}", report.diagnostics);
        }
    }

    fn fixture_json(name: &str) -> String {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/stack-provenance-chain/negative")
            .join(name);
        std::fs::read_to_string(root).expect("fixture reads")
    }

    fn graph_artifact(label: &str, kind: &str, producer_role: &str) -> StackProvenanceGraphArtifact {
        StackProvenanceGraphArtifact {
            artifact_id: String::new(),
            artifact_kind: kind.to_owned(),
            producer_role: producer_role.to_owned(),
            verification_role: VERIFICATION_ROLE_BOUNDARY.to_owned(),
            evidence_tier: EVIDENCE_TIER_PROVENANCE_EXPORT.to_owned(),
            canonical_hash: String::new(),
            requirement_ids: vec!["r[valence.stack_provenance_adapter.canonical_chain_identity]".to_owned()],
            span_ids: vec!["span:fixture".to_owned()],
            parent_artifact_ids: Vec::new(),
            display_path: format!("fixtures/{label}.json"),
            payload: json!({"label": label}),
            non_claims: vec![STACK_PROVENANCE_GRAPH_NON_CLAIM.to_owned()],
        }
    }
}
