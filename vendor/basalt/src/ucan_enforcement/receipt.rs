use super::core::format_digest;
use super::types::UcanEnforcementReason;
use crate::ucan_profile::BasaltUcanProtocolProfile;
use crate::ucan_semantics_identity::{UCAN_RUNTIME_REVISION, UCAN_SUPPORT_CATALOG_BLAKE3};
use serde::{Deserialize, Serialize};

pub(super) const BLAKE3_DIGEST_BYTES: usize = 32;
const RECEIPT_SCHEMA: &str = "basalt.ucan-enforcement-receipt.v2";
const RECEIPT_HASH_DOMAIN: &[u8] = b"basalt/ucan-enforcement-receipt/v2";
const RECEIPT_NON_CLAIM_COUNT: usize = 9;
const OPTION_TAG_BYTES: usize = 1;
const OPTIONAL_LENGTH_FIELD_BYTES: usize = OPTION_TAG_BYTES + size_of::<u64>();
const OPTIONAL_DIGEST_FIELD_BYTES: usize = OPTION_TAG_BYTES + BLAKE3_DIGEST_BYTES;
const OPTION_NONE_TAG: u8 = 0;
const OPTION_SOME_TAG: u8 = 1;

const _: () = assert!(usize::BITS <= u64::BITS);

/// Claim boundaries carried by each redacted adapter receipt.
pub const UCAN_ENFORCEMENT_NON_CLAIMS: [&str; RECEIPT_NON_CLAIM_COUNT] = [
    "key-discovery",
    "global-revocation-freshness",
    "replay-durability",
    "transport-security",
    "policy-truth",
    "request-canonicalizer-truth",
    "effect-correctness",
    "protocol-profile-equivalence",
    "whole-system-safety",
];

/// Deterministic redacted evidence for one adapter outcome.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UcanEnforcementReceipt {
    pub schema: String,
    pub allowed: bool,
    pub reason: UcanEnforcementReason,
    pub credential_bytes: Option<u64>,
    pub credential_blake3: Option<String>,
    pub request_binding_blake3: Option<String>,
    pub protocol_profile: String,
    pub operation_profile_blake3: String,
    pub policy_blake3: Option<String>,
    pub ucan_revision: String,
    pub ucan_support_catalog_blake3: String,
    pub non_claims: Vec<String>,
    pub receipt_blake3: String,
}

impl UcanEnforcementReceipt {
    /// Return true when this receipt accompanies live authority.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        self.allowed
    }
}

#[derive(Clone, Copy)]
pub(super) struct ReceiptContext {
    pub(super) protocol_profile: BasaltUcanProtocolProfile,
    pub(super) credential_bytes: Option<u64>,
    pub(super) credential: Option<[u8; BLAKE3_DIGEST_BYTES]>,
    pub(super) request_binding: Option<[u8; BLAKE3_DIGEST_BYTES]>,
    pub(super) operation_profile: [u8; BLAKE3_DIGEST_BYTES],
    pub(super) policy: Option<[u8; BLAKE3_DIGEST_BYTES]>,
}

pub(super) fn build_receipt(
    reason: UcanEnforcementReason,
    allowed: bool,
    context: ReceiptContext,
) -> UcanEnforcementReceipt {
    let receipt_digest = receipt_digest(reason, allowed, &context);
    UcanEnforcementReceipt {
        schema: RECEIPT_SCHEMA.to_owned(),
        allowed,
        reason,
        credential_bytes: context.credential_bytes,
        credential_blake3: context.credential.map(format_digest),
        request_binding_blake3: context.request_binding.map(format_digest),
        protocol_profile: context.protocol_profile.as_str().to_owned(),
        operation_profile_blake3: format_digest(context.operation_profile),
        policy_blake3: context.policy.map(format_digest),
        ucan_revision: UCAN_RUNTIME_REVISION.to_owned(),
        ucan_support_catalog_blake3: UCAN_SUPPORT_CATALOG_BLAKE3.to_owned(),
        non_claims: UCAN_ENFORCEMENT_NON_CLAIMS.map(str::to_owned).to_vec(),
        receipt_blake3: format_digest(receipt_digest),
    }
}

fn receipt_digest(
    reason: UcanEnforcementReason,
    allowed: bool,
    context: &ReceiptContext,
) -> [u8; BLAKE3_DIGEST_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hash_field(&mut hasher, RECEIPT_HASH_DOMAIN);
    hash_field(&mut hasher, RECEIPT_SCHEMA.as_bytes());
    hash_field(&mut hasher, &[u8::from(allowed)]);
    hash_field(&mut hasher, reason.as_str().as_bytes());
    hash_optional_length(&mut hasher, context.credential_bytes);
    hash_optional_digest(&mut hasher, context.credential.as_ref());
    hash_optional_digest(&mut hasher, context.request_binding.as_ref());
    hash_field(&mut hasher, context.protocol_profile.as_str().as_bytes());
    hash_field(&mut hasher, &context.operation_profile);
    hash_optional_digest(&mut hasher, context.policy.as_ref());
    hash_field(&mut hasher, UCAN_RUNTIME_REVISION.as_bytes());
    hash_field(&mut hasher, UCAN_SUPPORT_CATALOG_BLAKE3.as_bytes());
    hash_field(&mut hasher, &(RECEIPT_NON_CLAIM_COUNT as u64).to_be_bytes());
    for non_claim in UCAN_ENFORCEMENT_NON_CLAIMS {
        hash_field(&mut hasher, non_claim.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn hash_field(hasher: &mut blake3::Hasher, field: &[u8]) {
    let field_length = field.len() as u64;
    hasher.update(&field_length.to_be_bytes());
    hasher.update(field);
}

fn hash_optional_length(hasher: &mut blake3::Hasher, length: Option<u64>) {
    let Some(length) = length else {
        hash_field(hasher, &[OPTION_NONE_TAG]);
        return;
    };
    let mut field = [0_u8; OPTIONAL_LENGTH_FIELD_BYTES];
    field[0] = OPTION_SOME_TAG;
    field[OPTION_TAG_BYTES..].copy_from_slice(&length.to_be_bytes());
    hash_field(hasher, &field);
}

fn hash_optional_digest(hasher: &mut blake3::Hasher, digest: Option<&[u8; BLAKE3_DIGEST_BYTES]>) {
    let Some(digest) = digest else {
        hash_field(hasher, &[OPTION_NONE_TAG]);
        return;
    };
    let mut field = [0_u8; OPTIONAL_DIGEST_FIELD_BYTES];
    field[0] = OPTION_SOME_TAG;
    field[OPTION_TAG_BYTES..].copy_from_slice(digest);
    hash_field(hasher, &field);
}

#[cfg(test)]
pub(super) const fn receipt_schema() -> &'static str {
    RECEIPT_SCHEMA
}

#[cfg(test)]
mod tests {
    use super::{BLAKE3_DIGEST_BYTES, hash_field, hash_optional_digest};

    fn digest_fields(fields: &[&[u8]]) -> [u8; super::BLAKE3_DIGEST_BYTES] {
        let mut hasher = blake3::Hasher::new();
        for field in fields {
            hash_field(&mut hasher, field);
        }
        *hasher.finalize().as_bytes()
    }

    #[test]
    fn framed_fields_are_deterministic() {
        let fields: [&[u8]; 2] = [b"reason", b"revision"];

        assert_eq!(digest_fields(&fields), digest_fields(&fields));
    }

    #[test]
    fn framed_fields_reject_concatenation_ambiguity() {
        let left: [&[u8]; 2] = [b"ab", b"c"];
        let right: [&[u8]; 2] = [b"a", b"bc"];

        assert_ne!(digest_fields(&left), digest_fields(&right));
    }

    #[test]
    fn optional_digest_presence_has_a_distinct_frame() {
        let digest = [0_u8; BLAKE3_DIGEST_BYTES];
        let mut absent = blake3::Hasher::new();
        let mut present = blake3::Hasher::new();
        hash_optional_digest(&mut absent, None);
        hash_optional_digest(&mut present, Some(&digest));

        assert_ne!(absent.finalize(), present.finalize());
    }
}
