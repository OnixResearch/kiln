use artifact_auth_core::{
    evaluate_authentication, statement_identity, AuthenticationDecision, CryptographicObservation,
    SignatureEvidence,
};

/// Evaluate an explicit standalone cryptographic observation and project it into Cairn's report.
///
/// The observation must describe verification of `mapped.statement`; callers should normally
/// obtain it from `artifact_auth_ed25519::verify_statement` in the Cairn shell.
#[must_use]
// r[impl cairn.artifact_auth.repository_extraction.consumer_authority]
// r[impl cairn.artifact_auth.repository_extraction.consumer_authority.no_promotion]
pub fn evaluate_cairn_artifact_auth(
    input: &crate::AuthenticatedEvidenceInput,
    mapped: &super::CairnArtifactAuthInput,
    cryptographic: CryptographicObservation,
) -> super::CairnArtifactAuthReport {
    let evidence = SignatureEvidence {
        statement: mapped.statement.clone(),
        generation: mapped.generation,
        cryptographic,
    };
    let decision = evaluate_authentication(&mapped.policy, &mapped.scope, &[evidence]);
    project_decision(input, mapped, decision)
}

fn project_decision(
    input: &crate::AuthenticatedEvidenceInput,
    mapped: &super::CairnArtifactAuthInput,
    decision: AuthenticationDecision,
) -> super::CairnArtifactAuthReport {
    let issues = decision
        .issues
        .iter()
        .map(|issue| project_issue(&issue.code, &issue.field))
        .collect::<Vec<_>>();
    let authentication_hash = decision.passed.then(|| decision.decision_blake3.clone());
    super::CairnArtifactAuthReport {
        evidence_kind: input.evidence_kind.clone(),
        decision_passed: decision.passed,
        authentication_hash,
        measured_subject_hash: input.receipt.recomputed_receipt_hash.clone(),
        producer: input.signature.producer.clone(),
        key_id: input.signature.key_id.clone(),
        issues,
        boundary: super::CAIRN_ARTIFACT_AUTH_BOUNDARY.to_string(),
        standalone_statement_blake3: statement_identity(&mapped.statement).ok(),
        standalone_policy_blake3: decision.policy_blake3,
        standalone_scope_blake3: decision.scope_blake3,
        standalone_decision_blake3: decision.decision_blake3,
        standalone_verified_key_blake3: decision.verified_key_blake3,
        legacy_public_key_blake3: mapped.legacy_public_key_blake3.clone(),
        standalone_public_key_blake3: mapped.standalone_public_key_blake3.clone(),
        non_claims: decision.non_claims,
    }
}

fn project_issue(code: &str, field_path: &str) -> crate::AuthenticatedEvidenceIssue {
    crate::AuthenticatedEvidenceIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        expected: "standalone artifact-auth acceptance".to_string(),
        actual: "standalone issue present".to_string(),
    }
}
