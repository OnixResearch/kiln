//! Interactive review semantics. Provider observations and identifiers are admitted by
//! shells; this module never performs I/O, hashes identities, or grants provider authority.
//! `ReviewState` is persistence-only. `project` borrows caller-private drafts,
//! inbox and saved queries; public change views must filter markers with `reviewed`.
//! Commands validate before mutation. Submission prepares an exact candidate,
//! re-admits it with `validate_candidate` immediately before the provider effect,
//! then `RecordSubmit` admits read-back facts and retains `SubmitOutcome`.
//! r[impl kiln.review.workflow]
//! r[impl kiln.review.parity.ownership]

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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub id: String,
    pub number: u32,
    pub commit: String,
    pub base: String,
    pub files: Vec<String>,
    pub author: String,
    pub created_ms: i64,
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
    pub attention: BTreeSet<String>,
    pub dependencies: Vec<Dependency>,
    pub candidate: Option<Candidate>,
    pub submission: Option<SubmitOutcome>,
    pub maintenance: Vec<MaintenanceOutcome>,
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
}

/// All matching rules apply cumulatively; each needs one expanded owner approval.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipPolicy {
    pub teams: BTreeMap<String, BTreeSet<String>>,
    pub rules: Vec<OwnershipRule>,
}
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
fn canonical_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn valid_pattern(pattern: &str) -> bool {
    canonical_path(pattern)
        && !pattern.contains(['[', ']', '{', '}'])
        && pattern
            .split('/')
            .all(|part| !part.contains("**") || part == "**")
}

/// Greedy wildcard matching without allocation, filesystem state or locale rules.
fn component_matches(pattern: &str, value: &str) -> bool {
    let mut pattern = pattern.chars();
    let mut value = value.chars();
    let mut retry = None;
    loop {
        match (pattern.clone().next(), value.clone().next()) {
            (Some('*'), _) => {
                pattern.next();
                retry = Some((pattern.clone(), value.clone()));
            }
            (Some(want), Some(got)) if want == '?' || want == got => {
                pattern.next();
                value.next();
            }
            (None, None) => return true,
            _ => {
                let Some((resume, rest)) = retry.as_mut() else {
                    return false;
                };
                if rest.next().is_none() {
                    return false;
                }
                pattern = resume.clone();
                value = rest.clone();
            }
        }
    }
}

fn path_matches(pattern: &str, path: &str) -> bool {
    let mut pattern = pattern.split('/');
    let mut path = path.split('/');
    let mut retry = None;
    loop {
        match (pattern.clone().next(), path.clone().next()) {
            (Some("**"), _) => {
                pattern.next();
                retry = Some((pattern.clone(), path.clone()));
            }
            (Some(want), Some(got)) if component_matches(want, got) => {
                pattern.next();
                path.next();
            }
            (None, None) => return true,
            _ => {
                let Some((resume, rest)) = retry.as_mut() else {
                    return false;
                };
                if rest.next().is_none() {
                    return false;
                }
                pattern = resume.clone();
                path = rest.clone();
            }
        }
    }
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
    let mut ids = BTreeSet::new();
    for rule in &policy.ownership.rules {
        if !principal_valid(&rule.id)
            || !ids.insert(&rule.id)
            || rule.paths.is_empty()
            || !rule.paths.iter().all(|path| valid_pattern(path))
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
                    || !canonical_path(target)
                    || target.contains(['*', '?', '[', ']', ' ', '~', '^', ':'])
            })
        {
            return fail(
                "invalid_policy",
                "ownership rule needs unique identity, valid paths, known eligible owners and exact target refs",
            );
        }
    }
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
            !canonical_path(&file.path)
                || file
                    .old_path
                    .as_ref()
                    .is_some_and(|path| !canonical_path(path))
        })
    {
        return fail(
            "invalid_changed_files",
            "ownership changed-file facts must be complete and match the exact revision paths",
        );
    }
    Ok(&files.files)
}

fn rule_matches(rule: &OwnershipRule, files: &[ChangedFile]) -> bool {
    files.iter().any(|file| {
        rule.paths.iter().any(|pattern| {
            path_matches(pattern, &file.path)
                || file
                    .old_path
                    .as_ref()
                    .is_some_and(|path| path_matches(pattern, path))
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
    let approvals = change
        .votes
        .iter()
        .filter(eligible)
        .filter(|vote| vote.value == VoteValue::Approve)
        .count();
    let ci_commit = facts
        .integration
        .as_ref()
        .map_or(revision.commit.as_str(), |integration| {
            integration.candidate_commit.as_str()
        });
    let latest = facts
        .ci
        .iter()
        .filter(|ci| {
            ci.project == change.project
                && ci.repository == change.repository
                && ci.revision == ci_commit
                && ci.full_run
                && ci.clean
                && !ci.run_id.is_empty()
        })
        .max_by_key(|ci| ci.ordinal);
    let ci_ok = policy.is_some_and(|p| !p.require_ci || latest.is_some_and(|ci| ci.success));
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
                    for rule in &policy.ownership.rules {
                        if !applicable(rule, change) || !rule_matches(rule, files) {
                            continue;
                        }
                        let approved = change.votes.iter().filter(eligible).any(|vote| {
                            vote.value == VoteValue::Approve
                                && rule_owners(policy, rule).any(|owner| owner == &vote.actor)
                        });
                        items.push(ChecklistItem {
                            key: alloc::format!("owner:{}", rule.id),
                            satisfied: policy_valid && approved,
                            detail: alloc::format!(
                                "ownership rule {} for changed paths {:?}: one eligible requested non-author owner from {:?} / teams {:?} must approve exact revision {}",
                                rule.id, rule.paths, rule.owners, rule.teams, revision.id
                            ),
                        });
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
            revision,
        } => {
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
                attention: BTreeSet::new(),
                dependencies: Vec::new(),
                candidate: None,
                submission: None,
                maintenance: Vec::new(),
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
            revision,
        } => {
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
            current.revisions.push(revision);
            current.reviewers.extend(
                reviewers
                    .into_iter()
                    .map(|actor| (actor, ReviewerRole::Reviewer)),
            );
            current.reviewed.clear();
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
        Command::MarkReviewed {
            change,
            expected_revision,
            file,
            reviewed,
        } => {
            let current = get(state, &change, &expected_revision)?;
            if !current.current()?.files.contains(&file) {
                return fail("invalid_file", "file is not in the current revision");
            }
            let current = state.changes.get_mut(&change).ok_or_else(|| ReviewError {
                code: "invalid_state".into(),
                message: "validated change".into(),
            })?;
            current.reviewed.retain(|marker| {
                !(marker.actor == actor
                    && marker.revision == expected_revision
                    && marker.file == file)
            });
            if reviewed {
                current.reviewed.push(ReviewedFile {
                    actor: actor.into(),
                    revision: expected_revision,
                    file,
                });
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
    }
    state.used_ids.insert(facts.event_id.clone());
    if let Some((change, kind)) = notification {
        notify(state, &change, actor, kind, facts);
    }
    Ok(())
}

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
    fn ownership_rejects_inconsistent_policy_and_matches_globs_without_io() -> ReviewResult<()> {
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
        for (pattern, path, matched) in [
            ("src/**", "src/lib.rs", true),
            ("src/*", "src/nested/lib.rs", false),
            ("**/lib.?s", "src/nested/lib.rs", true),
            ("**/lib.rs", "lib.rs", true),
            ("src/**/test/*.rs", "src/a/b/test/lib.rs", true),
            ("src/**/test/*.rs", "src/test/lib.rs", true),
            ("src/**/test/*.rs", "src/test/lib.py", false),
            ("src/**", "other/src/lib.rs", false),
            ("src/?.rs", "src/é.rs", true),
        ] {
            assert_eq!(path_matches(pattern, path), matched, "{pattern}: {path}");
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
            reviewed(state.changes.get("c").required()?, "bob")
                .next()
                .required()?
                .revision,
            "c-r1"
        );
        assert!(
            reviewed(state.changes.get("c").required()?, "alice")
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
                    status: None
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
}
