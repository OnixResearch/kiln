pub const AUTHENTICATED_EVIDENCE_SCHEMA_V1: &str = "cairn.authenticated-evidence.v1";
pub const AUTHENTICATED_EVIDENCE_PROFILE_DIAGNOSTIC: &str = "diagnostic";
pub const AUTHENTICATED_EVIDENCE_PROFILE_ONIX_RELEASE: &str = "onix-release";
pub const AUTHENTICATED_EVIDENCE_KIND_STACK_PROVENANCE: &str = "stack-provenance";
pub const AUTHENTICATED_EVIDENCE_KIND_FUNCTION_ADDRESS: &str = "function-address";
pub const AUTHENTICATED_EVIDENCE_SIGNATURE_ALGORITHM_ED25519: &str = "ed25519";
pub const AUTHENTICATED_EVIDENCE_CANONICALIZATION_CORRECTED_V1: &str =
    "valence.preserves-integrity.corrected.v1";
pub const AUTHENTICATED_EVIDENCE_BOUNDARY: &str = "Authenticated readiness proves measured-byte linkage, configured producer authorization, and lifecycle-policy conformance only; it does not prove source correctness, verifier soundness, build correctness, semantic equivalence, runtime authority, or universal release fitness";

const BLAKE3_HEX_LENGTH: usize = 64;
const ED25519_PUBLIC_KEY_HEX_LENGTH: usize = 64;
const MAX_MEASUREMENTS_COUNT: usize = 16;
const MAX_PARENT_IDENTITIES_COUNT: usize = 16;
const MAX_POLICY_EXPORTS_COUNT: usize = 16;
const MAX_TRUSTED_PRODUCERS_COUNT: usize = 32;
const ADJACENT_PAIR_WINDOW: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedPolicyExport {
    pub evidence_kind: String,
    pub path: String,
    pub digest_blake3: String,
    pub signature_purpose: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedTrustedProducer {
    pub producer: String,
    pub key_id: String,
    pub algorithm: String,
    pub public_key_hex: String,
    pub allowed_purposes: Vec<String>,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedEvidencePolicy {
    pub schema_version: String,
    pub release_profile: String,
    pub accepted_canonicalization_profiles: Vec<String>,
    pub accepted_valence_integrity_receipt_path: String,
    pub accepted_valence_integrity_receipt_hash: String,
    pub policy_exports: Vec<AuthenticatedPolicyExport>,
    pub trusted_producers: Vec<AuthenticatedTrustedProducer>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MeasuredEvidenceInput {
    pub role: String,
    pub path: String,
    pub declared_blake3: String,
    pub measured_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CanonicalReceiptObservation {
    pub profile: String,
    pub supplied_receipt_hash: String,
    pub recomputed_receipt_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PolicyExportObservation {
    pub path: String,
    pub declared_blake3: String,
    pub measured_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SignatureVerificationObservation {
    pub algorithm: String,
    pub verified: bool,
    pub producer: String,
    pub key_id: String,
    pub purpose: String,
    pub subject_hash: String,
    pub parent_hashes: Vec<String>,
    pub public_key_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedEvidenceInput {
    pub schema_version: String,
    pub evidence_kind: String,
    pub measurements: Vec<MeasuredEvidenceInput>,
    pub receipt: CanonicalReceiptObservation,
    pub policy_export: PolicyExportObservation,
    pub signature: SignatureVerificationObservation,
    pub required_parent_hashes: Vec<String>,
    pub valence_integrity_receipt_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedEvidenceIssue {
    pub code: String,
    pub field_path: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedEvidenceReport {
    pub evidence_kind: String,
    pub passed: bool,
    pub authentication_hash: Option<String>,
    pub measured_subject_hash: String,
    pub producer: String,
    pub key_id: String,
    pub issues: Vec<AuthenticatedEvidenceIssue>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedReleaseGateInput<'a> {
    pub stack_provenance_mode: &'a str,
    pub function_address_mode: &'a str,
    pub stack_provenance: Option<&'a AuthenticatedEvidenceReport>,
    pub function_address: Option<&'a AuthenticatedEvidenceReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedReleaseGateReport {
    pub required: bool,
    pub passed: bool,
    pub issues: Vec<AuthenticatedEvidenceIssue>,
    pub boundary: String,
}

pub fn authenticated_evidence_policy_from_value(
    value: &serde_json::Value,
) -> Result<AuthenticatedEvidencePolicy, String> {
    value
        .get("authenticated_evidence_policy")
        .cloned()
        .ok_or_else(|| "missing authenticated_evidence_policy".to_string())
        .and_then(|policy| serde_json::from_value(policy).map_err(|error| error.to_string()))
}

#[must_use]
pub fn authenticated_signature_message(
    input: &AuthenticatedEvidenceInput,
    canonical_subject: &str,
) -> String {
    let mut measurements = input.measurements.clone();
    measurements.sort_by(|left, right| left.role.cmp(&right.role).then(left.path.cmp(&right.path)));
    let mut parent_hashes = input.required_parent_hashes.clone();
    parent_hashes.sort();
    super::evidence::canonical_json(&serde_json::json!({
        "domain": AUTHENTICATED_EVIDENCE_SCHEMA_V1,
        "evidence_kind": input.evidence_kind,
        "measurements": measurements,
        "policy_export": input.policy_export,
        "producer": input.signature.producer,
        "key_id": input.signature.key_id,
        "purpose": input.signature.purpose,
        "receipt_profile": input.receipt.profile,
        "canonical_subject": canonical_subject,
        "subject_hash": input.receipt.recomputed_receipt_hash,
        "parent_hashes": parent_hashes,
        "valence_integrity_receipt_hash": input.valence_integrity_receipt_hash,
    }))
}

// r[impl cairn.stack_provenance_gates.authenticated_inputs.core]
// r[impl cairn.stack_provenance_gates.authenticated_inputs.final_validation]
// r[impl cairn.stack_provenance_gates.authenticated_inputs.measurement]
// r[impl cairn.stack_provenance_gates.authenticated_inputs.receipt_identity]
// r[impl cairn.stack_provenance_gates.authenticated_inputs.signature_policy]
// r[impl cairn.stack_provenance_gates.authenticated_inputs.policy_identity]
#[must_use]
pub fn evaluate_authenticated_evidence(
    policy: &AuthenticatedEvidencePolicy,
    input: &AuthenticatedEvidenceInput,
) -> AuthenticatedEvidenceReport {
    let mut issues = validate_policy(policy);
    validate_input(policy, input, &mut issues);
    sort_issues(&mut issues);
    let passed = issues.is_empty();
    let authentication_hash = passed.then(|| {
        super::evidence::canonical_json_hash(&serde_json::json!({
            "policy": policy,
            "input": input,
        }))
    });
    AuthenticatedEvidenceReport {
        evidence_kind: input.evidence_kind.clone(),
        passed,
        authentication_hash,
        measured_subject_hash: input.receipt.recomputed_receipt_hash.clone(),
        producer: input.signature.producer.clone(),
        key_id: input.signature.key_id.clone(),
        issues,
        boundary: AUTHENTICATED_EVIDENCE_BOUNDARY.to_string(),
    }
}

// r[impl cairn.stack_provenance_gates.authenticated_inputs.required_release_profile]
#[must_use]
pub fn evaluate_authenticated_release_gate(
    policy: &AuthenticatedEvidencePolicy,
    input: &AuthenticatedReleaseGateInput<'_>,
) -> AuthenticatedReleaseGateReport {
    let required = policy.release_profile == AUTHENTICATED_EVIDENCE_PROFILE_ONIX_RELEASE;
    let mut issues = validate_policy(policy);
    if required {
        validate_onix_release_policy(policy, &mut issues);
        validate_required_component(
            AUTHENTICATED_EVIDENCE_KIND_STACK_PROVENANCE,
            input.stack_provenance_mode,
            input.stack_provenance,
            &mut issues,
        );
        validate_required_component(
            AUTHENTICATED_EVIDENCE_KIND_FUNCTION_ADDRESS,
            input.function_address_mode,
            input.function_address,
            &mut issues,
        );
    }
    sort_issues(&mut issues);
    AuthenticatedReleaseGateReport {
        required,
        passed: issues.is_empty(),
        issues,
        boundary: AUTHENTICATED_EVIDENCE_BOUNDARY.to_string(),
    }
}

fn validate_policy(policy: &AuthenticatedEvidencePolicy) -> Vec<AuthenticatedEvidenceIssue> {
    let mut issues = Vec::new();
    if policy.schema_version != AUTHENTICATED_EVIDENCE_SCHEMA_V1 {
        issues.push(issue(
            "authenticated_inputs.policy_schema_unsupported",
            "policy.schema_version",
            AUTHENTICATED_EVIDENCE_SCHEMA_V1,
            &policy.schema_version,
        ));
    }
    if !matches!(
        policy.release_profile.as_str(),
        AUTHENTICATED_EVIDENCE_PROFILE_DIAGNOSTIC | AUTHENTICATED_EVIDENCE_PROFILE_ONIX_RELEASE
    ) {
        issues.push(issue(
            "authenticated_inputs.release_profile_unsupported",
            "policy.release_profile",
            "diagnostic or onix-release",
            &policy.release_profile,
        ));
    }
    validate_count(
        policy.accepted_canonicalization_profiles.len(),
        MAX_POLICY_EXPORTS_COUNT,
        "policy.accepted_canonicalization_profiles",
        &mut issues,
    );
    if policy.accepted_canonicalization_profiles.is_empty()
        || policy
            .accepted_canonicalization_profiles
            .iter()
            .any(|profile| profile.trim().is_empty())
    {
        issues.push(issue(
            "authenticated_inputs.canonicalization_policy_missing",
            "policy.accepted_canonicalization_profiles",
            "at least one non-empty accepted profile",
            "missing",
        ));
    }
    let mut canonicalization_profiles = policy.accepted_canonicalization_profiles.clone();
    canonicalization_profiles.sort();
    if canonicalization_profiles
        .windows(ADJACENT_PAIR_WINDOW)
        .any(|pair| pair[0] == pair[1])
    {
        issues.push(issue(
            "authenticated_inputs.canonicalization_policy_duplicate",
            "policy.accepted_canonicalization_profiles",
            "unique profiles",
            "duplicate",
        ));
    }
    validate_count(
        policy.policy_exports.len(),
        MAX_POLICY_EXPORTS_COUNT,
        "policy.policy_exports",
        &mut issues,
    );
    validate_count(
        policy.trusted_producers.len(),
        MAX_TRUSTED_PRODUCERS_COUNT,
        "policy.trusted_producers",
        &mut issues,
    );
    if policy
        .accepted_valence_integrity_receipt_path
        .trim()
        .is_empty()
    {
        issues.push(issue(
            "authenticated_inputs.valence_integrity_receipt_path_missing",
            "policy.accepted_valence_integrity_receipt_path",
            "a reviewed receipt path",
            "empty",
        ));
    }
    validate_hash(
        &policy.accepted_valence_integrity_receipt_hash,
        "policy.accepted_valence_integrity_receipt_hash",
        &mut issues,
    );
    reject_placeholder_hash(
        &policy.accepted_valence_integrity_receipt_hash,
        "policy.accepted_valence_integrity_receipt_hash",
        &mut issues,
    );
    for (index, export) in policy.policy_exports.iter().enumerate() {
        validate_hash(
            &export.digest_blake3,
            &format!("policy.policy_exports[{index}].digest_blake3"),
            &mut issues,
        );
        reject_placeholder_hash(
            &export.digest_blake3,
            &format!("policy.policy_exports[{index}].digest_blake3"),
            &mut issues,
        );
        if export.evidence_kind.trim().is_empty()
            || export.path.trim().is_empty()
            || export.signature_purpose.trim().is_empty()
        {
            issues.push(issue(
                "authenticated_inputs.policy_export_malformed",
                &format!("policy.policy_exports[{index}]"),
                "non-empty kind, path, and purpose",
                "empty",
            ));
        }
    }
    validate_unique_policy_exports(&policy.policy_exports, &mut issues);
    validate_trusted_producers(&policy.trusted_producers, &mut issues);
    issues
}

fn validate_input(
    policy: &AuthenticatedEvidencePolicy,
    input: &AuthenticatedEvidenceInput,
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    if input.schema_version != AUTHENTICATED_EVIDENCE_SCHEMA_V1 {
        issues.push(issue(
            "authenticated_inputs.input_schema_unsupported",
            "input.schema_version",
            AUTHENTICATED_EVIDENCE_SCHEMA_V1,
            &input.schema_version,
        ));
    }
    validate_measurements(&input.measurements, issues);
    validate_receipt(policy, &input.receipt, issues);
    validate_policy_export(policy, input, issues);
    validate_signature(policy, input, issues);
    // r[impl cairn.stack_provenance_gates.authenticated_inputs.cross_repo_dependency]
    validate_hash(
        &input.valence_integrity_receipt_hash,
        "input.valence_integrity_receipt_hash",
        issues,
    );
    if input.valence_integrity_receipt_hash != policy.accepted_valence_integrity_receipt_hash {
        issues.push(issue(
            "authenticated_inputs.valence_integrity_receipt_stale",
            "input.valence_integrity_receipt_hash",
            &policy.accepted_valence_integrity_receipt_hash,
            &input.valence_integrity_receipt_hash,
        ));
    }
}

fn validate_measurements(
    measurements: &[MeasuredEvidenceInput],
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    validate_count(
        measurements.len(),
        MAX_MEASUREMENTS_COUNT,
        "input.measurements",
        issues,
    );
    if measurements.is_empty() {
        issues.push(issue(
            "authenticated_inputs.measurement_missing",
            "input.measurements",
            "at least one measured input",
            "empty",
        ));
    }
    let mut roles = Vec::new();
    for (index, measurement) in measurements.iter().enumerate() {
        let field = format!("input.measurements[{index}]");
        if measurement.role.trim().is_empty() || measurement.path.trim().is_empty() {
            issues.push(issue(
                "authenticated_inputs.measurement_malformed",
                &field,
                "non-empty role and path",
                "empty",
            ));
        }
        roles.push(measurement.role.as_str());
        validate_hash(
            &measurement.declared_blake3,
            &format!("{field}.declared_blake3"),
            issues,
        );
        validate_hash(
            &measurement.measured_blake3,
            &format!("{field}.measured_blake3"),
            issues,
        );
        if measurement.declared_blake3 != measurement.measured_blake3 {
            issues.push(issue(
                "authenticated_inputs.measured_digest_mismatch",
                &format!("{field}.declared_blake3"),
                &measurement.measured_blake3,
                &measurement.declared_blake3,
            ));
        }
    }
    roles.sort_unstable();
    if roles
        .windows(ADJACENT_PAIR_WINDOW)
        .any(|pair| pair[0] == pair[1])
    {
        issues.push(issue(
            "authenticated_inputs.measurement_role_duplicate",
            "input.measurements",
            "unique measured roles",
            "duplicate",
        ));
    }
}

fn validate_receipt(
    policy: &AuthenticatedEvidencePolicy,
    receipt: &CanonicalReceiptObservation,
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    if !policy
        .accepted_canonicalization_profiles
        .contains(&receipt.profile)
    {
        issues.push(issue(
            "authenticated_inputs.canonicalization_profile_unsupported",
            "input.receipt.profile",
            "an accepted canonicalization profile",
            &receipt.profile,
        ));
    }
    validate_hash(
        &receipt.supplied_receipt_hash,
        "input.receipt.supplied_receipt_hash",
        issues,
    );
    validate_hash(
        &receipt.recomputed_receipt_hash,
        "input.receipt.recomputed_receipt_hash",
        issues,
    );
    if receipt.supplied_receipt_hash != receipt.recomputed_receipt_hash {
        issues.push(issue(
            "authenticated_inputs.receipt_identity_mismatch",
            "input.receipt.supplied_receipt_hash",
            &receipt.recomputed_receipt_hash,
            &receipt.supplied_receipt_hash,
        ));
    }
}

fn validate_policy_export(
    policy: &AuthenticatedEvidencePolicy,
    input: &AuthenticatedEvidenceInput,
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    let expected = policy
        .policy_exports
        .iter()
        .find(|export| export.evidence_kind == input.evidence_kind);
    let Some(expected) = expected else {
        issues.push(issue(
            "authenticated_inputs.policy_export_missing",
            "input.evidence_kind",
            "a configured policy export",
            &input.evidence_kind,
        ));
        return;
    };
    validate_hash(
        &input.policy_export.declared_blake3,
        "input.policy_export.declared_blake3",
        issues,
    );
    validate_hash(
        &input.policy_export.measured_blake3,
        "input.policy_export.measured_blake3",
        issues,
    );
    if input.policy_export.path != expected.path
        || input.policy_export.declared_blake3 != expected.digest_blake3
        || input.policy_export.measured_blake3 != expected.digest_blake3
    {
        issues.push(issue(
            "authenticated_inputs.policy_identity_stale",
            "input.policy_export",
            &format!("{}@{}", expected.path, expected.digest_blake3),
            &format!(
                "{}@{}@{}",
                input.policy_export.path,
                input.policy_export.declared_blake3,
                input.policy_export.measured_blake3
            ),
        ));
    }
}

fn validate_signature(
    policy: &AuthenticatedEvidencePolicy,
    input: &AuthenticatedEvidenceInput,
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    let signature = &input.signature;
    if signature.algorithm != AUTHENTICATED_EVIDENCE_SIGNATURE_ALGORITHM_ED25519 {
        issues.push(issue(
            "authenticated_inputs.signature_algorithm_unsupported",
            "input.signature.algorithm",
            AUTHENTICATED_EVIDENCE_SIGNATURE_ALGORITHM_ED25519,
            &signature.algorithm,
        ));
    }
    if !signature.verified {
        issues.push(issue(
            "authenticated_inputs.signature_unverified",
            "input.signature.verified",
            "true",
            "false",
        ));
    }
    let trusted = policy.trusted_producers.iter().find(|producer| {
        producer.producer == signature.producer && producer.key_id == signature.key_id
    });
    let Some(trusted) = trusted else {
        issues.push(issue(
            "authenticated_inputs.signer_unauthorized",
            "input.signature.key_id",
            "an authorized producer key",
            &format!("{}@{}", signature.producer, signature.key_id),
        ));
        return;
    };
    if trusted.revoked {
        issues.push(issue(
            "authenticated_inputs.signer_revoked",
            "input.signature.key_id",
            "a non-revoked key",
            &trusted.key_id,
        ));
    }
    if !trusted.allowed_purposes.contains(&signature.purpose) {
        issues.push(issue(
            "authenticated_inputs.signature_purpose_unauthorized",
            "input.signature.purpose",
            &trusted.allowed_purposes.join(","),
            &signature.purpose,
        ));
    }
    if let Some(export) = policy
        .policy_exports
        .iter()
        .find(|export| export.evidence_kind == input.evidence_kind)
    {
        if signature.purpose != export.signature_purpose {
            issues.push(issue(
                "authenticated_inputs.signature_purpose_mismatch",
                "input.signature.purpose",
                &export.signature_purpose,
                &signature.purpose,
            ));
        }
    }
    if signature.subject_hash != input.receipt.recomputed_receipt_hash {
        issues.push(issue(
            "authenticated_inputs.signature_subject_mismatch",
            "input.signature.subject_hash",
            &input.receipt.recomputed_receipt_hash,
            &signature.subject_hash,
        ));
    }
    let mut observed_parents = signature.parent_hashes.clone();
    let mut required_parents = input.required_parent_hashes.clone();
    observed_parents.sort();
    required_parents.sort();
    if observed_parents != required_parents {
        issues.push(issue(
            "authenticated_inputs.signature_parent_mismatch",
            "input.signature.parent_hashes",
            &required_parents.join(","),
            &observed_parents.join(","),
        ));
    }
    validate_count(
        required_parents.len(),
        MAX_PARENT_IDENTITIES_COUNT,
        "input.required_parent_hashes",
        issues,
    );
    if required_parents.is_empty() {
        issues.push(issue(
            "authenticated_inputs.signature_parent_missing",
            "input.required_parent_hashes",
            "at least one linked parent identity",
            "empty",
        ));
    }
    let mut linked_hashes = input
        .measurements
        .iter()
        .map(|measurement| measurement.measured_blake3.as_str())
        .collect::<Vec<_>>();
    linked_hashes.push(input.policy_export.measured_blake3.as_str());
    linked_hashes.push(input.valence_integrity_receipt_hash.as_str());
    for (index, parent) in required_parents.iter().enumerate() {
        validate_hash(
            parent,
            &format!("input.required_parent_hashes[{index}]"),
            issues,
        );
        if !linked_hashes.contains(&parent.as_str()) {
            issues.push(issue(
                "authenticated_inputs.signature_parent_unlinked",
                &format!("input.required_parent_hashes[{index}]"),
                "a measured artifact, policy export, or integrity-receipt identity",
                parent,
            ));
        }
    }
    let expected_public_key_blake3 = blake3::hash(trusted.public_key_hex.as_bytes()).to_hex();
    if signature.public_key_blake3 != expected_public_key_blake3.as_str() {
        issues.push(issue(
            "authenticated_inputs.signature_key_identity_mismatch",
            "input.signature.public_key_blake3",
            expected_public_key_blake3.as_str(),
            &signature.public_key_blake3,
        ));
    }
}

fn validate_trusted_producers(
    producers: &[AuthenticatedTrustedProducer],
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    let mut keys = Vec::new();
    for (index, producer) in producers.iter().enumerate() {
        keys.push((producer.producer.as_str(), producer.key_id.as_str()));
        if producer.producer.trim().is_empty()
            || producer.key_id.trim().is_empty()
            || producer.allowed_purposes.is_empty()
        {
            issues.push(issue(
                "authenticated_inputs.trusted_producer_malformed",
                &format!("policy.trusted_producers[{index}]"),
                "non-empty producer, key id, and purposes",
                "empty",
            ));
        }
        if producer.algorithm != AUTHENTICATED_EVIDENCE_SIGNATURE_ALGORITHM_ED25519
            || producer.public_key_hex.len() != ED25519_PUBLIC_KEY_HEX_LENGTH
            || !is_lower_hex(&producer.public_key_hex)
        {
            issues.push(issue(
                "authenticated_inputs.trusted_key_malformed",
                &format!("policy.trusted_producers[{index}].public_key_hex"),
                "an Ed25519 public key as 64 lowercase hexadecimal characters",
                &producer.public_key_hex,
            ));
        }
    }
    keys.sort_unstable();
    if keys
        .windows(ADJACENT_PAIR_WINDOW)
        .any(|pair| pair[0] == pair[1])
    {
        issues.push(issue(
            "authenticated_inputs.trusted_key_duplicate",
            "policy.trusted_producers",
            "unique producer and key-id pairs",
            "duplicate",
        ));
    }
}

fn validate_unique_policy_exports(
    exports: &[AuthenticatedPolicyExport],
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    let mut kinds = exports
        .iter()
        .map(|export| export.evidence_kind.as_str())
        .collect::<Vec<_>>();
    kinds.sort_unstable();
    if kinds
        .windows(ADJACENT_PAIR_WINDOW)
        .any(|pair| pair[0] == pair[1])
    {
        issues.push(issue(
            "authenticated_inputs.policy_export_duplicate",
            "policy.policy_exports",
            "unique evidence kinds",
            "duplicate",
        ));
    }
}

fn validate_onix_release_policy(
    policy: &AuthenticatedEvidencePolicy,
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    if !policy
        .accepted_canonicalization_profiles
        .iter()
        .any(|profile| profile == AUTHENTICATED_EVIDENCE_CANONICALIZATION_CORRECTED_V1)
    {
        issues.push(issue(
            "authenticated_inputs.corrected_valence_profile_missing",
            "policy.accepted_canonicalization_profiles",
            AUTHENTICATED_EVIDENCE_CANONICALIZATION_CORRECTED_V1,
            "missing",
        ));
    }
    for kind in [
        AUTHENTICATED_EVIDENCE_KIND_STACK_PROVENANCE,
        AUTHENTICATED_EVIDENCE_KIND_FUNCTION_ADDRESS,
    ] {
        if !policy
            .policy_exports
            .iter()
            .any(|export| export.evidence_kind == kind)
        {
            issues.push(issue(
                "authenticated_inputs.release_policy_export_missing",
                &format!("policy.policy_exports.{kind}"),
                "a reviewed policy export",
                "missing",
            ));
        }
    }
    if !policy
        .trusted_producers
        .iter()
        .any(|producer| !producer.revoked)
    {
        issues.push(issue(
            "authenticated_inputs.release_trust_root_missing",
            "policy.trusted_producers",
            "at least one non-revoked trusted producer",
            "missing",
        ));
    }
}

fn validate_required_component(
    kind: &str,
    mode: &str,
    report: Option<&AuthenticatedEvidenceReport>,
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    if mode != "required" {
        issues.push(issue(
            "authenticated_inputs.release_gate_not_required",
            &format!("release.{kind}.mode"),
            "required",
            mode,
        ));
    }
    match report {
        Some(report) if report.passed && report.authentication_hash.is_some() => {}
        Some(report) => issues.push(issue(
            "authenticated_inputs.release_gate_failed",
            &format!("release.{kind}.authentication"),
            "passing authenticated evidence",
            if report.passed {
                "missing hash"
            } else {
                "failed"
            },
        )),
        None => issues.push(issue(
            "authenticated_inputs.release_gate_missing",
            &format!("release.{kind}.authentication"),
            "present authenticated evidence",
            "missing",
        )),
    }
}

fn validate_hash(value: &str, field_path: &str, issues: &mut Vec<AuthenticatedEvidenceIssue>) {
    if value.len() != BLAKE3_HEX_LENGTH || !is_lower_hex(value) {
        issues.push(issue(
            "authenticated_inputs.digest_malformed",
            field_path,
            "64 lowercase BLAKE3 hexadecimal characters",
            value,
        ));
    }
}

fn reject_placeholder_hash(
    value: &str,
    field_path: &str,
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    let mut chars = value.chars();
    let first = chars.next();
    if first.is_some() && chars.all(|character| Some(character) == first) {
        issues.push(issue(
            "authenticated_inputs.policy_identity_placeholder",
            field_path,
            "a generated reviewed policy identity",
            value,
        ));
    }
}

fn is_lower_hex(value: &str) -> bool {
    value
        .chars()
        .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

fn validate_count(
    actual: usize,
    maximum: usize,
    field_path: &str,
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) {
    if actual > maximum {
        issues.push(issue(
            "authenticated_inputs.bound_exceeded",
            field_path,
            &format!("at most {maximum} entries"),
            &actual.to_string(),
        ));
    }
}

fn sort_issues(issues: &mut Vec<AuthenticatedEvidenceIssue>) {
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.field_path.cmp(&right.field_path))
            .then(left.actual.cmp(&right.actual))
    });
    issues.dedup();
}

fn issue(code: &str, field_path: &str, expected: &str, actual: &str) -> AuthenticatedEvidenceIssue {
    AuthenticatedEvidenceIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

#[cfg(test)]
// r[verify cairn.stack_provenance_gates.authenticated_inputs.final_validation]
mod tests {
    use super::*;

    const HASH_A: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const HASH_B: &str = "123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0";
    const HASH_C: &str = "23456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef01";
    const HASH_D: &str = "93993e99e87206d232b64efcc8631611aad3b5b0c4b81855e7e1a0cdcc391cff";
    const PUBLIC_KEY: &str = "456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123";
    const PURPOSE: &str = "onix.stack-provenance.release-evidence.v1";
    const FUNCTION_PURPOSE: &str = "onix.function-address.release-evidence.v1";

    type InvalidCase = (
        &'static str,
        fn(&mut AuthenticatedEvidencePolicy, &mut AuthenticatedEvidenceInput),
        &'static str,
    );

    fn policy() -> AuthenticatedEvidencePolicy {
        AuthenticatedEvidencePolicy {
            schema_version: AUTHENTICATED_EVIDENCE_SCHEMA_V1.to_string(),
            release_profile: AUTHENTICATED_EVIDENCE_PROFILE_ONIX_RELEASE.to_string(),
            accepted_canonicalization_profiles: vec![
                AUTHENTICATED_EVIDENCE_CANONICALIZATION_CORRECTED_V1.to_string(),
            ],
            accepted_valence_integrity_receipt_path: "valence-integrity.json".to_string(),
            accepted_valence_integrity_receipt_hash: HASH_C.to_string(),
            policy_exports: vec![
                AuthenticatedPolicyExport {
                    evidence_kind: AUTHENTICATED_EVIDENCE_KIND_STACK_PROVENANCE.to_string(),
                    path: "policy.json".to_string(),
                    digest_blake3: HASH_B.to_string(),
                    signature_purpose: PURPOSE.to_string(),
                },
                AuthenticatedPolicyExport {
                    evidence_kind: AUTHENTICATED_EVIDENCE_KIND_FUNCTION_ADDRESS.to_string(),
                    path: "function-policy.json".to_string(),
                    digest_blake3: HASH_C.to_string(),
                    signature_purpose: FUNCTION_PURPOSE.to_string(),
                },
            ],
            trusted_producers: vec![AuthenticatedTrustedProducer {
                producer: "valence".to_string(),
                key_id: "release-key".to_string(),
                algorithm: AUTHENTICATED_EVIDENCE_SIGNATURE_ALGORITHM_ED25519.to_string(),
                public_key_hex: PUBLIC_KEY.to_string(),
                allowed_purposes: vec![PURPOSE.to_string(), FUNCTION_PURPOSE.to_string()],
                revoked: false,
            }],
        }
    }

    fn input() -> AuthenticatedEvidenceInput {
        AuthenticatedEvidenceInput {
            schema_version: AUTHENTICATED_EVIDENCE_SCHEMA_V1.to_string(),
            evidence_kind: AUTHENTICATED_EVIDENCE_KIND_STACK_PROVENANCE.to_string(),
            measurements: vec![MeasuredEvidenceInput {
                role: "receipt".to_string(),
                path: "receipt.json".to_string(),
                declared_blake3: HASH_A.to_string(),
                measured_blake3: HASH_A.to_string(),
            }],
            receipt: CanonicalReceiptObservation {
                profile: AUTHENTICATED_EVIDENCE_CANONICALIZATION_CORRECTED_V1.to_string(),
                supplied_receipt_hash: HASH_A.to_string(),
                recomputed_receipt_hash: HASH_A.to_string(),
            },
            policy_export: PolicyExportObservation {
                path: "policy.json".to_string(),
                declared_blake3: HASH_B.to_string(),
                measured_blake3: HASH_B.to_string(),
            },
            signature: SignatureVerificationObservation {
                algorithm: AUTHENTICATED_EVIDENCE_SIGNATURE_ALGORITHM_ED25519.to_string(),
                verified: true,
                producer: "valence".to_string(),
                key_id: "release-key".to_string(),
                purpose: PURPOSE.to_string(),
                subject_hash: HASH_A.to_string(),
                parent_hashes: vec![HASH_B.to_string()],
                public_key_blake3: blake3::hash(PUBLIC_KEY.as_bytes()).to_hex().to_string(),
            },
            required_parent_hashes: vec![HASH_B.to_string()],
            valence_integrity_receipt_hash: HASH_C.to_string(),
        }
    }

    fn function_input() -> AuthenticatedEvidenceInput {
        let mut input = input();
        input.evidence_kind = AUTHENTICATED_EVIDENCE_KIND_FUNCTION_ADDRESS.to_string();
        input.policy_export = PolicyExportObservation {
            path: "function-policy.json".to_string(),
            declared_blake3: HASH_C.to_string(),
            measured_blake3: HASH_C.to_string(),
        };
        input.signature.purpose = FUNCTION_PURPOSE.to_string();
        input.signature.parent_hashes = vec![HASH_C.to_string()];
        input.required_parent_hashes = vec![HASH_C.to_string()];
        input
    }

    fn codes(report: &AuthenticatedEvidenceReport) -> Vec<&str> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect()
    }

    // r[verify cairn.stack_provenance_gates.authenticated_inputs.fixtures.positive]
    #[test]
    fn complete_authenticated_input_passes() {
        let report = evaluate_authenticated_evidence(&policy(), &input());
        assert!(report.passed, "{:?}", report.issues);
        assert!(report.authentication_hash.is_some());
    }

    // r[verify cairn.stack_provenance_gates.authenticated_inputs.fixtures.negative]
    #[test]
    fn fabricated_tampered_stale_and_unauthorized_inputs_fail() {
        let cases: Vec<InvalidCase> = vec![
            (
                "fabricated-hash",
                |_, input| input.measurements[0].declared_blake3 = HASH_B.to_string(),
                "authenticated_inputs.measured_digest_mismatch",
            ),
            (
                "tampered-receipt",
                |_, input| input.receipt.recomputed_receipt_hash = HASH_B.to_string(),
                "authenticated_inputs.receipt_identity_mismatch",
            ),
            (
                "wrong-domain",
                |_, input| input.receipt.profile = "unknown".to_string(),
                "authenticated_inputs.canonicalization_profile_unsupported",
            ),
            (
                "stale-policy",
                |_, input| input.policy_export.measured_blake3 = HASH_A.to_string(),
                "authenticated_inputs.policy_identity_stale",
            ),
            (
                "placeholder-policy",
                |policy, _| policy.policy_exports[0].digest_blake3 = "a".repeat(BLAKE3_HEX_LENGTH),
                "authenticated_inputs.policy_identity_placeholder",
            ),
            (
                "missing-signature",
                |_, input| input.signature.verified = false,
                "authenticated_inputs.signature_unverified",
            ),
            (
                "wrong-purpose",
                |_, input| input.signature.purpose = "other".to_string(),
                "authenticated_inputs.signature_purpose_unauthorized",
            ),
            (
                "unauthorized-signer",
                |_, input| input.signature.key_id = "unknown".to_string(),
                "authenticated_inputs.signer_unauthorized",
            ),
            (
                "revoked-key",
                |policy, _| policy.trusted_producers[0].revoked = true,
                "authenticated_inputs.signer_revoked",
            ),
            (
                "unlinked-parent",
                |_, input| {
                    input.signature.parent_hashes = vec![HASH_D.to_string()];
                    input.required_parent_hashes = vec![HASH_D.to_string()];
                },
                "authenticated_inputs.signature_parent_unlinked",
            ),
        ];
        for (name, mutate, expected) in cases {
            let mut policy = policy();
            let mut input = input();
            mutate(&mut policy, &mut input);
            let report = evaluate_authenticated_evidence(&policy, &input);
            assert!(!report.passed, "case {name} unexpectedly passed");
            assert!(
                codes(&report).contains(&expected),
                "case {name}: {:?}",
                report.issues
            );
            assert!(report.authentication_hash.is_none(), "case {name}");
        }
    }

    #[test]
    fn onix_release_requires_both_required_authenticated_gates() {
        let stack = evaluate_authenticated_evidence(&policy(), &input());
        let function = evaluate_authenticated_evidence(&policy(), &function_input());
        assert!(stack.passed, "{:?}", stack.issues);
        assert!(function.passed, "{:?}", function.issues);
        let accepted = evaluate_authenticated_release_gate(
            &policy(),
            &AuthenticatedReleaseGateInput {
                stack_provenance_mode: "required",
                function_address_mode: "required",
                stack_provenance: Some(&stack),
                function_address: Some(&function),
            },
        );
        assert!(accepted.passed, "{:?}", accepted.issues);

        let blocked = evaluate_authenticated_release_gate(
            &policy(),
            &AuthenticatedReleaseGateInput {
                stack_provenance_mode: "required",
                function_address_mode: "optional",
                stack_provenance: Some(&stack),
                function_address: None,
            },
        );
        assert!(!blocked.passed);
        assert!(blocked
            .issues
            .iter()
            .any(|issue| issue.code == "authenticated_inputs.release_gate_not_required"));
    }
}
