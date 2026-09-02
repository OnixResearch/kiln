//! Pure one-way mapping from already protocol-verified Radicle observations into artifact-auth.
//!
//! Radicle verification, actor authorization, Evidence IR, COB reduction, Git storage, SSH-agent
//! effects, lifecycle handoff, and release interpretation remain outside this module.

use std::collections::BTreeSet;

use artifact_auth_core::ALGORITHM_BLAKE3;
use artifact_auth_core::ALGORITHM_ED25519;
use artifact_auth_core::ArtifactRef;
use artifact_auth_core::ArtifactStatement;
use artifact_auth_core::AuthenticationDecision;
use artifact_auth_core::AuthenticationPolicy;
use artifact_auth_core::AuthenticationScope;
use artifact_auth_core::CryptographicObservation;
use artifact_auth_core::ED25519_PUBLIC_KEY_PROFILE_V1;
use artifact_auth_core::KeyCurrentness;
use artifact_auth_core::POLICY_SCHEMA_V1;
use artifact_auth_core::STATEMENT_SCHEMA_V1;
use artifact_auth_core::SignatureEvidence;
use artifact_auth_core::TrustedKeyObservation;
use artifact_auth_core::evaluate_authentication;
use artifact_auth_core::required_non_claims;

use crate::cob_evidence::ObservedCobEvent;

const THRESHOLD_ONE: u16 = 1;
const GIT_OID_HEX_CHARS: usize = 40;
const BLAKE3_HEX_CHARS: usize = 64;
const RADICLE_ENTRY_PROFILE: &str = "radicle-entry-oid.v1";
const RADICLE_PARENT_PROFILE: &str = "radicle-parent-oid.v1";
const RADICLE_RELATED_PROFILE: &str = "radicle-related-target.v1";
const VALENCE_PROFILE_CONTEXT: &str = "valence-artifact-auth-profile-context.v1";
const VALENCE_CURRENTNESS_CONTEXT: &str = "valence-artifact-auth-currentness.v1";
const ARTIFACT_AUTH_DOMAIN: &str = "valence.radicle-cob-evidence.artifact-auth.v1";
const ARTIFACT_AUTH_PURPOSE: &str = "radicle-cob-evidence";
const ENTRY_HASH_DOMAIN: &[u8] = b"valence.artifact-auth.radicle-entry.v1\0";
const PARENT_HASH_DOMAIN: &[u8] = b"valence.artifact-auth.radicle-parent.v1\0";
const RELATED_HASH_DOMAIN: &[u8] = b"valence.artifact-auth.radicle-related.v1\0";
const PROFILE_HASH_DOMAIN: &[u8] = b"valence.artifact-auth.profile-context.v1\0";
const CURRENTNESS_HASH_DOMAIN: &[u8] = b"valence.artifact-auth.currentness.v1\0";
const FAILED_VERIFICATION: &str = "valence-standalone-cryptographic-verification-failed";
const PREIMAGE_CLASS: &str = "radicle-protocol-preimage-vs-artifact-auth-statement";
const AUTHORITY_BOUNDARY: &str = "standalone authentication is a bounded post-Radicle observation only; Valence retains protocol verification, actor authorization, Evidence IR identity and roles, COB reduction, Git storage, SSH-agent effects, lifecycle handoff, and release interpretation";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValenceArtifactAuthStatementInput<'a> {
    pub observed: &'a ObservedCobEvent,
    pub related_target: &'a str,
    pub profile_id: &'a str,
    pub profile_context: &'a str,
    pub generation: u64,
    pub currentness: KeyCurrentness,
    pub currentness_context: &'a str,
    pub protocol_key_identity_blake3: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValenceArtifactAuthObservation<'a> {
    pub observed: &'a ObservedCobEvent,
    pub related_target: &'a str,
    pub profile_id: &'a str,
    pub profile_context: &'a str,
    pub generation: u64,
    pub currentness: KeyCurrentness,
    pub currentness_context: &'a str,
    pub protocol_key_identity_blake3: &'a str,
    pub standalone_cryptographic: CryptographicObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValenceArtifactAuthCompatibility {
    pub case_explained: bool,
    pub preimage_class: String,
    pub identity_drift_explained: bool,
    pub decision_drift: bool,
    pub issue_class: String,
    pub protocol_failure_causes: Vec<String>,
    pub standalone_failure_causes: Vec<String>,
    pub non_claim_drift: bool,
    pub blockers: Vec<String>,
    pub legacy_authoritative: bool,
    pub standalone_authority_admitted: bool,
    pub rollback_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValenceArtifactAuthReport {
    pub protocol_signature_valid: bool,
    pub standalone: Option<AuthenticationDecision>,
    pub compatibility: ValenceArtifactAuthCompatibility,
    pub radicle_entry_oid: String,
    pub radicle_public_key: String,
    pub git_parent_oids: Vec<String>,
    pub related_target: String,
    pub authority_boundary: String,
}

// r[impl valence.artifact_auth_adoption.authority]
// r[impl valence.artifact_auth_adoption.cutover]
#[must_use]
pub fn evaluate_valence_artifact_auth(observation: &ValenceArtifactAuthObservation<'_>) -> ValenceArtifactAuthReport {
    let mapped = map_observation(observation);
    let standalone = mapped
        .as_ref()
        .ok()
        .map(|(policy, scope, evidence)| evaluate_authentication(policy, scope, evidence));
    let mapping_blockers = mapped.err().unwrap_or_default();
    let compatibility = compare(observation, standalone.as_ref(), mapping_blockers);
    ValenceArtifactAuthReport {
        protocol_signature_valid: observation.observed.signature_valid,
        standalone,
        compatibility,
        radicle_entry_oid: observation.observed.entry_oid.clone(),
        radicle_public_key: observation.observed.signer.clone(),
        git_parent_oids: observation.observed.parent_oids.clone(),
        related_target: observation.related_target.to_string(),
        authority_boundary: AUTHORITY_BOUNDARY.to_string(),
    }
}

// r[impl valence.artifact_auth_shell.exact_verification]
/// Map a protocol observation to the exact signer-specific standalone statement.
pub fn map_valence_artifact_auth_statement(
    input: &ValenceArtifactAuthStatementInput<'_>,
) -> Result<ArtifactStatement, Vec<String>> {
    let (_, _, statement) = map_statement_and_policy(input)?;
    Ok(statement)
}

fn map_observation(
    observation: &ValenceArtifactAuthObservation<'_>,
) -> Result<(AuthenticationPolicy, AuthenticationScope, Vec<SignatureEvidence>), Vec<String>> {
    let input = ValenceArtifactAuthStatementInput {
        observed: observation.observed,
        related_target: observation.related_target,
        profile_id: observation.profile_id,
        profile_context: observation.profile_context,
        generation: observation.generation,
        currentness: observation.currentness,
        currentness_context: observation.currentness_context,
        protocol_key_identity_blake3: observation.protocol_key_identity_blake3,
    };
    let (policy, scope, statement) = map_statement_and_policy(&input)?;
    let evidence = vec![SignatureEvidence {
        statement,
        generation: observation.generation,
        cryptographic: observation.standalone_cryptographic.clone(),
    }];
    Ok((policy, scope, evidence))
}

fn map_statement_and_policy(
    input: &ValenceArtifactAuthStatementInput<'_>,
) -> Result<(AuthenticationPolicy, AuthenticationScope, ArtifactStatement), Vec<String>> {
    let observed = input.observed;
    validate_oid(&observed.entry_oid, "entry_oid")?;
    validate_oid(input.related_target, "related_target")?;
    if !observed.related_oids.iter().any(|oid| oid == input.related_target) {
        return Err(vec!["related_target:not-observed".to_string()]);
    }
    let mut seen_parents = BTreeSet::new();
    let mut parents = Vec::with_capacity(observed.parent_oids.len().saturating_add(1));
    for parent in &observed.parent_oids {
        validate_oid(parent, "parent_oid")?;
        if !seen_parents.insert(parent.as_str()) {
            return Err(vec!["parent_oid:duplicate".to_string()]);
        }
        parents.push(hashed_ref(RADICLE_PARENT_PROFILE, PARENT_HASH_DOMAIN, parent));
    }
    parents.push(hashed_ref(RADICLE_RELATED_PROFILE, RELATED_HASH_DOMAIN, input.related_target));
    if !valid_blake3_hex(input.protocol_key_identity_blake3) {
        return Err(vec!["protocol_key_identity_blake3:malformed".to_string()]);
    }
    if input.profile_id.is_empty() || input.profile_context.is_empty() || input.currentness_context.is_empty() {
        return Err(vec!["profile_or_currentness_context:empty".to_string()]);
    }
    let key_identity = ArtifactRef {
        profile: ED25519_PUBLIC_KEY_PROFILE_V1.to_string(),
        algorithm: ALGORITHM_BLAKE3.to_string(),
        digest_hex: input.protocol_key_identity_blake3.to_string(),
    };
    let scope = AuthenticationScope {
        domain: ARTIFACT_AUTH_DOMAIN.to_string(),
        purpose: ARTIFACT_AUTH_PURPOSE.to_string(),
        profile_id: input.profile_id.to_string(),
        subject: hashed_ref(RADICLE_ENTRY_PROFILE, ENTRY_HASH_DOMAIN, &observed.entry_oid),
        parents,
        verifier_context: hashed_ref(VALENCE_PROFILE_CONTEXT, PROFILE_HASH_DOMAIN, input.profile_context),
    };
    let policy = AuthenticationPolicy {
        schema: POLICY_SCHEMA_V1.to_string(),
        profile_id: input.profile_id.to_string(),
        threshold: THRESHOLD_ONE,
        trusted_keys: vec![TrustedKeyObservation {
            producer_id: observed.signer.clone(),
            key_id: observed.signer.clone(),
            key_identity: key_identity.clone(),
            allowed_purposes: vec![ARTIFACT_AUTH_PURPOSE.to_string()],
            generation: input.generation,
            currentness: input.currentness,
            currentness_ref: hashed_ref(
                VALENCE_CURRENTNESS_CONTEXT,
                CURRENTNESS_HASH_DOMAIN,
                input.currentness_context,
            ),
        }],
    };
    let statement = ArtifactStatement {
        schema: STATEMENT_SCHEMA_V1.to_string(),
        scope: scope.clone(),
        producer_id: observed.signer.clone(),
        key_id: observed.signer.clone(),
        key_identity,
    };
    Ok((policy, scope, statement))
}

fn compare(
    observation: &ValenceArtifactAuthObservation<'_>,
    standalone: Option<&AuthenticationDecision>,
    mut blockers: Vec<String>,
) -> ValenceArtifactAuthCompatibility {
    let protocol_causes = protocol_failure_causes(observation.observed);
    let standalone_causes = standalone.map_or_else(BTreeSet::new, standalone_failure_causes);
    let protocol_passed = observation.observed.signature_valid;
    let standalone_passed = standalone.is_some_and(|decision| decision.passed);
    let decision_drift = standalone.is_some() && protocol_passed != standalone_passed;
    if standalone.is_none() {
        blockers.push("standalone-evaluation-unavailable".to_string());
    }
    if decision_drift {
        blockers.push("decision-drift".to_string());
    }
    let identity_drift_explained =
        observation.standalone_cryptographic.key_identity.digest_hex == observation.protocol_key_identity_blake3;
    if !identity_drift_explained {
        blockers.push("identity-drift".to_string());
    }
    let non_claim_drift = standalone.is_none_or(|decision| decision.non_claims != required_non_claims());
    if non_claim_drift {
        blockers.push("non-claim-drift".to_string());
    }
    if !protocol_passed && !standalone_passed && protocol_causes.is_disjoint(&standalone_causes) {
        blockers.push("unrelated-rejection-causes".to_string());
    }
    if !protocol_passed && !standalone_passed && observation.observed.decode_issue.is_some() {
        blockers.push("unrelated-protocol-and-decode-failures".to_string());
    }
    blockers.sort();
    blockers.dedup();
    ValenceArtifactAuthCompatibility {
        case_explained: blockers.is_empty(),
        preimage_class: PREIMAGE_CLASS.to_string(),
        identity_drift_explained,
        decision_drift,
        issue_class: if protocol_passed && standalone_passed {
            "no-issues".to_string()
        } else {
            "protocol-vs-standalone-taxonomy".to_string()
        },
        protocol_failure_causes: protocol_causes.into_iter().collect(),
        standalone_failure_causes: standalone_causes.into_iter().collect(),
        non_claim_drift,
        blockers,
        legacy_authoritative: true,
        standalone_authority_admitted: false,
        rollback_available: true,
    }
}

fn protocol_failure_causes(observed: &ObservedCobEvent) -> BTreeSet<String> {
    let mut causes = BTreeSet::new();
    if !observed.signature_valid {
        causes.insert("signature".to_string());
    }
    if observed.decode_issue.is_some() {
        causes.insert("decode".to_string());
    }
    causes
}

fn standalone_failure_causes(decision: &AuthenticationDecision) -> BTreeSet<String> {
    decision.issues.iter().map(|issue| standalone_issue_class(&issue.code)).collect()
}

fn standalone_issue_class(issue_code: &str) -> String {
    if issue_code.contains("crypto") || issue_code.contains("signature") || issue_code.contains("ed25519") {
        return "signature".to_string();
    }
    if issue_code.contains("current") || issue_code.contains("unknown") || issue_code.contains("revoked") {
        return "currentness".to_string();
    }
    if issue_code.contains("generation") {
        return "generation".to_string();
    }
    if issue_code.contains("identity") || issue_code.contains("key") {
        return "signer-identity".to_string();
    }
    "standalone-policy".to_string()
}

fn validate_oid(value: &str, field: &str) -> Result<(), Vec<String>> {
    if value.len() != GIT_OID_HEX_CHARS || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(vec![format!("{field}:malformed-git-oid")]);
    }
    Ok(())
}

fn valid_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_CHARS && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hashed_ref(profile: &str, domain: &[u8], value: &str) -> ArtifactRef {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(value.as_bytes());
    ArtifactRef {
        profile: profile.to_string(),
        algorithm: ALGORITHM_BLAKE3.to_string(),
        digest_hex: hasher.finalize().to_hex().to_string(),
    }
}

pub fn valence_standalone_observation(
    key_identity_blake3: &str,
    verified: bool,
) -> Result<CryptographicObservation, String> {
    if !valid_blake3_hex(key_identity_blake3) {
        return Err("key_identity_blake3:malformed".to_string());
    }
    Ok(CryptographicObservation {
        algorithm: ALGORITHM_ED25519.to_string(),
        key_identity: ArtifactRef {
            profile: ED25519_PUBLIC_KEY_PROFILE_V1.to_string(),
            algorithm: ALGORITHM_BLAKE3.to_string(),
            digest_hex: key_identity_blake3.to_string(),
        },
        verified,
        failure_code: (!verified).then(|| FAILED_VERIFICATION.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY_OID: &str = "1111111111111111111111111111111111111111";
    const PARENT_OID: &str = "2222222222222222222222222222222222222222";
    const RELATED_OID: &str = "3333333333333333333333333333333333333333";
    const KEY_IDENTITY: &str = "4444444444444444444444444444444444444444444444444444444444444444";
    const OTHER_KEY_IDENTITY: &str = "5555555555555555555555555555555555555555555555555555555555555555";
    const GENERATION_ONE: u64 = 1;

    fn observed(signature_valid: bool) -> ObservedCobEvent {
        ObservedCobEvent {
            sidecar_object_id: "sidecar-object".to_string(),
            entry_oid: ENTRY_OID.to_string(),
            operation_index: 0,
            parent_oids: vec![PARENT_OID.to_string()],
            related_oids: vec![RELATED_OID.to_string()],
            signer: "z6MkradicleActorPublicKey".to_string(),
            signature_valid,
            event: None,
            decode_issue: None,
        }
    }

    fn input<'a>(
        observed: &'a ObservedCobEvent,
        cryptographic: CryptographicObservation,
    ) -> ValenceArtifactAuthObservation<'a> {
        ValenceArtifactAuthObservation {
            observed,
            related_target: RELATED_OID,
            profile_id: "valence-radicle-cob-artifact-auth-v1",
            profile_context: "org.onixresearch.valence.evidence:v1",
            generation: GENERATION_ONE,
            currentness: KeyCurrentness::Current,
            currentness_context: "reviewed-radicle-key-currentness",
            protocol_key_identity_blake3: KEY_IDENTITY,
            standalone_cryptographic: cryptographic,
        }
    }

    // r[verify valence.artifact_auth_adoption.authority]
    // r[verify valence.artifact_auth_adoption.cutover]
    #[test]
    fn protocol_verified_mapping_preserves_identity_and_authority_boundaries() {
        let event = observed(true);
        let cryptographic = valence_standalone_observation(KEY_IDENTITY, true).expect("observation");
        let report = evaluate_valence_artifact_auth(&input(&event, cryptographic));

        assert!(report.protocol_signature_valid);
        assert!(report.standalone.as_ref().is_some_and(|decision| decision.passed));
        assert!(report.compatibility.case_explained);
        assert!(report.compatibility.identity_drift_explained);
        assert!(!report.compatibility.decision_drift);
        assert!(!report.compatibility.non_claim_drift);
        assert!(report.compatibility.legacy_authoritative);
        assert!(!report.compatibility.standalone_authority_admitted);
        assert!(report.compatibility.rollback_available);
        assert_eq!(report.radicle_entry_oid, ENTRY_OID);
        assert_eq!(report.git_parent_oids, vec![PARENT_OID.to_string()]);
        assert!(report.authority_boundary.contains("actor authorization"));
    }

    // r[verify valence.artifact_auth_adoption.cutover]
    #[test]
    fn mapping_rejects_lossy_oids_related_targets_and_key_identity_drift() {
        let mut malformed = observed(true);
        malformed.entry_oid = "short-oid".to_string();
        let cryptographic = valence_standalone_observation(KEY_IDENTITY, true).expect("observation");
        let malformed_report = evaluate_valence_artifact_auth(&input(&malformed, cryptographic));
        assert!(malformed_report.standalone.is_none());
        assert!(!malformed_report.compatibility.case_explained);

        let mut unrelated = observed(true);
        unrelated.related_oids.clear();
        let cryptographic = valence_standalone_observation(KEY_IDENTITY, true).expect("observation");
        let unrelated_report = evaluate_valence_artifact_auth(&input(&unrelated, cryptographic));
        assert!(unrelated_report.standalone.is_none());

        let event = observed(true);
        let wrong_key = valence_standalone_observation(OTHER_KEY_IDENTITY, true).expect("wrong key");
        let wrong_key_report = evaluate_valence_artifact_auth(&input(&event, wrong_key));
        assert!(!wrong_key_report.compatibility.case_explained);
        assert!(wrong_key_report.compatibility.blockers.contains(&"identity-drift".to_string()));
    }

    // r[verify valence.artifact_auth_adoption.authority]
    // r[verify valence.artifact_auth_adoption.cutover]
    #[test]
    fn dual_run_rejects_false_parity_unknown_currentness_and_decode_failure_masking() {
        let rejected_event = observed(false);
        let rejected_crypto = valence_standalone_observation(KEY_IDENTITY, false).expect("rejected observation");
        let rejected = evaluate_valence_artifact_auth(&input(&rejected_event, rejected_crypto));
        assert!(rejected.compatibility.case_explained);

        let passing_crypto = valence_standalone_observation(KEY_IDENTITY, true).expect("passing observation");
        let false_parity = evaluate_valence_artifact_auth(&input(&rejected_event, passing_crypto));
        assert!(false_parity.compatibility.decision_drift);
        assert!(!false_parity.compatibility.case_explained);

        let current_event = observed(true);
        let crypto = valence_standalone_observation(KEY_IDENTITY, true).expect("unknown currentness observation");
        let mut unknown = input(&current_event, crypto);
        unknown.currentness = KeyCurrentness::Unknown;
        let unknown_report = evaluate_valence_artifact_auth(&unknown);
        assert!(unknown_report.compatibility.decision_drift);
        assert!(!unknown_report.compatibility.case_explained);

        let mut decode_failed = observed(false);
        decode_failed.decode_issue = Some("event_json_invalid".to_string());
        let crypto = valence_standalone_observation(KEY_IDENTITY, false).expect("decode observation");
        let decode_report = evaluate_valence_artifact_auth(&input(&decode_failed, crypto));
        assert!(!decode_report.compatibility.case_explained);
        assert!(decode_report.compatibility.blockers.contains(&"unrelated-protocol-and-decode-failures".to_string()));
    }
}
