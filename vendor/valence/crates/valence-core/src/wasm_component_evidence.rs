//! Bounded WebAssembly component evidence identity and linkage profile.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use crate::evidence_ir::VerificationRole;

pub const WASM_COMPONENT_NON_CLAIM: &str = "Wasm component evidence proves identity and linkage only, not behavior, semantic equivalence, package trust, sandbox effectiveness, authority, runtime correctness, portability, or release eligibility";
const BLAKE3_HEX_LENGTH: usize = 64;
const SHA256_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WasmArtifactRole {
    WitInterface,
    PortableComponent,
    CompositionPlan,
    VirtualizationPlan,
    Transform,
    PrecompiledNative,
    MantleMaterializationBundle,
    ProducerReceipt,
    Configuration,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WasmEdgeKind {
    ImplementsWorld,
    ComposedFrom,
    VirtualizedBy,
    TransformedFrom,
    PrecompiledFrom,
    MaterializedIn,
    ConfiguredBy,
    ExecutedAs,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalOciDigest {
    pub algorithm: String,
    pub registry: String,
    pub package: String,
    pub version: String,
    pub digest: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmEvidenceNode {
    pub id: String,
    pub role: WasmArtifactRole,
    pub blake3: String,
    pub verification_role: VerificationRole,
    pub target: Option<String>,
    pub cpu_features: Vec<String>,
    pub external_oci: Option<ExternalOciDigest>,
    pub non_claims: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmEvidenceEdge {
    pub from: String,
    pub to: String,
    pub kind: WasmEdgeKind,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmComponentEvidenceGraph {
    pub nodes: Vec<WasmEvidenceNode>,
    pub edges: Vec<WasmEvidenceEdge>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WasmEvidenceIssue {
    pub field: &'static str,
    pub code: &'static str,
}
#[derive(Debug, Clone, Serialize)]
pub struct WasmEvidenceReport {
    pub valid: bool,
    pub issues: Vec<WasmEvidenceIssue>,
    pub node_count: usize,
    pub edge_count: usize,
}

/// r[impl valence.wasm_component.profile] r[impl valence.wasm_component.graph]
/// r[impl valence.wasm_component.materialization_bundle] r[impl
/// valence.wasm_component.functional_core]
#[must_use]
pub fn validate_wasm_component_graph(graph: &WasmComponentEvidenceGraph) -> WasmEvidenceReport {
    let mut issues = Vec::new();
    let mut nodes = BTreeMap::new();
    for node in &graph.nodes {
        if node.id.trim().is_empty() {
            issues.push(issue("nodes.id", "id_missing"));
        }
        if nodes.insert(node.id.as_str(), node).is_some() {
            issues.push(issue("nodes.id", "duplicate_node"));
        }
        if !valid_hex(&node.blake3, BLAKE3_HEX_LENGTH) {
            issues.push(issue("nodes.blake3", "malformed_blake3"));
        }
        if node.verification_role == VerificationRole::Property {
            issues.push(issue("nodes.verification_role", "property_role_denied"));
        }
        if !node.non_claims.iter().any(|claim| claim == WASM_COMPONENT_NON_CLAIM) {
            issues.push(issue("nodes.non_claims", "non_claims_weakened"));
        }
        if node.role == WasmArtifactRole::PrecompiledNative
            && (node.target.as_deref().unwrap_or("").is_empty() || node.cpu_features.is_empty())
        {
            issues.push(issue("nodes.precompiled", "target_configuration_missing"));
        }
        if let Some(oci) = &node.external_oci {
            if oci.algorithm != "sha256"
                || !valid_hex(&oci.digest, SHA256_HEX_LENGTH)
                || oci.registry.is_empty()
                || oci.package.is_empty()
                || oci.version.is_empty()
            {
                issues.push(issue("nodes.external_oci", "external_digest_invalid"));
            }
            if normalize(&oci.digest) == normalize(&node.blake3) {
                issues.push(issue("nodes.external_oci", "external_digest_used_as_canonical"));
            }
        }
    }
    let mut edges = BTreeSet::new();
    for edge in &graph.edges {
        if !nodes.contains_key(edge.from.as_str()) || !nodes.contains_key(edge.to.as_str()) {
            issues.push(issue("edges", "missing_parent"));
        }
        if !edges.insert((&edge.from, &edge.to, edge.kind)) {
            issues.push(issue("edges", "duplicate_edge"));
        }
        if edge.from == edge.to {
            issues.push(issue("edges", "cycle"));
        }
        if let (Some(from), Some(to)) = (nodes.get(edge.from.as_str()), nodes.get(edge.to.as_str()))
            && !allowed_edge(from.role, to.role, edge.kind)
        {
            issues.push(issue("edges.kind", "wrong_edge"));
        }
    }
    for node in &graph.nodes {
        if matches!(node.role, WasmArtifactRole::PrecompiledNative | WasmArtifactRole::MantleMaterializationBundle)
            && !graph.edges.iter().any(|edge| edge.from == node.id)
        {
            issues.push(issue("edges", "required_parent_missing"));
        }
    }
    issues.sort_by_key(|item| (item.field, item.code));
    issues.dedup();
    WasmEvidenceReport {
        valid: issues.is_empty(),
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        issues,
    }
}
fn allowed_edge(from: WasmArtifactRole, to: WasmArtifactRole, kind: WasmEdgeKind) -> bool {
    matches!(
        (from, to, kind),
        (WasmArtifactRole::PortableComponent, WasmArtifactRole::WitInterface, WasmEdgeKind::ImplementsWorld)
            | (
                WasmArtifactRole::PrecompiledNative,
                WasmArtifactRole::PortableComponent,
                WasmEdgeKind::PrecompiledFrom
            )
            | (WasmArtifactRole::PrecompiledNative, WasmArtifactRole::Configuration, WasmEdgeKind::ConfiguredBy)
            | (WasmArtifactRole::MantleMaterializationBundle, _, WasmEdgeKind::MaterializedIn)
            | (WasmArtifactRole::Transform, WasmArtifactRole::PortableComponent, WasmEdgeKind::TransformedFrom)
            | (WasmArtifactRole::PortableComponent, WasmArtifactRole::CompositionPlan, WasmEdgeKind::ComposedFrom)
            | (
                WasmArtifactRole::PortableComponent,
                WasmArtifactRole::VirtualizationPlan,
                WasmEdgeKind::VirtualizedBy
            )
            | (WasmArtifactRole::ProducerReceipt, _, WasmEdgeKind::ExecutedAs)
    )
}
fn normalize(value: &str) -> &str {
    value.strip_prefix("b3:").unwrap_or(value)
}
fn valid_hex(value: &str, length: usize) -> bool {
    let value = normalize(value);
    value.len() == length && value.chars().all(|c| c.is_ascii_hexdigit())
}
const fn issue(field: &'static str, code: &'static str) -> WasmEvidenceIssue {
    WasmEvidenceIssue { field, code }
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    fn node(id: &str, role: WasmArtifactRole) -> WasmEvidenceNode {
        WasmEvidenceNode {
            id: id.to_owned(),
            role,
            blake3: A.to_owned(),
            verification_role: VerificationRole::RecordedOnly,
            target: (role == WasmArtifactRole::PrecompiledNative).then(|| "x86_64-linux".to_owned()),
            cpu_features: if role == WasmArtifactRole::PrecompiledNative {
                vec!["sse2".to_owned()]
            } else {
                Vec::new()
            },
            external_oci: None,
            non_claims: vec![WASM_COMPONENT_NON_CLAIM.to_owned()],
        }
    }
    #[test]
    fn complete_component_materialization_graph_passes() {
        let graph = WasmComponentEvidenceGraph {
            nodes: vec![
                node("wit", WasmArtifactRole::WitInterface),
                node("component", WasmArtifactRole::PortableComponent),
                node("config", WasmArtifactRole::Configuration),
                node("native", WasmArtifactRole::PrecompiledNative),
                node("bundle", WasmArtifactRole::MantleMaterializationBundle),
            ],
            edges: vec![
                WasmEvidenceEdge {
                    from: "component".to_owned(),
                    to: "wit".to_owned(),
                    kind: WasmEdgeKind::ImplementsWorld,
                },
                WasmEvidenceEdge {
                    from: "native".to_owned(),
                    to: "component".to_owned(),
                    kind: WasmEdgeKind::PrecompiledFrom,
                },
                WasmEvidenceEdge {
                    from: "native".to_owned(),
                    to: "config".to_owned(),
                    kind: WasmEdgeKind::ConfiguredBy,
                },
                WasmEvidenceEdge {
                    from: "bundle".to_owned(),
                    to: "native".to_owned(),
                    kind: WasmEdgeKind::MaterializedIn,
                },
            ],
        };
        assert!(validate_wasm_component_graph(&graph).valid);
    }
    #[test]
    fn digest_role_equivalence_cycle_property_and_bundle_shortcut_fail() {
        let mut bad = node("native", WasmArtifactRole::PrecompiledNative);
        bad.verification_role = VerificationRole::Property;
        bad.external_oci = Some(ExternalOciDigest {
            algorithm: "sha256".to_owned(),
            registry: "r".to_owned(),
            package: "p".to_owned(),
            version: "v".to_owned(),
            digest: A.to_owned(),
        });
        bad.non_claims.clear();
        let graph = WasmComponentEvidenceGraph {
            nodes: vec![bad, node("other", WasmArtifactRole::PortableComponent)],
            edges: vec![WasmEvidenceEdge {
                from: "native".to_owned(),
                to: "native".to_owned(),
                kind: WasmEdgeKind::PrecompiledFrom,
            }],
        };
        let report = validate_wasm_component_graph(&graph);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|item| item.code == "external_digest_used_as_canonical"));
        let _ = B;
    }
}
