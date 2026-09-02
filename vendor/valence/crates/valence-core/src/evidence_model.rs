//! Unison-inspired Evidence IR support models.
//!
//! This module keeps the functional core for content-addressed evidence
//! definitions, name bindings, queryable graphs, verifier capabilities,
//! dependency-closure cache keys, executable transcripts, structured
//! refactoring sessions, and typed durable storage envelopes. All helpers are
//! pure over in-memory values; CLI/filesystem/process orchestration belongs in
//! `valence-cli` or downstream tools.
//!
//! # Boundary
//!
//! These models prove identity, linkage, decode compatibility, deterministic
//! fixture behavior, and cache-key equality only. They do not prove behavioral
//! correctness, semantic equivalence, verifier soundness, release eligibility,
//! requirement satisfaction, evidence truth, or whole-program safety.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;

use serde::Deserialize;
use serde::Serialize;

use crate::evidence_ir::check_evidence_artifact;
use crate::stack_provenance_adapter::verify_built_binary_thread_json;

/// Prefix used for BLAKE3 identities rendered in Valence receipts.
pub const BLAKE3_IDENTITY_PREFIX: &str = "b3:";
/// Content-addressed evidence definition schema.
pub const EVIDENCE_DEFINITION_SCHEMA: &str = "valence.evidence-definition.v1";
/// Name-binding history schema.
pub const NAME_BINDING_HISTORY_SCHEMA: &str = "valence.name-binding-history.v1";
/// Queryable evidence graph schema.
pub const EVIDENCE_GRAPH_SCHEMA: &str = "valence.evidence-graph.v1";
/// Verifier capability set schema.
pub const VERIFIER_CAPABILITY_SET_SCHEMA: &str = "valence.verifier-capabilities.v1";
/// Verifier cache key schema.
pub const VERIFIER_CACHE_KEY_SCHEMA: &str = "valence.verifier-cache-key.v1";
/// Executable receipt transcript schema.
pub const RECEIPT_TRANSCRIPT_SCHEMA: &str = "valence.receipt-transcript.v1";
/// Structured refactoring session schema.
pub const REFACTORING_SESSION_SCHEMA: &str = "valence.refactoring-session.v1";
/// Typed durable evidence storage schema.
pub const DURABLE_STORAGE_SCHEMA: &str = "valence.typed-durable-storage.v1";
/// Current cache schema version included in every cache key.
pub const CURRENT_CACHE_SCHEMA_VERSION: &str = VERIFIER_CACHE_KEY_SCHEMA;

/// Boundary text for content-addressed evidence definition receipts.
pub const CONTENT_IDENTITY_NON_CLAIM: &str = "content identity proves canonical content and dependency identity only, not behavioral correctness, semantic equivalence, verifier soundness, release eligibility, or requirement satisfaction";
/// Boundary text for name-binding receipts.
pub const NAME_BINDING_NON_CLAIM: &str =
    "aliases are review metadata and do not prove semantic equivalence or requirement satisfaction";
/// Boundary text for queryable graph output.
pub const EVIDENCE_GRAPH_NON_CLAIM: &str = "evidence graph indexes evidence identity and linkage only, not behavioral correctness, verifier soundness, release eligibility, or requirement satisfaction";
/// Boundary text for verifier capability receipts.
pub const CAPABILITY_NON_CLAIM: &str = "capability labels are receipt evidence unless a sandboxed runner enforces them, and labels do not prove verifier soundness or requirement satisfaction";
/// Boundary text for cache receipts.
pub const CACHE_NON_CLAIM: &str = "cache hits prove matching cache-key identities only, not verifier soundness, correctness, or requirement satisfaction";
/// Boundary text for transcript fixtures.
pub const TRANSCRIPT_NON_CLAIM: &str = "transcripts prove fixture behavior for checked inputs only, not general correctness, verifier soundness, or requirement satisfaction";
/// Boundary text for refactoring receipts.
pub const REFACTORING_NON_CLAIM: &str = "refactoring sessions preserve and rewrite identity bindings only, not semantic equivalence, correctness, or requirement satisfaction";
/// Boundary text for typed durable storage envelopes.
pub const DURABLE_STORAGE_NON_CLAIM: &str = "typed storage proves decode identity and compatibility only, not evidence truth, correctness, verifier soundness, or requirement satisfaction";

const EMPTY_HASH_INPUT: &str = "";
const ISSUE_ALIAS_AMBIGUOUS: &str = "alias_ambiguous";
const ISSUE_ALIAS_MISSING: &str = "alias_missing";
const ISSUE_BINDING_STALE: &str = "binding_stale";
const ISSUE_BOUNDARY_MISSING: &str = "non_claim_boundary_missing";
const ISSUE_CACHE_ENTRY_MISSING: &str = "cache_entry_missing";
const ISSUE_CACHE_FIELD_CHANGED: &str = "cache_key_field_changed";
const ISSUE_CAPABILITY_DENIED: &str = "capability_denied";
const ISSUE_CAPABILITY_NOT_REPLAY_SAFE: &str = "capability_not_replay_safe";
const ISSUE_CODEC_MISMATCH: &str = "codec_mismatch";
const ISSUE_CONTENT_HASH_MISMATCH: &str = "content_hash_mismatch";
const ISSUE_DANGLING_EDGE: &str = "dangling_edge";
const ISSUE_DUPLICATE_NODE: &str = "duplicate_node_identity";
const ISSUE_EXPECTED_HASH_MISMATCH: &str = "expected_hash_mismatch";
const ISSUE_EXPECTED_HASH_MISSING: &str = "expected_hash_missing";
const ISSUE_GRAPH_INVALID: &str = "graph_invalid";
const ISSUE_INPUT_MISSING: &str = "input_missing";
const ISSUE_ISSUE_CODES_MISMATCH: &str = "issue_codes_mismatch";
const ISSUE_MALFORMED_INPUT: &str = "malformed_input";
const ISSUE_MIGRATION_MISSING: &str = "migration_missing";
const ISSUE_MISSING_IDENTITY: &str = "missing_identity";
const ISSUE_REPORT_VALID_MISMATCH: &str = "report_valid_mismatch";
const ISSUE_SCHEMA_MISMATCH: &str = "schema_mismatch";
const ISSUE_SEMANTIC_EQUIVALENCE_REQUIRED: &str = "semantic_equivalence_required";
const ISSUE_TYPE_MISMATCH: &str = "value_type_mismatch";
const ISSUE_UNKNOWN_CAPABILITY: &str = "unknown_capability";
const ISSUE_UNSUPPORTED_MUTATION_SURFACE: &str = "unsupported_mutation_surface";

/// Error returned by canonical serialization helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceModelError {
    /// A typed value could not be serialized for hashing/export.
    CanonicalSerialization { detail: String },
}

impl std::fmt::Display for EvidenceModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CanonicalSerialization { detail } => write!(f, "canonicalize evidence model value: {detail}"),
        }
    }
}

impl std::error::Error for EvidenceModelError {}

/// Content identity string wrapper used where a BLAKE3 identity is required.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceContentIdentity {
    /// Rendered identity, normally `b3:<hex>`.
    pub value: String,
}

impl EvidenceContentIdentity {
    /// Creates a content identity from a rendered string.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self { value: value.into() }
    }

    /// Returns true when the identity uses Valence's BLAKE3 prefix.
    #[must_use]
    pub fn is_blake3_identity(&self) -> bool {
        self.value.starts_with(BLAKE3_IDENTITY_PREFIX)
    }
}

/// A dependency reference in canonical evidence-definition content.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceDependency {
    /// Dependency role, for example `definition`, `artifact`, `verifier-input`, or `requirement`.
    pub role: String,
    /// Content identity of the dependency when known.
    pub content_identity: EvidenceContentIdentity,
}

impl EvidenceDependency {
    /// Creates a dependency reference.
    #[must_use]
    pub fn new(role: impl Into<String>, content_identity: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content_identity: EvidenceContentIdentity::new(content_identity),
        }
    }
}

/// Review metadata deliberately excluded from content identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceDefinitionMetadata {
    /// Human-readable display name.
    pub display_name: Option<String>,
    /// Source path for review.
    pub source_path: Option<String>,
    /// Aliases that may resolve to the content identity.
    pub aliases: Vec<String>,
}

impl EvidenceDefinitionMetadata {
    /// Creates deterministic review metadata.
    #[must_use]
    pub fn new(display_name: Option<String>, source_path: Option<String>, aliases: Vec<String>) -> Self {
        let mut canonical_aliases = aliases;
        canonical_aliases.sort();
        canonical_aliases.dedup();
        Self {
            display_name,
            source_path,
            aliases: canonical_aliases,
        }
    }
}

/// Typed canonical evidence-definition value used for identity hashing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceDefinitionCanonicalValue {
    /// Schema identity for the canonical value.
    pub schema: String,
    /// Definition kind such as `function-object`, `receipt`, or `requirement-binding`.
    pub definition_kind: String,
    /// Canonical body projected by the owning adapter.
    pub canonical_body: serde_json::Value,
    /// Dependency content identities included in parent identity.
    pub dependencies: Vec<EvidenceDependency>,
    /// Explicit non-claim boundary.
    pub non_claims: Vec<String>,
}

/// Content-addressed evidence definition plus review metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceDefinition {
    /// BLAKE3 identity over the canonical value.
    pub content_identity: EvidenceContentIdentity,
    /// Canonical value used for hashing.
    pub canonical: EvidenceDefinitionCanonicalValue,
    /// Human-review metadata excluded from the hash.
    pub metadata: EvidenceDefinitionMetadata,
}

/// Validation report for content-addressed definitions and other pure models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCheckReport {
    /// True when no issue codes were produced.
    pub valid: bool,
    /// Deterministic issue codes.
    pub issue_codes: Vec<&'static str>,
}

/// Builds a content-addressed evidence definition.
///
/// r[impl valence.evidence_ir.content_addressed_definitions]
/// r[impl valence.evidence_ir.content_addressed_definitions.dependencies]
pub fn build_evidence_definition(
    definition_kind: &str,
    canonical_body: serde_json::Value,
    dependencies: Vec<EvidenceDependency>,
    metadata: EvidenceDefinitionMetadata,
    non_claims: Vec<String>,
) -> Result<EvidenceDefinition, EvidenceModelError> {
    let canonical = EvidenceDefinitionCanonicalValue {
        schema: EVIDENCE_DEFINITION_SCHEMA.to_owned(),
        definition_kind: definition_kind.to_owned(),
        canonical_body,
        dependencies: sorted_vec(dependencies),
        non_claims: sorted_vec(non_claims),
    };
    let content_identity = EvidenceContentIdentity::new(hash_typed_value(&canonical)?);
    Ok(EvidenceDefinition {
        content_identity,
        canonical,
        metadata,
    })
}

/// Computes the identity of a canonical evidence definition value.
///
/// r[impl valence.evidence_ir.content_addressed_definitions]
pub fn evidence_definition_identity(
    canonical: &EvidenceDefinitionCanonicalValue,
) -> Result<EvidenceContentIdentity, EvidenceModelError> {
    Ok(EvidenceContentIdentity::new(hash_typed_value(canonical)?))
}

/// Validates a content-addressed evidence definition.
#[must_use]
pub fn validate_evidence_definition(definition: &EvidenceDefinition) -> ModelCheckReport {
    let mut issue_codes = Vec::new();

    if definition.canonical.schema != EVIDENCE_DEFINITION_SCHEMA {
        issue_codes.push(ISSUE_SCHEMA_MISMATCH);
    }
    if !definition.canonical.non_claims.iter().any(|claim| claim == CONTENT_IDENTITY_NON_CLAIM) {
        issue_codes.push(ISSUE_BOUNDARY_MISSING);
    }
    for dependency in &definition.canonical.dependencies {
        if !dependency.content_identity.is_blake3_identity() {
            issue_codes.push(ISSUE_MISSING_IDENTITY);
        }
    }
    match evidence_definition_identity(&definition.canonical) {
        Ok(identity) if identity == definition.content_identity => {}
        Ok(_) | Err(_) => issue_codes.push(ISSUE_CONTENT_HASH_MISMATCH),
    }

    report_from_issues(issue_codes)
}

/// Kinds of human-readable bindings tracked as metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameBindingKind {
    /// User-facing alias.
    Alias,
    /// Display name.
    DisplayName,
    /// Source path.
    SourcePath,
    /// Requirement label/name.
    RequirementName,
}

/// A single alias/name/path binding to a content identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceNameBinding {
    /// Binding kind.
    pub kind: NameBindingKind,
    /// Human-readable name or path.
    pub name: String,
    /// Bound content identity.
    pub content_identity: EvidenceContentIdentity,
    /// Monotonic sequence within the history supplied by the caller.
    pub sequence: u64,
    /// True when this binding is currently active.
    pub current: bool,
}

impl EvidenceNameBinding {
    /// Creates a binding record.
    #[must_use]
    pub fn new(
        kind: NameBindingKind,
        name: impl Into<String>,
        content_identity: impl Into<String>,
        sequence: u64,
        current: bool,
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            content_identity: EvidenceContentIdentity::new(content_identity),
            sequence,
            current,
        }
    }
}

/// Name-binding history carried by reports/receipts when review-relevant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameBindingHistory {
    /// Schema identity.
    pub schema: String,
    /// Binding records.
    pub bindings: Vec<EvidenceNameBinding>,
    /// Explicit boundary text.
    pub non_claims: Vec<String>,
}

impl NameBindingHistory {
    /// Creates a deterministic binding history.
    #[must_use]
    pub fn new(bindings: Vec<EvidenceNameBinding>) -> Self {
        Self {
            schema: NAME_BINDING_HISTORY_SCHEMA.to_owned(),
            bindings: sorted_vec(bindings),
            non_claims: vec![NAME_BINDING_NON_CLAIM.to_owned()],
        }
    }
}

/// Alias resolution result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingResolution {
    /// Resolved identity when resolution succeeds.
    pub identity: Option<EvidenceContentIdentity>,
    /// Deterministic diagnostics.
    pub issue_codes: Vec<&'static str>,
}

impl BindingResolution {
    /// True when an identity was resolved without diagnostics.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.identity.is_some() && self.issue_codes.is_empty()
    }
}

/// Resolves a human-readable name to a single current content identity.
///
/// r[impl valence.evidence_ir.name_binding_history.resolve]
/// r[impl valence.evidence_ir.name_binding_history.ambiguous]
#[must_use]
pub fn resolve_name_binding(history: &NameBindingHistory, name: &str) -> BindingResolution {
    let identities: BTreeSet<EvidenceContentIdentity> = history
        .bindings
        .iter()
        .filter(|binding| binding.current && binding.name == name)
        .map(|binding| binding.content_identity.clone())
        .collect();

    let mut issue_codes = Vec::new();
    let identity = match identities.len() {
        len if len == CollectionCardinality::None.as_usize() => {
            issue_codes.push(ISSUE_ALIAS_MISSING);
            None
        }
        len if len == CollectionCardinality::One.as_usize() => identities.iter().next().cloned(),
        _ => {
            issue_codes.push(ISSUE_ALIAS_AMBIGUOUS);
            None
        }
    };

    BindingResolution { identity, issue_codes }
}

/// Validates binding history against known content identities.
///
/// r[impl valence.evidence_ir.name_binding_history.stale]
#[must_use]
pub fn validate_name_bindings(history: &NameBindingHistory, known_identities: &BTreeSet<String>) -> ModelCheckReport {
    let mut issue_codes = Vec::new();
    if history.schema != NAME_BINDING_HISTORY_SCHEMA {
        issue_codes.push(ISSUE_SCHEMA_MISMATCH);
    }
    if !history.non_claims.iter().any(|claim| claim == NAME_BINDING_NON_CLAIM) {
        issue_codes.push(ISSUE_BOUNDARY_MISSING);
    }
    for binding in &history.bindings {
        if !known_identities.contains(&binding.content_identity.value) {
            issue_codes.push(ISSUE_BINDING_STALE);
        }
    }
    for name in current_binding_names(history) {
        let resolution = resolve_name_binding(history, &name);
        if resolution.issue_codes.contains(&ISSUE_ALIAS_AMBIGUOUS) {
            issue_codes.push(ISSUE_ALIAS_AMBIGUOUS);
        }
    }
    report_from_issues(issue_codes)
}

/// Typed graph node kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceGraphNodeKind {
    Definition,
    Artifact,
    Requirement,
    Verifier,
    Receipt,
    Binary,
    Sidecar,
    Alias,
}

/// Typed graph edge kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceGraphEdgeKind {
    DependsOn,
    CoversRequirement,
    ProducedByVerifier,
    RecordedInReceipt,
    BuiltAsBinary,
    HasSidecar,
    AliasBinding,
    TracesTo,
}

/// A typed evidence graph node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceGraphNode {
    pub identity: String,
    pub kind: EvidenceGraphNodeKind,
    pub label: Option<String>,
}

impl EvidenceGraphNode {
    /// Creates a graph node.
    #[must_use]
    pub fn new(identity: impl Into<String>, kind: EvidenceGraphNodeKind, label: Option<String>) -> Self {
        Self {
            identity: identity.into(),
            kind,
            label,
        }
    }
}

/// A typed evidence graph edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceGraphEdge {
    pub source: String,
    pub target: String,
    pub kind: EvidenceGraphEdgeKind,
}

impl EvidenceGraphEdge {
    /// Creates a graph edge.
    #[must_use]
    pub fn new(source: impl Into<String>, target: impl Into<String>, kind: EvidenceGraphEdgeKind) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            kind,
        }
    }
}

/// Deterministic evidence graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceGraph {
    pub schema: String,
    pub nodes: Vec<EvidenceGraphNode>,
    pub edges: Vec<EvidenceGraphEdge>,
    pub bindings: NameBindingHistory,
    pub non_claims: Vec<String>,
}

impl EvidenceGraph {
    /// Creates a graph without dropping duplicates, so validation can fail closed.
    #[must_use]
    pub fn new(nodes: Vec<EvidenceGraphNode>, edges: Vec<EvidenceGraphEdge>, bindings: NameBindingHistory) -> Self {
        Self {
            schema: EVIDENCE_GRAPH_SCHEMA.to_owned(),
            nodes,
            edges,
            bindings,
            non_claims: vec![EVIDENCE_GRAPH_NON_CLAIM.to_owned()],
        }
    }

    fn canonical(&self) -> Self {
        Self {
            schema: self.schema.clone(),
            nodes: sorted_vec(self.nodes.clone()),
            edges: sorted_vec(self.edges.clone()),
            bindings: NameBindingHistory {
                schema: self.bindings.schema.clone(),
                bindings: sorted_vec(self.bindings.bindings.clone()),
                non_claims: sorted_vec(self.bindings.non_claims.clone()),
            },
            non_claims: sorted_vec(self.non_claims.clone()),
        }
    }
}

/// Evidence graph validation report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceGraphReport {
    pub valid: bool,
    pub node_count: usize,
    pub edge_count: usize,
    pub issue_codes: Vec<&'static str>,
}

/// Exports a canonical graph value suitable for byte-stable serialization.
///
/// r[impl valence.evidence_ir.queryable_graph.export]
pub fn export_evidence_graph(graph: &EvidenceGraph) -> Result<serde_json::Value, EvidenceModelError> {
    serde_json::to_value(graph.canonical()).map_err(|error| EvidenceModelError::CanonicalSerialization {
        detail: error.to_string(),
    })
}

/// Computes a deterministic graph identity.
///
/// r[impl valence.evidence_ir.queryable_graph.determinism]
pub fn evidence_graph_identity(graph: &EvidenceGraph) -> Result<String, EvidenceModelError> {
    hash_typed_value(&graph.canonical())
}

/// Validates graph shape, edges, and aliases.
///
/// r[impl valence.evidence_ir.queryable_graph.fail_closed]
#[must_use]
pub fn validate_evidence_graph(graph: &EvidenceGraph) -> EvidenceGraphReport {
    let mut issue_codes = Vec::new();
    if graph.schema != EVIDENCE_GRAPH_SCHEMA {
        issue_codes.push(ISSUE_SCHEMA_MISMATCH);
    }
    if !graph.non_claims.iter().any(|claim| claim == EVIDENCE_GRAPH_NON_CLAIM) {
        issue_codes.push(ISSUE_BOUNDARY_MISSING);
    }

    let mut node_ids = BTreeSet::new();
    for node in &graph.nodes {
        if !node_ids.insert(node.identity.clone()) {
            issue_codes.push(ISSUE_DUPLICATE_NODE);
        }
    }
    for edge in &graph.edges {
        if !node_ids.contains(&edge.source) || !node_ids.contains(&edge.target) {
            issue_codes.push(ISSUE_DANGLING_EDGE);
        }
    }

    let binding_report = validate_name_bindings(&graph.bindings, &node_ids);
    issue_codes.extend(binding_report.issue_codes);

    let mut issue_codes = sorted_vec(issue_codes);
    issue_codes.dedup();
    EvidenceGraphReport {
        valid: issue_codes.is_empty(),
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        issue_codes,
    }
}

/// Returns dependency closure identities from a root node.
///
/// r[impl valence.evidence_ir.queryable_graph.queries]
#[must_use]
pub fn dependency_closure(graph: &EvidenceGraph, root_identity: &str) -> Vec<String> {
    traverse_edges(graph, root_identity, EvidenceGraphEdgeKind::DependsOn, TraversalDirection::Forward)
}

/// Returns reverse dependencies of a node.
#[must_use]
pub fn reverse_dependencies(graph: &EvidenceGraph, identity: &str) -> Vec<String> {
    traverse_edges(graph, identity, EvidenceGraphEdgeKind::DependsOn, TraversalDirection::Reverse)
}

/// Returns identities covering a requirement node.
#[must_use]
pub fn requirement_coverage(graph: &EvidenceGraph, requirement_identity: &str) -> Vec<String> {
    direct_sources_for_target(graph, requirement_identity, EvidenceGraphEdgeKind::CoversRequirement)
}

/// Returns source identities traced to a binary.
#[must_use]
pub fn binary_traceability(graph: &EvidenceGraph, binary_identity: &str) -> Vec<String> {
    let mut traced = direct_sources_for_target(graph, binary_identity, EvidenceGraphEdgeKind::BuiltAsBinary);
    traced.extend(direct_sources_for_target(graph, binary_identity, EvidenceGraphEdgeKind::TracesTo));
    sorted_dedup(traced)
}

/// Returns evidence identities produced by a verifier.
#[must_use]
pub fn verifier_evidence(graph: &EvidenceGraph, verifier_identity: &str) -> Vec<String> {
    direct_sources_for_target(graph, verifier_identity, EvidenceGraphEdgeKind::ProducedByVerifier)
}

/// Resolves an alias through the graph's binding history.
#[must_use]
pub fn resolve_graph_alias(graph: &EvidenceGraph, alias: &str) -> BindingResolution {
    resolve_name_binding(&graph.bindings, alias)
}

/// Capability labels for verifier effect receipts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifierCapabilityKind {
    Pure,
    Filesystem,
    Clock,
    Process,
    Network,
    RemoteBuilder,
    Secret,
    LocalMachine,
    Unknown,
}

/// A typed verifier capability label.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VerifierCapabilityLabel {
    pub kind: VerifierCapabilityKind,
    pub detail: Option<String>,
}

impl VerifierCapabilityLabel {
    /// Creates a capability label.
    #[must_use]
    pub fn new(kind: VerifierCapabilityKind) -> Self {
        Self { kind, detail: None }
    }

    /// Creates an unknown capability label with source detail.
    #[must_use]
    pub fn unknown(detail: impl Into<String>) -> Self {
        Self {
            kind: VerifierCapabilityKind::Unknown,
            detail: Some(detail.into()),
        }
    }

    fn canonical_label(&self) -> String {
        match (&self.kind, &self.detail) {
            (VerifierCapabilityKind::Pure, _) => "pure".to_owned(),
            (VerifierCapabilityKind::Filesystem, _) => "filesystem".to_owned(),
            (VerifierCapabilityKind::Clock, _) => "clock".to_owned(),
            (VerifierCapabilityKind::Process, _) => "process".to_owned(),
            (VerifierCapabilityKind::Network, _) => "network".to_owned(),
            (VerifierCapabilityKind::RemoteBuilder, _) => "remote_builder".to_owned(),
            (VerifierCapabilityKind::Secret, _) => "secret".to_owned(),
            (VerifierCapabilityKind::LocalMachine, _) => "local_machine".to_owned(),
            (VerifierCapabilityKind::Unknown, Some(detail)) => format!("unknown:{detail}"),
            (VerifierCapabilityKind::Unknown, None) => "unknown".to_owned(),
        }
    }
}

/// Deterministic verifier capability set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierCapabilitySet {
    pub schema: String,
    pub capabilities: Vec<VerifierCapabilityLabel>,
    pub non_claims: Vec<String>,
}

impl VerifierCapabilitySet {
    /// Creates a sorted and deduplicated capability set.
    #[must_use]
    pub fn new(capabilities: Vec<VerifierCapabilityLabel>) -> Self {
        let mut capabilities = capabilities;
        capabilities.sort_by_key(VerifierCapabilityLabel::canonical_label);
        capabilities.dedup();
        Self {
            schema: VERIFIER_CAPABILITY_SET_SCHEMA.to_owned(),
            capabilities,
            non_claims: vec![CAPABILITY_NON_CLAIM.to_owned()],
        }
    }

    /// True when the set contains only the pure capability.
    #[must_use]
    pub fn is_pure_only(&self) -> bool {
        self.capabilities.iter().all(|capability| capability.kind == VerifierCapabilityKind::Pure)
            && !self.capabilities.is_empty()
    }
}

/// Capability policy for gate/cache decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityPolicy {
    pub denied: Vec<VerifierCapabilityLabel>,
    pub replay_safe: Vec<VerifierCapabilityLabel>,
}

impl CapabilityPolicy {
    /// Creates a sorted policy.
    #[must_use]
    pub fn new(denied: Vec<VerifierCapabilityLabel>, replay_safe: Vec<VerifierCapabilityLabel>) -> Self {
        Self {
            denied: sorted_capabilities(denied),
            replay_safe: sorted_capabilities(replay_safe),
        }
    }

    fn denies(&self, capability: &VerifierCapabilityLabel) -> bool {
        self.denied.contains(capability)
    }

    fn marks_replay_safe(&self, capability: &VerifierCapabilityLabel) -> bool {
        self.replay_safe.contains(capability)
    }
}

/// Capability policy report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPolicyReport {
    pub valid: bool,
    pub pure_cache_eligible: bool,
    pub replay_safe: bool,
    pub issue_codes: Vec<&'static str>,
}

/// Evaluates verifier capabilities against a policy.
///
/// r[impl valence.evidence_ir.verifier_capabilities.policy]
#[must_use]
pub fn evaluate_capability_policy(
    capabilities: &VerifierCapabilitySet,
    policy: &CapabilityPolicy,
) -> CapabilityPolicyReport {
    let mut issue_codes = Vec::new();
    if capabilities.schema != VERIFIER_CAPABILITY_SET_SCHEMA {
        issue_codes.push(ISSUE_SCHEMA_MISMATCH);
    }
    if !capabilities.non_claims.iter().any(|claim| claim == CAPABILITY_NON_CLAIM) {
        issue_codes.push(ISSUE_BOUNDARY_MISSING);
    }
    for capability in &capabilities.capabilities {
        if capability.kind == VerifierCapabilityKind::Unknown {
            issue_codes.push(ISSUE_UNKNOWN_CAPABILITY);
        }
        if policy.denies(capability) {
            issue_codes.push(ISSUE_CAPABILITY_DENIED);
        }
    }

    let replay_safe = issue_codes.is_empty()
        && capabilities
            .capabilities
            .iter()
            .all(|capability| capability.kind == VerifierCapabilityKind::Pure || policy.marks_replay_safe(capability));
    let pure_cache_eligible = issue_codes.is_empty() && capabilities.is_pure_only();

    CapabilityPolicyReport {
        valid: issue_codes.is_empty(),
        pure_cache_eligible,
        replay_safe,
        issue_codes: sorted_dedup(issue_codes),
    }
}

/// Computes a deterministic capability set identity.
///
/// r[impl valence.evidence_ir.verifier_capabilities.serialization]
pub fn capability_set_identity(capabilities: &VerifierCapabilitySet) -> Result<String, EvidenceModelError> {
    hash_typed_value(capabilities)
}

/// Verifier cache key over all reuse-relevant identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierCacheKey {
    pub schema: String,
    pub canonical_input_hash: String,
    pub verifier_identity: String,
    pub policy_hash: String,
    pub dependency_closure_hash: String,
    pub capability_set_hash: String,
    pub cache_schema_version: String,
}

impl VerifierCacheKey {
    /// Builds a cache key from identity inputs.
    ///
    /// r[impl valence.evidence_ir.dependency_closure_cache]
    #[must_use]
    pub fn new(
        canonical_input_hash: impl Into<String>,
        verifier_identity: impl Into<String>,
        policy_hash: impl Into<String>,
        dependency_closure_hash: impl Into<String>,
        capability_set_hash: impl Into<String>,
    ) -> Self {
        Self {
            schema: VERIFIER_CACHE_KEY_SCHEMA.to_owned(),
            canonical_input_hash: canonical_input_hash.into(),
            verifier_identity: verifier_identity.into(),
            policy_hash: policy_hash.into(),
            dependency_closure_hash: dependency_closure_hash.into(),
            capability_set_hash: capability_set_hash.into(),
            cache_schema_version: CURRENT_CACHE_SCHEMA_VERSION.to_owned(),
        }
    }
}

/// Cached verifier result metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedVerifierResult {
    pub key: VerifierCacheKey,
    pub result_hash: String,
}

/// Cache decision kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheDecisionKind {
    Hit,
    Miss,
    Blocked,
}

/// Deterministic cache decision report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheDecisionReport {
    pub decision: CacheDecisionKind,
    pub issue_codes: Vec<&'static str>,
}

/// Computes the hash identity of a cache key.
pub fn cache_key_identity(key: &VerifierCacheKey) -> Result<String, EvidenceModelError> {
    hash_typed_value(key)
}

/// Decides whether a cached result may be reused.
///
/// r[impl valence.evidence_ir.dependency_closure_cache.decisions]
#[must_use]
pub fn decide_cache_reuse(
    cached: Option<&CachedVerifierResult>,
    current_key: &VerifierCacheKey,
    capabilities: &VerifierCapabilitySet,
    policy: &CapabilityPolicy,
) -> CacheDecisionReport {
    let capability_report = evaluate_capability_policy(capabilities, policy);
    if !capability_report.replay_safe {
        let mut issue_codes = capability_report.issue_codes;
        if issue_codes.is_empty() {
            issue_codes.push(ISSUE_CAPABILITY_NOT_REPLAY_SAFE);
        }
        return CacheDecisionReport {
            decision: CacheDecisionKind::Blocked,
            issue_codes: sorted_dedup(issue_codes),
        };
    }

    let Some(cached) = cached else {
        return CacheDecisionReport {
            decision: CacheDecisionKind::Miss,
            issue_codes: vec![ISSUE_CACHE_ENTRY_MISSING],
        };
    };

    if &cached.key == current_key {
        return CacheDecisionReport {
            decision: CacheDecisionKind::Hit,
            issue_codes: Vec::new(),
        };
    }

    CacheDecisionReport {
        decision: CacheDecisionKind::Miss,
        issue_codes: changed_cache_key_issues(&cached.key, current_key),
    }
}

/// Transcript verifier actions supported by the pure runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptVerifierAction {
    EvidenceIrCheck,
    CapabilityPolicyCheck,
    StackProvenanceBuiltBinaryCheck,
}

/// Expected report shape for transcript scenarios.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptExpectedReport {
    pub valid: bool,
    pub issue_codes: Vec<String>,
}

/// Executable receipt transcript fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReceiptTranscript {
    pub schema: String,
    pub scenario_identity: String,
    pub inputs: serde_json::Value,
    pub verifier_action: TranscriptVerifierAction,
    pub expected_report: TranscriptExpectedReport,
    pub expected_hashes: BTreeMap<String, String>,
    pub non_claims: Vec<String>,
}

impl ReceiptTranscript {
    /// Creates a transcript fixture with deterministic expected issue codes.
    #[must_use]
    pub fn new(
        scenario_identity: impl Into<String>,
        inputs: serde_json::Value,
        verifier_action: TranscriptVerifierAction,
        expected_report: TranscriptExpectedReport,
        expected_hashes: BTreeMap<String, String>,
    ) -> Self {
        Self {
            schema: RECEIPT_TRANSCRIPT_SCHEMA.to_owned(),
            scenario_identity: scenario_identity.into(),
            inputs,
            verifier_action,
            expected_report: TranscriptExpectedReport {
                valid: expected_report.valid,
                issue_codes: sorted_vec(expected_report.issue_codes),
            },
            expected_hashes,
            non_claims: vec![TRANSCRIPT_NON_CLAIM.to_owned()],
        }
    }
}

/// Transcript evaluation report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptEvaluationReport {
    pub valid: bool,
    pub observed_valid: bool,
    pub observed_issue_codes: Vec<String>,
    pub actual_hashes: BTreeMap<String, String>,
    pub issue_codes: Vec<&'static str>,
}

/// Evaluates a pure transcript scenario.
///
/// r[impl valence.evidence_ir.executable_transcripts.runner]
#[must_use]
pub fn evaluate_receipt_transcript(transcript: &ReceiptTranscript) -> TranscriptEvaluationReport {
    let mut shape_issues = Vec::new();
    if transcript.schema != RECEIPT_TRANSCRIPT_SCHEMA {
        shape_issues.push(ISSUE_SCHEMA_MISMATCH);
    }
    if !transcript.non_claims.iter().any(|claim| claim == TRANSCRIPT_NON_CLAIM) {
        shape_issues.push(ISSUE_BOUNDARY_MISSING);
    }
    if transcript.scenario_identity.trim().is_empty() {
        shape_issues.push(ISSUE_MISSING_IDENTITY);
    }
    if !shape_issues.is_empty() {
        return TranscriptEvaluationReport {
            valid: false,
            observed_valid: false,
            observed_issue_codes: Vec::new(),
            actual_hashes: BTreeMap::new(),
            issue_codes: sorted_dedup(shape_issues),
        };
    }

    let observed = run_transcript_action(transcript);
    let mut issue_codes = Vec::new();
    if observed.valid != transcript.expected_report.valid {
        issue_codes.push(ISSUE_REPORT_VALID_MISMATCH);
    }
    let expected_issues = sorted_vec(transcript.expected_report.issue_codes.clone());
    let observed_issues = sorted_vec(observed.issue_codes.clone());
    if observed_issues != expected_issues {
        issue_codes.push(ISSUE_ISSUE_CODES_MISMATCH);
    }
    for (key, expected_hash) in &transcript.expected_hashes {
        match observed.hashes.get(key) {
            Some(actual_hash) if actual_hash == expected_hash => {}
            Some(_) => issue_codes.push(ISSUE_EXPECTED_HASH_MISMATCH),
            None => issue_codes.push(ISSUE_EXPECTED_HASH_MISSING),
        }
    }

    TranscriptEvaluationReport {
        valid: issue_codes.is_empty(),
        observed_valid: observed.valid,
        observed_issue_codes: observed_issues,
        actual_hashes: observed.hashes,
        issue_codes: sorted_dedup(issue_codes),
    }
}

/// Plan to refresh transcript expectations. This is explicit and returns a
/// reviewable diff instead of mutating fixtures during validation.
#[must_use]
pub fn plan_transcript_refresh(transcript: &ReceiptTranscript) -> BTreeMap<String, String> {
    evaluate_receipt_transcript(transcript).actual_hashes
}

/// Mutation surfaces a structured refactor may request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefactoringMutationSurface {
    AliasIndex,
    ReceiptMetadata,
    RequirementBinding,
    FilesystemPath,
    SourceRewrite,
}

/// Refactoring policy constraints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactoringPolicyConstraints {
    pub policy_identity: String,
    pub allowed_mutation_surfaces: Vec<RefactoringMutationSurface>,
    pub requires_semantic_equivalence_claim: bool,
}

impl RefactoringPolicyConstraints {
    /// Creates deterministic constraints.
    #[must_use]
    pub fn new(
        policy_identity: impl Into<String>,
        allowed_mutation_surfaces: Vec<RefactoringMutationSurface>,
        requires_semantic_equivalence_claim: bool,
    ) -> Self {
        Self {
            policy_identity: policy_identity.into(),
            allowed_mutation_surfaces: sorted_vec(allowed_mutation_surfaces),
            requires_semantic_equivalence_claim,
        }
    }
}

/// Typed refactoring session input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactoringSession {
    pub schema: String,
    pub session_identity: String,
    pub source_bindings: Vec<EvidenceNameBinding>,
    pub target_bindings: Vec<EvidenceNameBinding>,
    pub intended_content_identities: Vec<String>,
    pub requested_mutation_surfaces: Vec<RefactoringMutationSurface>,
    pub policy: RefactoringPolicyConstraints,
    pub non_claims: Vec<String>,
}

impl RefactoringSession {
    /// Creates a deterministic typed refactoring session.
    #[must_use]
    pub fn new(
        session_identity: impl Into<String>,
        source_bindings: Vec<EvidenceNameBinding>,
        target_bindings: Vec<EvidenceNameBinding>,
        intended_content_identities: Vec<String>,
        requested_mutation_surfaces: Vec<RefactoringMutationSurface>,
        policy: RefactoringPolicyConstraints,
    ) -> Self {
        Self {
            schema: REFACTORING_SESSION_SCHEMA.to_owned(),
            session_identity: session_identity.into(),
            source_bindings: sorted_vec(source_bindings),
            target_bindings: sorted_vec(target_bindings),
            intended_content_identities: sorted_vec(intended_content_identities),
            requested_mutation_surfaces: sorted_vec(requested_mutation_surfaces),
            policy,
            non_claims: vec![REFACTORING_NON_CLAIM.to_owned()],
        }
    }
}

/// Structured refactoring plan report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefactoringPlanReport {
    pub blocked: bool,
    pub affected_nodes: Vec<String>,
    pub affected_edges: Vec<EvidenceGraphEdge>,
    pub affected_aliases: Vec<String>,
    pub affected_receipts: Vec<String>,
    pub mutation_surfaces: Vec<RefactoringMutationSurface>,
    pub issue_codes: Vec<&'static str>,
}

/// Migration receipt for a planned/executed structured refactor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactoringMigrationReceipt {
    pub before_identities: Vec<String>,
    pub after_identities: Vec<String>,
    pub binding_changes: Vec<EvidenceNameBinding>,
    pub policy_identity: String,
    pub mutation_diagnostics: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Builds a dry-run refactoring plan over an in-memory graph.
///
/// r[impl valence.evidence_ir.structured_refactoring.plan]
/// r[impl valence.evidence_ir.structured_refactoring.blockers]
#[must_use]
pub fn plan_refactoring_session(session: &RefactoringSession, graph: &EvidenceGraph) -> RefactoringPlanReport {
    let mut issue_codes = Vec::new();
    if session.schema != REFACTORING_SESSION_SCHEMA {
        issue_codes.push(ISSUE_SCHEMA_MISMATCH);
    }
    if !session.non_claims.iter().any(|claim| claim == REFACTORING_NON_CLAIM) {
        issue_codes.push(ISSUE_BOUNDARY_MISSING);
    }
    if session.policy.requires_semantic_equivalence_claim {
        issue_codes.push(ISSUE_SEMANTIC_EQUIVALENCE_REQUIRED);
    }
    for surface in &session.requested_mutation_surfaces {
        if !session.policy.allowed_mutation_surfaces.contains(surface) {
            issue_codes.push(ISSUE_UNSUPPORTED_MUTATION_SURFACE);
        }
    }

    let graph_report = validate_evidence_graph(graph);
    if !graph_report.valid {
        issue_codes.push(ISSUE_GRAPH_INVALID);
        if graph_report.issue_codes.contains(&ISSUE_DANGLING_EDGE) {
            issue_codes.push(ISSUE_DANGLING_EDGE);
        }
        if graph_report.issue_codes.contains(&ISSUE_ALIAS_AMBIGUOUS) {
            issue_codes.push(ISSUE_ALIAS_AMBIGUOUS);
        }
    }

    let node_ids: BTreeSet<String> = graph.nodes.iter().map(|node| node.identity.clone()).collect();
    for identity in &session.intended_content_identities {
        if !node_ids.contains(identity) {
            issue_codes.push(ISSUE_MISSING_IDENTITY);
        }
    }

    let binding_names = session
        .source_bindings
        .iter()
        .chain(session.target_bindings.iter())
        .map(|binding| binding.name.clone())
        .collect::<BTreeSet<_>>();
    for name in &binding_names {
        let resolution = resolve_graph_alias(graph, name);
        if resolution.issue_codes.contains(&ISSUE_ALIAS_AMBIGUOUS) {
            issue_codes.push(ISSUE_ALIAS_AMBIGUOUS);
        }
    }

    let affected_nodes = session
        .intended_content_identities
        .iter()
        .filter(|identity| node_ids.contains(*identity))
        .cloned()
        .collect::<Vec<_>>();
    let affected_node_set: BTreeSet<String> = affected_nodes.iter().cloned().collect();
    let affected_edges = graph
        .edges
        .iter()
        .filter(|edge| affected_node_set.contains(&edge.source) || affected_node_set.contains(&edge.target))
        .cloned()
        .collect::<Vec<_>>();
    let affected_receipts = graph
        .nodes
        .iter()
        .filter(|node| node.kind == EvidenceGraphNodeKind::Receipt && affected_node_set.contains(&node.identity))
        .map(|node| node.identity.clone())
        .collect::<Vec<_>>();

    RefactoringPlanReport {
        blocked: !issue_codes.is_empty(),
        affected_nodes: sorted_vec(affected_nodes),
        affected_edges: sorted_vec(affected_edges),
        affected_aliases: sorted_vec(binding_names.into_iter().collect()),
        affected_receipts: sorted_vec(affected_receipts),
        mutation_surfaces: sorted_vec(session.requested_mutation_surfaces.clone()),
        issue_codes: sorted_dedup(issue_codes),
    }
}

/// Durable storage codec identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurableStorageCodec {
    CanonicalJson,
}

/// Typed durable evidence storage envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DurableEvidenceEnvelope {
    pub schema: String,
    pub value_type: String,
    pub codec: DurableStorageCodec,
    pub content_hash: String,
    pub value: serde_json::Value,
    pub migrations: Vec<DurableMigrationReceipt>,
    pub non_claims: Vec<String>,
}

/// Durable value migration receipt.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DurableMigrationReceipt {
    pub source_identity: String,
    pub target_identity: String,
    pub migration_tool_identity: String,
    pub diagnostics: Vec<String>,
    pub non_claims: Vec<String>,
}

impl DurableMigrationReceipt {
    /// Creates deterministic migration metadata.
    #[must_use]
    pub fn new(
        source_identity: impl Into<String>,
        target_identity: impl Into<String>,
        migration_tool_identity: impl Into<String>,
        diagnostics: Vec<String>,
    ) -> Self {
        Self {
            source_identity: source_identity.into(),
            target_identity: target_identity.into(),
            migration_tool_identity: migration_tool_identity.into(),
            diagnostics: sorted_vec(diagnostics),
            non_claims: vec![DURABLE_STORAGE_NON_CLAIM.to_owned()],
        }
    }
}

/// Durable decode report that only carries a value after validation passes.
#[derive(Debug, Clone, PartialEq)]
pub struct DurableDecodeReport {
    pub valid: bool,
    pub value: Option<serde_json::Value>,
    pub issue_codes: Vec<&'static str>,
}

/// Encodes a value into a typed durable storage envelope.
///
/// r[impl valence.evidence_ir.typed_durable_storage]
pub fn encode_durable_value(
    value_type: impl Into<String>,
    codec: DurableStorageCodec,
    value: serde_json::Value,
    migrations: Vec<DurableMigrationReceipt>,
) -> Result<DurableEvidenceEnvelope, EvidenceModelError> {
    let content_hash = hash_typed_value(&value)?;
    Ok(DurableEvidenceEnvelope {
        schema: DURABLE_STORAGE_SCHEMA.to_owned(),
        value_type: value_type.into(),
        codec,
        content_hash,
        value,
        migrations: sorted_vec(migrations),
        non_claims: vec![DURABLE_STORAGE_NON_CLAIM.to_owned()],
    })
}

/// Validates and decodes a durable value envelope before exposing the DTO.
///
/// r[impl valence.evidence_ir.typed_durable_storage.decode]
#[must_use]
pub fn decode_durable_value(
    envelope: &DurableEvidenceEnvelope,
    expected_value_type: &str,
    expected_codec: DurableStorageCodec,
    migration_required: bool,
) -> DurableDecodeReport {
    let mut issue_codes = Vec::new();
    if envelope.schema != DURABLE_STORAGE_SCHEMA {
        issue_codes.push(ISSUE_SCHEMA_MISMATCH);
    }
    if envelope.value_type != expected_value_type {
        issue_codes.push(ISSUE_TYPE_MISMATCH);
    }
    if envelope.codec != expected_codec {
        issue_codes.push(ISSUE_CODEC_MISMATCH);
    }
    if !envelope.non_claims.iter().any(|claim| claim == DURABLE_STORAGE_NON_CLAIM) {
        issue_codes.push(ISSUE_BOUNDARY_MISSING);
    }
    if migration_required && envelope.migrations.is_empty() {
        issue_codes.push(ISSUE_MIGRATION_MISSING);
    }
    match hash_typed_value(&envelope.value) {
        Ok(hash) if hash == envelope.content_hash => {}
        Ok(_) | Err(_) => issue_codes.push(ISSUE_CONTENT_HASH_MISMATCH),
    }

    let issue_codes = sorted_dedup(issue_codes);
    DurableDecodeReport {
        valid: issue_codes.is_empty(),
        value: if issue_codes.is_empty() {
            Some(envelope.value.clone())
        } else {
            None
        },
        issue_codes,
    }
}

#[derive(Clone, Copy)]
enum CollectionCardinality {
    None,
    One,
}

impl CollectionCardinality {
    const fn as_usize(self) -> usize {
        match self {
            Self::None => 0,
            Self::One => 1,
        }
    }
}

#[derive(Clone, Copy)]
enum TraversalDirection {
    Forward,
    Reverse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedTranscriptReport {
    valid: bool,
    issue_codes: Vec<String>,
    hashes: BTreeMap<String, String>,
}

fn run_transcript_action(transcript: &ReceiptTranscript) -> ObservedTranscriptReport {
    match transcript.verifier_action {
        TranscriptVerifierAction::EvidenceIrCheck => run_evidence_ir_transcript(transcript),
        TranscriptVerifierAction::CapabilityPolicyCheck => run_capability_policy_transcript(transcript),
        TranscriptVerifierAction::StackProvenanceBuiltBinaryCheck => run_stack_provenance_transcript(transcript),
    }
}

fn run_evidence_ir_transcript(transcript: &ReceiptTranscript) -> ObservedTranscriptReport {
    let Some(artifact_json) = transcript.inputs.get("artifact_json").and_then(serde_json::Value::as_str) else {
        return observed_report(false, vec![ISSUE_INPUT_MISSING.to_owned()], BTreeMap::new());
    };
    let report = check_evidence_artifact(artifact_json);
    let mut hashes = BTreeMap::new();
    hashes.insert("artifact_json".to_owned(), hash_str(artifact_json));
    observed_report(report.valid, report.issue_codes.into_iter().map(ToOwned::to_owned).collect(), hashes)
}

fn run_capability_policy_transcript(transcript: &ReceiptTranscript) -> ObservedTranscriptReport {
    let Some(capabilities_value) = transcript.inputs.get("capabilities") else {
        return observed_report(false, vec![ISSUE_INPUT_MISSING.to_owned()], BTreeMap::new());
    };
    let Some(policy_value) = transcript.inputs.get("policy") else {
        return observed_report(false, vec![ISSUE_INPUT_MISSING.to_owned()], BTreeMap::new());
    };
    let Ok(capabilities) = serde_json::from_value::<VerifierCapabilitySet>(capabilities_value.clone()) else {
        return observed_report(false, vec![ISSUE_MALFORMED_INPUT.to_owned()], BTreeMap::new());
    };
    let Ok(policy) = serde_json::from_value::<CapabilityPolicy>(policy_value.clone()) else {
        return observed_report(false, vec![ISSUE_MALFORMED_INPUT.to_owned()], BTreeMap::new());
    };
    let report = evaluate_capability_policy(&capabilities, &policy);
    let mut hashes = BTreeMap::new();
    let capability_hash = capability_set_identity(&capabilities).unwrap_or_else(|_| hash_str(EMPTY_HASH_INPUT));
    hashes.insert("capabilities".to_owned(), capability_hash);
    observed_report(report.valid, report.issue_codes.into_iter().map(ToOwned::to_owned).collect(), hashes)
}

fn run_stack_provenance_transcript(transcript: &ReceiptTranscript) -> ObservedTranscriptReport {
    let Some(binary_bytes) = transcript.inputs.get("binary_bytes").and_then(serde_json::Value::as_str) else {
        return observed_report(false, vec![ISSUE_INPUT_MISSING.to_owned()], BTreeMap::new());
    };
    let Some(octet_export_json) = transcript.inputs.get("octet_export_json").and_then(serde_json::Value::as_str) else {
        return observed_report(false, vec![ISSUE_INPUT_MISSING.to_owned()], BTreeMap::new());
    };
    let Some(sidecar_json) = transcript.inputs.get("sidecar_json").and_then(serde_json::Value::as_str) else {
        return observed_report(false, vec![ISSUE_INPUT_MISSING.to_owned()], BTreeMap::new());
    };
    let Some(mantle_manifest_json) = transcript.inputs.get("mantle_manifest_json").and_then(serde_json::Value::as_str)
    else {
        return observed_report(false, vec![ISSUE_INPUT_MISSING.to_owned()], BTreeMap::new());
    };
    let report =
        verify_built_binary_thread_json(binary_bytes.as_bytes(), octet_export_json, sidecar_json, mantle_manifest_json);
    let mut hashes = BTreeMap::new();
    hashes.insert("binary_hash".to_owned(), report.binary_hash.clone());
    hashes.insert("sidecar_digest_blake3".to_owned(), report.sidecar_digest_blake3.clone());
    observed_report(report.valid, report.issue_codes, hashes)
}

fn observed_report(
    valid: bool,
    issue_codes: Vec<String>,
    hashes: BTreeMap<String, String>,
) -> ObservedTranscriptReport {
    ObservedTranscriptReport {
        valid,
        issue_codes: sorted_vec(issue_codes),
        hashes,
    }
}

fn changed_cache_key_issues(cached: &VerifierCacheKey, current: &VerifierCacheKey) -> Vec<&'static str> {
    let mut issues = Vec::new();
    if cached.schema != current.schema {
        issues.push(ISSUE_SCHEMA_MISMATCH);
    }
    if cached.canonical_input_hash != current.canonical_input_hash
        || cached.verifier_identity != current.verifier_identity
        || cached.policy_hash != current.policy_hash
        || cached.dependency_closure_hash != current.dependency_closure_hash
        || cached.capability_set_hash != current.capability_set_hash
        || cached.cache_schema_version != current.cache_schema_version
    {
        issues.push(ISSUE_CACHE_FIELD_CHANGED);
    }
    sorted_dedup(issues)
}

fn current_binding_names(history: &NameBindingHistory) -> BTreeSet<String> {
    history
        .bindings
        .iter()
        .filter(|binding| binding.current)
        .map(|binding| binding.name.clone())
        .collect()
}

fn traverse_edges(
    graph: &EvidenceGraph,
    root_identity: &str,
    kind: EvidenceGraphEdgeKind,
    direction: TraversalDirection,
) -> Vec<String> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([root_identity.to_owned()]);
    while let Some(identity) = queue.pop_front() {
        for next in direct_neighbors(graph, &identity, kind, direction) {
            if visited.insert(next.clone()) {
                queue.push_back(next);
            }
        }
    }
    visited.remove(root_identity);
    visited.into_iter().collect()
}

fn direct_neighbors(
    graph: &EvidenceGraph,
    identity: &str,
    kind: EvidenceGraphEdgeKind,
    direction: TraversalDirection,
) -> Vec<String> {
    let neighbors = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == kind)
        .filter_map(|edge| match direction {
            TraversalDirection::Forward if edge.source == identity => Some(edge.target.clone()),
            TraversalDirection::Reverse if edge.target == identity => Some(edge.source.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    sorted_dedup(neighbors)
}

fn direct_sources_for_target(graph: &EvidenceGraph, target: &str, kind: EvidenceGraphEdgeKind) -> Vec<String> {
    sorted_dedup(
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == kind && edge.target == target)
            .map(|edge| edge.source.clone())
            .collect::<Vec<_>>(),
    )
}

fn sorted_capabilities(mut capabilities: Vec<VerifierCapabilityLabel>) -> Vec<VerifierCapabilityLabel> {
    capabilities.sort_by_key(VerifierCapabilityLabel::canonical_label);
    capabilities.dedup();
    capabilities
}

fn report_from_issues(issue_codes: Vec<&'static str>) -> ModelCheckReport {
    let issue_codes = sorted_dedup(issue_codes);
    ModelCheckReport {
        valid: issue_codes.is_empty(),
        issue_codes,
    }
}

fn sorted_vec<T>(mut values: Vec<T>) -> Vec<T>
where T: Ord {
    values.sort();
    values
}

fn sorted_dedup<T>(mut values: Vec<T>) -> Vec<T>
where T: Ord {
    values.sort();
    values.dedup();
    values
}

fn hash_typed_value<T>(value: &T) -> Result<String, EvidenceModelError>
where T: Serialize {
    let bytes = serde_json::to_vec(value).map_err(|error| EvidenceModelError::CanonicalSerialization {
        detail: error.to_string(),
    })?;
    Ok(hash_bytes(&bytes))
}

fn hash_str(value: &str) -> String {
    hash_bytes(value.as_bytes())
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{}{}", BLAKE3_IDENTITY_PREFIX, blake3::hash(bytes).to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_ir::REQUIRED_NON_CLAIM;
    use crate::evidence_ir::VerificationRole;
    use crate::evidence_ir::build_evidence_artifact;

    const DEFINITION_KIND: &str = "function-object";
    const IDENTITY_ALPHA: &str = "b3:alpha";
    const IDENTITY_BETA: &str = "b3:beta";
    const IDENTITY_GAMMA: &str = "b3:gamma";
    const REQUIREMENT_IDENTITY: &str = "r[valence.example]";
    const VERIFIER_IDENTITY: &str = "b3:verifier";
    const RECEIPT_IDENTITY: &str = "b3:receipt";
    const BINARY_IDENTITY: &str = "b3:binary";
    const SIDECAR_IDENTITY: &str = "b3:sidecar";
    const POLICY_IDENTITY: &str = "b3:policy";
    const CACHE_RESULT_IDENTITY: &str = "b3:result";
    const TYPE_IDENTITY: &str = "valence.example-value.v1";
    const MIGRATION_TOOL_IDENTITY: &str = "b3:migration-tool";
    const INITIAL_SEQUENCE: u64 = 1;
    const RENAMED_SEQUENCE: u64 = 2;
    const EXPECTED_NONE: usize = 0;
    const EXPECTED_ONE: usize = 1;
    const EXPECTED_TWO: usize = 2;

    fn body() -> serde_json::Value {
        serde_json::json!({
            "operation": "check",
            "predicate_hash": "b3:predicate",
        })
    }

    fn metadata(name: &str, path: &str) -> EvidenceDefinitionMetadata {
        EvidenceDefinitionMetadata::new(Some(name.to_owned()), Some(path.to_owned()), vec![name.to_owned()])
    }

    fn content_definition(name: &str, path: &str, dependency_identity: &str) -> EvidenceDefinition {
        build_evidence_definition(
            DEFINITION_KIND,
            body(),
            vec![EvidenceDependency::new("definition", dependency_identity)],
            metadata(name, path),
            vec![CONTENT_IDENTITY_NON_CLAIM.to_owned()],
        )
        .expect("definition builds")
    }

    fn binding(name: &str, identity: &str, sequence: u64, current: bool) -> EvidenceNameBinding {
        EvidenceNameBinding::new(NameBindingKind::Alias, name, identity, sequence, current)
    }

    fn graph() -> EvidenceGraph {
        let nodes = vec![
            EvidenceGraphNode::new(IDENTITY_ALPHA, EvidenceGraphNodeKind::Definition, Some("alpha".to_owned())),
            EvidenceGraphNode::new(IDENTITY_BETA, EvidenceGraphNodeKind::Artifact, Some("beta".to_owned())),
            EvidenceGraphNode::new(REQUIREMENT_IDENTITY, EvidenceGraphNodeKind::Requirement, None),
            EvidenceGraphNode::new(VERIFIER_IDENTITY, EvidenceGraphNodeKind::Verifier, None),
            EvidenceGraphNode::new(RECEIPT_IDENTITY, EvidenceGraphNodeKind::Receipt, None),
            EvidenceGraphNode::new(BINARY_IDENTITY, EvidenceGraphNodeKind::Binary, None),
            EvidenceGraphNode::new(SIDECAR_IDENTITY, EvidenceGraphNodeKind::Sidecar, None),
        ];
        let edges = vec![
            EvidenceGraphEdge::new(IDENTITY_ALPHA, IDENTITY_BETA, EvidenceGraphEdgeKind::DependsOn),
            EvidenceGraphEdge::new(IDENTITY_ALPHA, REQUIREMENT_IDENTITY, EvidenceGraphEdgeKind::CoversRequirement),
            EvidenceGraphEdge::new(IDENTITY_ALPHA, VERIFIER_IDENTITY, EvidenceGraphEdgeKind::ProducedByVerifier),
            EvidenceGraphEdge::new(IDENTITY_ALPHA, RECEIPT_IDENTITY, EvidenceGraphEdgeKind::RecordedInReceipt),
            EvidenceGraphEdge::new(IDENTITY_ALPHA, BINARY_IDENTITY, EvidenceGraphEdgeKind::BuiltAsBinary),
            EvidenceGraphEdge::new(IDENTITY_ALPHA, SIDECAR_IDENTITY, EvidenceGraphEdgeKind::HasSidecar),
        ];
        EvidenceGraph::new(
            nodes,
            edges,
            NameBindingHistory::new(vec![binding("alpha", IDENTITY_ALPHA, INITIAL_SEQUENCE, true)]),
        )
    }

    fn pure_capabilities() -> VerifierCapabilitySet {
        VerifierCapabilitySet::new(vec![VerifierCapabilityLabel::new(VerifierCapabilityKind::Pure)])
    }

    fn deny_network_policy() -> CapabilityPolicy {
        CapabilityPolicy::new(vec![VerifierCapabilityLabel::new(VerifierCapabilityKind::Network)], Vec::new())
    }

    fn cache_key() -> VerifierCacheKey {
        VerifierCacheKey::new(IDENTITY_ALPHA, VERIFIER_IDENTITY, POLICY_IDENTITY, IDENTITY_BETA, "b3:capabilities")
    }

    #[test]
    fn evidence_definition_identity_ignores_name_and_path_metadata() {
        let original = content_definition("alpha", "src/old.rs", IDENTITY_BETA);
        let renamed = content_definition("renamed", "src/new.rs", IDENTITY_BETA);
        assert_eq!(original.content_identity, renamed.content_identity);
        assert_eq!(validate_evidence_definition(&original).issue_codes.len(), EXPECTED_NONE);
    }

    #[test]
    fn evidence_definition_identity_changes_when_dependency_identity_changes() {
        let original = content_definition("alpha", "src/lib.rs", IDENTITY_BETA);
        let changed = content_definition("alpha", "src/lib.rs", IDENTITY_GAMMA);
        assert_ne!(original.content_identity, changed.content_identity);
    }

    #[test]
    fn evidence_definition_validation_fails_without_boundary_or_dependency_identity() {
        let definition = build_evidence_definition(
            DEFINITION_KIND,
            body(),
            vec![EvidenceDependency::new("definition", "name-not-content")],
            metadata("alpha", "src/lib.rs"),
            Vec::new(),
        )
        .expect("definition builds");
        let report = validate_evidence_definition(&definition);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&ISSUE_BOUNDARY_MISSING));
        assert!(report.issue_codes.contains(&ISSUE_MISSING_IDENTITY));
    }

    #[test]
    fn binding_history_resolves_alias_to_content_identity() {
        let history = NameBindingHistory::new(vec![binding("alpha", IDENTITY_ALPHA, INITIAL_SEQUENCE, true)]);
        let resolution = resolve_name_binding(&history, "alpha");
        assert!(resolution.valid());
        assert_eq!(resolution.identity.expect("resolved").value, IDENTITY_ALPHA);
    }

    #[test]
    fn binding_history_fails_closed_for_ambiguous_alias() {
        let history = NameBindingHistory::new(vec![
            binding("alpha", IDENTITY_ALPHA, INITIAL_SEQUENCE, true),
            binding("alpha", IDENTITY_BETA, RENAMED_SEQUENCE, true),
        ]);
        let resolution = resolve_name_binding(&history, "alpha");
        assert!(!resolution.valid());
        assert!(resolution.issue_codes.contains(&ISSUE_ALIAS_AMBIGUOUS));
    }

    #[test]
    fn binding_history_reports_stale_bindings() {
        let history = NameBindingHistory::new(vec![binding("missing", IDENTITY_GAMMA, INITIAL_SEQUENCE, true)]);
        let known = BTreeSet::from([IDENTITY_ALPHA.to_owned()]);
        let report = validate_name_bindings(&history, &known);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&ISSUE_BINDING_STALE));
    }

    #[test]
    fn graph_export_and_identity_are_stable_for_reordered_inputs() {
        let one = graph();
        let mut two = graph();
        two.nodes.reverse();
        two.edges.reverse();
        assert_eq!(evidence_graph_identity(&one).unwrap(), evidence_graph_identity(&two).unwrap());
        assert_eq!(export_evidence_graph(&one).unwrap(), export_evidence_graph(&two).unwrap());
    }

    #[test]
    fn graph_queries_return_deterministic_linkage() {
        let graph = graph();
        assert_eq!(dependency_closure(&graph, IDENTITY_ALPHA), vec![IDENTITY_BETA.to_owned()]);
        assert_eq!(reverse_dependencies(&graph, IDENTITY_BETA), vec![IDENTITY_ALPHA.to_owned()]);
        assert_eq!(requirement_coverage(&graph, REQUIREMENT_IDENTITY), vec![IDENTITY_ALPHA.to_owned()]);
        assert_eq!(binary_traceability(&graph, BINARY_IDENTITY), vec![IDENTITY_ALPHA.to_owned()]);
        assert_eq!(verifier_evidence(&graph, VERIFIER_IDENTITY), vec![IDENTITY_ALPHA.to_owned()]);
        assert_eq!(resolve_graph_alias(&graph, "alpha").identity.unwrap().value, IDENTITY_ALPHA);
    }

    #[test]
    fn graph_validation_fails_for_duplicate_nodes_dangling_edges_and_ambiguous_aliases() {
        let mut graph = graph();
        graph.nodes.push(EvidenceGraphNode::new(IDENTITY_ALPHA, EvidenceGraphNodeKind::Definition, None));
        graph
            .edges
            .push(EvidenceGraphEdge::new(IDENTITY_ALPHA, "b3:missing", EvidenceGraphEdgeKind::DependsOn));
        graph.bindings.bindings.push(binding("alpha", IDENTITY_BETA, RENAMED_SEQUENCE, true));
        let report = validate_evidence_graph(&graph);
        assert!(!report.valid);
        assert_eq!(report.node_count, graph.nodes.len());
        assert!(report.issue_codes.contains(&ISSUE_DUPLICATE_NODE));
        assert!(report.issue_codes.contains(&ISSUE_DANGLING_EDGE));
        assert!(report.issue_codes.contains(&ISSUE_ALIAS_AMBIGUOUS));
    }

    #[test]
    fn capability_sets_are_sorted_round_trippable_and_pure_cache_eligible() {
        let capabilities = VerifierCapabilitySet::new(vec![
            VerifierCapabilityLabel::new(VerifierCapabilityKind::Filesystem),
            VerifierCapabilityLabel::new(VerifierCapabilityKind::Pure),
        ]);
        let json = serde_json::to_string(&capabilities).expect("serialize capabilities");
        let round_tripped: VerifierCapabilitySet = serde_json::from_str(&json).expect("deserialize capabilities");
        assert_eq!(capabilities, round_tripped);
        assert_eq!(capabilities.capabilities.len(), EXPECTED_TWO);

        let report = evaluate_capability_policy(&pure_capabilities(), &deny_network_policy());
        assert!(report.valid);
        assert!(report.pure_cache_eligible);
    }

    #[test]
    fn capability_policy_fails_for_denied_or_unknown_capabilities() {
        let capabilities = VerifierCapabilitySet::new(vec![
            VerifierCapabilityLabel::new(VerifierCapabilityKind::Network),
            VerifierCapabilityLabel::unknown("gpu"),
        ]);
        let report = evaluate_capability_policy(&capabilities, &deny_network_policy());
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&ISSUE_CAPABILITY_DENIED));
        assert!(report.issue_codes.contains(&ISSUE_UNKNOWN_CAPABILITY));
    }

    #[test]
    fn cache_decision_hits_for_identical_replay_safe_keys() {
        let key = cache_key();
        let cached = CachedVerifierResult {
            key: key.clone(),
            result_hash: CACHE_RESULT_IDENTITY.to_owned(),
        };
        let decision = decide_cache_reuse(Some(&cached), &key, &pure_capabilities(), &deny_network_policy());
        assert_eq!(decision.decision, CacheDecisionKind::Hit);
        assert_eq!(decision.issue_codes.len(), EXPECTED_NONE);
    }

    #[test]
    fn cache_decision_misses_for_changed_key_fields() {
        let key = cache_key();
        let cached = CachedVerifierResult {
            key: key.clone(),
            result_hash: CACHE_RESULT_IDENTITY.to_owned(),
        };
        let mut changed = key;
        changed.policy_hash = "b3:other-policy".to_owned();
        let decision = decide_cache_reuse(Some(&cached), &changed, &pure_capabilities(), &deny_network_policy());
        assert_eq!(decision.decision, CacheDecisionKind::Miss);
        assert!(decision.issue_codes.contains(&ISSUE_CACHE_FIELD_CHANGED));
    }

    #[test]
    fn cache_decision_blocks_effectful_capabilities_by_default() {
        let capabilities =
            VerifierCapabilitySet::new(vec![VerifierCapabilityLabel::new(VerifierCapabilityKind::Filesystem)]);
        let decision = decide_cache_reuse(None, &cache_key(), &capabilities, &deny_network_policy());
        assert_eq!(decision.decision, CacheDecisionKind::Blocked);
        assert!(decision.issue_codes.contains(&ISSUE_CAPABILITY_NOT_REPLAY_SAFE));
    }

    #[test]
    fn evidence_ir_transcript_passes_when_expected_report_and_hash_match() {
        let artifact = build_evidence_artifact(
            "test",
            "id",
            None,
            VerificationRole::RecordedOnly,
            serde_json::json!({"claim": "identity only"}),
            vec![REQUIRED_NON_CLAIM.to_owned()],
        )
        .expect("artifact builds");
        let artifact_json = serde_json::to_string(&artifact).expect("artifact serializes");
        let mut expected_hashes = BTreeMap::new();
        expected_hashes.insert("artifact_json".to_owned(), hash_str(&artifact_json));
        let transcript = ReceiptTranscript::new(
            "valid-artifact",
            serde_json::json!({"artifact_json": artifact_json}),
            TranscriptVerifierAction::EvidenceIrCheck,
            TranscriptExpectedReport {
                valid: true,
                issue_codes: Vec::new(),
            },
            expected_hashes,
        );
        let report = evaluate_receipt_transcript(&transcript);
        assert!(report.valid);
        assert!(report.observed_valid);
    }

    #[test]
    fn transcript_negative_fixture_passes_when_expected_failure_matches() {
        let transcript = ReceiptTranscript::new(
            "malformed-artifact",
            serde_json::json!({"artifact_json": "{ not json"}),
            TranscriptVerifierAction::EvidenceIrCheck,
            TranscriptExpectedReport {
                valid: false,
                issue_codes: vec!["malformed_json".to_owned()],
            },
            BTreeMap::new(),
        );
        let report = evaluate_receipt_transcript(&transcript);
        assert!(report.valid);
        assert!(!report.observed_valid);
        assert_eq!(report.observed_issue_codes.len(), EXPECTED_ONE);
    }

    #[test]
    fn transcript_fails_when_expected_issue_codes_are_stale() {
        let transcript = ReceiptTranscript::new(
            "stale-expectation",
            serde_json::json!({"artifact_json": "{ not json"}),
            TranscriptVerifierAction::EvidenceIrCheck,
            TranscriptExpectedReport {
                valid: false,
                issue_codes: vec!["wrong_issue".to_owned()],
            },
            BTreeMap::new(),
        );
        let report = evaluate_receipt_transcript(&transcript);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&ISSUE_ISSUE_CODES_MISMATCH));
    }

    #[test]
    fn refactoring_plan_allows_identity_preserving_alias_migration() {
        let graph = graph();
        let session = RefactoringSession::new(
            "rename-alpha",
            vec![binding("alpha", IDENTITY_ALPHA, INITIAL_SEQUENCE, true)],
            vec![binding("renamed-alpha", IDENTITY_ALPHA, RENAMED_SEQUENCE, true)],
            vec![IDENTITY_ALPHA.to_owned()],
            vec![RefactoringMutationSurface::AliasIndex],
            RefactoringPolicyConstraints::new(POLICY_IDENTITY, vec![RefactoringMutationSurface::AliasIndex], false),
        );
        let plan = plan_refactoring_session(&session, &graph);
        assert!(!plan.blocked);
        assert_eq!(plan.affected_nodes, vec![IDENTITY_ALPHA.to_owned()]);
        assert!(plan.affected_aliases.contains(&"alpha".to_owned()));
    }

    #[test]
    fn refactoring_plan_blocks_ambiguous_dangling_or_semantic_equivalence_sessions() {
        let mut graph = graph();
        graph.bindings.bindings.push(binding("alpha", IDENTITY_BETA, RENAMED_SEQUENCE, true));
        graph
            .edges
            .push(EvidenceGraphEdge::new(IDENTITY_ALPHA, "b3:missing", EvidenceGraphEdgeKind::DependsOn));
        let session = RefactoringSession::new(
            "unsafe-rename",
            vec![binding("alpha", IDENTITY_ALPHA, INITIAL_SEQUENCE, true)],
            Vec::new(),
            vec!["b3:absent".to_owned()],
            vec![RefactoringMutationSurface::SourceRewrite],
            RefactoringPolicyConstraints::new(POLICY_IDENTITY, vec![RefactoringMutationSurface::AliasIndex], true),
        );
        let plan = plan_refactoring_session(&session, &graph);
        assert!(plan.blocked);
        assert!(plan.issue_codes.contains(&ISSUE_ALIAS_AMBIGUOUS));
        assert!(plan.issue_codes.contains(&ISSUE_DANGLING_EDGE));
        assert!(plan.issue_codes.contains(&ISSUE_MISSING_IDENTITY));
        assert!(plan.issue_codes.contains(&ISSUE_SEMANTIC_EQUIVALENCE_REQUIRED));
        assert!(plan.issue_codes.contains(&ISSUE_UNSUPPORTED_MUTATION_SURFACE));
    }

    #[test]
    fn durable_storage_decodes_valid_current_envelope() {
        let value = serde_json::json!({"identity": IDENTITY_ALPHA});
        let envelope =
            encode_durable_value(TYPE_IDENTITY, DurableStorageCodec::CanonicalJson, value.clone(), Vec::new())
                .expect("envelope encodes");
        let report = decode_durable_value(&envelope, TYPE_IDENTITY, DurableStorageCodec::CanonicalJson, false);
        assert!(report.valid);
        assert_eq!(report.value.expect("decoded"), value);
    }

    #[test]
    fn durable_storage_fails_closed_for_hash_type_codec_and_missing_migration() {
        let value = serde_json::json!({"identity": IDENTITY_ALPHA});
        let mut envelope = encode_durable_value(TYPE_IDENTITY, DurableStorageCodec::CanonicalJson, value, Vec::new())
            .expect("envelope encodes");
        envelope.content_hash = IDENTITY_BETA.to_owned();
        let report = decode_durable_value(&envelope, "other.type", DurableStorageCodec::CanonicalJson, true);
        assert!(!report.valid);
        assert!(report.value.is_none());
        assert!(report.issue_codes.contains(&ISSUE_CONTENT_HASH_MISMATCH));
        assert!(report.issue_codes.contains(&ISSUE_TYPE_MISMATCH));
        assert!(report.issue_codes.contains(&ISSUE_MIGRATION_MISSING));
    }

    #[test]
    fn durable_storage_accepts_supported_legacy_migration_metadata() {
        let value = serde_json::json!({"identity": IDENTITY_ALPHA});
        let migration = DurableMigrationReceipt::new(IDENTITY_BETA, IDENTITY_ALPHA, MIGRATION_TOOL_IDENTITY, vec![
            "legacy-json-promoted".to_owned(),
        ]);
        let envelope = encode_durable_value(TYPE_IDENTITY, DurableStorageCodec::CanonicalJson, value, vec![migration])
            .expect("envelope encodes");
        let report = decode_durable_value(&envelope, TYPE_IDENTITY, DurableStorageCodec::CanonicalJson, true);
        assert!(report.valid);
        assert_eq!(report.issue_codes.len(), EXPECTED_NONE);
    }
}
