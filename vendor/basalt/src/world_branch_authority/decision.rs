use super::identity::{authority_identity, request_identity, rule_identity};
use super::policy::PolicyRule;
use super::{
    AuthorityCurrentness, BranchAction, BranchAuthorityDiagnostic, BranchAuthorityPolicy,
    BranchAuthorityRequest, BranchMode, BranchObligation, MAXIMUM_OBLIGATIONS,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchAuthorityDecision {
    allowed: bool,
    policy_ref: String,
    request_ref: String,
    authority_input_ref: String,
    selected_rule_ref: Option<String>,
    selected_rule_id: Option<String>,
    mode: Option<BranchMode>,
    obligations: Vec<BranchObligation>,
    diagnostic: BranchAuthorityDiagnostic,
}

impl BranchAuthorityDecision {
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        self.allowed
    }

    #[must_use]
    pub fn policy_ref(&self) -> &str {
        &self.policy_ref
    }

    #[must_use]
    pub fn request_ref(&self) -> &str {
        &self.request_ref
    }

    #[must_use]
    pub fn authority_input_ref(&self) -> &str {
        &self.authority_input_ref
    }

    #[must_use]
    pub fn selected_rule_ref(&self) -> Option<&str> {
        self.selected_rule_ref.as_deref()
    }

    #[must_use]
    pub fn selected_rule_id(&self) -> Option<&str> {
        self.selected_rule_id.as_deref()
    }

    #[must_use]
    pub const fn mode(&self) -> Option<BranchMode> {
        self.mode
    }

    #[must_use]
    pub fn obligations(&self) -> &[BranchObligation] {
        &self.obligations
    }

    #[must_use]
    pub const fn diagnostic(&self) -> BranchAuthorityDiagnostic {
        self.diagnostic
    }
}

/// Evaluate supplied normalized facts without parsing tokens or performing effects.
#[must_use]
pub fn evaluate_branch_authority(
    policy: &BranchAuthorityPolicy,
    request: &BranchAuthorityRequest,
    authority: &AuthorityCurrentness,
) -> BranchAuthorityDecision {
    let request_ref = request_identity(request);
    let authority_input_ref = authority_identity(authority);
    if let Some(diagnostic) = initial_diagnostic(policy, request, authority) {
        return denied(policy, request_ref, authority_input_ref, diagnostic);
    }
    let rule = match select_rule(policy, request) {
        Ok(rule) => rule,
        Err(diagnostic) => return denied(policy, request_ref, authority_input_ref, diagnostic),
    };
    if let Some(diagnostic) = rule_diagnostic(rule, request, authority) {
        return denied_with_rule(policy, request_ref, authority_input_ref, rule, diagnostic);
    }
    let obligations = obligations(rule.mode);
    if obligations.len() > MAXIMUM_OBLIGATIONS {
        return denied_with_rule(
            policy,
            request_ref,
            authority_input_ref,
            rule,
            BranchAuthorityDiagnostic::PolicyMalformed,
        );
    }
    BranchAuthorityDecision {
        allowed: true,
        policy_ref: policy.identity().to_string(),
        request_ref,
        authority_input_ref,
        selected_rule_ref: Some(rule_identity(policy.identity(), &rule.rule_id)),
        selected_rule_id: Some(rule.rule_id.clone()),
        mode: Some(rule.mode),
        obligations,
        diagnostic: BranchAuthorityDiagnostic::Admitted,
    }
}

fn initial_diagnostic(
    policy: &BranchAuthorityPolicy,
    request: &BranchAuthorityRequest,
    authority: &AuthorityCurrentness,
) -> Option<BranchAuthorityDiagnostic> {
    if authority.validate().is_err() || !authority.all_current() {
        return Some(BranchAuthorityDiagnostic::CurrentnessDenied);
    }
    if request.policy_generation != policy.generation() {
        return Some(BranchAuthorityDiagnostic::PolicyStale);
    }
    if !request.mapping_lossless {
        return Some(BranchAuthorityDiagnostic::MappingLossy);
    }
    if !request.destination_scope.is_within(&request.source_scope) {
        return Some(BranchAuthorityDiagnostic::ScopeWidened);
    }
    None
}

fn select_rule<'a>(
    policy: &'a BranchAuthorityPolicy,
    request: &BranchAuthorityRequest,
) -> Result<&'a PolicyRule, BranchAuthorityDiagnostic> {
    let rules = policy.matching_rules(request.capability_class);
    let Some(maximum_priority) = rules.iter().map(|rule| rule.priority).max() else {
        return Err(BranchAuthorityDiagnostic::UnknownCapabilityClass);
    };
    let selected = rules
        .into_iter()
        .filter(|rule| rule.priority == maximum_priority)
        .collect::<Vec<_>>();
    match selected.as_slice() {
        [rule] => Ok(rule),
        _ => Err(BranchAuthorityDiagnostic::AmbiguousRules),
    }
}

fn rule_diagnostic(
    rule: &PolicyRule,
    request: &BranchAuthorityRequest,
    authority: &AuthorityCurrentness,
) -> Option<BranchAuthorityDiagnostic> {
    if rule.require_ucan && !authority.ucan.is_verified() {
        return Some(BranchAuthorityDiagnostic::UcanCompositionMissing);
    }
    if !action_matches_mode(request.action, rule.mode) {
        return Some(BranchAuthorityDiagnostic::ActionModeMismatch);
    }
    if rule.mode == BranchMode::Attenuated
        && !request
            .destination_scope
            .is_strictly_narrower_than(&request.source_scope)
    {
        return Some(BranchAuthorityDiagnostic::AttenuationNotNarrower);
    }
    if rule.mode == BranchMode::NonBranchable {
        return Some(BranchAuthorityDiagnostic::NonBranchable);
    }
    None
}

fn denied(
    policy: &BranchAuthorityPolicy,
    request_ref: String,
    authority_input_ref: String,
    diagnostic: BranchAuthorityDiagnostic,
) -> BranchAuthorityDecision {
    BranchAuthorityDecision {
        allowed: false,
        policy_ref: policy.identity().to_string(),
        request_ref,
        authority_input_ref,
        selected_rule_ref: None,
        selected_rule_id: None,
        mode: None,
        obligations: vec![BranchObligation::DenyActivation],
        diagnostic,
    }
}

fn denied_with_rule(
    policy: &BranchAuthorityPolicy,
    request_ref: String,
    authority_input_ref: String,
    rule: &PolicyRule,
    diagnostic: BranchAuthorityDiagnostic,
) -> BranchAuthorityDecision {
    BranchAuthorityDecision {
        allowed: false,
        policy_ref: policy.identity().to_string(),
        request_ref,
        authority_input_ref,
        selected_rule_ref: Some(rule_identity(policy.identity(), &rule.rule_id)),
        selected_rule_id: Some(rule.rule_id.clone()),
        mode: Some(rule.mode),
        obligations: vec![BranchObligation::DenyActivation],
        diagnostic,
    }
}

const fn action_matches_mode(action: BranchAction, mode: BranchMode) -> bool {
    match mode {
        BranchMode::Copyable | BranchMode::Attenuated => {
            matches!(action, BranchAction::Create | BranchAction::Activate)
        }
        BranchMode::Linear => matches!(action, BranchAction::Transfer),
        BranchMode::SimulationOnly => matches!(action, BranchAction::Simulate),
        BranchMode::PromotionGated => matches!(action, BranchAction::Promote),
        BranchMode::ReplaceBeforeActivation => matches!(action, BranchAction::Activate),
        BranchMode::NonBranchable => false,
    }
}

fn obligations(mode: BranchMode) -> Vec<BranchObligation> {
    match mode {
        BranchMode::Copyable => vec![BranchObligation::IndependentDestinationGrant],
        BranchMode::Attenuated => vec![BranchObligation::NarrowerDerivation],
        BranchMode::Linear => vec![
            BranchObligation::GenerationFencedTransfer,
            BranchObligation::SourceDeactivation,
        ],
        BranchMode::SimulationOnly => vec![BranchObligation::ExactSimulationAdapter],
        BranchMode::PromotionGated => vec![
            BranchObligation::PromotionRecheck,
            BranchObligation::ReleaseReservation,
        ],
        BranchMode::ReplaceBeforeActivation => vec![BranchObligation::FreshDestinationGrant],
        BranchMode::NonBranchable => vec![BranchObligation::DenyActivation],
    }
}
