use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    evidence::canonical_json_hash,
    evidence_chain_readiness::{
        evaluate_evidence_chain_readiness, EvidenceChainPolicy, EvidenceChainProfile,
        EvidenceChainReadiness, EvidenceChainReadinessInput, EVIDENCE_CHAIN_MODE_OPTIONAL,
        EVIDENCE_CHAIN_MODE_REQUIRED,
    },
};

pub const TRELLIS_PROOF_EVIDENCE_KIND: &str = "trellis-proof";
pub const TRELLIS_PROOF_PROFILE_V1: &str = "cairn.evidence-chain.trellis-proof.v1";
pub const TRELLIS_PROOF_CLAIM_SCOPE: &str = "trellis_proof.v1.lifecycle_policy_conformance";
pub const TRELLIS_PROOF_REQUIRED_NON_CLAIM: &str = "Trellis proof readiness proves lifecycle policy conformance for declared proof evidence only and does not prove verifier soundness, compiler correctness, whole-program safety, Rust semantic correctness, downstream system correctness, or release eligibility";
pub const TRELLIS_PROOF_MODE_OPTIONAL: &str = "optional";
pub const TRELLIS_PROOF_MODE_REQUIRED_RECORDED: &str = "required-recorded";
pub const TRELLIS_PROOF_MODE_REQUIRED_FORMAL: &str = "required-formal-proof";
pub const TRELLIS_PROOF_ROLE_RECORDED: &str = "recorded-only";
pub const TRELLIS_PROOF_ROLE_ACCEPTED_FORMAL: &str = "accepted-formal-proof";

const MAX_SCOPE_IDS: usize = 256;
const MAX_ASSUMPTIONS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrellisProofPolicy {
    pub mode: String,
    pub evidence_chain_policy: EvidenceChainPolicy,
    pub required_requirement_ids: Vec<String>,
    pub required_property_ids: Vec<String>,
    pub allowed_assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrellisProofInput {
    pub evidence_chain: EvidenceChainReadinessInput,
    pub proof_role: String,
    pub requirement_ids: Vec<String>,
    pub property_ids: Vec<String>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrellisProofIssue {
    pub code: String,
    pub field_path: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrellisProofReadiness {
    pub mode: String,
    pub required: bool,
    pub passed: bool,
    pub skipped: bool,
    pub disposition: String,
    pub proof_role: Option<String>,
    pub evidence_chain: EvidenceChainReadiness,
    pub readiness_hash: Option<String>,
    pub issues: Vec<TrellisProofIssue>,
    pub boundary: String,
}

#[must_use]
pub fn trellis_proof_evidence_chain_profile() -> EvidenceChainProfile {
    // r[impl cairn.stack_provenance_gates.trellis_proof_readiness.profile]
    EvidenceChainProfile {
        evidence_kind: TRELLIS_PROOF_EVIDENCE_KIND.to_string(),
        profile_version: TRELLIS_PROOF_PROFILE_V1.to_string(),
        allowed_claim_scopes: vec![TRELLIS_PROOF_CLAIM_SCOPE.to_string()],
        required_non_claims: vec![TRELLIS_PROOF_REQUIRED_NON_CLAIM.to_string()],
    }
}

#[must_use]
pub fn evaluate_trellis_proof_readiness(
    policy: &TrellisProofPolicy,
    input: Option<&TrellisProofInput>,
) -> TrellisProofReadiness {
    // r[impl cairn.stack_provenance_gates.trellis_proof_readiness.links]
    // r[impl cairn.stack_provenance_gates.trellis_proof_readiness.policy_modes]
    // r[impl cairn.stack_provenance_gates.trellis_proof_readiness.assumptions]
    let mut chain_policy = policy.evidence_chain_policy.clone();
    chain_policy.mode = generic_mode(&policy.mode).to_string();
    ensure_trellis_profile(&mut chain_policy.profiles);
    let chain =
        evaluate_evidence_chain_readiness(&chain_policy, input.map(|value| &value.evidence_chain));
    let mut issues = validate_policy(policy);
    if let Some(input) = input {
        validate_input(policy, input, &mut issues);
    }
    issues.extend(chain.issues.iter().map(|issue| TrellisProofIssue {
        code: issue.code.clone(),
        field_path: issue.field_path.clone(),
        expected: issue.expected.clone(),
        actual: issue.actual.clone(),
    }));
    sort_issues(&mut issues);
    let passed = chain.passed && issues.is_empty();
    let skipped = input.is_none() && policy.mode == TRELLIS_PROOF_MODE_OPTIONAL && passed;
    let readiness_hash = input
        .filter(|_| passed)
        .map(|value| trellis_proof_readiness_hash(policy, value));
    let readiness = TrellisProofReadiness {
        mode: policy.mode.clone(),
        required: policy.mode != TRELLIS_PROOF_MODE_OPTIONAL,
        passed,
        skipped,
        disposition: if skipped {
            "absent".to_string()
        } else if passed {
            "present".to_string()
        } else {
            "invalid".to_string()
        },
        proof_role: input.map(|value| value.proof_role.clone()),
        evidence_chain: chain,
        readiness_hash,
        issues,
        boundary: TRELLIS_PROOF_REQUIRED_NON_CLAIM.to_string(),
    };
    debug_assert_eq!(readiness.passed, readiness.issues.is_empty());
    debug_assert!(!readiness.skipped || readiness.readiness_hash.is_none());
    readiness
}

#[must_use]
pub fn trellis_proof_readiness_hash(
    policy: &TrellisProofPolicy,
    input: &TrellisProofInput,
) -> String {
    let mut requirements = input.requirement_ids.clone();
    requirements.sort();
    let mut properties = input.property_ids.clone();
    properties.sort();
    let mut assumptions = input.assumptions.clone();
    assumptions.sort();
    canonical_json_hash(&json!({
        "mode": policy.mode,
        "gate_policy_identity": policy.evidence_chain_policy.gate_policy_identity,
        "evidence_chain": input.evidence_chain,
        "proof_role": input.proof_role,
        "requirement_ids": requirements,
        "property_ids": properties,
        "assumptions": assumptions,
    }))
}

fn generic_mode(mode: &str) -> &'static str {
    if mode == TRELLIS_PROOF_MODE_OPTIONAL {
        EVIDENCE_CHAIN_MODE_OPTIONAL
    } else {
        EVIDENCE_CHAIN_MODE_REQUIRED
    }
}

fn ensure_trellis_profile(profiles: &mut Vec<EvidenceChainProfile>) {
    let registered = profiles.iter().any(|profile| {
        profile.evidence_kind == TRELLIS_PROOF_EVIDENCE_KIND
            && profile.profile_version == TRELLIS_PROOF_PROFILE_V1
    });
    if !registered {
        profiles.push(trellis_proof_evidence_chain_profile());
    }
}

fn validate_policy(policy: &TrellisProofPolicy) -> Vec<TrellisProofIssue> {
    let mut issues = Vec::new();
    if !matches!(
        policy.mode.as_str(),
        TRELLIS_PROOF_MODE_OPTIONAL
            | TRELLIS_PROOF_MODE_REQUIRED_RECORDED
            | TRELLIS_PROOF_MODE_REQUIRED_FORMAL
    ) {
        issues.push(issue(
            "trellis_proof.mode_unsupported",
            "policy.mode",
            "optional, required-recorded, or required-formal-proof",
            &policy.mode,
        ));
    }
    validate_unique_values(
        &policy.required_requirement_ids,
        "policy.required_requirement_ids",
        MAX_SCOPE_IDS,
        &mut issues,
    );
    validate_unique_values(
        &policy.required_property_ids,
        "policy.required_property_ids",
        MAX_SCOPE_IDS,
        &mut issues,
    );
    validate_unique_values(
        &policy.allowed_assumptions,
        "policy.allowed_assumptions",
        MAX_ASSUMPTIONS,
        &mut issues,
    );
    issues
}

fn validate_input(
    policy: &TrellisProofPolicy,
    input: &TrellisProofInput,
    issues: &mut Vec<TrellisProofIssue>,
) {
    if input.evidence_chain.evidence_kind != TRELLIS_PROOF_EVIDENCE_KIND
        || input.evidence_chain.profile_version != TRELLIS_PROOF_PROFILE_V1
    {
        issues.push(issue(
            "trellis_proof.profile_mismatch",
            "input.evidence_chain.profile",
            &format!("{TRELLIS_PROOF_EVIDENCE_KIND}@{TRELLIS_PROOF_PROFILE_V1}"),
            &format!(
                "{}@{}",
                input.evidence_chain.evidence_kind, input.evidence_chain.profile_version
            ),
        ));
    }
    validate_role(policy, input, issues);
    validate_unique_values(
        &input.requirement_ids,
        "input.requirement_ids",
        MAX_SCOPE_IDS,
        issues,
    );
    validate_unique_values(
        &input.property_ids,
        "input.property_ids",
        MAX_SCOPE_IDS,
        issues,
    );
    validate_unique_values(
        &input.assumptions,
        "input.assumptions",
        MAX_ASSUMPTIONS,
        issues,
    );
    if input.requirement_ids.is_empty() && input.property_ids.is_empty() {
        issues.push(issue(
            "trellis_proof.scope_missing",
            "input.requirement_ids",
            "at least one requirement or property id",
            "missing",
        ));
    }
    require_scope_ids(
        &policy.required_requirement_ids,
        &input.requirement_ids,
        "input.requirement_ids",
        issues,
    );
    require_scope_ids(
        &policy.required_property_ids,
        &input.property_ids,
        "input.property_ids",
        issues,
    );
    for assumption in &input.assumptions {
        if !policy
            .allowed_assumptions
            .iter()
            .any(|allowed| allowed == assumption)
        {
            issues.push(issue(
                "trellis_proof.assumption_disallowed",
                "input.assumptions",
                &policy.allowed_assumptions.join(","),
                assumption,
            ));
        }
    }
}

fn validate_role(
    policy: &TrellisProofPolicy,
    input: &TrellisProofInput,
    issues: &mut Vec<TrellisProofIssue>,
) {
    let supported = matches!(
        input.proof_role.as_str(),
        TRELLIS_PROOF_ROLE_RECORDED | TRELLIS_PROOF_ROLE_ACCEPTED_FORMAL
    );
    if !supported {
        issues.push(issue(
            "trellis_proof.role_unsupported",
            "input.proof_role",
            "recorded-only or accepted-formal-proof",
            &input.proof_role,
        ));
        return;
    }
    if policy.mode == TRELLIS_PROOF_MODE_REQUIRED_FORMAL
        && input.proof_role != TRELLIS_PROOF_ROLE_ACCEPTED_FORMAL
    {
        issues.push(issue(
            "trellis_proof.role_mismatch",
            "input.proof_role",
            TRELLIS_PROOF_ROLE_ACCEPTED_FORMAL,
            &input.proof_role,
        ));
    }
}

fn require_scope_ids(
    required: &[String],
    actual: &[String],
    field_path: &str,
    issues: &mut Vec<TrellisProofIssue>,
) {
    for required_id in required {
        if !actual.iter().any(|actual_id| actual_id == required_id) {
            issues.push(issue(
                "trellis_proof.required_scope_missing",
                field_path,
                required_id,
                "missing",
            ));
        }
    }
}

fn validate_unique_values(
    values: &[String],
    field_path: &str,
    maximum: usize,
    issues: &mut Vec<TrellisProofIssue>,
) {
    if values.len() > maximum {
        issues.push(issue(
            "trellis_proof.bound_exceeded",
            field_path,
            &format!("at most {maximum} values"),
            &values.len().to_string(),
        ));
    }
    if values.iter().any(|value| value.trim().is_empty()) {
        issues.push(issue(
            "trellis_proof.value_empty",
            field_path,
            "non-empty values",
            "empty",
        ));
    }
    let unique = values.iter().collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        issues.push(issue(
            "trellis_proof.value_duplicate",
            field_path,
            "unique values",
            "duplicate",
        ));
    }
}

fn sort_issues(issues: &mut Vec<TrellisProofIssue>) {
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.field_path.cmp(&right.field_path))
            .then(left.actual.cmp(&right.actual))
    });
    issues.dedup();
}

fn issue(code: &str, field_path: &str, expected: &str, actual: &str) -> TrellisProofIssue {
    TrellisProofIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verified::evidence_chain_readiness::{
        EvidenceChainMantleLink, EvidenceChainPolicyIdentity, EvidenceChainProjection,
        EvidenceChainValenceLink,
    };

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const HASH_D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const HASH_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const HASH_F: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

    fn policy(mode: &str) -> TrellisProofPolicy {
        TrellisProofPolicy {
            mode: mode.to_string(),
            evidence_chain_policy: EvidenceChainPolicy {
                mode: generic_mode(mode).to_string(),
                gate_policy_identity: HASH_F.to_string(),
                profiles: vec![trellis_proof_evidence_chain_profile()],
                required_policy_hashes: vec![EvidenceChainPolicyIdentity {
                    policy_kind: "trellis-proof-policy".to_string(),
                    digest_blake3: HASH_E.to_string(),
                }],
            },
            required_requirement_ids: vec!["trellis.graph.topological-order".to_string()],
            required_property_ids: vec!["acyclic-order-is-total".to_string()],
            allowed_assumptions: vec!["finite-graph".to_string()],
        }
    }

    fn input(role: &str) -> TrellisProofInput {
        TrellisProofInput {
            evidence_chain: EvidenceChainReadinessInput {
                evidence_kind: TRELLIS_PROOF_EVIDENCE_KIND.to_string(),
                profile_version: TRELLIS_PROOF_PROFILE_V1.to_string(),
                canonical_envelope_hash: HASH_A.to_string(),
                valence_validation: EvidenceChainValenceLink {
                    artifact_hash: HASH_B.to_string(),
                    canonical_envelope_hash: HASH_A.to_string(),
                },
                mantle_binding: EvidenceChainMantleLink {
                    artifact_hash: HASH_C.to_string(),
                    valence_validation_hash: HASH_B.to_string(),
                    source_artifact_hash: HASH_D.to_string(),
                    release_binary_hash: HASH_E.to_string(),
                },
                source_artifact_hash: HASH_D.to_string(),
                release_binary_hash: HASH_E.to_string(),
                policy_hashes: vec![EvidenceChainPolicyIdentity {
                    policy_kind: "trellis-proof-policy".to_string(),
                    digest_blake3: HASH_E.to_string(),
                }],
                claim_scope: TRELLIS_PROOF_CLAIM_SCOPE.to_string(),
                projections: vec![EvidenceChainProjection {
                    role: "proof-graph".to_string(),
                    artifact_hash: HASH_F.to_string(),
                    canonical_envelope_hash: HASH_A.to_string(),
                }],
                non_claims: vec![TRELLIS_PROOF_REQUIRED_NON_CLAIM.to_string()],
            },
            proof_role: role.to_string(),
            requirement_ids: vec!["trellis.graph.topological-order".to_string()],
            property_ids: vec!["acyclic-order-is-total".to_string()],
            assumptions: vec!["finite-graph".to_string()],
        }
    }

    fn codes(readiness: &TrellisProofReadiness) -> Vec<&str> {
        readiness
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect()
    }

    #[test]
    fn positive_policy_modes_preserve_role_boundaries() {
        // r[verify cairn.stack_provenance_gates.trellis_proof_readiness.positive]
        // r[verify cairn.stack_provenance_gates.trellis_proof_readiness.validation]
        let optional = evaluate_trellis_proof_readiness(&policy(TRELLIS_PROOF_MODE_OPTIONAL), None);
        assert!(optional.passed);
        assert!(optional.skipped);
        let optional_recorded = evaluate_trellis_proof_readiness(
            &policy(TRELLIS_PROOF_MODE_OPTIONAL),
            Some(&input(TRELLIS_PROOF_ROLE_RECORDED)),
        );
        assert!(optional_recorded.passed, "{:?}", optional_recorded.issues);
        let recorded = evaluate_trellis_proof_readiness(
            &policy(TRELLIS_PROOF_MODE_REQUIRED_RECORDED),
            Some(&input(TRELLIS_PROOF_ROLE_RECORDED)),
        );
        assert!(recorded.passed, "{:?}", recorded.issues);
        let formal = evaluate_trellis_proof_readiness(
            &policy(TRELLIS_PROOF_MODE_REQUIRED_FORMAL),
            Some(&input(TRELLIS_PROOF_ROLE_ACCEPTED_FORMAL)),
        );
        assert!(formal.passed, "{:?}", formal.issues);
        assert!(formal.readiness_hash.is_some());
    }

    #[test]
    fn negative_proof_fixture_matrix_fails_closed() {
        // r[verify cairn.stack_provenance_gates.trellis_proof_readiness.negative]
        let cases = [
            ("missing-canonical", "evidence_chain.digest_malformed"),
            ("stale-valence", "evidence_chain.valence_link_stale"),
            ("stale-mantle", "evidence_chain.mantle_link_stale"),
            ("wrong-role", "trellis_proof.role_mismatch"),
            ("missing-scope", "trellis_proof.scope_missing"),
            ("assumption", "trellis_proof.assumption_disallowed"),
            ("projection", "evidence_chain.projection_drift"),
            ("source", "evidence_chain.source_link_mismatch"),
            ("malformed-digest", "evidence_chain.digest_malformed"),
            (
                "unsupported-scope",
                "evidence_chain.claim_scope_unsupported",
            ),
            ("overclaim", "evidence_chain.overclaim"),
        ];
        for (case, expected) in cases {
            let policy = policy(TRELLIS_PROOF_MODE_REQUIRED_FORMAL);
            let mut input = input(TRELLIS_PROOF_ROLE_ACCEPTED_FORMAL);
            match case {
                "missing-canonical" => input.evidence_chain.canonical_envelope_hash.clear(),
                "stale-valence" => {
                    input
                        .evidence_chain
                        .valence_validation
                        .canonical_envelope_hash = HASH_B.to_string()
                }
                "stale-mantle" => {
                    input.evidence_chain.mantle_binding.valence_validation_hash = HASH_C.to_string()
                }
                "wrong-role" => input.proof_role = TRELLIS_PROOF_ROLE_RECORDED.to_string(),
                "missing-scope" => {
                    input.requirement_ids.clear();
                    input.property_ids.clear();
                }
                "assumption" => input.assumptions = vec!["ambient-axiom".to_string()],
                "projection" => {
                    input.evidence_chain.projections[0].canonical_envelope_hash = HASH_B.to_string()
                }
                "source" => {
                    input.evidence_chain.mantle_binding.source_artifact_hash = HASH_C.to_string()
                }
                "malformed-digest" => {
                    input.evidence_chain.mantle_binding.artifact_hash = "invalid".to_string()
                }
                "unsupported-scope" => {
                    input.evidence_chain.claim_scope = "release-eligible".to_string()
                }
                "overclaim" => input
                    .evidence_chain
                    .non_claims
                    .push("proves release eligibility".to_string()),
                _ => unreachable!(),
            }
            let readiness = evaluate_trellis_proof_readiness(&policy, Some(&input));
            assert!(!readiness.passed, "case {case}");
            assert!(
                codes(&readiness).contains(&expected),
                "case {case}: {:?}",
                readiness.issues
            );
        }
    }

    #[test]
    fn recorded_only_cannot_satisfy_formal_mode() {
        let readiness = evaluate_trellis_proof_readiness(
            &policy(TRELLIS_PROOF_MODE_REQUIRED_FORMAL),
            Some(&input(TRELLIS_PROOF_ROLE_RECORDED)),
        );
        assert!(!readiness.passed);
        assert!(codes(&readiness).contains(&"trellis_proof.role_mismatch"));
    }
}
