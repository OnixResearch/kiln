//! Provenance threading: the unified ID chain from source to binary.
//!
//! This module connects the stable identifiers across the trust stack:
//!
//! - `EvidenceSpan.source_id` (valence) — where in source a function lives
//! - `FunctionObjectV1.semantic_id` (valence) — the function's review identity
//! - cairn `r[...]` requirement IDs — what the function is supposed to verify
//! - artifact hashes (cairn/mantle receipts) — the receipt that records the check
//! - binary hashes (mantle release-evidence) — the binary whose behavior is traced
//!
//! The chain answers "show me where this number comes from" mechanically:
//! binary → artifact_hash → requirement_id → function_object_id → source_span.
//! The chain preserves span linkage through the stack.
//!
//! # Non-claims
//!
//! Provenance threading proves identity and linkage only. It does not prove
//! behavioral correctness, semantic equivalence, or that the binary satisfies
//! the requirement. It proves that the binary's behavior is *traceable* to a
//! source function and a requirement, not that it is *correct*.

use serde::Deserialize;
use serde::Serialize;

/// The provenance chain schema identifier.
pub const PROVENANCE_CHAIN_SCHEMA: &str = "valence.provenance-chain.v1";

/// Required non-claim boundary for provenance chains.
pub const PROVENANCE_BOUNDARY: &str = "provenance chain proves identity and linkage only; it does not prove behavioral correctness, semantic equivalence, or that the binary satisfies the requirement";

/// A single link in the provenance chain connecting a source function to the
/// artifacts and requirements that trace it through to the binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceLink {
    /// The valence `EvidenceSpan.source_id` — where in source the function lives.
    pub source_id: String,
    /// The valence `FunctionObjectV1.semantic_id` — the function's review identity.
    pub function_object_id: String,
    /// The cairn `r[...]` requirement ID(s) the function discharges.
    pub requirement_ids: Vec<String>,
    /// The artifact hash (cairn/mantle receipt) that records the check.
    pub artifact_hash: Option<String>,
    /// The binary hash (mantle release-evidence) whose behavior is traced.
    pub binary_hash: Option<String>,
}

/// A provenance chain threading source → function → requirement → artifact → binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceChain {
    pub schema: String,
    pub links: Vec<ProvenanceLink>,
    pub boundary: String,
}

/// Check report for a provenance chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceCheckReport {
    pub valid: bool,
    pub link_count: usize,
    pub issue_codes: Vec<&'static str>,
}

impl ProvenanceChain {
    /// Build a provenance chain from a set of links.
    #[must_use]
    pub fn from_links(links: Vec<ProvenanceLink>) -> Self {
        Self {
            schema: PROVENANCE_CHAIN_SCHEMA.to_owned(),
            links,
            boundary: PROVENANCE_BOUNDARY.to_owned(),
        }
    }

    /// Resolve a source ID to its full provenance link.
    #[must_use]
    pub fn resolve(&self, source_id: &str) -> Option<&ProvenanceLink> {
        self.links.iter().find(|link| link.source_id == source_id)
    }

    /// Resolve a requirement ID to the function objects that discharge it.
    #[must_use]
    pub fn resolve_requirement(&self, requirement_id: &str) -> Vec<&ProvenanceLink> {
        self.links
            .iter()
            .filter(|link| link.requirement_ids.iter().any(|id| id == requirement_id))
            .collect()
    }

    /// Resolve a binary hash to all source functions traced into that binary.
    #[must_use]
    pub fn trace_binary(&self, binary_hash: &str) -> Vec<&ProvenanceLink> {
        self.links.iter().filter(|link| link.binary_hash.as_deref() == Some(binary_hash)).collect()
    }
}

/// Validates a provenance chain.
///
/// Checks: schema field, every link has non-empty source_id and
/// function_object_id, no duplicate source_ids, boundary present.
#[must_use]
pub fn check_provenance_chain(chain: &ProvenanceChain) -> ProvenanceCheckReport {
    let mut issue_codes = Vec::new();

    if chain.schema != PROVENANCE_CHAIN_SCHEMA {
        issue_codes.push("schema_mismatch");
    }
    if chain.boundary != PROVENANCE_BOUNDARY {
        issue_codes.push("boundary_weakened");
    }

    let mut seen_source_ids = std::collections::BTreeSet::new();
    for link in &chain.links {
        if link.source_id.trim().is_empty() {
            issue_codes.push("source_id_empty");
        }
        if link.function_object_id.trim().is_empty() {
            issue_codes.push("function_object_id_empty");
        }
        if !seen_source_ids.insert(link.source_id.clone()) {
            issue_codes.push("duplicate_source_id");
        }
    }

    issue_codes.sort_unstable();
    issue_codes.dedup();
    ProvenanceCheckReport {
        valid: issue_codes.is_empty(),
        link_count: chain.links.len(),
        issue_codes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_link() -> ProvenanceLink {
        ProvenanceLink {
            source_id: "eq:bs_001".to_owned(),
            function_object_id: "b3:abc123def456".to_owned(),
            requirement_ids: vec!["r[cairn.pricing.black_scholes]".to_owned()],
            artifact_hash: Some("b3:receipt_hash_001".to_owned()),
            binary_hash: Some("b3:binary_hash_001".to_owned()),
        }
    }

    fn sample_chain() -> ProvenanceChain {
        ProvenanceChain::from_links(vec![sample_link()])
    }

    #[test]
    fn chain_resolves_source_id_to_link() {
        let chain = sample_chain();
        let link = chain.resolve("eq:bs_001");
        assert!(link.is_some());
        assert_eq!(link.unwrap().function_object_id, "b3:abc123def456");
    }

    #[test]
    fn chain_resolves_unknown_source_id_to_none() {
        let chain = sample_chain();
        assert!(chain.resolve("nonexistent").is_none());
    }

    #[test]
    fn chain_resolves_requirement_to_function_objects() {
        let chain = sample_chain();
        let links = chain.resolve_requirement("r[cairn.pricing.black_scholes]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].source_id, "eq:bs_001");
    }

    #[test]
    fn chain_resolves_unknown_requirement_to_empty() {
        let chain = sample_chain();
        let links = chain.resolve_requirement("r[nonexistent]");
        assert!(links.is_empty());
    }

    #[test]
    fn chain_traces_binary_to_source_functions() {
        let chain = sample_chain();
        let links = chain.trace_binary("b3:binary_hash_001");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].source_id, "eq:bs_001");
    }

    #[test]
    fn chain_traces_unknown_binary_to_empty() {
        let chain = sample_chain();
        let links = chain.trace_binary("b3:nonexistent");
        assert!(links.is_empty());
    }

    #[test]
    fn check_passes_for_valid_chain() {
        let chain = sample_chain();
        let report = check_provenance_chain(&chain);
        assert!(report.valid);
        assert_eq!(report.link_count, 1);
        assert!(report.issue_codes.is_empty());
    }

    #[test]
    fn check_fails_closed_for_schema_mismatch() {
        let mut chain = sample_chain();
        chain.schema = "wrong.schema".to_owned();
        let report = check_provenance_chain(&chain);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"schema_mismatch"));
    }

    #[test]
    fn check_fails_closed_for_weakened_boundary() {
        let mut chain = sample_chain();
        chain.boundary = "this is fine".to_owned();
        let report = check_provenance_chain(&chain);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"boundary_weakened"));
    }

    #[test]
    fn check_fails_closed_for_empty_source_id() {
        let mut link = sample_link();
        link.source_id = String::new();
        let chain = ProvenanceChain::from_links(vec![link]);
        let report = check_provenance_chain(&chain);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"source_id_empty"));
    }

    #[test]
    fn check_fails_closed_for_empty_function_object_id() {
        let mut link = sample_link();
        link.function_object_id = String::new();
        let chain = ProvenanceChain::from_links(vec![link]);
        let report = check_provenance_chain(&chain);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"function_object_id_empty"));
    }

    #[test]
    fn check_fails_closed_for_duplicate_source_id() {
        let chain = ProvenanceChain::from_links(vec![sample_link(), sample_link()]);
        let report = check_provenance_chain(&chain);
        assert!(!report.valid);
        assert!(report.issue_codes.contains(&"duplicate_source_id"));
    }

    #[test]
    fn chain_with_multiple_links_resolves_correctly() {
        let link1 = ProvenanceLink {
            source_id: "fn:abs_diff".to_owned(),
            function_object_id: "b3:abs_diff_hash".to_owned(),
            requirement_ids: vec!["r[trellis.arithmetic.abs_diff]".to_owned()],
            artifact_hash: Some("b3:receipt_a".to_owned()),
            binary_hash: Some("b3:binary_001".to_owned()),
        };
        let link2 = ProvenanceLink {
            source_id: "fn:clamp".to_owned(),
            function_object_id: "b3:clamp_hash".to_owned(),
            requirement_ids: vec!["r[trellis.bounds.clamp]".to_owned()],
            artifact_hash: Some("b3:receipt_b".to_owned()),
            binary_hash: Some("b3:binary_001".to_owned()),
        };
        let chain = ProvenanceChain::from_links(vec![link1, link2]);

        assert_eq!(chain.links.len(), 2);
        assert!(chain.resolve("fn:abs_diff").is_some());
        assert!(chain.resolve("fn:clamp").is_some());
        assert_eq!(chain.trace_binary("b3:binary_001").len(), 2);
        let report = check_provenance_chain(&chain);
        assert!(report.valid);
        assert_eq!(report.link_count, 2);
    }

    #[test]
    fn link_without_artifact_or_binary_hash_is_valid() {
        let link = ProvenanceLink {
            source_id: "fn:spec_only".to_owned(),
            function_object_id: "b3:spec_hash".to_owned(),
            requirement_ids: vec![],
            artifact_hash: None,
            binary_hash: None,
        };
        let chain = ProvenanceChain::from_links(vec![link]);
        let report = check_provenance_chain(&chain);
        assert!(report.valid);
    }

    #[test]
    fn chain_serializes_to_json_with_boundary() {
        let chain = sample_chain();
        let json = serde_json::to_string(&chain).expect("serialize");
        let deserialized: ProvenanceChain = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.schema, PROVENANCE_CHAIN_SCHEMA);
        assert_eq!(deserialized.boundary, PROVENANCE_BOUNDARY);
        assert_eq!(deserialized.links.len(), 1);
    }
}
