use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub const MAXIMUM_POLICY_BYTES: usize = 65_536;
pub const MAXIMUM_POLICY_RULES: usize = 64;
pub const MAXIMUM_TEXT_BYTES: usize = 512;
pub const MAXIMUM_ABILITIES: usize = 32;
pub const MAXIMUM_OBLIGATIONS: usize = 8;
pub const CAPABILITY_CLASS_COUNT: usize = 7;
pub const POLICY_SCHEMA: &str = "basalt.world-branch-authority-policy.v1";
pub const POLICY_PROFILE: &str = "basalt-world-branch-authority-v1";
pub const RECEIPT_SCHEMA: &str = "basalt.world-branch-authority-decision-receipt.v1";

pub const POLICY_NON_CLAIMS: &[&str] = &[
    "a policy decision does not mint or move a capability",
    "a policy receipt is not current authority",
    "token parsing signature verification revocation and replay remain consumer-owned",
    "derivation transfer adapter activation and effect enforcement remain Molten-owned",
    "linear policy does not prove source deactivation or destination activation",
    "simulation-only policy does not prove simulator parity or prevent host escape",
    "promotion-gated policy does not authorize effect dispatch",
    "branch-authority evidence does not prove release eligibility",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityClass {
    PublicArtifact,
    ScopedService,
    ExclusiveLease,
    ExternalEffect,
    DeferredEffect,
    HostSecret,
    BearerCredential,
}

impl CapabilityClass {
    pub const ALL: [Self; CAPABILITY_CLASS_COUNT] = [
        Self::PublicArtifact,
        Self::ScopedService,
        Self::ExclusiveLease,
        Self::ExternalEffect,
        Self::DeferredEffect,
        Self::HostSecret,
        Self::BearerCredential,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublicArtifact => "public-artifact",
            Self::ScopedService => "scoped-service",
            Self::ExclusiveLease => "exclusive-lease",
            Self::ExternalEffect => "external-effect",
            Self::DeferredEffect => "deferred-effect",
            Self::HostSecret => "host-secret",
            Self::BearerCredential => "bearer-credential",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BranchMode {
    Copyable,
    Attenuated,
    Linear,
    SimulationOnly,
    PromotionGated,
    ReplaceBeforeActivation,
    NonBranchable,
}

impl BranchMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Copyable => "copyable",
            Self::Attenuated => "attenuated",
            Self::Linear => "linear",
            Self::SimulationOnly => "simulation-only",
            Self::PromotionGated => "promotion-gated",
            Self::ReplaceBeforeActivation => "replace-before-activation",
            Self::NonBranchable => "non-branchable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BranchAction {
    Create,
    Activate,
    Promote,
    Simulate,
    Transfer,
}

impl BranchAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Activate => "activate",
            Self::Promote => "promote",
            Self::Simulate => "simulate",
            Self::Transfer => "transfer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityScope {
    resource: String,
    abilities: BTreeSet<String>,
    limit: Option<u64>,
}

impl CapabilityScope {
    /// Construct one bounded normalized scope.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized, control-bearing, bearer-like, or unbounded ability input.
    pub fn new(
        resource: impl Into<String>,
        abilities: impl IntoIterator<Item = String>,
        limit: Option<u64>,
    ) -> Result<Self, BranchAuthorityInputError> {
        let resource = resource.into();
        let abilities = abilities.into_iter().collect::<BTreeSet<_>>();
        if !valid_normalized_text(&resource)
            || abilities.is_empty()
            || abilities.len() > MAXIMUM_ABILITIES
            || abilities
                .iter()
                .any(|ability| !valid_normalized_text(ability))
        {
            return Err(BranchAuthorityInputError::InvalidScope);
        }
        Ok(Self {
            resource,
            abilities,
            limit,
        })
    }

    #[must_use]
    pub fn resource(&self) -> &str {
        &self.resource
    }

    #[must_use]
    pub const fn limit(&self) -> Option<u64> {
        self.limit
    }

    #[must_use]
    pub const fn abilities(&self) -> &BTreeSet<String> {
        &self.abilities
    }

    #[must_use]
    pub fn is_within(&self, source: &Self) -> bool {
        self.resource.starts_with(&source.resource)
            && self.abilities.is_subset(&source.abilities)
            && match (self.limit, source.limit) {
                (Some(destination), Some(origin)) => destination <= origin,
                (None, Some(_)) => false,
                _ => true,
            }
    }

    #[must_use]
    pub fn is_strictly_narrower_than(&self, source: &Self) -> bool {
        self.is_within(source)
            && (self.resource != source.resource
                || self.abilities != source.abilities
                || matches!((self.limit, source.limit), (Some(destination), Some(origin)) if destination < origin)
                || matches!((self.limit, source.limit), (Some(_), None)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchAuthorityRequest {
    pub capability_class: CapabilityClass,
    pub action: BranchAction,
    pub source_branch_ref: String,
    pub destination_branch_ref: String,
    pub capability_ref: String,
    pub source_scope: CapabilityScope,
    pub destination_scope: CapabilityScope,
    pub policy_generation: u64,
    pub mapping_lossless: bool,
}

impl BranchAuthorityRequest {
    /// Construct one normalized branch-authority request.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identities, a self-branch transition, zero generation, or unsafe scope input.
    #[allow(
        clippy::too_many_arguments,
        reason = "every authority binding is explicit at construction"
    )]
    pub fn new(
        capability_class: CapabilityClass,
        action: BranchAction,
        source_branch_ref: impl Into<String>,
        destination_branch_ref: impl Into<String>,
        capability_ref: impl Into<String>,
        source_scope: CapabilityScope,
        destination_scope: CapabilityScope,
        policy_generation: u64,
        mapping_lossless: bool,
    ) -> Result<Self, BranchAuthorityInputError> {
        let source_branch_ref = source_branch_ref.into();
        let destination_branch_ref = destination_branch_ref.into();
        let capability_ref = capability_ref.into();
        if policy_generation == 0
            || !valid_content_ref(&source_branch_ref)
            || !valid_content_ref(&destination_branch_ref)
            || !valid_content_ref(&capability_ref)
            || source_branch_ref == destination_branch_ref
        {
            return Err(BranchAuthorityInputError::InvalidReference);
        }
        Ok(Self {
            capability_class,
            action,
            source_branch_ref,
            destination_branch_ref,
            capability_ref,
            source_scope,
            destination_scope,
            policy_generation,
            mapping_lossless,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentFact {
    Current,
    Stale,
}

impl CurrentFact {
    #[must_use]
    pub const fn is_current(self) -> bool {
        matches!(self, Self::Current)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UcanComposition {
    Verified,
    Missing,
}

impl UcanComposition {
    #[must_use]
    pub const fn is_verified(self) -> bool {
        matches!(self, Self::Verified)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityCurrentness {
    pub observation_ref: String,
    pub policy: CurrentFact,
    pub capability: CurrentFact,
    pub revocation: CurrentFact,
    pub replay: CurrentFact,
    pub scope: CurrentFact,
    pub ucan: UcanComposition,
}

impl AuthorityCurrentness {
    /// Validate the detached observation identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the observation identity is not canonical BLAKE3 text.
    pub fn validate(&self) -> Result<(), BranchAuthorityInputError> {
        if !valid_content_ref(&self.observation_ref) {
            return Err(BranchAuthorityInputError::InvalidReference);
        }
        Ok(())
    }

    #[must_use]
    pub const fn all_current(&self) -> bool {
        self.policy.is_current()
            && self.capability.is_current()
            && self.revocation.is_current()
            && self.replay.is_current()
            && self.scope.is_current()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BranchObligation {
    IndependentDestinationGrant,
    NarrowerDerivation,
    GenerationFencedTransfer,
    SourceDeactivation,
    ExactSimulationAdapter,
    PromotionRecheck,
    ReleaseReservation,
    FreshDestinationGrant,
    DenyActivation,
}

impl BranchObligation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IndependentDestinationGrant => "independent-destination-grant",
            Self::NarrowerDerivation => "narrower-derivation",
            Self::GenerationFencedTransfer => "generation-fenced-transfer",
            Self::SourceDeactivation => "source-deactivation",
            Self::ExactSimulationAdapter => "exact-simulation-adapter",
            Self::PromotionRecheck => "promotion-recheck",
            Self::ReleaseReservation => "release-reservation",
            Self::FreshDestinationGrant => "fresh-destination-grant",
            Self::DenyActivation => "deny-activation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BranchAuthorityDiagnostic {
    Admitted,
    PolicyMalformed,
    PolicyStale,
    UnknownCapabilityClass,
    AmbiguousRules,
    MappingLossy,
    CurrentnessDenied,
    UcanCompositionMissing,
    ScopeWidened,
    AttenuationNotNarrower,
    ActionModeMismatch,
    NonBranchable,
}

impl BranchAuthorityDiagnostic {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::PolicyMalformed => "policy-malformed",
            Self::PolicyStale => "policy-stale",
            Self::UnknownCapabilityClass => "unknown-capability-class",
            Self::AmbiguousRules => "ambiguous-rules",
            Self::MappingLossy => "mapping-lossy",
            Self::CurrentnessDenied => "currentness-denied",
            Self::UcanCompositionMissing => "ucan-composition-missing",
            Self::ScopeWidened => "scope-widened",
            Self::AttenuationNotNarrower => "attenuation-not-narrower",
            Self::ActionModeMismatch => "action-mode-mismatch",
            Self::NonBranchable => "non-branchable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchAuthorityInputError {
    InvalidReference,
    InvalidScope,
}

pub(super) fn valid_content_ref(value: &str) -> bool {
    const PREFIX: &str = "blake3:";
    const HEX_LENGTH: usize = 64;
    value.strip_prefix(PREFIX).is_some_and(|hex| {
        hex.len() == HEX_LENGTH
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

pub(super) fn valid_normalized_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_TEXT_BYTES
        && !value.chars().any(char::is_control)
        && !value.contains("token")
        && !value.contains("secret=")
}
