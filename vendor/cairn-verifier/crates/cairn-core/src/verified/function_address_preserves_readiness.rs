pub const FUNCTION_ADDRESS_PRESERVES_SCHEMA_V1: &str = "kamacite.function-address-receipt.v1";
pub const FUNCTION_ADDRESS_PRESERVES_ROLE: &str = "kamacite-function-address-receipt";
pub const FUNCTION_ADDRESS_JSON_PROJECTION_AUTHORITY: &str = "non-authoritative";
pub const FUNCTION_ADDRESS_JSON_PROJECTION_ROLE: &str = "compatibility-json";
pub const FUNCTION_ADDRESS_MANTLE_DISPOSITION_PRESENT: &str = "present";
pub const FUNCTION_ADDRESS_VALENCE_POLICY_KIND: &str = "valence-function-address-policy";
pub const FUNCTION_ADDRESS_PRESERVES_BOUNDARY: &str = "Cairn function-address Preserves readiness proves typed canonical-receipt linkage and lifecycle policy conformance only; it does not prove evidence truth, Rust semantic correctness, compiler correctness, whole-program safety, verifier soundness, runtime authority, or release eligibility";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FunctionAddressJsonProjection {
    pub artifact_hash: String,
    pub canonical_preserves_receipt_hash: String,
    pub authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FunctionAddressPreservesInput {
    pub schema_version: String,
    pub canonical_preserves_receipt_hash: String,
    pub evidence_role: String,
    pub valence_receipt_hash: String,
    pub valence_canonical_preserves_receipt_hash: String,
    pub mantle_binding_hash: String,
    pub mantle_valence_receipt_hash: String,
    pub mantle_disposition: String,
    pub source_artifact_hash: String,
    pub release_binary_hash: String,
    pub valence_policy_hash: String,
    pub claim_scope: String,
    pub valence_claim_scope: String,
    pub mantle_claim_scope: String,
    pub json_projection: Option<FunctionAddressJsonProjection>,
    pub non_claims: Vec<String>,
}

// r[impl cairn.stack_provenance_gates.function_address_preserves_readiness.contract]
#[must_use]
pub fn function_address_preserves_policy(
    mode: &str,
    gate_policy_identity: &str,
) -> super::evidence_chain_readiness::EvidenceChainPolicy {
    super::evidence_chain_readiness::EvidenceChainPolicy {
        mode: mode.to_string(),
        gate_policy_identity: gate_policy_identity.to_string(),
        profiles: vec![super::evidence_chain_readiness::function_address_evidence_chain_profile()],
        required_policy_hashes: Vec::new(),
    }
}

// r[impl cairn.stack_provenance_gates.function_address_preserves_readiness.core]
// r[impl cairn.stack_provenance_gates.function_address_preserves_readiness.json_projection]
#[must_use]
pub fn evaluate_function_address_preserves_readiness(
    policy: &super::evidence_chain_readiness::EvidenceChainPolicy,
    input: Option<&FunctionAddressPreservesInput>,
) -> super::evidence_chain_readiness::EvidenceChainReadiness {
    let generic_input = input.map(generic_evidence_input);
    let mut readiness = super::evidence_chain_readiness::evaluate_evidence_chain_readiness(
        policy,
        generic_input.as_ref(),
    );
    let Some(input) = input else {
        return readiness;
    };

    validate_preserves_metadata(input, &mut readiness.issues);
    sort_issues(&mut readiness.issues);
    if !readiness.issues.is_empty() {
        readiness.passed = false;
        readiness.skipped = false;
        readiness.disposition =
            super::evidence_chain_readiness::EVIDENCE_CHAIN_DISPOSITION_INVALID.to_string();
        readiness.chain_hash = None;
    }
    readiness.boundary = FUNCTION_ADDRESS_PRESERVES_BOUNDARY.to_string();
    debug_assert_eq!(readiness.passed, readiness.issues.is_empty());
    debug_assert!(!readiness.skipped || readiness.chain_hash.is_none());
    readiness
}

fn generic_evidence_input(
    input: &FunctionAddressPreservesInput,
) -> super::evidence_chain_readiness::EvidenceChainReadinessInput {
    let projections = input
        .json_projection
        .iter()
        .map(
            |projection| super::evidence_chain_readiness::EvidenceChainProjection {
                role: FUNCTION_ADDRESS_JSON_PROJECTION_ROLE.to_string(),
                artifact_hash: projection.artifact_hash.clone(),
                canonical_envelope_hash: projection.canonical_preserves_receipt_hash.clone(),
            },
        )
        .collect();
    super::evidence_chain_readiness::EvidenceChainReadinessInput {
        evidence_kind: super::evidence_chain_readiness::EVIDENCE_CHAIN_FUNCTION_ADDRESS_KIND
            .to_string(),
        profile_version:
            super::evidence_chain_readiness::EVIDENCE_CHAIN_FUNCTION_ADDRESS_PROFILE_V1.to_string(),
        canonical_envelope_hash: input.canonical_preserves_receipt_hash.clone(),
        valence_validation: super::evidence_chain_readiness::EvidenceChainValenceLink {
            artifact_hash: input.valence_receipt_hash.clone(),
            canonical_envelope_hash: input.valence_canonical_preserves_receipt_hash.clone(),
        },
        mantle_binding: super::evidence_chain_readiness::EvidenceChainMantleLink {
            artifact_hash: input.mantle_binding_hash.clone(),
            valence_validation_hash: input.mantle_valence_receipt_hash.clone(),
            source_artifact_hash: input.source_artifact_hash.clone(),
            release_binary_hash: input.release_binary_hash.clone(),
        },
        source_artifact_hash: input.source_artifact_hash.clone(),
        release_binary_hash: input.release_binary_hash.clone(),
        policy_hashes: vec![
            super::evidence_chain_readiness::EvidenceChainPolicyIdentity {
                policy_kind: FUNCTION_ADDRESS_VALENCE_POLICY_KIND.to_string(),
                digest_blake3: input.valence_policy_hash.clone(),
            },
        ],
        claim_scope: input.claim_scope.clone(),
        projections,
        non_claims: input.non_claims.clone(),
    }
}

fn validate_preserves_metadata(
    input: &FunctionAddressPreservesInput,
    issues: &mut Vec<super::evidence_chain_readiness::EvidenceChainReadinessIssue>,
) {
    if input.schema_version != FUNCTION_ADDRESS_PRESERVES_SCHEMA_V1 {
        issues.push(issue(
            "function_address_preserves.schema_unsupported",
            "evidence.schema_version",
            FUNCTION_ADDRESS_PRESERVES_SCHEMA_V1,
            &input.schema_version,
        ));
    }
    if input.evidence_role != FUNCTION_ADDRESS_PRESERVES_ROLE {
        issues.push(issue(
            "function_address_preserves.role_unsupported",
            "evidence.evidence_role",
            FUNCTION_ADDRESS_PRESERVES_ROLE,
            &input.evidence_role,
        ));
    }
    validate_claim_scope_link(
        &input.valence_claim_scope,
        &input.claim_scope,
        "evidence.valence_claim_scope",
        "function_address_preserves.valence_claim_scope_stale",
        issues,
    );
    validate_claim_scope_link(
        &input.mantle_claim_scope,
        &input.claim_scope,
        "evidence.mantle_claim_scope",
        "function_address_preserves.mantle_claim_scope_stale",
        issues,
    );
    if input.mantle_disposition != FUNCTION_ADDRESS_MANTLE_DISPOSITION_PRESENT {
        issues.push(issue(
            "function_address_preserves.mantle_disposition_unsupported",
            "evidence.mantle_disposition",
            FUNCTION_ADDRESS_MANTLE_DISPOSITION_PRESENT,
            &input.mantle_disposition,
        ));
    }
    if let Some(projection) = &input.json_projection {
        if projection.authority != FUNCTION_ADDRESS_JSON_PROJECTION_AUTHORITY {
            issues.push(issue(
                "function_address_preserves.json_projection_authoritative",
                "evidence.json_projection.authority",
                FUNCTION_ADDRESS_JSON_PROJECTION_AUTHORITY,
                &projection.authority,
            ));
        }
    }
}

fn validate_claim_scope_link(
    actual: &str,
    expected: &str,
    field_path: &str,
    code: &str,
    issues: &mut Vec<super::evidence_chain_readiness::EvidenceChainReadinessIssue>,
) {
    if actual != expected {
        issues.push(issue(code, field_path, expected, actual));
    }
}

fn sort_issues(issues: &mut Vec<super::evidence_chain_readiness::EvidenceChainReadinessIssue>) {
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.field_path.cmp(&right.field_path))
            .then(left.actual.cmp(&right.actual))
    });
    issues.dedup();
}

fn issue(
    code: &str,
    field_path: &str,
    expected: &str,
    actual: &str,
) -> super::evidence_chain_readiness::EvidenceChainReadinessIssue {
    super::evidence_chain_readiness::EvidenceChainReadinessIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

#[cfg(test)]
// r[verify cairn.stack_provenance_gates.function_address_preserves_readiness.validation]
mod tests {
    use super::*;

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const HASH_D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const HASH_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const HASH_F: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

    type InvalidFixtureCase = (
        &'static str,
        fn(&mut FunctionAddressPreservesInput),
        &'static str,
    );

    fn policy(mode: &str) -> super::super::evidence_chain_readiness::EvidenceChainPolicy {
        function_address_preserves_policy(mode, HASH_F)
    }

    fn input() -> FunctionAddressPreservesInput {
        FunctionAddressPreservesInput {
            schema_version: FUNCTION_ADDRESS_PRESERVES_SCHEMA_V1.to_string(),
            canonical_preserves_receipt_hash: HASH_A.to_string(),
            evidence_role: FUNCTION_ADDRESS_PRESERVES_ROLE.to_string(),
            valence_receipt_hash: HASH_B.to_string(),
            valence_canonical_preserves_receipt_hash: HASH_A.to_string(),
            mantle_binding_hash: HASH_C.to_string(),
            mantle_valence_receipt_hash: HASH_B.to_string(),
            mantle_disposition: FUNCTION_ADDRESS_MANTLE_DISPOSITION_PRESENT.to_string(),
            source_artifact_hash: HASH_D.to_string(),
            release_binary_hash: HASH_E.to_string(),
            valence_policy_hash: HASH_F.to_string(),
            claim_scope: crate::verified::evidence_chain_readiness::EVIDENCE_CHAIN_FUNCTION_ADDRESS_CLAIM_SCOPE.to_string(),
            valence_claim_scope: crate::verified::evidence_chain_readiness::EVIDENCE_CHAIN_FUNCTION_ADDRESS_CLAIM_SCOPE.to_string(),
            mantle_claim_scope: crate::verified::evidence_chain_readiness::EVIDENCE_CHAIN_FUNCTION_ADDRESS_CLAIM_SCOPE.to_string(),
            json_projection: Some(FunctionAddressJsonProjection {
                artifact_hash: HASH_F.to_string(),
                canonical_preserves_receipt_hash: HASH_A.to_string(),
                authority: FUNCTION_ADDRESS_JSON_PROJECTION_AUTHORITY.to_string(),
            }),
            non_claims: vec![crate::verified::evidence_chain_readiness::EVIDENCE_CHAIN_REQUIRED_NON_CLAIM.to_string()],
        }
    }

    fn issue_codes(
        readiness: &super::super::evidence_chain_readiness::EvidenceChainReadiness,
    ) -> Vec<&str> {
        readiness
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect()
    }

    // r[verify cairn.stack_provenance_gates.function_address_preserves_readiness.positive]
    #[test]
    fn optional_absent_preserves_evidence_is_skipped() {
        let readiness = evaluate_function_address_preserves_readiness(
            &policy(crate::verified::evidence_chain_readiness::EVIDENCE_CHAIN_MODE_OPTIONAL),
            None,
        );
        assert!(readiness.passed);
        assert!(readiness.skipped);
        assert!(readiness.chain_hash.is_none());
    }

    #[test]
    fn optional_present_preserves_evidence_passes() {
        let readiness = evaluate_function_address_preserves_readiness(
            &policy(crate::verified::evidence_chain_readiness::EVIDENCE_CHAIN_MODE_OPTIONAL),
            Some(&input()),
        );
        assert!(readiness.passed, "{:?}", readiness.issues);
        assert!(!readiness.skipped);
        assert!(readiness.chain_hash.is_some());
    }

    #[test]
    fn required_complete_preserves_evidence_passes() {
        let readiness = evaluate_function_address_preserves_readiness(
            &policy(crate::verified::evidence_chain_readiness::EVIDENCE_CHAIN_MODE_REQUIRED),
            Some(&input()),
        );
        assert!(readiness.passed, "{:?}", readiness.issues);
        assert!(!readiness.skipped);
        assert!(readiness.chain_hash.is_some());
        assert_eq!(readiness.boundary, FUNCTION_ADDRESS_PRESERVES_BOUNDARY);
    }

    // r[verify cairn.stack_provenance_gates.function_address_preserves_readiness.negative]
    #[test]
    fn invalid_preserves_fixture_matrix_fails_closed() {
        let cases: Vec<InvalidFixtureCase> = vec![
            (
                "missing-canonical-hash",
                |input| input.canonical_preserves_receipt_hash.clear(),
                "evidence_chain.digest_malformed",
            ),
            (
                "stale-valence-link",
                |input| input.valence_canonical_preserves_receipt_hash = HASH_B.to_string(),
                "evidence_chain.valence_link_stale",
            ),
            (
                "stale-mantle-link",
                |input| input.mantle_valence_receipt_hash = HASH_C.to_string(),
                "evidence_chain.mantle_link_stale",
            ),
            (
                "absent-mantle-disposition",
                |input| input.mantle_disposition = "absent".to_string(),
                "function_address_preserves.mantle_disposition_unsupported",
            ),
            (
                "unsupported-schema",
                |input| input.schema_version = "kamacite.unsupported.v0".to_string(),
                "function_address_preserves.schema_unsupported",
            ),
            (
                "malformed-digest",
                |input| input.mantle_binding_hash = "not-a-digest".to_string(),
                "evidence_chain.digest_malformed",
            ),
            (
                "projection-drift",
                |input| {
                    input
                        .json_projection
                        .as_mut()
                        .unwrap()
                        .canonical_preserves_receipt_hash = HASH_B.to_string()
                },
                "evidence_chain.projection_drift",
            ),
            (
                "authoritative-json-projection",
                |input| {
                    input.json_projection.as_mut().unwrap().authority = "authoritative".to_string()
                },
                "function_address_preserves.json_projection_authoritative",
            ),
            (
                "wrong-scope",
                |input| input.claim_scope = "release-eligible".to_string(),
                "evidence_chain.claim_scope_unsupported",
            ),
            (
                "stale-valence-scope",
                |input| input.valence_claim_scope = "stale-scope".to_string(),
                "function_address_preserves.valence_claim_scope_stale",
            ),
            (
                "stale-mantle-scope",
                |input| input.mantle_claim_scope = "stale-scope".to_string(),
                "function_address_preserves.mantle_claim_scope_stale",
            ),
            (
                "wrong-role",
                |input| input.evidence_role = "release-proof".to_string(),
                "function_address_preserves.role_unsupported",
            ),
            (
                "weakened-non-claims",
                |input| input.non_claims.clear(),
                "evidence_chain.non_claim_missing",
            ),
            (
                "overclaim",
                |input| {
                    input
                        .non_claims
                        .push("proves release eligibility".to_string())
                },
                "evidence_chain.overclaim",
            ),
        ];
        for (name, mutate, expected) in cases {
            let mut value = input();
            mutate(&mut value);
            let readiness = evaluate_function_address_preserves_readiness(
                &policy(crate::verified::evidence_chain_readiness::EVIDENCE_CHAIN_MODE_REQUIRED),
                Some(&value),
            );
            assert!(!readiness.passed, "case {name} unexpectedly passed");
            assert!(!readiness.skipped, "case {name} unexpectedly skipped");
            assert!(
                issue_codes(&readiness).contains(&expected),
                "case {name}: {:?}",
                readiness.issues
            );
            assert!(readiness.chain_hash.is_none(), "case {name}");
        }
    }
}
