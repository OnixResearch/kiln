//! Read-only Cairn workspace projection (#84).
//!
//! Joins canonical implementation work, attempts with their verification,
//! publication and native lifecycle records, and selected campaign members by
//! exact Cairn identity: the repository identity and the change name. Every
//! value comes from one bounded canonical snapshot supplied by the caller.
//! Nothing here reads storage, claims, dispatches, mutates lifecycle or grants
//! authority.

use std::collections::{BTreeMap, BTreeSet};

use kiln_core::implementation::campaign::{
    CampaignDecision, CampaignOperation, CampaignState, CompletionGoal, DecisionKind,
    DecisionRequestState, EffectState, EnumerationState, MemberBinding, MemberSelector,
    MemberStage, selector_key,
};
use kiln_core::implementation::{
    AcceptanceObligation, AttemptLedger, AttemptPhase, BlockReason, Eligibility, FailureClass,
    ImplementationAttempt, ImplementationWork, PublicationPhase, SourceRef, TerminalPolicy,
    WorkLedger, WorkSourceKind,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::campaign::AdmissionState;
use kiln_implementer::native_publication::NativeCairnLifecycleReceipt;
use kiln_implementer::observation::CanonicalSnapshot;
use kiln_implementer::publication_effects::CairnLifecycleAction;
use serde::Serialize;

use crate::inventory::campaign_projection::repository_identity;

pub const INDEX_SCHEMA: &str = "kiln.cairn-workspace-index.v2";
pub const DETAIL_SCHEMA: &str = "kiln.cairn-workspace-detail.v2";
/// Changes one index lists before reporting truncation.
pub const MAX_INDEX_CHANGES: usize = 512;
/// Error code of a retained record the projection cannot identify.
pub const MALFORMED: &str = "cairn_workspace_observation_malformed";
/// Owner of native Cairn source observations, in the campaign owner vocabulary.
const NATIVE_SOURCE_OWNER: &str = "native-source-owner";
const NON_CLAIMS: [&str; 4] = [
    "display is not campaign, work, verification or lifecycle authority",
    "no claim, dispatch, reservation, publication or lifecycle mutation follows from this view",
    "every authenticated viewer and operator sees the same projection; capability-restricted observers are not implemented",
    "requirement text is limited to canonical acceptance obligations; native Cairn requirement references are not read",
];
/// Board lanes in display order. A lane is a position derived from a unit's
/// facets; the facets stay the evidence.
pub const LANES: [&str; 7] = [
    "attention",
    "awaiting",
    "ready",
    "in_progress",
    "verified",
    "integrated",
    "recorded",
];
/// Attempt phases that do not stop a unit; any other latest phase, including
/// one this projection does not know, places it in `attention`.
const MOVING_PHASES: [&str; 6] = [
    "not_started",
    "claimed",
    "executing",
    "candidate_produced",
    "verifying",
    "verified",
];
/// Blocker conditions in the order a reader must act on them. An unknown
/// dispatch outcome refuses every further attempt, so it comes first.
const BLOCKER_RANK: [&str; 7] = [
    "dispatch_recovery",
    "dependency_cycle",
    "no_acceptance_obligations",
    "missing_dependency",
    "stale_work_binding",
    "pending_effect",
    "campaign_hold",
];

/// One Cairn change in one repository.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CairnKey {
    pub repository_identity: String,
    pub change: String,
}

/// The change and unit of a Cairn native identity `<change>/<unit>`.
fn split_unit(native_id: &str) -> Option<(&str, &str)> {
    native_id
        .split_once('/')
        .filter(|(change, unit)| !change.is_empty() && !unit.is_empty())
}

fn cairn_aliases(work: &ImplementationWork) -> impl Iterator<Item = &SourceRef> {
    work.aliases.iter().filter(|alias| {
        matches!(
            alias.kind,
            WorkSourceKind::CairnTask | WorkSourceKind::CairnArtifact
        )
    })
}

/// The snake_case wire name of a unit enum value.
fn name<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(text)) => text,
        _ => "unknown".into(),
    }
}

/// Every canonical work version and campaign member naming one change.
#[derive(Default)]
struct ChangeFacts<'a> {
    projects: BTreeSet<&'a str>,
    /// Canonical work versions naming a unit of this change; `true` if current.
    work: Vec<(&'a ImplementationWork, bool)>,
    /// Campaign members selecting this change, with their store versions.
    members: Vec<(&'a CampaignState, u64, &'a MemberBinding)>,
}

fn catalog(snapshot: &CanonicalSnapshot) -> BTreeMap<CairnKey, ChangeFacts<'_>> {
    let mut changes: BTreeMap<CairnKey, ChangeFacts<'_>> = BTreeMap::new();
    if let Some(ledger) = &snapshot.work {
        let current = ledger.current().map(|work| (work, true));
        let superseded = ledger.superseded().iter().map(|work| (work, false));
        for (work, is_current) in current.chain(superseded) {
            let repository = repository_identity(&work.repository);
            let named: BTreeSet<&str> = cairn_aliases(work)
                .filter_map(|alias| split_unit(&alias.native_id).map(|(change, _)| change))
                .collect();
            for change in named {
                let facts = changes
                    .entry(CairnKey {
                        repository_identity: repository.clone(),
                        change: change.to_owned(),
                    })
                    .or_default();
                facts.projects.insert(work.project.as_str());
                facts.work.push((work, is_current));
            }
        }
    }
    for (state, version) in &snapshot.campaigns {
        let repository = repository_identity(&state.contract.repository);
        for binding in &state.contract.members {
            let (MemberSelector::WholeChange { change } | MemberSelector::Tasks { change, .. }) =
                &binding.selector;
            changes
                .entry(CairnKey {
                    repository_identity: repository.clone(),
                    change: change.clone(),
                })
                .or_default()
                .members
                .push((state, *version, binding));
        }
    }
    changes
}

/// Facts shared by every change of one snapshot.
struct Context<'a> {
    ledger: Option<&'a WorkLedger>,
    attempts: Option<&'a AttemptLedger>,
    lifecycle: &'a BTreeMap<String, NativeCairnLifecycleReceipt>,
    /// Current canonical work identity to its Cairn native identities.
    current: BTreeMap<String, Vec<String>>,
    /// Current canonical work identity to its repository identity.
    repositories: BTreeMap<String, String>,
    /// Current canonical work identity to the current work that depends on it.
    dependents: BTreeMap<String, BTreeSet<String>>,
    superseded: BTreeSet<String>,
    /// Attempts whose unknown provider outcome is not reconciled.
    unresolved: BTreeSet<String>,
    /// Campaign identity to its holds by member, from one evaluation each.
    holds: BTreeMap<&'a str, BTreeMap<String, CampaignDecision>>,
}

impl<'a> Context<'a> {
    fn new(snapshot: &'a CanonicalSnapshot) -> Result<Self, &'static str> {
        let mut current = BTreeMap::new();
        let mut repositories = BTreeMap::new();
        let mut dependents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut superseded = BTreeSet::new();
        if let Some(ledger) = &snapshot.work {
            for work in ledger.current() {
                let identity = work.identity().map_err(|_| MALFORMED)?;
                for dependency in &work.dependencies {
                    dependents
                        .entry(dependency.as_str().to_owned())
                        .or_default()
                        .insert(identity.as_str().to_owned());
                }
                repositories.insert(
                    identity.as_str().to_owned(),
                    repository_identity(&work.repository),
                );
                current.insert(
                    identity.as_str().to_owned(),
                    cairn_aliases(work)
                        .map(|alias| alias.native_id.clone())
                        .collect(),
                );
            }
            for work in ledger.superseded() {
                let identity = work.identity().map_err(|_| MALFORMED)?;
                superseded.insert(identity.as_str().to_owned());
            }
        }
        let unresolved = snapshot
            .attempts
            .as_ref()
            .map(|ledger| {
                ledger
                    .unresolved()
                    .map(|attempt| attempt.identity.as_str().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        let holds = snapshot
            .campaigns
            .iter()
            .map(|(state, _)| {
                let holds = state
                    .evaluate()
                    .holds
                    .into_iter()
                    .filter_map(|hold| Some((hold.member.clone()?, hold)))
                    .collect();
                (state.contract.campaign_id.as_str(), holds)
            })
            .collect();
        Ok(Self {
            ledger: snapshot.work.as_ref(),
            attempts: snapshot.attempts.as_ref(),
            lifecycle: &snapshot.lifecycle,
            current,
            repositories,
            dependents,
            superseded,
            unresolved,
            holds,
        })
    }

    fn binding_state(&self, identity: Option<&ContentId>) -> &'static str {
        match identity {
            None => "not_bound",
            Some(identity) if self.current.contains_key(identity.as_str()) => "current",
            Some(identity) if self.superseded.contains(identity.as_str()) => "stale",
            Some(_) => "unknown_work",
        }
    }

    /// One work identity as a dependency or dependent: its Cairn units and
    /// repository when it is current work, otherwise only its identity.
    fn dependency_view(&self, identity: &str) -> DependencyView {
        let native_ids = self.current.get(identity);
        DependencyView {
            identity: identity.to_owned(),
            state: if native_ids.is_some() {
                "current"
            } else {
                "not_current"
            },
            native_ids: native_ids.cloned().unwrap_or_default(),
            repository_identity: self.repositories.get(identity).cloned(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SnapshotFacts {
    pub work_recorded: bool,
    pub attempts_recorded: bool,
    pub campaigns_read: usize,
    pub campaigns_truncated: bool,
    pub lifecycle_truncated: bool,
}

fn snapshot_facts(snapshot: &CanonicalSnapshot) -> SnapshotFacts {
    SnapshotFacts {
        work_recorded: snapshot.work.is_some(),
        attempts_recorded: snapshot.attempts.is_some(),
        campaigns_read: snapshot.campaigns.len(),
        campaigns_truncated: snapshot.campaigns_truncated,
        lifecycle_truncated: snapshot.lifecycle_truncated,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DependencyView {
    pub identity: String,
    /// `current`, or `not_current` when absent from current canonical work.
    pub state: &'static str,
    /// Cairn native identities of the dependency when it is current.
    pub native_ids: Vec<String>,
    /// Repository identity of the dependency when it is current work.
    pub repository_identity: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkView {
    pub identity: String,
    pub canonical_key: String,
    pub revision: u64,
    pub project: String,
    pub title: String,
    pub requested_outcome: String,
    /// Admitted Cairn readiness snapshot of this unit.
    pub readiness_snapshot: String,
    pub acceptance: Vec<AcceptanceObligation>,
    pub dependencies: Vec<DependencyView>,
    /// Current canonical work that names this work as a dependency.
    pub dependents: Vec<DependencyView>,
    pub terminal_policy: TerminalPolicy,
    pub priority: u8,
    /// `ready`, `blocked`, or `unknown` when canonical eligibility is unavailable.
    pub readiness: &'static str,
    pub block_reasons: Vec<BlockReason>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkRevisionView {
    pub identity: String,
    pub revision: u64,
    pub readiness_snapshot: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckView {
    pub name: String,
    pub kind: String,
    pub outcome: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerificationView {
    pub run_id: String,
    pub success: bool,
    pub candidate: ExternalRef,
    pub tree: ExternalRef,
    pub verification_profile: String,
    pub checks: Vec<CheckView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PublicationView {
    pub phase: PublicationPhase,
    pub branch: String,
    pub base_branch: String,
    pub pull_request: Option<ExternalRef>,
    pub remote_head: Option<ExternalRef>,
    pub merge_commit: Option<ExternalRef>,
    pub source_status: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LifecycleView {
    pub action: CairnLifecycleAction,
    pub lifecycle_receipt: ExternalRef,
    pub observed_tree: ExternalRef,
    pub lifecycle_policy: String,
    /// `matches_attempt` only when the receipt binds this attempt's work and fence.
    pub binding: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct AttemptView {
    pub identity: String,
    pub ordinal: u8,
    pub fence: u64,
    pub phase: AttemptPhase,
    pub active: bool,
    pub work_identity: String,
    pub work_revision: u64,
    /// `current` when bound to the unit's current canonical work, else `historical`.
    pub applicability: &'static str,
    pub claimed_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub candidate: Option<ExternalRef>,
    pub failure: Option<FailureClass>,
    pub cause: Option<String>,
    pub superseded_by: Option<u64>,
    /// `not_reserved`, `reserved` or `transport_started`.
    pub dispatch: &'static str,
    /// For an unknown provider outcome: `unreconciled` or `reconciled`.
    pub unknown_outcome: Option<&'static str>,
    pub verification: Option<VerificationView>,
    pub publication: Option<PublicationView>,
    pub lifecycle: Option<LifecycleView>,
}

/// Separately legible stages of one unit.
#[derive(Clone, Debug, Serialize)]
pub struct Facets {
    /// `ready`, `blocked`, `unknown` or `not_admitted`.
    pub readiness: &'static str,
    /// Phase of the latest attempt on current work, or `not_started`.
    pub execution: String,
    /// `current_verified`, `current_failed`, `historical_only` or `none`.
    pub evidence: &'static str,
    /// Publication phase of the latest attempt on current work, or `not_published`.
    pub integration: String,
    /// `receipt_observed` for current work, or `not_observed`.
    pub lifecycle: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnitView {
    pub native_id: String,
    pub unit: String,
    /// `canonical_work`, `campaign_selection` and/or `campaign_remaining_obligation`.
    pub sources: BTreeSet<&'static str>,
    pub current_work: Vec<WorkView>,
    pub superseded_work: Vec<WorkRevisionView>,
    pub facets: Facets,
    /// Board lane from [`LANES`], derived from `facets` by [`lane`].
    pub lane: &'static str,
    pub attempts: Vec<AttemptView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EffectView {
    pub effect_id: String,
    pub operation: CampaignOperation,
    pub owner: String,
    pub state: EffectState,
    pub progress_applied: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct MembershipView {
    pub campaign_id: String,
    pub store_version: u64,
    pub selection_revision: u64,
    pub admission: AdmissionState,
    pub member: String,
    /// `whole_change` or `tasks`.
    pub selector: &'static str,
    pub tasks: Vec<String>,
    pub goal: CompletionGoal,
    pub observed: bool,
    pub stage: Option<MemberStage>,
    pub enumeration: Option<EnumerationState>,
    /// `current`, `stale`, `unknown_work` or `not_bound`.
    pub work_binding: &'static str,
    pub work_identity: Option<String>,
    pub attempt_identity: Option<String>,
    pub attempt_fence: Option<u64>,
    pub dependencies: Vec<String>,
    pub remaining_obligations: Vec<String>,
    pub blockers: Vec<String>,
    pub hold: Option<CampaignDecision>,
    pub pending_effects: Vec<EffectView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DecisionView {
    pub campaign_id: String,
    pub identity: String,
    pub member: String,
    pub work_identity: Option<String>,
    pub category: DecisionKind,
    pub state: DecisionRequestState,
    pub question: String,
    pub options: Vec<String>,
    pub recommendation: Option<String>,
    pub resolved_answer: Option<String>,
    pub priority: u8,
    pub created_ms: u64,
    pub pending_reason: Option<String>,
    /// `current`, `changed` or `member_unobserved`.
    pub scope: &'static str,
}

/// One structured reason a unit or member cannot advance. `detail` is
/// display text only and is never parsed into a decision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Blocker {
    pub condition: &'static str,
    /// Native unit identity or campaign member key.
    pub subject: String,
    /// `unit` for a native unit subject, `member` for a campaign member key.
    pub subject_kind: &'static str,
    /// The campaign that owns a member subject; absent for a unit subject.
    pub campaign_id: Option<String>,
    /// Supporting observation: `work:`, `attempt:`, `campaign:` or `effect:` identity.
    pub observation: String,
    pub operation: CampaignOperation,
    pub owner: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Overview {
    pub units: usize,
    pub current_work: usize,
    pub attempts: usize,
    pub active_attempts: usize,
    pub campaigns: usize,
    pub blockers: usize,
    pub first_blocker: Option<Blocker>,
    /// Board lane of the change, by [`change_lane`].
    pub lane: &'static str,
    /// Units in each of [`LANES`].
    pub lanes: BTreeMap<&'static str, usize>,
    /// Decisions of the change's members that are unissued or outstanding.
    pub open_decisions: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Detail {
    pub schema: &'static str,
    /// BLAKE3 of this projection with the identity empty.
    pub identity: String,
    pub repository_identity: String,
    pub change: String,
    pub projects: Vec<String>,
    pub snapshot: SnapshotFacts,
    pub overview: Overview,
    pub units: Vec<UnitView>,
    pub memberships: Vec<MembershipView>,
    pub decisions: Vec<DecisionView>,
    pub blockers: Vec<Blocker>,
    pub non_claims: [&'static str; 4],
}

#[derive(Clone, Debug, Serialize)]
pub struct IndexEntry {
    pub repository_identity: String,
    pub change: String,
    pub projects: Vec<String>,
    pub units: usize,
    pub current_work: usize,
    pub attempts: usize,
    pub active_attempts: usize,
    pub campaigns: Vec<String>,
    pub blockers: usize,
    pub first_blocker: Option<Blocker>,
    pub lane: &'static str,
    pub lanes: BTreeMap<&'static str, usize>,
    pub open_decisions: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Index {
    pub schema: &'static str,
    pub changes: Vec<IndexEntry>,
    pub truncated: bool,
    pub snapshot: SnapshotFacts,
    pub non_claims: [&'static str; 4],
}

fn readiness_snapshot(work: &ImplementationWork, native_id: &str) -> Result<String, &'static str> {
    cairn_aliases(work)
        .find(|alias| alias.native_id == native_id)
        .map(|alias| alias.version.as_str().to_owned())
        .ok_or(MALFORMED)
}

fn work_view(
    work: &ImplementationWork,
    native_id: &str,
    context: &Context<'_>,
) -> Result<WorkView, &'static str> {
    let identity = work.identity().map_err(|_| MALFORMED)?;
    let canonical_key = work.canonical_key().map_err(|_| MALFORMED)?;
    let (readiness, block_reasons) = match context
        .ledger
        .and_then(|ledger| ledger.eligibility_of(&canonical_key))
    {
        Some(Eligibility::Ready) => ("ready", Vec::new()),
        Some(Eligibility::Blocked { reasons }) => ("blocked", reasons),
        None => ("unknown", Vec::new()),
    };
    Ok(WorkView {
        identity: identity.as_str().to_owned(),
        canonical_key: canonical_key.as_str().to_owned(),
        revision: work.revision,
        project: work.project.clone(),
        title: work.title.clone(),
        requested_outcome: work.requested_outcome.clone(),
        readiness_snapshot: readiness_snapshot(work, native_id)?,
        acceptance: work.acceptance.clone(),
        dependencies: work
            .dependencies
            .iter()
            .map(|dependency| context.dependency_view(dependency.as_str()))
            .collect(),
        dependents: context
            .dependents
            .get(identity.as_str())
            .into_iter()
            .flatten()
            .map(|dependent| context.dependency_view(dependent))
            .collect(),
        terminal_policy: work.terminal_policy,
        priority: work.priority,
        readiness,
        block_reasons,
    })
}

fn revision_view(
    work: &ImplementationWork,
    native_id: &str,
) -> Result<WorkRevisionView, &'static str> {
    Ok(WorkRevisionView {
        identity: work.identity().map_err(|_| MALFORMED)?.as_str().to_owned(),
        revision: work.revision,
        readiness_snapshot: readiness_snapshot(work, native_id)?,
    })
}

fn attempt_view(
    attempt: &ImplementationAttempt,
    current: &BTreeSet<&str>,
    context: &Context<'_>,
) -> AttemptView {
    let dispatch = if context
        .attempts
        .is_some_and(|ledger| ledger.dispatch_requires_recovery(&attempt.identity))
    {
        "transport_started"
    } else if attempt.dispatch.is_some() {
        "reserved"
    } else {
        "not_reserved"
    };
    let unknown_outcome = (attempt.failure == Some(FailureClass::Unknown)).then(|| {
        if context.unresolved.contains(attempt.identity.as_str()) {
            "unreconciled"
        } else {
            "reconciled"
        }
    });
    AttemptView {
        identity: attempt.identity.as_str().to_owned(),
        ordinal: attempt.ordinal,
        fence: attempt.fence,
        phase: attempt.phase,
        active: attempt.phase.is_active(),
        work_identity: attempt.work_identity.as_str().to_owned(),
        work_revision: attempt.work_revision,
        applicability: if current.contains(attempt.work_identity.as_str()) {
            "current"
        } else {
            "historical"
        },
        claimed_at_ms: attempt.claimed_at_ms,
        ended_at_ms: attempt.ended_at_ms,
        candidate: attempt.candidate.clone(),
        failure: attempt.failure,
        cause: attempt.cause.clone(),
        superseded_by: attempt.superseded_by,
        dispatch,
        unknown_outcome,
        verification: attempt
            .verification
            .as_ref()
            .map(|verification| VerificationView {
                run_id: verification.run.run_id.clone(),
                success: verification.run.success,
                candidate: verification.candidate.clone(),
                tree: verification.tree.clone(),
                verification_profile: verification.verification_profile.as_str().to_owned(),
                checks: verification
                    .run
                    .checks
                    .iter()
                    .map(|check| CheckView {
                        name: check.name.clone(),
                        kind: check.kind.clone(),
                        outcome: check.outcome.clone(),
                    })
                    .collect(),
            }),
        publication: attempt.publication.as_ref().map(|record| PublicationView {
            phase: record.phase,
            branch: record.plan.branch.clone(),
            base_branch: record.plan.base_branch.clone(),
            pull_request: record.pull_request.clone(),
            remote_head: record.remote_head.clone(),
            merge_commit: record
                .integration
                .as_ref()
                .map(|receipt| receipt.merge_commit.clone()),
            source_status: record
                .source_status
                .as_ref()
                .map(|receipt| receipt.state.clone()),
        }),
        lifecycle: context
            .lifecycle
            .get(attempt.identity.as_str())
            .map(|receipt| LifecycleView {
                action: receipt.binding.action,
                lifecycle_receipt: receipt.lifecycle_receipt.clone(),
                observed_tree: receipt.observed_tree.clone(),
                lifecycle_policy: receipt.lifecycle_policy.as_str().to_owned(),
                binding: if receipt.work_identity == attempt.work_identity
                    && receipt.fence == attempt.fence
                {
                    "matches_attempt"
                } else {
                    "crossed"
                },
            }),
    }
}

// r[impl kiln.cairn.workspace.scenario.stages]
// r[impl kiln.cairn.workspace.scenario.evidence]
fn facets(current_work: &[WorkView], attempts: &[AttemptView]) -> Facets {
    let readiness = if current_work.is_empty() {
        "not_admitted"
    } else if current_work.iter().any(|work| work.readiness == "blocked") {
        "blocked"
    } else if current_work.iter().any(|work| work.readiness == "unknown") {
        "unknown"
    } else {
        "ready"
    };
    let on_current = || {
        attempts
            .iter()
            .filter(|attempt| attempt.applicability == "current")
    };
    let latest = on_current().max_by_key(|attempt| (attempt.ordinal, attempt.fence));
    let evidence = if on_current().any(|attempt| {
        attempt
            .verification
            .as_ref()
            .is_some_and(|verification| verification.success)
    }) {
        "current_verified"
    } else if on_current().any(|attempt| attempt.verification.is_some()) {
        "current_failed"
    } else if attempts
        .iter()
        .any(|attempt| attempt.verification.is_some())
    {
        "historical_only"
    } else {
        "none"
    };
    let lifecycle = if on_current().any(|attempt| {
        attempt
            .lifecycle
            .as_ref()
            .is_some_and(|receipt| receipt.binding == "matches_attempt")
    }) {
        "receipt_observed"
    } else {
        "not_observed"
    };
    Facets {
        readiness,
        execution: latest.map_or_else(|| "not_started".to_owned(), |attempt| name(&attempt.phase)),
        evidence,
        integration: latest
            .and_then(|attempt| attempt.publication.as_ref())
            .map_or_else(|| "not_published".to_owned(), |record| name(&record.phase)),
        lifecycle,
    }
}

// r[impl kiln.cairn.workspace.scenario.lanes]
/// The board lane of one unit. Any stop wins; otherwise the furthest stage
/// the facets show. Only current verification advances a unit, so historical
/// evidence never does, and an unknown attempt phase stops it.
fn lane(facets: &Facets) -> &'static str {
    let stopped = matches!(facets.readiness, "blocked" | "unknown")
        || !MOVING_PHASES.contains(&facets.execution.as_str())
        || facets.evidence == "current_failed";
    if stopped {
        "attention"
    } else if facets.readiness == "not_admitted" {
        "awaiting"
    } else if facets.lifecycle == "receipt_observed" {
        "recorded"
    } else if matches!(
        facets.integration.as_str(),
        "integrated" | "source_reconciled"
    ) {
        "integrated"
    } else if facets.evidence == "current_verified" {
        "verified"
    } else if facets.execution == "not_started" {
        "ready"
    } else {
        "in_progress"
    }
}

fn lane_rank(lane: &str) -> usize {
    LANES
        .iter()
        .position(|known| *known == lane)
        .unwrap_or(LANES.len())
}

/// A change needs attention when any unit does; otherwise it sits in the lane
/// of its least-advanced unit, or awaits work when it names no unit.
fn change_lane(units: &[UnitView]) -> &'static str {
    if units.iter().any(|unit| unit.lane == "attention") {
        return "attention";
    }
    units
        .iter()
        .map(|unit| unit.lane)
        .min_by_key(|lane| lane_rank(lane))
        .unwrap_or("awaiting")
}

/// Units in each lane, with every lane present.
fn lane_counts(units: &[UnitView]) -> BTreeMap<&'static str, usize> {
    let mut counts: BTreeMap<&'static str, usize> = LANES.iter().map(|lane| (*lane, 0)).collect();
    for unit in units {
        *counts.entry(unit.lane).or_default() += 1;
    }
    counts
}

/// Units of one change from canonical aliases, campaign task selections and
/// campaign remaining obligations. A whole-change selection names the change,
/// not a unit.
fn unit_sources(
    key: &CairnKey,
    facts: &ChangeFacts<'_>,
) -> BTreeMap<String, BTreeSet<&'static str>> {
    let mut units: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();
    let mut add = |native_id: &str, source: &'static str| {
        if split_unit(native_id).is_some_and(|(change, _)| change == key.change) {
            units
                .entry(native_id.to_owned())
                .or_default()
                .insert(source);
        }
    };
    for (work, _) in &facts.work {
        for alias in cairn_aliases(work) {
            add(&alias.native_id, "canonical_work");
        }
    }
    for (state, _, binding) in &facts.members {
        if let MemberSelector::Tasks { change, tasks } = &binding.selector {
            for task in tasks {
                add(&format!("{change}/{task}"), "campaign_selection");
            }
        }
        if let Some(observation) = state.observations.get(&selector_key(&binding.selector)) {
            for native_id in &observation.remaining_obligations {
                add(native_id, "campaign_remaining_obligation");
            }
        }
    }
    units
}

fn units(
    key: &CairnKey,
    facts: &ChangeFacts<'_>,
    context: &Context<'_>,
) -> Result<Vec<UnitView>, &'static str> {
    let mut units = Vec::new();
    for (native_id, sources) in unit_sources(key, facts) {
        let mut current_work = Vec::new();
        let mut superseded_work = Vec::new();
        let mut keys = BTreeSet::new();
        for (work, is_current) in facts
            .work
            .iter()
            .copied()
            .filter(|(work, _)| cairn_aliases(work).any(|alias| alias.native_id == native_id))
        {
            keys.insert(
                work.canonical_key()
                    .map_err(|_| MALFORMED)?
                    .as_str()
                    .to_owned(),
            );
            if is_current {
                current_work.push(work_view(work, &native_id, context)?);
            } else {
                superseded_work.push(revision_view(work, &native_id)?);
            }
        }
        superseded_work.sort_by_key(|work| work.revision);
        let current: BTreeSet<&str> = current_work
            .iter()
            .map(|work| work.identity.as_str())
            .collect();
        let mut attempts: Vec<AttemptView> = context
            .attempts
            .map(|ledger| {
                ledger
                    .attempts()
                    .filter(|attempt| keys.contains(attempt.canonical_key.as_str()))
                    .map(|attempt| attempt_view(attempt, &current, context))
                    .collect()
            })
            .unwrap_or_default();
        attempts.sort_by_key(|attempt| (attempt.ordinal, attempt.fence));
        let unit = split_unit(&native_id).map_or_else(String::new, |(_, unit)| unit.to_owned());
        let facets = facets(&current_work, &attempts);
        units.push(UnitView {
            lane: lane(&facets),
            facets,
            native_id,
            unit,
            sources,
            current_work,
            superseded_work,
            attempts,
        });
    }
    Ok(units)
}

fn membership(
    state: &CampaignState,
    version: u64,
    binding: &MemberBinding,
    context: &Context<'_>,
) -> MembershipView {
    let member = selector_key(&binding.selector);
    let observation = state.observations.get(&member);
    let (selector, tasks) = match &binding.selector {
        MemberSelector::WholeChange { .. } => ("whole_change", Vec::new()),
        MemberSelector::Tasks { tasks, .. } => ("tasks", tasks.clone()),
    };
    let work_identity = observation.and_then(|observation| observation.work_identity.as_ref());
    MembershipView {
        campaign_id: state.contract.campaign_id.clone(),
        store_version: version,
        selection_revision: state.contract.revision,
        admission: AdmissionState::of(state),
        selector,
        tasks,
        goal: binding.goal,
        observed: observation.is_some(),
        stage: observation.map(|observation| observation.stage),
        enumeration: observation.map(|observation| observation.enumeration.clone()),
        work_binding: context.binding_state(work_identity),
        work_identity: work_identity.map(|identity| identity.as_str().to_owned()),
        attempt_identity: observation
            .and_then(|observation| observation.attempt_identity.as_ref())
            .map(|identity| identity.as_str().to_owned()),
        attempt_fence: observation.and_then(|observation| observation.attempt_fence),
        dependencies: observation
            .map(|observation| observation.dependencies.clone())
            .unwrap_or_default(),
        remaining_obligations: observation
            .map(|observation| observation.remaining_obligations.iter().cloned().collect())
            .unwrap_or_default(),
        blockers: observation
            .map(|observation| observation.blockers.clone())
            .unwrap_or_default(),
        hold: context
            .holds
            .get(state.contract.campaign_id.as_str())
            .and_then(|holds| holds.get(&member))
            .cloned(),
        pending_effects: state
            .pending_effects
            .values()
            .filter(|effect| effect.member == member)
            .map(|effect| EffectView {
                effect_id: effect.effect_id.as_str().to_owned(),
                operation: effect.operation,
                owner: effect.owner.clone(),
                state: effect.state,
                progress_applied: effect.progress_applied,
            })
            .collect(),
        member,
    }
}

// r[impl kiln.cairn.workspace.scenario.blockers]
fn blockers(units: &[UnitView], memberships: &[MembershipView]) -> Vec<Blocker> {
    let mut blockers = Vec::new();
    for unit in units {
        for work in &unit.current_work {
            for reason in &work.block_reasons {
                let (condition, operation, detail) = match reason {
                    BlockReason::MissingDependency { identity } => (
                        "missing_dependency",
                        CampaignOperation::Wait,
                        format!("dependency work {} is not current", identity.as_str()),
                    ),
                    BlockReason::NoAcceptanceObligations => (
                        "no_acceptance_obligations",
                        CampaignOperation::Blocked,
                        "no acceptance obligation is admitted".to_owned(),
                    ),
                    BlockReason::DependencyCycle => (
                        "dependency_cycle",
                        CampaignOperation::Blocked,
                        "the dependency graph reaches this work again".to_owned(),
                    ),
                };
                blockers.push(Blocker {
                    condition,
                    subject: unit.native_id.clone(),
                    subject_kind: "unit",
                    campaign_id: None,
                    observation: format!("work:{}", work.identity),
                    operation,
                    owner: NATIVE_SOURCE_OWNER.to_owned(),
                    detail,
                });
            }
        }
        for attempt in unit
            .attempts
            .iter()
            .filter(|attempt| attempt.unknown_outcome == Some("unreconciled"))
        {
            blockers.push(Blocker {
                condition: "dispatch_recovery",
                subject: unit.native_id.clone(),
                subject_kind: "unit",
                campaign_id: None,
                observation: format!("attempt:{}", attempt.identity),
                operation: CampaignOperation::Reconcile,
                owner: CampaignOperation::Reconcile.owner().to_owned(),
                detail: "the provider outcome is unknown; its termination must be observed before another attempt"
                    .to_owned(),
            });
        }
    }
    for membership in memberships {
        let campaign = format!(
            "campaign:{}@{}",
            membership.campaign_id, membership.store_version
        );
        if let Some(hold) = &membership.hold {
            blockers.push(Blocker {
                condition: "campaign_hold",
                subject: membership.member.clone(),
                subject_kind: "member",
                campaign_id: Some(membership.campaign_id.clone()),
                observation: campaign.clone(),
                operation: hold.operation,
                owner: hold
                    .owner
                    .clone()
                    .unwrap_or_else(|| hold.operation.owner().to_owned()),
                detail: hold.reason.clone(),
            });
        }
        for effect in membership
            .pending_effects
            .iter()
            .filter(|effect| !effect.progress_applied)
        {
            blockers.push(Blocker {
                condition: "pending_effect",
                subject: membership.member.clone(),
                subject_kind: "member",
                campaign_id: Some(membership.campaign_id.clone()),
                observation: format!("effect:{}", effect.effect_id),
                operation: CampaignOperation::Reconcile,
                owner: effect.owner.clone(),
                detail: format!(
                    "{} effect is {}",
                    name(&effect.operation),
                    name(&effect.state)
                ),
            });
        }
        if matches!(membership.work_binding, "stale" | "unknown_work") {
            blockers.push(Blocker {
                condition: "stale_work_binding",
                subject: membership.member.clone(),
                subject_kind: "member",
                campaign_id: Some(membership.campaign_id.clone()),
                observation: campaign,
                operation: CampaignOperation::Replan,
                owner: CampaignOperation::Replan.owner().to_owned(),
                detail: format!(
                    "the member binds {} canonical work {}",
                    membership.work_binding,
                    membership.work_identity.as_deref().unwrap_or_default()
                ),
            });
        }
    }
    blockers.sort_by(|a, b| {
        (blocker_rank(a.condition), &a.subject, &a.observation).cmp(&(
            blocker_rank(b.condition),
            &b.subject,
            &b.observation,
        ))
    });
    blockers
}

fn blocker_rank(condition: &str) -> usize {
    BLOCKER_RANK
        .iter()
        .position(|known| *known == condition)
        .unwrap_or(BLOCKER_RANK.len())
}

fn decisions(facts: &ChangeFacts<'_>) -> Vec<DecisionView> {
    let mut decisions = Vec::new();
    for (state, _, binding) in &facts.members {
        let member = selector_key(&binding.selector);
        for request in state
            .decisions
            .values()
            .filter(|request| request.member == member)
        {
            decisions.push(DecisionView {
                campaign_id: state.contract.campaign_id.clone(),
                identity: request.identity.as_str().to_owned(),
                member: request.member.clone(),
                work_identity: request
                    .work_identity
                    .as_ref()
                    .map(|identity| identity.as_str().to_owned()),
                category: request.kind.clone(),
                state: request.state.clone(),
                question: request.question.clone(),
                options: request.options.clone(),
                recommendation: request.recommendation.clone(),
                resolved_answer: request.resolved_answer.clone(),
                priority: request.priority,
                created_ms: request.created_ms,
                pending_reason: request.pending_reason.clone(),
                scope: crate::campaign_controls::decision_scope(state, request),
            });
        }
    }
    decisions.sort_by(|a, b| {
        (&a.campaign_id, &a.member, a.created_ms, &a.identity).cmp(&(
            &b.campaign_id,
            &b.member,
            b.created_ms,
            &b.identity,
        ))
    });
    decisions
}

/// The detail of one change, with its identity still empty.
fn project(
    key: &CairnKey,
    facts: &ChangeFacts<'_>,
    snapshot: &CanonicalSnapshot,
    context: &Context<'_>,
) -> Result<Detail, &'static str> {
    let units = units(key, facts, context)?;
    let memberships: Vec<MembershipView> = facts
        .members
        .iter()
        .map(|(state, version, binding)| membership(state, *version, binding, context))
        .collect();
    let blockers = blockers(&units, &memberships);
    let attempts = units.iter().flat_map(|unit| &unit.attempts);
    let campaigns: BTreeSet<&str> = facts
        .members
        .iter()
        .map(|(state, _, _)| state.contract.campaign_id.as_str())
        .collect();
    let decisions = decisions(facts);
    let open_decisions = decisions
        .iter()
        .filter(|decision| {
            matches!(
                decision.state,
                DecisionRequestState::Unissued | DecisionRequestState::Outstanding
            )
        })
        .count();
    Ok(Detail {
        schema: DETAIL_SCHEMA,
        identity: String::new(),
        repository_identity: key.repository_identity.clone(),
        change: key.change.clone(),
        projects: facts
            .projects
            .iter()
            .map(|&project| project.to_owned())
            .collect(),
        snapshot: snapshot_facts(snapshot),
        overview: Overview {
            units: units.len(),
            current_work: units.iter().map(|unit| unit.current_work.len()).sum(),
            attempts: attempts.clone().count(),
            active_attempts: attempts.filter(|attempt| attempt.active).count(),
            campaigns: campaigns.len(),
            blockers: blockers.len(),
            first_blocker: blockers.first().cloned(),
            lane: change_lane(&units),
            lanes: lane_counts(&units),
            open_decisions,
        },
        units,
        memberships,
        decisions,
        blockers,
        non_claims: NON_CLAIMS,
    })
}

// r[impl kiln.cairn.workspace]
// r[impl kiln.cairn.workspace.scenario.identity]
// r[impl kiln.cairn.workspace.scenario.visibility]
/// The detail of one change, or `None` when nothing names it.
pub fn detail(
    snapshot: &CanonicalSnapshot,
    key: &CairnKey,
) -> Result<Option<Detail>, &'static str> {
    let catalog = catalog(snapshot);
    let Some(facts) = catalog.get(key) else {
        return Ok(None);
    };
    let mut detail = project(key, facts, snapshot, &Context::new(snapshot)?)?;
    let bytes = serde_json::to_vec(&detail).map_err(|_| "cairn_workspace_encoding_invalid")?;
    detail.identity = blake3::hash(&bytes).to_hex().to_string();
    Ok(Some(detail))
}

/// Every change named by canonical work or a campaign selection.
pub fn index(snapshot: &CanonicalSnapshot) -> Result<Index, &'static str> {
    index_bounded(snapshot, MAX_INDEX_CHANGES)
}

fn index_bounded(snapshot: &CanonicalSnapshot, max_changes: usize) -> Result<Index, &'static str> {
    let catalog = catalog(snapshot);
    let context = Context::new(snapshot)?;
    let mut changes = Vec::with_capacity(catalog.len().min(max_changes));
    for (key, facts) in catalog.iter().take(max_changes) {
        let detail = project(key, facts, snapshot, &context)?;
        changes.push(IndexEntry {
            campaigns: detail
                .memberships
                .iter()
                .map(|membership| membership.campaign_id.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            repository_identity: detail.repository_identity,
            change: detail.change,
            projects: detail.projects,
            units: detail.overview.units,
            current_work: detail.overview.current_work,
            attempts: detail.overview.attempts,
            active_attempts: detail.overview.active_attempts,
            blockers: detail.overview.blockers,
            first_blocker: detail.overview.first_blocker,
            lane: detail.overview.lane,
            lanes: detail.overview.lanes,
            open_decisions: detail.overview.open_decisions,
        });
    }
    Ok(Index {
        schema: INDEX_SCHEMA,
        changes,
        truncated: catalog.len() > max_changes,
        snapshot: snapshot_facts(snapshot),
        non_claims: NON_CLAIMS,
    })
}

#[path = "cairn_workspace/http.rs"]
pub mod http;

#[cfg(test)]
#[path = "cairn_workspace/tests.rs"]
mod tests;
