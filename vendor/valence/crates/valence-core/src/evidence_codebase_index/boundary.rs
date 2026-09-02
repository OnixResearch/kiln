use super::model::INDEX_BOUNDARY;

const UNSUPPORTED_CLAIMS: &[(&str, &str)] = &[
    ("behavioral_correctness", "proves whole-program correctness"),
    ("semantic_equivalence", "proves semantic equivalence"),
    ("dependency_closure", "complete dependency closure proof"),
    ("remote_code_mobility", "remote code mobility guarantee"),
    ("distributed_execution_correctness", "distributed execution correctness guarantee"),
    ("non_proof_formal_proof", "formal proof for non-proof tiers"),
    ("standalone_valence_release_ownership", "claims standalone Valence release ownership"),
    ("standalone_valence_release_ownership", "Valence owns the evidence codebase index release"),
];

pub(crate) fn boundary_violations(text: &str) -> Vec<&'static str> {
    UNSUPPORTED_CLAIMS
        .iter()
        .filter_map(|(claim, marker)| text.contains(marker).then_some(*claim))
        .collect()
}

#[must_use]
pub fn output_has_required_boundary(text: &str) -> bool {
    text.contains(INDEX_BOUNDARY) && boundary_violations(text).is_empty()
}
