//! Deterministic runtime-effect permission policy.
//!
//! The core evaluates supplied normalized facts only. It performs no file,
//! network, process, environment, binding, approval, clock, or policy-loading effect.
// r[impl contract_backends.runtime_effect_permissions.boundary]
// r[impl contract_backends.runtime_effect_permissions.policy]
// r[impl contract_backends.runtime_effect_permissions.decision]
// r[impl contract_backends.runtime_effect_permissions.authority]
// r[impl contract_backends.runtime_effect_permissions.consumer]
// r[impl contract_backends.runtime_effect_permissions.evidence]

use crate::authority_core::AuthorityDecision;
use crate::ucan_profile::VerifiedUcanAuthorityInput;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

const POLICY_SCHEMA: &str = "basalt.runtime-effect-policy.v1";
const PROFILE_ID: &str = "basalt-runtime-effects-v1";
const AGENTOS_REPOSITORY: &str = "https://github.com/rivet-dev/agentos";
const AGENTOS_REVISION: &str = "44a6c8022f26ef0edd56e3d224f8296ee6387232";
const AGENTOS_VM_CONFIG_BLOB: &str = "e38172cfa97b21922fafcb822d254929bf1c191f";
const AGENTOS_EVALUATION_BLOB: &str = "1ab663a8a2f8bab31218f39c49193bd63d31f82e";
const AGENTOS_DEFAULT_DENY_BLOB: &str = "0fd804e703fe38d50efd353514ebea16f36452ac";
const AGENTOS_LICENSE_BLOB: &str = "f23ef9d48af638ddf215b378916767349a9591a8";
const AGENTOS_LICENSE: &str = "Apache-2.0";
const MAXIMUM_POLICY_BYTES: usize = 65_536;
const MAXIMUM_POLICY_RULES: usize = 64;
const MAXIMUM_FIELD_BYTES: usize = 4_096;
const MAXIMUM_RULE_PRIORITY: u32 = 1_000_000;
const BLAKE3_REFERENCE_PREFIX: &str = "blake3:";
const BLAKE3_HEX_BYTES: usize = 64;
const RECEIPT_SCHEMA: &str = "basalt.runtime-effect-permission-receipt.v1";
const RECEIPT_DOMAIN: &[u8] = b"basalt/runtime-effect-permission-receipt/v1";
const NON_CLAIM_COUNT: usize = 8;
const POLICY_NON_CLAIMS: [&str; NON_CLAIM_COUNT] = [
    "a permission allow does not execute an effect",
    "a permission receipt is not live authority",
    "native parsing and normalization remain consumer-owned",
    "filesystem policy does not replace Cap Root or capability-relative I/O",
    "network, process, environment, and binding implementations remain consumer-owned",
    "human approval cannot override a denial or missing authority",
    "consumer currentness and release decisions remain consumer-owned",
    "the agentOS reference is a bounded comparison, not parity or sandbox evidence",
];

/// Checked-in Nickel-exported runtime-effect policy.
pub const DEFAULT_RUNTIME_EFFECT_POLICY_JSON: &str =
    include_str!("../contracts/runtime-effect/generated/policy.json");
/// Checked-in frozen Animus and Molten mapping fixtures.
pub const RUNTIME_EFFECT_CONSUMER_MAPPINGS_JSON: &str =
    include_str!("../contracts/runtime-effect/generated/consumer-mappings.json");

/// Closed runtime-effect scope.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum EffectScope {
    Filesystem,
    Network,
    Process,
    Environment,
    Binding,
}

impl EffectScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Filesystem => "filesystem",
            Self::Network => "network",
            Self::Process => "process",
            Self::Environment => "environment",
            Self::Binding => "binding",
        }
    }

    const fn subject_prefix(self) -> &'static str {
        match self {
            Self::Filesystem => "cap://",
            Self::Network => "net://",
            Self::Process => BLAKE3_REFERENCE_PREFIX,
            Self::Environment => "env://",
            Self::Binding => "binding://",
        }
    }
}

/// Closed operation vocabulary across all scopes.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum EffectOperation {
    Read,
    Write,
    Create,
    Delete,
    Metadata,
    Connect,
    Listen,
    Resolve,
    Spawn,
    Signal,
    Invoke,
}

impl EffectOperation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Create => "create",
            Self::Delete => "delete",
            Self::Metadata => "metadata",
            Self::Connect => "connect",
            Self::Listen => "listen",
            Self::Resolve => "resolve",
            Self::Spawn => "spawn",
            Self::Signal => "signal",
            Self::Invoke => "invoke",
        }
    }

    #[must_use]
    pub const fn belongs_to(self, scope: EffectScope) -> bool {
        matches!(
            (scope, self),
            (
                EffectScope::Filesystem,
                Self::Read | Self::Write | Self::Create | Self::Delete | Self::Metadata
            ) | (
                EffectScope::Network,
                Self::Connect | Self::Listen | Self::Resolve
            ) | (EffectScope::Process, Self::Spawn | Self::Signal)
                | (EffectScope::Environment, Self::Read | Self::Write)
                | (EffectScope::Binding, Self::Invoke)
        )
    }
}

/// Exact or lexical-prefix matching over a consumer-normalized subject.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EffectMatchMode {
    Exact,
    Prefix,
}

/// Closed policy disposition.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EffectDisposition {
    Allow,
    Deny,
}

/// Consumer-owned human approval observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDisposition {
    NotRequired,
    Approved,
    Denied,
}

impl ApprovalDisposition {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not-required",
            Self::Approved => "approved",
            Self::Denied => "denied",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct EffectRule {
    rule_id: String,
    scope: EffectScope,
    operation: EffectOperation,
    match_mode: EffectMatchMode,
    subject_pattern: String,
    priority: u32,
    decision: EffectDisposition,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentOsReference {
    repository: String,
    revision: String,
    vm_config_blob: String,
    evaluation_blob: String,
    default_deny_blob: String,
    license_blob: String,
    license: String,
    claim: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyDto {
    schema: String,
    profile_id: String,
    generation: u64,
    default_decision: EffectDisposition,
    require_contract: bool,
    require_verified_ucan: bool,
    rules: Vec<EffectRule>,
    reference: AgentOsReference,
    non_claims: Vec<String>,
}

/// Validated runtime-effect policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEffectPolicy {
    generation: u64,
    default_decision: EffectDisposition,
    require_contract: bool,
    require_verified_ucan: bool,
    rules: Vec<EffectRule>,
    identity: [u8; blake3::OUT_LEN],
}

impl RuntimeEffectPolicy {
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn identity(&self) -> &[u8; blake3::OUT_LEN] {
        &self.identity
    }
}

/// Stable policy-admission issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEffectPolicyError {
    Oversized,
    Malformed,
    SchemaMismatch,
    ProfileMismatch,
    EmptyRules,
    TooManyRules,
    DuplicateRule,
    InvalidRule,
    InvalidSubject,
    UnsafeDefault,
    ReferenceMismatch,
    NonClaimsWeakened,
}

impl fmt::Display for RuntimeEffectPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Oversized => "runtime-effect policy exceeds its byte limit",
            Self::Malformed => "runtime-effect policy is malformed",
            Self::SchemaMismatch => "runtime-effect policy schema does not match",
            Self::ProfileMismatch => "runtime-effect policy profile does not match",
            Self::EmptyRules => "runtime-effect policy has no rules",
            Self::TooManyRules => "runtime-effect policy exceeds its rule limit",
            Self::DuplicateRule => "runtime-effect policy contains a duplicate rule identifier",
            Self::InvalidRule => "runtime-effect policy contains an invalid rule",
            Self::InvalidSubject => "runtime-effect policy contains an invalid normalized subject",
            Self::UnsafeDefault => "runtime-effect policy must deny by default",
            Self::ReferenceMismatch => "runtime-effect policy reference does not match",
            Self::NonClaimsWeakened => "runtime-effect policy non-claims were weakened",
        })
    }
}

impl std::error::Error for RuntimeEffectPolicyError {}

/// Parse and validate one Nickel-exported runtime-effect policy.
///
/// # Errors
///
/// Returns a stable issue for malformed, unsafe, stale, or ambiguous policy.
///
/// # Panics
///
/// Panics only if validated unique rule identifiers diverge internally.
pub fn parse_runtime_effect_policy(
    json: &str,
) -> Result<RuntimeEffectPolicy, RuntimeEffectPolicyError> {
    if json.len() > MAXIMUM_POLICY_BYTES {
        return Err(RuntimeEffectPolicyError::Oversized);
    }
    let dto: PolicyDto =
        serde_json::from_str(json).map_err(|_| RuntimeEffectPolicyError::Malformed)?;
    if dto.schema != POLICY_SCHEMA {
        return Err(RuntimeEffectPolicyError::SchemaMismatch);
    }
    if dto.profile_id != PROFILE_ID || dto.generation == 0 {
        return Err(RuntimeEffectPolicyError::ProfileMismatch);
    }
    if dto.default_decision != EffectDisposition::Deny {
        return Err(RuntimeEffectPolicyError::UnsafeDefault);
    }
    validate_reference(&dto.reference)?;
    if dto.non_claims != POLICY_NON_CLAIMS.map(str::to_owned) {
        return Err(RuntimeEffectPolicyError::NonClaimsWeakened);
    }
    if dto.rules.is_empty() {
        return Err(RuntimeEffectPolicyError::EmptyRules);
    }
    if dto.rules.len() > MAXIMUM_POLICY_RULES {
        return Err(RuntimeEffectPolicyError::TooManyRules);
    }
    let mut rule_ids = BTreeSet::new();
    for rule in &dto.rules {
        validate_rule(rule)?;
        if !rule_ids.insert(rule.rule_id.as_str()) {
            return Err(RuntimeEffectPolicyError::DuplicateRule);
        }
    }
    assert_eq!(
        rule_ids.len(),
        dto.rules.len(),
        "validated rule IDs must remain unique"
    );
    Ok(RuntimeEffectPolicy {
        generation: dto.generation,
        default_decision: dto.default_decision,
        require_contract: dto.require_contract,
        require_verified_ucan: dto.require_verified_ucan,
        rules: dto.rules,
        identity: *blake3::hash(json.as_bytes()).as_bytes(),
    })
}

fn validate_reference(reference: &AgentOsReference) -> Result<(), RuntimeEffectPolicyError> {
    if reference.repository != AGENTOS_REPOSITORY
        || reference.revision != AGENTOS_REVISION
        || reference.vm_config_blob != AGENTOS_VM_CONFIG_BLOB
        || reference.evaluation_blob != AGENTOS_EVALUATION_BLOB
        || reference.default_deny_blob != AGENTOS_DEFAULT_DENY_BLOB
        || reference.license_blob != AGENTOS_LICENSE_BLOB
        || reference.license != AGENTOS_LICENSE
        || reference.claim != "bounded-comparison-only"
    {
        return Err(RuntimeEffectPolicyError::ReferenceMismatch);
    }
    Ok(())
}

fn validate_rule(rule: &EffectRule) -> Result<(), RuntimeEffectPolicyError> {
    if !bounded_text(rule.rule_id.as_str())
        || !rule.operation.belongs_to(rule.scope)
        || rule.priority > MAXIMUM_RULE_PRIORITY
    {
        return Err(RuntimeEffectPolicyError::InvalidRule);
    }
    validate_subject(rule.scope, rule.subject_pattern.as_str())
        .map_err(|_| RuntimeEffectPolicyError::InvalidSubject)
}

/// Consumer-normalized effect request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectRequest {
    scope: EffectScope,
    operation: EffectOperation,
    subject: String,
    profile_generation: u64,
    adapter_current: bool,
    mapping_lossless: bool,
}

impl EffectRequest {
    /// Construct one validated normalized request.
    ///
    /// # Errors
    ///
    /// Returns a stable issue for a wrong scope, malformed subject, or zero generation.
    pub fn new(
        scope: EffectScope,
        operation: EffectOperation,
        subject: impl Into<String>,
        profile_generation: u64,
        adapter_current: bool,
        mapping_lossless: bool,
    ) -> Result<Self, EffectRequestError> {
        let subject = subject.into();
        if !operation.belongs_to(scope) {
            return Err(EffectRequestError::OperationScopeMismatch);
        }
        validate_subject(scope, subject.as_str())?;
        if profile_generation == 0 {
            return Err(EffectRequestError::InvalidGeneration);
        }
        Ok(Self {
            scope,
            operation,
            subject,
            profile_generation,
            adapter_current,
            mapping_lossless,
        })
    }

    #[must_use]
    pub const fn scope(&self) -> EffectScope {
        self.scope
    }

    #[must_use]
    pub const fn operation(&self) -> EffectOperation {
        self.operation
    }
}

/// Stable normalized-request error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectRequestError {
    OperationScopeMismatch,
    InvalidSubject,
    InvalidGeneration,
}

impl fmt::Display for EffectRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OperationScopeMismatch => "effect operation does not belong to its scope",
            Self::InvalidSubject => "effect subject is not normalized for its scope",
            Self::InvalidGeneration => "effect profile generation must be non-zero",
        })
    }
}

impl std::error::Error for EffectRequestError {}

/// Explicit authority and approval facts supplied to the pure core.
pub struct EffectAuthorityInput<'a> {
    pub contract_decision: Option<&'a AuthorityDecision>,
    pub verified_ucan: Option<&'a VerifiedUcanAuthorityInput>,
    pub approval: ApprovalDisposition,
}

/// Stable permission decision diagnostic.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EffectPermissionDiagnostic {
    Allowed,
    StaleProfile,
    StaleAdapter,
    LossyMapping,
    NoMatchingRule,
    AmbiguousRules,
    RuleDenied,
    ContractMissing,
    ContractDenied,
    UcanMissing,
    UcanBoundaryWeakened,
    ApprovalDenied,
}

impl EffectPermissionDiagnostic {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::StaleProfile => "stale-profile",
            Self::StaleAdapter => "stale-adapter",
            Self::LossyMapping => "lossy-mapping",
            Self::NoMatchingRule => "no-matching-rule",
            Self::AmbiguousRules => "ambiguous-rules",
            Self::RuleDenied => "rule-denied",
            Self::ContractMissing => "contract-missing",
            Self::ContractDenied => "contract-denied",
            Self::UcanMissing => "ucan-missing",
            Self::UcanBoundaryWeakened => "ucan-boundary-weakened",
            Self::ApprovalDenied => "approval-denied",
        }
    }
}

/// Metadata-only effect permission receipt.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EffectPermissionReceipt {
    schema: String,
    allowed: bool,
    diagnostic: EffectPermissionDiagnostic,
    scope: String,
    operation: String,
    request_blake3: String,
    policy_blake3: String,
    selected_rule: Option<String>,
    authority_blake3: String,
    approval: String,
    agentos_revision: String,
    non_claims: Vec<String>,
    receipt_blake3: String,
}

impl EffectPermissionReceipt {
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        self.allowed
    }

    #[must_use]
    pub const fn diagnostic(&self) -> EffectPermissionDiagnostic {
        self.diagnostic
    }
}

/// Deterministic permission result. It is evidence, not a live effect capability.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EffectPermissionDecision {
    receipt: EffectPermissionReceipt,
}

impl EffectPermissionDecision {
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        self.receipt.allowed
    }

    #[must_use]
    pub const fn receipt(&self) -> &EffectPermissionReceipt {
        &self.receipt
    }
}

/// Evaluate one normalized request without executing an effect.
#[must_use]
pub fn evaluate_effect_permission(
    policy: &RuntimeEffectPolicy,
    request: &EffectRequest,
    authority: &EffectAuthorityInput<'_>,
) -> EffectPermissionDecision {
    let (diagnostic, selected_rule) = decide(policy, request, authority);
    let allowed = diagnostic == EffectPermissionDiagnostic::Allowed;
    let receipt = build_receipt(
        policy,
        request,
        authority,
        diagnostic,
        selected_rule.as_deref(),
        allowed,
    );
    EffectPermissionDecision { receipt }
}

fn decide(
    policy: &RuntimeEffectPolicy,
    request: &EffectRequest,
    authority: &EffectAuthorityInput<'_>,
) -> (EffectPermissionDiagnostic, Option<String>) {
    if request.profile_generation != policy.generation {
        return (EffectPermissionDiagnostic::StaleProfile, None);
    }
    if !request.adapter_current {
        return (EffectPermissionDiagnostic::StaleAdapter, None);
    }
    if !request.mapping_lossless {
        return (EffectPermissionDiagnostic::LossyMapping, None);
    }
    let selection = select_rule(policy, request);
    let rule = match selection {
        RuleSelection::None => return (EffectPermissionDiagnostic::NoMatchingRule, None),
        RuleSelection::Ambiguous => return (EffectPermissionDiagnostic::AmbiguousRules, None),
        RuleSelection::Selected(rule) => rule,
    };
    if rule.decision != EffectDisposition::Allow {
        return (
            EffectPermissionDiagnostic::RuleDenied,
            Some(rule.rule_id.clone()),
        );
    }
    if policy.require_contract {
        let Some(contract) = authority.contract_decision else {
            return (
                EffectPermissionDiagnostic::ContractMissing,
                Some(rule.rule_id.clone()),
            );
        };
        if !contract.is_allowed() {
            return (
                EffectPermissionDiagnostic::ContractDenied,
                Some(rule.rule_id.clone()),
            );
        }
    }
    if policy.require_verified_ucan {
        let Some(verified) = authority.verified_ucan else {
            return (
                EffectPermissionDiagnostic::UcanMissing,
                Some(rule.rule_id.clone()),
            );
        };
        if *verified.non_claims() != crate::ucan_profile::VERIFIED_UCAN_INPUT_NON_CLAIMS {
            return (
                EffectPermissionDiagnostic::UcanBoundaryWeakened,
                Some(rule.rule_id.clone()),
            );
        }
    }
    if authority.approval == ApprovalDisposition::Denied {
        return (
            EffectPermissionDiagnostic::ApprovalDenied,
            Some(rule.rule_id.clone()),
        );
    }
    (
        EffectPermissionDiagnostic::Allowed,
        Some(rule.rule_id.clone()),
    )
}

enum RuleSelection<'a> {
    None,
    Ambiguous,
    Selected(&'a EffectRule),
}

fn select_rule<'a>(policy: &'a RuntimeEffectPolicy, request: &EffectRequest) -> RuleSelection<'a> {
    let mut matching = policy
        .rules
        .iter()
        .filter(|rule| {
            rule.scope == request.scope
                && rule.operation == request.operation
                && rule_matches(rule, request.subject.as_str())
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        debug_assert_eq!(policy.default_decision, EffectDisposition::Deny);
        return RuleSelection::None;
    }
    let maximum_priority = matching
        .iter()
        .map(|rule| rule.priority)
        .max()
        .expect("non-empty matching rules must have a maximum priority");
    matching.retain(|rule| rule.priority == maximum_priority);
    let first_decision = matching[0].decision;
    if matching.iter().any(|rule| rule.decision != first_decision) {
        return RuleSelection::Ambiguous;
    }
    matching.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
    RuleSelection::Selected(matching[0])
}

fn rule_matches(rule: &EffectRule, subject: &str) -> bool {
    match rule.match_mode {
        EffectMatchMode::Exact => subject == rule.subject_pattern,
        EffectMatchMode::Prefix => subject.starts_with(rule.subject_pattern.as_str()),
    }
}

fn validate_subject(scope: EffectScope, subject: &str) -> Result<(), EffectRequestError> {
    if !bounded_text(subject) || !subject.starts_with(scope.subject_prefix()) {
        return Err(EffectRequestError::InvalidSubject);
    }
    if scope == EffectScope::Process && !canonical_blake3_reference(subject) {
        return Err(EffectRequestError::InvalidSubject);
    }
    Ok(())
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAXIMUM_FIELD_BYTES && !value.chars().any(char::is_control)
}

fn canonical_blake3_reference(value: &str) -> bool {
    let Some(hex) = value.strip_prefix(BLAKE3_REFERENCE_PREFIX) else {
        return false;
    };
    hex.len() == BLAKE3_HEX_BYTES
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn build_receipt(
    policy: &RuntimeEffectPolicy,
    request: &EffectRequest,
    authority: &EffectAuthorityInput<'_>,
    diagnostic: EffectPermissionDiagnostic,
    selected_rule: Option<&str>,
    allowed: bool,
) -> EffectPermissionReceipt {
    let request_digest = request_digest(request);
    let authority_digest = authority_digest(authority);
    let receipt_digest = receipt_digest(
        policy,
        request,
        diagnostic,
        selected_rule,
        &request_digest,
        &authority_digest,
    );
    EffectPermissionReceipt {
        schema: RECEIPT_SCHEMA.to_owned(),
        allowed,
        diagnostic,
        scope: request.scope.as_str().to_owned(),
        operation: request.operation.as_str().to_owned(),
        request_blake3: format_blake3(request_digest),
        policy_blake3: format_blake3(policy.identity),
        selected_rule: selected_rule.map(str::to_owned),
        authority_blake3: format_blake3(authority_digest),
        approval: authority.approval.as_str().to_owned(),
        agentos_revision: AGENTOS_REVISION.to_owned(),
        non_claims: POLICY_NON_CLAIMS.map(str::to_owned).to_vec(),
        receipt_blake3: format_blake3(receipt_digest),
    }
}

fn request_digest(request: &EffectRequest) -> [u8; blake3::OUT_LEN] {
    framed_digest(
        b"basalt/runtime-effect-request/v1",
        &[
            request.scope.as_str().as_bytes(),
            request.operation.as_str().as_bytes(),
            request.subject.as_bytes(),
            &request.profile_generation.to_be_bytes(),
            &[u8::from(request.adapter_current)],
            &[u8::from(request.mapping_lossless)],
        ],
    )
}

fn authority_digest(authority: &EffectAuthorityInput<'_>) -> [u8; blake3::OUT_LEN] {
    let contract_class = authority.contract_decision.map_or("missing", |decision| {
        if decision.is_allowed() {
            "allowed"
        } else {
            "denied"
        }
    });
    let ucan_class = authority.verified_ucan.map_or("missing", |verified| {
        if verified.is_delegated() {
            "delegated"
        } else {
            "direct"
        }
    });
    framed_digest(
        b"basalt/runtime-effect-authority/v1",
        &[
            contract_class.as_bytes(),
            ucan_class.as_bytes(),
            authority.approval.as_str().as_bytes(),
        ],
    )
}

fn receipt_digest(
    policy: &RuntimeEffectPolicy,
    request: &EffectRequest,
    diagnostic: EffectPermissionDiagnostic,
    selected_rule: Option<&str>,
    request_digest: &[u8; blake3::OUT_LEN],
    authority_digest: &[u8; blake3::OUT_LEN],
) -> [u8; blake3::OUT_LEN] {
    framed_digest(
        RECEIPT_DOMAIN,
        &[
            &policy.identity,
            request_digest,
            authority_digest,
            request.scope.as_str().as_bytes(),
            request.operation.as_str().as_bytes(),
            diagnostic.as_str().as_bytes(),
            selected_rule.unwrap_or("none").as_bytes(),
            AGENTOS_REVISION.as_bytes(),
        ],
    )
}

fn framed_digest(domain: &[u8], fields: &[&[u8]]) -> [u8; blake3::OUT_LEN] {
    let mut hasher = blake3::Hasher::new();
    hash_field(&mut hasher, domain);
    for field in fields {
        hash_field(&mut hasher, field);
    }
    *hasher.finalize().as_bytes()
}

fn hash_field(hasher: &mut blake3::Hasher, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("runtime-effect identity fields must fit u64");
    hasher.update(&length.to_be_bytes());
    hasher.update(value);
}

fn format_blake3(digest: [u8; blake3::OUT_LEN]) -> String {
    format!(
        "{BLAKE3_REFERENCE_PREFIX}{}",
        blake3::Hash::from_bytes(digest).to_hex()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority_core::{AuthorityDecision, AuthorityRequest};
    use crate::ucan_profile::VerifiedUcanAuthorityInput;

    const GENERATION: u64 = 1;
    const DIGEST_BYTE: u8 = 7;
    const PROCESS_SUBJECT: &str =
        "blake3:1111111111111111111111111111111111111111111111111111111111111111";

    fn allowed_contract() -> AuthorityDecision {
        AuthorityDecision {
            schema: "basalt.authority-decision.v1".to_owned(),
            allowed: true,
            request_ref: "blake3:2222222222222222222222222222222222222222222222222222222222222222"
                .to_owned(),
            binding: Some(AuthorityRequest {
                contract_id: "runtime-effect".to_owned(),
                resource: "effect://request".to_owned(),
                ability: "effect/execute".to_owned(),
                request_ref:
                    "blake3:2222222222222222222222222222222222222222222222222222222222222222"
                        .to_owned(),
            }),
            diagnostic: None,
            supported_claim: "exact-request-only".to_owned(),
            non_claims: Vec::new(),
        }
    }

    fn verified_ucan() -> VerifiedUcanAuthorityInput {
        VerifiedUcanAuthorityInput::from_final_verified(
            "did:key:z6Mkruntime",
            [DIGEST_BYTE; blake3::OUT_LEN],
            [DIGEST_BYTE; blake3::OUT_LEN],
            [DIGEST_BYTE; blake3::OUT_LEN],
            false,
        )
    }

    fn authority<'a>(
        contract: &'a AuthorityDecision,
        ucan: &'a VerifiedUcanAuthorityInput,
    ) -> EffectAuthorityInput<'a> {
        EffectAuthorityInput {
            contract_decision: Some(contract),
            verified_ucan: Some(ucan),
            approval: ApprovalDisposition::NotRequired,
        }
    }

    #[test]
    fn all_supported_scopes_admit_exact_or_prefix_rules() {
        let policy = parse_runtime_effect_policy(DEFAULT_RUNTIME_EFFECT_POLICY_JSON)
            .expect("checked policy");
        let contract = allowed_contract();
        let ucan = verified_ucan();
        let authority = authority(&contract, &ucan);
        let cases = [
            (
                EffectScope::Filesystem,
                EffectOperation::Read,
                "cap://workspace/input",
            ),
            (
                EffectScope::Filesystem,
                EffectOperation::Write,
                "cap://workspace/output/report",
            ),
            (
                EffectScope::Network,
                EffectOperation::Connect,
                "net://api.example.invalid:443",
            ),
            (
                EffectScope::Process,
                EffectOperation::Spawn,
                PROCESS_SUBJECT,
            ),
            (
                EffectScope::Environment,
                EffectOperation::Read,
                "env://PUBLIC_SETTING",
            ),
            (
                EffectScope::Binding,
                EffectOperation::Invoke,
                "binding://molten.fabric/status",
            ),
        ];
        for (scope, operation, subject) in cases {
            let request = EffectRequest::new(scope, operation, subject, GENERATION, true, true)
                .expect("normalized request");
            let decision = evaluate_effect_permission(&policy, &request, &authority);
            assert!(decision.is_allowed(), "{} must allow", scope.as_str());
        }
    }

    #[test]
    fn missing_authority_approval_and_lossy_mapping_fail_closed() {
        let policy = parse_runtime_effect_policy(DEFAULT_RUNTIME_EFFECT_POLICY_JSON)
            .expect("checked policy");
        let request = EffectRequest::new(
            EffectScope::Filesystem,
            EffectOperation::Read,
            "cap://workspace/input",
            GENERATION,
            true,
            true,
        )
        .expect("normalized request");
        let missing = EffectAuthorityInput {
            contract_decision: None,
            verified_ucan: None,
            approval: ApprovalDisposition::Approved,
        };
        assert_eq!(
            evaluate_effect_permission(&policy, &request, &missing)
                .receipt()
                .diagnostic(),
            EffectPermissionDiagnostic::ContractMissing
        );

        let contract = allowed_contract();
        let ucan = verified_ucan();
        let denied_approval = EffectAuthorityInput {
            approval: ApprovalDisposition::Denied,
            ..authority(&contract, &ucan)
        };
        assert_eq!(
            evaluate_effect_permission(&policy, &request, &denied_approval)
                .receipt()
                .diagnostic(),
            EffectPermissionDiagnostic::ApprovalDenied
        );

        let lossy_request = EffectRequest::new(
            EffectScope::Filesystem,
            EffectOperation::Read,
            "cap://workspace/input",
            GENERATION,
            true,
            false,
        )
        .expect("structurally normalized request");
        assert_eq!(
            evaluate_effect_permission(&policy, &lossy_request, &authority(&contract, &ucan))
                .receipt()
                .diagnostic(),
            EffectPermissionDiagnostic::LossyMapping
        );
    }
}
