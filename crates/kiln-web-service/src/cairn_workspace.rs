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
use kiln_core::projection::authority::{
    EffectiveSourceScope, MAX_SOURCE_GRANTS, OriginScope, PathScope, SnapshotRule, SourceAuthority,
    SourceAuthorityRequest, SourceCurrentness, SourceViewOperation, check_request,
};
use kiln_core::projection::disclosure::{
    CairnItem, CairnItemKind, CairnObservation, CairnStatus, HandleKey, RestrictedContext,
    ScopedEnumeration, ScopedSelection, ViewCeiling, restrict, restrict_context,
};
use kiln_core::projection::{AdmittedDefinition, AdmittedRepository, ProjectionFacts, resolve};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::campaign::AdmissionState;
use kiln_implementer::native_publication::NativeCairnLifecycleReceipt;
use kiln_implementer::observation::CanonicalSnapshot;
use kiln_implementer::publication_effects::CairnLifecycleAction;
use serde::Serialize;

use crate::inventory::campaign_projection::repository_identity;
use crate::worker::source_auth::RequestProfile;

pub const INDEX_SCHEMA: &str = "kiln.cairn-workspace-index.v2";
pub const DETAIL_SCHEMA: &str = "kiln.cairn-workspace-detail.v4";
/// Changes one index lists before reporting truncation.
pub const MAX_INDEX_CHANGES: usize = 512;
/// Error code of a retained record the projection cannot identify.
pub const MALFORMED: &str = "cairn_workspace_observation_malformed";
/// Owner of native Cairn source observations, in the campaign owner vocabulary.
const NATIVE_SOURCE_OWNER: &str = "native-source-owner";
const NON_CLAIMS: [&str; 4] = [
    "display is not campaign, work, verification or lifecycle authority",
    "no claim, dispatch, reservation, publication or lifecycle mutation follows from this view",
    "unrestricted local mode shows the full projection; restricted observers filter changes, text and prerequisites through SourceAuthority and RestrictedContext",
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
const BLOCKER_RANK: [&str; 8] = [
    "dispatch_recovery",
    "dependency_cycle",
    "no_acceptance_obligations",
    "missing_dependency",
    "plan_not_accepted",
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
    /// Agent revisions, capability and wait reasons from the resource
    /// inventory, with their own source state.
    pub agents: agents::AgentRevisions,
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
                    BlockReason::PlanNotAccepted => (
                        "plan_not_accepted",
                        CampaignOperation::RequestDecision,
                        "opt-in plan review has no accepted plan for this exact work version"
                            .to_owned(),
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
    inventory: &agents::InventoryObservation,
) -> Result<Detail, &'static str> {
    let units = units(key, facts, context)?;
    let memberships: Vec<MembershipView> = facts
        .members
        .iter()
        .map(|(state, version, binding)| membership(state, *version, binding, context))
        .collect();
    let blockers = blockers(&units, &memberships);
    let attempts = units.iter().flat_map(|unit| &unit.attempts);
    let attempt_keys: BTreeSet<(&str, u64)> = attempts
        .clone()
        .map(|attempt| (attempt.identity.as_str(), attempt.fence))
        .collect();
    let agents = agents::project(inventory, &attempt_keys);
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
        agents,
        non_claims: NON_CLAIMS,
    })
}

// r[impl kiln.cairn.workspace]
// r[impl kiln.cairn.workspace.scenario.identity]
// r[impl kiln.cairn.workspace.scenario.visibility]
// r[impl kiln.cairn.workspace.agents.scenario.source]
/// The detail of one change, or `None` when nothing names it. The view
/// identity binds the inventory observation the agents section came from.
pub fn detail(
    snapshot: &CanonicalSnapshot,
    key: &CairnKey,
    inventory: &agents::InventoryObservation,
) -> Result<Option<Detail>, &'static str> {
    let catalog = catalog(snapshot);
    let Some(facts) = catalog.get(key) else {
        return Ok(None);
    };
    let mut detail = project(key, facts, snapshot, &Context::new(snapshot)?, inventory)?;
    let bytes = serde_json::to_vec(&detail).map_err(|_| "cairn_workspace_encoding_invalid")?;
    detail.identity = blake3::hash(&bytes).to_hex().to_string();
    Ok(Some(detail))
}

/// Every change named by canonical work or a campaign selection.
pub fn index(snapshot: &CanonicalSnapshot) -> Result<Index, &'static str> {
    index_bounded(snapshot, MAX_INDEX_CHANGES)
}

fn index_bounded(snapshot: &CanonicalSnapshot, max_changes: usize) -> Result<Index, &'static str> {
    index_filtered(snapshot, max_changes, |_| true)
}

fn index_filtered(
    snapshot: &CanonicalSnapshot,
    max_changes: usize,
    mut include: impl FnMut(&CairnKey) -> bool,
) -> Result<Index, &'static str> {
    let catalog = catalog(snapshot);
    let context = Context::new(snapshot)?;
    let matching: Vec<_> = catalog.iter().filter(|(key, _)| include(key)).collect();
    let total = matching.len();
    let mut changes = Vec::with_capacity(total.min(max_changes));
    for (key, facts) in matching.into_iter().take(max_changes) {
        // The index carries no agents section.
        let detail = project(
            key,
            facts,
            snapshot,
            &context,
            &agents::InventoryObservation::NotConfigured,
        )?;
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
        truncated: total > max_changes,
        snapshot: snapshot_facts(snapshot),
        non_claims: NON_CLAIMS,
    })
}

pub const RESTRICTED_INDEX_SCHEMA: &str = "kiln.cairn-workspace-restricted-index.v1";
pub const RESTRICTED_DETAIL_SCHEMA: &str = "kiln.cairn-workspace-restricted-detail.v1";

/// Source-authority observer context for restricted Cairn workspace views (#84).
pub struct RestrictedCairnObserver<'a> {
    pub authority: &'a mut dyn SourceAuthority,
    pub profile: RequestProfile<'a>,
    pub current: &'a SourceCurrentness,
    pub repository_id: &'a ContentId,
    pub definition: &'a AdmittedDefinition,
    pub admitted_repository: &'a AdmittedRepository,
    pub facts: &'a ProjectionFacts,
    pub handle_key: &'a HandleKey,
    pub now_unix_s: u64,
}

impl std::fmt::Debug for RestrictedCairnObserver<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RestrictedCairnObserver")
            .field("repository_id", &self.repository_id)
            .field("now_unix_s", &self.now_unix_s)
            .finish_non_exhaustive()
    }
}

/// Restricted Cairn workspace index filtered by `SourceAuthority` and
/// `RestrictedContext`.
#[derive(Clone, Debug, Serialize)]
pub struct RestrictedIndex {
    pub schema: &'static str,
    pub index: Index,
    pub restricted_context: serde_json::Value,
}

/// Restricted Cairn workspace detail filtered by `SourceAuthority` and
/// `RestrictedContext`. Unauthorized prerequisite IDs/paths and unauthorized
/// text are redacted.
#[derive(Clone, Debug, Serialize)]
pub struct RestrictedDetail {
    pub schema: &'static str,
    pub detail: Detail,
    pub restricted_context: serde_json::Value,
}

struct AdmittedCairnContext {
    scope: EffectiveSourceScope,
    context: RestrictedContext,
    work_to_change: BTreeMap<String, String>,
    change_to_native: BTreeMap<String, String>,
    visible_work_ids: BTreeSet<String>,
    text_allowed_changes: BTreeSet<String>,
}

fn admit_change_op(
    observer: &mut RestrictedCairnObserver<'_>,
    op: SourceViewOperation,
    path: PathScope,
    payload: &[u8],
) -> Option<EffectiveSourceScope> {
    let request = SourceAuthorityRequest {
        operation: op,
        repository: observer.repository_id.clone(),
        snapshot: SnapshotRule::Exact {
            revision: observer.facts.exact_revision.clone(),
        },
        view_binding: observer.definition.identity().clone(),
        scope: OriginScope::Components { path },
        request_digest: *blake3::hash(payload).as_bytes(),
        attempt: None,
    };
    let owned_profile;
    let profile = match &observer.profile {
        RequestProfile::Static(p) => *p,
        RequestProfile::Signer(sign) => {
            owned_profile = sign(&request).ok()?;
            &owned_profile
        }
    };
    let decision = observer
        .authority
        .admit(&request, profile, observer.current, observer.now_unix_s)
        .ok()?;
    let session = decision.session?;
    let revalid = observer
        .authority
        .revalidate(session, &request, observer.current, observer.now_unix_s)
        .is_ok();
    let scope = observer.authority.effective_scope(session).cloned();
    observer.authority.close(session);
    if !revalid {
        return None;
    }
    let scope = scope?;
    check_request(&scope, &request, observer.current, observer.now_unix_s).ok()?;
    Some(scope)
}

struct BuiltCairnObservation {
    observation: CairnObservation,
    work_to_change: BTreeMap<String, String>,
    change_to_native: BTreeMap<String, String>,
}

fn build_cairn_observation(
    snapshot: &CanonicalSnapshot,
    observer_repo_hex: &str,
    repository_id: &ContentId,
    revision: &ExternalRef,
) -> Result<BuiltCairnObservation, &'static str> {
    let catalog = catalog(snapshot);
    let mut work_to_change: BTreeMap<String, String> = BTreeMap::new();
    let mut change_to_native: BTreeMap<String, String> = BTreeMap::new();
    for (key, facts) in catalog
        .iter()
        .filter(|(key, _)| key.repository_identity == observer_repo_hex)
    {
        let native_id = format!("change-{}", key.change);
        change_to_native.insert(key.change.clone(), native_id.clone());
        for (work, is_current) in &facts.work {
            if *is_current && let Ok(id) = work.identity() {
                work_to_change.insert(id.as_str().to_owned(), key.change.clone());
            }
        }
    }
    let mut items = Vec::new();
    let mut seen_native = BTreeSet::new();
    for (key, facts) in catalog
        .iter()
        .filter(|(key, _)| key.repository_identity == observer_repo_hex)
    {
        let Some(native_id) = change_to_native.get(&key.change).cloned() else {
            continue;
        };
        if !seen_native.insert(native_id.clone()) {
            continue;
        }
        let mut prereqs = BTreeSet::new();
        let mut text_parts = Vec::new();
        for (work, is_current) in &facts.work {
            if !*is_current {
                continue;
            }
            text_parts.push(work.title.clone());
            for dep in &work.dependencies {
                if let Some(dep_change) = work_to_change.get(dep.as_str())
                    && dep_change != &key.change
                    && let Some(dep_native) = change_to_native.get(dep_change)
                {
                    prereqs.insert(dep_native.clone());
                }
            }
        }
        items.push(CairnItem {
            change: key.change.clone(),
            kind: CairnItemKind::Change,
            native_id,
            path: format!(".cairn/changes/{}/tasks.md", key.change),
            text: text_parts.join("; "),
            status: CairnStatus::Open,
            prerequisites: prereqs.into_iter().collect(),
        });
    }
    let observation = CairnObservation::new(repository_id.clone(), revision.clone(), items)
        .map_err(|_| "cairn_observation_invalid")?;
    Ok(BuiltCairnObservation {
        observation,
        work_to_change,
        change_to_native,
    })
}

fn can_add_grant_path(grant_paths: &BTreeSet<PathScope>, path: &PathScope) -> bool {
    grant_paths.contains(path) || grant_paths.len() < MAX_SOURCE_GRANTS
}

fn push_cairn_scope(
    scopes: &mut Vec<EffectiveSourceScope>,
    grant_paths: &mut BTreeSet<PathScope>,
    scope: EffectiveSourceScope,
) -> bool {
    if scope.grants().is_empty() {
        return false;
    }
    let new_count = scope
        .grants()
        .iter()
        .filter(|grant| !grant_paths.contains(&grant.path))
        .count();
    if grant_paths.len().saturating_add(new_count) > MAX_SOURCE_GRANTS {
        return false;
    }
    for grant in scope.grants() {
        grant_paths.insert(grant.path.clone());
    }
    scopes.push(scope);
    true
}

fn collect_cairn_discover_scopes(
    observer: &mut RestrictedCairnObserver<'_>,
    ceiling: &ViewCeiling,
    ceiling_changes: &[(String, PathScope)],
    scopes: &mut Vec<EffectiveSourceScope>,
    grant_paths: &mut BTreeSet<PathScope>,
) {
    if !ceiling
        .operations()
        .contains(&SourceViewOperation::Discover)
    {
        return;
    }
    let is_covered = |scopes: &[EffectiveSourceScope], scope: &PathScope| {
        scopes
            .iter()
            .any(|s| s.permits_scope(SourceViewOperation::Discover, scope))
    };
    if ceiling_changes.len() > 1 {
        for cpath in ceiling.paths() {
            if !can_add_grant_path(grant_paths, cpath) {
                continue;
            }
            if !ceiling_changes
                .iter()
                .any(|(_, cs)| cpath.contains(cs) && !is_covered(scopes, cs))
            {
                continue;
            }
            if let Some(scope) = admit_change_op(
                observer,
                SourceViewOperation::Discover,
                cpath.clone(),
                b"cairn:discover-ceiling",
            ) {
                let _ = push_cairn_scope(scopes, grant_paths, scope);
            }
        }
        for prefix in [".cairn", ".cairn/changes"] {
            let Some(dir_scope) = PathScope::from_path(prefix, true) else {
                continue;
            };
            if !can_add_grant_path(grant_paths, &dir_scope)
                || !ceiling.contains(&dir_scope)
                || ceiling.paths().contains(&dir_scope)
            {
                continue;
            }
            let uncovered = ceiling_changes
                .iter()
                .filter(|(_, cs)| dir_scope.contains(cs) && !is_covered(scopes, cs))
                .count();
            if uncovered < 2 {
                continue;
            }
            if let Some(scope) = admit_change_op(
                observer,
                SourceViewOperation::Discover,
                dir_scope,
                prefix.as_bytes(),
            ) {
                let _ = push_cairn_scope(scopes, grant_paths, scope);
            }
        }
    }
    for (change, change_scope) in ceiling_changes {
        if is_covered(scopes, change_scope) || !can_add_grant_path(grant_paths, change_scope) {
            continue;
        }
        if let Some(scope) = admit_change_op(
            observer,
            SourceViewOperation::Discover,
            change_scope.clone(),
            change.as_bytes(),
        ) {
            let _ = push_cairn_scope(scopes, grant_paths, scope);
        }
    }
}

fn collect_cairn_read_scopes(
    observer: &mut RestrictedCairnObserver<'_>,
    ceiling: &ViewCeiling,
    discovered_changes: &[String],
    prioritized_change: Option<&str>,
    scopes: &mut Vec<EffectiveSourceScope>,
    grant_paths: &mut BTreeSet<PathScope>,
) {
    if !ceiling
        .operations()
        .contains(&SourceViewOperation::ReadContent)
    {
        return;
    }
    let is_read_covered = |scopes: &[EffectiveSourceScope], path: &str| {
        scopes
            .iter()
            .any(|s| s.permits(SourceViewOperation::ReadContent, path))
    };
    if discovered_changes.len() > 1 {
        for cpath in ceiling.paths() {
            if !can_add_grant_path(grant_paths, cpath) {
                continue;
            }
            let uncovered = discovered_changes
                .iter()
                .filter(|change| {
                    let tasks_path = format!(".cairn/changes/{change}/tasks.md");
                    ceiling.allows(SourceViewOperation::ReadContent, &tasks_path)
                        && cpath.covers_path(&tasks_path)
                        && !is_read_covered(scopes, &tasks_path)
                })
                .count();
            if uncovered < 2 {
                continue;
            }
            if let Some(scope) = admit_change_op(
                observer,
                SourceViewOperation::ReadContent,
                cpath.clone(),
                b"cairn:read-ceiling",
            ) {
                let _ = push_cairn_scope(scopes, grant_paths, scope);
            }
        }
        for prefix in [".cairn", ".cairn/changes"] {
            let Some(dir_scope) = PathScope::from_path(prefix, true) else {
                continue;
            };
            if !can_add_grant_path(grant_paths, &dir_scope)
                || !ceiling.contains(&dir_scope)
                || ceiling.paths().contains(&dir_scope)
            {
                continue;
            }
            let uncovered = discovered_changes
                .iter()
                .filter(|change| {
                    let tasks_path = format!(".cairn/changes/{change}/tasks.md");
                    ceiling.allows(SourceViewOperation::ReadContent, &tasks_path)
                        && dir_scope.covers_path(&tasks_path)
                        && !is_read_covered(scopes, &tasks_path)
                })
                .count();
            if uncovered < 2 {
                continue;
            }
            if let Some(scope) = admit_change_op(
                observer,
                SourceViewOperation::ReadContent,
                dir_scope,
                prefix.as_bytes(),
            ) {
                let _ = push_cairn_scope(scopes, grant_paths, scope);
            }
        }
    }
    let ordered = prioritized_change
        .into_iter()
        .filter(|target| discovered_changes.iter().any(|c| c == target))
        .chain(
            discovered_changes
                .iter()
                .map(String::as_str)
                .filter(|change| Some(*change) != prioritized_change),
        );
    for change in ordered {
        let tasks_path = format!(".cairn/changes/{change}/tasks.md");
        if !ceiling.allows(SourceViewOperation::ReadContent, &tasks_path)
            || is_read_covered(scopes, &tasks_path)
        {
            continue;
        }
        if let Some(tasks_scope) = PathScope::from_path(&tasks_path, false)
            && can_add_grant_path(grant_paths, &tasks_scope)
            && let Some(scope) = admit_change_op(
                observer,
                SourceViewOperation::ReadContent,
                tasks_scope,
                change.as_bytes(),
            )
        {
            let _ = push_cairn_scope(scopes, grant_paths, scope);
        }
    }
}

fn visible_campaigns_read(
    snapshot: &CanonicalSnapshot,
    repo_hex: &str,
    visible_changes: &BTreeSet<&str>,
) -> usize {
    snapshot
        .campaigns
        .iter()
        .filter(|(state, _)| {
            repository_identity(&state.contract.repository) == repo_hex
                && state.contract.members.iter().any(|binding| {
                    let (MemberSelector::WholeChange { change }
                    | MemberSelector::Tasks { change, .. }) = &binding.selector;
                    visible_changes.contains(change.as_str())
                })
        })
        .count()
}

fn admit_cairn_context(
    snapshot: &CanonicalSnapshot,
    observer: &mut RestrictedCairnObserver<'_>,
    prioritized_change: Option<&str>,
) -> Result<AdmittedCairnContext, ()> {
    let repo_hex = repository_identity(observer.admitted_repository.repository_id());
    let ceiling = ViewCeiling::from_definition(observer.definition);
    let catalog = catalog(snapshot);
    let distinct_changes: BTreeSet<String> = catalog
        .keys()
        .filter(|k| k.repository_identity == repo_hex)
        .map(|k| k.change.clone())
        .collect();
    let ceiling_changes: Vec<(String, PathScope)> = distinct_changes
        .iter()
        .filter_map(|change| {
            let scope = PathScope::cairn_change(change)?;
            ceiling.contains(&scope).then(|| (change.clone(), scope))
        })
        .collect();
    if ceiling_changes.is_empty() {
        return Err(());
    }
    let mut scopes = Vec::new();
    let mut grant_paths = BTreeSet::new();
    collect_cairn_discover_scopes(
        observer,
        &ceiling,
        &ceiling_changes,
        &mut scopes,
        &mut grant_paths,
    );
    let discovered_changes: Vec<String> = ceiling_changes
        .iter()
        .filter(|(_, change_scope)| {
            scopes
                .iter()
                .any(|s| s.permits_scope(SourceViewOperation::Discover, change_scope))
        })
        .map(|(change, _)| change.clone())
        .collect();
    if discovered_changes.is_empty() || scopes.is_empty() {
        return Err(());
    }
    collect_cairn_read_scopes(
        observer,
        &ceiling,
        &discovered_changes,
        prioritized_change,
        &mut scopes,
        &mut grant_paths,
    );
    let joined = EffectiveSourceScope::join(&scopes).map_err(|_| ())?;
    let selection = ScopedSelection::for_holder(&joined, &ceiling);
    let filtered = selection.filter_facts(observer.facts).map_err(|_| ())?;
    let partial =
        resolve(observer.definition, observer.admitted_repository, &filtered).map_err(|_| ())?;
    let enumeration = ScopedEnumeration::new(&selection, partial).map_err(|_| ())?;
    let view = restrict(
        &enumeration,
        &joined,
        &ceiling,
        observer.handle_key,
        observer.now_unix_s,
    )
    .map_err(|_| ())?;
    let built = build_cairn_observation(
        snapshot,
        &repo_hex,
        observer.repository_id,
        &observer.facts.exact_revision,
    )
    .map_err(|_| ())?;
    let context = restrict_context(
        &view,
        &built.observation,
        &joined,
        observer.handle_key,
        observer.now_unix_s,
    )
    .map_err(|_| ())?;
    let visible_changes: BTreeSet<&str> = context
        .items()
        .iter()
        .map(|item| item.change.as_str())
        .collect();
    let text_allowed_changes: BTreeSet<String> = context
        .items()
        .iter()
        .filter(|item| item.text.is_some())
        .map(|item| item.change.clone())
        .collect();
    let mut visible_work_ids = BTreeSet::new();
    for (_, facts) in catalog.iter().filter(|(k, _)| {
        k.repository_identity == repo_hex && visible_changes.contains(k.change.as_str())
    }) {
        for (work, _) in &facts.work {
            if let Ok(id) = work.identity() {
                visible_work_ids.insert(id.as_str().to_owned());
            }
        }
    }
    Ok(AdmittedCairnContext {
        scope: joined,
        context,
        work_to_change: built.work_to_change,
        change_to_native: built.change_to_native,
        visible_work_ids,
        text_allowed_changes,
    })
}

fn redacted_work_id() -> Result<ContentId, &'static str> {
    ContentId::from_blake3_hex("0".repeat(64)).map_err(|_| "cairn_workspace_encoding_invalid")
}

fn redact_blocker(blocker: &mut Blocker, visible_work_ids: &BTreeSet<String>, text_allowed: bool) {
    if blocker.condition == "missing_dependency" {
        let visible = blocker
            .detail
            .strip_prefix("dependency work ")
            .and_then(|rest| rest.strip_suffix(" is not current"))
            .is_some_and(|id| visible_work_ids.contains(id));
        if !visible {
            blocker.detail = "prerequisite unavailable".into();
        }
    }
    if !text_allowed && blocker.condition == "campaign_hold" {
        blocker.detail = "[unavailable]".into();
    }
}

// r[impl kiln.source.projection.disclosure.cairn]
/// Filter the Cairn workspace index for a capability-restricted observer (#84).
/// Fails closed if authority is missing, denied, expired, revoked, or stale.
pub fn restricted_index(
    snapshot: &CanonicalSnapshot,
    observer: &mut RestrictedCairnObserver<'_>,
) -> Result<RestrictedIndex, &'static str> {
    restricted_index_bounded(snapshot, observer, MAX_INDEX_CHANGES)
}

fn restricted_index_bounded(
    snapshot: &CanonicalSnapshot,
    observer: &mut RestrictedCairnObserver<'_>,
    max_changes: usize,
) -> Result<RestrictedIndex, &'static str> {
    let repo_hex = repository_identity(observer.admitted_repository.repository_id());
    let admitted = admit_cairn_context(snapshot, observer, None).map_err(|()| "unavailable")?;
    let visible_changes: BTreeSet<&str> = admitted
        .context
        .items()
        .iter()
        .map(|item| item.change.as_str())
        .collect();
    let mut idx = index_filtered(snapshot, max_changes, |key| {
        key.repository_identity == repo_hex && visible_changes.contains(key.change.as_str())
    })?;
    idx.snapshot.campaigns_read = visible_campaigns_read(snapshot, &repo_hex, &visible_changes);
    for entry in &mut idx.changes {
        let text_allowed = admitted.text_allowed_changes.contains(&entry.change);
        if let Some(blocker) = &mut entry.first_blocker {
            redact_blocker(blocker, &admitted.visible_work_ids, text_allowed);
        }
    }
    let presentation = serde_json::to_value(admitted.context.presentation())
        .map_err(|_| "cairn_workspace_encoding_invalid")?;
    Ok(RestrictedIndex {
        schema: RESTRICTED_INDEX_SCHEMA,
        index: idx,
        restricted_context: presentation,
    })
}

// r[impl kiln.source.projection.disclosure.cairn]
/// Filter one Cairn change detail for a capability-restricted observer (#84).
/// Returns `Ok(None)` when the change does not exist or is outside the
/// observer's authorized scope, preventing 403/404 discovery oracles, and
/// redacts unauthorized prerequisite IDs/paths and unauthorized text.
pub fn restricted_detail(
    snapshot: &CanonicalSnapshot,
    key: &CairnKey,
    inventory: &agents::InventoryObservation,
    observer: &mut RestrictedCairnObserver<'_>,
) -> Result<Option<RestrictedDetail>, &'static str> {
    let repo_hex = repository_identity(observer.admitted_repository.repository_id());
    let admitted = admit_cairn_context(snapshot, observer, Some(key.change.as_str()))
        .map_err(|()| "unavailable")?;
    if key.repository_identity != repo_hex {
        return Ok(None);
    }
    let Some(visible_item) = admitted
        .context
        .items()
        .iter()
        .find(|item| item.change == key.change)
    else {
        return Ok(None);
    };
    let Some(mut det) = detail(snapshot, key, inventory)? else {
        return Ok(None);
    };
    let text_allowed = visible_item.text.is_some();
    let visible_changes: BTreeSet<&str> = admitted
        .context
        .items()
        .iter()
        .map(|item| item.change.as_str())
        .collect();
    let ceiling = ViewCeiling::from_definition(observer.definition);
    let can_discover_change = |change: &str| {
        visible_changes.contains(change)
            && PathScope::cairn_change(change).is_some_and(|scope| {
                ceiling.contains(&scope)
                    && admitted
                        .scope
                        .permits_scope(SourceViewOperation::Discover, &scope)
            })
    };
    let hidden_work_ids: BTreeSet<&str> = admitted
        .work_to_change
        .iter()
        .filter(|(_, ch)| !can_discover_change(ch))
        .map(|(work_id, _)| work_id.as_str())
        .collect();
    let redacted_id = redacted_work_id()?;

    for unit in &mut det.units {
        for work in &mut unit.current_work {
            if !text_allowed {
                work.title = "[unavailable]".into();
                work.requested_outcome = "[unavailable]".into();
                work.acceptance.clear();
            }
            for reason in &mut work.block_reasons {
                if let BlockReason::MissingDependency { identity } = reason
                    && !admitted.visible_work_ids.contains(identity.as_str())
                {
                    *identity = redacted_id.clone();
                }
            }
            for dep in work
                .dependencies
                .iter_mut()
                .chain(work.dependents.iter_mut())
            {
                let dep_hidden = !admitted.visible_work_ids.contains(dep.identity.as_str());
                if dep_hidden {
                    let opaque_id = admitted
                        .work_to_change
                        .get(&dep.identity)
                        .and_then(|ch| admitted.change_to_native.get(ch))
                        .map_or_else(
                            || "unavailable".to_string(),
                            |native_id| {
                                format!(
                                    "handle:{}",
                                    admitted
                                        .context
                                        .item_handle(observer.handle_key, native_id)
                                        .to_hex()
                                )
                            },
                        );
                    dep.identity = opaque_id;
                    dep.state = "unavailable";
                    dep.native_ids.clear();
                    dep.repository_identity = None;
                }
            }
        }
        if !text_allowed {
            for attempt in &mut unit.attempts {
                if attempt.cause.is_some() {
                    attempt.cause = Some("[unavailable]".into());
                }
            }
        }
    }
    if !text_allowed {
        for decision in &mut det.decisions {
            decision.question = "[unavailable]".into();
            decision.options.clear();
            if decision.recommendation.is_some() {
                decision.recommendation = Some("[unavailable]".into());
            }
            if decision.resolved_answer.is_some() {
                decision.resolved_answer = Some("[unavailable]".into());
            }
            if decision.pending_reason.is_some() {
                decision.pending_reason = Some("[unavailable]".into());
            }
        }
    }
    for blocker in &mut det.blockers {
        redact_blocker(blocker, &admitted.visible_work_ids, text_allowed);
    }
    det.overview.first_blocker = det.blockers.first().cloned();
    for membership in &mut det.memberships {
        membership.dependencies.retain(|dep| {
            !hidden_work_ids.contains(dep.as_str())
                && dep
                    .split_once('/')
                    .is_some_and(|(ch, _)| can_discover_change(ch))
        });
        if !text_allowed {
            for blocker in &mut membership.blockers {
                *blocker = "[unavailable]".into();
            }
            if let Some(hold) = &mut membership.hold {
                hold.reason = "[unavailable]".into();
            }
        }
    }
    det.snapshot.campaigns_read = visible_campaigns_read(snapshot, &repo_hex, &visible_changes);
    det.identity.clear();
    let bytes = serde_json::to_vec(&det).map_err(|_| "cairn_workspace_encoding_invalid")?;
    det.identity = blake3::hash(&bytes).to_hex().to_string();
    let presentation = serde_json::to_value(admitted.context.presentation())
        .map_err(|_| "cairn_workspace_encoding_invalid")?;
    Ok(Some(RestrictedDetail {
        schema: RESTRICTED_DETAIL_SCHEMA,
        detail: det,
        restricted_context: presentation,
    }))
}

#[path = "cairn_workspace/agents.rs"]
pub mod agents;
#[path = "cairn_workspace/http.rs"]
pub mod http;

#[cfg(test)]
#[path = "cairn_workspace/tests.rs"]
mod tests;
