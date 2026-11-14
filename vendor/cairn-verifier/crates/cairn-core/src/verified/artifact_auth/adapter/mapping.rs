use artifact_auth_core::{
    ArtifactRef, ArtifactStatement, AuthenticationPolicy, AuthenticationScope, KeyCurrentness,
    TrustedKeyObservation, ALGORITHM_BLAKE3, ALGORITHM_ED25519, ED25519_PUBLIC_KEY_PROFILE_V1,
    POLICY_SCHEMA_V1, STATEMENT_SCHEMA_V1,
};

use crate::{
    AuthenticatedEvidenceInput, AuthenticatedEvidenceIssue, AuthenticatedEvidencePolicy,
    AuthenticatedTrustedProducer, AUTHENTICATED_EVIDENCE_SCHEMA_V1,
};

use super::{
    CairnArtifactAuthInput, CAIRN_ARTIFACT_AUTH_PARENT_PROFILE_V1, CAIRN_ARTIFACT_AUTH_PROFILE_V1,
    CAIRN_ARTIFACT_AUTH_VERIFIER_CONTEXT_PROFILE_V1, CAIRN_LEGACY_KEY_GENERATION,
    CAIRN_LEGACY_KEY_THRESHOLD, ED25519_PUBLIC_KEY_BYTES,
};

const HEX_PAIR_BYTES: usize = 2;
const NIBBLE_BITS: u8 = 4;
const HEX_ALPHA_OFFSET: u8 = 10;

type ResolvedSigner<'a> = (
    &'a AuthenticatedTrustedProducer,
    [u8; ED25519_PUBLIC_KEY_BYTES],
    String,
);

/// Map Cairn's already measured lifecycle observations into the standalone contract.
///
/// This function performs no I/O, JSON parsing, receipt reconstruction, signature
/// verification, lifecycle admission, readiness composition, or presentation.
// r[impl cairn.artifact_auth.repository_extraction.consumer_authority]
// r[impl cairn.artifact_auth.repository_extraction.consumer_authority.cairn]
pub fn map_cairn_artifact_auth(
    policy: &AuthenticatedEvidencePolicy,
    input: &AuthenticatedEvidenceInput,
) -> Result<CairnArtifactAuthInput, Vec<AuthenticatedEvidenceIssue>> {
    let verifier_context = artifact_reference(
        CAIRN_ARTIFACT_AUTH_VERIFIER_CONTEXT_PROFILE_V1,
        &input.valence_integrity_receipt_hash,
    );
    let mut mapping_issues = validate_source_contract(policy, input);
    let trusted_keys = map_trusted_keys(policy, &verifier_context, &mut mapping_issues);
    let resolved = resolve_signer(policy, input, &mut mapping_issues);
    sort_adapter_issues(&mut mapping_issues);
    if !mapping_issues.is_empty() {
        return Err(mapping_issues);
    }
    let Some((trusted_signer, public_key_bytes, legacy_public_key_blake3)) = resolved else {
        return Err(vec![adapter_issue(
            "artifact_auth.adapter.signer_resolution_failed",
            "input.signature.key_id",
            "a resolved Cairn producer key",
            "resolution failed",
        )]);
    };

    let standalone_key_identity = raw_public_key_identity(&public_key_bytes);
    let standalone_public_key_blake3 = standalone_key_identity.digest_hex.clone();
    let scope = map_scope(input, verifier_context);
    let statement = ArtifactStatement {
        schema: STATEMENT_SCHEMA_V1.to_string(),
        scope: scope.clone(),
        producer_id: trusted_signer.producer.clone(),
        key_id: trusted_signer.key_id.clone(),
        key_identity: standalone_key_identity,
    };
    Ok(CairnArtifactAuthInput {
        policy: AuthenticationPolicy {
            schema: POLICY_SCHEMA_V1.to_string(),
            profile_id: CAIRN_ARTIFACT_AUTH_PROFILE_V1.to_string(),
            threshold: CAIRN_LEGACY_KEY_THRESHOLD,
            trusted_keys,
        },
        scope,
        statement,
        generation: CAIRN_LEGACY_KEY_GENERATION,
        public_key_bytes,
        legacy_public_key_blake3,
        standalone_public_key_blake3,
    })
}

fn validate_source_contract(
    policy: &AuthenticatedEvidencePolicy,
    input: &AuthenticatedEvidenceInput,
) -> Vec<AuthenticatedEvidenceIssue> {
    let mut issues = Vec::new();
    if policy.schema_version != AUTHENTICATED_EVIDENCE_SCHEMA_V1 {
        issues.push(adapter_issue(
            "artifact_auth.adapter.policy_schema_unsupported",
            "policy.schema_version",
            AUTHENTICATED_EVIDENCE_SCHEMA_V1,
            "unsupported Cairn policy schema",
        ));
    }
    if input.schema_version != AUTHENTICATED_EVIDENCE_SCHEMA_V1 {
        issues.push(adapter_issue(
            "artifact_auth.adapter.input_schema_unsupported",
            "input.schema_version",
            AUTHENTICATED_EVIDENCE_SCHEMA_V1,
            "unsupported Cairn input schema",
        ));
    }
    if input.signature.algorithm != ALGORITHM_ED25519 {
        issues.push(adapter_issue(
            "artifact_auth.adapter.signature_algorithm_unsupported",
            "input.signature.algorithm",
            ALGORITHM_ED25519,
            "unsupported signature algorithm",
        ));
    }
    issues
}

fn resolve_signer<'a>(
    policy: &'a AuthenticatedEvidencePolicy,
    input: &AuthenticatedEvidenceInput,
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) -> Option<ResolvedSigner<'a>> {
    let trusted_signer = policy.trusted_producers.iter().find(|producer| {
        producer.producer == input.signature.producer && producer.key_id == input.signature.key_id
    });
    let Some(trusted_signer) = trusted_signer else {
        issues.push(adapter_issue(
            "artifact_auth.adapter.signer_key_missing",
            "input.signature.key_id",
            "a configured Cairn producer and key label",
            "unmatched producer/key label",
        ));
        return None;
    };
    let public_key_bytes = match decode_public_key(trusted_signer, "input.signature.key_id") {
        Ok(bytes) => bytes,
        Err(issue) => {
            issues.push(issue);
            return None;
        }
    };
    let legacy_public_key_blake3 = blake3::hash(trusted_signer.public_key_hex.as_bytes())
        .to_hex()
        .to_string();
    if input.signature.public_key_blake3 != legacy_public_key_blake3 {
        issues.push(adapter_issue(
            "artifact_auth.adapter.legacy_key_identity_mismatch",
            "input.signature.public_key_blake3",
            "the BLAKE3 identity of the configured lowercase public-key text",
            "mismatched compatibility identity",
        ));
    }
    Some((trusted_signer, public_key_bytes, legacy_public_key_blake3))
}

fn map_scope(
    input: &AuthenticatedEvidenceInput,
    verifier_context: ArtifactRef,
) -> AuthenticationScope {
    AuthenticationScope {
        domain: format!("{AUTHENTICATED_EVIDENCE_SCHEMA_V1}/{}", input.evidence_kind),
        purpose: input.signature.purpose.clone(),
        profile_id: CAIRN_ARTIFACT_AUTH_PROFILE_V1.to_string(),
        subject: artifact_reference(
            &input.receipt.profile,
            &input.receipt.recomputed_receipt_hash,
        ),
        parents: input
            .required_parent_hashes
            .iter()
            .map(|digest| artifact_reference(CAIRN_ARTIFACT_AUTH_PARENT_PROFILE_V1, digest))
            .collect(),
        verifier_context,
    }
}

fn map_trusted_keys(
    policy: &AuthenticatedEvidencePolicy,
    currentness_ref: &ArtifactRef,
    issues: &mut Vec<AuthenticatedEvidenceIssue>,
) -> Vec<TrustedKeyObservation> {
    policy
        .trusted_producers
        .iter()
        .enumerate()
        .filter_map(|(index, producer)| {
            let field = format!("policy.trusted_producers[{index}]");
            let public_key_bytes = match decode_public_key(producer, &field) {
                Ok(bytes) => bytes,
                Err(issue) => {
                    issues.push(issue);
                    return None;
                }
            };
            Some(TrustedKeyObservation {
                producer_id: producer.producer.clone(),
                key_id: producer.key_id.clone(),
                key_identity: raw_public_key_identity(&public_key_bytes),
                allowed_purposes: producer.allowed_purposes.clone(),
                generation: CAIRN_LEGACY_KEY_GENERATION,
                currentness: if producer.revoked {
                    KeyCurrentness::Revoked
                } else {
                    KeyCurrentness::Current
                },
                currentness_ref: currentness_ref.clone(),
            })
        })
        .collect()
}

fn decode_public_key(
    producer: &AuthenticatedTrustedProducer,
    field: &str,
) -> Result<[u8; ED25519_PUBLIC_KEY_BYTES], AuthenticatedEvidenceIssue> {
    if producer.algorithm != ALGORITHM_ED25519 {
        return Err(adapter_issue(
            "artifact_auth.adapter.key_algorithm_unsupported",
            &format!("{field}.algorithm"),
            ALGORITHM_ED25519,
            "unsupported key algorithm",
        ));
    }
    decode_fixed_hex(&producer.public_key_hex).map_err(|()| {
        adapter_issue(
            "artifact_auth.adapter.public_key_malformed",
            &format!("{field}.public_key_hex"),
            "an Ed25519 public key as lowercase hexadecimal bytes",
            "malformed public key encoding",
        )
    })
}

fn decode_fixed_hex(value: &str) -> Result<[u8; ED25519_PUBLIC_KEY_BYTES], ()> {
    let expected_length = ED25519_PUBLIC_KEY_BYTES
        .checked_mul(HEX_PAIR_BYTES)
        .ok_or(())?;
    if value.len() != expected_length {
        return Err(());
    }
    let mut bytes = [0_u8; ED25519_PUBLIC_KEY_BYTES];
    for (index, pair) in value.as_bytes().chunks_exact(HEX_PAIR_BYTES).enumerate() {
        let high = decode_nibble(pair[0]).ok_or(())?;
        let low = decode_nibble(pair[1]).ok_or(())?;
        bytes[index] = (high << NIBBLE_BITS) | low;
    }
    Ok(bytes)
}

fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + HEX_ALPHA_OFFSET),
        _ => None,
    }
}

fn raw_public_key_identity(public_key_bytes: &[u8]) -> ArtifactRef {
    artifact_reference(
        ED25519_PUBLIC_KEY_PROFILE_V1,
        blake3::hash(public_key_bytes).to_hex().as_ref(),
    )
}

fn artifact_reference(profile: &str, digest_hex: &str) -> ArtifactRef {
    ArtifactRef {
        profile: profile.to_string(),
        algorithm: ALGORITHM_BLAKE3.to_string(),
        digest_hex: digest_hex.to_string(),
    }
}

fn adapter_issue(
    code: &str,
    field_path: &str,
    expected: &str,
    actual: &str,
) -> AuthenticatedEvidenceIssue {
    AuthenticatedEvidenceIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

fn sort_adapter_issues(issues: &mut Vec<AuthenticatedEvidenceIssue>) {
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.field_path.cmp(&right.field_path))
    });
    issues.dedup();
}
