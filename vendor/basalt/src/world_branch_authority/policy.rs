use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;

use super::{
    BranchMode, CapabilityClass, MAXIMUM_POLICY_BYTES, MAXIMUM_POLICY_RULES, POLICY_NON_CLAIMS,
    POLICY_PROFILE, POLICY_SCHEMA, valid_normalized_text,
};

const MAXIMUM_RULE_PRIORITY: u32 = 1_000_000;
const POLICY_IDENTITY_CONTEXT: &str =
    "onixresearch.basalt.world-branch-authority.policy.identity.v1";

pub const DEFAULT_WORLD_BRANCH_AUTHORITY_POLICY_JSON: &str =
    include_str!("../../contracts/world-branch-authority/generated/policy.json");
pub const WORLD_BRANCH_AUTHORITY_CONSUMER_MAPPINGS_JSON: &str =
    include_str!("../../contracts/world-branch-authority/generated/consumer-mappings.json");

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct PolicyRule {
    pub rule_id: String,
    pub capability_class: CapabilityClass,
    pub mode: BranchMode,
    pub priority: u32,
    pub require_current: bool,
    pub require_ucan: bool,
    pub required_limits: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyDto {
    schema: String,
    profile_id: String,
    generation: u64,
    default_mode: BranchMode,
    rules: Vec<PolicyRule>,
    non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchAuthorityPolicy {
    generation: u64,
    rules: Vec<PolicyRule>,
    identity: String,
}

impl BranchAuthorityPolicy {
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub(super) fn matching_rules(&self, capability_class: CapabilityClass) -> Vec<&PolicyRule> {
        self.rules
            .iter()
            .filter(|rule| rule.capability_class == capability_class)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchAuthorityPolicyError {
    Oversized,
    Malformed,
    SchemaMismatch,
    ProfileMismatch,
    UnsafeDefault,
    EmptyRules,
    TooManyRules,
    DuplicateRule,
    MissingCapabilityClass,
    InvalidRule,
    NonClaimsWeakened,
}

impl fmt::Display for BranchAuthorityPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Oversized => "world-branch authority policy exceeds its byte limit",
            Self::Malformed => "world-branch authority policy is malformed",
            Self::SchemaMismatch => "world-branch authority policy schema does not match",
            Self::ProfileMismatch => "world-branch authority policy profile does not match",
            Self::UnsafeDefault => "world-branch authority policy must deny by default",
            Self::EmptyRules => "world-branch authority policy has no rules",
            Self::TooManyRules => "world-branch authority policy exceeds its rule bound",
            Self::DuplicateRule => {
                "world-branch authority policy contains a duplicate rule identifier"
            }
            Self::MissingCapabilityClass => {
                "world-branch authority policy omits a capability class"
            }
            Self::InvalidRule => "world-branch authority policy contains an invalid rule",
            Self::NonClaimsWeakened => "world-branch authority policy non-claims were weakened",
        })
    }
}

impl std::error::Error for BranchAuthorityPolicyError {}

/// Parse and validate one Nickel-exported world-branch authority policy.
///
/// # Errors
///
/// Returns a stable error for malformed, stale, unsafe, incomplete, duplicate, or weakened policy input.
pub fn parse_branch_authority_policy(
    json: &str,
) -> Result<BranchAuthorityPolicy, BranchAuthorityPolicyError> {
    if json.len() > MAXIMUM_POLICY_BYTES {
        return Err(BranchAuthorityPolicyError::Oversized);
    }
    let dto: PolicyDto =
        serde_json::from_str(json).map_err(|_| BranchAuthorityPolicyError::Malformed)?;
    if dto.schema != POLICY_SCHEMA {
        return Err(BranchAuthorityPolicyError::SchemaMismatch);
    }
    if dto.profile_id != POLICY_PROFILE || dto.generation == 0 {
        return Err(BranchAuthorityPolicyError::ProfileMismatch);
    }
    if dto.default_mode != BranchMode::NonBranchable {
        return Err(BranchAuthorityPolicyError::UnsafeDefault);
    }
    if dto.non_claims
        != POLICY_NON_CLAIMS
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    {
        return Err(BranchAuthorityPolicyError::NonClaimsWeakened);
    }
    if dto.rules.is_empty() {
        return Err(BranchAuthorityPolicyError::EmptyRules);
    }
    if dto.rules.len() > MAXIMUM_POLICY_RULES {
        return Err(BranchAuthorityPolicyError::TooManyRules);
    }
    let mut rule_ids = BTreeSet::new();
    let mut classes = BTreeSet::new();
    for rule in &dto.rules {
        validate_rule(rule)?;
        if !rule_ids.insert(rule.rule_id.as_str()) {
            return Err(BranchAuthorityPolicyError::DuplicateRule);
        }
        classes.insert(rule.capability_class);
    }
    if CapabilityClass::ALL
        .iter()
        .any(|capability_class| !classes.contains(capability_class))
    {
        return Err(BranchAuthorityPolicyError::MissingCapabilityClass);
    }
    let mut hasher = blake3::Hasher::new_derive_key(POLICY_IDENTITY_CONTEXT);
    hasher.update(json.as_bytes());
    Ok(BranchAuthorityPolicy {
        generation: dto.generation,
        rules: dto.rules,
        identity: format!("blake3:{}", hasher.finalize().to_hex()),
    })
}

fn validate_rule(rule: &PolicyRule) -> Result<(), BranchAuthorityPolicyError> {
    if !valid_normalized_text(&rule.rule_id)
        || rule.priority == 0
        || rule.priority > MAXIMUM_RULE_PRIORITY
        || !rule.require_current
        || rule.mode != expected_mode(rule.capability_class)
        || rule
            .required_limits
            .iter()
            .any(|limit| !valid_normalized_text(limit))
    {
        return Err(BranchAuthorityPolicyError::InvalidRule);
    }
    let expected_limits = expected_limits(rule.mode);
    let observed_limits = rule
        .required_limits
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if observed_limits.as_slice() != expected_limits {
        return Err(BranchAuthorityPolicyError::InvalidRule);
    }
    let expected_ucan = matches!(
        rule.mode,
        BranchMode::Copyable
            | BranchMode::Attenuated
            | BranchMode::Linear
            | BranchMode::PromotionGated
    );
    if rule.require_ucan != expected_ucan {
        return Err(BranchAuthorityPolicyError::InvalidRule);
    }
    Ok(())
}

const fn expected_mode(capability_class: CapabilityClass) -> BranchMode {
    match capability_class {
        CapabilityClass::PublicArtifact => BranchMode::Copyable,
        CapabilityClass::ScopedService => BranchMode::Attenuated,
        CapabilityClass::ExclusiveLease => BranchMode::Linear,
        CapabilityClass::ExternalEffect => BranchMode::SimulationOnly,
        CapabilityClass::DeferredEffect => BranchMode::PromotionGated,
        CapabilityClass::HostSecret => BranchMode::ReplaceBeforeActivation,
        CapabilityClass::BearerCredential => BranchMode::NonBranchable,
    }
}

const fn expected_limits(mode: BranchMode) -> &'static [&'static str] {
    match mode {
        BranchMode::Copyable | BranchMode::NonBranchable => &[],
        BranchMode::Attenuated => &["narrower-resource-or-ability"],
        BranchMode::Linear => &["generation-fenced-transfer", "source-deactivation"],
        BranchMode::SimulationOnly => &["exact-simulation-adapter"],
        BranchMode::PromotionGated => &["promotion-recheck", "release-reservation"],
        BranchMode::ReplaceBeforeActivation => &["fresh-destination-grant"],
    }
}
