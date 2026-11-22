// r[impl cairn.nominal_evidence_identities.readiness]
// r[verify cairn.nominal_evidence_identities.readiness.validation]

pub const EVIDENCE_CHAIN_MODE_OPTIONAL: &str = "optional";
pub const EVIDENCE_CHAIN_MODE_REQUIRED: &str = "required";
pub const EVIDENCE_CHAIN_DISPOSITION_ABSENT: &str = "absent";
pub const EVIDENCE_CHAIN_DISPOSITION_PRESENT: &str = "present";
pub const EVIDENCE_CHAIN_DISPOSITION_INVALID: &str = "invalid";
pub const EVIDENCE_CHAIN_FUNCTION_ADDRESS_KIND: &str = "function-address";
pub const EVIDENCE_CHAIN_FUNCTION_ADDRESS_PROFILE_V1: &str =
    "cairn.evidence-chain.function-address.v1";
pub const EVIDENCE_CHAIN_FUNCTION_ADDRESS_CLAIM_SCOPE: &str =
    "function_address.v1.lifecycle_policy_conformance";
pub const EVIDENCE_CHAIN_READINESS_BOUNDARY: &str = "Cairn evidence-chain readiness proves typed identity linkage and lifecycle policy conformance only; it does not prove evidence truth, Rust semantic correctness, compiler correctness, whole-program safety, verifier soundness, runtime authority, or release eligibility";
pub const EVIDENCE_CHAIN_REQUIRED_NON_CLAIM: &str =
    "evidence-chain readiness does not prove evidence truth or release eligibility";

const MAX_PROFILES_COUNT: usize = 32;
const MAX_PROFILE_CLAIM_SCOPES_COUNT: usize = 16;
const MAX_PROFILE_NON_CLAIMS_COUNT: usize = 32;
const MAX_POLICY_HASHES_COUNT: usize = 16;
const MAX_PROJECTIONS_COUNT: usize = 16;
const MAX_NON_CLAIMS_COUNT: usize = 32;
const FORBIDDEN_CLAIM_FRAGMENTS: &[&str] = &[
    "proves evidence truth",
    "proves rust semantic correctness",
    "proves compiler correctness",
    "proves whole-program safety",
    "proves verifier soundness",
    "proves runtime authority",
    "proves release eligibility",
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceChainProfile {
    pub evidence_kind: String,
    pub profile_version: String,
    pub allowed_claim_scopes: Vec<String>,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceChainPolicyIdentity {
    pub policy_kind: String,
    pub digest_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceChainPolicy {
    pub mode: String,
    pub gate_policy_identity: String,
    pub profiles: Vec<EvidenceChainProfile>,
    pub required_policy_hashes: Vec<EvidenceChainPolicyIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceChainValenceLink {
    pub artifact_hash: String,
    pub canonical_envelope_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceChainMantleLink {
    pub artifact_hash: String,
    pub valence_validation_hash: String,
    pub source_artifact_hash: String,
    pub release_binary_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceChainProjection {
    pub role: String,
    pub artifact_hash: String,
    pub canonical_envelope_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceChainReadinessInput {
    pub evidence_kind: String,
    pub profile_version: String,
    pub canonical_envelope_hash: String,
    pub valence_validation: EvidenceChainValenceLink,
    pub mantle_binding: EvidenceChainMantleLink,
    pub source_artifact_hash: String,
    pub release_binary_hash: String,
    pub policy_hashes: Vec<EvidenceChainPolicyIdentity>,
    pub claim_scope: String,
    pub projections: Vec<EvidenceChainProjection>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceChainReadinessIssue {
    pub code: String,
    pub field_path: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceChainReadiness {
    pub mode: String,
    pub required: bool,
    pub passed: bool,
    pub skipped: bool,
    pub disposition: String,
    pub evidence_kind: Option<String>,
    pub profile_version: Option<String>,
    pub chain_hash: Option<String>,
    pub gate_policy_identity: String,
    pub issues: Vec<EvidenceChainReadinessIssue>,
    pub boundary: String,
}

// r[impl cairn.stack_provenance_gates.evidence_chain_readiness.profile_registry]
#[must_use]
pub fn function_address_evidence_chain_profile() -> EvidenceChainProfile {
    EvidenceChainProfile {
        evidence_kind: EVIDENCE_CHAIN_FUNCTION_ADDRESS_KIND.to_string(),
        profile_version: EVIDENCE_CHAIN_FUNCTION_ADDRESS_PROFILE_V1.to_string(),
        allowed_claim_scopes: vec![EVIDENCE_CHAIN_FUNCTION_ADDRESS_CLAIM_SCOPE.to_string()],
        required_non_claims: vec![EVIDENCE_CHAIN_REQUIRED_NON_CLAIM.to_string()],
    }
}

// r[impl cairn.stack_provenance_gates.evidence_chain_readiness.contract]
// r[impl cairn.stack_provenance_gates.evidence_chain_readiness.core]
// r[impl cairn.stack_provenance_gates.evidence_chain_readiness.policy_modes]
#[must_use]
pub fn evaluate_evidence_chain_readiness(
    policy: &EvidenceChainPolicy,
    evidence: Option<&EvidenceChainReadinessInput>,
) -> EvidenceChainReadiness {
    let mut policy_issues = validate_policy(policy);
    let Some(evidence) = evidence else {
        if policy.mode == EVIDENCE_CHAIN_MODE_OPTIONAL && policy_issues.is_empty() {
            return readiness(
                policy,
                None,
                true,
                true,
                EVIDENCE_CHAIN_DISPOSITION_ABSENT,
                Vec::new(),
            );
        }
        if policy.mode == EVIDENCE_CHAIN_MODE_REQUIRED {
            policy_issues.push(issue(
                "evidence_chain.evidence_missing",
                "evidence",
                "present evidence for a required profile",
                "missing",
            ));
        }
        sort_issues(&mut policy_issues);
        return readiness(
            policy,
            None,
            false,
            false,
            EVIDENCE_CHAIN_DISPOSITION_INVALID,
            policy_issues,
        );
    };

    let profile = registered_profile(policy, evidence);
    let mut issues = policy_issues;
    if profile.is_none() {
        issues.push(issue(
            "evidence_chain.profile_unsupported",
            "evidence.profile",
            "an exact registered evidence kind and profile version",
            &format!("{}@{}", evidence.evidence_kind, evidence.profile_version),
        ));
    }
    validate_evidence(policy, profile, evidence, &mut issues);
    sort_issues(&mut issues);
    let passed = issues.is_empty();
    readiness(
        policy,
        Some(evidence),
        passed,
        false,
        if passed {
            EVIDENCE_CHAIN_DISPOSITION_PRESENT
        } else {
            EVIDENCE_CHAIN_DISPOSITION_INVALID
        },
        issues,
    )
}

fn validate_policy(policy: &EvidenceChainPolicy) -> Vec<EvidenceChainReadinessIssue> {
    let mut issues = Vec::new();
    if !matches!(
        policy.mode.as_str(),
        EVIDENCE_CHAIN_MODE_OPTIONAL | EVIDENCE_CHAIN_MODE_REQUIRED
    ) {
        issues.push(issue(
            "evidence_chain.mode_unsupported",
            "policy.mode",
            "optional or required",
            &policy.mode,
        ));
    }
    validate_digest::<super::nominal_identity::PolicyDigestRole>(
        &policy.gate_policy_identity,
        "policy.gate_policy_identity",
        &mut issues,
    );
    validate_count(
        policy.profiles.len(),
        MAX_PROFILES_COUNT,
        "policy.profiles",
        &mut issues,
    );
    validate_count(
        policy.required_policy_hashes.len(),
        MAX_POLICY_HASHES_COUNT,
        "policy.required_policy_hashes",
        &mut issues,
    );
    validate_policy_identities(
        &policy.required_policy_hashes,
        "policy.required_policy_hashes",
        &mut issues,
    );
    validate_profile_registry(&policy.profiles, &mut issues);
    issues
}

fn validate_profile_registry(
    profiles: &[EvidenceChainProfile],
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    let mut keys = profiles
        .iter()
        .map(|profile| (&profile.evidence_kind, &profile.profile_version))
        .collect::<Vec<_>>();
    keys.sort();
    if keys.windows(2).any(|pair| pair[0] == pair[1]) {
        issues.push(issue(
            "evidence_chain.profile_duplicate",
            "policy.profiles",
            "unique evidence kind and profile version pairs",
            "duplicate",
        ));
    }
    for (index, profile) in profiles.iter().enumerate() {
        let profile_path = format!("policy.profiles[{index}]");
        let evidence_kind = super::nominal_identity::DomainIdRef::<
            super::nominal_identity::EvidenceKindRole,
        >::parse(&profile.evidence_kind, "policy.profile.evidence_kind");
        if evidence_kind.is_err() || profile.profile_version.trim().is_empty() {
            issues.push(issue(
                "evidence_chain.profile_malformed",
                &profile_path,
                "canonical evidence kind and non-empty profile version",
                if profile.evidence_kind.is_empty() || profile.profile_version.is_empty() {
                    "empty"
                } else {
                    "malformed"
                },
            ));
        }
        if profile.allowed_claim_scopes.is_empty() || profile.required_non_claims.is_empty() {
            issues.push(issue(
                "evidence_chain.profile_boundary_missing",
                &profile_path,
                "at least one claim scope and required non-claim",
                "missing",
            ));
        }
        validate_count(
            profile.allowed_claim_scopes.len(),
            MAX_PROFILE_CLAIM_SCOPES_COUNT,
            &format!("{profile_path}.allowed_claim_scopes"),
            issues,
        );
        validate_count(
            profile.required_non_claims.len(),
            MAX_PROFILE_NON_CLAIMS_COUNT,
            &format!("{profile_path}.required_non_claims"),
            issues,
        );
        validate_non_empty_unique_values(
            &profile.allowed_claim_scopes,
            &format!("{profile_path}.allowed_claim_scopes"),
            "evidence_chain.profile_claim_scope_empty",
            "evidence_chain.profile_claim_scope_duplicate",
            issues,
        );
        validate_non_empty_unique_values(
            &profile.required_non_claims,
            &format!("{profile_path}.required_non_claims"),
            "evidence_chain.profile_non_claim_empty",
            "evidence_chain.profile_non_claim_duplicate",
            issues,
        );
    }
}

fn validate_non_empty_unique_values(
    values: &[String],
    field_path: &str,
    empty_code: &str,
    duplicate_code: &str,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    if values.iter().any(|value| value.trim().is_empty()) {
        issues.push(issue(empty_code, field_path, "non-empty values", "empty"));
    }
    let mut sorted_values = values.iter().map(String::as_str).collect::<Vec<_>>();
    sorted_values.sort_unstable();
    if sorted_values.windows(2).any(|pair| pair[0] == pair[1]) {
        issues.push(issue(
            duplicate_code,
            field_path,
            "unique values",
            "duplicate",
        ));
    }
}

fn registered_profile<'a>(
    policy: &'a EvidenceChainPolicy,
    evidence: &EvidenceChainReadinessInput,
) -> Option<&'a EvidenceChainProfile> {
    let evidence_kind = super::nominal_identity::DomainIdRef::<
        super::nominal_identity::EvidenceKindRole,
    >::parse(&evidence.evidence_kind, "evidence.evidence_kind")
    .ok()?;
    policy.profiles.iter().find(|profile| {
        let profile_kind = super::nominal_identity::DomainIdRef::<
            super::nominal_identity::EvidenceKindRole,
        >::parse(&profile.evidence_kind, "policy.profile.evidence_kind");
        profile_kind.is_ok_and(|kind| kind == evidence_kind)
            && profile.profile_version == evidence.profile_version
    })
}

fn validate_evidence(
    policy: &EvidenceChainPolicy,
    profile: Option<&EvidenceChainProfile>,
    evidence: &EvidenceChainReadinessInput,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    if super::nominal_identity::DomainIdRef::<super::nominal_identity::EvidenceKindRole>::parse(
        &evidence.evidence_kind,
        "evidence.evidence_kind",
    )
    .is_err()
    {
        issues.push(issue(
            "evidence_chain.evidence_kind_malformed",
            "evidence.evidence_kind",
            "canonical evidence kind",
            &evidence.evidence_kind,
        ));
    }
    validate_evidence_counts(evidence, issues);
    validate_evidence_hashes(evidence, issues);
    validate_links(evidence, issues);
    validate_policy_identities(&evidence.policy_hashes, "evidence.policy_hashes", issues);
    validate_required_policy_hashes(policy, evidence, issues);
    validate_profile_boundary(profile, evidence, issues);
    validate_projections(evidence, issues);
}

fn validate_evidence_counts(
    evidence: &EvidenceChainReadinessInput,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    validate_count(
        evidence.policy_hashes.len(),
        MAX_POLICY_HASHES_COUNT,
        "evidence.policy_hashes",
        issues,
    );
    validate_count(
        evidence.projections.len(),
        MAX_PROJECTIONS_COUNT,
        "evidence.projections",
        issues,
    );
    validate_count(
        evidence.non_claims.len(),
        MAX_NON_CLAIMS_COUNT,
        "evidence.non_claims",
        issues,
    );
    if evidence
        .non_claims
        .iter()
        .any(|non_claim| non_claim.trim().is_empty())
    {
        issues.push(issue(
            "evidence_chain.non_claim_empty",
            "evidence.non_claims",
            "non-empty values",
            "empty",
        ));
    }
}

fn validate_evidence_hashes(
    evidence: &EvidenceChainReadinessInput,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    validate_digest::<super::nominal_identity::ArtifactDigestRole>(
        &evidence.canonical_envelope_hash,
        "evidence.canonical_envelope_hash",
        issues,
    );
    validate_digest::<super::nominal_identity::ValenceDigestRole>(
        &evidence.valence_validation.artifact_hash,
        "evidence.valence_validation.artifact_hash",
        issues,
    );
    validate_digest::<super::nominal_identity::MantleDigestRole>(
        &evidence.mantle_binding.artifact_hash,
        "evidence.mantle_binding.artifact_hash",
        issues,
    );
    validate_digest::<super::nominal_identity::SourceDigestRole>(
        &evidence.source_artifact_hash,
        "evidence.source_artifact_hash",
        issues,
    );
    validate_digest::<super::nominal_identity::BinaryDigestRole>(
        &evidence.release_binary_hash,
        "evidence.release_binary_hash",
        issues,
    );
}

fn validate_links(
    evidence: &EvidenceChainReadinessInput,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    compare_digest_link::<super::nominal_identity::ArtifactDigestRole>(
        &evidence.valence_validation.canonical_envelope_hash,
        &evidence.canonical_envelope_hash,
        "evidence.valence_validation.canonical_envelope_hash",
        "evidence_chain.valence_link_stale",
        issues,
    );
    compare_digest_link::<super::nominal_identity::ValenceDigestRole>(
        &evidence.mantle_binding.valence_validation_hash,
        &evidence.valence_validation.artifact_hash,
        "evidence.mantle_binding.valence_validation_hash",
        "evidence_chain.mantle_link_stale",
        issues,
    );
    compare_digest_link::<super::nominal_identity::SourceDigestRole>(
        &evidence.mantle_binding.source_artifact_hash,
        &evidence.source_artifact_hash,
        "evidence.mantle_binding.source_artifact_hash",
        "evidence_chain.source_link_mismatch",
        issues,
    );
    compare_digest_link::<super::nominal_identity::BinaryDigestRole>(
        &evidence.mantle_binding.release_binary_hash,
        &evidence.release_binary_hash,
        "evidence.mantle_binding.release_binary_hash",
        "evidence_chain.binary_link_mismatch",
        issues,
    );
}

fn validate_policy_identities(
    identities: &[EvidenceChainPolicyIdentity],
    field_path: &str,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    let mut kinds = identities
        .iter()
        .map(|identity| identity.policy_kind.as_str())
        .collect::<Vec<_>>();
    kinds.sort_unstable();
    if kinds.windows(2).any(|pair| pair[0] == pair[1]) {
        issues.push(issue(
            "evidence_chain.policy_kind_duplicate",
            field_path,
            "unique policy kinds",
            "duplicate",
        ));
    }
    for (index, identity) in identities.iter().enumerate() {
        if super::nominal_identity::DomainIdRef::<super::nominal_identity::PolicyKindRole>::parse(
            &identity.policy_kind,
            "evidence.policy_kind",
        )
        .is_err()
        {
            issues.push(issue(
                "evidence_chain.policy_kind_empty",
                &format!("{field_path}[{index}].policy_kind"),
                "canonical policy kind",
                if identity.policy_kind.is_empty() {
                    "empty"
                } else {
                    "malformed"
                },
            ));
        }
        validate_digest::<super::nominal_identity::PolicyDigestRole>(
            &identity.digest_blake3,
            &format!("{field_path}[{index}].digest_blake3"),
            issues,
        );
    }
}

fn validate_required_policy_hashes(
    policy: &EvidenceChainPolicy,
    evidence: &EvidenceChainReadinessInput,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    for required in &policy.required_policy_hashes {
        let actual = evidence.policy_hashes.iter().find(|identity| {
            same_domain::<super::nominal_identity::PolicyKindRole>(
                &identity.policy_kind,
                &required.policy_kind,
            )
        });
        match actual {
            Some(actual)
                if same_digest::<super::nominal_identity::PolicyDigestRole>(
                    &actual.digest_blake3,
                    &required.digest_blake3,
                ) => {}
            Some(actual) => issues.push(issue(
                "evidence_chain.policy_hash_stale",
                &format!("evidence.policy_hashes.{}", required.policy_kind),
                &required.digest_blake3,
                &actual.digest_blake3,
            )),
            None => issues.push(issue(
                "evidence_chain.policy_hash_missing",
                &format!("evidence.policy_hashes.{}", required.policy_kind),
                &required.digest_blake3,
                "missing",
            )),
        }
    }
}

fn validate_profile_boundary(
    profile: Option<&EvidenceChainProfile>,
    evidence: &EvidenceChainReadinessInput,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    let Some(profile) = profile else {
        return;
    };
    if !profile
        .allowed_claim_scopes
        .iter()
        .any(|scope| scope == &evidence.claim_scope)
    {
        issues.push(issue(
            "evidence_chain.claim_scope_unsupported",
            "evidence.claim_scope",
            &profile.allowed_claim_scopes.join(","),
            &evidence.claim_scope,
        ));
    }
    for required in &profile.required_non_claims {
        if !evidence
            .non_claims
            .iter()
            .any(|non_claim| non_claim == required)
        {
            issues.push(issue(
                "evidence_chain.non_claim_missing",
                "evidence.non_claims",
                required,
                "missing",
            ));
        }
    }
    for non_claim in &evidence.non_claims {
        let normalized = non_claim.to_ascii_lowercase();
        if FORBIDDEN_CLAIM_FRAGMENTS
            .iter()
            .any(|fragment| normalized.contains(fragment))
        {
            issues.push(issue(
                "evidence_chain.overclaim",
                "evidence.non_claims",
                "bounded non-claim text",
                non_claim,
            ));
        }
    }
}

fn validate_projections(
    evidence: &EvidenceChainReadinessInput,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    let mut roles = evidence
        .projections
        .iter()
        .map(|projection| projection.role.as_str())
        .collect::<Vec<_>>();
    roles.sort_unstable();
    if roles.windows(2).any(|pair| pair[0] == pair[1]) {
        issues.push(issue(
            "evidence_chain.projection_role_duplicate",
            "evidence.projections",
            "unique projection roles",
            "duplicate",
        ));
    }
    for (index, projection) in evidence.projections.iter().enumerate() {
        if super::nominal_identity::DomainIdRef::<super::nominal_identity::ProjectionRoleMarker>::parse(&projection.role, "evidence.projection.role")
            .is_err()
        {
            issues.push(issue(
                "evidence_chain.projection_role_empty",
                &format!("evidence.projections[{index}].role"),
                "a canonical projection role",
                if projection.role.is_empty() {
                    "empty"
                } else {
                    "malformed"
                },
            ));
        }
        validate_digest::<super::nominal_identity::ArtifactDigestRole>(
            &projection.artifact_hash,
            &format!("evidence.projections[{index}].artifact_hash"),
            issues,
        );
        compare_digest_link::<super::nominal_identity::ArtifactDigestRole>(
            &projection.canonical_envelope_hash,
            &evidence.canonical_envelope_hash,
            &format!("evidence.projections[{index}].canonical_envelope_hash"),
            "evidence_chain.projection_drift",
            issues,
        );
    }
}

// r[impl cairn.stack_provenance_gates.evidence_chain_readiness.chain_hash]
#[must_use]
pub fn evidence_chain_readiness_hash(
    policy: &EvidenceChainPolicy,
    evidence: &EvidenceChainReadinessInput,
) -> String {
    let mut policy_hashes = evidence.policy_hashes.clone();
    policy_hashes.sort_by(|left, right| left.policy_kind.cmp(&right.policy_kind));
    let mut projections = evidence.projections.clone();
    projections.sort_by(|left, right| left.role.cmp(&right.role));
    let mut non_claims = evidence.non_claims.clone();
    non_claims.sort();
    super::evidence::canonical_json_hash(&serde_json::json!({
        "evidence_kind": evidence.evidence_kind,
        "profile_version": evidence.profile_version,
        "canonical_envelope_hash": evidence.canonical_envelope_hash,
        "valence_validation": evidence.valence_validation,
        "mantle_binding": evidence.mantle_binding,
        "source_artifact_hash": evidence.source_artifact_hash,
        "release_binary_hash": evidence.release_binary_hash,
        "policy_hashes": policy_hashes,
        "claim_scope": evidence.claim_scope,
        "projections": projections,
        "non_claims": non_claims,
        "gate_policy_identity": policy.gate_policy_identity,
    }))
}

fn readiness(
    policy: &EvidenceChainPolicy,
    evidence: Option<&EvidenceChainReadinessInput>,
    passed: bool,
    skipped: bool,
    disposition: &str,
    issues: Vec<EvidenceChainReadinessIssue>,
) -> EvidenceChainReadiness {
    let chain_hash = evidence
        .filter(|_| passed)
        .map(|value| evidence_chain_readiness_hash(policy, value));
    let readiness = EvidenceChainReadiness {
        mode: policy.mode.clone(),
        required: policy.mode == EVIDENCE_CHAIN_MODE_REQUIRED,
        passed,
        skipped,
        disposition: disposition.to_string(),
        evidence_kind: evidence.map(|value| value.evidence_kind.clone()),
        profile_version: evidence.map(|value| value.profile_version.clone()),
        chain_hash,
        gate_policy_identity: policy.gate_policy_identity.clone(),
        issues,
        boundary: EVIDENCE_CHAIN_READINESS_BOUNDARY.to_string(),
    };
    debug_assert_eq!(readiness.passed, readiness.issues.is_empty());
    debug_assert!(!readiness.skipped || readiness.chain_hash.is_none());
    debug_assert!(!readiness.boundary.is_empty());
    readiness
}

fn same_domain<Role: super::nominal_identity::DomainRoleMarker>(left: &str, right: &str) -> bool {
    let left =
        super::nominal_identity::DomainIdRef::<Role>::parse(left, "evidence_chain.left_domain");
    let right =
        super::nominal_identity::DomainIdRef::<Role>::parse(right, "evidence_chain.right_domain");
    matches!((left, right), (Ok(left), Ok(right)) if left == right)
}

fn same_digest<Role: super::nominal_identity::DigestRoleMarker>(left: &str, right: &str) -> bool {
    let left = super::nominal_identity::Blake3DigestRef::<Role>::parse(
        left,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "evidence_chain.left_digest",
    );
    let right = super::nominal_identity::Blake3DigestRef::<Role>::parse(
        right,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "evidence_chain.right_digest",
    );
    matches!((left, right), (Ok(left), Ok(right)) if left.matches(right))
}

fn compare_digest_link<Role: super::nominal_identity::DigestRoleMarker>(
    actual: &str,
    expected: &str,
    field_path: &str,
    code: &str,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    let actual_digest = super::nominal_identity::Blake3DigestRef::<Role>::parse(
        actual,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "evidence_chain.actual_digest",
    );
    let expected_digest = super::nominal_identity::Blake3DigestRef::<Role>::parse(
        expected,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "evidence_chain.expected_digest",
    );
    let Ok(actual_digest) = actual_digest else {
        push_digest_issue(actual, field_path, issues);
        return;
    };
    let Ok(expected_digest) = expected_digest else {
        return;
    };
    if !actual_digest.matches(expected_digest) {
        issues.push(issue(code, field_path, expected, actual));
    }
}

fn validate_digest<Role: super::nominal_identity::DigestRoleMarker>(
    value: &str,
    field_path: &str,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    if super::nominal_identity::Blake3DigestRef::<Role>::parse(
        value,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "evidence_chain.digest",
    )
    .is_err()
    {
        push_digest_issue(value, field_path, issues);
    }
}

fn push_digest_issue(value: &str, field_path: &str, issues: &mut Vec<EvidenceChainReadinessIssue>) {
    issues.push(issue(
        "evidence_chain.digest_malformed",
        field_path,
        "64 lowercase BLAKE3 hexadecimal characters",
        value,
    ));
}

fn validate_count(
    actual: usize,
    maximum: usize,
    field_path: &str,
    issues: &mut Vec<EvidenceChainReadinessIssue>,
) {
    if actual > maximum {
        issues.push(issue(
            "evidence_chain.bound_exceeded",
            field_path,
            &format!("at most {maximum} entries"),
            &actual.to_string(),
        ));
    }
}

fn sort_issues(issues: &mut Vec<EvidenceChainReadinessIssue>) {
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
) -> EvidenceChainReadinessIssue {
    EvidenceChainReadinessIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

#[cfg(test)]
// r[verify cairn.stack_provenance_gates.evidence_chain_readiness.validation]
mod tests {
    use super::*;

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const HASH_D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const HASH_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const HASH_F: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    const STUB_KIND: &str = "test-stub";
    const STUB_PROFILE: &str = "cairn.evidence-chain.test-stub.v1";
    const STUB_SCOPE: &str = "test_stub.v1.lifecycle_policy_conformance";

    type InvalidFixtureCase = (
        &'static str,
        fn(&mut EvidenceChainPolicy, &mut EvidenceChainReadinessInput),
        &'static str,
    );

    fn policy(mode: &str) -> EvidenceChainPolicy {
        EvidenceChainPolicy {
            mode: mode.to_string(),
            gate_policy_identity: HASH_F.to_string(),
            profiles: vec![function_address_evidence_chain_profile()],
            required_policy_hashes: vec![EvidenceChainPolicyIdentity {
                policy_kind: "upstream-profile-policy".to_string(),
                digest_blake3: HASH_E.to_string(),
            }],
        }
    }

    fn evidence() -> EvidenceChainReadinessInput {
        EvidenceChainReadinessInput {
            evidence_kind: EVIDENCE_CHAIN_FUNCTION_ADDRESS_KIND.to_string(),
            profile_version: EVIDENCE_CHAIN_FUNCTION_ADDRESS_PROFILE_V1.to_string(),
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
                policy_kind: "upstream-profile-policy".to_string(),
                digest_blake3: HASH_E.to_string(),
            }],
            claim_scope: EVIDENCE_CHAIN_FUNCTION_ADDRESS_CLAIM_SCOPE.to_string(),
            projections: vec![EvidenceChainProjection {
                role: "compatibility-json".to_string(),
                artifact_hash: HASH_F.to_string(),
                canonical_envelope_hash: HASH_A.to_string(),
            }],
            non_claims: vec![EVIDENCE_CHAIN_REQUIRED_NON_CLAIM.to_string()],
        }
    }

    fn issue_codes(readiness: &EvidenceChainReadiness) -> Vec<&str> {
        readiness
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect()
    }

    // r[verify cairn.stack_provenance_gates.evidence_chain_readiness.positive]
    #[test]
    fn optional_absent_is_skipped() {
        let readiness =
            evaluate_evidence_chain_readiness(&policy(EVIDENCE_CHAIN_MODE_OPTIONAL), None);
        assert!(readiness.passed);
        assert!(readiness.skipped);
        assert_eq!(readiness.disposition, EVIDENCE_CHAIN_DISPOSITION_ABSENT);
        assert!(readiness.chain_hash.is_none());
        assert!(readiness.issues.is_empty());
    }

    #[test]
    fn required_function_address_profile_passes() {
        let input = evidence();
        let readiness =
            evaluate_evidence_chain_readiness(&policy(EVIDENCE_CHAIN_MODE_REQUIRED), Some(&input));
        assert!(readiness.passed, "{:?}", readiness.issues);
        assert!(!readiness.skipped);
        assert_eq!(readiness.disposition, EVIDENCE_CHAIN_DISPOSITION_PRESENT);
        assert_eq!(
            readiness.evidence_kind.as_deref(),
            Some(EVIDENCE_CHAIN_FUNCTION_ADDRESS_KIND)
        );
        assert_eq!(
            readiness.profile_version.as_deref(),
            Some(EVIDENCE_CHAIN_FUNCTION_ADDRESS_PROFILE_V1)
        );
        assert_eq!(
            readiness.chain_hash.as_deref(),
            Some(
                evidence_chain_readiness_hash(&policy(EVIDENCE_CHAIN_MODE_REQUIRED), &input)
                    .as_str()
            )
        );
    }

    #[test]
    fn explicitly_registered_non_function_stub_passes() {
        let mut policy = policy(EVIDENCE_CHAIN_MODE_REQUIRED);
        policy.profiles.push(EvidenceChainProfile {
            evidence_kind: STUB_KIND.to_string(),
            profile_version: STUB_PROFILE.to_string(),
            allowed_claim_scopes: vec![STUB_SCOPE.to_string()],
            required_non_claims: vec![EVIDENCE_CHAIN_REQUIRED_NON_CLAIM.to_string()],
        });
        let mut input = evidence();
        input.evidence_kind = STUB_KIND.to_string();
        input.profile_version = STUB_PROFILE.to_string();
        input.claim_scope = STUB_SCOPE.to_string();
        let readiness = evaluate_evidence_chain_readiness(&policy, Some(&input));
        assert!(readiness.passed, "{:?}", readiness.issues);
        assert_eq!(readiness.evidence_kind.as_deref(), Some(STUB_KIND));
    }

    #[test]
    fn chain_hash_is_order_independent_for_set_like_inputs() {
        let mut policy = policy(EVIDENCE_CHAIN_MODE_REQUIRED);
        policy
            .required_policy_hashes
            .push(EvidenceChainPolicyIdentity {
                policy_kind: "mantle-release-policy".to_string(),
                digest_blake3: HASH_D.to_string(),
            });
        let mut left = evidence();
        left.policy_hashes.push(EvidenceChainPolicyIdentity {
            policy_kind: "mantle-release-policy".to_string(),
            digest_blake3: HASH_D.to_string(),
        });
        left.projections.push(EvidenceChainProjection {
            role: "preserves".to_string(),
            artifact_hash: HASH_C.to_string(),
            canonical_envelope_hash: HASH_A.to_string(),
        });
        left.non_claims.push("not runtime authority".to_string());
        let mut right = left.clone();
        right.policy_hashes.reverse();
        right.projections.reverse();
        right.non_claims.reverse();
        assert_eq!(
            evidence_chain_readiness_hash(&policy, &left),
            evidence_chain_readiness_hash(&policy, &right)
        );
    }

    // r[verify cairn.stack_provenance_gates.evidence_chain_readiness.negative]
    #[test]
    fn invalid_fixture_matrix_fails_closed() {
        let cases: Vec<InvalidFixtureCase> = vec![
            (
                "unknown-profile",
                |_, input| input.profile_version = "unknown".to_string(),
                "evidence_chain.profile_unsupported",
            ),
            (
                "empty-profile-claim-scope",
                |policy, _| policy.profiles[0].allowed_claim_scopes[0].clear(),
                "evidence_chain.profile_claim_scope_empty",
            ),
            (
                "duplicate-profile-claim-scope",
                |policy, _| {
                    let duplicate = policy.profiles[0].allowed_claim_scopes[0].clone();
                    policy.profiles[0].allowed_claim_scopes.push(duplicate);
                },
                "evidence_chain.profile_claim_scope_duplicate",
            ),
            (
                "profile-claim-scope-bound",
                |policy, _| {
                    policy.profiles[0].allowed_claim_scopes = (0..=MAX_PROFILE_CLAIM_SCOPES_COUNT)
                        .map(|index| format!("scope-{index}"))
                        .collect();
                },
                "evidence_chain.bound_exceeded",
            ),
            (
                "empty-profile-non-claim",
                |policy, _| policy.profiles[0].required_non_claims[0].clear(),
                "evidence_chain.profile_non_claim_empty",
            ),
            (
                "duplicate-profile-non-claim",
                |policy, _| {
                    let duplicate = policy.profiles[0].required_non_claims[0].clone();
                    policy.profiles[0].required_non_claims.push(duplicate);
                },
                "evidence_chain.profile_non_claim_duplicate",
            ),
            (
                "profile-non-claim-bound",
                |policy, _| {
                    policy.profiles[0].required_non_claims = (0..=MAX_PROFILE_NON_CLAIMS_COUNT)
                        .map(|index| format!("non-claim-{index}"))
                        .collect();
                },
                "evidence_chain.bound_exceeded",
            ),
            (
                "empty-projection-role",
                |_, input| input.projections[0].role.clear(),
                "evidence_chain.projection_role_empty",
            ),
            (
                "duplicate-projection-role",
                |_, input| input.projections.push(input.projections[0].clone()),
                "evidence_chain.projection_role_duplicate",
            ),
            (
                "empty-evidence-non-claim",
                |_, input| input.non_claims.push(String::new()),
                "evidence_chain.non_claim_empty",
            ),
            (
                "missing-canonical-hash",
                |_, input| input.canonical_envelope_hash.clear(),
                "evidence_chain.digest_malformed",
            ),
            (
                "stale-valence-link",
                |_, input| input.valence_validation.canonical_envelope_hash = HASH_B.to_string(),
                "evidence_chain.valence_link_stale",
            ),
            (
                "stale-mantle-link",
                |_, input| input.mantle_binding.valence_validation_hash = HASH_C.to_string(),
                "evidence_chain.mantle_link_stale",
            ),
            (
                "source-mismatch",
                |_, input| input.mantle_binding.source_artifact_hash = HASH_C.to_string(),
                "evidence_chain.source_link_mismatch",
            ),
            (
                "binary-mismatch",
                |_, input| input.mantle_binding.release_binary_hash = HASH_D.to_string(),
                "evidence_chain.binary_link_mismatch",
            ),
            (
                "unsupported-scope",
                |_, input| input.claim_scope = "release-eligible".to_string(),
                "evidence_chain.claim_scope_unsupported",
            ),
            (
                "stale-policy",
                |_, input| input.policy_hashes[0].digest_blake3 = HASH_D.to_string(),
                "evidence_chain.policy_hash_stale",
            ),
            (
                "projection-drift",
                |_, input| input.projections[0].canonical_envelope_hash = HASH_B.to_string(),
                "evidence_chain.projection_drift",
            ),
            (
                "weakened-non-claims",
                |_, input| input.non_claims.clear(),
                "evidence_chain.non_claim_missing",
            ),
            (
                "overclaim",
                |_, input| {
                    input
                        .non_claims
                        .push("proves release eligibility".to_string())
                },
                "evidence_chain.overclaim",
            ),
            (
                "malformed-digest",
                |_, input| input.mantle_binding.artifact_hash = "not-a-digest".to_string(),
                "evidence_chain.digest_malformed",
            ),
        ];
        for (name, mutate, expected) in cases {
            let mut policy = policy(EVIDENCE_CHAIN_MODE_REQUIRED);
            let mut input = evidence();
            mutate(&mut policy, &mut input);
            let readiness = evaluate_evidence_chain_readiness(&policy, Some(&input));
            assert!(!readiness.passed, "case {name} unexpectedly passed");
            assert!(!readiness.skipped, "case {name} unexpectedly skipped");
            assert_eq!(
                readiness.disposition, EVIDENCE_CHAIN_DISPOSITION_INVALID,
                "case {name}"
            );
            assert!(
                issue_codes(&readiness).contains(&expected),
                "case {name}: {:?}",
                readiness.issues
            );
            assert!(readiness.chain_hash.is_none(), "case {name}");
        }
    }

    #[test]
    fn optional_present_invalid_evidence_does_not_skip() {
        let mut input = evidence();
        input.profile_version = "unknown".to_string();
        let readiness =
            evaluate_evidence_chain_readiness(&policy(EVIDENCE_CHAIN_MODE_OPTIONAL), Some(&input));
        assert!(!readiness.passed);
        assert!(!readiness.skipped);
        assert_eq!(readiness.disposition, EVIDENCE_CHAIN_DISPOSITION_INVALID);
        assert!(issue_codes(&readiness).contains(&"evidence_chain.profile_unsupported"));
    }

    #[test]
    fn required_missing_evidence_fails() {
        let readiness =
            evaluate_evidence_chain_readiness(&policy(EVIDENCE_CHAIN_MODE_REQUIRED), None);
        assert!(!readiness.passed);
        assert!(!readiness.skipped);
        assert!(issue_codes(&readiness).contains(&"evidence_chain.evidence_missing"));
        assert!(readiness.chain_hash.is_none());
    }
}
