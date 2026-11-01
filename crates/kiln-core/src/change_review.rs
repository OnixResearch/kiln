//! Interactive review semantics. Provider observations and identifiers are admitted by
//! shells; this module never performs I/O, hashes identities, or grants provider authority.
//! `ReviewState` is persistence-only. `project` borrows caller-private drafts,
//! inbox and saved queries; public change views must filter markers with `reviewed`.
//! Commands validate before mutation. Submission prepares an exact candidate,
//! re-admits it with `validate_candidate` immediately before the provider effect,
//! then `RecordSubmit` admits read-back facts and retains `SubmitOutcome`.
//! r[impl kiln.review.workflow]
//! r[impl kiln.review.parity.ownership]

use crate::paths;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub type ReviewResult<T> = Result<T, ReviewError>;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReviewError {
    pub code: String,
    pub message: String,
}
impl core::fmt::Display for ReviewError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
fn fail<T>(code: &str, message: &str) -> ReviewResult<T> {
    Err(ReviewError {
        code: code.into(),
        message: message.into(),
    })
}

/// Canonical engine evidence, never request-body evidence. Latest exact-source run wins.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CiFact {
    pub project: String,
    pub repository: String,
    pub revision: String,
    pub run_id: String,
    pub ordinal: u64,
    pub success: bool,
    pub full_run: bool,
    pub clean: bool,
    /// Scope identity of the run's project when it is scoped (monorepo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_identity: Option<String>,
    /// The run reached a terminal outcome; `false` means still pending.
    #[serde(default = "terminal_default")]
    pub terminal: bool,
    /// Untrusted pull-request run gate projection; absent for trusted runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untrusted: Option<crate::untrusted::UntrustedSummary>,
}
fn terminal_default() -> bool {
    true
}
impl CiFact {
    // r[impl kiln.untrusted.review]
    /// A successful run satisfies required CI; an untrusted one only for its
    /// exact approved commit and only when its reviewed policy says so.
    pub fn satisfies_required(&self) -> bool {
        self.success
            && self.untrusted.as_ref().is_none_or(|untrusted| {
                untrusted.counts_for_required_ci
                    && untrusted.approved_commit.as_deref() == Some(self.revision.as_str())
                    && untrusted.source_commit == self.revision
            })
    }
}
/// Required CI for one affected project of the review repository, computed by
/// the shell from the monorepo affected decision for the revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CiRequirement {
    pub project: String,
    /// Current scope identity; `None` for an unscoped project.
    #[serde(default)]
    pub scope_identity: Option<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeStatus {
    Open,
    Abandoned,
    Submitted,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerRole {
    Reviewer,
    Cc,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoteValue {
    Approve,
    NeedsRevision,
    Veto,
    Abstain,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Base,
    Revision,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
/// `base_commit` is the exact displayed comparison's old commit, even on the
/// revision side. Base-side paths are old paths (including renames/deletions).
#[serde(deny_unknown_fields)]
pub struct Anchor {
    pub revision: String,
    pub base_commit: String,
    pub file: String,
    pub side: Side,
    pub start_line: u32,
    pub end_line: u32,
}
/// Named byte bound for a declared agent harness or model.
pub const MAX_AUTHORSHIP_TEXT_BYTES: usize = 128;
/// Named bound on advisory approvals of one revision; matches the shell's
/// read-principal bound, so every reader can advise once.
pub const MAX_ADVISORY_PER_REVISION: usize = 128;

/// Self-declared authorship of one patch set. Kiln never infers or verifies it;
/// `Undeclared` is the default and older journals replay as `Undeclared`.
/// Parsing is strict: harness/model text is admitted only for `agent`.
// r[impl kiln.review.authorship]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    try_from = "AuthorshipDocument"
)]
pub enum Authorship {
    #[default]
    Undeclared,
    Human,
    Agent {
        harness: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    },
    Mixed,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorshipDocument {
    kind: AuthorshipClass,
    #[serde(default)]
    harness: Option<String>,
    #[serde(default)]
    model: Option<String>,
}
impl TryFrom<AuthorshipDocument> for Authorship {
    type Error = &'static str;
    fn try_from(document: AuthorshipDocument) -> Result<Self, Self::Error> {
        match (document.kind, document.harness, document.model) {
            (AuthorshipClass::Agent, Some(harness), model) => Ok(Self::Agent { harness, model }),
            (AuthorshipClass::Agent, None, _) => Err("agent authorship requires a harness"),
            (_, Some(_), _) | (_, _, Some(_)) => {
                Err("only agent authorship declares a harness or model")
            }
            (AuthorshipClass::Undeclared, None, None) => Ok(Self::Undeclared),
            (AuthorshipClass::Human, None, None) => Ok(Self::Human),
            (AuthorshipClass::Mixed, None, None) => Ok(Self::Mixed),
        }
    }
}
impl Authorship {
    #[must_use]
    pub fn is_undeclared(&self) -> bool {
        matches!(self, Self::Undeclared)
    }
    #[must_use]
    pub const fn class(&self) -> AuthorshipClass {
        match self {
            Self::Undeclared => AuthorshipClass::Undeclared,
            Self::Human => AuthorshipClass::Human,
            Self::Agent { .. } => AuthorshipClass::Agent,
            Self::Mixed => AuthorshipClass::Mixed,
        }
    }
}
/// Aggregation class of declared authorship, in fixed reporting order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorshipClass {
    Human,
    Agent,
    Mixed,
    Undeclared,
}
impl AuthorshipClass {
    pub const ALL: [Self; 4] = [Self::Human, Self::Agent, Self::Mixed, Self::Undeclared];
}
fn authorship_text_valid(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= MAX_AUTHORSHIP_TEXT_BYTES
        && !value.chars().any(char::is_control)
}
/// Reject empty, untrimmed, control-character or over-bound harness/model text.
// r[impl kiln.review.authorship.scenario.invalid]
pub fn validate_authorship(authorship: &Authorship) -> ReviewResult<()> {
    if let Authorship::Agent { harness, model } = authorship
        && (!authorship_text_valid(harness)
            || model
                .as_deref()
                .is_some_and(|model| !authorship_text_valid(model)))
    {
        return fail(
            "invalid_authorship",
            "declared agent harness and model must be bounded non-empty trimmed text",
        );
    }
    Ok(())
}
/// Class of a change from its declared patch sets only: none declared is
/// `Undeclared`, one shared class is that class, differing classes are `Mixed`.
#[must_use]
pub fn authorship_class(change: &Change) -> AuthorshipClass {
    authorship_class_of(&change.revisions)
}
/// [`authorship_class`] over an explicit patch-set prefix, such as the patch
/// sets admitted before an observation window end.
#[must_use]
pub fn authorship_class_of(revisions: &[Revision]) -> AuthorshipClass {
    let mut declared = revisions
        .iter()
        .map(|revision| revision.authorship.class())
        .filter(|class| *class != AuthorshipClass::Undeclared);
    let Some(first) = declared.next() else {
        return AuthorshipClass::Undeclared;
    };
    if declared.all(|class| class == first) {
        first
    } else {
        AuthorshipClass::Mixed
    }
}
/// Declared agent harness/model attached to an attested submission. This is
/// operator-supplied identity metadata, not proof of execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentIdentity {
    pub harness: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttestedSubmission {
    pub principal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentIdentity>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub id: String,
    pub number: u32,
    pub commit: String,
    pub base: String,
    pub files: Vec<String>,
    pub author: String,
    pub created_ms: i64,
    /// Omitted when undeclared so pre-authorship journals keep identical bytes.
    #[serde(default, skip_serializing_if = "Authorship::is_undeclared")]
    pub authorship: Authorship,
    /// Attested agent principal and optional declared harness/model at submission.
    /// Omitted for human and unattested submissions, preserving old bytes.
    // r[impl kiln.review.agent.attestation]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submitted_by: Option<AttestedSubmission>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Vote {
    pub actor: String,
    pub revision: String,
    pub value: VoteValue,
    pub created_ms: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub body: String,
    pub created_ms: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    pub anchor: Anchor,
    pub blocking: bool,
    pub resolved: bool,
    pub comments: Vec<Comment>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub id: String,
    pub change: String,
    pub author: String,
    pub anchor: Anchor,
    pub thread: Option<String>,
    pub body: String,
    pub blocking: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReviewedFile {
    pub actor: String,
    pub revision: String,
    pub file: String,
}
/// Private, principal-owned viewed preference for one normalized hunk of an
/// exact revision. The shell computes `hunk_digest` (BLAKE3 over path, removed
/// and added lines, no line numbers); it never counts as approval.
// r[impl kiln.review.hunks]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedHunk {
    pub actor: String,
    pub revision: String,
    pub file: String,
    pub hunk_digest: String,
}
/// Trusted shell fact: one normalized hunk of a revision's exact base comparison.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HunkFact {
    pub file: String,
    pub digest: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub change: String,
    pub revision: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub revision: String,
    pub commit: String,
    pub target_ref: String,
    pub target: String,
    pub dependencies: Vec<Dependency>,
    pub policy_id: String,
    pub target_is_ancestor: bool,
    pub ci_run: Option<String>,
    pub prepared_by: String,
    /// Provider-admitted ancestry proof for an exact queued integration candidate.
    #[serde(default)]
    pub integration: Option<IntegrationFact>,
}
/// Durable pure receipt of exact authorized state; provider-effect receipts stay
/// in the shell. The candidate is moved here on successful read-back admission.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubmitOutcome {
    pub candidate: Candidate,
    pub result_commit: String,
    pub submitted_by: String,
    pub submitted_ms: i64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceKind {
    Rebase,
    Revert,
    CherryPick,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MaintenanceOutcome {
    pub operation_id: String,
    pub kind: MaintenanceKind,
    pub source_revision: String,
    pub result_change: String,
    pub result_revision: String,
    pub result_commit: String,
    pub actor: String,
    pub created_ms: i64,
}
/// Non-binding "looks good to me" bound to one exact revision. Kept apart from
/// `Change.votes` so no vote admission or checklist filter can ever count it.
// r[impl kiln.review.advisory]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvisoryApproval {
    pub actor: String,
    pub revision: String,
    pub created_ms: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub id: String,
    pub project: String,
    pub repository: String,
    pub target_ref: String,
    pub owner: String,
    pub title: String,
    pub description: String,
    pub state: ChangeStatus,
    pub wip: bool,
    pub revisions: Vec<Revision>,
    pub reviewers: BTreeMap<String, ReviewerRole>,
    pub votes: Vec<Vote>,
    pub threads: Vec<Thread>,
    pub reviewed: Vec<ReviewedFile>,
    /// Omitted when empty so documents written before hunk markers keep identical bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewed_hunks: Vec<ReviewedHunk>,
    /// Each principal's latest revision with a positive viewed mark (private).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub last_viewed: BTreeMap<String, String>,
    pub attention: BTreeSet<String>,
    pub dependencies: Vec<Dependency>,
    pub candidate: Option<Candidate>,
    pub submission: Option<SubmitOutcome>,
    pub maintenance: Vec<MaintenanceOutcome>,
    /// Advisory approvals, current and historical; omitted when empty so
    /// documents written before advisory approvals keep identical bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisory: Vec<AdvisoryApproval>,
}
impl Change {
    pub fn current(&self) -> ReviewResult<&Revision> {
        self.revisions.last().ok_or_else(|| ReviewError {
            code: "invalid_history".into(),
            message: "change has no immutable revision".into(),
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    All,
    AwaitingReview,
    OwnedNeedsRevision,
    Unresolved,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchQuery {
    pub kind: SearchKind,
    pub text: String,
    pub status: Option<ChangeStatus>,
    /// `Some(true)`: at least one current-revision advisory approval;
    /// `Some(false)`: none. Omitted when unset so saved views keep their bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisory: Option<bool>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedQuery {
    pub id: String,
    pub name: String,
    pub query: SearchQuery,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub change: String,
    pub revision: String,
    pub actor: String,
    pub kind: String,
    pub created_ms: i64,
    pub read: bool,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReviewState {
    pub changes: BTreeMap<String, Change>,
    drafts: BTreeMap<String, Vec<Draft>>,
    saved_queries: BTreeMap<String, Vec<SavedQuery>>,
    inbox: BTreeMap<String, Vec<Notification>>,
    used_ids: BTreeSet<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Projection<'a> {
    pub drafts: &'a [Draft],
    pub saved_queries: &'a [SavedQuery],
    pub inbox: &'a [Notification],
}
/// How declared agent principals' approvals count for policy gates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentVotes {
    /// Counted and displayed, but never satisfies approval or ownership items.
    #[default]
    Advisory,
    /// Explicit policy opt-in: agent approvals count like human approvals.
    Binding,
}
impl AgentVotes {
    fn is_advisory(&self) -> bool {
        *self == Self::Advisory
    }
}
/// No default policy: missing policy blocks submission. All identities are explicit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub id: String,
    pub eligible_reviewers: BTreeSet<String>,
    pub submitters: BTreeSet<String>,
    pub minimum_approvals: u32,
    pub require_ci: bool,
    #[serde(default)]
    pub ownership: OwnershipPolicy,
    /// Required checks that may be absent when every exactly known changed
    /// file lies in the declared paths (a pipeline filtered by `paths`).
    /// Without a rule a filtered required check leaves CI unmet.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_ci: Vec<OptionalCi>,
    /// Whether declared agent approvals can satisfy approval and ownership
    /// items. Defaults to advisory; absence serializes unchanged.
    #[serde(default, skip_serializing_if = "AgentVotes::is_advisory")]
    pub agent_votes: AgentVotes,
}

#[path = "change_review_optional_ci.rs"]
mod optional_ci;
pub use optional_ci::{MAX_OPTIONAL_CI_PATTERNS, MAX_OPTIONAL_CI_RULES, OptionalCi};

/// All matching rules apply cumulatively; each needs one expanded owner approval
/// unless its exact change class explicitly selects evidence-only approval.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipPolicy {
    pub teams: BTreeMap<String, BTreeSet<String>>,
    pub rules: Vec<OwnershipRule>,
    /// Policy-wide change classes in precedence order; unmatched changes are
    /// the reserved `logic` class.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<ChangeClass>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeClass {
    pub id: String,
    /// Every changed path must match at least one pattern for the class to apply.
    pub paths: Vec<String>,
}
/// Named bounds for policy-declared change classes.
pub const MAX_CHANGE_CLASSES: usize = 8;
pub const MAX_CHANGE_CLASS_PATTERNS: usize = 16;
/// How a matching ownership rule treats the change's class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipApprovalMode {
    OwnerApproval,
    EvidenceOnly,
}
/// Bounded named evidence a matching ownership rule requires in addition to
/// its owner approval. Evidence is collected by the shell, never asserted by
/// a request body.
// r[impl kiln.review.evidence.policy]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipEvidenceKind {
    ProjectCi,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipEvidence {
    pub kind: OwnershipEvidenceKind,
    /// Project whose current-scope CI must satisfy this evidence.
    pub project: String,
}
/// Named bound for evidence attached to one ownership rule.
pub const MAX_OWNERSHIP_EVIDENCE: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipRule {
    pub id: String,
    /// Root-relative case-sensitive patterns: component `*`, `?`, standalone `**`.
    pub paths: Vec<String>,
    pub owners: BTreeSet<String>,
    pub teams: BTreeSet<String>,
    /// Exact target refs; empty means every target.
    pub target_refs: BTreeSet<String>,
    /// Additional exact-revision evidence this matching rule requires.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_evidence: Vec<OwnershipEvidence>,
    /// Per-class approval mode; absence means owner approval.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub approval: BTreeMap<String, OwnershipApprovalMode>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangedFile {
    pub path: String,
    /// Renames retain the old path; deletions retain their path in `path`.
    pub old_path: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangedFiles {
    pub files: Vec<ChangedFile>,
    pub complete: bool,
}
/// Trusted provider observation, never accepted from request-body assertions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationFact {
    pub source_commit: String,
    pub target_commit: String,
    pub candidate_commit: String,
    pub target_is_ancestor: bool,
    pub source_is_ancestor: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceFact {
    pub project: String,
    pub repository: String,
    pub target_ref: String,
    pub commit: String,
    pub base: String,
    pub current_target: String,
    pub immutable: bool,
    pub target_contains_commit: bool,
    /// Provider proof bound to `commit` and `current_target`, not a UI assertion.
    pub target_is_ancestor: bool,
    /// Exact dependency commits observed reachable from the candidate target.
    pub target_ancestors: BTreeSet<String>,
    #[serde(default)]
    pub changed_files: Option<ChangedFiles>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Facts {
    pub now_ms: i64,
    pub event_id: String,
    pub policy: Option<Policy>,
    pub source: Option<SourceFact>,
    pub ci: Vec<CiFact>,
    pub anchors: Vec<Anchor>,
    #[serde(default)]
    pub integration: Option<IntegrationFact>,
    /// Monorepo required CI: every affected project of the review repository
    /// with its current scope identity. `None` requires the change's own
    /// project; an empty list means every project was skipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_required: Option<Vec<CiRequirement>>,
    /// Every normalized hunk of the current (or uploaded) revision's exact base
    /// comparison. `None` means the shell did not observe them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hunks: Option<Vec<HunkFact>>,
    /// Declared agent principals; trusted shell facts, never request assertions.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub agents: BTreeSet<String>,
    /// Declared harness/model metadata for agent principals.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub agent_identities: BTreeMap<String, AgentIdentity>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub key: String,
    pub satisfied: bool,
    pub detail: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Checklist {
    pub ready: bool,
    pub items: Vec<ChecklistItem>,
    pub ci_run: Option<String>,
}
/// `expected_revision` always names the current Kiln revision ID, not a moving ref.
/// IDs are unique shell-generated BLAKE3 identities. Source/anchor facts are trusted
/// shell input, never fields accepted directly from an HTTP client.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Create {
        id: String,
        project: String,
        repository: String,
        target_ref: String,
        title: String,
        description: String,
        wip: bool,
        revision: Revision,
    },
    Upload {
        change: String,
        expected_revision: String,
        revision: Revision,
    },
    Edit {
        change: String,
        expected_revision: String,
        title: String,
        description: String,
    },
    SetWip {
        change: String,
        expected_revision: String,
        wip: bool,
    },
    Abandon {
        change: String,
        expected_revision: String,
    },
    Restore {
        change: String,
        expected_revision: String,
    },
    SetReviewer {
        change: String,
        expected_revision: String,
        principal: String,
        role: Option<ReviewerRole>,
    },
    Vote {
        change: String,
        expected_revision: String,
        value: VoteValue,
    },
    StartThread {
        change: String,
        expected_revision: String,
        id: String,
        comment_id: String,
        anchor: Anchor,
        body: String,
        blocking: bool,
    },
    Reply {
        change: String,
        expected_revision: String,
        thread: String,
        comment_id: String,
        body: String,
    },
    Resolve {
        change: String,
        expected_revision: String,
        thread: String,
        resolved: bool,
    },
    SaveDraft {
        change: String,
        expected_revision: String,
        draft: Draft,
    },
    DeleteDraft {
        id: String,
    },
    Publish {
        change: String,
        expected_revision: String,
        draft_ids: Vec<String>,
        vote: Option<VoteValue>,
    },
    MarkReviewed {
        change: String,
        expected_revision: String,
        file: String,
        reviewed: bool,
    },
    /// Toggle one normalized hunk; the digest must be a current shell fact.
    MarkHunk {
        change: String,
        expected_revision: String,
        file: String,
        hunk_digest: String,
        reviewed: bool,
    },
    SetAttention {
        change: String,
        expected_revision: String,
        principal: String,
        attention: bool,
    },
    SetDependencies {
        change: String,
        expected_revision: String,
        dependencies: Vec<Dependency>,
    },
    SaveQuery {
        query: SavedQuery,
    },
    DeleteQuery {
        id: String,
    },
    ReadNotification {
        id: String,
        read: bool,
    },
    PrepareSubmit {
        change: String,
        expected_revision: String,
        candidate_id: String,
    },
    RecordSubmit {
        change: String,
        expected_revision: String,
        candidate_id: String,
        expected_target: String,
        result_commit: String,
    },
    RecordMaintenance {
        change: String,
        expected_revision: String,
        outcome: MaintenanceOutcome,
    },
    /// Non-binding approval of the exact current revision; never a vote.
    AdvisoryApprove {
        change: String,
        expected_revision: String,
    },
    /// Withdraw the actor's advisory approval of the exact current revision.
    AdvisoryWithdraw {
        change: String,
        expected_revision: String,
    },
}

pub fn project<'a>(state: &'a ReviewState, actor: &str) -> Projection<'a> {
    Projection {
        drafts: state
            .drafts
            .get(actor)
            .map(Vec::as_slice)
            .unwrap_or_default(),
        saved_queries: state
            .saved_queries
            .get(actor)
            .map(Vec::as_slice)
            .unwrap_or_default(),
        inbox: state
            .inbox
            .get(actor)
            .map(Vec::as_slice)
            .unwrap_or_default(),
    }
}

/// Actor-owned file preferences; public shells must not serialize another
/// principal's rows from the persistence-oriented `Change.reviewed` collection.
pub fn reviewed<'a>(change: &'a Change, actor: &'a str) -> impl Iterator<Item = &'a ReviewedFile> {
    change
        .reviewed
        .iter()
        .filter(move |marker| marker.actor == actor)
}

/// Actor-owned hunk preferences, filtered like [`reviewed`].
pub fn reviewed_hunks<'a>(
    change: &'a Change,
    actor: &'a str,
) -> impl Iterator<Item = &'a ReviewedHunk> {
    change
        .reviewed_hunks
        .iter()
        .filter(move |marker| marker.actor == actor)
}

/// Whether `actor` viewed the hunk `(file, digest)` on `revision`: an exact hunk
/// marker, or a legacy file marker, which keeps its meaning on its own revision.
#[must_use]
pub fn hunk_viewed(change: &Change, actor: &str, revision: &str, file: &str, digest: &str) -> bool {
    change
        .reviewed
        .iter()
        .any(|marker| marker.actor == actor && marker.revision == revision && marker.file == file)
        || change.reviewed_hunks.iter().any(|marker| {
            marker.actor == actor
                && marker.revision == revision
                && marker.file == file
                && marker.hunk_digest == digest
        })
}

fn file_hunks_of<'a>(facts: &'a Facts, file: &'a str) -> impl Iterator<Item = String> + 'a {
    facts
        .hunks
        .iter()
        .flatten()
        .filter(move |hunk| hunk.file == file)
        .map(|hunk| hunk.digest.clone())
}

/// Digests of `file` in the current revision; fails closed without hunk facts.
fn file_hunks(change: &Change, file: &str, facts: &Facts) -> ReviewResult<BTreeSet<String>> {
    if !change.current()?.files.iter().any(|path| path == file) {
        return fail("invalid_file", "file is not in the current revision");
    }
    if facts.hunks.is_none() {
        return fail(
            "hunks_unavailable",
            "current revision hunks were not observed",
        );
    }
    let digests: BTreeSet<String> = file_hunks_of(facts, file).collect();
    if digests.is_empty() {
        return fail("invalid_file", "file has no observed hunks");
    }
    Ok(digests)
}

// r[impl kiln.review.hunks.scenario.carry_forward]
/// Rebind to `next` exactly the markers on `previous` whose normalized hunk is
/// present unchanged in the uploaded revision; drop every other hunk marker.
fn carry_forward(change: &mut Change, previous: &str, next: &str, facts: &Facts) {
    let present: BTreeSet<&HunkFact> = facts.hunks.iter().flatten().collect();
    let mut carried: Vec<ReviewedHunk> = Vec::new();
    for marker in core::mem::take(&mut change.reviewed_hunks) {
        let fact = HunkFact {
            file: marker.file,
            digest: marker.hunk_digest,
        };
        if marker.revision == previous && present.contains(&fact) {
            let moved = ReviewedHunk {
                actor: marker.actor,
                revision: next.into(),
                file: fact.file,
                hunk_digest: fact.digest,
            };
            if !carried.contains(&moved) {
                carried.push(moved);
            }
        }
    }
    change.reviewed_hunks = carried;
}

pub fn search<'a>(state: &'a ReviewState, actor: &str, query: &SearchQuery) -> Vec<&'a Change> {
    let text = query.text.to_lowercase();
    state
        .changes
        .values()
        .filter(|change| {
            let Ok(revision) = change.current() else {
                return false;
            };
            if query.status.is_some_and(|status| status != change.state) {
                return false;
            }
            if query
                .advisory
                .is_some_and(|wanted| wanted != current_advisory(change).next().is_some())
            {
                return false;
            }
            if !text.is_empty()
                && !change.title.to_lowercase().contains(&text)
                && !change.description.to_lowercase().contains(&text)
                && !change.id.contains(&text)
            {
                return false;
            }
            match query.kind {
                SearchKind::All => true,
                SearchKind::AwaitingReview => {
                    change.state == ChangeStatus::Open
                        && !change.wip
                        && change.reviewers.get(actor) == Some(&ReviewerRole::Reviewer)
                        && (change.attention.contains(actor)
                            || !change.votes.iter().any(|v| {
                                v.actor == actor
                                    && v.revision == revision.id
                                    && v.value != VoteValue::Abstain
                            }))
                }
                SearchKind::OwnedNeedsRevision => {
                    change.owner == actor
                        && change.state == ChangeStatus::Open
                        && change.votes.iter().any(|v| {
                            v.revision == revision.id
                                && matches!(v.value, VoteValue::NeedsRevision | VoteValue::Veto)
                        })
                }
                SearchKind::Unresolved => change.threads.iter().any(|thread| !thread.resolved),
            }
        })
        .collect()
}

fn get<'a>(state: &'a ReviewState, id: &str, expected: &str) -> ReviewResult<&'a Change> {
    let Some(change) = state.changes.get(id) else {
        return fail("not_found", "change does not exist");
    };
    if change.revisions.is_empty() {
        return fail("invalid_history", "change has no immutable revision");
    }
    if change.current()?.id != expected {
        return fail("stale_revision", "expected revision is no longer current");
    }
    Ok(change)
}
fn owner(change: &Change, actor: &str) -> ReviewResult<()> {
    if change.owner != actor {
        return fail("forbidden", "only the change owner may perform this action");
    }
    Ok(())
}
fn open(change: &Change) -> ReviewResult<()> {
    if change.state != ChangeStatus::Open {
        return fail("closed", "change is not open");
    }
    Ok(())
}
fn fresh(state: &ReviewState, id: &str) -> ReviewResult<()> {
    if id.trim().is_empty() || state.used_ids.contains(id) {
        return fail("replayed_identity", "identity is empty or already used");
    }
    Ok(())
}
fn body_valid(body: &str) -> ReviewResult<()> {
    if body.trim().is_empty() {
        return fail("empty_body", "text must not be empty");
    }
    Ok(())
}
fn rule_owners<'a>(
    policy: &'a Policy,
    rule: &'a OwnershipRule,
) -> impl Iterator<Item = &'a String> {
    rule.owners.iter().chain(
        rule.teams
            .iter()
            .flat_map(|team| policy.ownership.teams.get(team).into_iter().flatten()),
    )
}

/// Admit human policy once at shell configuration boundaries and again at authority.
/// r[impl kiln.review.parity.ownership]
pub fn validate_policy(policy: &Policy) -> ReviewResult<()> {
    let principal_valid =
        |principal: &String| !principal.trim().is_empty() && principal.trim() == principal;
    if policy.id.trim().is_empty()
        || policy.minimum_approvals == 0
        || policy.minimum_approvals as usize > policy.eligible_reviewers.len()
        || policy.submitters.is_empty()
        || !policy.eligible_reviewers.iter().all(principal_valid)
        || !policy.submitters.iter().all(principal_valid)
    {
        return fail(
            "invalid_policy",
            "policy needs attributable reviewers, submitters and achievable approvals",
        );
    }
    for (team, members) in &policy.ownership.teams {
        if !principal_valid(team)
            || members.is_empty()
            || !members
                .iter()
                .all(|member| policy.eligible_reviewers.contains(member))
        {
            return fail(
                "invalid_policy",
                "ownership teams must name nonempty eligible membership",
            );
        }
    }
    if policy.ownership.classes.len() > MAX_CHANGE_CLASSES {
        return fail(
            "invalid_policy",
            "ownership change classes exceed their named bound",
        );
    }
    let mut classes = BTreeSet::new();
    for class in &policy.ownership.classes {
        if !principal_valid(&class.id)
            || class.id == "logic"
            || !classes.insert(class.id.as_str())
            || class.paths.is_empty()
            || class.paths.len() > MAX_CHANGE_CLASS_PATTERNS
            || !class.paths.iter().all(|path| paths::valid_pattern(path))
        {
            return fail(
                "invalid_policy",
                "ownership change classes need unique non-logic identities and valid bounded paths",
            );
        }
    }
    let mut ids = BTreeSet::new();
    for rule in &policy.ownership.rules {
        if !principal_valid(&rule.id)
            || !ids.insert(&rule.id)
            || rule.paths.is_empty()
            || !rule.paths.iter().all(|path| paths::valid_pattern(path))
            || !rule
                .owners
                .iter()
                .all(|owner| policy.eligible_reviewers.contains(owner))
            || !rule
                .teams
                .iter()
                .all(|team| policy.ownership.teams.contains_key(team))
            || rule_owners(policy, rule).next().is_none()
            || rule.target_refs.iter().any(|target| {
                !target.starts_with("refs/heads/")
                    || !paths::canonical(target)
                    || target.contains(['*', '?', '[', ']', ' ', '~', '^', ':'])
            })
        {
            return fail(
                "invalid_policy",
                "ownership rule needs unique identity, valid paths, known eligible owners and exact target refs",
            );
        }
    }
    for rule in &policy.ownership.rules {
        if rule.required_evidence.len() > MAX_OWNERSHIP_EVIDENCE {
            return fail(
                "invalid_policy",
                "ownership rule evidence exceeds its named bound",
            );
        }
        let mut projects = BTreeSet::new();
        for evidence in &rule.required_evidence {
            if evidence.project.trim().is_empty()
                || evidence.project.trim() != evidence.project
                || evidence.project.len() > 128
                || !projects.insert(evidence.project.as_str())
            {
                return fail(
                    "invalid_policy",
                    "ownership evidence needs a unique bounded project identity",
                );
            }
        }
        for (class, mode) in &rule.approval {
            if (class != "logic" && !classes.contains(class.as_str()))
                || (*mode == OwnershipApprovalMode::EvidenceOnly
                    && rule.required_evidence.is_empty())
            {
                return fail(
                    "invalid_policy",
                    "ownership approval must name a declared class and evidence-only needs evidence",
                );
            }
        }
    }
    optional_ci::validate(policy)?;
    Ok(())
}

fn applicable(rule: &OwnershipRule, change: &Change) -> bool {
    rule.target_refs.is_empty() || rule.target_refs.contains(&change.target_ref)
}

fn ownership_files<'a>(
    change: &Change,
    revision: &Revision,
    facts: &'a Facts,
) -> ReviewResult<&'a [ChangedFile]> {
    let Some(files) = facts
        .source
        .as_ref()
        .and_then(|source| source.changed_files.as_ref())
    else {
        return fail(
            "missing_changed_files",
            "ownership requires complete exact-source changed-file facts including rename paths",
        );
    };
    let paths: BTreeSet<_> = files.files.iter().map(|file| &file.path).collect();
    if !source_matches(change, revision, facts)
        || !files.complete
        || paths.len() != files.files.len()
        || paths != revision.files.iter().collect()
        || files.files.iter().any(|file| {
            !paths::canonical(&file.path)
                || file
                    .old_path
                    .as_ref()
                    .is_some_and(|path| !paths::canonical(path))
        })
    {
        return fail(
            "invalid_changed_files",
            "ownership changed-file facts must be complete and match the exact revision paths",
        );
    }
    Ok(&files.files)
}

/// The first ownership rule, in policy order, that applies to `path` on `target_ref`.
#[must_use]
pub fn owning_rule<'a>(
    ownership: &'a OwnershipPolicy,
    target_ref: &str,
    path: &str,
) -> Option<&'a str> {
    ownership
        .rules
        .iter()
        .find(|rule| {
            (rule.target_refs.is_empty() || rule.target_refs.contains(target_ref))
                && rule
                    .paths
                    .iter()
                    .any(|pattern| paths::matches(pattern, path))
        })
        .map(|rule| rule.id.as_str())
}
/// The first policy class whose patterns match every current and old changed
/// path. Empty or incomplete facts fall back to the reserved `logic` class.
fn change_class<'a>(ownership: &'a OwnershipPolicy, files: &[ChangedFile]) -> &'a str {
    if files.is_empty() {
        return "logic";
    }
    ownership
        .classes
        .iter()
        .find(|class| {
            files.iter().all(|file| {
                class
                    .paths
                    .iter()
                    .any(|pattern| paths::matches(pattern, &file.path))
                    && file.old_path.as_ref().is_none_or(|old| {
                        class
                            .paths
                            .iter()
                            .any(|pattern| paths::matches(pattern, old))
                    })
            })
        })
        .map_or("logic", |class| class.id.as_str())
}

fn rule_matches(rule: &OwnershipRule, files: &[ChangedFile]) -> bool {
    files.iter().any(|file| {
        rule.paths.iter().any(|pattern| {
            paths::matches(pattern, &file.path)
                || file
                    .old_path
                    .as_ref()
                    .is_some_and(|path| paths::matches(pattern, path))
        })
    })
}

fn automatic_reviewers(
    change: &Change,
    revision: &Revision,
    facts: &Facts,
) -> ReviewResult<BTreeSet<String>> {
    let mut reviewers = BTreeSet::new();
    let Some(policy) = &facts.policy else {
        return Ok(reviewers);
    };
    validate_policy(policy)?;
    if !policy
        .ownership
        .rules
        .iter()
        .any(|rule| applicable(rule, change))
    {
        return Ok(reviewers);
    }
    let files = ownership_files(change, revision, facts)?;
    for rule in &policy.ownership.rules {
        if applicable(rule, change) && rule_matches(rule, files) {
            reviewers.extend(
                rule_owners(policy, rule)
                    .filter(|principal| {
                        **principal != change.owner && **principal != revision.author
                    })
                    .cloned(),
            );
        }
    }
    Ok(reviewers)
}

fn integration_matches(revision: &Revision, target: &str, integration: &IntegrationFact) -> bool {
    integration.source_commit == revision.commit
        && integration.target_commit == target
        && !integration.candidate_commit.is_empty()
        && integration.source_is_ancestor
        && integration.target_is_ancestor
}

fn source_matches(change: &Change, revision: &Revision, facts: &Facts) -> bool {
    facts.source.as_ref().is_some_and(|source| {
        source.immutable
            && source.project == change.project
            && source.repository == change.repository
            && source.target_ref == change.target_ref
            && source.commit == revision.commit
            && source.base == revision.base
    })
}
fn revision_valid(
    state: &ReviewState,
    change: &Change,
    revision: &Revision,
    actor: &str,
    facts: &Facts,
) -> ReviewResult<()> {
    fresh(state, &revision.id)?;
    validate_authorship(&revision.authorship)?;
    if facts.agents.contains(actor) && revision.authorship == Authorship::Human {
        return fail(
            "agent_authorship_mismatch",
            "an agent principal cannot submit human-declared authorship",
        );
    }
    if revision.commit.is_empty()
        || revision.base.is_empty()
        || revision.author != actor
        || revision.created_ms != facts.now_ms
        || revision.number as usize != change.revisions.len() + 1
        || revision.files.iter().any(|file| file.is_empty())
        || revision.files.iter().collect::<BTreeSet<_>>().len() != revision.files.len()
        || change
            .revisions
            .iter()
            .any(|old| old.commit == revision.commit)
        || !source_matches(change, revision, facts)
    {
        return fail(
            "invalid_revision",
            "revision must be unique, sequential, immutable, and exactly source-bound",
        );
    }
    Ok(())
}
fn anchor_valid(change: &Change, anchor: &Anchor, facts: &Facts) -> ReviewResult<()> {
    let revision = change
        .revisions
        .iter()
        .find(|revision| revision.id == anchor.revision);
    let comparison_valid = revision.is_some_and(|revision| {
        anchor.base_commit == revision.base
            || change
                .revisions
                .iter()
                .any(|old| old.number < revision.number && old.commit == anchor.base_commit)
    });
    if anchor.start_line == 0
        || anchor.end_line < anchor.start_line
        || anchor.file.is_empty()
        || !comparison_valid
        || !facts.anchors.contains(anchor)
    {
        return fail(
            "invalid_anchor",
            "anchor is not an admitted exact revision/comparison/path/range",
        );
    }
    Ok(())
}
fn voter(change: &Change, actor: &str, facts: &Facts) -> ReviewResult<()> {
    open(change)?;
    if actor == change.owner
        || actor == change.current()?.author
        || change.reviewers.get(actor) != Some(&ReviewerRole::Reviewer)
        || !facts.policy.as_ref().is_some_and(|policy| {
            validate_policy(policy).is_ok() && policy.eligible_reviewers.contains(actor)
        })
    {
        return fail(
            "ineligible_vote",
            "votes require an eligible requested non-author reviewer",
        );
    }
    Ok(())
}
/// Advisory approvals of the change's current revision.
pub fn current_advisory(change: &Change) -> impl Iterator<Item = &AdvisoryApproval> {
    let current = change.revisions.last().map(|revision| revision.id.as_str());
    change
        .advisory
        .iter()
        .filter(move |advice| Some(advice.revision.as_str()) == current)
}
/// The owner and every revision author may not advise on their own change.
// r[impl kiln.review.advisory.scenario.forbidden]
fn advisory_admissible(change: &Change, actor: &str) -> ReviewResult<()> {
    open(change)?;
    if actor == change.owner
        || change
            .revisions
            .iter()
            .any(|revision| revision.author == actor)
    {
        return fail(
            "advisory_forbidden",
            "owners and authors may not advise on their own change",
        );
    }
    Ok(())
}
/// Record or withdraw one advisory approval of the exact current revision.
/// Touches no vote, attention, candidate, reviewer or inbox state, so no
/// checklist item, candidate or delivery recipient can change.
// r[impl kiln.review.advisory.authority]
fn advise(
    state: &mut ReviewState,
    actor: &str,
    change_id: &str,
    expected_revision: &str,
    approve: bool,
    facts: &Facts,
) -> ReviewResult<()> {
    let current = get(state, change_id, expected_revision)?;
    advisory_admissible(current, actor)?;
    let held = current_advisory(current).any(|advice| advice.actor == actor);
    if !approve && !held {
        return fail(
            "advisory_missing",
            "no advisory approval of the current revision to withdraw",
        );
    }
    if approve && !held && current_advisory(current).count() >= MAX_ADVISORY_PER_REVISION {
        return fail(
            "advisory_bound",
            "advisory approvals of this revision exceed their named bound",
        );
    }
    let change = state
        .changes
        .get_mut(change_id)
        .ok_or_else(|| ReviewError {
            code: "invalid_state".into(),
            message: "validated change".into(),
        })?;
    change
        .advisory
        .retain(|advice| advice.actor != actor || advice.revision != expected_revision);
    if approve {
        change.advisory.push(AdvisoryApproval {
            actor: actor.into(),
            revision: expected_revision.into(),
            created_ms: facts.now_ms,
        });
    }
    Ok(())
}
fn handoff_attention(change: &mut Change, actor: &str) {
    change.attention.remove(actor);
    if actor == change.owner {
        change.attention.extend(
            change
                .reviewers
                .iter()
                .filter(|(_, role)| **role == ReviewerRole::Reviewer)
                .map(|(principal, _)| principal.clone()),
        );
    } else {
        change.attention.insert(change.owner.clone());
    }
}
fn set_vote(change: &mut Change, actor: &str, value: VoteValue, now_ms: i64) -> ReviewResult<()> {
    let revision = change.current()?.id.clone();
    change
        .votes
        .retain(|vote| vote.actor != actor || vote.revision != revision);
    change.votes.push(Vote {
        actor: actor.into(),
        revision,
        value,
        created_ms: now_ms,
    });
    change.attention.remove(actor);
    change.attention.insert(change.owner.clone());
    change.candidate = None;
    Ok(())
}
fn dependency_valid(
    state: &ReviewState,
    change: &Change,
    dependencies: &[Dependency],
) -> ReviewResult<()> {
    let mut seen = BTreeSet::new();
    for dependency in dependencies {
        if dependency.change == change.id || !seen.insert(&dependency.change) {
            return fail("dependency_cycle", "self or duplicate dependency");
        }
        let dep = get(state, &dependency.change, &dependency.revision)?;
        if dep.state == ChangeStatus::Abandoned
            || dep.repository != change.repository
            || dep.project != change.project
            || dep.target_ref != change.target_ref
        {
            return fail(
                "invalid_dependency",
                "dependency must share project, repository, and target and not be abandoned",
            );
        }
        let mut pending = alloc::vec![dep.id.as_str()];
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if id == change.id {
                return fail("dependency_cycle", "dependency introduces a cycle");
            }
            if visited.insert(id) {
                if let Some(node) = state.changes.get(id) {
                    pending.extend(node.dependencies.iter().map(|edge| edge.change.as_str()));
                }
            }
        }
    }
    Ok(())
}

/// Dependency-first deterministic stack, including the selected change last.
/// Shared prerequisites appear once; every edge must bind the current revision.
pub fn stack<'a>(state: &'a ReviewState, change_id: &str) -> ReviewResult<Vec<&'a Change>> {
    let Some(root) = state.changes.get(change_id) else {
        return fail("not_found", "change does not exist");
    };
    root.current()?;
    let mut pending = alloc::vec![(root, false)];
    let mut active = BTreeSet::new();
    let mut finished = BTreeSet::new();
    let mut ordered = Vec::new();
    while let Some((change, exit)) = pending.pop() {
        if exit {
            active.remove(change.id.as_str());
            finished.insert(change.id.as_str());
            ordered.push(change);
        } else if !finished.contains(change.id.as_str()) {
            if !active.insert(change.id.as_str()) {
                return fail("dependency_cycle", "dependency graph contains a cycle");
            }
            pending.push((change, true));
            for edge in change.dependencies.iter().rev() {
                let dependency = get(state, &edge.change, &edge.revision)?;
                if dependency.state == ChangeStatus::Abandoned
                    || dependency.project != root.project
                    || dependency.repository != root.repository
                    || dependency.target_ref != root.target_ref
                {
                    return fail(
                        "invalid_dependency",
                        "stack dependency is not in the same admitted target",
                    );
                }
                pending.push((dependency, false));
            }
        }
    }
    Ok(ordered)
}

/// Re-admits prepared authority immediately before provider effects. A moved
/// target, policy, dependency, vote, thread or CI observation requires preparation
/// of a fresh candidate; a post-effect read-back is not a substitute for this step.
pub fn validate_candidate(
    state: &ReviewState,
    change_id: &str,
    expected_revision: &str,
    candidate_id: &str,
    actor: &str,
    facts: &Facts,
) -> ReviewResult<()> {
    let change = get(state, change_id, expected_revision)?;
    let Some(candidate) = &change.candidate else {
        return fail("stale_candidate", "no prepared candidate remains");
    };
    let Some(policy) = &facts.policy else {
        return fail("missing_policy", "submission requires explicit policy");
    };
    let Some(source) = &facts.source else {
        return fail(
            "missing_source",
            "submission requires fresh target observation",
        );
    };
    let checks = checklist(state, change_id, facts)?;
    if !checks.ready
        || !policy.submitters.contains(actor)
        || candidate.prepared_by != actor
        || candidate.id != candidate_id
        || candidate.revision != expected_revision
        || candidate.commit
            != facts
                .integration
                .as_ref()
                .map_or(change.current()?.commit.as_str(), |integration| {
                    integration.candidate_commit.as_str()
                })
        || candidate.integration != facts.integration
        || candidate.target_ref != change.target_ref
        || candidate.target != source.current_target
        || candidate.target_is_ancestor
            != facts
                .integration
                .as_ref()
                .map_or(source.target_is_ancestor, |integration| {
                    integration.target_is_ancestor
                })
        || candidate.dependencies != change.dependencies
        || candidate.policy_id != policy.id
        || candidate.ci_run != checks.ci_run
    {
        return fail(
            "stale_candidate",
            "prepared authority no longer matches the exact current checklist and target",
        );
    }
    Ok(())
}

pub fn checklist(state: &ReviewState, change_id: &str, facts: &Facts) -> ReviewResult<Checklist> {
    checklist_at(state, change_id, facts, None)
}
fn checklist_at(
    state: &ReviewState,
    change_id: &str,
    facts: &Facts,
    target: Option<(&str, bool)>,
) -> ReviewResult<Checklist> {
    let Some(change) = state.changes.get(change_id) else {
        return fail("not_found", "change does not exist");
    };
    if change.revisions.is_empty() {
        return fail("invalid_history", "change has no revision");
    }
    let revision = change.current()?;
    let policy = facts.policy.as_ref();
    let policy_valid = policy.is_some_and(|policy| validate_policy(policy).is_ok());
    let eligible = |vote: &&Vote| {
        vote.revision == revision.id
            && vote.actor != change.owner
            && vote.actor != revision.author
            && change.reviewers.get(&vote.actor) == Some(&ReviewerRole::Reviewer)
            && policy.is_some_and(|p| p.eligible_reviewers.contains(&vote.actor))
    };
    let agent_votes_bind = policy.is_some_and(|policy| policy.agent_votes == AgentVotes::Binding);
    let counts_for_approval =
        |vote: &&Vote| eligible(vote) && (agent_votes_bind || !facts.agents.contains(&vote.actor));
    let approvals = change
        .votes
        .iter()
        .filter(counts_for_approval)
        .filter(|vote| vote.value == VoteValue::Approve)
        .count();
    let ci_commit = facts
        .integration
        .as_ref()
        .map_or(revision.commit.as_str(), |integration| {
            integration.candidate_commit.as_str()
        });
    // r[impl kiln.monorepo.gating]
    // Monorepo reviews require the latest full clean run for the exact commit
    // of every affected project, bound to that project's current scope
    // identity; skipped projects never block and other scopes never satisfy.
    let latest_for = |project: &str, scope: Option<Option<&str>>| {
        facts
            .ci
            .iter()
            .filter(|ci| {
                ci.project == project
                    && ci.repository == change.repository
                    && ci.revision == ci_commit
                    && ci.full_run
                    && ci.clean
                    && !ci.run_id.is_empty()
                    && scope.is_none_or(|scope| ci.scope_identity.as_deref() == scope)
            })
            .max_by_key(|ci| ci.ordinal)
    };
    let (latest, required_ok) = match &facts.ci_required {
        None => {
            let latest = latest_for(&change.project, None);
            (
                latest,
                latest.is_some_and(CiFact::satisfies_required)
                    || (latest.is_none()
                        && optional_ci::waived(policy, &change.project, change, revision, facts)),
            )
        }
        Some(required) => {
            let mut all = true;
            let mut primary = None;
            let mut first = None;
            for requirement in required {
                let latest = latest_for(
                    &requirement.project,
                    Some(requirement.scope_identity.as_deref()),
                );
                // r[impl kiln.triggers.required]
                all &= latest.is_some_and(CiFact::satisfies_required)
                    || (latest.is_none()
                        && optional_ci::waived(
                            policy,
                            &requirement.project,
                            change,
                            revision,
                            facts,
                        ));
                first = first.or(latest);
                if requirement.project == change.project {
                    primary = latest;
                }
            }
            (primary.or(first), all)
        }
    };
    let ci_ok = policy.is_some_and(|p| !p.require_ci || required_ok);
    let dependencies_ok = dependency_valid(state, change, &change.dependencies).is_ok()
        && change.dependencies.iter().all(|dep| {
            state.changes.get(&dep.change).is_some_and(|node| {
                node.state == ChangeStatus::Submitted
                    && node.current().is_ok_and(|revision| {
                        revision.id == dep.revision
                            && facts.source.as_ref().is_some_and(|source| {
                                source.target_ancestors.contains(&revision.commit)
                            })
                    })
            })
        });
    let source_ok = source_matches(change, revision, facts)
        && facts.source.as_ref().is_some_and(|source| {
            let (target, target_is_ancestor) =
                target.unwrap_or((&source.current_target, source.target_is_ancestor));
            if let Some(integration) = &facts.integration {
                integration_matches(revision, target, integration)
            } else {
                target_is_ancestor
                    && (target == revision.base
                        || change.dependencies.iter().any(|edge| {
                            state.changes.get(&edge.change).is_some_and(|dependency| {
                                dependency.state == ChangeStatus::Submitted
                                    && dependency.current().is_ok_and(|current| {
                                        current.id == edge.revision && current.commit == target
                                    })
                            })
                        }))
            }
        });
    let mut items = Vec::new();
    for (key, satisfied, detail) in [
        ("policy", policy_valid, "explicit nonzero review policy"),
        ("open", change.state == ChangeStatus::Open, "change is open"),
        ("ready", !change.wip, "work in progress is disabled"),
        (
            "approvals",
            policy.is_some_and(|p| approvals >= p.minimum_approvals as usize),
            "eligible non-author current-revision approvals",
        ),
        (
            "vetoes",
            !change
                .votes
                .iter()
                .filter(eligible)
                .any(|vote| matches!(vote.value, VoteValue::Veto | VoteValue::NeedsRevision)),
            "no current veto or needs-revision vote",
        ),
        (
            "threads",
            !change
                .threads
                .iter()
                .any(|thread| thread.blocking && !thread.resolved),
            "all blocking threads are resolved, including historical anchors",
        ),
        (
            "dependencies",
            dependencies_ok,
            "dependencies are current and submitted",
        ),
        (
            "source_target",
            source_ok,
            "immutable reviewed source and exact target, or proven source-and-target integration candidate",
        ),
        (
            "ci",
            ci_ok,
            if policy.is_some_and(|policy| !policy.require_ci) {
                "CI is not required by this policy; no successful run is inferred"
            } else {
                "latest full clean CI for the exact submitted commit satisfies policy"
            },
        ),
    ] {
        items.push(ChecklistItem {
            key: key.into(),
            satisfied,
            detail: detail.into(),
        });
    }
    if let Some(policy) = policy {
        if policy
            .ownership
            .rules
            .iter()
            .any(|rule| applicable(rule, change))
        {
            match ownership_files(change, revision, facts) {
                Err(error) => items.push(ChecklistItem {
                    key: "ownership_files".into(),
                    satisfied: false,
                    detail: error.message,
                }),
                Ok(files) => {
                    let class = change_class(&policy.ownership, files);
                    for rule in &policy.ownership.rules {
                        if !applicable(rule, change) || !rule_matches(rule, files) {
                            continue;
                        }
                        let approved =
                            change.votes.iter().filter(counts_for_approval).any(|vote| {
                                vote.value == VoteValue::Approve
                                    && rule_owners(policy, rule).any(|owner| owner == &vote.actor)
                            });
                        let mode = rule
                            .approval
                            .get(class)
                            .copied()
                            .unwrap_or(OwnershipApprovalMode::OwnerApproval);
                        let approval_required = mode == OwnershipApprovalMode::OwnerApproval;
                        items.push(ChecklistItem {
                            key: alloc::format!("owner:{}", rule.id),
                            satisfied: policy_valid && (!approval_required || approved),
                            detail: alloc::format!(
                                "ownership rule {} for changed paths {:?}, class {}{}: one eligible requested non-author owner from {:?} / teams {:?} {} exact revision {}",
                                rule.id,
                                rule.paths,
                                class,
                                if approval_required {
                                    " requires owner approval;"
                                } else {
                                    " explicitly accepts evidence instead of owner approval;"
                                },
                                rule.owners,
                                rule.teams,
                                if approval_required {
                                    "must approve"
                                } else {
                                    "is not required to approve"
                                },
                                revision.id
                            ),
                        });
                        // r[impl kiln.review.evidence.project_ci]
                        for evidence in &rule.required_evidence {
                            let scope = if let Some(required) = &facts.ci_required {
                                required
                                    .iter()
                                    .find(|required| required.project == evidence.project)
                                    .map(|required| Some(required.scope_identity.as_deref()))
                            } else if evidence.project == change.project {
                                Some(None)
                            } else {
                                None
                            };
                            let observed =
                                scope.and_then(|scope| latest_for(&evidence.project, scope));
                            let satisfied =
                                policy_valid && observed.is_some_and(CiFact::satisfies_required);
                            items.push(ChecklistItem {
                                key: alloc::format!(
                                    "evidence:{}:project_ci:{}",
                                    rule.id,
                                    evidence.project
                                ),
                                satisfied,
                                detail: alloc::format!(
                                    "ownership rule {} requires full clean CI for project {} at exact commit {} and its current scope identity",
                                    rule.id, evidence.project, ci_commit
                                ),
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(Checklist {
        ready: items.iter().all(|item| item.satisfied),
        items,
        ci_run: latest.map(|ci| ci.run_id.clone()),
    })
}

fn notify(state: &mut ReviewState, change_id: &str, actor: &str, kind: &str, facts: &Facts) {
    let Some(change) = state.changes.get(change_id) else {
        return;
    };
    let mut recipients: BTreeSet<_> = change.reviewers.keys().cloned().collect();
    recipients.insert(change.owner.clone());
    recipients.extend(change.attention.iter().cloned());
    recipients.remove(actor);
    let Ok(revision) = change.current() else {
        return;
    };
    let revision = revision.id.clone();
    for recipient in recipients {
        state
            .inbox
            .entry(recipient)
            .or_default()
            .push(Notification {
                id: facts.event_id.clone(),
                change: change_id.into(),
                revision: revision.clone(),
                actor: actor.into(),
                kind: kind.into(),
                created_ms: facts.now_ms,
                read: false,
            });
    }
}

/// Atomically applies one admitted command. Failure does not modify state. The
/// shell must persist the result before effects and never serialize state to clients.
pub fn apply(
    state: &mut ReviewState,
    actor: &str,
    command: Command,
    facts: &Facts,
) -> ReviewResult<()> {
    if actor.trim().is_empty() {
        return fail("unauthenticated", "an attributable principal is required");
    }
    fresh(state, &facts.event_id)?;
    let notification: Option<(String, &str)>;
    match command {
        Command::Create {
            id,
            project,
            repository,
            target_ref,
            title,
            description,
            wip,
            mut revision,
        } => {
            revision.submitted_by = facts.agents.contains(actor).then(|| AttestedSubmission {
                principal: actor.into(),
                agent: facts.agent_identities.get(actor).cloned(),
            });
            fresh(state, &id)?;
            body_valid(&title)?;
            if project.is_empty()
                || repository.is_empty()
                || target_ref.is_empty()
                || id == revision.id
                || id == facts.event_id
                || revision.id == facts.event_id
            {
                return fail(
                    "invalid_change",
                    "change source and distinct identities are required",
                );
            }
            let mut change = Change {
                id: id.clone(),
                project,
                repository,
                target_ref,
                owner: actor.into(),
                title,
                description,
                state: ChangeStatus::Open,
                wip,
                revisions: Vec::new(),
                reviewers: BTreeMap::new(),
                votes: Vec::new(),
                threads: Vec::new(),
                reviewed: Vec::new(),
                reviewed_hunks: Vec::new(),
                last_viewed: BTreeMap::new(),
                attention: BTreeSet::new(),
                dependencies: Vec::new(),
                candidate: None,
                submission: None,
                maintenance: Vec::new(),
                advisory: Vec::new(),
            };
            revision_valid(state, &change, &revision, actor, facts)?;
            let reviewers = automatic_reviewers(&change, &revision, facts)?;
            change.attention.extend(reviewers.iter().cloned());
            change.reviewers.extend(
                reviewers
                    .into_iter()
                    .map(|actor| (actor, ReviewerRole::Reviewer)),
            );
            state.used_ids.insert(revision.id.clone());
            change.revisions.push(revision);
            state.used_ids.insert(id.clone());
            state.changes.insert(id.clone(), change);
            notification = Some((id, "review_requested"));
        }
        Command::Upload {
            change,
            expected_revision,
            mut revision,
        } => {
            revision.submitted_by = facts.agents.contains(actor).then(|| AttestedSubmission {
                principal: actor.into(),
                agent: facts.agent_identities.get(actor).cloned(),
            });
            let current = get(state, &change, &expected_revision)?;
            owner(current, actor)?;
            open(current)?;
            revision_valid(state, current, &revision, actor, facts)?;
            let reviewers = automatic_reviewers(current, &revision, facts)?;
            if revision.id == facts.event_id {
                return fail(
                    "duplicate_identity",
                    "revision and event identities must differ",
                );
            }
            state.used_ids.insert(revision.id.clone());
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            let new_revision = revision.id.clone();
            current.revisions.push(revision);
            current.reviewers.extend(
                reviewers
                    .into_iter()
                    .map(|actor| (actor, ReviewerRole::Reviewer)),
            );
            // Legacy file markers keep their meaning on their own revision only.
            current.reviewed.clear();
            carry_forward(current, &expected_revision, &new_revision, facts);
            current.candidate = None;
            current.attention = current
                .reviewers
                .iter()
                .filter(|(_, role)| **role == ReviewerRole::Reviewer)
                .map(|(actor, _)| actor.clone())
                .collect();
            notification = Some((change, "revision_uploaded"));
        }
        Command::Edit {
            change,
            expected_revision,
            title,
            description,
        } => {
            let current = get(state, &change, &expected_revision)?;
            owner(current, actor)?;
            open(current)?;
            body_valid(&title)?;
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            current.title = title;
            current.description = description;
            current.candidate = None;
            notification = Some((change, "metadata_updated"));
        }
        Command::SetWip {
            change,
            expected_revision,
            wip,
        } => {
            let current = get(state, &change, &expected_revision)?;
            owner(current, actor)?;
            open(current)?;
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            current.wip = wip;
            current.candidate = None;
            if !wip {
                current.attention.extend(
                    current
                        .reviewers
                        .iter()
                        .filter(|(_, role)| **role == ReviewerRole::Reviewer)
                        .map(|(actor, _)| actor.clone()),
                );
            }
            notification = Some((change, "readiness_changed"));
        }
        Command::Abandon {
            change,
            expected_revision,
        } => {
            let current = get(state, &change, &expected_revision)?;
            owner(current, actor)?;
            open(current)?;
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            current.state = ChangeStatus::Abandoned;
            current.candidate = None;
            current.attention.clear();
            notification = Some((change, "abandoned"));
        }
        Command::Restore {
            change,
            expected_revision,
        } => {
            let current = get(state, &change, &expected_revision)?;
            owner(current, actor)?;
            if current.state != ChangeStatus::Abandoned {
                return fail(
                    "invalid_transition",
                    "only abandoned changes can be restored",
                );
            }
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            current.state = ChangeStatus::Open;
            current.candidate = None;
            notification = Some((change, "restored"));
        }
        Command::SetReviewer {
            change,
            expected_revision,
            principal,
            role,
        } => {
            let current = get(state, &change, &expected_revision)?;
            owner(current, actor)?;
            open(current)?;
            if principal.trim().is_empty() || principal == current.owner {
                return fail(
                    "invalid_reviewer",
                    "review role needs a non-author principal",
                );
            }
            if role == Some(ReviewerRole::Reviewer)
                && !facts
                    .policy
                    .as_ref()
                    .is_some_and(|policy| policy.eligible_reviewers.contains(&principal))
            {
                return fail(
                    "ineligible_reviewer",
                    "reviewer is not eligible in the explicit policy",
                );
            }
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            match role {
                Some(role) => {
                    current.reviewers.insert(principal.clone(), role);
                }
                None => {
                    current.reviewers.remove(&principal);
                }
            }
            current.votes.retain(|vote| vote.actor != principal);
            current.attention.remove(&principal);
            if role == Some(ReviewerRole::Reviewer) {
                current.attention.insert(principal);
            }
            current.candidate = None;
            notification = Some((change, "reviewers_changed"));
        }
        Command::Vote {
            change,
            expected_revision,
            value,
        } => {
            let current = get(state, &change, &expected_revision)?;
            voter(current, actor, facts)?;
            set_vote(
                state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                    code: "invalid_state".into(),
                    message: "validated change".into(),
                })?,
                actor,
                value,
                facts.now_ms,
            )?;
            notification = Some((change, "vote"));
        }
        Command::StartThread {
            change,
            expected_revision,
            id,
            comment_id,
            anchor,
            body,
            blocking,
        } => {
            let current = get(state, &change, &expected_revision)?;
            open(current)?;
            anchor_valid(current, &anchor, facts)?;
            body_valid(&body)?;
            fresh(state, &id)?;
            fresh(state, &comment_id)?;
            if id == comment_id || id == facts.event_id || comment_id == facts.event_id {
                return fail(
                    "duplicate_identity",
                    "thread, comment and event identities must differ",
                );
            }
            state.used_ids.insert(id.clone());
            state.used_ids.insert(comment_id.clone());
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            current.threads.push(Thread {
                id,
                anchor,
                blocking,
                resolved: false,
                comments: alloc::vec![Comment {
                    id: comment_id,
                    author: actor.into(),
                    body,
                    created_ms: facts.now_ms
                }],
            });
            handoff_attention(current, actor);
            current.candidate = None;
            notification = Some((change, "thread_started"));
        }
        Command::Reply {
            change,
            expected_revision,
            thread,
            comment_id,
            body,
        } => {
            let current = get(state, &change, &expected_revision)?;
            open(current)?;
            body_valid(&body)?;
            fresh(state, &comment_id)?;
            if !current.threads.iter().any(|item| item.id == thread) {
                return fail("not_found", "thread does not exist");
            }
            if comment_id == facts.event_id {
                return fail(
                    "duplicate_identity",
                    "comment and event identities must differ",
                );
            }
            state.used_ids.insert(comment_id.clone());
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            let thread = current
                .threads
                .iter_mut()
                .find(|item| item.id == thread)
                .ok_or_else(|| ReviewError {
                    code: "invalid_state".into(),
                    message: "validated thread".into(),
                })?;
            thread.comments.push(Comment {
                id: comment_id,
                author: actor.into(),
                body,
                created_ms: facts.now_ms,
            });
            handoff_attention(current, actor);
            current.candidate = None;
            notification = Some((change, "reply"));
        }
        Command::Resolve {
            change,
            expected_revision,
            thread,
            resolved,
        } => {
            let current = get(state, &change, &expected_revision)?;
            open(current)?;
            let Some(item) = current.threads.iter().find(|item| item.id == thread) else {
                return fail("not_found", "thread does not exist");
            };
            if current.owner != actor
                && item
                    .comments
                    .first()
                    .is_none_or(|comment| comment.author != actor)
            {
                return fail(
                    "forbidden",
                    "only owner or thread author may resolve or reopen",
                );
            }
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            current
                .threads
                .iter_mut()
                .find(|item| item.id == thread)
                .ok_or_else(|| ReviewError {
                    code: "invalid_state".into(),
                    message: "validated thread".into(),
                })?
                .resolved = resolved;
            current.candidate = None;
            notification = Some((change, "thread_resolution"));
        }
        Command::SaveDraft {
            change,
            expected_revision,
            draft,
        } => {
            let current = get(state, &change, &expected_revision)?;
            open(current)?;
            anchor_valid(current, &draft.anchor, facts)?;
            body_valid(&draft.body)?;
            if draft.author != actor || draft.change != change || draft.id == facts.event_id {
                return fail(
                    "invalid_draft",
                    "draft is bound to its author and exact change anchor",
                );
            }
            if let Some(thread) = &draft.thread {
                if !current
                    .threads
                    .iter()
                    .any(|item| &item.id == thread && item.anchor == draft.anchor)
                {
                    return fail(
                        "invalid_thread",
                        "reply draft anchor must match existing thread",
                    );
                }
            }
            let exists = state
                .drafts
                .get(actor)
                .is_some_and(|drafts| drafts.iter().any(|item| item.id == draft.id));
            if !exists {
                fresh(state, &draft.id)?;
            }
            state.used_ids.insert(draft.id.clone());
            let drafts = state.drafts.entry(actor.into()).or_default();
            if let Some(item) = drafts.iter_mut().find(|item| item.id == draft.id) {
                *item = draft;
            } else {
                drafts.push(draft);
            }
            notification = None;
        }
        Command::DeleteDraft { id } => {
            if !state
                .drafts
                .get(actor)
                .is_some_and(|drafts| drafts.iter().any(|draft| draft.id == id))
            {
                return fail("not_found", "private draft does not exist");
            }
            state
                .drafts
                .get_mut(actor)
                .ok_or_else(|| ReviewError {
                    code: "invalid_state".into(),
                    message: "validated drafts".into(),
                })?
                .retain(|draft| draft.id != id);
            notification = None;
        }
        Command::Publish {
            change,
            expected_revision,
            draft_ids,
            vote,
        } => {
            let current = get(state, &change, &expected_revision)?;
            open(current)?;
            if draft_ids.is_empty() && vote.is_none() {
                return fail(
                    "empty_publication",
                    "publication must contain drafts or a vote",
                );
            }
            if draft_ids.iter().collect::<BTreeSet<_>>().len() != draft_ids.len() {
                return fail("duplicate_draft", "publication repeats a draft");
            }
            if vote.is_some() {
                voter(current, actor, facts)?;
            }
            for id in &draft_ids {
                let Some(draft) = state
                    .drafts
                    .get(actor)
                    .and_then(|drafts| drafts.iter().find(|draft| &draft.id == id))
                else {
                    return fail("not_found", "private draft does not exist");
                };
                if draft.change != change {
                    return fail("invalid_draft", "draft belongs to a different change");
                }
                anchor_valid(current, &draft.anchor, facts)?;
                if let Some(thread) = &draft.thread {
                    if !current
                        .threads
                        .iter()
                        .any(|item| &item.id == thread && item.anchor == draft.anchor)
                    {
                        return fail(
                            "invalid_thread",
                            "reply draft thread or anchor no longer matches",
                        );
                    }
                }
            }
            let drafts = state.drafts.entry(actor.into()).or_default();
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            for id in draft_ids {
                let Some(index) = drafts.iter().position(|draft| draft.id == id) else {
                    return fail("invalid_state", "validated private draft is missing");
                };
                // Position comes from this exact vector with no intervening mutation.
                let draft = drafts.remove(index);
                let comment = Comment {
                    id: draft.id.clone(),
                    author: actor.into(),
                    body: draft.body,
                    created_ms: facts.now_ms,
                };
                if let Some(thread_id) = draft.thread {
                    current
                        .threads
                        .iter_mut()
                        .find(|thread| thread.id == thread_id)
                        .ok_or_else(|| ReviewError {
                            code: "invalid_state".into(),
                            message: "validated thread".into(),
                        })?
                        .comments
                        .push(comment);
                } else {
                    current.threads.push(Thread {
                        id: draft.id,
                        anchor: draft.anchor,
                        blocking: draft.blocking,
                        resolved: false,
                        comments: alloc::vec![comment],
                    });
                }
            }
            if let Some(value) = vote {
                set_vote(current, actor, value, facts.now_ms)?;
            }
            current.candidate = None;
            handoff_attention(current, actor);
            notification = Some((change, "review_published"));
        }
        // r[impl kiln.review.hunks]
        // File-level "mark all": writes or removes a marker for every hunk fact of the file.
        Command::MarkReviewed {
            change,
            expected_revision,
            file,
            reviewed,
        } => {
            let current = get(state, &change, &expected_revision)?;
            let digests = file_hunks(current, &file, facts)?;
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            current.reviewed.retain(|marker| {
                !(marker.actor == actor
                    && marker.revision == expected_revision
                    && marker.file == file)
            });
            current.reviewed_hunks.retain(|marker| {
                !(marker.actor == actor
                    && marker.revision == expected_revision
                    && marker.file == file)
            });
            if reviewed {
                for hunk_digest in digests {
                    current.reviewed_hunks.push(ReviewedHunk {
                        actor: actor.into(),
                        revision: expected_revision.clone(),
                        file: file.clone(),
                        hunk_digest,
                    });
                }
                current.last_viewed.insert(actor.into(), expected_revision);
            }
            notification = None;
        }
        Command::MarkHunk {
            change,
            expected_revision,
            file,
            hunk_digest,
            reviewed,
        } => {
            let current = get(state, &change, &expected_revision)?;
            if !file_hunks(current, &file, facts)?.contains(&hunk_digest) {
                return fail("invalid_hunk", "hunk is not in the current revision");
            }
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            let legacy = current.reviewed.iter().position(|marker| {
                marker.actor == actor && marker.revision == expected_revision && marker.file == file
            });
            if let Some(index) = legacy {
                // Unmarking one hunk of a legacy whole-file marker keeps the others viewed.
                current.reviewed.remove(index);
                for other in file_hunks_of(facts, &file) {
                    current.reviewed_hunks.push(ReviewedHunk {
                        actor: actor.into(),
                        revision: expected_revision.clone(),
                        file: file.clone(),
                        hunk_digest: other,
                    });
                }
            }
            current.reviewed_hunks.retain(|marker| {
                !(marker.actor == actor
                    && marker.revision == expected_revision
                    && marker.file == file
                    && marker.hunk_digest == hunk_digest)
            });
            if reviewed {
                current.reviewed_hunks.push(ReviewedHunk {
                    actor: actor.into(),
                    revision: expected_revision.clone(),
                    file,
                    hunk_digest,
                });
                current.last_viewed.insert(actor.into(), expected_revision);
            }
            notification = None;
        }
        Command::SetAttention {
            change,
            expected_revision,
            principal,
            attention,
        } => {
            let current = get(state, &change, &expected_revision)?;
            open(current)?;
            if (actor != current.owner && actor != principal)
                || (principal != current.owner && !current.reviewers.contains_key(&principal))
            {
                return fail(
                    "forbidden",
                    "attention may target only participants and be changed by owner or self",
                );
            }
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            if attention {
                current.attention.insert(principal);
            } else {
                current.attention.remove(&principal);
            }
            notification = Some((change, "attention_changed"));
        }
        Command::SetDependencies {
            change,
            expected_revision,
            dependencies,
        } => {
            let current = get(state, &change, &expected_revision)?;
            owner(current, actor)?;
            open(current)?;
            dependency_valid(state, current, &dependencies)?;
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            current.dependencies = dependencies;
            current.candidate = None;
            notification = Some((change, "dependencies_changed"));
        }
        Command::SaveQuery { query } => {
            body_valid(&query.name)?;
            let exists = state
                .saved_queries
                .get(actor)
                .is_some_and(|queries| queries.iter().any(|saved| saved.id == query.id));
            if !exists {
                fresh(state, &query.id)?;
            }
            if query.id == facts.event_id {
                return fail(
                    "duplicate_identity",
                    "query and event identities must differ",
                );
            }
            state.used_ids.insert(query.id.clone());
            let queries = state.saved_queries.entry(actor.into()).or_default();
            if let Some(saved) = queries.iter_mut().find(|saved| saved.id == query.id) {
                *saved = query;
            } else {
                queries.push(query);
            }
            notification = None;
        }
        Command::DeleteQuery { id } => {
            if !state
                .saved_queries
                .get(actor)
                .is_some_and(|queries| queries.iter().any(|saved| saved.id == id))
            {
                return fail("not_found", "private query does not exist");
            }
            state
                .saved_queries
                .get_mut(actor)
                .ok_or_else(|| ReviewError {
                    code: "invalid_state".into(),
                    message: "validated queries".into(),
                })?
                .retain(|saved| saved.id != id);
            notification = None;
        }
        Command::ReadNotification { id, read } => {
            let Some(item) = state
                .inbox
                .get_mut(actor)
                .and_then(|inbox| inbox.iter_mut().find(|item| item.id == id))
            else {
                return fail("not_found", "principal inbox event does not exist");
            };
            item.read = read;
            notification = None;
        }
        Command::PrepareSubmit {
            change,
            expected_revision,
            candidate_id,
        } => {
            let current = get(state, &change, &expected_revision)?;
            if !facts
                .policy
                .as_ref()
                .is_some_and(|policy| policy.submitters.contains(actor))
            {
                return fail("forbidden", "principal is not an explicit policy submitter");
            }
            fresh(state, &candidate_id)?;
            if candidate_id == facts.event_id {
                return fail(
                    "duplicate_identity",
                    "candidate and event identities must differ",
                );
            }
            let checks = checklist(state, &change, facts)?;
            if !checks.ready {
                return fail("submit_blocked", "submit checklist is not satisfied");
            }
            let source = facts.source.as_ref().ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "checklist admitted source".into(),
            })?;
            let policy = facts.policy.as_ref().ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "checklist admitted policy".into(),
            })?;
            let candidate = Candidate {
                id: candidate_id.clone(),
                revision: expected_revision,
                commit: facts.integration.as_ref().map_or_else(
                    || current.current().map(|revision| revision.commit.clone()),
                    |integration| Ok(integration.candidate_commit.clone()),
                )?,
                target_ref: current.target_ref.clone(),
                target: source.current_target.clone(),
                target_is_ancestor: facts
                    .integration
                    .as_ref()
                    .map_or(source.target_is_ancestor, |integration| {
                        integration.target_is_ancestor
                    }),
                dependencies: current.dependencies.clone(),
                policy_id: policy.id.clone(),
                ci_run: checks.ci_run,
                prepared_by: actor.into(),
                integration: facts.integration.clone(),
            };
            state.used_ids.insert(candidate_id);
            state
                .changes
                .get_mut(&change)
                .ok_or_else(|| ReviewError {
                    code: "invalid_state".into(),
                    message: "validated change".into(),
                })?
                .candidate = Some(candidate);
            notification = None;
        }
        Command::RecordSubmit {
            change,
            expected_revision,
            candidate_id,
            expected_target,
            result_commit,
        } => {
            let current = get(state, &change, &expected_revision)?;
            let Some(candidate) = &current.candidate else {
                return fail("stale_candidate", "no prepared candidate remains");
            };
            let Some(policy) = &facts.policy else {
                return fail("missing_policy", "submission requires explicit policy");
            };
            let Some(source) = &facts.source else {
                return fail(
                    "missing_source",
                    "submission requires exact provider read-back",
                );
            };
            let checks = checklist_at(
                state,
                &change,
                facts,
                Some((&expected_target, candidate.target_is_ancestor)),
            )?;
            if !checks.ready
                || !policy.submitters.contains(actor)
                || candidate.prepared_by != actor
                || candidate.id != candidate_id
                || candidate.revision != expected_revision
                || candidate.commit != result_commit
                || candidate.integration != facts.integration
                || candidate.commit
                    != facts
                        .integration
                        .as_ref()
                        .map_or(current.current()?.commit.as_str(), |integration| {
                            integration.candidate_commit.as_str()
                        })
                || (candidate.integration.is_some() && source.current_target != result_commit)
                || candidate.target != expected_target
                || candidate.target_ref != current.target_ref
                || candidate.dependencies != current.dependencies
                || candidate.policy_id != policy.id
                || candidate.ci_run != checks.ci_run
                || source.current_target.is_empty()
                || !source.target_contains_commit
            {
                return fail(
                    "stale_candidate",
                    "candidate, target, policy, dependencies, CI or exact provider read-back changed",
                );
            }
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            let candidate = current.candidate.take().ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated candidate".into(),
            })?;
            current.submission = Some(SubmitOutcome {
                candidate,
                result_commit,
                submitted_by: actor.into(),
                submitted_ms: facts.now_ms,
            });
            current.state = ChangeStatus::Submitted;
            current.attention.clear();
            notification = Some((change, "submitted"));
        }
        Command::RecordMaintenance {
            change,
            expected_revision,
            outcome,
        } => {
            let current = get(state, &change, &expected_revision)?;
            owner(current, actor)?;
            fresh(state, &outcome.operation_id)?;
            if outcome.actor != actor
                || !current
                    .revisions
                    .iter()
                    .any(|revision| revision.id == outcome.source_revision)
                || outcome.created_ms != facts.now_ms
                || outcome.operation_id == facts.event_id
            {
                return fail(
                    "invalid_maintenance",
                    "maintenance outcome must bind actor, historical source revision, event and operation",
                );
            }
            let result = get(state, &outcome.result_change, &outcome.result_revision)?;
            if result.current()?.commit != outcome.result_commit
                || result.owner != actor
                || result.state != ChangeStatus::Open
                || result.repository != current.repository
                || result.project != current.project
            {
                return fail(
                    "invalid_maintenance",
                    "maintenance must produce a registered open exact review revision",
                );
            }
            match outcome.kind {
                MaintenanceKind::Rebase => {
                    if outcome.result_change != change
                        || outcome.result_revision != expected_revision
                        || outcome.source_revision == expected_revision
                        || current
                            .revisions
                            .iter()
                            .rev()
                            .nth(1)
                            .is_none_or(|revision| revision.id != outcome.source_revision)
                    {
                        return fail(
                            "invalid_maintenance",
                            "rebase must register the immediate new revision on the same change",
                        );
                    }
                }
                MaintenanceKind::Revert => {
                    if current.state != ChangeStatus::Submitted
                        || outcome.source_revision != expected_revision
                        || outcome.result_change == change
                        || result.target_ref != current.target_ref
                    {
                        return fail(
                            "invalid_maintenance",
                            "revert creates a separate review on the submitted change target",
                        );
                    }
                }
                MaintenanceKind::CherryPick => {
                    if outcome.source_revision != expected_revision
                        || outcome.result_change == change
                        || result.target_ref == current.target_ref
                    {
                        return fail(
                            "invalid_maintenance",
                            "cherry-pick creates a separate review on another target",
                        );
                    }
                }
            }
            state.used_ids.insert(outcome.operation_id.clone());
            state
                .changes
                .get_mut(&change)
                .ok_or_else(|| ReviewError {
                    code: "invalid_state".into(),
                    message: "validated change".into(),
                })?
                .maintenance
                .push(outcome);
            notification = Some((change, "maintenance_registered"));
        }
        Command::AdvisoryApprove {
            change,
            expected_revision,
        } => {
            advise(state, actor, &change, &expected_revision, true, facts)?;
            notification = None;
        }
        Command::AdvisoryWithdraw {
            change,
            expected_revision,
        } => {
            advise(state, actor, &change, &expected_revision, false, facts)?;
            notification = None;
        }
    }
    state.used_ids.insert(facts.event_id.clone());
    if let Some((change, kind)) = notification {
        notify(state, &change, actor, kind, facts);
    }
    Ok(())
}

#[cfg(test)]
#[path = "change_review_hunks_tests.rs"]
mod hunks_tests;

#[cfg(test)]
#[path = "change_review_advisory_tests.rs"]
mod advisory_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::vec;

    trait Required<T> {
        fn required(self) -> ReviewResult<T>;
    }
    impl<T> Required<T> for Option<T> {
        fn required(self) -> ReviewResult<T> {
            self.ok_or_else(|| ReviewError {
                code: "missing_fixture".into(),
                message: "expected test fixture value".into(),
            })
        }
    }
    impl<T> Required<T> for ReviewResult<T> {
        fn required(self) -> ReviewResult<T> {
            self
        }
    }

    fn facts() -> Facts {
        Facts {
            now_ms: 0,
            event_id: String::new(),
            policy: Some(Policy {
                id: "policy-1".into(),
                eligible_reviewers: ["bob".into(), "carol".into()].into_iter().collect(),
                submitters: ["alice".into()].into_iter().collect(),
                minimum_approvals: 1,
                require_ci: true,
                ownership: OwnershipPolicy::default(),
                optional_ci: Vec::new(),
                agent_votes: AgentVotes::Advisory,
            }),
            source: Some(SourceFact {
                project: "project".into(),
                repository: "source-repository".into(),
                target_ref: "refs/heads/main".into(),
                commit: "commit-c".into(),
                base: "base".into(),
                current_target: "base".into(),
                immutable: true,
                target_contains_commit: false,
                target_is_ancestor: true,
                target_ancestors: BTreeSet::new(),
                changed_files: None,
            }),
            ci: vec![CiFact {
                project: "project".into(),
                repository: "source-repository".into(),
                revision: "commit-c".into(),
                run_id: "run-1".into(),
                ordinal: 1,
                success: true,
                full_run: true,
                clean: true,
                scope_identity: None,
                terminal: true,
                untrusted: None,
            }],
            anchors: vec![Anchor {
                revision: "c-r1".into(),
                base_commit: "base".into(),
                file: "src/lib.rs".into(),
                side: Side::Revision,
                start_line: 2,
                end_line: 3,
            }],
            integration: None,
            ci_required: None,
            hunks: None,
            agents: BTreeSet::new(),
            agent_identities: BTreeMap::new(),
        }
    }
    fn send(
        state: &mut ReviewState,
        facts: &mut Facts,
        actor: &str,
        mut command: Command,
    ) -> ReviewResult<()> {
        facts.now_ms += 1;
        facts.event_id = format!("event-{}", facts.now_ms);
        match &mut command {
            Command::Create { revision, .. } | Command::Upload { revision, .. } => {
                revision.created_ms = facts.now_ms
            }
            Command::RecordMaintenance { outcome, .. } => outcome.created_ms = facts.now_ms,
            _ => {}
        }
        apply(state, actor, command, facts)
    }
    fn revision(id: &str, number: u32, commit: &str) -> Revision {
        Revision {
            id: id.into(),
            number,
            commit: commit.into(),
            base: "base".into(),
            files: vec!["src/lib.rs".into()],
            author: "alice".into(),
            created_ms: 0,
            authorship: Authorship::Undeclared,
            submitted_by: None,
        }
    }
    fn create(state: &mut ReviewState, facts: &mut Facts, id: &str) -> ReviewResult<()> {
        facts.source.as_mut().required()?.commit = format!("commit-{id}");
        send(
            state,
            facts,
            "alice",
            Command::Create {
                id: id.into(),
                project: "project".into(),
                repository: "source-repository".into(),
                target_ref: "refs/heads/main".into(),
                title: format!("Review {id}"),
                description: "Fix behavior".into(),
                wip: false,
                revision: revision(&format!("{id}-r1"), 1, &format!("commit-{id}")),
            },
        )
        .required()?;
        Ok(())
    }
    fn setup() -> ReviewResult<(ReviewState, Facts)> {
        let mut state = ReviewState::default();
        let mut facts = facts();
        create(&mut state, &mut facts, "c")?;
        for principal in ["bob", "carol"] {
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::SetReviewer {
                    change: "c".into(),
                    expected_revision: "c-r1".into(),
                    principal: principal.into(),
                    role: Some(ReviewerRole::Reviewer),
                },
            )
            .required()?;
        }
        Ok((state, facts))
    }
    fn vote(
        state: &mut ReviewState,
        facts: &mut Facts,
        actor: &str,
        value: VoteValue,
    ) -> ReviewResult<()> {
        send(
            state,
            facts,
            actor,
            Command::Vote {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                value,
            },
        )
    }
    fn prepare(state: &mut ReviewState, facts: &mut Facts, id: &str) -> ReviewResult<()> {
        send(
            state,
            facts,
            "alice",
            Command::PrepareSubmit {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                candidate_id: id.into(),
            },
        )
    }
    fn record(
        state: &mut ReviewState,
        facts: &mut Facts,
        candidate: &str,
        target: &str,
    ) -> ReviewResult<()> {
        send(
            state,
            facts,
            "alice",
            Command::RecordSubmit {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                candidate_id: candidate.into(),
                expected_target: target.into(),
                result_commit: "commit-c".into(),
            },
        )
    }
    fn draft(id: &str) -> ReviewResult<Draft> {
        Ok(Draft {
            id: id.into(),
            change: "c".into(),
            author: "bob".into(),
            anchor: facts().anchors.into_iter().next().required()?,
            thread: None,
            body: "This needs explanation".into(),
            blocking: true,
        })
    }

    fn owned_facts() -> ReviewResult<Facts> {
        let mut facts = facts();
        let policy = facts.policy.as_mut().required()?;
        policy
            .ownership
            .teams
            .insert("maintainers".into(), ["bob".into()].into_iter().collect());
        policy.ownership.rules.push(OwnershipRule {
            id: "core".into(),
            paths: vec!["src/**".into()],
            owners: BTreeSet::new(),
            teams: ["maintainers".into()].into_iter().collect(),
            target_refs: ["refs/heads/main".into()].into_iter().collect(),
            required_evidence: Vec::new(),
            approval: BTreeMap::new(),
        });
        facts.source.as_mut().required()?.changed_files = Some(ChangedFiles {
            complete: true,
            files: vec![ChangedFile {
                path: "src/lib.rs".into(),
                old_path: None,
            }],
        });
        Ok(facts)
    }

    #[test]
    fn ownership_approvals_expand_teams_and_vetoes_still_win() -> ReviewResult<()> {
        let mut facts = owned_facts()?;
        let mut state = ReviewState::default();
        create(&mut state, &mut facts, "c")?;
        let change = state.changes.get("c").required()?;
        assert_eq!(
            change.reviewers,
            [("bob".into(), ReviewerRole::Reviewer)]
                .into_iter()
                .collect()
        );
        assert_eq!(change.attention, ["bob".into()].into_iter().collect());
        assert_eq!(
            state.inbox.get("bob").required()?.first().required()?.kind,
            "review_requested"
        );
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetReviewer {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                principal: "carol".into(),
                role: Some(ReviewerRole::Reviewer),
            },
        )?;
        vote(&mut state, &mut facts, "carol", VoteValue::Approve)?;
        let checks = checklist(&state, "c", &facts)?;
        assert!(!checks.ready);
        assert!(
            !checks
                .items
                .iter()
                .find(|item| item.key == "owner:core")
                .required()?
                .satisfied
        );
        vote(&mut state, &mut facts, "bob", VoteValue::Approve)?;
        assert!(checklist(&state, "c", &facts)?.ready);
        facts
            .policy
            .as_mut()
            .required()?
            .ownership
            .teams
            .insert("maintainers".into(), ["carol".into()].into_iter().collect());
        vote(&mut state, &mut facts, "carol", VoteValue::Abstain)?;
        assert!(!checklist(&state, "c", &facts)?.ready);
        vote(&mut state, &mut facts, "carol", VoteValue::Approve)?;
        assert!(checklist(&state, "c", &facts)?.ready);
        vote(&mut state, &mut facts, "bob", VoteValue::Veto)?;
        assert!(!checklist(&state, "c", &facts)?.ready);
        assert_eq!(
            prepare(&mut state, &mut facts, "blocked")
                .err()
                .required()?
                .code,
            "submit_blocked"
        );
        Ok(())
    }

    #[test]
    fn ownership_is_path_and_branch_specific_and_checks_rename_origin() -> ReviewResult<()> {
        let mut facts = owned_facts()?;
        facts
            .policy
            .as_mut()
            .required()?
            .ownership
            .rules
            .first_mut()
            .required()?
            .paths = vec!["private/**".into()];
        let mut state = ReviewState::default();
        create(&mut state, &mut facts, "c")?;
        assert!(state.changes.get("c").required()?.reviewers.is_empty());
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetReviewer {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                principal: "carol".into(),
                role: Some(ReviewerRole::Reviewer),
            },
        )?;
        vote(&mut state, &mut facts, "carol", VoteValue::Approve)?;
        assert!(checklist(&state, "c", &facts)?.ready);
        facts
            .source
            .as_mut()
            .required()?
            .changed_files
            .as_mut()
            .required()?
            .files
            .first_mut()
            .required()?
            .old_path = Some("private/secret.rs".into());
        assert!(!checklist(&state, "c", &facts)?.ready);
        facts
            .policy
            .as_mut()
            .required()?
            .ownership
            .rules
            .first_mut()
            .required()?
            .target_refs = ["refs/heads/release".into()].into_iter().collect();
        facts.source.as_mut().required()?.changed_files = None;
        assert!(checklist(&state, "c", &facts)?.ready);
        Ok(())
    }

    #[test]
    fn ownership_upload_requests_new_owners_and_invalidates_old_approval() -> ReviewResult<()> {
        let mut facts = owned_facts()?;
        let mut state = ReviewState::default();
        create(&mut state, &mut facts, "c")?;
        vote(&mut state, &mut facts, "bob", VoteValue::Approve)?;
        assert!(checklist(&state, "c", &facts)?.ready);
        facts.policy.as_mut().required()?.ownership.teams.insert(
            "maintainers".into(),
            ["bob".into(), "carol".into()].into_iter().collect(),
        );
        facts.source.as_mut().required()?.commit = "commit-new".into();
        facts.ci.first_mut().required()?.revision = "commit-new".into();
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::Upload {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                revision: revision("c-r2", 2, "commit-new"),
            },
        )?;
        let change = state.changes.get("c").required()?;
        assert_eq!(
            change.attention,
            ["bob".into(), "carol".into()].into_iter().collect()
        );
        assert_eq!(change.reviewers.get("carol"), Some(&ReviewerRole::Reviewer));
        assert!(!checklist(&state, "c", &facts)?.ready);
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::Vote {
                change: "c".into(),
                expected_revision: "c-r2".into(),
                value: VoteValue::Approve,
            },
        )?;
        assert!(checklist(&state, "c", &facts)?.ready);
        Ok(())
    }

    #[test]
    fn ownership_never_requests_or_counts_authors_and_ineligible_owners() -> ReviewResult<()> {
        let mut facts = owned_facts()?;
        let policy = facts.policy.as_mut().required()?;
        policy.eligible_reviewers.insert("alice".into());
        policy
            .ownership
            .teams
            .insert("maintainers".into(), ["alice".into()].into_iter().collect());
        let mut state = ReviewState::default();
        create(&mut state, &mut facts, "c")?;
        assert!(state.changes.get("c").required()?.reviewers.is_empty());
        assert!(!checklist(&state, "c", &facts)?.ready);
        assert_eq!(
            vote(&mut state, &mut facts, "alice", VoteValue::Approve)
                .err()
                .required()?
                .code,
            "ineligible_vote"
        );
        facts
            .policy
            .as_mut()
            .required()?
            .eligible_reviewers
            .remove("alice");
        assert_eq!(
            validate_policy(facts.policy.as_ref().required()?)
                .err()
                .required()?
                .code,
            "invalid_policy"
        );
        Ok(())
    }

    #[test]
    fn ownership_requires_complete_exact_changed_file_facts_atomically() -> ReviewResult<()> {
        let baseline = owned_facts()?;
        for variant in 0..5 {
            let mut facts = baseline.clone();
            let source = facts.source.as_mut().required()?;
            match variant {
                0 => source.changed_files = None,
                1 => source.changed_files.as_mut().required()?.complete = false,
                2 => source.changed_files.as_mut().required()?.files.clear(),
                3 => {
                    source
                        .changed_files
                        .as_mut()
                        .required()?
                        .files
                        .first_mut()
                        .required()?
                        .old_path = Some("../private".into())
                }
                _ => {
                    let file = source
                        .changed_files
                        .as_ref()
                        .required()?
                        .files
                        .first()
                        .required()?
                        .clone();
                    source.changed_files.as_mut().required()?.files.push(file);
                }
            }
            let mut state = ReviewState::default();
            assert!(create(&mut state, &mut facts, "c").is_err());
            assert_eq!(state, ReviewState::default());
            let mut good = baseline.clone();
            create(&mut state, &mut good, "c")?;
            vote(&mut state, &mut good, "bob", VoteValue::Approve)?;
            assert!(!checklist(&state, "c", &facts)?.ready);
        }
        Ok(())
    }

    #[test]
    fn ownership_rejects_inconsistent_policy_without_io() -> ReviewResult<()> {
        let facts = owned_facts()?;
        let baseline = facts.policy.required()?;
        for variant in 0..8 {
            let mut policy = baseline.clone();
            match variant {
                0 => {
                    policy.ownership.rules.first_mut().required()?.paths = vec!["../src/**".into()]
                }
                1 => policy.ownership.rules.first_mut().required()?.paths = vec!["src/a**b".into()],
                2 => {
                    policy
                        .ownership
                        .rules
                        .first_mut()
                        .required()?
                        .teams
                        .insert("missing".into());
                }
                3 => {
                    policy
                        .ownership
                        .teams
                        .get_mut("maintainers")
                        .required()?
                        .insert("mallory".into());
                }
                4 => policy
                    .ownership
                    .teams
                    .get_mut("maintainers")
                    .required()?
                    .clear(),
                5 => {
                    let rule = policy.ownership.rules.first().required()?.clone();
                    policy.ownership.rules.push(rule);
                }
                6 => policy.ownership.rules.first_mut().required()?.paths.clear(),
                _ => {
                    policy.ownership.rules.first_mut().required()?.target_refs =
                        ["refs/heads/*".into()].into_iter().collect()
                }
            }
            assert_eq!(
                validate_policy(&policy).err().required()?.code,
                "invalid_policy"
            );
        }
        Ok(())
    }

    fn integration_facts(facts: &mut Facts) -> ReviewResult<()> {
        facts.source.as_mut().required()?.current_target = "advanced-target".into();
        facts.source.as_mut().required()?.target_is_ancestor = false;
        facts.integration = Some(IntegrationFact {
            source_commit: "commit-c".into(),
            target_commit: "advanced-target".into(),
            candidate_commit: "merged-candidate".into(),
            target_is_ancestor: true,
            source_is_ancestor: true,
        });
        Ok(())
    }

    #[test]
    fn integration_requires_exact_candidate_ci_and_preserves_original_owner_approval()
    -> ReviewResult<()> {
        let mut facts = owned_facts()?;
        let mut state = ReviewState::default();
        create(&mut state, &mut facts, "c")?;
        vote(&mut state, &mut facts, "bob", VoteValue::Approve)?;
        integration_facts(&mut facts)?;
        assert!(
            !checklist(&state, "c", &facts)?.ready,
            "source CI is not candidate CI"
        );
        facts.ci.first_mut().required()?.revision = "merged-candidate".into();
        facts.ci.first_mut().required()?.run_id = "integration-run".into();
        assert!(checklist(&state, "c", &facts)?.ready);
        prepare(&mut state, &mut facts, "integrated")?;
        let candidate = state
            .changes
            .get("c")
            .required()?
            .candidate
            .as_ref()
            .required()?;
        assert_eq!(candidate.revision, "c-r1");
        assert_eq!(candidate.commit, "merged-candidate");
        assert_eq!(candidate.target, "advanced-target");
        assert_eq!(candidate.ci_run.as_deref(), Some("integration-run"));
        validate_candidate(&state, "c", "c-r1", "integrated", "alice", &facts)?;
        let before = state.clone();
        facts.source.as_mut().required()?.current_target = "wrong-readback".into();
        facts.source.as_mut().required()?.target_contains_commit = true;
        let command = Command::RecordSubmit {
            change: "c".into(),
            expected_revision: "c-r1".into(),
            candidate_id: "integrated".into(),
            expected_target: "advanced-target".into(),
            result_commit: "merged-candidate".into(),
        };
        assert!(send(&mut state, &mut facts, "alice", command.clone()).is_err());
        assert_eq!(state, before);
        facts.source.as_mut().required()?.current_target = "merged-candidate".into();
        send(&mut state, &mut facts, "alice", command)?;
        let change = state.changes.get("c").required()?;
        assert_eq!(change.state, ChangeStatus::Submitted);
        assert_eq!(change.current()?.commit, "commit-c");
        assert_eq!(
            change.submission.as_ref().required()?.result_commit,
            "merged-candidate"
        );
        Ok(())
    }

    #[test]
    fn integration_fails_closed_on_stale_source_target_ancestry_or_ci() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        vote(&mut state, &mut facts, "bob", VoteValue::Approve)?;
        integration_facts(&mut facts)?;
        facts.ci.first_mut().required()?.revision = "merged-candidate".into();
        prepare(&mut state, &mut facts, "integrated")?;
        let baseline = facts.clone();
        for variant in 0..8 {
            let mut changed = baseline.clone();
            match variant {
                0 => changed.integration.as_mut().required()?.source_is_ancestor = false,
                1 => changed.integration.as_mut().required()?.target_is_ancestor = false,
                2 => {
                    changed.integration.as_mut().required()?.source_commit =
                        "different-source".into()
                }
                3 => {
                    changed.integration.as_mut().required()?.target_commit =
                        "different-target".into()
                }
                4 => {
                    changed.integration.as_mut().required()?.candidate_commit =
                        "different-candidate".into()
                }
                5 => changed.source.as_mut().required()?.current_target = "moved-target".into(),
                6 => changed.integration = None,
                _ => changed.ci.push(CiFact {
                    ordinal: 2,
                    success: false,
                    ..changed.ci.first().required()?.clone()
                }),
            }
            assert!(!checklist(&state, "c", &changed)?.ready);
            assert_eq!(
                validate_candidate(&state, "c", "c-r1", "integrated", "alice", &changed)
                    .err()
                    .required()?
                    .code,
                "stale_candidate"
            );
        }
        facts.integration.as_mut().required()?.candidate_commit = "second-valid-candidate".into();
        facts.ci.first_mut().required()?.revision = "second-valid-candidate".into();
        assert!(checklist(&state, "c", &facts)?.ready);
        assert!(validate_candidate(&state, "c", "c-r1", "integrated", "alice", &facts).is_err());
        Ok(())
    }

    // r[verify kiln.monorepo.gating]
    #[test]
    fn monorepo_ci_requires_every_affected_project_at_its_current_scope() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        vote(&mut state, &mut facts, "bob", VoteValue::Approve)?;
        let primary = facts.ci.first().required()?.clone();
        let requirement = |project: &str, scope: Option<&str>| CiRequirement {
            project: project.into(),
            scope_identity: scope.map(Into::into),
        };
        // Only the change's own unscoped project affected: today's behaviour.
        facts.ci_required = Some(vec![requirement("project", None)]);
        assert!(checklist(&state, "c", &facts)?.ready);
        // Every project skipped: nothing blocks.
        facts.ci_required = Some(Vec::new());
        facts.ci.clear();
        assert!(checklist(&state, "c", &facts)?.ready);
        // A second affected scoped project needs its own run at its scope.
        facts.ci.push(primary.clone());
        facts.ci_required = Some(vec![
            requirement("project", None),
            requirement("api", Some("scope-2")),
        ]);
        let checks = checklist(&state, "c", &facts)?;
        assert!(!checks.ready);
        assert_eq!(checks.ci_run.as_deref(), Some("run-1"));
        // Another project's run never satisfies the requirement.
        facts.ci.push(CiFact {
            project: "web".into(),
            run_id: "run-web".into(),
            ordinal: 2,
            scope_identity: Some("scope-2".into()),
            ..primary.clone()
        });
        assert!(!checklist(&state, "c", &facts)?.ready);
        // An older scope of the right project never satisfies it either.
        facts.ci.push(CiFact {
            project: "api".into(),
            run_id: "run-api-old".into(),
            ordinal: 3,
            scope_identity: Some("scope-1".into()),
            ..primary.clone()
        });
        assert!(!checklist(&state, "c", &facts)?.ready);
        // A pending run at the current scope does not pass yet.
        facts.ci.push(CiFact {
            project: "api".into(),
            run_id: "run-api".into(),
            ordinal: 4,
            scope_identity: Some("scope-2".into()),
            success: false,
            terminal: false,
            ..primary.clone()
        });
        assert!(!checklist(&state, "c", &facts)?.ready);
        facts.ci.push(CiFact {
            project: "api".into(),
            run_id: "run-api-2".into(),
            ordinal: 5,
            scope_identity: Some("scope-2".into()),
            ..primary
        });
        let checks = checklist(&state, "c", &facts)?;
        assert!(checks.ready);
        assert_eq!(checks.ci_run.as_deref(), Some("run-1"));
        Ok(())
    }

    // r[verify kiln.untrusted.review]
    // r[verify kiln.untrusted.review.scenario.checklist]
    #[test]
    fn untrusted_approval_never_flips_checklist_items_and_counts_only_by_policy() -> ReviewResult<()>
    {
        let (state, mut facts) = setup()?;
        let met = |facts: &Facts, key: &str| -> ReviewResult<bool> {
            Ok(checklist(&state, "c", facts)?
                .items
                .iter()
                .find(|item| item.key == key)
                .required()?
                .satisfied)
        };
        let commit = facts.ci.first().required()?.revision.clone();
        let trusted_ci = met(&facts, "ci")?;
        assert!(trusted_ci, "trusted exact success satisfies CI");
        assert!(!met(&facts, "approvals")?);
        let summary = |counts: bool, approved: Option<&str>| crate::untrusted::UntrustedSummary {
            state: if approved.is_some() {
                crate::untrusted::UntrustedState::Approved
            } else {
                crate::untrusted::UntrustedState::AwaitingApproval
            },
            reason: crate::untrusted::UntrustedReason::Fork,
            pull_request: 7,
            head_repository: "outside/source".into(),
            source_commit: commit.clone(),
            approved_by: approved.map(|_| "Operator".into()),
            approved_by_id: approved.map(|_| "operator".into()),
            approved_commit: approved.map(Into::into),
            approved_at_ms: approved.map(|_| 1),
            counts_for_required_ci: counts,
            superseded_by: None,
        };
        // Approved, successful, exact commit, but policy does not count it.
        facts.ci.first_mut().required()?.untrusted = Some(summary(false, Some(&commit)));
        assert!(!met(&facts, "ci")?);
        assert!(!met(&facts, "approvals")?);
        assert!(!checklist(&state, "c", &facts)?.ready);
        // A projection claiming to count without an exact approval never does.
        facts.ci.first_mut().required()?.untrusted = Some(summary(true, None));
        assert!(!met(&facts, "ci")?);
        facts.ci.first_mut().required()?.untrusted = Some(summary(true, Some("other-commit")));
        assert!(!met(&facts, "ci")?);
        // Policy counts the exact approved commit: CI only; approvals stay unmet.
        facts.ci.first_mut().required()?.untrusted = Some(summary(true, Some(&commit)));
        assert!(met(&facts, "ci")?);
        assert!(!met(&facts, "approvals")?);
        assert!(!checklist(&state, "c", &facts)?.ready);
        Ok(())
    }

    #[test]
    fn overlapping_ownership_rules_each_require_their_owner() -> ReviewResult<()> {
        let mut facts = owned_facts()?;
        let policy = facts.policy.as_mut().required()?;
        policy.ownership.rules.push(OwnershipRule {
            id: "security".into(),
            paths: vec!["**/*.rs".into()],
            owners: ["carol".into()].into_iter().collect(),
            teams: BTreeSet::new(),
            target_refs: BTreeSet::new(),
            required_evidence: Vec::new(),
            approval: BTreeMap::new(),
        });
        let mut state = ReviewState::default();
        create(&mut state, &mut facts, "c")?;
        vote(&mut state, &mut facts, "bob", VoteValue::Approve)?;
        assert!(!checklist(&state, "c", &facts)?.ready);
        vote(&mut state, &mut facts, "carol", VoteValue::Approve)?;
        assert!(checklist(&state, "c", &facts)?.ready);
        Ok(())
    }

    #[test]
    fn immutable_upload_invalidates_current_approvals_and_actor_file_markers() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        facts.hunks = Some(vec![HunkFact {
            file: "src/lib.rs".into(),
            digest: "hunk-1".into(),
        }]);
        vote(&mut state, &mut facts, "bob", VoteValue::Approve).required()?;
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::MarkReviewed {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                file: "src/lib.rs".into(),
                reviewed: true,
            },
        )
        .required()?;
        assert_eq!(
            reviewed_hunks(state.changes.get("c").required()?, "bob")
                .next()
                .required()?
                .revision,
            "c-r1"
        );
        assert!(
            reviewed(state.changes.get("c").required()?, "bob")
                .next()
                .is_none()
        );
        facts.hunks = None;
        assert!(
            reviewed_hunks(state.changes.get("c").required()?, "alice")
                .next()
                .is_none()
        );
        prepare(&mut state, &mut facts, "candidate-old").required()?;
        let original = state.changes.get("c").required()?.current()?.clone();
        facts.source.as_mut().required()?.commit = "commit-new".into();
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::Upload {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                revision: revision("c-r2", 2, "commit-new"),
            },
        )
        .required()?;
        assert_eq!(
            state
                .changes
                .get("c")
                .required()?
                .revisions
                .first()
                .required()?
                .clone(),
            original
        );
        assert_eq!(state.changes.get("c").required()?.revisions.len(), 2);
        assert!(state.changes.get("c").required()?.candidate.is_none());
        assert!(state.changes.get("c").required()?.reviewed.is_empty());
        assert!(state.changes.get("c").required()?.reviewed_hunks.is_empty());
        assert!(!checklist(&state, "c", &facts).required()?.ready);
        let before = state.clone();
        assert_eq!(
            vote(&mut state, &mut facts, "bob", VoteValue::Approve)
                .err()
                .required()?
                .code,
            "stale_revision"
        );
        assert_eq!(state, before);
        assert!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::Upload {
                    change: "c".into(),
                    expected_revision: "c-r2".into(),
                    revision: revision("c-r3", 3, "commit-new")
                }
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn role_eligibility_non_author_approval_and_veto_are_enforced() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        assert_eq!(
            vote(&mut state, &mut facts, "alice", VoteValue::Approve)
                .err()
                .required()?
                .code,
            "ineligible_vote"
        );
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetReviewer {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                principal: "dave".into(),
                role: Some(ReviewerRole::Cc),
            },
        )
        .required()?;
        assert!(vote(&mut state, &mut facts, "dave", VoteValue::Veto).is_err());
        vote(&mut state, &mut facts, "bob", VoteValue::Approve).required()?;
        assert!(checklist(&state, "c", &facts).required()?.ready);
        vote(&mut state, &mut facts, "carol", VoteValue::Veto).required()?;
        assert!(!checklist(&state, "c", &facts).required()?.ready);
        vote(&mut state, &mut facts, "carol", VoteValue::Abstain).required()?;
        assert!(checklist(&state, "c", &facts).required()?.ready);
        vote(&mut state, &mut facts, "carol", VoteValue::NeedsRevision).required()?;
        assert_eq!(
            search(
                &state,
                "alice",
                &SearchQuery {
                    kind: SearchKind::OwnedNeedsRevision,
                    text: String::new(),
                    status: None,
                    advisory: None,
                }
            )
            .first()
            .required()?
            .id,
            "c"
        );
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetReviewer {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                principal: "bob".into(),
                role: None,
            },
        )
        .required()?;
        assert!(!checklist(&state, "c", &facts).required()?.ready);
        assert!(vote(&mut state, &mut facts, "bob", VoteValue::Approve).is_err());
        Ok(())
    }

    #[test]
    fn private_drafts_publish_atomically_with_vote_and_single_principal_notifications()
    -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        let inbox_before = project(&state, "alice").inbox.to_vec();
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::SaveDraft {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                draft: draft("draft-1")?,
            },
        )
        .required()?;
        assert_eq!(project(&state, "alice").inbox, inbox_before);
        assert!(project(&state, "alice").drafts.is_empty());
        assert_eq!(
            project(&state, "bob").drafts.first().required()?.body,
            "This needs explanation"
        );
        let before = state.clone();
        assert!(
            send(
                &mut state,
                &mut facts,
                "bob",
                Command::Publish {
                    change: "c".into(),
                    expected_revision: "c-r1".into(),
                    draft_ids: vec!["draft-1".into(), "missing".into()],
                    vote: Some(VoteValue::Approve)
                }
            )
            .is_err()
        );
        assert_eq!(state, before);
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::Publish {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                draft_ids: vec!["draft-1".into()],
                vote: Some(VoteValue::Approve),
            },
        )
        .required()?;
        assert!(project(&state, "bob").drafts.is_empty());
        assert_eq!(
            state
                .changes
                .get("c")
                .required()?
                .threads
                .first()
                .required()?
                .comments
                .first()
                .required()?
                .author,
            "bob"
        );
        assert_eq!(
            state
                .changes
                .get("c")
                .required()?
                .votes
                .first()
                .required()?
                .value,
            VoteValue::Approve
        );
        assert!(!checklist(&state, "c", &facts).required()?.ready);
        let notification = project(&state, "alice").inbox.last().required()?.clone();
        assert_eq!(notification.kind, "review_published");
        assert!(
            send(
                &mut state,
                &mut facts,
                "mallory",
                Command::ReadNotification {
                    id: notification.id.clone(),
                    read: true
                }
            )
            .is_err()
        );
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::ReadNotification {
                id: notification.id,
                read: true,
            },
        )
        .required()?;
        assert!(project(&state, "alice").inbox.last().required()?.read);
        assert!(!project(&state, "carol").inbox.last().required()?.read);
        Ok(())
    }

    #[test]
    fn anchors_resolution_replies_and_attention_preserve_thread_authority() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        let mut bad = facts.anchors.first().required()?.clone();
        bad.end_line = 999;
        assert!(
            send(
                &mut state,
                &mut facts,
                "bob",
                Command::StartThread {
                    change: "c".into(),
                    expected_revision: "c-r1".into(),
                    id: "thread-1".into(),
                    comment_id: "comment-1".into(),
                    anchor: bad,
                    body: "Bad range".into(),
                    blocking: true
                }
            )
            .is_err()
        );
        let anchor = facts.anchors.first().required()?.clone();
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::StartThread {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                id: "thread-1".into(),
                comment_id: "comment-1".into(),
                anchor,
                body: "Explain this".into(),
                blocking: true,
            },
        )
        .required()?;
        assert!(
            state
                .changes
                .get("c")
                .required()?
                .attention
                .contains("alice")
        );
        assert!(!state.changes.get("c").required()?.attention.contains("bob"));
        assert!(
            send(
                &mut state,
                &mut facts,
                "carol",
                Command::Resolve {
                    change: "c".into(),
                    expected_revision: "c-r1".into(),
                    thread: "thread-1".into(),
                    resolved: true
                }
            )
            .is_err()
        );
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::Reply {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                thread: "thread-1".into(),
                comment_id: "comment-2".into(),
                body: "Here is why".into(),
            },
        )
        .required()?;
        assert!(state.changes.get("c").required()?.attention.contains("bob"));
        assert!(
            !state
                .changes
                .get("c")
                .required()?
                .attention
                .contains("alice")
        );
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::Resolve {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                thread: "thread-1".into(),
                resolved: true,
            },
        )
        .required()?;
        vote(&mut state, &mut facts, "bob", VoteValue::Approve).required()?;
        assert!(checklist(&state, "c", &facts).required()?.ready);
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::Resolve {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                thread: "thread-1".into(),
                resolved: false,
            },
        )
        .required()?;
        assert_eq!(
            state
                .changes
                .get("c")
                .required()?
                .threads
                .first()
                .required()?
                .comments
                .get(1)
                .required()?
                .body,
            "Here is why"
        );
        assert!(!checklist(&state, "c", &facts).required()?.ready);
        Ok(())
    }

    fn evidence_item(checks: &Checklist, key: &str) -> ReviewResult<bool> {
        checks
            .items
            .iter()
            .find(|item| item.key == key)
            .map(|item| item.satisfied)
            .ok_or_else(|| ReviewError {
                code: "missing_fixture".into(),
                message: format!("checklist item {key}"),
            })
    }

    #[test]
    fn ownership_evidence_requires_exact_current_project_ci() -> ReviewResult<()> {
        let mut facts = owned_facts()?;
        facts
            .policy
            .as_mut()
            .required()?
            .ownership
            .rules
            .first_mut()
            .required()?
            .required_evidence
            .push(OwnershipEvidence {
                kind: OwnershipEvidenceKind::ProjectCi,
                project: "project".into(),
            });
        let mut state = ReviewState::default();
        create(&mut state, &mut facts, "c")?;
        let checks = checklist(&state, "c", &facts)?;
        assert!(evidence_item(&checks, "evidence:core:project_ci:project")?);
        assert!(!evidence_item(&checks, "owner:core")?);
        assert!(!checks.ready);
        facts.ci.first_mut().required()?.revision = "older-commit".into();
        assert!(!evidence_item(
            &checklist(&state, "c", &facts)?,
            "evidence:core:project_ci:project"
        )?);
        facts.ci.first_mut().required()?.revision = "commit-c".into();
        facts.ci.first_mut().required()?.clean = false;
        assert!(!evidence_item(
            &checklist(&state, "c", &facts)?,
            "evidence:core:project_ci:project"
        )?);
        facts.ci.first_mut().required()?.clean = true;
        facts.ci.first_mut().required()?.full_run = false;
        assert!(!evidence_item(
            &checklist(&state, "c", &facts)?,
            "evidence:core:project_ci:project"
        )?);
        Ok(())
    }

    #[test]
    fn ownership_evidence_binds_another_project_scope() -> ReviewResult<()> {
        let mut facts = owned_facts()?;
        facts
            .policy
            .as_mut()
            .required()?
            .ownership
            .rules
            .first_mut()
            .required()?
            .required_evidence
            .push(OwnershipEvidence {
                kind: OwnershipEvidenceKind::ProjectCi,
                project: "api".into(),
            });
        facts.ci_required = Some(vec![CiRequirement {
            project: "api".into(),
            scope_identity: Some("api-scope".into()),
        }]);
        facts.ci.push(CiFact {
            project: "api".into(),
            repository: "source-repository".into(),
            revision: "commit-c".into(),
            run_id: "api-run".into(),
            ordinal: 2,
            success: true,
            full_run: true,
            clean: true,
            scope_identity: None,
            terminal: true,
            untrusted: None,
        });
        let mut state = ReviewState::default();
        create(&mut state, &mut facts, "c")?;
        let key = "evidence:core:project_ci:api";
        assert!(!evidence_item(&checklist(&state, "c", &facts)?, key)?);
        facts.ci.last_mut().required()?.scope_identity = Some("api-scope".into());
        assert!(evidence_item(&checklist(&state, "c", &facts)?, key)?);
        facts.ci.last_mut().required()?.scope_identity = Some("other-scope".into());
        assert!(!evidence_item(&checklist(&state, "c", &facts)?, key)?);
        Ok(())
    }

    #[test]
    fn ownership_evidence_policy_admission_is_bounded_and_unique() -> ReviewResult<()> {
        let facts = owned_facts()?;
        let base = facts.policy.as_ref().required()?.clone();
        let reject = |policy: &Policy| -> ReviewResult<String> {
            validate_policy(policy)
                .err()
                .required()
                .map(|error| error.code)
        };
        let mut empty = base.clone();
        empty
            .ownership
            .rules
            .first_mut()
            .required()?
            .required_evidence
            .push(OwnershipEvidence {
                kind: OwnershipEvidenceKind::ProjectCi,
                project: " ".into(),
            });
        assert_eq!(reject(&empty)?, "invalid_policy");
        let mut duplicate = base.clone();
        for _ in 0..2 {
            duplicate
                .ownership
                .rules
                .first_mut()
                .required()?
                .required_evidence
                .push(OwnershipEvidence {
                    kind: OwnershipEvidenceKind::ProjectCi,
                    project: "api".into(),
                });
        }
        assert_eq!(reject(&duplicate)?, "invalid_policy");
        let mut over_bound = base;
        over_bound
            .ownership
            .rules
            .first_mut()
            .required()?
            .required_evidence = (0..=MAX_OWNERSHIP_EVIDENCE)
            .map(|index| OwnershipEvidence {
                kind: OwnershipEvidenceKind::ProjectCi,
                project: format!("project-{index}"),
            })
            .collect();
        assert_eq!(reject(&over_bound)?, "invalid_policy");
        Ok(())
    }

    #[test]
    fn evidence_only_class_skips_owner_approval_but_law_class_does_not() -> ReviewResult<()> {
        let mut facts = owned_facts()?;
        let policy = facts.policy.as_mut().required()?;
        policy.ownership.classes.push(ChangeClass {
            id: "docs".into(),
            paths: vec!["docs/**".into()],
        });
        let rule = policy.ownership.rules.first_mut().required()?;
        rule.paths = vec!["**".into()];
        rule.required_evidence.push(OwnershipEvidence {
            kind: OwnershipEvidenceKind::ProjectCi,
            project: "project".into(),
        });
        rule.approval
            .insert("docs".into(), OwnershipApprovalMode::EvidenceOnly);
        let mut state = ReviewState::default();
        create(&mut state, &mut facts, "c")?;
        state
            .changes
            .get_mut("c")
            .required()?
            .revisions
            .first_mut()
            .required()?
            .files = vec!["docs/guide.md".into()];
        facts.source.as_mut().required()?.changed_files = Some(ChangedFiles {
            complete: true,
            files: vec![ChangedFile {
                path: "docs/guide.md".into(),
                old_path: None,
            }],
        });
        let checks = checklist(&state, "c", &facts)?;
        assert!(evidence_item(&checks, "evidence:core:project_ci:project")?);
        assert!(evidence_item(&checks, "owner:core")?);
        assert!(
            checks
                .items
                .iter()
                .any(|item| item.key == "owner:core" && item.detail.contains("docs"))
        );
        // A rename whose old path is outside the class falls back to logic.
        facts.source.as_mut().required()?.changed_files = Some(ChangedFiles {
            complete: true,
            files: vec![ChangedFile {
                path: "docs/guide.md".into(),
                old_path: Some("src/old.rs".into()),
            }],
        });
        let checks = checklist(&state, "c", &facts)?;
        assert!(!evidence_item(&checks, "owner:core")?);
        // The same rule still requires an owner approval when the change is
        // outside the declared class.
        state
            .changes
            .get_mut("c")
            .required()?
            .revisions
            .first_mut()
            .required()?
            .files = vec!["src/lib.rs".into()];
        facts.source.as_mut().required()?.changed_files = Some(ChangedFiles {
            complete: true,
            files: vec![ChangedFile {
                path: "src/lib.rs".into(),
                old_path: None,
            }],
        });
        let checks = checklist(&state, "c", &facts)?;
        assert!(evidence_item(&checks, "evidence:core:project_ci:project")?);
        assert!(!evidence_item(&checks, "owner:core")?);
        assert!(
            checks
                .items
                .iter()
                .any(|item| item.key == "owner:core" && item.detail.contains("logic"))
        );
        Ok(())
    }

    #[test]
    fn change_class_policy_admission_is_closed_bounded_and_explicit() -> ReviewResult<()> {
        let facts = owned_facts()?;
        let base = facts.policy.as_ref().required()?.clone();
        let reject = |policy: &Policy| -> ReviewResult<String> {
            validate_policy(policy)
                .err()
                .required()
                .map(|error| error.code)
        };
        let mut unknown = base.clone();
        unknown
            .ownership
            .rules
            .first_mut()
            .required()?
            .approval
            .insert("unknown".into(), OwnershipApprovalMode::OwnerApproval);
        assert_eq!(reject(&unknown)?, "invalid_policy");
        let mut missing_evidence = base.clone();
        missing_evidence.ownership.classes.push(ChangeClass {
            id: "docs".into(),
            paths: vec!["docs/**".into()],
        });
        missing_evidence
            .ownership
            .rules
            .first_mut()
            .required()?
            .approval
            .insert("docs".into(), OwnershipApprovalMode::EvidenceOnly);
        assert_eq!(reject(&missing_evidence)?, "invalid_policy");
        let mut reserved = base.clone();
        reserved.ownership.classes.push(ChangeClass {
            id: "logic".into(),
            paths: vec!["docs/**".into()],
        });
        assert_eq!(reject(&reserved)?, "invalid_policy");
        let mut duplicate = base.clone();
        for _ in 0..2 {
            duplicate.ownership.classes.push(ChangeClass {
                id: "docs".into(),
                paths: vec!["docs/**".into()],
            });
        }
        assert_eq!(reject(&duplicate)?, "invalid_policy");
        let mut over_bound = base.clone();
        over_bound.ownership.classes = (0..=MAX_CHANGE_CLASSES)
            .map(|index| ChangeClass {
                id: format!("class-{index}"),
                paths: vec![format!("class-{index}/**")],
            })
            .collect();
        assert_eq!(reject(&over_bound)?, "invalid_policy");
        let mut valid = base;
        valid.ownership.classes.push(ChangeClass {
            id: "docs".into(),
            paths: vec!["docs/**".into()],
        });
        let rule = valid.ownership.rules.first_mut().required()?;
        rule.required_evidence.push(OwnershipEvidence {
            kind: OwnershipEvidenceKind::ProjectCi,
            project: "project".into(),
        });
        rule.approval
            .insert("docs".into(), OwnershipApprovalMode::EvidenceOnly);
        assert!(validate_policy(&valid).is_ok());
        Ok(())
    }

    #[test]
    fn advisory_agent_votes_do_not_approve_until_policy_binds_them() -> ReviewResult<()> {
        let mut facts = owned_facts()?;
        facts.agents.insert("bob".into());
        let mut state = ReviewState::default();
        create(&mut state, &mut facts, "c")?;
        vote(&mut state, &mut facts, "bob", VoteValue::NeedsRevision)?;
        let checks = checklist(&state, "c", &facts)?;
        assert!(!evidence_item(&checks, "vetoes")?);
        vote(&mut state, &mut facts, "bob", VoteValue::Approve)?;
        let checks = checklist(&state, "c", &facts)?;
        assert!(!evidence_item(&checks, "approvals")?);
        assert!(!evidence_item(&checks, "owner:core")?);
        facts.policy.as_mut().required()?.agent_votes = AgentVotes::Binding;
        let checks = checklist(&state, "c", &facts)?;
        assert!(evidence_item(&checks, "approvals")?);
        assert!(evidence_item(&checks, "owner:core")?);
        Ok(())
    }

    #[test]
    fn owner_lifecycle_wip_and_metadata_do_not_grant_submission() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        assert!(
            send(
                &mut state,
                &mut facts,
                "bob",
                Command::Abandon {
                    change: "c".into(),
                    expected_revision: "c-r1".into()
                }
            )
            .is_err()
        );
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::Edit {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                title: "Renamed review".into(),
                description: "Detailed plan".into(),
            },
        )
        .required()?;
        assert_eq!(state.changes.get("c").required()?.title, "Renamed review");
        vote(&mut state, &mut facts, "bob", VoteValue::Approve).required()?;
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetWip {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                wip: true,
            },
        )
        .required()?;
        assert!(prepare(&mut state, &mut facts, "blocked-wip").is_err());
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetWip {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                wip: false,
            },
        )
        .required()?;
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::Abandon {
                change: "c".into(),
                expected_revision: "c-r1".into(),
            },
        )
        .required()?;
        assert_eq!(
            state.changes.get("c").required()?.state,
            ChangeStatus::Abandoned
        );
        assert!(vote(&mut state, &mut facts, "bob", VoteValue::Approve).is_err());
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::Restore {
                change: "c".into(),
                expected_revision: "c-r1".into(),
            },
        )
        .required()?;
        assert_eq!(state.changes.get("c").required()?.state, ChangeStatus::Open);
        assert!(checklist(&state, "c", &facts).required()?.ready);
        Ok(())
    }

    #[test]
    fn exact_ci_requires_source_clean_full_latest_success_and_explicit_policy() -> ReviewResult<()>
    {
        let (mut state, mut facts) = setup()?;
        vote(&mut state, &mut facts, "bob", VoteValue::Approve).required()?;
        assert!(checklist(&state, "c", &facts).required()?.ready);
        let mut failure = facts.ci.first_mut().required()?.clone();
        failure.run_id = "run-2".into();
        failure.ordinal = 2;
        failure.success = false;
        facts.ci.push(failure);
        assert!(!checklist(&state, "c", &facts).required()?.ready);
        facts.ci.pop();
        for field in ["project", "repository", "revision", "partial", "dirty"] {
            let mut invalid = facts.clone();
            match field {
                "project" => invalid.ci.first_mut().required()?.project = "other".into(),
                "repository" => invalid.ci.first_mut().required()?.repository = "other".into(),
                "revision" => invalid.ci.first_mut().required()?.revision = "other".into(),
                "partial" => invalid.ci.first_mut().required()?.full_run = false,
                _ => invalid.ci.first_mut().required()?.clean = false,
            }
            assert!(
                !checklist(&state, "c", &invalid).required()?.ready,
                "{field}"
            );
        }
        facts.policy = None;
        assert!(prepare(&mut state, &mut facts, "missing-policy").is_err());
        Ok(())
    }

    #[test]
    fn exact_candidate_and_target_are_required_and_provider_readback_can_advance()
    -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        vote(&mut state, &mut facts, "bob", VoteValue::Approve).required()?;
        prepare(&mut state, &mut facts, "candidate-1").required()?;
        facts.source.as_mut().required()?.current_target = "moved-target".into();
        assert!(!checklist(&state, "c", &facts).required()?.ready);
        assert!(prepare(&mut state, &mut facts, "candidate-stale").is_err());
        assert!(record(&mut state, &mut facts, "candidate-1", "moved-target").is_err());
        facts.source.as_mut().required()?.target_contains_commit = true;
        assert!(record(&mut state, &mut facts, "wrong-candidate", "base").is_err());
        record(&mut state, &mut facts, "candidate-1", "base").required()?;
        assert_eq!(
            state.changes.get("c").required()?.state,
            ChangeStatus::Submitted
        );
        assert!(record(&mut state, &mut facts, "candidate-1", "base").is_err());
        assert!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::Restore {
                    change: "c".into(),
                    expected_revision: "c-r1".into()
                }
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn changed_votes_and_ci_invalidate_prepared_candidate_authority() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        vote(&mut state, &mut facts, "bob", VoteValue::Approve).required()?;
        prepare(&mut state, &mut facts, "candidate-1").required()?;
        vote(&mut state, &mut facts, "carol", VoteValue::Veto).required()?;
        assert!(state.changes.get("c").required()?.candidate.is_none());
        facts.source.as_mut().required()?.target_contains_commit = true;
        assert!(record(&mut state, &mut facts, "candidate-1", "base").is_err());
        vote(&mut state, &mut facts, "carol", VoteValue::Abstain).required()?;
        prepare(&mut state, &mut facts, "candidate-2").required()?;
        facts.ci.first_mut().required()?.run_id = "new-successful-run".into();
        facts.ci.first_mut().required()?.ordinal = 3;
        assert!(record(&mut state, &mut facts, "candidate-2", "base").is_err());
        Ok(())
    }

    #[test]
    fn dependency_stacks_reject_cycles_and_stale_revision_bindings() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        create(&mut state, &mut facts, "d")?;
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetDependencies {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                dependencies: vec![Dependency {
                    change: "d".into(),
                    revision: "d-r1".into(),
                }],
            },
        )
        .required()?;
        assert_eq!(
            stack(&state, "c")?
                .into_iter()
                .map(|change| change.id.as_str())
                .collect::<Vec<_>>(),
            vec!["d", "c"]
        );
        assert_eq!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::SetDependencies {
                    change: "d".into(),
                    expected_revision: "d-r1".into(),
                    dependencies: vec![Dependency {
                        change: "c".into(),
                        revision: "c-r1".into()
                    }]
                }
            )
            .err()
            .required()?
            .code,
            "dependency_cycle"
        );
        facts.source.as_mut().required()?.commit = "commit-d-new".into();
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::Upload {
                change: "d".into(),
                expected_revision: "d-r1".into(),
                revision: revision("d-r2", 2, "commit-d-new"),
            },
        )
        .required()?;
        assert!(stack(&state, "c").is_err());
        facts.source.as_mut().required()?.commit = "commit-c".into();
        vote(&mut state, &mut facts, "bob", VoteValue::Approve).required()?;
        assert!(
            !checklist(&state, "c", &facts)
                .required()?
                .items
                .iter()
                .find(|item| item.key == "dependencies")
                .required()?
                .satisfied
        );
        assert!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::SetDependencies {
                    change: "c".into(),
                    expected_revision: "c-r1".into(),
                    dependencies: vec![Dependency {
                        change: "d".into(),
                        revision: "d-r1".into()
                    }]
                }
            )
            .is_err()
        );
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetDependencies {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                dependencies: vec![Dependency {
                    change: "d".into(),
                    revision: "d-r2".into(),
                }],
            },
        )
        .required()?;
        assert!(!checklist(&state, "c", &facts).required()?.ready);
        Ok(())
    }

    #[test]
    fn saved_queries_draft_deletion_and_attention_are_principal_owned() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        let query = SavedQuery {
            id: "view-1".into(),
            name: "My requests".into(),
            query: SearchQuery {
                kind: SearchKind::AwaitingReview,
                text: "review".into(),
                status: Some(ChangeStatus::Open),
                advisory: None,
            },
        };
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::SaveQuery {
                query: query.clone(),
            },
        )
        .required()?;
        assert_eq!(
            project(&state, "bob")
                .saved_queries
                .first()
                .required()?
                .clone(),
            query
        );
        assert!(project(&state, "alice").saved_queries.is_empty());
        assert_eq!(
            search(&state, "bob", &query.query).first().required()?.id,
            "c"
        );
        assert!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::DeleteQuery {
                    id: "view-1".into()
                }
            )
            .is_err()
        );
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::DeleteQuery {
                id: "view-1".into(),
            },
        )
        .required()?;
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::SaveDraft {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                draft: draft("draft-delete")?,
            },
        )
        .required()?;
        assert!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::DeleteDraft {
                    id: "draft-delete".into()
                }
            )
            .is_err()
        );
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::DeleteDraft {
                id: "draft-delete".into(),
            },
        )
        .required()?;
        assert!(project(&state, "bob").drafts.is_empty());
        assert!(
            send(
                &mut state,
                &mut facts,
                "carol",
                Command::SetAttention {
                    change: "c".into(),
                    expected_revision: "c-r1".into(),
                    principal: "bob".into(),
                    attention: false
                }
            )
            .is_err()
        );
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::SetAttention {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                principal: "bob".into(),
                attention: false,
            },
        )
        .required()?;
        assert!(!state.changes.get("c").required()?.attention.contains("bob"));
        Ok(())
    }

    #[test]
    fn maintenance_rebase_registers_exact_new_revision_and_rejects_old_or_forged_outcomes()
    -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        facts.source.as_mut().required()?.commit = "rebased-commit".into();
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::Upload {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                revision: revision("c-r2", 2, "rebased-commit"),
            },
        )
        .required()?;
        let outcome = MaintenanceOutcome {
            operation_id: "rebase-operation".into(),
            kind: MaintenanceKind::Rebase,
            source_revision: "c-r1".into(),
            result_change: "c".into(),
            result_revision: "c-r2".into(),
            result_commit: "rebased-commit".into(),
            actor: "alice".into(),
            created_ms: 0,
        };
        assert!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::RecordMaintenance {
                    change: "c".into(),
                    expected_revision: "c-r1".into(),
                    outcome: outcome.clone()
                }
            )
            .is_err()
        );
        let mut forged = outcome.clone();
        forged.result_commit = "unregistered-commit".into();
        assert!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::RecordMaintenance {
                    change: "c".into(),
                    expected_revision: "c-r2".into(),
                    outcome: forged
                }
            )
            .is_err()
        );
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::RecordMaintenance {
                change: "c".into(),
                expected_revision: "c-r2".into(),
                outcome: outcome.clone(),
            },
        )
        .required()?;
        assert_eq!(
            state
                .changes
                .get("c")
                .required()?
                .maintenance
                .first()
                .required()?
                .source_revision,
            "c-r1"
        );
        assert!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::RecordMaintenance {
                    change: "c".into(),
                    expected_revision: "c-r2".into(),
                    outcome
                }
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn historical_drafts_publish_without_retargeting_and_comparison_base_is_exact()
    -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::SaveDraft {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                draft: draft("historical-draft")?,
            },
        )
        .required()?;
        facts.source.as_mut().required()?.commit = "new-commit".into();
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::Upload {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                revision: revision("c-r2", 2, "new-commit"),
            },
        )
        .required()?;
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::Publish {
                change: "c".into(),
                expected_revision: "c-r2".into(),
                draft_ids: vec!["historical-draft".into()],
                vote: Some(VoteValue::Approve),
            },
        )
        .required()?;
        assert_eq!(
            state
                .changes
                .get("c")
                .required()?
                .threads
                .first()
                .required()?
                .anchor
                .revision,
            "c-r1"
        );
        assert_eq!(
            state
                .changes
                .get("c")
                .required()?
                .votes
                .first()
                .required()?
                .revision,
            "c-r2"
        );
        let comparison = Anchor {
            revision: "c-r2".into(),
            base_commit: "commit-c".into(),
            file: "old-name.rs".into(),
            side: Side::Base,
            start_line: 4,
            end_line: 4,
        };
        facts.anchors.push(comparison.clone());
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::StartThread {
                change: "c".into(),
                expected_revision: "c-r2".into(),
                id: "comparison-thread".into(),
                comment_id: "comparison-comment".into(),
                anchor: comparison.clone(),
                body: "Old-side rename comment".into(),
                blocking: false,
            },
        )
        .required()?;
        assert_eq!(
            state
                .changes
                .get("c")
                .required()?
                .threads
                .get(1)
                .required()?
                .anchor,
            comparison
        );
        let mut forged = comparison;
        forged.base_commit = "unrelated-old-commit".into();
        facts.anchors.push(forged.clone());
        assert!(
            send(
                &mut state,
                &mut facts,
                "bob",
                Command::StartThread {
                    change: "c".into(),
                    expected_revision: "c-r2".into(),
                    id: "forged-thread".into(),
                    comment_id: "forged-comment".into(),
                    anchor: forged,
                    body: "Wrong comparison".into(),
                    blocking: false
                }
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn prepared_candidate_rechecks_exact_target_and_current_ci_before_effects() -> ReviewResult<()>
    {
        let (mut state, mut facts) = setup()?;
        vote(&mut state, &mut facts, "bob", VoteValue::Approve).required()?;
        prepare(&mut state, &mut facts, "candidate-1").required()?;
        validate_candidate(&state, "c", "c-r1", "candidate-1", "alice", &facts).required()?;
        assert!(validate_candidate(&state, "c", "c-r1", "candidate-1", "bob", &facts).is_err());
        facts.source.as_mut().required()?.current_target = "new-target".into();
        assert!(validate_candidate(&state, "c", "c-r1", "candidate-1", "alice", &facts).is_err());
        facts.source.as_mut().required()?.current_target = "base".into();
        facts.ci.first_mut().required()?.success = false;
        assert!(validate_candidate(&state, "c", "c-r1", "candidate-1", "alice", &facts).is_err());
        Ok(())
    }

    #[test]
    fn ordered_stack_admits_only_proven_exact_declared_parent_target_moves() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        create(&mut state, &mut facts, "d")?;
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetReviewer {
                change: "d".into(),
                expected_revision: "d-r1".into(),
                principal: "bob".into(),
                role: Some(ReviewerRole::Reviewer),
            },
        )?;
        send(
            &mut state,
            &mut facts,
            "bob",
            Command::Vote {
                change: "d".into(),
                expected_revision: "d-r1".into(),
                value: VoteValue::Approve,
            },
        )?;
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::SetDependencies {
                change: "d".into(),
                expected_revision: "d-r1".into(),
                dependencies: vec![Dependency {
                    change: "c".into(),
                    revision: "c-r1".into(),
                }],
            },
        )?;
        let mut ci = facts.ci.first().required()?.clone();
        ci.revision = "commit-d".into();
        ci.run_id = "run-d".into();
        ci.ordinal = 2;
        facts.ci.push(ci);
        assert!(!checklist(&state, "d", &facts)?.ready);
        facts.source.as_mut().required()?.commit = "commit-c".into();
        vote(&mut state, &mut facts, "bob", VoteValue::Approve)?;
        prepare(&mut state, &mut facts, "candidate-parent")?;
        validate_candidate(&state, "c", "c-r1", "candidate-parent", "alice", &facts)?;
        facts.source.as_mut().required()?.current_target = "commit-c".into();
        facts.source.as_mut().required()?.target_contains_commit = true;
        record(&mut state, &mut facts, "candidate-parent", "base")?;
        let source = facts.source.as_mut().required()?;
        source.commit = "commit-d".into();
        source.target_contains_commit = false;
        source.target_is_ancestor = false;
        assert!(!checklist(&state, "d", &facts)?.ready);
        facts.source.as_mut().required()?.target_is_ancestor = true;
        assert!(!checklist(&state, "d", &facts)?.ready);
        facts
            .source
            .as_mut()
            .required()?
            .target_ancestors
            .insert("commit-c".into());
        assert!(checklist(&state, "d", &facts)?.ready);
        facts.source.as_mut().required()?.current_target = "unrelated-descendant".into();
        assert!(!checklist(&state, "d", &facts)?.ready);
        facts.source.as_mut().required()?.current_target = "commit-c".into();
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::PrepareSubmit {
                change: "d".into(),
                expected_revision: "d-r1".into(),
                candidate_id: "candidate-child".into(),
            },
        )?;
        validate_candidate(&state, "d", "d-r1", "candidate-child", "alice", &facts)?;
        let source = facts.source.as_mut().required()?;
        source.current_target = "later-descendant-of-child".into();
        source.target_contains_commit = true;
        source.target_is_ancestor = false;
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::RecordSubmit {
                change: "d".into(),
                expected_revision: "d-r1".into(),
                candidate_id: "candidate-child".into(),
                expected_target: "commit-c".into(),
                result_commit: "commit-d".into(),
            },
        )?;
        let child = state.changes.get("d").required()?;
        assert_eq!(child.state, ChangeStatus::Submitted);
        assert_eq!(child.current()?.id, "d-r1");
        assert_eq!(child.current()?.base, "base");
        assert_eq!(child.votes.first().required()?.revision, "d-r1");
        let submitted = child.submission.as_ref().required()?;
        assert_eq!(submitted.candidate.target, "commit-c");
        assert!(submitted.candidate.target_is_ancestor);
        assert_eq!(submitted.result_commit, "commit-d");
        Ok(())
    }

    #[test]
    fn revert_and_cherry_pick_are_new_reviews_never_target_mutations() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        create(&mut state, &mut facts, "revert")?;
        let revert = MaintenanceOutcome {
            operation_id: "revert-op".into(),
            kind: MaintenanceKind::Revert,
            source_revision: "c-r1".into(),
            result_change: "revert".into(),
            result_revision: "revert-r1".into(),
            result_commit: "commit-revert".into(),
            actor: "alice".into(),
            created_ms: 0,
        };
        assert!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::RecordMaintenance {
                    change: "c".into(),
                    expected_revision: "c-r1".into(),
                    outcome: revert.clone()
                }
            )
            .is_err()
        );
        facts.source.as_mut().required()?.commit = "commit-c".into();
        vote(&mut state, &mut facts, "bob", VoteValue::Approve).required()?;
        prepare(&mut state, &mut facts, "candidate-1").required()?;
        facts.source.as_mut().required()?.target_contains_commit = true;
        facts.source.as_mut().required()?.current_target = "commit-c".into();
        record(&mut state, &mut facts, "candidate-1", "base").required()?;
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::RecordMaintenance {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                outcome: revert,
            },
        )
        .required()?;
        assert_eq!(
            state.changes.get("revert").required()?.state,
            ChangeStatus::Open
        );
        let source = facts.source.as_mut().required()?;
        source.target_ref = "refs/heads/release".into();
        source.commit = "picked-commit".into();
        source.base = "base".into();
        source.current_target = "base".into();
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::Create {
                id: "picked".into(),
                project: "project".into(),
                repository: "source-repository".into(),
                target_ref: "refs/heads/release".into(),
                title: "Release pick".into(),
                description: String::new(),
                wip: false,
                revision: revision("picked-r1", 1, "picked-commit"),
            },
        )
        .required()?;
        let picked = MaintenanceOutcome {
            operation_id: "pick-op".into(),
            kind: MaintenanceKind::CherryPick,
            source_revision: "c-r1".into(),
            result_change: "picked".into(),
            result_revision: "picked-r1".into(),
            result_commit: "picked-commit".into(),
            actor: "alice".into(),
            created_ms: 0,
        };
        send(
            &mut state,
            &mut facts,
            "alice",
            Command::RecordMaintenance {
                change: "c".into(),
                expected_revision: "c-r1".into(),
                outcome: picked,
            },
        )
        .required()?;
        assert_eq!(
            state.changes.get("c").required()?.state,
            ChangeStatus::Submitted
        );
        assert_eq!(
            state.changes.get("picked").required()?.state,
            ChangeStatus::Open
        );
        assert!(state.changes.get("picked").required()?.candidate.is_none());
        Ok(())
    }

    #[test]
    fn replayed_events_and_unadmitted_sources_leave_state_unchanged() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        let before = state.clone();
        assert_eq!(
            apply(
                &mut state,
                "alice",
                Command::SetWip {
                    change: "c".into(),
                    expected_revision: "c-r1".into(),
                    wip: true
                },
                &facts
            )
            .err()
            .required()?
            .code,
            "replayed_identity"
        );
        assert_eq!(state, before);
        facts.source.as_mut().required()?.immutable = false;
        facts.source.as_mut().required()?.commit = "unadmitted".into();
        assert!(
            send(
                &mut state,
                &mut facts,
                "alice",
                Command::Upload {
                    change: "c".into(),
                    expected_revision: "c-r1".into(),
                    revision: revision("c-r2", 2, "unadmitted")
                }
            )
            .is_err()
        );
        assert_eq!(state, before);
        Ok(())
    }

    fn agent(harness: &str, model: Option<&str>) -> Authorship {
        Authorship::Agent {
            harness: harness.into(),
            model: model.map(Into::into),
        }
    }

    fn upload_declared(
        state: &mut ReviewState,
        facts: &mut Facts,
        number: u32,
        authorship: Authorship,
    ) -> ReviewResult<()> {
        let commit = format!("commit-c{number}");
        facts.source.as_mut().required()?.commit = commit.clone();
        let mut next = revision(&format!("c-r{number}"), number, &commit);
        next.authorship = authorship;
        send(
            state,
            facts,
            "alice",
            Command::Upload {
                change: "c".into(),
                expected_revision: format!("c-r{}", number - 1),
                revision: next,
            },
        )
    }

    // r[verify kiln.review.authorship]
    // r[verify kiln.review.authorship.scenario.declared]
    #[test]
    fn declared_authorship_is_stored_per_patch_set_and_folded_from_declarations_only()
    -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        let class = |state: &ReviewState| -> ReviewResult<AuthorshipClass> {
            Ok(authorship_class(state.changes.get("c").required()?))
        };
        assert_eq!(class(&state)?, AuthorshipClass::Undeclared);
        upload_declared(&mut state, &mut facts, 2, agent("omp", Some("claude")))?;
        assert_eq!(
            state.changes.get("c").required()?.current()?.authorship,
            agent("omp", Some("claude"))
        );
        assert_eq!(class(&state)?, AuthorshipClass::Agent);
        upload_declared(&mut state, &mut facts, 3, agent("other", None))?;
        upload_declared(&mut state, &mut facts, 4, Authorship::Undeclared)?;
        assert_eq!(
            class(&state)?,
            AuthorshipClass::Agent,
            "undeclared is silent"
        );
        upload_declared(&mut state, &mut facts, 5, Authorship::Human)?;
        assert_eq!(class(&state)?, AuthorshipClass::Mixed);
        let mut human = state.changes.get("c").required()?.clone();
        for revision in &mut human.revisions {
            revision.authorship = Authorship::Human;
        }
        assert_eq!(authorship_class(&human), AuthorshipClass::Human);
        human.revisions.first_mut().required()?.authorship = Authorship::Mixed;
        assert_eq!(authorship_class(&human), AuthorshipClass::Mixed);
        Ok(())
    }

    // r[verify kiln.review.authorship.scenario.invalid]
    #[test]
    fn invalid_declared_authorship_is_rejected_without_mutation() -> ReviewResult<()> {
        let (mut state, mut facts) = setup()?;
        let oversized = "h".repeat(MAX_AUTHORSHIP_TEXT_BYTES + 1);
        let bounded = "h".repeat(MAX_AUTHORSHIP_TEXT_BYTES);
        for invalid in [
            agent("", None),
            agent(" omp", None),
            agent("omp\n", None),
            agent(&oversized, None),
            agent("omp", Some("")),
            agent("omp", Some("model\u{7}")),
            agent("omp", Some(&oversized)),
        ] {
            assert_eq!(
                validate_authorship(&invalid).err().map(|error| error.code),
                Some("invalid_authorship".into())
            );
            let before = state.clone();
            let error = upload_declared(&mut state, &mut facts, 2, invalid);
            assert_eq!(
                error.err().map(|error| error.code),
                Some("invalid_authorship".into())
            );
            assert_eq!(state, before);
        }
        validate_authorship(&agent(&bounded, Some(&bounded)))?;
        upload_declared(&mut state, &mut facts, 2, agent(&bounded, Some(&bounded)))?;
        Ok(())
    }

    #[test]
    fn agent_submission_rejects_human_authorship_and_records_attestation() -> ReviewResult<()> {
        let create = |revision: Revision| Command::Create {
            id: "c".into(),
            project: "project".into(),
            repository: "source-repository".into(),
            target_ref: "refs/heads/main".into(),
            title: "Review".into(),
            description: "Agent change".into(),
            wip: false,
            revision,
        };
        let mut facts = self::facts();
        facts.agents.insert("alice".into());
        facts.agent_identities.insert(
            "alice".into(),
            AgentIdentity {
                harness: "omp".into(),
                model: Some("deepseek".into()),
            },
        );
        let mut state = ReviewState::default();
        let mut human = revision("c-r1", 1, "commit-c");
        human.authorship = Authorship::Human;
        assert_eq!(
            send(&mut state, &mut facts, "alice", create(human))
                .err()
                .map(|error| error.code),
            Some("agent_authorship_mismatch".into())
        );
        assert!(state.changes.is_empty());
        let undeclared = revision("c-r1", 1, "commit-c");
        send(&mut state, &mut facts, "alice", create(undeclared))?;
        let attested = state
            .changes
            .get("c")
            .required()?
            .revisions
            .first()
            .required()?
            .submitted_by
            .as_ref()
            .required()?;
        assert_eq!(attested.principal, "alice");
        assert_eq!(
            attested.agent.as_ref().map(|agent| agent.harness.as_str()),
            Some("omp")
        );
        // A human principal keeps today's un-attested behavior.
        let mut facts = self::facts();
        let mut state = ReviewState::default();
        let mut human = revision("c-r1", 1, "commit-c");
        human.authorship = Authorship::Human;
        send(&mut state, &mut facts, "alice", create(human))?;
        assert!(
            state
                .changes
                .get("c")
                .required()?
                .revisions
                .first()
                .required()?
                .submitted_by
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn owning_rule_is_first_applicable_rule_in_policy_order() -> ReviewResult<()> {
        let mut ownership = OwnershipPolicy::default();
        for (id, path, target) in [
            ("release-only", "src/**", Some("refs/heads/release")),
            ("core", "src/**", None),
            ("rust", "**/*.rs", None),
        ] {
            ownership.rules.push(OwnershipRule {
                id: id.into(),
                paths: vec![path.into()],
                owners: BTreeSet::new(),
                teams: BTreeSet::new(),
                target_refs: target.into_iter().map(Into::into).collect(),
                required_evidence: Vec::new(),
                approval: BTreeMap::new(),
            });
        }
        assert_eq!(
            owning_rule(&ownership, "refs/heads/main", "src/lib.rs"),
            Some("core")
        );
        assert_eq!(
            owning_rule(&ownership, "refs/heads/release", "src/lib.rs"),
            Some("release-only")
        );
        assert_eq!(
            owning_rule(&ownership, "refs/heads/main", "tests/a.rs"),
            Some("rust")
        );
        assert_eq!(
            owning_rule(&ownership, "refs/heads/main", "README.md"),
            None
        );
        Ok(())
    }
}
