use ucan_nostr::AuthenticationProfile;

use super::policy_model::IndexedBinding;
use super::{
    AUTHENTICATION_RECEIPT_DOMAIN, BASALT_NOSTR_IMPLEMENTATION_REVISION, BINDING_IDENTITY_DOMAIN,
    COMPOSITION_RECEIPT_DOMAIN, DIGEST_HEX_CHARS, IMPLEMENTATION_IDENTITY_DOMAIN,
    NOSTR_AUTHENTICATION_RECEIPT_SCHEMA, NOSTR_COMPOSITION_RECEIPT_SCHEMA,
    NOSTR_IDENTITY_NON_CLAIMS, NOSTR_IDENTITY_POLICY_SCHEMA, NostrAuthenticationReceipt,
    NostrCompositionReceipt, NostrDigestRole, NostrIdentityIssue, NostrIdentityPolicy,
    NostrTypedDigest, POLICY_IDENTITY_DOMAIN, PUBLIC_FIELD_IDENTITY_DOMAIN, SCHEMA_IDENTITY_DOMAIN,
    UCAN_SOURCE_REVISION, ValidatedNostrIdentityPolicy,
};

pub(super) const fn authentication_profile_name(profile: AuthenticationProfile) -> &'static str {
    match profile {
        AuthenticationProfile::Nip42Relay => "nip42-relay",
        AuthenticationProfile::Nip98Http => "nip98-http",
        AuthenticationProfile::ApplicationV1 => "application-v1",
    }
}

/// Return the stable identity of the closed Basalt Nostr producer schemas.
#[must_use]
pub fn nostr_identity_schema_identity() -> NostrTypedDigest {
    let mut fields = vec![
        NOSTR_IDENTITY_POLICY_SCHEMA.as_bytes(),
        NOSTR_AUTHENTICATION_RECEIPT_SCHEMA.as_bytes(),
        NOSTR_COMPOSITION_RECEIPT_SCHEMA.as_bytes(),
    ];
    fields.extend(
        NOSTR_IDENTITY_NON_CLAIMS
            .iter()
            .map(|non_claim| non_claim.as_bytes()),
    );
    NostrTypedDigest::new(
        NostrDigestRole::BasaltBlake3,
        tagged_blake3(SCHEMA_IDENTITY_DOMAIN, &fields),
    )
}

/// Return the stable identity of the reviewed Basalt Nostr implementation profile.
#[must_use]
pub fn nostr_identity_implementation_identity() -> NostrTypedDigest {
    NostrTypedDigest::new(
        NostrDigestRole::BasaltBlake3,
        tagged_blake3(
            IMPLEMENTATION_IDENTITY_DOMAIN,
            &[
                BASALT_NOSTR_IMPLEMENTATION_REVISION.as_bytes(),
                UCAN_SOURCE_REVISION.as_bytes(),
            ],
        ),
    )
}

pub(super) fn policy_identity(policy: &NostrIdentityPolicy) -> NostrTypedDigest {
    let mut normalized = policy.clone();
    normalized
        .subjects
        .sort_by(|left, right| left.subject_id.cmp(&right.subject_id));
    for subject in &mut normalized.subjects {
        subject
            .authenticators
            .sort_by(|left, right| left.binding_id.cmp(&right.binding_id));
    }
    normalized.non_claims.sort();
    let bytes =
        serde_json::to_vec(&normalized).expect("crate-owned policy serialization is infallible");
    NostrTypedDigest::new(
        NostrDigestRole::BasaltBlake3,
        tagged_blake3(POLICY_IDENTITY_DOMAIN, &[bytes.as_slice()]),
    )
}

pub(super) fn binding_identity(
    policy: &ValidatedNostrIdentityPolicy,
    binding: &IndexedBinding,
) -> NostrTypedDigest {
    NostrTypedDigest::new(
        NostrDigestRole::BasaltBlake3,
        tagged_blake3(
            BINDING_IDENTITY_DOMAIN,
            &[
                policy.policy_identity.hex.as_bytes(),
                binding.subject_id.as_bytes(),
                binding.binding_id.as_bytes(),
                binding.public_key.as_bytes(),
            ],
        ),
    )
}

pub(super) fn public_field_identity(field: &str, value: &str) -> NostrTypedDigest {
    NostrTypedDigest::new(
        NostrDigestRole::BasaltBlake3,
        tagged_blake3(
            PUBLIC_FIELD_IDENTITY_DOMAIN,
            &[field.as_bytes(), value.as_bytes()],
        ),
    )
}

pub(super) fn authentication_receipt_identity(
    receipt: &NostrAuthenticationReceipt,
) -> NostrTypedDigest {
    let mut material = receipt.clone();
    material.receipt_identity.hex.clear();
    let bytes =
        serde_json::to_vec(&material).expect("crate-owned receipt serialization is infallible");
    NostrTypedDigest::new(
        NostrDigestRole::BasaltBlake3,
        tagged_blake3(AUTHENTICATION_RECEIPT_DOMAIN, &[bytes.as_slice()]),
    )
}

pub(super) fn composition_receipt_identity(receipt: &NostrCompositionReceipt) -> NostrTypedDigest {
    let mut material = receipt.clone();
    material.receipt_identity.hex.clear();
    let bytes =
        serde_json::to_vec(&material).expect("crate-owned receipt serialization is infallible");
    NostrTypedDigest::new(
        NostrDigestRole::BasaltBlake3,
        tagged_blake3(COMPOSITION_RECEIPT_DOMAIN, &[bytes.as_slice()]),
    )
}

fn tagged_blake3(domain: &str, fields: &[&[u8]]) -> String {
    let mut hasher = blake3::Hasher::new();
    update_framed(&mut hasher, domain.as_bytes());
    for field in fields {
        update_framed(&mut hasher, field);
    }
    hasher.finalize().to_hex().to_string()
}

fn update_framed(hasher: &mut blake3::Hasher, field: &[u8]) {
    let length = u64::try_from(field.len()).expect("bounded identity field length must fit u64");
    hasher.update(&length.to_be_bytes());
    hasher.update(field);
}

pub(super) fn valid_digest(digest: &NostrTypedDigest, expected_role: NostrDigestRole) -> bool {
    digest.role == expected_role && canonical_lower_hex(&digest.hex)
}

pub(super) fn check_digest(
    digest: &NostrTypedDigest,
    expected_role: NostrDigestRole,
    field: &'static str,
    issues: &mut Vec<NostrIdentityIssue>,
) {
    if !valid_digest(digest, expected_role) {
        issues.push(NostrIdentityIssue::new(field, "digest-role-mismatch"));
    }
}

fn canonical_lower_hex(value: &str) -> bool {
    value.len() == DIGEST_HEX_CHARS
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn valid_public_text(value: &str, maximum_bytes: usize) -> bool {
    if value.is_empty() || value.len() > maximum_bytes || value.trim() != value {
        return false;
    }
    let lowered = value.to_ascii_lowercase();
    let forbidden = [
        "private_key",
        "private-key",
        "secret=",
        "credential=",
        "challenge=",
        "nip46",
        "nip-46",
        "encrypted_payload",
        "encrypted-payload",
    ];
    !forbidden.iter().any(|marker| lowered.contains(marker))
}

pub(super) fn has_exact_non_claims(non_claims: &[String]) -> bool {
    non_claims.len() == NOSTR_IDENTITY_NON_CLAIMS.len()
        && NOSTR_IDENTITY_NON_CLAIMS
            .iter()
            .all(|required| non_claims.iter().any(|claim| claim == required))
}

pub(super) fn required_non_claims() -> Vec<String> {
    NOSTR_IDENTITY_NON_CLAIMS
        .iter()
        .map(|claim| (*claim).to_owned())
        .collect()
}
