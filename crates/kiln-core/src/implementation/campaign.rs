//! Versioned selected-set implementation campaigns.
//!
//! Campaign state coordinates existing owners; it never creates work claims,
//! verifies candidates, publishes, integrates, or mutates Cairn lifecycle.
use alloc::{
    collections::{BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec::Vec,
};
use serde::{Deserialize, Serialize};

use crate::types::{ContentId, CoreResult, Diagnostic};

pub const CAMPAIGN_SCHEMA: &str = "kiln.implementation-campaign.v1";
pub const MAX_MEMBERS: usize = 1_000;
pub const MAX_DEPENDENCIES: usize = 64;
pub const MAX_CONTEXT_ITEMS: usize = 128;
pub const MAX_CONTEXT_MANIFEST_BYTES: usize = 65_536;
pub const MAX_OUTSTANDING_DECISIONS: usize = 256;
pub const MAX_ID_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemberSelector {
    WholeChange { change: String },
    Tasks { change: String, tasks: Vec<String> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionGoal {
    VerifiedCandidate,
    VerifiedPullRequest,
    IntegratedChange,
    NativeLifecycleComplete,
}

impl CompletionGoal {
    pub const fn reached_by(self, stage: MemberStage) -> bool {
        match self {
            Self::VerifiedCandidate => matches!(
                stage,
                MemberStage::VerifiedCandidate
                    | MemberStage::VerifiedPullRequest
                    | MemberStage::Integrated
                    | MemberStage::Complete
            ),
            Self::VerifiedPullRequest => matches!(
                stage,
                MemberStage::VerifiedPullRequest | MemberStage::Integrated | MemberStage::Complete
            ),
            Self::IntegratedChange => {
                matches!(stage, MemberStage::Integrated | MemberStage::Complete)
            }
            Self::NativeLifecycleComplete => matches!(stage, MemberStage::Complete),
        }
    }

    pub const fn unit_goal(self, remaining: bool) -> Self {
        if matches!(self, Self::NativeLifecycleComplete) && remaining {
            Self::IntegratedChange
        } else {
            self
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowProfile {
    SpecDriven,
    Assurance,
    OutcomeMachine,
    NoSpecDelta,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
// r[impl kiln.implementation.campaign.context]
pub struct DecisionContextBinding {
    pub decision_id: String,
    pub native_adr_id: Option<String>,
    pub revision: String,
    pub selected_alternative: String,
    pub evidence: Vec<ContentId>,
    pub supersedes: Vec<String>,
    pub projection_identity: ContentId,
    pub required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
// r[impl kiln.implementation.campaign.bindings]
pub struct MemberBinding {
    pub selector: MemberSelector,
    pub profile: WorkflowProfile,
    pub profile_version: ContentId,
    pub repository_instructions: ContentId,
    pub policy: ContentId,
    pub required_checks: Vec<String>,
    pub required_reviews: Vec<String>,
    pub integration_policy: ContentId,
    pub goal: CompletionGoal,
    pub prerequisite_inclusion: bool,
    pub context: Vec<DecisionContextBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
// r[impl kiln.implementation.campaign.scope]
pub struct CampaignContract {
    pub schema: String,
    pub campaign_id: String,
    pub revision: u64,
    pub repository: String,
    pub members: Vec<MemberBinding>,
    pub max_concurrency: u16,
    pub max_outstanding_decisions: u16,
    pub parent_budget_id: ContentId,
    pub agent_definition: ContentId,
    pub require_one_writer_per_repository: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnumerationState {
    NotObserved,
    Complete,
    IncompleteLimit,
    MissingSource,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberStage {
    Selected,
    Missing,
    Planning,
    Blocked,
    Ready,
    Claimed,
    Executing,
    Candidate,
    Verifying,
    VerifiedCandidate,
    VerifiedPullRequest,
    Integrated,
    Lifecycle,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Current,
    Stale,
    Unknown,
    NotApplicable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Preference,
    EmpiricalUncertainty,
    RequiredReview,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionRequestState {
    Unissued,
    Outstanding,
    Resolved,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanDecisionRequest {
    pub identity: ContentId,
    pub kind: DecisionKind,
    pub member: String,
    pub work_identity: Option<ContentId>,
    pub decision_revision: Option<String>,
    pub policy_identity: ContentId,
    pub candidate_identity: Option<ContentId>,
    pub state: DecisionRequestState,
    pub created_ms: u64,
    pub question: String,
    pub options: Vec<String>,
    pub recommendation: Option<String>,
    pub priority: u8,
    pub pending_reason: Option<String>,
    pub resolved_answer: Option<String>,
    pub active_effort_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberObservation {
    pub member: String,
    pub stage: MemberStage,
    pub enumeration: EnumerationState,
    pub source_identity: Option<ContentId>,
    pub policy_identity: ContentId,
    pub subject_identity: Option<ContentId>,
    pub integration_target_identity: Option<ContentId>,
    pub profile_identity: ContentId,
    pub dependencies: Vec<String>,
    pub remaining_obligations: BTreeSet<String>,
    pub dependency_reason: Option<String>,
    pub canonical_work_key: Option<ContentId>,
    pub work_identity: Option<ContentId>,
    pub attempt_identity: Option<ContentId>,
    pub attempt_fence: Option<u64>,
    pub context_identity: Option<ContentId>,
    #[serde(default)]
    pub native_context_identity: Option<ContentId>,
    pub context_freshness: Freshness,
    pub candidate_freshness: Freshness,
    pub policy_freshness: Freshness,
    pub verifier_freshness: Freshness,
    pub target_freshness: Freshness,
    pub evidence: Vec<ContentId>,
    pub blockers: Vec<String>,
    pub required_checks_satisfied: bool,
    pub required_reviews_satisfied: bool,
    pub delivery_route_supported: Option<bool>,
    pub recovery: Option<RecoveryDisposition>,
    pub consumed_budget: u64,
    pub reserved_budget: u64,
    pub unknown_budget: u64,
    pub cancelled: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDisposition {
    ArtifactRepair,
    CodingRepair,
    InfrastructureRetry,
    UnknownOutcome,
    SemanticReplan,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedAttemptContext {
    pub attempt_identity: ContentId,
    pub work_identity: ContentId,
    pub selection_identity: ContentId,
    pub context_identity: ContentId,
    pub decision_revisions: Vec<String>,
    pub exact_manifest: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImpactObservation {
    pub member: String,
    pub source_before: Option<ContentId>,
    pub source_after: Option<ContentId>,
    pub policy_before: ContentId,
    pub policy_after: ContentId,
    pub target_freshness_before: Freshness,
    pub target_freshness_after: Freshness,
    pub context_before: Option<ContentId>,
    pub context_after: Option<ContentId>,
    pub target_identity_before: Option<ContentId>,
    pub target_identity_after: Option<ContentId>,
    #[serde(default)]
    pub native_context_before: Option<ContentId>,
    #[serde(default)]
    pub native_context_after: Option<ContentId>,
    pub action: CampaignOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignOperation {
    Reconcile,
    AuthorArtifact,
    RepairArtifact,
    Code,
    RepairCode,
    RetryInfrastructure,
    Verify,
    Publish,
    Integrate,
    Lifecycle,
    RequestDecision,
    Wait,
    Replan,
    Blocked,
    Complete,
}

impl CampaignOperation {
    /// Owner effects the campaign persists as intent before dispatch.
    #[must_use]
    pub const fn is_owner_effect(self) -> bool {
        matches!(
            self,
            Self::AuthorArtifact
                | Self::RepairArtifact
                | Self::Code
                | Self::RepairCode
                | Self::RetryInfrastructure
                | Self::Verify
                | Self::Publish
                | Self::Integrate
                | Self::Lifecycle
        )
    }

    /// Owner effects that need the repository's single canonical writer claim.
    #[must_use]
    pub const fn requires_writer_claim(self) -> bool {
        matches!(
            self,
            Self::AuthorArtifact
                | Self::RepairArtifact
                | Self::Code
                | Self::RepairCode
                | Self::RetryInfrastructure
        )
    }

    /// Owner that resolves this operation when no observation names one.
    #[must_use]
    pub const fn owner(self) -> &'static str {
        match self {
            Self::Code | Self::RepairCode | Self::RetryInfrastructure => "work-attempt-owner",
            Self::AuthorArtifact | Self::RepairArtifact => "artifact-owner",
            Self::Verify => "verification-owner",
            Self::Publish | Self::Integrate => "publication-owner",
            Self::Lifecycle => "cairn-lifecycle-owner",
            Self::Reconcile => "attempt-reconciliation-owner",
            Self::RequestDecision | Self::Wait | Self::Replan | Self::Blocked | Self::Complete => {
                "campaign-policy"
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignBudgetObservation {
    pub parent_budget_identity: ContentId,
    pub owner_observation_identity: ContentId,
    pub consumed: u64,
    pub reserved: u64,
    pub unknown: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingEffect {
    pub effect_id: ContentId,
    pub member: String,
    pub operation: CampaignOperation,
    pub owner: String,
    pub intent_identity: ContentId,
    pub state: EffectState,
    pub progress_applied: bool,
    pub selection_identity: ContentId,
    #[serde(default)]
    pub source_identity: Option<ContentId>,
    pub work_identity: Option<ContentId>,
    pub attempt_identity: Option<ContentId>,
    pub attempt_fence: Option<u64>,
    pub subject_identity: Option<ContentId>,
    pub policy_identity: ContentId,
    pub context_identity: Option<ContentId>,
    #[serde(default)]
    pub native_context_identity: Option<ContentId>,
    pub profile_identity: ContentId,
    #[serde(default)]
    pub evidence_identity: Option<ContentId>,
    pub integration_target_identity: Option<ContentId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectState {
    IntentPersisted,
    Unknown,
    ObservedComplete,
    ObservedFailed,
    RejectedStale,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignState {
    pub contract: CampaignContract,
    pub selection_identity: ContentId,
    pub observations: BTreeMap<String, MemberObservation>,
    pub decisions: BTreeMap<ContentId, HumanDecisionRequest>,
    pub pending_effects: BTreeMap<ContentId, PendingEffect>,
    pub paused: bool,
    pub cancelled: bool,
    pub revision_history: Vec<ContentId>,
    pub attempt_contexts: BTreeMap<ContentId, BTreeMap<ContentId, RetainedAttemptContext>>,
    pub impacts: Vec<ImpactObservation>,
    pub budget: Option<CampaignBudgetObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignDecision {
    pub operation: CampaignOperation,
    pub member: Option<String>,
    pub reason: String,
    pub owner: Option<String>,
    pub effect_intent: Option<ContentId>,
}

/// Why a campaign evaluation selected its decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionBasis {
    /// An unapplied effect, cancellation, completion, pause or bounded
    /// decision demand precedes all member work.
    CampaignBarrier,
    /// The first member, in deterministic order, with an admissible owner effect.
    MemberAction,
    /// No member can act; the first member-local hold is surfaced.
    MemberHold,
    /// No member has an action or a hold of its own.
    NoEligibleMember,
}

/// One selected decision and every member-local explanation retained with it.
/// A hold binds only its member: another member's action neither resolves
/// nor hides it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignEvaluation {
    pub decision: CampaignDecision,
    pub basis: SelectionBasis,
    /// Waits, blockers and replans in selection order, one per unsatisfied member.
    pub holds: Vec<CampaignDecision>,
}

/// One member's position in campaign selection.
enum MemberEvaluation {
    /// An owner effect the campaign may persist and dispatch now.
    Action(CampaignDecision),
    /// A condition in this member's own scope, surfaced only when no member can act.
    Hold(CampaignDecision),
    /// A wait on a dependency, the repository writer slot, an incomplete
    /// observation or a stage without a campaign action. It stays visible but
    /// is never surfaced in place of a member's own hold.
    Deferred(CampaignDecision),
}

impl CampaignContract {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema != CAMPAIGN_SCHEMA
            || !valid(&self.campaign_id)
            || !valid(&self.repository)
            || self.revision == 0
            || self.members.is_empty()
            || self.members.len() > MAX_MEMBERS
            || self.max_concurrency == 0
            || self.max_concurrency as usize > MAX_MEMBERS
            || self.max_outstanding_decisions as usize > MAX_OUTSTANDING_DECISIONS
            || !self.require_one_writer_per_repository
            || self.members.iter().any(|m| {
                !selector_valid(&m.selector)
                    || m.context.len() > MAX_CONTEXT_ITEMS
                    || m.required_checks.len() > 128
                    || m.required_reviews.len() > 128
                    || m.context.iter().any(|c| {
                        !valid(&c.decision_id)
                            || !valid(&c.revision)
                            || !valid(&c.selected_alternative)
                            || c.evidence.len() > MAX_CONTEXT_ITEMS
                            || c.supersedes.len() > MAX_CONTEXT_ITEMS
                    })
            })
        {
            return Err(refusal(
                "campaign_contract_invalid",
                "campaign",
                "campaign contract is malformed or exceeds a declared bound",
            ));
        }
        self.parent_budget_id.validate()?;
        self.agent_definition.validate()?;
        for binding in &self.members {
            binding.profile_version.validate()?;
            binding.repository_instructions.validate()?;
            binding.policy.validate()?;
            binding.integration_policy.validate()?;
            if binding
                .required_checks
                .iter()
                .chain(&binding.required_reviews)
                .any(|value| !valid(value) || value.len() > 512)
            {
                return Err(refusal(
                    "campaign_policy_binding",
                    "campaign.policy",
                    "check or review binding is empty, oversized, or malformed",
                ));
            }
            for context in &binding.context {
                context.projection_identity.validate()?;
                for evidence in &context.evidence {
                    evidence.validate()?;
                }
                if context.native_adr_id.as_ref().is_some_and(|id| !valid(id))
                    || context.supersedes.iter().any(|id| !valid(id))
                {
                    return Err(refusal(
                        "campaign_context_binding",
                        "campaign.context",
                        "decision or ADR identity is malformed",
                    ));
                }
            }
        }
        let mut selectors = BTreeSet::new();
        if self
            .members
            .iter()
            .any(|m| !selectors.insert(m.selector.clone()))
        {
            return Err(refusal(
                "campaign_selection_duplicate",
                "campaign.members",
                "duplicate member selector",
            ));
        }
        Ok(())
    }
    pub fn identity(&self) -> CoreResult<ContentId> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| {
            refusal(
                "campaign_encoding",
                "campaign",
                "campaign identity encoding failed",
            )
        })?;
        ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
    }
}

impl CampaignState {
    pub fn start(contract: CampaignContract) -> CoreResult<Self> {
        contract.validate()?;
        let selection_identity = contract.identity()?;
        Ok(Self {
            contract,
            selection_identity,
            observations: BTreeMap::new(),
            decisions: BTreeMap::new(),
            pending_effects: BTreeMap::new(),
            paused: false,
            cancelled: false,
            revision_history: Vec::new(),
            attempt_contexts: BTreeMap::new(),
            impacts: Vec::new(),
            budget: None,
        })
    }
    pub fn revise(&self, next: CampaignContract, explicit_supersession: bool) -> CoreResult<Self> {
        if !explicit_supersession
            || next.campaign_id != self.contract.campaign_id
            || next.revision <= self.contract.revision
        {
            return Err(refusal(
                "campaign_revision_requires_supersession",
                "campaign.revision",
                "scope or policy revision requires explicit supersession and a higher revision",
            ));
        }
        next.validate()?;
        let mut state = Self::start(next)?;
        state.revision_history = self.revision_history.clone();
        state.revision_history.push(self.selection_identity.clone());
        state.observations = self.observations.clone();
        state.decisions = self.decisions.clone();
        state.pending_effects = self.pending_effects.clone();
        state.attempt_contexts = self.attempt_contexts.clone();
        state.impacts = self.impacts.clone();
        state.budget = self.budget.clone();
        Ok(state)
    }
    pub fn observe(&mut self, observation: MemberObservation) -> CoreResult<()> {
        self.observe_source(observation, false)
    }

    pub fn observe_successor(
        &mut self,
        observation: MemberObservation,
        integration_effect: &ContentId,
    ) -> CoreResult<()> {
        let effect = self
            .pending_effects
            .get(integration_effect)
            .ok_or_else(|| {
                refusal(
                    "campaign_successor_unobserved",
                    "observation.source",
                    "source progress has no retained integration effect",
                )
            })?;
        if self.cancelled
            || effect.operation != CampaignOperation::Integrate
            || effect.state != EffectState::ObservedComplete
            || !effect.progress_applied
            || effect.selection_identity != self.selection_identity
            || effect.evidence_identity.is_none()
            || observation.attempt_identity.is_some()
            || self
                .observations
                .get(&observation.member)
                .is_some_and(|old| {
                    matches!(
                        old.stage,
                        MemberStage::Claimed
                            | MemberStage::Executing
                            | MemberStage::Candidate
                            | MemberStage::Verifying
                    )
                })
        {
            return Err(refusal(
                "campaign_successor_unobserved",
                "observation.source",
                "ordinary source progress requires an exact observed integration and no in-flight replacement",
            ));
        }
        self.observe_source(observation, true)
    }

    // r[impl kiln.implementation.campaign.scope.scenario.freeze]
    fn observe_source(
        &mut self,
        observation: MemberObservation,
        observed_progress: bool,
    ) -> CoreResult<()> {
        let member = member_key(&observation.member);
        if !self
            .contract
            .members
            .iter()
            .any(|m| selector_key(&m.selector) == member)
        {
            return Err(refusal(
                "campaign_observation_out_of_scope",
                "observation.member",
                "observation is outside frozen selection",
            ));
        }
        let binding = self
            .contract
            .members
            .iter()
            .find(|binding| selector_key(&binding.selector) == member)
            .ok_or_else(|| {
                refusal(
                    "campaign_observation_out_of_scope",
                    "observation.member",
                    "observation is outside frozen selection",
                )
            })?;
        if !self.observations.contains_key(&member)
            && (observation.profile_identity != binding.profile_version
                || observation.policy_identity != binding.policy)
        {
            return Err(refusal(
                "campaign_native_binding_mismatch",
                "observation.profile_or_policy",
                "initial native observation does not match the selected profile and repository policy bindings",
            ));
        }
        if observation.dependencies.len() > MAX_DEPENDENCIES
            || observation.evidence.len() > MAX_CONTEXT_ITEMS
            || observation.remaining_obligations.len() > MAX_MEMBERS
            || observation
                .remaining_obligations
                .iter()
                .any(|unit| !valid(unit))
        {
            return Err(refusal(
                "campaign_observation_bound",
                "observation",
                "observation exceeds bounds",
            ));
        }
        if let Some(previous) = self.observations.get(&member) {
            let source_changed = previous.source_identity != observation.source_identity;
            let policy_changed = previous.policy_identity != observation.policy_identity
                || previous.profile_identity != observation.profile_identity;
            let target_changed = previous.target_freshness != observation.target_freshness
                || previous.integration_target_identity != observation.integration_target_identity;
            let context_changed = previous.context_identity != observation.context_identity;
            let native_context_changed =
                previous.native_context_identity != observation.native_context_identity;
            if source_changed
                || policy_changed
                || target_changed
                || context_changed
                || native_context_changed
            {
                let requires_replan = policy_changed
                    || context_changed
                    || (!observed_progress
                        && (source_changed || target_changed || native_context_changed));
                let mut changed = observation;
                self.impacts.push(ImpactObservation {
                    member: member.clone(),
                    source_before: previous.source_identity.clone(),
                    source_after: changed.source_identity.clone(),
                    policy_before: previous.policy_identity.clone(),
                    policy_after: changed.policy_identity.clone(),
                    target_freshness_before: previous.target_freshness,
                    target_freshness_after: changed.target_freshness,
                    context_before: previous.context_identity.clone(),
                    context_after: changed.context_identity.clone(),
                    target_identity_before: previous.integration_target_identity.clone(),
                    target_identity_after: changed.integration_target_identity.clone(),
                    native_context_before: previous.native_context_identity.clone(),
                    native_context_after: changed.native_context_identity.clone(),
                    action: if requires_replan {
                        CampaignOperation::Replan
                    } else if matches!(changed.stage, MemberStage::Selected | MemberStage::Planning)
                    {
                        CampaignOperation::AuthorArtifact
                    } else {
                        CampaignOperation::Code
                    },
                });
                if requires_replan {
                    changed.blockers.push(format!("impact_replan:source={source_changed},policy={policy_changed},target={target_changed},context={context_changed},native={native_context_changed}"));
                    changed.stage = MemberStage::Blocked;
                }
                self.observations.insert(member, changed);
                return Ok(());
            }
        }
        self.observations.insert(member, observation);
        Ok(())
    }
    pub fn observe_budget(&mut self, budget: CampaignBudgetObservation) -> CoreResult<()> {
        if budget.parent_budget_identity != self.contract.parent_budget_id {
            return Err(refusal(
                "campaign_budget_scope",
                "campaign.budget",
                "budget observation does not match the configured parent lineage",
            ));
        }
        budget.owner_observation_identity.validate()?;
        if self.budget.as_ref().is_some_and(|previous| {
            previous.owner_observation_identity == budget.owner_observation_identity
                && previous != &budget
        }) {
            return Err(refusal(
                "campaign_budget_conflict",
                "campaign.budget",
                "one parent-budget observation identity has conflicting accounting values",
            ));
        }
        self.budget = Some(budget);
        Ok(())
    }
    pub fn retain_attempt_context(&mut self, context: RetainedAttemptContext) -> CoreResult<()> {
        if context.exact_manifest.is_empty()
            || context.exact_manifest.len() > MAX_CONTEXT_MANIFEST_BYTES
            || context.decision_revisions.len() > MAX_CONTEXT_ITEMS
            || context.selection_identity != self.selection_identity
        {
            return Err(refusal(
                "campaign_context_bound",
                "attempt.context",
                "attempt context exceeds bounds or belongs to another frozen selection",
            ));
        }
        let observation = self
            .observations
            .values()
            .find(|o| o.attempt_identity.as_ref() == Some(&context.attempt_identity));
        let Some(observation) = observation else {
            return Err(refusal(
                "campaign_context_attempt_scope",
                "attempt.context",
                "context does not bind a retained attempt and work version",
            ));
        };
        if observation.work_identity.as_ref() != Some(&context.work_identity) {
            return Err(refusal(
                "campaign_context_attempt_scope",
                "attempt.context",
                "context does not bind a retained attempt and work version",
            ));
        }
        let binding = self
            .contract
            .members
            .iter()
            .find(|m| selector_key(&m.selector) == observation.member)
            .ok_or_else(|| {
                refusal(
                    "campaign_context_manifest_scope",
                    "attempt.context",
                    "attempt has no selected member binding",
                )
            })?;
        let expected_revisions = binding
            .context
            .iter()
            .filter(|manifest| manifest.projection_identity == context.context_identity)
            .map(|manifest| manifest.revision.clone())
            .collect::<Vec<_>>();
        if expected_revisions.is_empty() || expected_revisions != context.decision_revisions {
            return Err(refusal(
                "campaign_context_manifest_scope",
                "attempt.context",
                "attempt context does not match its declared decision projection and revisions",
            ));
        }
        let digest = blake3::hash(&context.exact_manifest).to_hex().to_string();
        if context.context_identity.as_str() != digest {
            return Err(refusal(
                "campaign_context_identity",
                "attempt.context",
                "exact retained manifest does not match context identity",
            ));
        }
        let retained = self
            .attempt_contexts
            .entry(context.attempt_identity.clone())
            .or_default();
        if let Some(existing) = retained.get(&context.context_identity) {
            if existing != &context {
                return Err(refusal(
                    "campaign_context_immutable",
                    "attempt.context",
                    "attempt context is immutable once retained",
                ));
            }
            return Ok(());
        }
        let retained_bytes = retained.values().fold(0usize, |total, entry| {
            total.saturating_add(entry.exact_manifest.len())
        });
        if retained_bytes.saturating_add(context.exact_manifest.len()) > MAX_CONTEXT_MANIFEST_BYTES
        {
            return Err(refusal(
                "campaign_context_bound",
                "attempt.context",
                "aggregate retained attempt context exceeds its byte bound",
            ));
        }
        retained.insert(context.context_identity.clone(), context);
        Ok(())
    }
    /// Whether a decision request still names the member's current scope: the
    /// member is observed under the same policy, and any bound work and
    /// candidate are the observed work and part of the observed evidence.
    #[must_use]
    pub fn decision_in_current_scope(&self, request: &HumanDecisionRequest) -> bool {
        self.observations
            .get(&member_key(&request.member))
            .is_some_and(|o| {
                o.policy_identity == request.policy_identity
                    && request
                        .work_identity
                        .as_ref()
                        .is_none_or(|id| o.work_identity.as_ref() == Some(id))
                    && request
                        .candidate_identity
                        .as_ref()
                        .is_none_or(|id| o.evidence.contains(id))
            })
    }
    pub fn add_decision(&mut self, request: HumanDecisionRequest) -> CoreResult<()> {
        if request.question.trim().is_empty()
            || request.question.len() > 1_024
            || request.options.is_empty()
            || request.options.len() > 32
            || request.options.iter().any(|v| !valid(v) || v.len() > 512)
            || request
                .recommendation
                .as_ref()
                .is_some_and(|v| v.len() > 512)
            || request
                .pending_reason
                .as_ref()
                .is_some_and(|v| v.len() > 512)
            || !self.decision_in_current_scope(&request)
        {
            return Err(refusal(
                "campaign_decision_scope",
                "decision.member",
                "decision request is malformed or does not match current retained work/policy/candidate scope",
            ));
        }
        if let Some(existing) = self.decisions.get(&request.identity) {
            let same_scope = existing.kind == request.kind
                && existing.member == request.member
                && existing.work_identity == request.work_identity
                && existing.decision_revision == request.decision_revision
                && existing.policy_identity == request.policy_identity
                && existing.candidate_identity == request.candidate_identity
                && existing.question == request.question
                && existing.options == request.options
                && existing.recommendation == request.recommendation
                && existing.priority == request.priority;
            if !same_scope {
                return Err(refusal(
                    "campaign_decision_identity_conflict",
                    "decision.identity",
                    "one decision identity cannot name conflicting question or scope data",
                ));
            }
            return Ok(());
        }
        let outstanding = self
            .decisions
            .values()
            .filter(|r| r.state == DecisionRequestState::Outstanding)
            .count();
        let mut retained = request;
        if retained.state == DecisionRequestState::Outstanding
            && outstanding >= self.contract.max_outstanding_decisions as usize
        {
            retained.state = DecisionRequestState::Unissued;
            retained.pending_reason = Some("configured outstanding decision bound reached".into());
        }
        self.decisions.insert(retained.identity.clone(), retained);
        Ok(())
    }
    pub fn issue_decision(&mut self, identity: &ContentId) -> CoreResult<()> {
        if self.cancelled || self.paused {
            return Err(refusal(
                "campaign_admission_closed",
                "decision",
                "paused or cancelled campaigns cannot issue new decision requests",
            ));
        }
        let outstanding = self
            .decisions
            .values()
            .filter(|r| r.state == DecisionRequestState::Outstanding)
            .count();
        if outstanding >= self.contract.max_outstanding_decisions as usize {
            return Err(refusal(
                "campaign_decision_queue_full",
                "decision",
                "outstanding decision bound is full",
            ));
        }
        let request = self.decisions.get_mut(identity).ok_or_else(|| {
            refusal(
                "campaign_decision_unknown",
                "decision.identity",
                "decision request is not retained",
            )
        })?;
        if request.state != DecisionRequestState::Unissued {
            return Err(refusal(
                "campaign_decision_state",
                "decision",
                "only an unissued request can be issued",
            ));
        }
        request.state = DecisionRequestState::Outstanding;
        request.pending_reason = None;
        Ok(())
    }
    pub fn resolve_decision(
        &mut self,
        identity: &ContentId,
        work: Option<&ContentId>,
        policy: &ContentId,
        candidate: Option<&ContentId>,
        answer: &str,
    ) -> CoreResult<()> {
        let in_scope = self
            .decisions
            .get(identity)
            .is_some_and(|request| self.decision_in_current_scope(request));
        let request = self.decisions.get_mut(identity).ok_or_else(|| {
            refusal(
                "campaign_decision_unknown",
                "decision.identity",
                "decision request is not retained",
            )
        })?;
        if request.state != DecisionRequestState::Outstanding
            || request.work_identity.as_ref() != work
            || &request.policy_identity != policy
            || request.candidate_identity.as_ref() != candidate
            || !request.options.iter().any(|option| option == answer)
        {
            return Err(refusal(
                "campaign_decision_stale",
                "decision",
                "response is stale, wrong-scope, or not one of the declared options",
            ));
        }
        if !in_scope {
            return Err(refusal(
                "campaign_decision_scope_changed",
                "decision",
                "the member's current work, policy or candidate no longer matches this decision",
            ));
        }
        request.resolved_answer = Some(answer.to_string());
        request.state = DecisionRequestState::Resolved;
        Ok(())
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.paused = true;
    }
    pub fn pause(&mut self) {
        self.paused = true;
    }
    pub fn continue_admission(&mut self) {
        if !self.cancelled {
            self.paused = false;
        }
    }
    /// The selected decision of [`Self::evaluate`].
    pub fn next_action(&self) -> CoreResult<CampaignDecision> {
        Ok(self.evaluate().decision)
    }

    // r[impl kiln.implementation.campaign.independent_progress]
    /// Evaluate every selected member into an admissible owner action or a
    /// member-local hold, then select one decision. Campaign barriers come
    /// first. Otherwise the first member action in deterministic order wins,
    /// so one member's review, context, replan or delivery hold cannot
    /// suppress another member's eligible work. A hold is surfaced only when
    /// no member can act, and every hold is retained beside the selection.
    pub fn evaluate(&self) -> CampaignEvaluation {
        let mut action = None;
        let mut surfaced = None;
        let mut holds = Vec::new();
        for member in self.evaluate_members() {
            match member {
                MemberEvaluation::Action(decision) => {
                    if action.is_none() {
                        action = Some(decision);
                    }
                }
                MemberEvaluation::Hold(decision) => {
                    if surfaced.is_none() {
                        surfaced = Some(decision.clone());
                    }
                    holds.push(decision);
                }
                MemberEvaluation::Deferred(decision) => holds.push(decision),
            }
        }
        let (decision, basis) = if let Some(barrier) = self.campaign_barrier() {
            (barrier, SelectionBasis::CampaignBarrier)
        } else if let Some(action) = action {
            (action, SelectionBasis::MemberAction)
        } else if let Some(hold) = surfaced {
            (hold, SelectionBasis::MemberHold)
        } else {
            (self.no_eligible_member(), SelectionBasis::NoEligibleMember)
        };
        CampaignEvaluation {
            decision,
            basis,
            holds,
        }
    }

    /// Campaign-wide conditions that precede every member action.
    // r[impl kiln.implementation.campaign.controller.scenario.restart]
    fn campaign_barrier(&self) -> Option<CampaignDecision> {
        if let Some(effect) = self.pending_effects.values().find(|e| !e.progress_applied) {
            let reason = if matches!(
                effect.state,
                EffectState::IntentPersisted | EffectState::Unknown
            ) {
                "pending/uncertain owner effect must reconcile before a new admission"
            } else {
                "observed owner result must be atomically applied to member progress before a new action"
            };
            return Some(decision(
                CampaignOperation::Reconcile,
                Some(effect.member.clone()),
                reason,
                Some(effect.owner.clone()),
                Some(effect.intent_identity.clone()),
            ));
        }
        if self.cancelled {
            return Some(decision(
                CampaignOperation::Blocked,
                None,
                "campaign cancelled; successor admission is closed and provider cleanup is unobserved",
                None,
                None,
            ));
        }
        if self.complete() {
            return Some(decision(
                CampaignOperation::Complete,
                None,
                "every required selected member reached its configured endpoint",
                None,
                None,
            ));
        }
        if self.paused {
            return Some(decision(
                CampaignOperation::Wait,
                None,
                "successor admission paused by operator",
                None,
                None,
            ));
        }
        let issued = self
            .decisions
            .values()
            .filter(|r| r.state == DecisionRequestState::Outstanding)
            .count();
        if issued >= self.contract.max_outstanding_decisions as usize {
            return None;
        }
        self.decisions
            .values()
            .find(|r| r.state == DecisionRequestState::Unissued)
            .map(|request| {
                decision(
                    CampaignOperation::RequestDecision,
                    Some(request.member.clone()),
                    "human-decision demand is retained and available under the configured outstanding bound",
                    Some("review-owner".into()),
                    Some(request.identity.clone()),
                )
            })
    }

    fn no_eligible_member(&self) -> CampaignDecision {
        let absent = self
            .contract
            .members
            .iter()
            .any(|m| !self.observations.contains_key(&selector_key(&m.selector)));
        decision(
            if absent {
                CampaignOperation::Wait
            } else {
                CampaignOperation::Blocked
            },
            None,
            if absent {
                "selected member awaits source/readiness observation"
            } else {
                "no currently eligible action; obligations and blockers remain visible"
            },
            None,
            None,
        )
    }

    /// Every unsatisfied selected member in deterministic selection order:
    /// unblocked before blocked, then by canonical member key.
    // r[impl kiln.implementation.campaign.controller]
    fn evaluate_members(&self) -> Vec<MemberEvaluation> {
        let active = self
            .observations
            .values()
            .filter(|o| {
                matches!(
                    o.stage,
                    MemberStage::Claimed
                        | MemberStage::Executing
                        | MemberStage::Candidate
                        | MemberStage::Verifying
                )
            })
            .count();
        let writing_limit = if self.contract.require_one_writer_per_repository {
            1
        } else {
            self.contract.max_concurrency as usize
        };
        let writer_slot_free = active < writing_limit;
        let mut members: Vec<_> = self
            .contract
            .members
            .iter()
            .filter_map(|binding| {
                let key = selector_key(&binding.selector);
                let Some(observation) = self.observations.get(&key) else {
                    let wait = deferred(
                        CampaignOperation::Wait,
                        &key,
                        "selected member awaits its first source/readiness observation",
                        "native-source-owner",
                    );
                    return Some((false, key, wait));
                };
                if observation.cancelled || self.member_endpoint_satisfied(binding, observation) {
                    return None;
                }
                let evaluation = self.evaluate_member(&key, binding, observation, writer_slot_free);
                Some((observation.stage == MemberStage::Blocked, key, evaluation))
            })
            .collect();
        members.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        members
            .into_iter()
            .map(|(_, _, evaluation)| evaluation)
            .collect()
    }

    /// One member's admissible owner action or member-local hold. Only writer
    /// claims consult state outside the member: the shared writer slot.
    fn evaluate_member(
        &self,
        key: &str,
        binding: &MemberBinding,
        obs: &MemberObservation,
        writer_slot_free: bool,
    ) -> MemberEvaluation {
        use CampaignOperation as Op;
        if obs
            .dependencies
            .iter()
            .any(|dependency| !self.dependency_satisfied(dependency))
        {
            return deferred(
                Op::Wait,
                key,
                "an unsatisfied selected dependency keeps this member waiting; the dependency is evaluated in its own scope",
                "campaign-policy",
            );
        }
        if obs.enumeration != EnumerationState::Complete || obs.stage == MemberStage::Missing {
            return deferred(
                Op::Wait,
                key,
                "native enumeration is incomplete or the member source is missing; selection needs a complete owner observation",
                "native-source-owner",
            );
        }
        if [
            obs.context_freshness,
            obs.policy_freshness,
            obs.candidate_freshness,
            obs.verifier_freshness,
            obs.target_freshness,
        ]
        .contains(&Freshness::Stale)
        {
            return hold(
                Op::Replan,
                key,
                "source/policy/context/candidate/verifier/target freshness requires explicit impact and replan",
                "campaign-policy",
                None,
            );
        }
        if binding.context.iter().any(|context| context.required)
            && (obs.context_identity.is_none() || obs.context_freshness != Freshness::Current)
        {
            return hold(
                Op::Blocked,
                key,
                "required decision context is missing or unobserved; optional context remains nonblocking",
                "decision-context-owner",
                None,
            );
        }
        let review = self.decisions.values().find_map(|request| {
            if request.member != key || request.kind != DecisionKind::RequiredReview {
                return None;
            }
            review_wait(&request.state).map(|reason| (request.identity.clone(), reason))
        });
        if let Some((identity, reason)) = review {
            return hold(Op::Wait, key, reason, "review-owner", Some(identity));
        }
        if matches!(obs.stage, MemberStage::Selected | MemberStage::Planning) {
            let operation = if obs.stage == MemberStage::Selected {
                Op::AuthorArtifact
            } else {
                Op::RepairArtifact
            };
            return owner_action(
                operation,
                key,
                "native planning artifact obligation; artifact-scoped only",
                writer_slot_free,
            );
        }
        let remaining = !obs.remaining_obligations.is_empty();
        if remaining && terminal_for(obs.stage, binding.goal.unit_goal(remaining)) {
            return hold(
                Op::Wait,
                key,
                "the current native unit reached its milestone; selected successor obligations await fresh native observation",
                "native-source-owner",
                None,
            );
        }
        let goal = binding.goal;
        let operation = match obs.stage {
            MemberStage::Blocked => match obs.recovery {
                Some(RecoveryDisposition::ArtifactRepair) => Op::RepairArtifact,
                Some(RecoveryDisposition::CodingRepair) => Op::RepairCode,
                Some(RecoveryDisposition::InfrastructureRetry) => Op::RetryInfrastructure,
                Some(RecoveryDisposition::UnknownOutcome) => Op::Reconcile,
                Some(RecoveryDisposition::SemanticReplan) | None => Op::Replan,
            },
            MemberStage::Ready => Op::Code,
            MemberStage::Claimed | MemberStage::Executing | MemberStage::Verifying => Op::Wait,
            MemberStage::Candidate => Op::Verify,
            MemberStage::VerifiedCandidate if goal == CompletionGoal::VerifiedCandidate => {
                Op::Complete
            }
            MemberStage::VerifiedCandidate if goal == CompletionGoal::VerifiedPullRequest => {
                Op::Publish
            }
            MemberStage::VerifiedCandidate => Op::Integrate,
            MemberStage::VerifiedPullRequest if goal == CompletionGoal::VerifiedPullRequest => {
                Op::Complete
            }
            MemberStage::VerifiedPullRequest => Op::Integrate,
            MemberStage::Integrated if goal == CompletionGoal::IntegratedChange => Op::Complete,
            MemberStage::Integrated | MemberStage::Lifecycle => Op::Lifecycle,
            MemberStage::Selected
            | MemberStage::Planning
            | MemberStage::Missing
            | MemberStage::Complete
            | MemberStage::Cancelled => {
                return deferred(
                    Op::Blocked,
                    key,
                    "the observed stage admits no campaign action; its obligations and blockers remain visible",
                    "campaign-policy",
                );
            }
        };
        if matches!(operation, Op::Publish | Op::Integrate)
            && goal != CompletionGoal::VerifiedCandidate
            && obs.delivery_route_supported != Some(true)
        {
            return hold(
                if obs.delivery_route_supported == Some(false) {
                    Op::Blocked
                } else {
                    Op::Wait
                },
                key,
                "selected repository delivery route is unsupported or not yet capability-observed; no fallback is permitted",
                "integration-capability-owner",
                None,
            );
        }
        let endpoint = matches!(
            operation,
            Op::Publish | Op::Integrate | Op::Lifecycle | Op::Complete
        );
        if endpoint
            && [
                obs.candidate_freshness,
                obs.policy_freshness,
                obs.verifier_freshness,
            ]
            .iter()
            .any(|freshness| *freshness != Freshness::Current)
        {
            return hold(
                Op::Wait,
                key,
                "selected endpoint lacks current candidate, policy or verification owner evidence",
                "verification-review-owner",
                None,
            );
        }
        if endpoint
            && ((!binding.required_checks.is_empty() && !obs.required_checks_satisfied)
                || (!binding.required_reviews.is_empty() && !obs.required_reviews_satisfied))
        {
            return hold(
                Op::Wait,
                key,
                "selected checks or required review lack current owner evidence",
                "verification-review-owner",
                None,
            );
        }
        if endpoint && !self.required_context_retained(binding, obs) {
            return hold(
                Op::Wait,
                key,
                "required decision projection was not retained for the exact selected work and attempt",
                "decision-context-owner",
                None,
            );
        }
        let reason = "next action derived from current owner observations and member endpoint";
        if operation.is_owner_effect() {
            owner_action(operation, key, reason, writer_slot_free)
        } else {
            hold(operation, key, reason, operation.owner(), None)
        }
    }
    // r[impl kiln.implementation.campaign.completion]
    fn member_endpoint_satisfied(
        &self,
        binding: &MemberBinding,
        observation: &MemberObservation,
    ) -> bool {
        observation.enumeration == EnumerationState::Complete
            && binding.goal.reached_by(observation.stage)
            && observation.remaining_obligations.is_empty()
            && observation.blockers.is_empty()
            && !observation.cancelled
            && (binding.required_checks.is_empty() || observation.required_checks_satisfied)
            && (binding.required_reviews.is_empty() || observation.required_reviews_satisfied)
            && [
                observation.candidate_freshness,
                observation.policy_freshness,
                observation.verifier_freshness,
            ]
            .iter()
            .all(|freshness| *freshness == Freshness::Current)
            && ![
                observation.context_freshness,
                observation.candidate_freshness,
                observation.policy_freshness,
                observation.verifier_freshness,
                observation.target_freshness,
            ]
            .contains(&Freshness::Stale)
            && (!binding.context.iter().any(|context| context.required)
                || (observation.context_identity.is_some()
                    && observation.context_freshness == Freshness::Current))
            && self.required_context_retained(binding, observation)
            && !self.decisions.values().any(|request| {
                request.member == observation.member
                    && request.kind == DecisionKind::RequiredReview
                    && request.state != DecisionRequestState::Resolved
            })
    }

    fn required_context_retained(
        &self,
        binding: &MemberBinding,
        observation: &MemberObservation,
    ) -> bool {
        binding
            .context
            .iter()
            .filter(|entry| entry.required)
            .all(|entry| {
                observation
                    .attempt_identity
                    .as_ref()
                    .and_then(|attempt| self.attempt_contexts.get(attempt))
                    .and_then(|contexts| contexts.get(&entry.projection_identity))
                    .is_some_and(|retained| {
                        retained.selection_identity == self.selection_identity
                            && Some(&retained.work_identity) == observation.work_identity.as_ref()
                    })
            })
    }

    fn dependency_satisfied(&self, member: &str) -> bool {
        let Some(observation) = self.observations.get(member) else {
            return false;
        };
        let Some(binding) = self
            .contract
            .members
            .iter()
            .find(|binding| selector_key(&binding.selector) == member)
        else {
            return false;
        };
        self.member_endpoint_satisfied(binding, observation)
    }
    pub fn complete(&self) -> bool {
        !self.cancelled
            && self
                .pending_effects
                .values()
                .all(|effect| effect.progress_applied)
            && self.contract.members.iter().all(|binding| {
                self.observations
                    .get(&selector_key(&binding.selector))
                    .is_some_and(|observation| {
                        self.member_endpoint_satisfied(binding, observation)
                            && observation
                                .dependencies
                                .iter()
                                .all(|dependency| self.dependency_satisfied(dependency))
                    })
            })
    }
}

fn terminal_for(stage: MemberStage, goal: CompletionGoal) -> bool {
    stage_satisfies(stage, goal)
}
fn stage_satisfies(stage: MemberStage, goal: CompletionGoal) -> bool {
    goal.reached_by(stage)
}
/// Canonical member key shared by admitted campaign owners and projections.
pub fn selector_key(selector: &MemberSelector) -> String {
    match selector {
        MemberSelector::WholeChange { change } => format!("{change}/*"),
        MemberSelector::Tasks { change, tasks } => format!("{change}/{}", tasks.join(",")),
    }
}
fn member_key(value: &str) -> String {
    value.to_string()
}
fn selector_valid(selector: &MemberSelector) -> bool {
    match selector {
        MemberSelector::WholeChange { change } => valid(change),
        MemberSelector::Tasks { change, tasks } => {
            valid(change)
                && !tasks.is_empty()
                && tasks.len() <= MAX_MEMBERS
                && tasks.iter().all(|t| valid(t))
                && tasks.iter().collect::<BTreeSet<_>>().len() == tasks.len()
        }
    }
}
fn valid(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_ID_BYTES && !value.chars().any(char::is_control)
}
fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}
fn decision(
    operation: CampaignOperation,
    member: Option<String>,
    reason: &str,
    owner: Option<String>,
    effect_intent: Option<ContentId>,
) -> CampaignDecision {
    CampaignDecision {
        operation,
        member,
        reason: reason.to_string(),
        owner,
        effect_intent,
    }
}
fn hold(
    operation: CampaignOperation,
    member: &str,
    reason: &str,
    owner: &str,
    effect_intent: Option<ContentId>,
) -> MemberEvaluation {
    MemberEvaluation::Hold(decision(
        operation,
        Some(member.to_string()),
        reason,
        Some(owner.to_string()),
        effect_intent,
    ))
}
fn deferred(
    operation: CampaignOperation,
    member: &str,
    reason: &str,
    owner: &str,
) -> MemberEvaluation {
    MemberEvaluation::Deferred(decision(
        operation,
        Some(member.to_string()),
        reason,
        Some(owner.to_string()),
        None,
    ))
}
/// An owner effect for one member. A writer claim waits while another active
/// attempt holds the repository's single writer slot; canonical admission
/// still makes the final claim decision.
fn owner_action(
    operation: CampaignOperation,
    member: &str,
    reason: &str,
    writer_slot_free: bool,
) -> MemberEvaluation {
    if operation.requires_writer_claim() && !writer_slot_free {
        return deferred(
            CampaignOperation::Wait,
            member,
            "another active attempt holds this repository's single writer slot; canonical admission still decides the claim",
            "work-attempt-owner",
        );
    }
    MemberEvaluation::Action(decision(
        operation,
        Some(member.to_string()),
        reason,
        Some(operation.owner().to_string()),
        None,
    ))
}
/// Why an unresolved required review keeps its member waiting.
fn review_wait(state: &DecisionRequestState) -> Option<&'static str> {
    match state {
        DecisionRequestState::Unissued => {
            Some("required review demand is retained but awaits an available shared review slot")
        }
        DecisionRequestState::Outstanding => {
            Some("required review response is outstanding; no successor may bypass it")
        }
        DecisionRequestState::Expired => {
            Some("required review expired unresolved; explicit review-owner action is required")
        }
        DecisionRequestState::Resolved => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    fn id() -> ContentId {
        ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|_| unreachable!())
    }
    fn contract() -> CampaignContract {
        CampaignContract {
            schema: CAMPAIGN_SCHEMA.into(),
            campaign_id: "campaign-1".into(),
            revision: 1,
            repository: "repo-a".into(),
            members: vec![
                MemberBinding {
                    selector: MemberSelector::Tasks {
                        change: "change-a".into(),
                        tasks: vec!["A".into()],
                    },
                    profile: WorkflowProfile::SpecDriven,
                    profile_version: id(),
                    repository_instructions: id(),
                    policy: id(),
                    required_checks: vec!["test".into()],
                    required_reviews: vec!["review".into()],
                    integration_policy: id(),
                    goal: CompletionGoal::NativeLifecycleComplete,
                    prerequisite_inclusion: false,
                    context: Vec::new(),
                },
                MemberBinding {
                    selector: MemberSelector::Tasks {
                        change: "change-a".into(),
                        tasks: vec!["B".into()],
                    },
                    profile: WorkflowProfile::Assurance,
                    profile_version: id(),
                    repository_instructions: id(),
                    policy: id(),
                    required_checks: Vec::new(),
                    required_reviews: Vec::new(),
                    integration_policy: id(),
                    goal: CompletionGoal::VerifiedCandidate,
                    prerequisite_inclusion: false,
                    context: Vec::new(),
                },
                MemberBinding {
                    selector: MemberSelector::WholeChange {
                        change: "change-c".into(),
                    },
                    profile: WorkflowProfile::NoSpecDelta,
                    profile_version: id(),
                    repository_instructions: id(),
                    policy: id(),
                    required_checks: Vec::new(),
                    required_reviews: Vec::new(),
                    integration_policy: id(),
                    goal: CompletionGoal::VerifiedCandidate,
                    prerequisite_inclusion: false,
                    context: Vec::new(),
                },
            ],
            max_concurrency: 2,
            max_outstanding_decisions: 1,
            parent_budget_id: id(),
            agent_definition: id(),
            require_one_writer_per_repository: true,
        }
    }
    fn obs(member: &str, stage: MemberStage, dependencies: Vec<String>) -> MemberObservation {
        MemberObservation {
            member: member.into(),
            stage,
            enumeration: EnumerationState::Complete,
            source_identity: Some(id()),
            policy_identity: id(),
            subject_identity: Some(id()),
            integration_target_identity: None,
            profile_identity: id(),
            dependencies,
            remaining_obligations: Default::default(),
            dependency_reason: None,
            canonical_work_key: Some(id()),
            work_identity: Some(id()),
            attempt_identity: None,
            attempt_fence: None,
            context_identity: None,
            native_context_identity: None,
            context_freshness: Freshness::NotApplicable,
            candidate_freshness: Freshness::Current,
            policy_freshness: Freshness::Current,
            verifier_freshness: if matches!(
                stage,
                MemberStage::VerifiedCandidate
                    | MemberStage::VerifiedPullRequest
                    | MemberStage::Integrated
                    | MemberStage::Lifecycle
                    | MemberStage::Complete
            ) {
                Freshness::Current
            } else {
                Freshness::Unknown
            },
            target_freshness: Freshness::Unknown,
            evidence: Vec::new(),
            blockers: Vec::new(),
            required_checks_satisfied: true,
            required_reviews_satisfied: true,
            delivery_route_supported: None,
            recovery: None,
            consumed_budget: 0,
            reserved_budget: 0,
            unknown_budget: 0,
            cancelled: false,
        }
    }
    fn fixture_member(contract: &CampaignContract, index: usize) -> &MemberBinding {
        contract
            .members
            .get(index)
            .unwrap_or_else(|| unreachable!("fixture member {index}"))
    }
    fn fixture_member_mut(contract: &mut CampaignContract, index: usize) -> &mut MemberBinding {
        contract
            .members
            .get_mut(index)
            .unwrap_or_else(|| unreachable!("fixture member {index}"))
    }
    fn key_at(state: &CampaignState, index: usize) -> String {
        selector_key(&fixture_member(&state.contract, index).selector)
    }
    fn observed<'a>(state: &'a CampaignState, key: &str) -> &'a MemberObservation {
        state
            .observations
            .get(key)
            .unwrap_or_else(|| unreachable!("observation {key}"))
    }
    fn first_impact(state: &CampaignState) -> &ImpactObservation {
        state
            .impacts
            .first()
            .unwrap_or_else(|| unreachable!("campaign impact"))
    }
    fn context_binding(state: &CampaignState, ordinal: usize) -> &DecisionContextBinding {
        fixture_member(&state.contract, 0)
            .context
            .get(ordinal)
            .unwrap_or_else(|| unreachable!("context binding {ordinal}"))
    }
    fn retained_contexts(state: &CampaignState) -> &BTreeMap<ContentId, RetainedAttemptContext> {
        state
            .attempt_contexts
            .get(&id())
            .unwrap_or_else(|| unreachable!("retained attempt contexts"))
    }
    #[test]
    fn first_native_observation_must_match_selected_profile_and_policy() {
        let mut state = CampaignState::start(contract()).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 0);
        let profile = fixture_member(&state.contract, 0).profile_version.clone();
        let policy = fixture_member(&state.contract, 0).policy.clone();
        let wrong = ContentId::from_blake3_hex("b".repeat(64)).unwrap_or_else(|_| unreachable!());
        for (observed_profile, observed_policy) in
            [(wrong.clone(), policy.clone()), (profile.clone(), wrong)]
        {
            let mut observation = obs(&key, MemberStage::Ready, Vec::new());
            observation.profile_identity = observed_profile;
            observation.policy_identity = observed_policy;
            let error = state
                .observe(observation)
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code, "campaign_native_binding_mismatch");
            assert!(!state.observations.contains_key(&key));
        }
        state
            .observe(obs(&key, MemberStage::Ready, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        let retained = state
            .observations
            .get(&key)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(retained.profile_identity, profile);
        assert_eq!(retained.policy_identity, policy);
    }
    #[test]
    fn partial_obligations_prevent_empty_success() {
        let mut state = CampaignState::start(contract()).unwrap_or_else(|_| unreachable!());
        let a = key_at(&state, 0);
        state
            .observe(obs(&a, MemberStage::Blocked, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        assert!(!state.complete());
    }
    #[test]
    // r[verify kiln.implementation.campaign.controller.scenario.dependencies]
    fn independent_member_can_progress_when_dependency_waits() {
        let mut state = CampaignState::start(contract()).unwrap_or_else(|_| unreachable!());
        let a = key_at(&state, 0);
        let b = key_at(&state, 1);
        let c = key_at(&state, 2);
        state
            .observe(obs(&a, MemberStage::Blocked, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        let mut dependent = obs(&b, MemberStage::Ready, vec![a]);
        dependent.enumeration = EnumerationState::Complete;
        state.observe(dependent).unwrap_or_else(|_| unreachable!());
        state
            .observe(obs(&c, MemberStage::Ready, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        let action = state.next_action().unwrap_or_else(|_| unreachable!());
        assert_eq!(action.member.as_deref(), Some(c.as_str()));
        assert_eq!(action.operation, CampaignOperation::Code);
    }
    #[test]
    // r[verify kiln.implementation.campaign.context.scenario.drift]
    fn decision_context_drift_is_not_candidate_drift() {
        let mut state = CampaignState::start(contract()).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 1);
        state
            .observe(obs(&key, MemberStage::Ready, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        let mut changed = obs(&key, MemberStage::Ready, Vec::new());
        changed.context_identity = Some(id());
        state.observe(changed).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            observed(&state, &key).candidate_freshness,
            Freshness::Current
        );
        assert_eq!(observed(&state, &key).stage, MemberStage::Blocked);
        let impact = first_impact(&state);
        assert_eq!(impact.context_before, None);
        assert_eq!(impact.context_after, Some(id()));
        assert_eq!(impact.action, CampaignOperation::Replan);
    }
    #[test]
    fn cancellation_closes_successor_admission() {
        let mut state = CampaignState::start(contract()).unwrap_or_else(|_| unreachable!());
        state.cancel();
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::Blocked
        );
    }
    #[test]
    // r[verify kiln.implementation.campaign.scope.scenario.partial]
    fn incomplete_enumeration_and_missing_member_never_complete() {
        let mut c = contract();
        c.members.truncate(1);
        let mut state = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 0);
        let mut observation = obs(&key, MemberStage::Ready, Vec::new());
        observation.enumeration = EnumerationState::IncompleteLimit;
        state
            .observe(observation)
            .unwrap_or_else(|_| unreachable!());
        assert!(!state.complete());
        assert_ne!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::Code
        );
    }
    #[test]
    // r[verify kiln.implementation.campaign.completion.scenario.endpoint]
    fn verified_pr_and_lifecycle_goals_are_distinct() {
        let mut c = contract();
        c.members.truncate(1);
        fixture_member_mut(&mut c, 0).goal = CompletionGoal::VerifiedPullRequest;
        let mut state = CampaignState::start(c.clone()).unwrap_or_else(|_| unreachable!());
        let key = selector_key(&fixture_member(&c, 0).selector);
        state
            .observe(obs(&key, MemberStage::VerifiedPullRequest, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        assert!(state.complete());
        fixture_member_mut(&mut c, 0).goal = CompletionGoal::NativeLifecycleComplete;
        let mut lifecycle = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
        lifecycle
            .observe(obs(&key, MemberStage::VerifiedPullRequest, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        assert!(!lifecycle.complete());
    }
    #[test]
    fn target_movement_records_replan_impact_separately() {
        let mut c = contract();
        c.members.truncate(1);
        let mut state = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 0);
        state
            .observe(obs(&key, MemberStage::Ready, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        let mut moved = obs(&key, MemberStage::Ready, Vec::new());
        moved.integration_target_identity = Some(id());
        state.observe(moved).unwrap_or_else(|_| unreachable!());
        assert_eq!(state.impacts.len(), 1);
        assert_eq!(first_impact(&state).target_identity_after, Some(id()));
        assert_eq!(first_impact(&state).action, CampaignOperation::Replan);
    }
    #[test]
    fn explicit_recovery_class_is_not_conflated() {
        let mut c = contract();
        c.members.truncate(1);
        let mut state = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 0);
        let mut blocked = obs(&key, MemberStage::Blocked, Vec::new());
        blocked.recovery = Some(RecoveryDisposition::InfrastructureRetry);
        state.observe(blocked).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::RetryInfrastructure
        );
    }
    #[test]
    fn required_review_waits_across_outstanding_and_expired_states() {
        let mut c = contract();
        c.members.truncate(1);
        let mut state = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 0);
        state
            .observe(obs(&key, MemberStage::Candidate, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        let request = HumanDecisionRequest {
            identity: id(),
            kind: DecisionKind::RequiredReview,
            member: key.clone(),
            work_identity: observed(&state, &key).work_identity.clone(),
            decision_revision: None,
            policy_identity: observed(&state, &key).policy_identity.clone(),
            candidate_identity: None,
            state: DecisionRequestState::Outstanding,
            created_ms: 1,
            question: "review candidate".into(),
            options: vec!["approve".into(), "reject".into()],
            recommendation: None,
            priority: 1,
            pending_reason: None,
            resolved_answer: None,
            active_effort_ms: None,
        };
        let identity = request.identity.clone();
        state
            .add_decision(request)
            .unwrap_or_else(|_| unreachable!());
        let decision = state.next_action().unwrap_or_else(|_| unreachable!());
        assert_eq!(decision.operation, CampaignOperation::Wait);
        assert_eq!(decision.effect_intent, Some(identity.clone()));
        state
            .decisions
            .get_mut(&identity)
            .unwrap_or_else(|| unreachable!())
            .state = DecisionRequestState::Expired;
        let decision = state.next_action().unwrap_or_else(|_| unreachable!());
        assert_eq!(decision.operation, CampaignOperation::Wait);
        assert!(decision.reason.contains("expired unresolved"));
    }
    #[test]
    fn one_decision_identity_cannot_alias_another_scope() {
        let mut c = contract();
        c.members.truncate(1);
        let mut state = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 0);
        state
            .observe(obs(&key, MemberStage::Ready, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        let request = HumanDecisionRequest {
            identity: id(),
            kind: DecisionKind::Preference,
            member: key.clone(),
            work_identity: observed(&state, &key).work_identity.clone(),
            decision_revision: None,
            policy_identity: observed(&state, &key).policy_identity.clone(),
            candidate_identity: None,
            state: DecisionRequestState::Outstanding,
            created_ms: 1,
            question: "choose".into(),
            options: vec!["a".into(), "b".into()],
            recommendation: None,
            priority: 1,
            pending_reason: None,
            resolved_answer: None,
            active_effort_ms: None,
        };
        state
            .add_decision(request.clone())
            .unwrap_or_else(|_| unreachable!());
        let mut conflicting = request;
        conflicting.question = "different question".into();
        assert!(state.add_decision(conflicting).is_err());
        assert_eq!(state.decisions.len(), 1);
    }
    #[test]
    fn native_unit_obligations_outlive_current_milestone() {
        let mut c = contract();
        c.members.truncate(1);
        let mut state = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 0);
        let mut current = obs(&key, MemberStage::Integrated, Vec::new());
        current.verifier_freshness = Freshness::Current;
        current
            .remaining_obligations
            .insert("change-a/tasks/B".into());
        state
            .observe(current.clone())
            .unwrap_or_else(|_| unreachable!());
        assert!(!state.complete());
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::Wait
        );
        current.remaining_obligations.clear();
        state
            .observe(current.clone())
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::Lifecycle
        );
        current.stage = MemberStage::Complete;
        state.observe(current).unwrap_or_else(|_| unreachable!());
        assert!(state.complete());
    }

    #[test]
    fn endpoint_requires_current_owner_evidence() {
        for (goal, stage) in [
            (
                CompletionGoal::VerifiedCandidate,
                MemberStage::VerifiedCandidate,
            ),
            (
                CompletionGoal::VerifiedPullRequest,
                MemberStage::VerifiedPullRequest,
            ),
            (CompletionGoal::IntegratedChange, MemberStage::Integrated),
            (
                CompletionGoal::NativeLifecycleComplete,
                MemberStage::Complete,
            ),
        ] {
            let mut c = contract();
            c.members.truncate(1);
            fixture_member_mut(&mut c, 0).goal = goal;
            let mut state = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
            let key = key_at(&state, 0);
            let mut current = obs(&key, stage, Vec::new());
            current.verifier_freshness = Freshness::Current;
            current.target_freshness = Freshness::Current;
            state.observe(current).unwrap_or_else(|_| unreachable!());
            assert!(state.complete());
            for field in ["candidate", "policy", "verifier", "checks", "reviews"] {
                let mut missing = state.clone();
                let member = missing
                    .observations
                    .get_mut(&key)
                    .unwrap_or_else(|| unreachable!());
                match field {
                    "candidate" => member.candidate_freshness = Freshness::Unknown,
                    "policy" => member.policy_freshness = Freshness::Unknown,
                    "verifier" => member.verifier_freshness = Freshness::Unknown,
                    "checks" => member.required_checks_satisfied = false,
                    "reviews" => member.required_reviews_satisfied = false,
                    _ => unreachable!(),
                }
                assert!(
                    !missing.complete(),
                    "{goal:?} accepted missing {field} evidence"
                );
                assert_ne!(
                    missing
                        .next_action()
                        .unwrap_or_else(|_| unreachable!())
                        .operation,
                    CampaignOperation::Complete
                );
            }
        }
    }

    #[test]
    // r[verify kiln.implementation.campaign.context.scenario.backpressure]
    fn paused_campaign_holds_unissued_decisions() {
        let mut c = contract();
        c.members.truncate(1);
        let mut state = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 0);
        state
            .observe(obs(&key, MemberStage::Ready, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        state
            .add_decision(HumanDecisionRequest {
                identity: id(),
                kind: DecisionKind::Preference,
                member: key,
                work_identity: Some(id()),
                decision_revision: None,
                policy_identity: id(),
                candidate_identity: None,
                state: DecisionRequestState::Unissued,
                created_ms: 1,
                question: "choose".into(),
                options: vec!["a".into(), "b".into()],
                recommendation: None,
                priority: 1,
                pending_reason: None,
                resolved_answer: None,
                active_effort_ms: None,
            })
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::RequestDecision
        );
        state.pause();
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::Wait
        );
        assert!(state.issue_decision(&id()).is_err());
        state.continue_admission();
        state
            .issue_decision(&id())
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            state
                .decisions
                .get(&id())
                .unwrap_or_else(|| unreachable!("issued decision"))
                .state,
            DecisionRequestState::Outstanding
        );
        state
            .decisions
            .get_mut(&id())
            .unwrap_or_else(|| unreachable!())
            .state = DecisionRequestState::Unissued;
        state.cancel();
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::Blocked
        );
        assert!(state.issue_decision(&id()).is_err());
    }

    #[test]
    fn active_repository_writer_blocks_independent_admission() {
        let mut state = CampaignState::start(contract()).unwrap_or_else(|_| unreachable!());
        let ready = key_at(&state, 0);
        let active = key_at(&state, 1);
        state
            .observe(obs(&ready, MemberStage::Ready, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        state
            .observe(obs(&active, MemberStage::Executing, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::Wait
        );
        state
            .observations
            .get_mut(&active)
            .unwrap_or_else(|| unreachable!())
            .stage = MemberStage::Blocked;
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::Code
        );
    }
    #[test]
    fn required_projections_preserve_exact_bytes_and_revision_scope() {
        let manifests = [
            b"first native decision".to_vec(),
            b"second native decision".to_vec(),
        ];
        let mut c = contract();
        c.members.truncate(1);
        fixture_member_mut(&mut c, 0).goal = CompletionGoal::VerifiedCandidate;
        for (ordinal, bytes) in manifests.iter().enumerate() {
            fixture_member_mut(&mut c, 0)
                .context
                .push(DecisionContextBinding {
                    decision_id: format!("decision-{ordinal}"),
                    native_adr_id: None,
                    revision: format!("revision-{ordinal}"),
                    selected_alternative: "chosen".into(),
                    evidence: Vec::new(),
                    supersedes: Vec::new(),
                    projection_identity: ContentId::from_blake3_hex(
                        blake3::hash(bytes).to_hex().to_string(),
                    )
                    .unwrap_or_else(|_| unreachable!()),
                    required: true,
                });
        }
        let mut state = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 0);
        let mut current = obs(&key, MemberStage::VerifiedCandidate, Vec::new());
        current.attempt_identity = Some(id());
        current.context_identity = Some(id());
        current.context_freshness = Freshness::Unknown;
        state
            .observe(current.clone())
            .unwrap_or_else(|_| unreachable!());
        assert!(!state.complete());
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::Blocked
        );
        current.context_freshness = Freshness::Current;
        state.observe(current).unwrap_or_else(|_| unreachable!());
        assert!(!state.complete());
        assert_eq!(
            state
                .next_action()
                .unwrap_or_else(|_| unreachable!())
                .operation,
            CampaignOperation::Wait
        );
        for (ordinal, bytes) in manifests.iter().enumerate() {
            let projection = context_binding(&state, ordinal).clone();
            let context = RetainedAttemptContext {
                attempt_identity: id(),
                work_identity: id(),
                selection_identity: state.selection_identity.clone(),
                context_identity: projection.projection_identity.clone(),
                decision_revisions: vec![projection.revision],
                exact_manifest: bytes.clone(),
            };
            let mut wrong_revision = context.clone();
            wrong_revision.decision_revisions = vec!["another-revision".into()];
            assert!(state.retain_attempt_context(wrong_revision).is_err());
            let mut changed_bytes = context.clone();
            changed_bytes.exact_manifest.push(b'!');
            assert!(state.retain_attempt_context(changed_bytes).is_err());
            state
                .retain_attempt_context(context.clone())
                .unwrap_or_else(|_| unreachable!());
            state
                .retain_attempt_context(context)
                .unwrap_or_else(|_| unreachable!());
            assert_eq!(
                &retained_contexts(&state)
                    .get(&projection.projection_identity)
                    .unwrap_or_else(|| unreachable!("retained projection {ordinal}"))
                    .exact_manifest,
                bytes
            );
        }
        assert!(state.complete());
    }

    #[test]
    fn aggregate_attempt_projection_bytes_are_bounded() {
        let first = vec![0; MAX_CONTEXT_MANIFEST_BYTES];
        let second = vec![1];
        let mut c = contract();
        c.members.truncate(1);
        for (ordinal, bytes) in [&first, &second].into_iter().enumerate() {
            fixture_member_mut(&mut c, 0)
                .context
                .push(DecisionContextBinding {
                    decision_id: format!("decision-{ordinal}"),
                    native_adr_id: None,
                    revision: format!("revision-{ordinal}"),
                    selected_alternative: "chosen".into(),
                    evidence: Vec::new(),
                    supersedes: Vec::new(),
                    projection_identity: ContentId::from_blake3_hex(
                        blake3::hash(bytes).to_hex().to_string(),
                    )
                    .unwrap_or_else(|_| unreachable!()),
                    required: false,
                });
        }
        let mut state = CampaignState::start(c).unwrap_or_else(|_| unreachable!());
        let key = key_at(&state, 0);
        let mut current = obs(&key, MemberStage::Ready, Vec::new());
        current.attempt_identity = Some(id());
        state.observe(current).unwrap_or_else(|_| unreachable!());
        for (ordinal, bytes) in [first, second].into_iter().enumerate() {
            let projection = context_binding(&state, ordinal);
            let context = RetainedAttemptContext {
                attempt_identity: id(),
                work_identity: id(),
                selection_identity: state.selection_identity.clone(),
                context_identity: projection.projection_identity.clone(),
                decision_revisions: vec![projection.revision.clone()],
                exact_manifest: bytes,
            };
            if ordinal == 0 {
                state
                    .retain_attempt_context(context)
                    .unwrap_or_else(|_| unreachable!());
            } else {
                assert!(state.retain_attempt_context(context).is_err());
            }
        }
        let projection = &context_binding(&state, 0).projection_identity;
        assert_eq!(
            retained_contexts(&state)
                .get(projection)
                .unwrap_or_else(|| unreachable!("retained first projection"))
                .exact_manifest,
            vec![0; MAX_CONTEXT_MANIFEST_BYTES]
        );
        assert!(
            !retained_contexts(&state)
                .contains_key(&context_binding(&state, 1).projection_identity)
        );
    }

    fn digest(byte: char) -> ContentId {
        ContentId::from_blake3_hex(byte.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }
    /// A needs a review before its verified-PR endpoint, B depends on A and C
    /// is independent. Names are parameters so selection order can be permuted.
    fn independence_contract(review: &str, dependent: &str, ready: &str) -> CampaignContract {
        let member = |change: &str, goal: CompletionGoal, reviews: Vec<String>| MemberBinding {
            selector: MemberSelector::WholeChange {
                change: change.into(),
            },
            profile: WorkflowProfile::SpecDriven,
            profile_version: id(),
            repository_instructions: id(),
            policy: id(),
            required_checks: Vec::new(),
            required_reviews: reviews,
            integration_policy: id(),
            goal,
            prerequisite_inclusion: false,
            context: Vec::new(),
        };
        let mut selected = contract();
        selected.members = vec![
            member(
                review,
                CompletionGoal::VerifiedPullRequest,
                vec!["maintainer".into()],
            ),
            member(dependent, CompletionGoal::VerifiedCandidate, Vec::new()),
            member(ready, CompletionGoal::VerifiedCandidate, Vec::new()),
        ];
        selected
    }
    /// A's verified candidate is otherwise ready to publish.
    fn verified_candidate(member: &str) -> MemberObservation {
        let mut verified = obs(member, MemberStage::VerifiedCandidate, Vec::new());
        verified.evidence = vec![digest('c')];
        verified.delivery_route_supported = Some(true);
        verified
    }
    fn review_request(state: &CampaignState, member: &str) -> HumanDecisionRequest {
        HumanDecisionRequest {
            identity: digest('e'),
            kind: DecisionKind::RequiredReview,
            member: member.into(),
            work_identity: observed(state, member).work_identity.clone(),
            decision_revision: None,
            policy_identity: observed(state, member).policy_identity.clone(),
            candidate_identity: Some(digest('c')),
            state: DecisionRequestState::Outstanding,
            created_ms: 1,
            question: "review the verified candidate".into(),
            options: vec!["approve".into(), "reject".into()],
            recommendation: None,
            priority: 1,
            pending_reason: None,
            resolved_answer: None,
            active_effort_ms: None,
        }
    }
    /// A awaits an outstanding required review, B depends on A and C is ready.
    /// Returns the A, B and C member keys.
    fn awaiting_review(review: &str, dependent: &str, ready: &str) -> (CampaignState, [String; 3]) {
        let mut state = CampaignState::start(independence_contract(review, dependent, ready))
            .unwrap_or_else(|_| unreachable!());
        let keys = [0, 1, 2].map(|index| key_at(&state, index));
        let [a, b, c] = &keys;
        let mut awaiting = verified_candidate(a);
        awaiting.required_reviews_satisfied = false;
        state.observe(awaiting).unwrap_or_else(|_| unreachable!());
        state
            .observe(obs(b, MemberStage::Ready, vec![a.clone()]))
            .unwrap_or_else(|_| unreachable!());
        state
            .observe(obs(c, MemberStage::Ready, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        let request = review_request(&state, a);
        state
            .add_decision(request)
            .unwrap_or_else(|_| unreachable!());
        (state, keys)
    }
    fn hold_of<'a>(evaluation: &'a CampaignEvaluation, member: &str) -> &'a CampaignDecision {
        evaluation
            .holds
            .iter()
            .find(|hold| hold.member.as_deref() == Some(member))
            .unwrap_or_else(|| unreachable!("hold for {member}"))
    }
    fn assert_selected(
        evaluation: &CampaignEvaluation,
        operation: CampaignOperation,
        member: &str,
    ) {
        assert_eq!(evaluation.basis, SelectionBasis::MemberAction);
        assert_eq!(evaluation.decision.operation, operation);
        assert_eq!(evaluation.decision.member.as_deref(), Some(member));
    }
    fn assert_review_wait(evaluation: &CampaignEvaluation, member: &str) {
        let review = hold_of(evaluation, member);
        assert_eq!(review.operation, CampaignOperation::Wait);
        assert_eq!(review.owner.as_deref(), Some("review-owner"));
        assert_eq!(review.effect_intent, Some(digest('e')));
        assert!(review.reason.contains("outstanding"), "{}", review.reason);
    }
    fn assert_dependency_wait(evaluation: &CampaignEvaluation, member: &str) {
        let dependent = hold_of(evaluation, member);
        assert_eq!(dependent.operation, CampaignOperation::Wait);
        assert!(
            dependent.reason.contains("dependency"),
            "{}",
            dependent.reason
        );
    }

    #[test]
    // r[verify kiln.implementation.campaign.independent_progress.scenario.review_wait]
    fn review_wait_stays_in_scope_while_independent_work_progresses() {
        let (state, [a, b, c]) = awaiting_review("change-a", "change-b", "change-c");
        let evaluation = state.evaluate();
        assert_selected(&evaluation, CampaignOperation::Code, &c);
        assert_review_wait(&evaluation, &a);
        assert_dependency_wait(&evaluation, &b);
        assert_eq!(evaluation.holds.len(), 2);
        assert_eq!(
            state.next_action().unwrap_or_else(|_| unreachable!()),
            evaluation.decision
        );
    }

    #[test]
    // r[verify kiln.implementation.campaign.independent_progress.scenario.permutation]
    fn member_identity_order_cannot_let_a_review_wait_suppress_eligible_work() {
        for [review, dependent, ready] in [
            ["change-a", "change-m", "change-z"],
            ["change-a", "change-z", "change-m"],
            ["change-m", "change-a", "change-z"],
            ["change-m", "change-z", "change-a"],
            ["change-z", "change-a", "change-m"],
            ["change-z", "change-m", "change-a"],
        ] {
            let (state, [a, b, c]) = awaiting_review(review, dependent, ready);
            let evaluation = state.evaluate();
            assert_selected(&evaluation, CampaignOperation::Code, &c);
            assert_review_wait(&evaluation, &a);
            assert_dependency_wait(&evaluation, &b);
        }
    }

    #[test]
    // r[verify kiln.implementation.campaign.independent_progress.scenario.local_scope]
    fn member_local_holds_stay_in_scope_unless_policy_makes_work_dependent() {
        type Adjust = fn(&mut MemberBinding, &mut MemberObservation);
        let cases: [(&str, Adjust, CampaignOperation, &str); 8] = [
            (
                "missing required context",
                |binding, _| {
                    binding.context.push(DecisionContextBinding {
                        decision_id: "decision".into(),
                        native_adr_id: None,
                        revision: "revision".into(),
                        selected_alternative: "chosen".into(),
                        evidence: Vec::new(),
                        supersedes: Vec::new(),
                        projection_identity: digest('d'),
                        required: true,
                    });
                },
                CampaignOperation::Blocked,
                "decision-context-owner",
            ),
            (
                "stale policy",
                |_, observation| observation.policy_freshness = Freshness::Stale,
                CampaignOperation::Replan,
                "campaign-policy",
            ),
            (
                "unsupported delivery route",
                |_, observation| observation.delivery_route_supported = Some(false),
                CampaignOperation::Blocked,
                "integration-capability-owner",
            ),
            (
                "unobserved delivery route",
                |_, observation| observation.delivery_route_supported = None,
                CampaignOperation::Wait,
                "integration-capability-owner",
            ),
            (
                "semantic replan",
                |_, observation| {
                    observation.stage = MemberStage::Blocked;
                    observation.recovery = Some(RecoveryDisposition::SemanticReplan);
                },
                CampaignOperation::Replan,
                "campaign-policy",
            ),
            (
                "successor milestone",
                |_, observation| {
                    observation.stage = MemberStage::VerifiedPullRequest;
                    observation
                        .remaining_obligations
                        .insert("change/tasks/next".into());
                },
                CampaignOperation::Wait,
                "native-source-owner",
            ),
            (
                "missing endpoint evidence",
                |_, observation| observation.candidate_freshness = Freshness::Unknown,
                CampaignOperation::Wait,
                "verification-review-owner",
            ),
            (
                "missing review evidence",
                |_, observation| observation.required_reviews_satisfied = false,
                CampaignOperation::Wait,
                "verification-review-owner",
            ),
        ];
        let start = |adjust: Option<Adjust>, ready_depends: bool| {
            let mut selected = independence_contract("change-a", "change-b", "change-c");
            let mut current = verified_candidate("change-a/*");
            if let Some(adjust) = adjust {
                adjust(fixture_member_mut(&mut selected, 0), &mut current);
            }
            let mut state = CampaignState::start(selected).unwrap_or_else(|_| unreachable!());
            state.observe(current).unwrap_or_else(|_| unreachable!());
            state
                .observe(obs(
                    "change-b/*",
                    MemberStage::Ready,
                    vec!["change-a/*".into()],
                ))
                .unwrap_or_else(|_| unreachable!());
            let dependencies = if ready_depends {
                vec!["change-a/*".into()]
            } else {
                Vec::new()
            };
            state
                .observe(obs("change-c/*", MemberStage::Ready, dependencies))
                .unwrap_or_else(|_| unreachable!());
            state
        };
        assert_selected(
            &start(None, false).evaluate(),
            CampaignOperation::Publish,
            "change-a/*",
        );
        for (label, adjust, operation, owner) in cases {
            let evaluation = start(Some(adjust), false).evaluate();
            assert_selected(&evaluation, CampaignOperation::Code, "change-c/*");
            let local = hold_of(&evaluation, "change-a/*");
            assert_eq!(local.operation, operation, "{label}");
            assert_eq!(local.owner.as_deref(), Some(owner), "{label}");
            assert_dependency_wait(&evaluation, "change-b/*");

            let dependent = start(Some(adjust), true).evaluate();
            assert_eq!(dependent.basis, SelectionBasis::MemberHold, "{label}");
            assert_eq!(&dependent.decision, local, "{label}");
            assert_dependency_wait(&dependent, "change-c/*");
        }
    }

    #[test]
    // r[verify kiln.implementation.campaign.independent_progress.scenario.review_gate]
    // r[verify kiln.implementation.campaign.bindings.scenario.review]
    fn required_review_holds_its_candidate_until_an_exact_answer_and_owner_evidence() {
        let mut selected = independence_contract("change-a", "change-b", "change-c");
        selected.members.truncate(1);
        let mut state = CampaignState::start(selected).unwrap_or_else(|_| unreachable!());
        let a = key_at(&state, 0);
        let mut awaiting = verified_candidate(&a);
        awaiting.required_reviews_satisfied = false;
        state
            .observe(awaiting.clone())
            .unwrap_or_else(|_| unreachable!());
        let request = review_request(&state, &a);
        state
            .add_decision(request)
            .unwrap_or_else(|_| unreachable!());
        let review = digest('e');
        let work = observed(&state, &a).work_identity.clone();
        let policy = observed(&state, &a).policy_identity.clone();
        let candidate = digest('c');
        let waiting = state.evaluate();
        assert_eq!(waiting.basis, SelectionBasis::MemberHold);
        assert_eq!(&waiting.decision, hold_of(&waiting, &a));
        assert_review_wait(&waiting, &a);

        for (work, policy, candidate, answer) in [
            (work.as_ref(), &policy, Some(&digest('f')), "approve"),
            (work.as_ref(), &digest('f'), Some(&candidate), "approve"),
            (Some(&digest('f')), &policy, Some(&candidate), "approve"),
            (work.as_ref(), &policy, Some(&candidate), "merge anyway"),
        ] {
            let refused = state
                .resolve_decision(&review, work, policy, candidate, answer)
                .err()
                .unwrap_or_else(|| unreachable!("wrong-scope answer accepted"));
            assert_eq!(refused.code, "campaign_decision_stale");
            assert_eq!(state.evaluate(), waiting);
        }

        let request = state
            .decisions
            .get_mut(&review)
            .unwrap_or_else(|| unreachable!("retained review"));
        request.state = DecisionRequestState::Expired;
        let expired = state.evaluate();
        assert_eq!(expired.decision.operation, CampaignOperation::Wait);
        assert!(expired.decision.reason.contains("expired unresolved"));
        assert!(
            state
                .resolve_decision(&review, work.as_ref(), &policy, Some(&candidate), "approve")
                .is_err()
        );

        state
            .decisions
            .get_mut(&review)
            .unwrap_or_else(|| unreachable!("retained review"))
            .state = DecisionRequestState::Outstanding;
        state
            .resolve_decision(&review, work.as_ref(), &policy, Some(&candidate), "approve")
            .unwrap_or_else(|_| unreachable!());
        let answered = state.evaluate();
        assert_eq!(answered.decision.operation, CampaignOperation::Wait);
        assert_eq!(
            answered.decision.owner.as_deref(),
            Some("verification-review-owner")
        );

        awaiting.required_reviews_satisfied = true;
        state.observe(awaiting).unwrap_or_else(|_| unreachable!());
        assert_selected(&state.evaluate(), CampaignOperation::Publish, &a);
    }

    #[test]
    // r[verify kiln.implementation.campaign.controls.scenario.decisions]
    fn an_answer_for_a_superseded_candidate_or_policy_is_refused() {
        let (mut state, [a, _, _]) = awaiting_review("change-a", "change-b", "change-c");
        let review = digest('e');
        let work = observed(&state, &a).work_identity.clone();
        let policy = observed(&state, &a).policy_identity.clone();
        let candidate = digest('c');
        let request = state
            .decisions
            .get(&review)
            .cloned()
            .unwrap_or_else(|| unreachable!("retained review"));
        assert!(state.decision_in_current_scope(&request));

        let mut successor = verified_candidate(&a);
        successor.required_reviews_satisfied = false;
        successor.evidence = vec![digest('d')];
        state
            .observe(successor.clone())
            .unwrap_or_else(|_| unreachable!());
        assert!(!state.decision_in_current_scope(&request));
        let refused = state
            .resolve_decision(&review, work.as_ref(), &policy, Some(&candidate), "approve")
            .err()
            .unwrap_or_else(|| unreachable!("answer for a superseded candidate accepted"));
        assert_eq!(refused.code, "campaign_decision_scope_changed");
        assert_eq!(
            state.decisions.get(&review).map(|request| &request.state),
            Some(&DecisionRequestState::Outstanding)
        );

        let mut drifted = state.clone();
        let mut repolicied = verified_candidate(&a);
        repolicied.required_reviews_satisfied = false;
        repolicied.policy_identity = digest('f');
        drifted
            .observe(repolicied)
            .unwrap_or_else(|_| unreachable!());
        successor.evidence = vec![candidate.clone()];
        assert_eq!(
            drifted
                .resolve_decision(&review, work.as_ref(), &policy, Some(&candidate), "approve")
                .err()
                .map(|refused| refused.code),
            Some("campaign_decision_scope_changed".into())
        );

        // The candidate under review is current again.
        state.observe(successor).unwrap_or_else(|_| unreachable!());
        state
            .resolve_decision(&review, work.as_ref(), &policy, Some(&candidate), "approve")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            state.decisions.get(&review).map(|request| &request.state),
            Some(&DecisionRequestState::Resolved)
        );
    }

    #[test]
    // r[verify kiln.implementation.campaign.independent_progress.scenario.barriers]
    fn writer_pause_cancel_and_uncertain_effects_still_constrain_independent_work() {
        let (state, [a, b, c]) = awaiting_review("change-a", "change-b", "change-c");
        let mut writer = state.clone();
        writer
            .observe(obs(&b, MemberStage::Executing, Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        let mut variants = Vec::new();
        for (stage, recovery) in [
            (MemberStage::Ready, None),
            (MemberStage::Selected, None),
            (
                MemberStage::Blocked,
                Some(RecoveryDisposition::CodingRepair),
            ),
            (
                MemberStage::Blocked,
                Some(RecoveryDisposition::InfrastructureRetry),
            ),
        ] {
            let mut variant = writer.clone();
            let mut current = obs(&c, stage, Vec::new());
            current.recovery = recovery;
            variant.observe(current).unwrap_or_else(|_| unreachable!());
            variants.push(variant);
        }
        for variant in variants {
            let evaluation = variant.evaluate();
            assert_eq!(evaluation.basis, SelectionBasis::MemberHold);
            assert_eq!(&evaluation.decision, hold_of(&evaluation, &a));
            assert!(hold_of(&evaluation, &c).reason.contains("writer slot"));
            let active = hold_of(&evaluation, &b);
            assert_eq!(active.operation, CampaignOperation::Wait);
            assert_eq!(active.owner.as_deref(), Some("campaign-policy"));
        }

        let mut paused = state.clone();
        paused.pause();
        let evaluation = paused.evaluate();
        assert_eq!(evaluation.basis, SelectionBasis::CampaignBarrier);
        assert_eq!(evaluation.decision.operation, CampaignOperation::Wait);
        assert_eq!(evaluation.decision.member, None);
        assert_review_wait(&evaluation, &a);

        let mut cancelled = state.clone();
        cancelled.cancel();
        let evaluation = cancelled.evaluate();
        assert_eq!(evaluation.basis, SelectionBasis::CampaignBarrier);
        assert_eq!(evaluation.decision.operation, CampaignOperation::Blocked);

        let mut uncertain = state;
        let current = observed(&uncertain, &c).clone();
        uncertain.pending_effects.insert(
            digest('b'),
            PendingEffect {
                effect_id: digest('b'),
                member: c.clone(),
                operation: CampaignOperation::Code,
                owner: "work-attempt-owner".into(),
                intent_identity: digest('b'),
                state: EffectState::Unknown,
                progress_applied: false,
                selection_identity: uncertain.selection_identity.clone(),
                source_identity: current.source_identity,
                work_identity: current.work_identity,
                attempt_identity: current.attempt_identity,
                attempt_fence: current.attempt_fence,
                subject_identity: current.subject_identity,
                policy_identity: current.policy_identity,
                context_identity: current.context_identity,
                native_context_identity: current.native_context_identity,
                profile_identity: current.profile_identity,
                evidence_identity: None,
                integration_target_identity: current.integration_target_identity,
            },
        );
        let evaluation = uncertain.evaluate();
        assert_eq!(evaluation.basis, SelectionBasis::CampaignBarrier);
        assert_eq!(evaluation.decision.operation, CampaignOperation::Reconcile);
        assert_eq!(evaluation.decision.member.as_deref(), Some(c.as_str()));
        assert_review_wait(&evaluation, &a);
    }
}
