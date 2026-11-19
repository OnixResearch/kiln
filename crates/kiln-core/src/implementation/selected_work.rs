//! Provider-neutral facts from one admitted selected-work snapshot, and the
//! per-attempt contracts, transitions and completion decisions Kiln owns.
//!
//! An adapter admits the producer record (the producer's own admission decides
//! producer validity) and maps it into these types. Nothing here parses
//! guidance prose, executes actions or grants authority. Lifecycle and
//! obligation meaning stay with the producer; Kiln decides attempt binding,
//! refresh or replan, and whether its own canonical checks and authoritative
//! completion discharge the same obligation identifiers.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::types::{ContentId, CoreResult, Diagnostic};

pub const SELECTED_WORK_CONTRACT_SCHEMA: &str = "kiln.selected-work-contract.v1";
pub const OBLIGATION_GUIDANCE_SCHEMA: &str = "kiln.obligation-guidance.v1";
pub const SELECTED_WORK_DISPLAY_SCHEMA: &str = "kiln.selected-work-display.v1";
/// Retained superseded contracts and transitions per campaign member.
pub const MAX_SELECTED_WORK_HISTORY: usize = 64;
/// Canonical check observations considered for one completion decision.
pub const MAX_CHECK_OBSERVATIONS: usize = 1_024;
/// Owner that resolves a stale or changed cohort: the campaign replans.
pub const REPLAN_OWNER: &str = "campaign-policy";
/// Owner of canonical checks and authoritative completion.
pub const VERIFICATION_OWNER: &str = "verification-owner";
/// Owner of producer lifecycle and obligation selection.
pub const LIFECYCLE_OWNER: &str = "cairn-lifecycle-owner";

/// How work entered Kiln. The legacy multi-projection reader stays available
/// only under its explicit compatibility profile.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntakeProfile {
    /// One versioned typed selected-work snapshot.
    TypedSnapshotV1,
    /// The pre-snapshot readiness/intake projections. Existing profiles that
    /// do not name an intake keep this profile; it never claims typed
    /// obligations.
    #[default]
    LegacyReadinessCompatibility,
}

/// The producer's eight cohort identities.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedWorkCohort {
    pub source: ContentId,
    pub intent: ContentId,
    pub context: ContentId,
    pub verification_subject: ContentId,
    pub dependencies: ContentId,
    pub obligations: ContentId,
    pub selection: ContentId,
    pub observation: ContentId,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CohortField {
    Source,
    Intent,
    Context,
    VerificationSubject,
    Dependencies,
    Obligations,
    Selection,
    Observation,
    Policy,
    Profile,
    Target,
}

impl CohortField {
    /// Fields whose change invalidates an attempt's contract rather than only
    /// its latest evaluation.
    #[must_use]
    pub const fn requires_replan(self) -> bool {
        !matches!(self, Self::Obligations | Self::Observation)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkSelection {
    pub change: String,
    pub task: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRevisionKind {
    Exact,
    Projection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRootKind {
    Lifecycle,
    Repository,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStatus {
    Present,
    Missing,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceArtifactFact {
    pub root: SourceRootKind,
    pub path: String,
    pub bytes: u64,
    pub content: ContentId,
    pub status: ArtifactStatus,
}

/// Exact native source/store revision and the selected artifact manifest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFacts {
    pub revision_kind: SourceRevisionKind,
    pub revision: String,
    pub artifacts: Vec<SourceArtifactFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactState {
    Ready,
    Blocked,
    Unavailable,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyFact {
    pub task: String,
    pub prerequisite: String,
    pub state: FactState,
    pub identity: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionInputs {
    pub state: FactState,
    pub contract: Option<ContentId>,
    pub results: Option<ContentId>,
    pub subject: Option<ContentId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextEntryFact {
    pub id: String,
    pub authority: String,
    pub source_root: Option<String>,
    pub path: Option<String>,
    pub status: FactState,
    pub content: Option<ContentId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextFacts {
    pub selection: ContentId,
    pub portable: ContentId,
    pub entries: Vec<ContextEntryFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageState {
    Ready,
    Blocked,
    Unavailable,
    NotApplicable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageReadiness {
    pub stage: String,
    pub state: StageState,
    pub decision: ContentId,
}

/// A producer blocker with its stable identifier; never reworded by Kiln.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableBlocker {
    pub id: String,
    pub stage: String,
    pub code: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessFacts {
    pub receipt: ContentId,
    pub input: ContentId,
    pub policy: ContentId,
    pub source: ContentId,
    pub evidence: ContentId,
    pub valid: bool,
    pub classification: String,
    pub stages: Vec<StageReadiness>,
    pub blockers: Vec<StableBlocker>,
    /// Advisory action identifiers only; they never grant execution.
    pub advisory_actions: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    /// A native `r[...]` requirement reference.
    Requirement,
    /// A native `a[...]` acceptance-criterion reference.
    Acceptance,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", content = "task", rename_all = "snake_case")]
pub enum ReferenceScope {
    Change,
    Task(String),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkReference {
    pub kind: ReferenceKind,
    pub id: String,
    pub scope: ReferenceScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitState {
    Ready,
    Blocked,
    Complete,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkUnitFact {
    pub task: Option<String>,
    pub state: UnitState,
    pub references: Vec<WorkReference>,
    pub verification_contracts: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemainingKind {
    Implementation,
    Completion,
    Sync,
    Archive,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemainingWorkFact {
    pub kind: RemainingKind,
    pub state: FactState,
    pub subject: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumerationFacts {
    pub complete: bool,
    pub observed: u32,
    pub total: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationNodeKind {
    Evidence,
    AllOf,
    AnyOf,
    Applicable,
    Temporal,
    Exception,
}

/// Producer selection state, ordered by restriction.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeSelection {
    Required,
    SelectedAlternative,
    Conditional,
    SelectionMissing,
    UnselectedAlternative,
}

impl NodeSelection {
    /// Only these states can contribute to completion.
    #[must_use]
    pub const fn contributes(self) -> bool {
        matches!(
            self,
            Self::Required | Self::SelectedAlternative | Self::Conditional
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Evidence,
    Applicability,
}

/// One exact native predicate reference supplied by the producer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRequirement {
    pub predicate: String,
    pub kind: EvidenceKind,
    pub requirement_id: String,
    pub owner: String,
    pub accepted_revision: ContentId,
    pub reference: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationNode {
    pub id: String,
    pub kind: ObligationNodeKind,
    pub parent: Option<String>,
    pub alternative: Option<String>,
    pub children: Vec<String>,
    pub selection: NodeSelection,
    pub conditions: Vec<String>,
    pub evidence: Vec<EvidenceRequirement>,
    pub obligation: ContentId,
    pub guidance: ContentId,
    pub condition: ContentId,
}

impl ObligationNode {
    /// A leaf that names native evidence and can contribute to completion.
    #[must_use]
    pub fn contributing_leaf(&self) -> bool {
        self.children.is_empty()
            && self.selection.contributes()
            && !self.evidence.is_empty()
            && matches!(
                self.kind,
                ObligationNodeKind::Evidence | ObligationNodeKind::Applicable
            )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationOutcome {
    Proved,
    ExceptionAccepted,
    Unsatisfied,
    Blocked,
    Abandoned,
    Unknown,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemainingCondition {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationEvaluation {
    pub outcome: ObligationOutcome,
    pub proved: bool,
    pub successful_delivery: bool,
    pub satisfied: Vec<String>,
    pub remaining: Vec<RemainingCondition>,
    pub report: ContentId,
}

/// The producer's exported obligation plan for one definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationPlan {
    pub workflow_id: String,
    pub workflow_version: u32,
    pub definition_id: String,
    pub binding: ContentId,
    pub intent: ContentId,
    pub workflow_policy: ContentId,
    pub predicate: ContentId,
    pub subject: ContentId,
    pub context: ContentId,
    pub selected_alternatives: BTreeMap<String, String>,
    pub nodes: Vec<ObligationNode>,
    pub observations_supplied: bool,
    pub evaluation: ObligationEvaluation,
}

impl ObligationPlan {
    #[must_use]
    pub fn node(&self, id: &str) -> Option<&ObligationNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "plan", rename_all = "snake_case")]
pub enum ObligationSection {
    /// No definition was selected; this is not an empty satisfied set.
    NotSelected,
    Exported(alloc::boxed::Box<ObligationPlan>),
}

/// One admitted complete trusted snapshot mapped into Kiln vocabulary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedSelectedWork {
    /// BLAKE3 over the exact admitted producer wire bytes.
    pub snapshot: ContentId,
    pub intake: IntakeProfile,
    pub producer_schema: String,
    pub selection: WorkSelection,
    pub cohort: SelectedWorkCohort,
    pub source: SourceFacts,
    pub policy: ContentId,
    pub profile: String,
    pub profile_identity: ContentId,
    pub completion: CompletionInputs,
    pub context: ContextFacts,
    pub readiness: ReadinessFacts,
    pub units: Vec<WorkUnitFact>,
    pub remaining: Vec<RemainingWorkFact>,
    pub dependencies: Vec<DependencyFact>,
    pub enumeration: EnumerationFacts,
    pub obligations: ObligationSection,
}

impl AdmittedSelectedWork {
    /// Every unit is complete and only completion, sync or archive remains.
    #[must_use]
    pub fn completion_only(&self) -> bool {
        self.units
            .iter()
            .all(|unit| unit.state == UnitState::Complete)
            && !self
                .remaining
                .iter()
                .any(|item| item.kind == RemainingKind::Implementation)
            && self
                .remaining
                .iter()
                .any(|item| item.kind == RemainingKind::Completion)
    }

    #[must_use]
    pub fn stage(&self, stage: &str) -> Option<&StageReadiness> {
        self.readiness
            .stages
            .iter()
            .find(|item| item.stage == stage)
    }

    #[must_use]
    pub fn references(&self) -> BTreeSet<&WorkReference> {
        self.units
            .iter()
            .flat_map(|unit| unit.references.iter())
            .collect()
    }

    #[must_use]
    pub fn plan(&self) -> Option<&ObligationPlan> {
        match &self.obligations {
            ObligationSection::NotSelected => None,
            ObligationSection::Exported(plan) => Some(plan),
        }
    }

    /// Structural checks Kiln relies on after the producer's admission.
    pub fn validate(&self) -> CoreResult<()> {
        if self.intake != IntakeProfile::TypedSnapshotV1
            || self.selection.change.is_empty()
            || !self.enumeration.complete
        {
            return Err(refusal(
                "selected_work_incomplete",
                "selected_work",
                "only a complete typed snapshot is an admitted selected-work record",
            ));
        }
        if let Some(plan) = self.plan()
            && (plan.subject != self.cohort.verification_subject
                || plan.context != self.cohort.context
                || plan.workflow_policy != self.policy)
        {
            return Err(refusal(
                "selected_work_crossed_subject",
                "selected_work.obligations",
                "obligation plan names another subject, context or policy",
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// One obligation projection for worker context, inspection and the UI.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BriefState {
    Satisfied,
    Remaining,
    /// Unselected alternatives and nodes outside the selected path.
    NotContributing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationBrief {
    pub id: String,
    pub kind: ObligationNodeKind,
    pub selection: NodeSelection,
    pub obligation: ContentId,
    pub guidance: ContentId,
    pub condition: ContentId,
    pub predicates: Vec<String>,
    pub evidence_owners: BTreeSet<String>,
    pub state: BriefState,
    pub remaining_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidanceAvailability {
    Exported,
    NotSelected,
}

/// Presentation-free obligation projection of one admitted snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationGuidanceProjection {
    pub schema: String,
    pub snapshot: ContentId,
    pub obligations_cohort: ContentId,
    pub verification_subject: ContentId,
    pub availability: GuidanceAvailability,
    pub definition: Option<String>,
    pub outcome: Option<ObligationOutcome>,
    pub successful_delivery: bool,
    pub obligations: Vec<ObligationBrief>,
    pub identity: ContentId,
}

/// The single projection every surface carries for one admitted snapshot.
// r[impl kiln.selected_work.completion.scenario.surfaces]
pub fn guidance_projection(
    snapshot: &AdmittedSelectedWork,
) -> CoreResult<ObligationGuidanceProjection> {
    let mut projection = ObligationGuidanceProjection {
        schema: OBLIGATION_GUIDANCE_SCHEMA.into(),
        snapshot: snapshot.snapshot.clone(),
        obligations_cohort: snapshot.cohort.obligations.clone(),
        verification_subject: snapshot.cohort.verification_subject.clone(),
        availability: GuidanceAvailability::NotSelected,
        definition: None,
        outcome: None,
        successful_delivery: false,
        obligations: Vec::new(),
        identity: snapshot.snapshot.clone(),
    };
    if let Some(plan) = snapshot.plan() {
        let satisfied: BTreeSet<&str> = plan
            .evaluation
            .satisfied
            .iter()
            .map(String::as_str)
            .collect();
        let remaining: BTreeMap<&str, &RemainingCondition> = plan
            .evaluation
            .remaining
            .iter()
            .map(|item| (item.id.as_str(), item))
            .collect();
        projection.availability = GuidanceAvailability::Exported;
        projection.definition = Some(plan.definition_id.clone());
        projection.outcome = Some(plan.evaluation.outcome);
        projection.successful_delivery = plan.evaluation.successful_delivery;
        projection.obligations = plan
            .nodes
            .iter()
            .map(|node| {
                let reason = remaining.get(node.id.as_str());
                let state = if !node.selection.contributes() {
                    BriefState::NotContributing
                } else if satisfied.contains(node.id.as_str()) && reason.is_none() {
                    BriefState::Satisfied
                } else {
                    BriefState::Remaining
                };
                ObligationBrief {
                    id: node.id.clone(),
                    kind: node.kind,
                    selection: node.selection,
                    obligation: node.obligation.clone(),
                    guidance: node.guidance.clone(),
                    condition: node.condition.clone(),
                    predicates: node
                        .evidence
                        .iter()
                        .map(|item| item.predicate.clone())
                        .collect(),
                    evidence_owners: node
                        .evidence
                        .iter()
                        .map(|item| item.owner.clone())
                        .collect(),
                    state,
                    remaining_reason: reason.map(|item| item.reason.clone()),
                }
            })
            .collect();
    }
    // The identity covers every field except itself.
    projection.identity = identity(
        OBLIGATION_GUIDANCE_SCHEMA,
        &(
            &projection.snapshot,
            &projection.obligations_cohort,
            &projection.verification_subject,
            projection.availability,
            &projection.definition,
            &projection.outcome,
            projection.successful_delivery,
            &projection.obligations,
        ),
    )?;
    Ok(projection)
}

// ---------------------------------------------------------------------------
// Per-attempt contracts and transitions.
// ---------------------------------------------------------------------------

/// The exact snapshot and guidance supplied to one attempt. Immutable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptSnapshotContract {
    pub schema: String,
    pub attempt: ContentId,
    pub work: ContentId,
    /// Kiln-owned integration target bound to the attempt, when resolved.
    pub target: Option<ContentId>,
    pub snapshot: AdmittedSelectedWork,
    pub guidance: ObligationGuidanceProjection,
    pub identity: ContentId,
}

// r[impl kiln.selected_work.attempts]
pub fn bind_attempt(
    attempt: ContentId,
    work: ContentId,
    target: Option<ContentId>,
    snapshot: AdmittedSelectedWork,
) -> CoreResult<AttemptSnapshotContract> {
    attempt.validate()?;
    work.validate()?;
    snapshot.validate()?;
    let guidance = guidance_projection(&snapshot)?;
    let identity = identity(
        SELECTED_WORK_CONTRACT_SCHEMA,
        &(&attempt, &work, &target, &snapshot, &guidance.identity),
    )?;
    Ok(AttemptSnapshotContract {
        schema: SELECTED_WORK_CONTRACT_SCHEMA.into(),
        attempt,
        work,
        target,
        snapshot,
        guidance,
        identity,
    })
}

impl AttemptSnapshotContract {
    /// Recompute the guidance and contract identity from retained bytes.
    pub fn verify(&self) -> CoreResult<()> {
        let guidance = guidance_projection(&self.snapshot)?;
        let identity = identity(
            SELECTED_WORK_CONTRACT_SCHEMA,
            &(
                &self.attempt,
                &self.work,
                &self.target,
                &self.snapshot,
                &guidance.identity,
            ),
        )?;
        if self.schema != SELECTED_WORK_CONTRACT_SCHEMA
            || guidance != self.guidance
            || identity != self.identity
        {
            return Err(refusal(
                "selected_work_contract_tampered",
                "selected_work.contract",
                "retained attempt contract does not match its exact snapshot",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    Unchanged,
    /// Only the evaluation or observation changed; the contract stands and a
    /// fresh evaluation is available.
    Refresh,
    /// The contract no longer describes current work; a new attempt contract
    /// must be admitted.
    Replan,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedWorkTransition {
    pub kind: TransitionKind,
    pub contract: ContentId,
    pub from_snapshot: ContentId,
    pub to_snapshot: ContentId,
    pub changed: Vec<CohortField>,
}

/// Classify a later observation against a bound contract. Pure: the contract
/// is not modified.
// r[impl kiln.selected_work.attempts.scenario.replan]
#[must_use]
pub fn classify_observation(
    contract: &AttemptSnapshotContract,
    observed: &AdmittedSelectedWork,
    observed_target: Option<&ContentId>,
) -> SelectedWorkTransition {
    let bound = &contract.snapshot;
    let mut changed = Vec::new();
    let pairs = [
        (
            CohortField::Source,
            &bound.cohort.source,
            &observed.cohort.source,
        ),
        (
            CohortField::Intent,
            &bound.cohort.intent,
            &observed.cohort.intent,
        ),
        (
            CohortField::Context,
            &bound.cohort.context,
            &observed.cohort.context,
        ),
        (
            CohortField::VerificationSubject,
            &bound.cohort.verification_subject,
            &observed.cohort.verification_subject,
        ),
        (
            CohortField::Dependencies,
            &bound.cohort.dependencies,
            &observed.cohort.dependencies,
        ),
        (
            CohortField::Obligations,
            &bound.cohort.obligations,
            &observed.cohort.obligations,
        ),
        (
            CohortField::Selection,
            &bound.cohort.selection,
            &observed.cohort.selection,
        ),
        (
            CohortField::Observation,
            &bound.cohort.observation,
            &observed.cohort.observation,
        ),
        (CohortField::Policy, &bound.policy, &observed.policy),
        (
            CohortField::Profile,
            &bound.profile_identity,
            &observed.profile_identity,
        ),
    ];
    for (field, before, after) in pairs {
        if before != after {
            changed.push(field);
        }
    }
    if contract.target.as_ref() != observed_target {
        changed.push(CohortField::Target);
    }
    let kind = if changed.is_empty() && bound.snapshot == observed.snapshot {
        TransitionKind::Unchanged
    } else if changed.iter().any(|field| field.requires_replan()) {
        TransitionKind::Replan
    } else {
        TransitionKind::Refresh
    };
    SelectedWorkTransition {
        kind,
        contract: contract.identity.clone(),
        from_snapshot: bound.snapshot.clone(),
        to_snapshot: observed.snapshot.clone(),
        changed,
    }
}

/// One member's latest selected-work read.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SelectedWorkObservation {
    NotObserved,
    /// The read failed, was refused or has an unknown outcome. This is never
    /// an empty obligation set.
    Unavailable {
        reason: String,
        refusal: Option<String>,
    },
    Admitted {
        snapshot: alloc::boxed::Box<AdmittedSelectedWork>,
        target: Option<ContentId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalContract {
    pub contract: AttemptSnapshotContract,
    pub superseded_by: ContentId,
    pub transition: TransitionKind,
}

/// Current versus historical contracts and the latest observation of one
/// campaign member.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedWorkHistory {
    pub intake: IntakeProfile,
    pub latest: SelectedWorkObservation,
    pub current: Option<AttemptSnapshotContract>,
    pub historical: Vec<HistoricalContract>,
    pub transitions: Vec<SelectedWorkTransition>,
}

impl Default for SelectedWorkHistory {
    fn default() -> Self {
        Self::new(IntakeProfile::TypedSnapshotV1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Currency {
    Current,
    Historical,
}

impl SelectedWorkHistory {
    #[must_use]
    pub const fn new(intake: IntakeProfile) -> Self {
        Self {
            intake,
            latest: SelectedWorkObservation::NotObserved,
            current: None,
            historical: Vec::new(),
            transitions: Vec::new(),
        }
    }

    /// Record a later observation. Returns the transition against the bound
    /// contract, if any; the bound contract is never modified.
    pub fn observe(
        &mut self,
        observation: SelectedWorkObservation,
    ) -> CoreResult<Option<SelectedWorkTransition>> {
        if self.intake != IntakeProfile::TypedSnapshotV1 {
            return Err(refusal(
                "selected_work_legacy_profile",
                "selected_work.intake",
                "the legacy compatibility profile does not record typed snapshots",
            ));
        }
        let transition = match (&observation, &self.current) {
            (SelectedWorkObservation::Admitted { snapshot, target }, Some(contract)) => {
                snapshot.validate()?;
                Some(classify_observation(contract, snapshot, target.as_ref()))
            }
            (SelectedWorkObservation::Admitted { snapshot, .. }, None) => {
                snapshot.validate()?;
                None
            }
            (SelectedWorkObservation::Unavailable { reason, .. }, _) if reason.is_empty() => {
                return Err(refusal(
                    "selected_work_unavailable_reason",
                    "selected_work.observation",
                    "an unavailable observation needs its reason",
                ));
            }
            _ => None,
        };
        if let Some(transition) = &transition
            && transition.kind != TransitionKind::Unchanged
        {
            self.push_transition(transition.clone())?;
        }
        self.latest = observation;
        Ok(transition)
    }

    /// Bind a new attempt to the latest admitted snapshot. An in-flight
    /// contract is never rewritten: rebinding needs a new attempt identity and
    /// moves the previous contract to history.
    pub fn bind(&mut self, contract: AttemptSnapshotContract) -> CoreResult<()> {
        contract.verify()?;
        let SelectedWorkObservation::Admitted { snapshot, target } = &self.latest else {
            return Err(refusal(
                "selected_work_unobserved",
                "selected_work.contract",
                "an attempt contract needs a current admitted snapshot",
            ));
        };
        if **snapshot != contract.snapshot || *target != contract.target {
            return Err(refusal(
                "selected_work_stale_contract",
                "selected_work.contract",
                "an attempt contract must bind the latest admitted snapshot and target",
            ));
        }
        if let Some(previous) = self.current.take() {
            if previous.attempt == contract.attempt {
                let identical = previous.identity == contract.identity;
                self.current = Some(previous);
                return if identical {
                    Ok(())
                } else {
                    Err(refusal(
                        "selected_work_in_flight",
                        "selected_work.contract",
                        "an in-flight attempt contract cannot be rewritten",
                    ))
                };
            }
            let transition = classify_observation(&previous, snapshot, target.as_ref()).kind;
            if self.historical.len() >= MAX_SELECTED_WORK_HISTORY {
                self.current = Some(previous);
                return Err(history_bound());
            }
            self.historical.push(HistoricalContract {
                contract: previous,
                superseded_by: contract.identity.clone(),
                transition,
            });
        }
        self.current = Some(contract);
        Ok(())
    }

    #[must_use]
    pub fn contract_for(
        &self,
        attempt: &ContentId,
    ) -> Option<(Currency, &AttemptSnapshotContract)> {
        if let Some(current) = self.current.as_ref().filter(|c| &c.attempt == attempt) {
            return Some((Currency::Current, current));
        }
        self.historical
            .iter()
            .rev()
            .find(|item| &item.contract.attempt == attempt)
            .map(|item| (Currency::Historical, &item.contract))
    }

    /// The one projection every operator and worker surface carries.
    pub fn surface(&self) -> CoreResult<SelectedWorkSurface> {
        let (observation, latest_snapshot, latest_guidance) = match &self.latest {
            SelectedWorkObservation::NotObserved => (SurfaceObservation::NotObserved, None, None),
            SelectedWorkObservation::Unavailable { .. } => {
                (SurfaceObservation::Unavailable, None, None)
            }
            SelectedWorkObservation::Admitted { snapshot, .. } => (
                SurfaceObservation::Admitted,
                Some(snapshot.snapshot.clone()),
                Some(guidance_projection(snapshot)?),
            ),
        };
        let unavailable_reason = match &self.latest {
            SelectedWorkObservation::Unavailable { reason, refusal } => {
                Some(refusal.clone().unwrap_or_else(|| reason.clone()))
            }
            _ => None,
        };
        Ok(SelectedWorkSurface {
            intake: self.intake,
            observation,
            unavailable_reason,
            latest_snapshot,
            current_contract: self.current.as_ref().map(|c| c.identity.clone()),
            current_attempt: self.current.as_ref().map(|c| c.attempt.clone()),
            guidance: self
                .current
                .as_ref()
                .map(|c| c.guidance.clone())
                .or(latest_guidance),
            pending_transition: self.transitions.last().cloned(),
            historical_contracts: self
                .historical
                .iter()
                .map(|item| item.contract.identity.clone())
                .collect(),
        })
    }

    fn push_transition(&mut self, transition: SelectedWorkTransition) -> CoreResult<()> {
        if self.transitions.len() >= MAX_SELECTED_WORK_HISTORY {
            return Err(history_bound());
        }
        self.transitions.push(transition);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceObservation {
    NotObserved,
    Unavailable,
    Admitted,
}

/// What worker context, campaign inspection and the UI show for one member.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedWorkSurface {
    pub intake: IntakeProfile,
    pub observation: SurfaceObservation,
    pub unavailable_reason: Option<String>,
    pub latest_snapshot: Option<ContentId>,
    pub current_contract: Option<ContentId>,
    pub current_attempt: Option<ContentId>,
    /// The bound contract's guidance, or the latest admitted snapshot's when
    /// no attempt is bound yet.
    pub guidance: Option<ObligationGuidanceProjection>,
    pub pending_transition: Option<SelectedWorkTransition>,
    pub historical_contracts: Vec<ContentId>,
}

// ---------------------------------------------------------------------------
// Completion over canonical checks and authoritative completion.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Passed,
    Failed,
    Skipped,
    Unknown,
    Interrupted,
}

/// One canonical check observation of one exact native predicate revision.
/// It discharges every obligation node naming that predicate, and failures
/// return to those same obligation identifiers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalCheckObservation {
    pub check: ContentId,
    pub predicate: String,
    pub requirement_id: String,
    pub revision: ContentId,
    pub subject: ContentId,
    pub context: ContentId,
    pub policy: ContentId,
    pub candidate: ContentId,
    pub state: CheckState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoritativeVerdict {
    Passed,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoritativeCompletion {
    pub subject: ContentId,
    pub policy: ContentId,
    pub candidate: ContentId,
    pub verdict: AuthoritativeVerdict,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapReason {
    ObligationsNotExported,
    /// The producer predicate changed under the same binding, such as a
    /// different selected alternative.
    ChangedSelection,
    SelectionMissing,
    ProducerNotDelivered,
    ProducerRemaining,
    MissingCheck,
    SkippedCheck,
    UnknownCheck,
    InterruptedCheck,
    FailedCheck,
    StaleSubject,
    StaleContext,
    StalePolicy,
    StaleCandidate,
    AuthoritativeMissing,
    AuthoritativeFailed,
    AuthoritativeUnknown,
}

/// A remaining condition with the existing operation owner that resolves it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationGap {
    pub obligation: String,
    pub reason: GapReason,
    pub owner: String,
    pub condition: Option<ContentId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ObligationCompletion {
    Complete {
        contract: ContentId,
        snapshot: ContentId,
        report: ContentId,
        checks: Vec<ContentId>,
    },
    Incomplete {
        contract: ContentId,
        gaps: Vec<ObligationGap>,
    },
}

impl ObligationCompletion {
    #[must_use]
    pub const fn complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }
}

/// Decide whether canonical checks and authoritative completion discharge the
/// contract's obligations, using the producer's evaluation of `evaluated`.
// r[impl kiln.selected_work.completion]
pub fn decide_obligation_completion(
    contract: &AttemptSnapshotContract,
    evaluated: &AdmittedSelectedWork,
    candidate: &ContentId,
    checks: &[CanonicalCheckObservation],
    authoritative: Option<&AuthoritativeCompletion>,
) -> CoreResult<ObligationCompletion> {
    contract.verify()?;
    if checks.len() > MAX_CHECK_OBSERVATIONS {
        return Err(refusal(
            "selected_work_check_bound",
            "selected_work.checks",
            "canonical check observations exceed their bound",
        ));
    }
    let bound = &contract.snapshot;
    let mut gaps = BTreeSet::new();
    let mut gap =
        |obligation: &str, reason: GapReason, owner: &str, condition: Option<&ContentId>| {
            gaps.insert(ObligationGap {
                obligation: obligation.to_string(),
                reason,
                owner: owner.to_string(),
                condition: condition.cloned(),
            });
        };
    let scope = bound
        .plan()
        .map_or(bound.selection.change.as_str(), |plan| {
            plan.definition_id.as_str()
        });
    if evaluated.selection != bound.selection {
        gap(scope, GapReason::StaleSubject, REPLAN_OWNER, None);
    }
    if evaluated.cohort.verification_subject != bound.cohort.verification_subject {
        gap(scope, GapReason::StaleSubject, REPLAN_OWNER, None);
    }
    if evaluated.cohort.context != bound.cohort.context {
        gap(scope, GapReason::StaleContext, REPLAN_OWNER, None);
    }
    if evaluated.policy != bound.policy || evaluated.profile_identity != bound.profile_identity {
        gap(scope, GapReason::StalePolicy, REPLAN_OWNER, None);
    }
    let checks_used = match (bound.plan(), evaluated.plan()) {
        (Some(bound_plan), Some(plan)) => {
            if bound_plan.binding != plan.binding {
                gap(scope, GapReason::StalePolicy, REPLAN_OWNER, None);
            } else if bound_plan.predicate != plan.predicate {
                gap(scope, GapReason::ChangedSelection, REPLAN_OWNER, None);
            }
            obligation_gaps(bound, plan, candidate, checks, &mut gap)
        }
        _ => {
            gap(
                scope,
                GapReason::ObligationsNotExported,
                LIFECYCLE_OWNER,
                None,
            );
            Vec::new()
        }
    };
    match authoritative {
        None => gap(
            scope,
            GapReason::AuthoritativeMissing,
            VERIFICATION_OWNER,
            None,
        ),
        Some(result) => {
            if result.subject != bound.cohort.verification_subject {
                gap(scope, GapReason::StaleSubject, VERIFICATION_OWNER, None);
            }
            if result.policy != bound.policy {
                gap(scope, GapReason::StalePolicy, VERIFICATION_OWNER, None);
            }
            if &result.candidate != candidate {
                gap(scope, GapReason::StaleCandidate, VERIFICATION_OWNER, None);
            }
            match result.verdict {
                AuthoritativeVerdict::Passed => {}
                AuthoritativeVerdict::Failed => {
                    gap(
                        scope,
                        GapReason::AuthoritativeFailed,
                        VERIFICATION_OWNER,
                        None,
                    );
                }
                AuthoritativeVerdict::Unknown => {
                    gap(
                        scope,
                        GapReason::AuthoritativeUnknown,
                        VERIFICATION_OWNER,
                        None,
                    );
                }
            }
        }
    }
    if gaps.is_empty()
        && let Some(plan) = evaluated.plan()
    {
        return Ok(ObligationCompletion::Complete {
            contract: contract.identity.clone(),
            snapshot: evaluated.snapshot.clone(),
            report: plan.evaluation.report.clone(),
            checks: checks_used,
        });
    }
    Ok(ObligationCompletion::Incomplete {
        contract: contract.identity.clone(),
        gaps: gaps.into_iter().collect(),
    })
}

fn obligation_gaps(
    bound: &AdmittedSelectedWork,
    plan: &ObligationPlan,
    candidate: &ContentId,
    checks: &[CanonicalCheckObservation],
    gap: &mut impl FnMut(&str, GapReason, &str, Option<&ContentId>),
) -> Vec<ContentId> {
    let evaluation = &plan.evaluation;
    if !evaluation.successful_delivery
        || !matches!(
            evaluation.outcome,
            ObligationOutcome::Proved | ObligationOutcome::ExceptionAccepted
        )
    {
        gap(
            &plan.definition_id,
            GapReason::ProducerNotDelivered,
            LIFECYCLE_OWNER,
            None,
        );
    }
    for remaining in &evaluation.remaining {
        let node = plan.node(&remaining.id);
        let owner = node
            .and_then(|node| node.evidence.first())
            .map_or(LIFECYCLE_OWNER, |item| item.owner.as_str());
        gap(
            &remaining.id,
            GapReason::ProducerRemaining,
            owner,
            node.map(|node| &node.condition),
        );
    }
    for node in plan
        .nodes
        .iter()
        .filter(|node| node.selection == NodeSelection::SelectionMissing)
    {
        gap(
            &node.id,
            GapReason::SelectionMissing,
            LIFECYCLE_OWNER,
            Some(&node.condition),
        );
    }
    // Kiln cross-checks every contributing leaf the producer counted as
    // satisfied, and explains the evidence state behind every remaining one.
    let considered: BTreeSet<&str> = evaluation
        .satisfied
        .iter()
        .map(String::as_str)
        .chain(evaluation.remaining.iter().map(|item| item.id.as_str()))
        .collect();
    let mut used = Vec::new();
    for node in plan
        .nodes
        .iter()
        .filter(|node| node.contributing_leaf() && considered.contains(node.id.as_str()))
    {
        for requirement in &node.evidence {
            let matching = checks.iter().filter(|check| {
                check.predicate == requirement.predicate
                    && check.requirement_id == requirement.requirement_id
                    && check.revision == requirement.accepted_revision
            });
            let mut stale = None;
            let mut best: Option<&CanonicalCheckObservation> = None;
            for check in matching {
                let reason = if check.subject != bound.cohort.verification_subject {
                    Some(GapReason::StaleSubject)
                } else if check.context != bound.cohort.context {
                    Some(GapReason::StaleContext)
                } else if check.policy != bound.policy {
                    Some(GapReason::StalePolicy)
                } else if &check.candidate != candidate {
                    Some(GapReason::StaleCandidate)
                } else {
                    None
                };
                match reason {
                    Some(reason) => stale = stale.or(Some(reason)),
                    None => {
                        let accepted = match requirement.kind {
                            EvidenceKind::Evidence => check.state == CheckState::Passed,
                            EvidenceKind::Applicability => {
                                matches!(check.state, CheckState::Passed | CheckState::Failed)
                            }
                        };
                        if accepted || best.is_none() {
                            best = Some(check);
                        }
                        if accepted {
                            break;
                        }
                    }
                }
            }
            let reason = match best.map(|check| (check, requirement.kind)) {
                Some((check, EvidenceKind::Evidence)) if check.state == CheckState::Passed => {
                    used.push(check.check.clone());
                    continue;
                }
                Some((check, EvidenceKind::Applicability))
                    if matches!(check.state, CheckState::Passed | CheckState::Failed) =>
                {
                    used.push(check.check.clone());
                    continue;
                }
                Some((check, _)) => match check.state {
                    CheckState::Failed => GapReason::FailedCheck,
                    CheckState::Skipped => GapReason::SkippedCheck,
                    CheckState::Interrupted => GapReason::InterruptedCheck,
                    CheckState::Unknown | CheckState::Passed => GapReason::UnknownCheck,
                },
                None => stale.unwrap_or(GapReason::MissingCheck),
            };
            gap(&node.id, reason, VERIFICATION_OWNER, Some(&node.condition));
        }
    }
    used.sort();
    used.dedup();
    used
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceObservationState {
    Satisfied,
    Unsatisfied,
    Unknown,
}

/// Producer-facing evidence derived from the same canonical checks, so the
/// producer evaluates exactly the evidence Kiln later cross-checks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationEvidenceObservation {
    pub predicate: String,
    pub requirement_id: String,
    pub revision: ContentId,
    pub subject: ContentId,
    pub context: ContentId,
    pub policy: ContentId,
    pub observation: ContentId,
    pub state: EvidenceObservationState,
}

/// Only checks bound to the contract's subject, context, policy and candidate
/// become producer evidence; skipped, unknown and interrupted checks stay
/// unknown rather than satisfied.
#[must_use]
pub fn evidence_observations(
    contract: &AttemptSnapshotContract,
    candidate: &ContentId,
    checks: &[CanonicalCheckObservation],
) -> Vec<ObligationEvidenceObservation> {
    let bound = &contract.snapshot;
    checks
        .iter()
        .filter(|check| {
            check.subject == bound.cohort.verification_subject
                && check.context == bound.cohort.context
                && check.policy == bound.policy
                && &check.candidate == candidate
        })
        .map(|check| ObligationEvidenceObservation {
            predicate: check.predicate.clone(),
            requirement_id: check.requirement_id.clone(),
            revision: check.revision.clone(),
            subject: check.subject.clone(),
            context: check.context.clone(),
            policy: check.policy.clone(),
            observation: check.check.clone(),
            state: match check.state {
                CheckState::Passed => EvidenceObservationState::Satisfied,
                CheckState::Failed => EvidenceObservationState::Unsatisfied,
                CheckState::Skipped | CheckState::Unknown | CheckState::Interrupted => {
                    EvidenceObservationState::Unknown
                }
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Member readiness from the admitted snapshot.
// ---------------------------------------------------------------------------

/// Native phases that must pass before implementation, in producer order.
pub const PLANNING_STAGES: [&str; 4] = ["artifact", "proposal", "design", "tasks"];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HoldReason {
    Unobserved,
    Unavailable,
    IncompleteEnumeration,
    PhaseBlocked,
    UnitBlocked,
    DependencyBlocked,
}

/// What the admitted snapshot says about selecting this member's work.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "readiness", rename_all = "snake_case")]
pub enum TypedReadiness {
    /// Every planning phase passes, the selected units are ready and their
    /// prerequisites are ready.
    Ready,
    /// No implementation remains; completion, sync or archive owners act.
    CompletionOnly,
    /// Never selectable; blockers keep the producer's stable identifiers.
    Held {
        reason: HoldReason,
        blockers: Vec<String>,
    },
}

impl TypedReadiness {
    #[must_use]
    pub const fn selectable(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// Derive member readiness only from the admitted snapshot's native phase
/// readiness, stable blockers, unit states and dependency facts. An
/// unobserved, unavailable or incompletely enumerated snapshot is held.
// r[impl kiln.selected_work.stages]
#[must_use]
pub fn typed_readiness(observation: &SelectedWorkObservation) -> TypedReadiness {
    let held = |reason, blockers: Vec<String>| TypedReadiness::Held { reason, blockers };
    let snapshot = match observation {
        SelectedWorkObservation::NotObserved => {
            return held(
                HoldReason::Unobserved,
                Vec::from(["selected_work.not_observed".into()]),
            );
        }
        SelectedWorkObservation::Unavailable { reason, refusal } => {
            return held(
                HoldReason::Unavailable,
                Vec::from([refusal.clone().unwrap_or_else(|| reason.clone())]),
            );
        }
        SelectedWorkObservation::Admitted { snapshot, .. } => snapshot,
    };
    if !snapshot.enumeration.complete {
        return held(
            HoldReason::IncompleteEnumeration,
            Vec::from(["selected_work.enumeration_incomplete".into()]),
        );
    }
    let implementation = snapshot
        .remaining
        .iter()
        .any(|item| item.kind == RemainingKind::Implementation);
    if !implementation {
        return TypedReadiness::CompletionOnly;
    }
    let mut phase_blockers = Vec::new();
    for name in PLANNING_STAGES {
        let state = snapshot.stage(name).map(|stage| stage.state);
        if matches!(state, Some(StageState::Ready | StageState::NotApplicable)) {
            continue;
        }
        let stable: Vec<String> = snapshot
            .readiness
            .blockers
            .iter()
            .filter(|blocker| blocker.stage == name)
            .map(|blocker| blocker.id.clone())
            .collect();
        if stable.is_empty() {
            let state = state.map_or("missing", |state| match state {
                StageState::Blocked => "blocked",
                StageState::Unavailable => "unavailable",
                StageState::Ready | StageState::NotApplicable => "ready",
            });
            phase_blockers.push(alloc::format!("selected_work.stage.{name}.{state}"));
        } else {
            phase_blockers.extend(stable);
        }
    }
    if !phase_blockers.is_empty() {
        return held(HoldReason::PhaseBlocked, phase_blockers);
    }
    let selected: Vec<&WorkUnitFact> = snapshot
        .units
        .iter()
        .filter(|unit| snapshot.selection.task.is_none() || unit.task == snapshot.selection.task)
        .collect();
    let unit_blockers: Vec<String> = selected
        .iter()
        .filter(|unit| matches!(unit.state, UnitState::Blocked | UnitState::Unavailable))
        .map(|unit| {
            alloc::format!(
                "selected_work.unit.{}.{}",
                unit.task.as_deref().unwrap_or("change"),
                if unit.state == UnitState::Blocked {
                    "blocked"
                } else {
                    "unavailable"
                }
            )
        })
        .collect();
    if !selected.iter().any(|unit| unit.state == UnitState::Ready) {
        let blockers = if unit_blockers.is_empty() {
            Vec::from(["selected_work.unit.none_ready".into()])
        } else {
            unit_blockers
        };
        return held(HoldReason::UnitBlocked, blockers);
    }
    let tasks: BTreeSet<&str> = selected
        .iter()
        .filter_map(|unit| unit.task.as_deref())
        .collect();
    let dependency_blockers: Vec<String> = snapshot
        .dependencies
        .iter()
        .filter(|fact| {
            (tasks.contains(fact.task.as_str()) || fact.task == snapshot.selection.change)
                && fact.state != FactState::Ready
        })
        .map(|fact| {
            alloc::format!(
                "selected_work.dependency.{}.{}",
                fact.task,
                fact.prerequisite
            )
        })
        .collect();
    if !dependency_blockers.is_empty() {
        return held(HoldReason::DependencyBlocked, dependency_blockers);
    }
    TypedReadiness::Ready
}

// ---------------------------------------------------------------------------
// Restricted display.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayCoverage {
    Restricted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayObligation {
    pub id: String,
    pub state: BriefState,
}

/// Audience-filtered display. It has no conversion into an admitted snapshot
/// and is never accepted as producer input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestrictedSelectedWorkDisplay {
    pub schema: String,
    pub coverage: DisplayCoverage,
    pub audience: String,
    pub snapshot: ContentId,
    pub change: String,
    pub task: Option<String>,
    pub obligations: Vec<DisplayObligation>,
    pub redacted: Vec<String>,
}

// r[impl kiln.selected_work.display]
pub fn restricted_display(
    snapshot: &AdmittedSelectedWork,
    audience: &str,
) -> CoreResult<RestrictedSelectedWorkDisplay> {
    let guidance = guidance_projection(snapshot)?;
    Ok(RestrictedSelectedWorkDisplay {
        schema: SELECTED_WORK_DISPLAY_SCHEMA.into(),
        coverage: DisplayCoverage::Restricted,
        audience: audience.into(),
        snapshot: snapshot.snapshot.clone(),
        change: snapshot.selection.change.clone(),
        task: snapshot.selection.task.clone(),
        obligations: guidance
            .obligations
            .into_iter()
            .map(|brief| DisplayObligation {
                id: brief.id,
                state: brief.state,
            })
            .collect(),
        redacted: ["cohort", "source", "context", "readiness", "evidence"]
            .into_iter()
            .map(String::from)
            .collect(),
    })
}

fn identity(domain: &str, value: &impl Serialize) -> CoreResult<ContentId> {
    let bytes = serde_json::to_vec(&(domain, value)).map_err(|_| {
        refusal(
            "selected_work_encoding",
            "selected_work.identity",
            "selected-work identity encoding failed",
        )
    })?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
}

fn history_bound() -> Diagnostic {
    refusal(
        "selected_work_history_bound",
        "selected_work.history",
        "selected-work history exceeds its retained bound",
    )
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
#[path = "selected_work_tests.rs"]
mod tests;
