//! Opt-in review of an implementation plan before attempt admission (#93).
//!
//! A plan is a pinned artifact for one exact work version. Reviewers named by
//! an explicit policy annotate named regions, resolve blocking feedback and
//! accept or request changes; the author proposes revisions that name the plan
//! they replace. The work ledger turns an unaccepted plan into a block reason
//! only while the policy selects the work's repository. Accepting a plan never
//! claims, reserves or dispatches an attempt.
// r[impl kiln.implementation.plan_review]
use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{ImplementationWork, content_identity, push_str};
use crate::types::{ContentId, CoreResult, Diagnostic};

pub const PLAN_SCHEMA: &str = "kiln.implementation-plan.v1";
pub const PLAN_REVIEW_POLICY_SCHEMA: &str = "kiln.implementation-plan-review-policy.v1";
pub const PLAN_CASES_SCHEMA: &str = "kiln.plan-review.cases.v1";
pub const PLAN_RECEIPT_SCHEMA: &str = "kiln.plan-review.receipt.v1";
pub const MAX_PLAN_STEPS: usize = 64;
pub const MAX_PLAN_REVISIONS: u32 = 16;
pub const MAX_PLAN_ANNOTATIONS: usize = 256;
pub const MAX_PLAN_DECISIONS: usize = 64;
pub const MAX_PLAN_TEXT_BYTES: usize = 4_096;
/// Bound on step, annotation, author and reviewer names.
pub const MAX_PLAN_NAME_BYTES: usize = 128;
pub const MAX_PLAN_REVIEWERS: usize = 32;
pub const MAX_PLAN_REPOSITORIES: usize = 64;
pub const MAX_PLAN_CASES: usize = 16;

/// Explicit opt-in. Without it, implementation admission is unchanged.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanReviewPolicy {
    pub schema: String,
    /// Repositories whose work needs an accepted plan before a claim.
    pub repositories: BTreeSet<String>,
    /// Reviewers who may annotate, resolve and decide.
    pub reviewers: BTreeSet<String>,
}

impl PlanReviewPolicy {
    #[must_use]
    pub fn selects(&self, work: &ImplementationWork) -> bool {
        self.repositories.contains(&work.repository)
    }

    pub(super) fn validate(&self) -> CoreResult<()> {
        let bounded = |values: &BTreeSet<String>, max: usize| {
            !values.is_empty() && values.len() <= max && values.iter().all(|value| name(value))
        };
        if self.schema != PLAN_REVIEW_POLICY_SCHEMA
            || !bounded(&self.repositories, MAX_PLAN_REPOSITORIES)
            || !bounded(&self.reviewers, MAX_PLAN_REVIEWERS)
        {
            return Err(refusal(
                "implementation_plan_policy",
                "plan_review.policy",
                "plan-review policy needs bounded repositories and reviewers",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanStep {
    pub id: String,
    pub text: String,
}

/// One pinned plan revision for one exact work version.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationPlan {
    pub schema: String,
    pub work_identity: ContentId,
    /// 1-based revision within the work version's review thread.
    pub revision: u32,
    /// The pinned plan this revision replaces; absent only for revision 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<ContentId>,
    pub author: String,
    pub steps: Vec<PlanStep>,
}

impl ImplementationPlan {
    /// BLAKE3 over every field, in order.
    pub fn identity(&self) -> CoreResult<ContentId> {
        let mut bytes = Vec::new();
        push_str(&mut bytes, PLAN_SCHEMA)?;
        push_str(&mut bytes, self.work_identity.as_str())?;
        bytes.extend_from_slice(&self.revision.to_be_bytes());
        match &self.previous {
            Some(previous) => {
                bytes.push(1);
                push_str(&mut bytes, previous.as_str())?;
            }
            None => bytes.push(0),
        }
        push_str(&mut bytes, &self.author)?;
        let count = u32::try_from(self.steps.len()).map_err(|_| plan_refusal())?;
        bytes.extend_from_slice(&count.to_be_bytes());
        for step in &self.steps {
            push_str(&mut bytes, &step.id)?;
            push_str(&mut bytes, &step.text)?;
        }
        content_identity(&bytes)
    }

    fn step(&self, id: &str) -> Option<&PlanStep> {
        self.steps.iter().find(|step| step.id == id)
    }

    fn validate(&self) -> CoreResult<()> {
        let mut ids = BTreeSet::new();
        if self.schema != PLAN_SCHEMA
            || !(1..=MAX_PLAN_REVISIONS).contains(&self.revision)
            || (self.revision == 1) != self.previous.is_none()
            || !name(&self.author)
            || self.steps.is_empty()
            || self.steps.len() > MAX_PLAN_STEPS
            || !self
                .steps
                .iter()
                .all(|step| name(&step.id) && text(&step.text) && ids.insert(step.id.as_str()))
        {
            return Err(plan_refusal());
        }
        Ok(())
    }
}

/// A byte range `[start, end)` of one step's text, on character boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSpan {
    pub start: u32,
    pub end: u32,
}

/// A named region of one pinned plan: a whole step, or a span of its text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRegion {
    pub step: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<TextSpan>,
}

impl PlanRegion {
    /// The region's text in `plan`, when the step and span exist there.
    fn quote<'a>(&self, plan: &'a ImplementationPlan) -> Option<&'a str> {
        let step = plan.step(&self.step)?;
        match self.span {
            None => Some(step.text.as_str()),
            Some(span) if span.start < span.end => step
                .text
                .get(usize::try_from(span.start).ok()?..usize::try_from(span.end).ok()?),
            Some(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanNote {
    Comment {
        text: String,
    },
    /// Remove the whole step.
    Delete,
    /// Replace the region with this text.
    Replace {
        text: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanAnnotation {
    /// Unique within the work version's thread.
    pub id: String,
    pub plan: ContentId,
    pub reviewer: String,
    pub region: PlanRegion,
    pub note: PlanNote,
    pub blocking: bool,
}

/// A reviewer's statement that a blocking annotation is resolved, against
/// the latest revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanResolution {
    pub annotation: String,
    pub plan: ContentId,
    pub reviewer: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanVerdict {
    Accepted,
    ChangesRequested,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanDecision {
    pub plan: ContentId,
    pub reviewer: String,
    pub verdict: PlanVerdict,
}

/// Whether the latest plan admits implementation, and if not, why.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "gate", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanGate {
    NoPlan,
    Unresolved { annotations: Vec<String> },
    Unreviewed { plan: ContentId },
    ChangesRequested { plan: ContentId },
    Accepted { plan: ContentId },
}

/// The review thread of one exact work version, in command order.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanThread {
    pub plans: Vec<ImplementationPlan>,
    pub annotations: Vec<PlanAnnotation>,
    pub resolutions: Vec<PlanResolution>,
    pub decisions: Vec<PlanDecision>,
}

impl PlanThread {
    /// Pin the next revision. An identical pinned plan is returned unchanged;
    /// any other duplicate, skipped or forked revision is refused.
    // r[impl kiln.implementation.plan_review.scenario.refused]
    pub fn propose(
        &mut self,
        work_identity: &ContentId,
        plan: &ImplementationPlan,
    ) -> CoreResult<ContentId> {
        plan.validate()?;
        let identity = plan.identity()?;
        if self.plans.iter().any(|pinned| pinned == plan) {
            return Ok(identity);
        }
        let follows = match self.plans.last() {
            None => plan.revision == 1,
            Some(latest) => {
                latest.revision.checked_add(1) == Some(plan.revision)
                    && plan.previous.as_ref() == Some(&latest.identity()?)
            }
        };
        if &plan.work_identity != work_identity || !follows {
            return Err(refusal(
                "implementation_plan_revision",
                "plan_review.plan",
                "plan must be the next revision of this exact work version's thread",
            ));
        }
        self.plans.push(plan.clone());
        Ok(identity)
    }

    /// Annotate a region of the latest revision.
    pub fn annotate(
        &mut self,
        reviewers: &BTreeSet<String>,
        annotation: &PlanAnnotation,
    ) -> CoreResult<()> {
        let latest = self.latest_for(&annotation.plan, reviewers, &annotation.reviewer)?;
        let bounded_note = match &annotation.note {
            PlanNote::Comment { text: note } | PlanNote::Replace { text: note } => text(note),
            PlanNote::Delete => annotation.region.span.is_none(),
        };
        if !name(&annotation.id)
            || self
                .annotations
                .iter()
                .any(|other| other.id == annotation.id)
            || self.annotations.len() >= MAX_PLAN_ANNOTATIONS
            || annotation.region.quote(latest).is_none()
            || !bounded_note
        {
            return Err(refusal(
                "implementation_plan_annotation",
                "plan_review.annotation",
                "annotation needs a unique id, an existing region and a bounded note",
            ));
        }
        self.annotations.push(annotation.clone());
        Ok(())
    }

    /// Resolve one unresolved blocking annotation against the latest revision.
    pub fn resolve(
        &mut self,
        reviewers: &BTreeSet<String>,
        resolution: &PlanResolution,
    ) -> CoreResult<()> {
        self.latest_for(&resolution.plan, reviewers, &resolution.reviewer)?;
        let open = self
            .annotations
            .iter()
            .any(|annotation| annotation.id == resolution.annotation && annotation.blocking)
            && !self.resolved(&resolution.annotation);
        if !open {
            return Err(refusal(
                "implementation_plan_resolution",
                "plan_review.resolution",
                "resolution must name one unresolved blocking annotation",
            ));
        }
        self.resolutions.push(resolution.clone());
        Ok(())
    }

    /// Accept or request changes on the latest revision. Acceptance is
    /// refused while any blocking annotation is unresolved.
    pub fn decide(
        &mut self,
        reviewers: &BTreeSet<String>,
        decision: &PlanDecision,
    ) -> CoreResult<PlanGate> {
        self.latest_for(&decision.plan, reviewers, &decision.reviewer)?;
        if self.decisions.len() >= MAX_PLAN_DECISIONS
            || (decision.verdict == PlanVerdict::Accepted && !self.unresolved().is_empty())
        {
            return Err(refusal(
                "implementation_plan_decision",
                "plan_review.decision",
                "a plan with unresolved blocking annotations cannot be accepted",
            ));
        }
        self.decisions.push(decision.clone());
        self.gate()
    }

    // r[impl kiln.implementation.plan_review.scenario.blocked]
    pub fn gate(&self) -> CoreResult<PlanGate> {
        let Some(latest) = self.plans.last() else {
            return Ok(PlanGate::NoPlan);
        };
        let unresolved = self.unresolved();
        if !unresolved.is_empty() {
            return Ok(PlanGate::Unresolved {
                annotations: unresolved,
            });
        }
        let plan = latest.identity()?;
        let verdict = self
            .decisions
            .iter()
            .rev()
            .find(|decision| decision.plan == plan)
            .map(|decision| decision.verdict);
        Ok(match verdict {
            None => PlanGate::Unreviewed { plan },
            Some(PlanVerdict::ChangesRequested) => PlanGate::ChangesRequested { plan },
            Some(PlanVerdict::Accepted) => PlanGate::Accepted { plan },
        })
    }

    /// Feedback for the author: every annotation with its region, quoted
    /// text and resolution, the gate, and the latest revision's comparison.
    // r[impl kiln.implementation.plan_review.scenario.revision]
    pub fn feedback(&self) -> CoreResult<PlanFeedback> {
        let latest = self.plans.last().ok_or_else(missing_plan)?;
        let mut items = Vec::with_capacity(self.annotations.len());
        for annotation in &self.annotations {
            let (revision, quoted) = self
                .plans
                .iter()
                .find_map(|plan| {
                    if plan.identity().ok()? != annotation.plan {
                        return None;
                    }
                    let quoted = annotation.region.quote(plan)?;
                    Some((plan.revision, quoted.to_string()))
                })
                .ok_or_else(missing_plan)?;
            let resolved_in = self
                .resolutions
                .iter()
                .find(|resolution| resolution.annotation == annotation.id)
                .and_then(|resolution| self.revision_of(&resolution.plan));
            items.push(PlanFeedbackItem {
                annotation: annotation.id.clone(),
                revision,
                reviewer: annotation.reviewer.clone(),
                region: annotation.region.clone(),
                quoted,
                note: annotation.note.clone(),
                blocking: annotation.blocking,
                resolved_in,
            });
        }
        let comparison = match self
            .plans
            .len()
            .checked_sub(2)
            .and_then(|index| self.plans.get(index))
        {
            Some(previous) => Some(compare_plans(previous, latest)?),
            None => None,
        };
        Ok(PlanFeedback {
            work_identity: latest.work_identity.clone(),
            plan: latest.identity()?,
            revision: latest.revision,
            gate: self.gate()?,
            items,
            comparison,
        })
    }

    /// The latest revision, when `plan` pins it and `reviewer` may act on it.
    fn latest_for(
        &self,
        plan: &ContentId,
        reviewers: &BTreeSet<String>,
        reviewer: &str,
    ) -> CoreResult<&ImplementationPlan> {
        let latest = self.plans.last().ok_or_else(missing_plan)?;
        if &latest.identity()? != plan || !reviewers.contains(reviewer) || reviewer == latest.author
        {
            return Err(refusal(
                "implementation_plan_reviewer",
                "plan_review.reviewer",
                "only a policy reviewer other than the author may act on the latest revision",
            ));
        }
        Ok(latest)
    }

    fn resolved(&self, annotation: &str) -> bool {
        self.resolutions
            .iter()
            .any(|resolution| resolution.annotation == annotation)
    }

    fn unresolved(&self) -> Vec<String> {
        self.annotations
            .iter()
            .filter(|annotation| annotation.blocking && !self.resolved(&annotation.id))
            .map(|annotation| annotation.id.clone())
            .collect()
    }

    fn revision_of(&self, plan: &ContentId) -> Option<u32> {
        self.plans
            .iter()
            .find(|pinned| pinned.identity().ok().as_ref() == Some(plan))
            .map(|pinned| pinned.revision)
    }
}

/// One annotation as returned to the plan's author.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanFeedbackItem {
    pub annotation: String,
    /// The revision that was annotated.
    pub revision: u32,
    pub reviewer: String,
    pub region: PlanRegion,
    pub quoted: String,
    pub note: PlanNote,
    pub blocking: bool,
    /// The revision a reviewer resolved it against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_in: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanFeedback {
    pub work_identity: ContentId,
    pub plan: ContentId,
    pub revision: u32,
    pub gate: PlanGate,
    pub items: Vec<PlanFeedbackItem>,
    /// The latest revision against the one it replaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<PlanComparison>,
}

/// Step-level comparison of a revision with an earlier pinned plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanComparison {
    pub previous: ContentId,
    pub current: ContentId,
    /// Same id and text, in current order.
    pub kept: Vec<String>,
    /// Same id, new text, in current order.
    pub changed: Vec<String>,
    /// Only in the current plan, in current order.
    pub added: Vec<String>,
    /// Only in the previous plan, in previous order.
    pub removed: Vec<String>,
    /// Steps present in both appear in a different order.
    pub reordered: bool,
}

pub fn compare_plans(
    previous: &ImplementationPlan,
    current: &ImplementationPlan,
) -> CoreResult<PlanComparison> {
    let mut kept = Vec::new();
    let mut changed = Vec::new();
    let mut added = Vec::new();
    for step in &current.steps {
        match previous.step(&step.id) {
            Some(before) if before.text == step.text => kept.push(step.id.clone()),
            Some(_) => changed.push(step.id.clone()),
            None => added.push(step.id.clone()),
        }
    }
    let removed = previous
        .steps
        .iter()
        .filter(|step| current.step(&step.id).is_none())
        .map(|step| step.id.clone())
        .collect();
    let common = |plan: &ImplementationPlan, other: &ImplementationPlan| -> Vec<String> {
        plan.steps
            .iter()
            .filter(|step| other.step(&step.id).is_some())
            .map(|step| step.id.clone())
            .collect()
    };
    Ok(PlanComparison {
        previous: previous.identity()?,
        current: current.identity()?,
        kept,
        changed,
        added,
        removed,
        reordered: common(previous, current) != common(current, previous),
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanReviewCommand {
    /// Set or clear the opt-in policy.
    Configure {
        policy: Option<PlanReviewPolicy>,
    },
    Propose {
        plan: ImplementationPlan,
    },
    Annotate {
        work: ContentId,
        annotation: PlanAnnotation,
    },
    Resolve {
        work: ContentId,
        resolution: PlanResolution,
    },
    Decide {
        work: ContentId,
        decision: PlanDecision,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanReviewResult {
    Configured,
    Proposed { plan: ContentId },
    Reviewed { gate: PlanGate },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseProvenance {
    /// Written for this fixture in the style of an agent plan.
    Synthetic,
    /// A captured agent plan; unavailable until its bytes are present.
    AgentCapture,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseAnnotation {
    pub id: String,
    pub revision: u32,
    pub region: PlanRegion,
    pub note: PlanNote,
    pub blocking: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseResolution {
    pub annotation: String,
    pub revision: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseDecision {
    pub revision: u32,
    pub verdict: PlanVerdict,
}

/// One frozen case. Commands of a revision replay in the order annotations,
/// resolutions, decisions, after that revision is proposed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCase {
    pub id: String,
    pub provenance: CaseProvenance,
    pub author: String,
    /// Plan revisions in order, as their steps.
    pub revisions: Vec<Vec<PlanStep>>,
    pub annotations: Vec<CaseAnnotation>,
    pub resolutions: Vec<CaseResolution>,
    pub decisions: Vec<CaseDecision>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCases {
    pub schema: String,
    /// The work version every case's plans bind to.
    pub work_identity: ContentId,
    pub reviewer: String,
    pub cases: Vec<PlanCase>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    MissingPlan,
    NotReviewed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaseOutcome {
    /// Review accepted a different plan than the first revision.
    Changed {
        accepted: ContentId,
        comparison: PlanComparison,
    },
    /// No plan was accepted, where accepting the first revision would have
    /// admitted implementation.
    Prevented {
        gate: PlanGate,
    },
    Unchanged {
        accepted: ContentId,
    },
    Unavailable {
        reason: UnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseResult {
    pub id: String,
    pub provenance: CaseProvenance,
    /// The first revision, which a skim-and-approve path would accept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first: Option<ContentId>,
    pub outcome: CaseOutcome,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseCounts {
    pub changed: u32,
    pub prevented: u32,
    pub unchanged: u32,
    pub unavailable: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanReviewReceipt {
    pub schema: String,
    /// BLAKE3 over the canonical JSON of the frozen cases.
    pub cases: ContentId,
    pub results: Vec<CaseResult>,
    pub counts: CaseCounts,
    pub non_claims: Vec<String>,
}

const RECEIPT_NON_CLAIMS: [&str; 3] = [
    "Synthetic cases show the review mechanism and its accounting, not a measured quality gain.",
    "Accepting the first revision unchanged stands in for skim-and-approve; no reviewer behaviour was observed.",
    "An unavailable case is not a pass.",
];

/// Replay every frozen case through the review rules and classify it. A case
/// whose commands are refused fails the receipt.
// r[impl kiln.implementation.plan_review.comparison]
// r[impl kiln.implementation.plan_review.comparison.scenario.frozen]
pub fn plan_review_receipt(cases: &PlanCases) -> CoreResult<PlanReviewReceipt> {
    let mut ids = BTreeSet::new();
    if cases.schema != PLAN_CASES_SCHEMA
        || !name(&cases.reviewer)
        || cases.cases.is_empty()
        || cases.cases.len() > MAX_PLAN_CASES
        || !cases
            .cases
            .iter()
            .all(|case| name(&case.id) && ids.insert(case.id.as_str()))
    {
        return Err(case_refusal());
    }
    let reviewers = BTreeSet::from([cases.reviewer.clone()]);
    let mut counts = CaseCounts::default();
    let mut results = Vec::with_capacity(cases.cases.len());
    for case in &cases.cases {
        let (first, outcome) = replay(&cases.work_identity, &reviewers, case)?;
        let count = match outcome {
            CaseOutcome::Changed { .. } => &mut counts.changed,
            CaseOutcome::Prevented { .. } => &mut counts.prevented,
            CaseOutcome::Unchanged { .. } => &mut counts.unchanged,
            CaseOutcome::Unavailable { .. } => &mut counts.unavailable,
        };
        *count = count.checked_add(1).ok_or_else(case_refusal)?;
        results.push(CaseResult {
            id: case.id.clone(),
            provenance: case.provenance,
            first,
            outcome,
        });
    }
    let encoded = serde_json::to_vec(cases).map_err(|_| case_refusal())?;
    Ok(PlanReviewReceipt {
        schema: PLAN_RECEIPT_SCHEMA.to_string(),
        cases: content_identity(&encoded)?,
        results,
        counts,
        non_claims: RECEIPT_NON_CLAIMS.iter().map(ToString::to_string).collect(),
    })
}

fn replay(
    work: &ContentId,
    reviewers: &BTreeSet<String>,
    case: &PlanCase,
) -> CoreResult<(Option<ContentId>, CaseOutcome)> {
    let count = u32::try_from(case.revisions.len()).map_err(|_| case_refusal())?;
    let in_range = |revision: u32| (1..=count).contains(&revision);
    if !case.annotations.iter().all(|item| in_range(item.revision))
        || !case.resolutions.iter().all(|item| in_range(item.revision))
        || !case.decisions.iter().all(|item| in_range(item.revision))
    {
        return Err(case_refusal());
    }
    let mut thread = PlanThread::default();
    let mut previous = None;
    let mut first = None;
    for (revision, steps) in (1..=count).zip(&case.revisions) {
        let plan = ImplementationPlan {
            schema: PLAN_SCHEMA.to_string(),
            work_identity: work.clone(),
            revision,
            previous: previous.clone(),
            author: case.author.clone(),
            steps: steps.clone(),
        };
        let identity = thread.propose(work, &plan)?;
        first.get_or_insert_with(|| identity.clone());
        for item in case
            .annotations
            .iter()
            .filter(|item| item.revision == revision)
        {
            thread.annotate(
                reviewers,
                &PlanAnnotation {
                    id: item.id.clone(),
                    plan: identity.clone(),
                    reviewer: reviewer(reviewers)?,
                    region: item.region.clone(),
                    note: item.note.clone(),
                    blocking: item.blocking,
                },
            )?;
        }
        for item in case
            .resolutions
            .iter()
            .filter(|item| item.revision == revision)
        {
            thread.resolve(
                reviewers,
                &PlanResolution {
                    annotation: item.annotation.clone(),
                    plan: identity.clone(),
                    reviewer: reviewer(reviewers)?,
                },
            )?;
        }
        for item in case
            .decisions
            .iter()
            .filter(|item| item.revision == revision)
        {
            thread.decide(
                reviewers,
                &PlanDecision {
                    plan: identity.clone(),
                    reviewer: reviewer(reviewers)?,
                    verdict: item.verdict,
                },
            )?;
        }
        previous = Some(identity);
    }
    let Some(first_identity) = first.clone() else {
        return Ok((
            None,
            CaseOutcome::Unavailable {
                reason: UnavailableReason::MissingPlan,
            },
        ));
    };
    if case.decisions.is_empty() {
        return Ok((
            first,
            CaseOutcome::Unavailable {
                reason: UnavailableReason::NotReviewed,
            },
        ));
    }
    let outcome = match thread.gate()? {
        PlanGate::Accepted { plan } if plan == first_identity => {
            CaseOutcome::Unchanged { accepted: plan }
        }
        PlanGate::Accepted { plan } => {
            let initial = thread.plans.first().ok_or_else(case_refusal)?;
            let accepted = thread.plans.last().ok_or_else(case_refusal)?;
            CaseOutcome::Changed {
                comparison: compare_plans(initial, accepted)?,
                accepted: plan,
            }
        }
        gate => CaseOutcome::Prevented { gate },
    };
    Ok((first, outcome))
}

fn reviewer(reviewers: &BTreeSet<String>) -> CoreResult<String> {
    reviewers.first().cloned().ok_or_else(case_refusal)
}

fn name(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_PLAN_NAME_BYTES && !value.chars().any(char::is_control)
}

fn text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_PLAN_TEXT_BYTES
        && !value
            .chars()
            .any(|character| character.is_control() && character != '\n')
}

fn plan_refusal() -> Diagnostic {
    refusal(
        "implementation_plan",
        "plan_review.plan",
        "plan needs its schema, a valid revision chain, a named author and bounded unique steps",
    )
}

fn missing_plan() -> Diagnostic {
    refusal(
        "implementation_plan_missing",
        "plan_review.plan",
        "the work version has no pinned plan",
    )
}

fn case_refusal() -> Diagnostic {
    refusal(
        "implementation_plan_cases",
        "plan_review.cases",
        "plan-review cases are malformed or over bound",
    )
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
#[path = "plan_review_tests.rs"]
mod tests;
