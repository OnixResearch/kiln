//! Pure UCAN/Basalt authority admission over already-verified external facts.
//!
//! This module does not parse tokens, verify signatures, resolve keys, read clocks,
//! evaluate Nickel, fetch proofs, or consult revocation/replay stores. A caller-owned
//! shell supplies reviewed policy values and references to already-verified UCAN facts.

use crate::ucan_semantics_identity::BASALT_POLICY_BLAKE3;
use crate::{CapabilityGrant, EnforcementRequest, Policy, SUPPORTED_SCHEMA_VERSION, enforce};
use serde::{Deserialize, Serialize};

pub use crate::authority_identity::{
    AUTHORITY_DECISION_SCHEMA, AUTHORITY_FIXTURE_SCHEMA, AUTHORITY_INPUT_SCHEMA,
    AUTHORITY_RELEASE_RECEIPT_SCHEMA,
};

const BLAKE3_REFERENCE_PREFIX: &str = "blake3:";
const BLAKE3_HEX_BYTES: usize = 64;
const BLAKE3_REFERENCE_BYTES: usize = BLAKE3_REFERENCE_PREFIX.len() + BLAKE3_HEX_BYTES;
const MAXIMUM_AUTHORITY_FIELD_BYTES: usize = 4_096;
const MAXIMUM_AUTHORITY_GRANTS: usize = 64;
const MAXIMUM_VERIFICATION_RECEIPTS: usize = 64;
const MAXIMUM_RECEIPT_GRANT_REFS: usize = 64;
const SUPPORTED_CLAIM: &str = "exact-request-only";
const DENIED_CLAIM: &str = "no-authority-admitted";
const REQUIRED_AUTHORITY_NON_CLAIM_COUNT: usize = 8;

/// Claim boundaries carried by every authority decision.
pub const REQUIRED_AUTHORITY_NON_CLAIMS: [&str; REQUIRED_AUTHORITY_NON_CLAIM_COUNT] = [
    "transport-authority",
    "storage-authority",
    "persistence-authority",
    "replay-store-truth",
    "key-resolution-trust",
    "downstream-business-authority",
    "release-eligibility",
    "whole-system-safety",
];

/// Provenance class for a caller-supplied verified grant reference.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VerifiedGrantKind {
    /// Grant was verified directly on the invoked token.
    Direct,
    /// Grant was derived from an admitted delegation chain.
    Delegated,
}

/// Caller-supplied status of a UCAN verification receipt reference.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationRefDisposition {
    /// The caller asserts that the receipt is current for this invocation.
    Current,
    /// The caller asserts that the receipt is stale.
    Stale,
    /// UCAN verification rejected the referenced facts.
    Rejected,
}

/// Caller-supplied caveat decision.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityCaveatDisposition {
    /// Every required caveat was evaluated and satisfied externally.
    Satisfied,
    /// At least one caveat denied the request.
    Denied,
    /// The required caveat policy was unavailable.
    MissingPolicy,
}

/// Caller-supplied replay admission decision.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityReplayDisposition {
    /// The caller-owned replay boundary admitted this invocation.
    Admitted,
    /// The caller-owned replay boundary denied this invocation.
    Denied,
    /// The replay backend failed or was unavailable.
    BackendError,
}

/// Authority claim requested from this bounded core.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityClaim {
    /// Admit only the exact contract/resource/ability/request binding.
    ExactRequest,
    /// Treat possession of a raw token as current authority.
    TokenPossession,
    /// Treat possession of a content reference as current authority.
    ContentReference,
    /// Claim transport authority.
    Transport,
    /// Claim storage authority.
    Storage,
    /// Claim persistence authority.
    Persistence,
    /// Claim replay-store truth.
    ReplayStoreTruth,
    /// Claim key-resolution trust.
    KeyResolutionTrust,
    /// Claim downstream business authority.
    DownstreamBusinessAuthority,
    /// Claim release eligibility.
    ReleaseEligibility,
    /// Claim whole-system safety.
    WholeSystemSafety,
}

/// Stable first-failure diagnostic emitted by the authority core.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityDenyDiagnostic {
    /// Input schema does not match this core.
    SchemaMismatch,
    /// Request contract/resource/ability text is empty, oversized, or invalid.
    MalformedRequest,
    /// Request reference is not a lowercase BLAKE3 content reference.
    MalformedRequestRef,
    /// Required bounded collections exceed their ceilings.
    LimitExceeded,
    /// Required non-claims were removed, reordered, or changed.
    NonClaimsWeakened,
    /// No Basalt policy reference was supplied.
    MissingPolicyRef,
    /// More than one Basalt policy reference was supplied.
    MultiplePolicyRefs,
    /// A policy reference is malformed.
    MalformedPolicyRef,
    /// Supplied and trusted policy references differ.
    PolicyRefMismatch,
    /// Policy reference contract/schema binding differs from the request or policy.
    PolicyContractMismatch,
    /// The caller requested authority broader than the exact request.
    AuthorityOverclaim,
    /// Caveat evaluation denied or was unavailable.
    CaveatDenied,
    /// Replay admission denied or was unavailable.
    ReplayDenied,
    /// No verified grant reference was supplied.
    MissingVerifiedGrant,
    /// A verified grant or its references are malformed.
    MalformedVerifiedGrant,
    /// Verified grants do not cover the exact requested resource.
    WrongResource,
    /// Verified grants do not cover the exact requested ability.
    WrongAbility,
    /// No verification receipt reference matches the selected grant.
    MissingVerificationReceipt,
    /// A verification receipt reference is malformed.
    MalformedVerificationReceipt,
    /// The receipt does not bind the selected verified grant reference.
    VerificationGrantMismatch,
    /// The receipt request reference differs from the authority request reference.
    VerificationRequestMismatch,
    /// The matching verification receipt is stale.
    StaleVerificationRef,
    /// The matching verification receipt records rejection.
    RejectedVerificationRef,
    /// Basalt's loaded policy denies the requested contract/resource/ability.
    PolicyDenied,
    /// The selected verified grant does not authorize the request.
    UcanCapabilityDenied,
    /// Existing Basalt enforcement returned an unexpected error and failed closed.
    EnforcementError,
}

impl AuthorityDenyDiagnostic {
    /// Return the stable serialized diagnostic identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SchemaMismatch => "schema-mismatch",
            Self::MalformedRequest => "malformed-request",
            Self::MalformedRequestRef => "malformed-request-ref",
            Self::LimitExceeded => "limit-exceeded",
            Self::NonClaimsWeakened => "non-claims-weakened",
            Self::MissingPolicyRef => "missing-policy-ref",
            Self::MultiplePolicyRefs => "multiple-policy-refs",
            Self::MalformedPolicyRef => "malformed-policy-ref",
            Self::PolicyRefMismatch => "policy-ref-mismatch",
            Self::PolicyContractMismatch => "policy-contract-mismatch",
            Self::AuthorityOverclaim => "authority-overclaim",
            Self::CaveatDenied => "caveat-denied",
            Self::ReplayDenied => "replay-denied",
            Self::MissingVerifiedGrant => "missing-verified-grant",
            Self::MalformedVerifiedGrant => "malformed-verified-grant",
            Self::WrongResource => "wrong-resource",
            Self::WrongAbility => "wrong-ability",
            Self::MissingVerificationReceipt => "missing-verification-receipt",
            Self::MalformedVerificationReceipt => "malformed-verification-receipt",
            Self::VerificationGrantMismatch => "verification-grant-mismatch",
            Self::VerificationRequestMismatch => "verification-request-mismatch",
            Self::StaleVerificationRef => "stale-verification-ref",
            Self::RejectedVerificationRef => "rejected-verification-ref",
            Self::PolicyDenied => "policy-denied",
            Self::UcanCapabilityDenied => "ucan-capability-denied",
            Self::EnforcementError => "enforcement-error",
        }
    }
}

/// Exact authority request facts.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorityRequest {
    /// Basalt contract identifier.
    pub contract_id: String,
    /// Exact requested resource.
    pub resource: String,
    /// Exact requested ability.
    pub ability: String,
    /// Caller-supplied content reference for this request.
    pub request_ref: String,
}

/// Reference to one verified UCAN grant and its verification receipt.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerifiedGrantRef {
    /// Content reference for the verified grant fact.
    pub grant_ref: String,
    /// Content reference for the UCAN verification receipt that admitted it.
    pub verification_receipt_ref: String,
    /// Verified resource fact.
    pub resource: String,
    /// Verified ability fact.
    pub ability: String,
    /// Direct or delegated provenance class.
    pub kind: VerifiedGrantKind,
}

/// Reference to an externally produced UCAN verification receipt.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UcanVerificationReceiptRef {
    /// Receipt content reference.
    pub receipt_ref: String,
    /// Exact request reference verified by the receipt.
    pub request_ref: String,
    /// Verified grant references admitted by the receipt.
    pub verified_grant_refs: Vec<String>,
    /// Caller-supplied freshness/admission disposition.
    pub disposition: VerificationRefDisposition,
}

/// Reference to a shell-loaded Basalt policy artifact and contract.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BasaltPolicyRef {
    /// Policy artifact content reference.
    pub artifact_ref: String,
    /// Parsed policy schema version.
    pub schema_version: String,
    /// Contract selected from the parsed policy.
    pub contract_id: String,
}

impl BasaltPolicyRef {
    /// Construct a policy reference from shell-supplied identity fields.
    #[must_use]
    pub fn new(
        artifact_ref: impl Into<String>,
        schema_version: impl Into<String>,
        contract_id: impl Into<String>,
    ) -> Self {
        Self {
            artifact_ref: artifact_ref.into(),
            schema_version: schema_version.into(),
            contract_id: contract_id.into(),
        }
    }
}

/// Pure authority input over loaded policy and externally verified facts.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorityInput {
    /// Authority input schema.
    pub schema: String,
    /// Exact request facts.
    pub request: AuthorityRequest,
    /// Verified direct/delegated grant references.
    pub verified_grants: Vec<VerifiedGrantRef>,
    /// UCAN verification receipt references.
    pub verification_receipts: Vec<UcanVerificationReceiptRef>,
    /// Exactly one shell-trusted Basalt policy reference is required.
    pub policy_refs: Vec<BasaltPolicyRef>,
    /// Caller-supplied caveat outcome.
    pub caveat: AuthorityCaveatDisposition,
    /// Caller-supplied replay outcome.
    pub replay: AuthorityReplayDisposition,
    /// Authority claim; only one exact-request claim is admitted.
    pub claims: Vec<AuthorityClaim>,
    /// Explicit bounded non-claims.
    pub non_claims: Vec<String>,
}

/// Stable pure authority decision.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorityDecision {
    /// Authority decision schema.
    pub schema: String,
    /// Whether the exact request was admitted.
    pub allowed: bool,
    /// Request reference evaluated by the core.
    pub request_ref: String,
    /// Exact request binding when allowed; absent on denial.
    pub binding: Option<AuthorityRequest>,
    /// Stable first-failure diagnostic; absent on allow.
    pub diagnostic: Option<AuthorityDenyDiagnostic>,
    /// Bounded supported claim.
    pub supported_claim: String,
    /// Explicit non-claims preserved on allow and denial.
    pub non_claims: Vec<String>,
}

impl AuthorityDecision {
    /// Return true when the exact request was admitted.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        self.allowed
    }

    /// Return true when the request was denied.
    #[must_use]
    pub const fn is_denied(&self) -> bool {
        !self.allowed
    }
}

/// Reference-only binding paired with an existing capability field.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerifiedGrantBinding {
    /// Content reference for the verified grant.
    pub grant_ref: String,
    /// Content reference for its verification receipt.
    pub verification_receipt_ref: String,
    /// Direct or delegated provenance class.
    pub kind: VerifiedGrantKind,
}

/// Explicit evidence needed to adapt an existing [`EnforcementRequest`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorityEvidence {
    /// Request content reference.
    pub request_ref: String,
    /// One explicit verified binding per existing capability field.
    pub grant_bindings: Vec<VerifiedGrantBinding>,
    /// UCAN verification receipt references.
    pub verification_receipts: Vec<UcanVerificationReceiptRef>,
    /// Shell-trusted policy references.
    pub policy_refs: Vec<BasaltPolicyRef>,
    /// Caller-supplied caveat outcome.
    pub caveat: AuthorityCaveatDisposition,
    /// Caller-supplied replay outcome.
    pub replay: AuthorityReplayDisposition,
    /// Requested authority claims.
    pub claims: Vec<AuthorityClaim>,
    /// Required non-claims.
    pub non_claims: Vec<String>,
}

/// Deterministic compatibility-adapter error.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityAdapterError {
    /// Existing capabilities and verified reference bindings differ in count.
    GrantBindingCountMismatch,
}

/// Return the exact policy reference for Basalt's embedded generated policy.
#[must_use]
pub fn default_authority_policy_ref(contract_id: &str) -> BasaltPolicyRef {
    BasaltPolicyRef::new(
        format!("{BLAKE3_REFERENCE_PREFIX}{BASALT_POLICY_BLAKE3}"),
        SUPPORTED_SCHEMA_VERSION,
        contract_id,
    )
}

/// Return whether a string is a canonical lowercase BLAKE3 content reference.
#[must_use]
pub fn is_blake3_reference(reference: &str) -> bool {
    if reference.len() != BLAKE3_REFERENCE_BYTES {
        return false;
    }
    let Some(hex) = reference.strip_prefix(BLAKE3_REFERENCE_PREFIX) else {
        return false;
    };
    hex.bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Adapt an existing Basalt request without changing its public fields.
///
/// Existing capability values become verified grant facts only when the caller
/// supplies one explicit grant/receipt binding for every capability.
///
/// # Errors
///
/// Returns [`AuthorityAdapterError::GrantBindingCountMismatch`] when capability
/// and binding counts differ.
pub fn adapt_enforcement_request(
    request: &EnforcementRequest,
    evidence: AuthorityEvidence,
) -> Result<AuthorityInput, AuthorityAdapterError> {
    if request.capabilities.len() != evidence.grant_bindings.len() {
        return Err(AuthorityAdapterError::GrantBindingCountMismatch);
    }

    let mut verified_grants = Vec::with_capacity(request.capabilities.len());
    for (capability, binding) in request
        .capabilities
        .iter()
        .zip(evidence.grant_bindings.iter())
    {
        verified_grants.push(VerifiedGrantRef {
            grant_ref: binding.grant_ref.clone(),
            verification_receipt_ref: binding.verification_receipt_ref.clone(),
            resource: capability.resource.clone(),
            ability: capability.ability.clone(),
            kind: binding.kind,
        });
    }

    Ok(AuthorityInput {
        schema: AUTHORITY_INPUT_SCHEMA.to_owned(),
        request: AuthorityRequest {
            contract_id: request.contract.clone(),
            resource: request.resource.clone(),
            ability: request.ability.clone(),
            request_ref: evidence.request_ref,
        },
        verified_grants,
        verification_receipts: evidence.verification_receipts,
        policy_refs: evidence.policy_refs,
        caveat: evidence.caveat,
        replay: evidence.replay,
        claims: evidence.claims,
        non_claims: evidence.non_claims,
    })
}

/// Evaluate exact UCAN/Basalt authority over in-memory verified facts and policy values.
///
/// `trusted_policy_ref` is supplied by the shell that loaded `policy`; the input's
/// policy reference must match it exactly.
#[must_use]
pub fn evaluate_authority(
    policy: &Policy,
    trusted_policy_ref: &BasaltPolicyRef,
    input: &AuthorityInput,
) -> AuthorityDecision {
    if let Some(diagnostic) = validate_authority_input(policy, trusted_policy_ref, input) {
        return denied(input, diagnostic);
    }
    if let Some(diagnostic) = validate_claims_and_dispositions(input) {
        return denied(input, diagnostic);
    }

    let grant = match select_exact_grant(input) {
        Ok(grant) => grant,
        Err(diagnostic) => return denied(input, diagnostic),
    };
    let receipt = match select_verification_receipt(input, grant) {
        Ok(receipt) => receipt,
        Err(diagnostic) => return denied(input, diagnostic),
    };
    if let Some(diagnostic) = validate_receipt_binding(input, grant, receipt) {
        return denied(input, diagnostic);
    }

    enforce_exact_grant(policy, input, grant)
}

fn validate_authority_input(
    policy: &Policy,
    trusted_policy_ref: &BasaltPolicyRef,
    input: &AuthorityInput,
) -> Option<AuthorityDenyDiagnostic> {
    if input.schema != AUTHORITY_INPUT_SCHEMA {
        return Some(AuthorityDenyDiagnostic::SchemaMismatch);
    }
    if collections_exceed_limits(input) {
        return Some(AuthorityDenyDiagnostic::LimitExceeded);
    }
    if !request_text_is_valid(&input.request) {
        return Some(AuthorityDenyDiagnostic::MalformedRequest);
    }
    if !is_blake3_reference(input.request.request_ref.as_str()) {
        return Some(AuthorityDenyDiagnostic::MalformedRequestRef);
    }
    if input.non_claims != REQUIRED_AUTHORITY_NON_CLAIMS.map(str::to_owned) {
        return Some(AuthorityDenyDiagnostic::NonClaimsWeakened);
    }
    validate_policy_binding(policy, trusted_policy_ref, input)
}

fn validate_policy_binding(
    policy: &Policy,
    trusted_policy_ref: &BasaltPolicyRef,
    input: &AuthorityInput,
) -> Option<AuthorityDenyDiagnostic> {
    if input.policy_refs.is_empty() {
        return Some(AuthorityDenyDiagnostic::MissingPolicyRef);
    }
    if input.policy_refs.len() != 1 {
        return Some(AuthorityDenyDiagnostic::MultiplePolicyRefs);
    }
    let input_policy_ref = &input.policy_refs[0];
    if !is_blake3_reference(input_policy_ref.artifact_ref.as_str())
        || !is_blake3_reference(trusted_policy_ref.artifact_ref.as_str())
    {
        return Some(AuthorityDenyDiagnostic::MalformedPolicyRef);
    }
    if input_policy_ref != trusted_policy_ref {
        return Some(AuthorityDenyDiagnostic::PolicyRefMismatch);
    }
    if trusted_policy_ref.schema_version != policy.schema_version
        || trusted_policy_ref.contract_id != input.request.contract_id
    {
        return Some(AuthorityDenyDiagnostic::PolicyContractMismatch);
    }
    let Some(contract) = policy.contract(input.request.contract_id.as_str()) else {
        return Some(AuthorityDenyDiagnostic::PolicyContractMismatch);
    };
    if contract.id != input.request.contract_id {
        return Some(AuthorityDenyDiagnostic::PolicyContractMismatch);
    }
    None
}

fn validate_claims_and_dispositions(input: &AuthorityInput) -> Option<AuthorityDenyDiagnostic> {
    if input.claims.as_slice() != [AuthorityClaim::ExactRequest] {
        return Some(AuthorityDenyDiagnostic::AuthorityOverclaim);
    }
    if input.caveat != AuthorityCaveatDisposition::Satisfied {
        return Some(AuthorityDenyDiagnostic::CaveatDenied);
    }
    if input.replay != AuthorityReplayDisposition::Admitted {
        return Some(AuthorityDenyDiagnostic::ReplayDenied);
    }
    None
}

fn select_exact_grant(
    input: &AuthorityInput,
) -> Result<&VerifiedGrantRef, AuthorityDenyDiagnostic> {
    if input.verified_grants.is_empty() {
        return Err(AuthorityDenyDiagnostic::MissingVerifiedGrant);
    }
    for grant in &input.verified_grants {
        if !verified_grant_is_well_formed(grant) {
            return Err(AuthorityDenyDiagnostic::MalformedVerifiedGrant);
        }
    }
    if let Some(grant) = input.verified_grants.iter().find(|grant| {
        grant.resource == input.request.resource && grant.ability == input.request.ability
    }) {
        return Ok(grant);
    }
    if input
        .verified_grants
        .iter()
        .any(|grant| grant.resource == input.request.resource)
    {
        return Err(AuthorityDenyDiagnostic::WrongAbility);
    }
    Err(AuthorityDenyDiagnostic::WrongResource)
}

fn select_verification_receipt<'a>(
    input: &'a AuthorityInput,
    grant: &VerifiedGrantRef,
) -> Result<&'a UcanVerificationReceiptRef, AuthorityDenyDiagnostic> {
    for receipt in &input.verification_receipts {
        if !verification_receipt_is_well_formed(receipt) {
            return Err(AuthorityDenyDiagnostic::MalformedVerificationReceipt);
        }
    }
    input
        .verification_receipts
        .iter()
        .find(|receipt| receipt.receipt_ref == grant.verification_receipt_ref)
        .ok_or(AuthorityDenyDiagnostic::MissingVerificationReceipt)
}

fn validate_receipt_binding(
    input: &AuthorityInput,
    grant: &VerifiedGrantRef,
    receipt: &UcanVerificationReceiptRef,
) -> Option<AuthorityDenyDiagnostic> {
    match receipt.disposition {
        VerificationRefDisposition::Stale => {
            return Some(AuthorityDenyDiagnostic::StaleVerificationRef);
        }
        VerificationRefDisposition::Rejected => {
            return Some(AuthorityDenyDiagnostic::RejectedVerificationRef);
        }
        VerificationRefDisposition::Current => {}
    }
    if receipt.request_ref != input.request.request_ref {
        return Some(AuthorityDenyDiagnostic::VerificationRequestMismatch);
    }
    if !receipt
        .verified_grant_refs
        .iter()
        .any(|grant_ref| grant_ref == &grant.grant_ref)
    {
        return Some(AuthorityDenyDiagnostic::VerificationGrantMismatch);
    }
    None
}

// Explicit branching keeps the unexpected enforcement error visible.
#[allow(clippy::option_if_let_else)]
fn enforce_exact_grant(
    policy: &Policy,
    input: &AuthorityInput,
    grant: &VerifiedGrantRef,
) -> AuthorityDecision {
    let request = EnforcementRequest::new(
        input.request.contract_id.clone(),
        input.request.resource.clone(),
        input.request.ability.clone(),
    )
    .with_capability(CapabilityGrant::new(
        grant.resource.clone(),
        grant.ability.clone(),
    ));

    match enforce(policy, &request) {
        Ok(receipt) if receipt.is_allowed() => allowed(input),
        Ok(receipt) if receipt.reason().contains("ucan capability set") => {
            denied(input, AuthorityDenyDiagnostic::UcanCapabilityDenied)
        }
        Ok(_) => denied(input, AuthorityDenyDiagnostic::PolicyDenied),
        Err(_) => denied(input, AuthorityDenyDiagnostic::EnforcementError),
    }
}

fn collections_exceed_limits(input: &AuthorityInput) -> bool {
    if input.verified_grants.len() > MAXIMUM_AUTHORITY_GRANTS
        || input.verification_receipts.len() > MAXIMUM_VERIFICATION_RECEIPTS
    {
        return true;
    }
    input
        .verification_receipts
        .iter()
        .any(|receipt| receipt.verified_grant_refs.len() > MAXIMUM_RECEIPT_GRANT_REFS)
}

fn request_text_is_valid(request: &AuthorityRequest) -> bool {
    if !bounded_non_empty(request.contract_id.as_str())
        || !bounded_non_empty(request.resource.as_str())
        || !bounded_non_empty(request.ability.as_str())
    {
        return false;
    }
    ucan::VerifiedCapability::new(request.resource.clone(), request.ability.clone()).is_ok()
}

fn verified_grant_is_well_formed(grant: &VerifiedGrantRef) -> bool {
    is_blake3_reference(grant.grant_ref.as_str())
        && is_blake3_reference(grant.verification_receipt_ref.as_str())
        && bounded_non_empty(grant.resource.as_str())
        && bounded_non_empty(grant.ability.as_str())
        && ucan::VerifiedCapability::new(grant.resource.clone(), grant.ability.clone()).is_ok()
}

fn verification_receipt_is_well_formed(receipt: &UcanVerificationReceiptRef) -> bool {
    if !is_blake3_reference(receipt.receipt_ref.as_str())
        || !is_blake3_reference(receipt.request_ref.as_str())
        || receipt.verified_grant_refs.is_empty()
    {
        return false;
    }
    receipt
        .verified_grant_refs
        .iter()
        .all(|grant_ref| is_blake3_reference(grant_ref.as_str()))
}

fn bounded_non_empty(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_AUTHORITY_FIELD_BYTES
        && !value.chars().any(char::is_control)
}

fn allowed(input: &AuthorityInput) -> AuthorityDecision {
    AuthorityDecision {
        schema: AUTHORITY_DECISION_SCHEMA.to_owned(),
        allowed: true,
        request_ref: input.request.request_ref.clone(),
        binding: Some(input.request.clone()),
        diagnostic: None,
        supported_claim: SUPPORTED_CLAIM.to_owned(),
        non_claims: REQUIRED_AUTHORITY_NON_CLAIMS.map(str::to_owned).to_vec(),
    }
}

fn denied(input: &AuthorityInput, diagnostic: AuthorityDenyDiagnostic) -> AuthorityDecision {
    AuthorityDecision {
        schema: AUTHORITY_DECISION_SCHEMA.to_owned(),
        allowed: false,
        request_ref: input.request.request_ref.clone(),
        binding: None,
        diagnostic: Some(diagnostic),
        supported_claim: DENIED_CLAIM.to_owned(),
        non_claims: REQUIRED_AUTHORITY_NON_CLAIMS.map(str::to_owned).to_vec(),
    }
}
