use crate::ucan_enforcement::EnforcementAction;
use crate::ucan_enforcement::format_digest;
use crate::ucan_semantics_identity::{UCAN_RUNTIME_REVISION, UCAN_SUPPORT_CATALOG_BLAKE3};
use serde::{Deserialize, Serialize};

const RECEIPT_SCHEMA: &str = "basalt.ucan-v1-enforcement-receipt.v1";
const RECEIPT_DOMAIN: &[u8] = b"basalt/ucan-v1-enforcement-receipt/v1";
const FINAL_PROFILE: &str = "ucan-wg-1.0.0";
const NON_CLAIM_COUNT: usize = 10;
const OPTION_TAG_BYTES: usize = 1;
const OPTION_NONE_TAG: u8 = 0;
const OPTION_SOME_TAG: u8 = 1;

/// Claim boundaries carried by every final-profile receipt.
pub const FINAL_PROFILE_NON_CLAIMS: [&str; NON_CLAIM_COUNT] = [
    "key-trust",
    "external-resource-ownership",
    "global-revocation-freshness",
    "replay-durability",
    "transport-security",
    "policy-completeness",
    "effect-execution",
    "effect-correctness",
    "release-eligibility",
    "whole-system-safety",
];

/// Stable final-profile enforcement outcome.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FinalProfileReason {
    Allowed,
    UcanDenied,
    UcanBackendDenied,
    UnknownCommand,
    ResourceOwnerDenied,
    ResourceOwnerUnavailable,
    ResourceOwnerEvidenceMalformed,
    OperationMismatch,
    PolicyDenied,
    ReplayDenied,
    ProtocolProfileDenied,
    InternalAuthorityDenied,
}

impl FinalProfileReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::UcanDenied => "ucan-denied",
            Self::UcanBackendDenied => "ucan-backend-denied",
            Self::UnknownCommand => "unknown-command",
            Self::ResourceOwnerDenied => "resource-owner-denied",
            Self::ResourceOwnerUnavailable => "resource-owner-unavailable",
            Self::ResourceOwnerEvidenceMalformed => "resource-owner-evidence-malformed",
            Self::OperationMismatch => "operation-mismatch",
            Self::PolicyDenied => "policy-denied",
            Self::ReplayDenied => "replay-denied",
            Self::ProtocolProfileDenied => "protocol-profile-denied",
            Self::InternalAuthorityDenied => "internal-authority-denied",
        }
    }
}

/// Metadata-only evidence for one final-profile Basalt decision.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FinalProfileReceipt {
    pub schema: String,
    pub allowed: bool,
    pub reason: FinalProfileReason,
    pub protocol_profile: String,
    pub ucan_revision: String,
    pub ucan_support_catalog_blake3: String,
    pub mapping_blake3: String,
    pub policy_blake3: Option<String>,
    pub action: Option<String>,
    pub invocation_blake3: String,
    pub invocation_cid_blake3: Option<String>,
    pub command_blake3: Option<String>,
    pub subject_blake3: Option<String>,
    pub resource_owner_evidence_blake3: Option<String>,
    pub request_binding_blake3: Option<String>,
    pub operation_profile_blake3: Option<String>,
    pub non_claims: Vec<String>,
    pub receipt_blake3: String,
}

impl FinalProfileReceipt {
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        self.allowed
    }
}

#[derive(Clone, Copy)]
pub(super) struct ReceiptContext {
    pub(super) mapping: [u8; blake3::OUT_LEN],
    pub(super) policy: Option<[u8; blake3::OUT_LEN]>,
    pub(super) action: Option<EnforcementAction>,
    pub(super) invocation: [u8; blake3::OUT_LEN],
    pub(super) invocation_cid: Option<[u8; blake3::OUT_LEN]>,
    pub(super) command: Option<[u8; blake3::OUT_LEN]>,
    pub(super) subject: Option<[u8; blake3::OUT_LEN]>,
    pub(super) resource_owner_evidence: Option<[u8; blake3::OUT_LEN]>,
    pub(super) request_binding: Option<[u8; blake3::OUT_LEN]>,
    pub(super) operation_profile: Option<[u8; blake3::OUT_LEN]>,
}

pub(super) fn build_receipt(
    reason: FinalProfileReason,
    allowed: bool,
    context: &ReceiptContext,
) -> FinalProfileReceipt {
    let digest = receipt_digest(reason, allowed, context);
    FinalProfileReceipt {
        schema: RECEIPT_SCHEMA.to_owned(),
        allowed,
        reason,
        protocol_profile: FINAL_PROFILE.to_owned(),
        ucan_revision: UCAN_RUNTIME_REVISION.to_owned(),
        ucan_support_catalog_blake3: UCAN_SUPPORT_CATALOG_BLAKE3.to_owned(),
        mapping_blake3: format_digest(context.mapping),
        policy_blake3: context.policy.map(format_digest),
        action: context.action.map(|action| action.as_str().to_owned()),
        invocation_blake3: format_digest(context.invocation),
        invocation_cid_blake3: context.invocation_cid.map(format_digest),
        command_blake3: context.command.map(format_digest),
        subject_blake3: context.subject.map(format_digest),
        resource_owner_evidence_blake3: context.resource_owner_evidence.map(format_digest),
        request_binding_blake3: context.request_binding.map(format_digest),
        operation_profile_blake3: context.operation_profile.map(format_digest),
        non_claims: FINAL_PROFILE_NON_CLAIMS.map(str::to_owned).to_vec(),
        receipt_blake3: format_digest(digest),
    }
}

fn receipt_digest(
    reason: FinalProfileReason,
    allowed: bool,
    context: &ReceiptContext,
) -> [u8; blake3::OUT_LEN] {
    let mut hasher = blake3::Hasher::new();
    hash_field(&mut hasher, RECEIPT_DOMAIN);
    hash_field(&mut hasher, RECEIPT_SCHEMA.as_bytes());
    hash_field(&mut hasher, &[u8::from(allowed)]);
    hash_field(&mut hasher, reason.as_str().as_bytes());
    hash_field(&mut hasher, FINAL_PROFILE.as_bytes());
    hash_field(&mut hasher, UCAN_RUNTIME_REVISION.as_bytes());
    hash_field(&mut hasher, UCAN_SUPPORT_CATALOG_BLAKE3.as_bytes());
    hash_field(&mut hasher, &context.mapping);
    hash_optional(&mut hasher, context.policy.as_ref());
    hash_optional_text(&mut hasher, context.action.map(EnforcementAction::as_str));
    hash_field(&mut hasher, &context.invocation);
    hash_optional(&mut hasher, context.invocation_cid.as_ref());
    hash_optional(&mut hasher, context.command.as_ref());
    hash_optional(&mut hasher, context.subject.as_ref());
    hash_optional(&mut hasher, context.resource_owner_evidence.as_ref());
    hash_optional(&mut hasher, context.request_binding.as_ref());
    hash_optional(&mut hasher, context.operation_profile.as_ref());
    let non_claim_count = u64::try_from(NON_CLAIM_COUNT).expect("non-claim count must fit u64");
    hash_field(&mut hasher, &non_claim_count.to_be_bytes());
    for non_claim in FINAL_PROFILE_NON_CLAIMS {
        hash_field(&mut hasher, non_claim.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn hash_optional(hasher: &mut blake3::Hasher, value: Option<&[u8; blake3::OUT_LEN]>) {
    match value {
        Some(value) => {
            hash_field(hasher, &[OPTION_SOME_TAG]);
            hash_field(hasher, value);
        }
        None => hash_field(hasher, &[OPTION_NONE_TAG]),
    }
}

fn hash_optional_text(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hash_field(hasher, &[OPTION_SOME_TAG]);
            hash_field(hasher, value.as_bytes());
        }
        None => hash_field(hasher, &[OPTION_NONE_TAG]),
    }
}

fn hash_field(hasher: &mut blake3::Hasher, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("receipt fields must fit u64");
    hasher.update(&length.to_be_bytes());
    hasher.update(value);
}

const _: () = assert!(OPTION_TAG_BYTES == size_of::<u8>());
