use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrySourceRecord {
    pub source_id: String,
    pub family: String,
    pub schema: String,
    pub content_hash: String,
    pub vocabulary_owner: String,
    pub source_ref: String,
    #[serde(default)]
    pub non_claims: Vec<String>,
    #[serde(default)]
    pub record_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryNode {
    pub stable_id: String,
    pub kind: String,
    pub schema: String,
    pub content_hash: String,
    pub payload_ref: String,
    pub source_id: String,
    pub vocabulary_owner: String,
    #[serde(default)]
    pub non_claims: Vec<String>,
    #[serde(default)]
    pub record_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryTagAssignment {
    pub assignment_id: String,
    pub subject_id: String,
    pub subject_kind: String,
    pub namespace: String,
    pub key: String,
    pub value: String,
    pub source_id: String,
    #[serde(default)]
    pub non_claims: Vec<String>,
    #[serde(default)]
    pub record_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEdge {
    pub stable_id: String,
    pub kind: String,
    pub from_id: String,
    pub to_id: String,
    pub vocabulary_owner: String,
    pub source_id: String,
    pub status: String,
    #[serde(default)]
    pub non_claims: Vec<String>,
    #[serde(default)]
    pub record_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryAdapterMetadata {
    pub adapter_id: String,
    pub adapter_kind: String,
    pub schema: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub record_families: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalRegistrySnapshot {
    pub schema: String,
    pub scope_id: String,
    #[serde(default)]
    pub sources: Vec<RegistrySourceRecord>,
    #[serde(default)]
    pub nodes: Vec<RegistryNode>,
    #[serde(default)]
    pub tags: Vec<RegistryTagAssignment>,
    #[serde(default)]
    pub edges: Vec<RegistryEdge>,
    #[serde(default)]
    pub adapters: Vec<RegistryAdapterMetadata>,
    #[serde(default)]
    pub non_claims: Vec<String>,
    #[serde(default)]
    pub snapshot_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RegistryCandidateRecords {
    #[serde(default)]
    pub sources: Vec<RegistrySourceRecord>,
    #[serde(default)]
    pub nodes: Vec<RegistryNode>,
    #[serde(default)]
    pub tags: Vec<RegistryTagAssignment>,
    #[serde(default)]
    pub edges: Vec<RegistryEdge>,
    #[serde(default)]
    pub adapters: Vec<RegistryAdapterMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegistryIssue {
    pub code: String,
    pub record_kind: String,
    pub stable_id: String,
    pub field: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegistrySnapshotReport {
    pub valid: bool,
    pub verdict: String,
    pub issues: Vec<RegistryIssue>,
    pub snapshot: Option<GlobalRegistrySnapshot>,
    pub input_hash: String,
    pub receipt_hash: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryCorePolicy {
    #[serde(default)]
    pub reserved_tag_namespaces: Vec<String>,
    #[serde(default)]
    pub allowed_vocabulary_owners: Vec<String>,
    #[serde(default)]
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryGraphProfile {
    pub id: String,
    #[serde(default)]
    pub allowed_node_kinds: Vec<String>,
    #[serde(default)]
    pub allowed_edge_kinds: Vec<String>,
    pub require_dag: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryGraphDirection {
    Ancestors,
    Descendants,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegistryGraphReport {
    pub valid: bool,
    pub verdict: String,
    pub profile_id: String,
    pub snapshot_hash: String,
    pub selected_nodes: Vec<String>,
    pub selected_edges: Vec<String>,
    pub topological_order: Vec<String>,
    pub cycle_witness: Vec<String>,
    pub unresolved_nodes: Vec<String>,
    pub issues: Vec<RegistryIssue>,
    pub result_hash: String,
    pub receipt_hash: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegistryQueryReport {
    pub valid: bool,
    pub verdict: String,
    pub snapshot_hash: String,
    pub selector: String,
    pub selected_ids: Vec<String>,
    pub issues: Vec<RegistryIssue>,
    pub result_hash: String,
    pub receipt_hash: String,
    pub non_claims: Vec<String>,
}
