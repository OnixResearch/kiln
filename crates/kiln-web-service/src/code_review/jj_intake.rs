//! Change-ID contribution intake (issue #6): one exact jj change ID maps to one
//! Kiln change; an ordered stack of change IDs maps to one change per ID with
//! ancestor-order dependencies; a pushed branch whose commits carry jj's git
//! `change-id` header uses the same mapping without a jj binary.
//!
//! Kiln stays read-only toward jj (see `kiln_adapter_jj::cli`). Every member
//! is observed and admitted by the pure planner before the first provider
//! effect; each member effect then reuses the journaled Create/Upload path
//! under a derived request id, so a retried request resumes idempotently.
// r[impl kiln.review.jj_intake]
// r[impl kiln.review.jj_intake.stack]
use git2::{Oid, Repository};
use kiln_adapter_jj::cli as jj;
use kiln_core::source::valid_jujutsu_change_id;

use super::store::Journal;
use super::*;

/// Largest stack one request admits (the profile may lower it).
pub const MAX_STACK: usize = jj::MAX_CHANGE_IDS;
/// Namespace of jj mappings in `State.contributions`; branch keys always start
/// with `refs/heads/`, so the two can never collide.
const KEY_PREFIX: &str = "jj:";
const SOURCE_JJ: &str = "jj";
const SOURCE_HEADER: &str = "git_change_id_header";
/// jj writes the change ID into Git commits under this header.
const CHANGE_ID_HEADER: &str = "change-id";
/// jj writes conflicted commits with this header listing the conflict trees.
const CONFLICT_TREES_HEADER: &str = "jj:trees";
const MAX_AUTHOR_BYTES: usize = 256;

/// Operator profile `intake.jujutsu`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JujutsuIntake {
    /// Absolute jj workspace whose Git store is the review repository.
    #[serde(default)]
    pub workspace_path: Option<PathBuf>,
    /// Absolute jj executable; required with `workspace_path`.
    #[serde(default)]
    pub executable: Option<PathBuf>,
    /// Admit branches whose commits carry the git `change-id` header.
    #[serde(default)]
    pub change_id_header: bool,
    pub max_stack: usize,
}

impl JujutsuIntake {
    pub fn admit(&self, provider: &LocalGitReviewProfile) -> Result<(), String> {
        let rejected = || "review jj intake configuration rejected".to_owned();
        if self.max_stack == 0 || self.max_stack > MAX_STACK {
            return Err(rejected());
        }
        match (&self.workspace_path, &self.executable) {
            (Some(workspace), Some(executable)) => {
                if !crate::profile::absolute(workspace) || !crate::profile::absolute(executable) {
                    return Err(rejected());
                }
                // Filesystem reads only: backend and Git store binding. The
                // operation head is observed per request.
                let observed = jj::observe_repository(workspace).map_err(|_| rejected())?;
                let git = intake::repository(provider).map_err(|_| rejected())?;
                let expected = git.path().canonicalize().map_err(|_| rejected())?;
                if observed.backend != kiln_core::source::JujutsuBackend::Git
                    || observed.git_store.as_deref() != Some(expected.as_path())
                {
                    return Err("review jj workspace is not backed by the review repository".into());
                }
                Ok(())
            }
            (None, None) if self.change_id_header => Ok(()),
            _ => Err(rejected()),
        }
    }
}

/// Provider observations of one change-ID intake, kept beside the domain.
/// Omitted from the state document when empty (byte-identical replay).
// r[impl kiln.review.jj_intake.observations.scenario.compatibility]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct JjState {
    /// Kiln revision id -> jj facts observed when it was admitted.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub revisions: BTreeMap<String, RevisionObservation>,
    /// Stack id -> ordered membership.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stacks: BTreeMap<String, Stack>,
}
impl JjState {
    pub(super) fn is_empty(&self) -> bool {
        self.revisions.is_empty() && self.stacks.is_empty()
    }
}

/// jj facts of one admitted revision. Provider observations, never Kiln
/// identities: they explain the revision and never select or authorize it.
// r[impl kiln.review.jj_intake.observations]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionObservation {
    /// `jj` or `git_change_id_header`.
    pub source: String,
    pub change_id: String,
    pub commit: String,
    /// The single jj operation head the commit was observed at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    /// The jj operation that created this commit (evolog).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by_operation: Option<String>,
    /// jj predecessor commits (evolog); empty for header intake.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub predecessors: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_time_seconds: i64,
    pub observed_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Stack {
    pub principal: String,
    /// jj change IDs, bottom to top.
    pub members: Vec<String>,
    /// Change IDs that left the stack; flagged on their change, never abandoned.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<String>,
    pub updated_ms: i64,
}

/// Internal member step, carried by `Action::JjMember` (never deserialized).
#[derive(Clone, Debug, Serialize)]
pub(super) struct Member {
    pub key: String,
    pub contribution: intake::Contribution,
    pub step: MemberStep,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum MemberStep {
    Create {
        commit: String,
        title: String,
        description: String,
        wip: bool,
        #[serde(skip_serializing_if = "domain::Authorship::is_undeclared")]
        authorship: domain::Authorship,
    },
    Upload {
        commit: String,
        expected_version: u64,
        #[serde(skip_serializing_if = "domain::Authorship::is_undeclared")]
        authorship: domain::Authorship,
    },
}

/// Lower an internal member step into the existing journaled Create/Upload
/// action and return the contribution mapping to reserve with it.
pub(super) fn lower(
    action: &mut Action,
    change_id: &mut Option<String>,
    expected_version: &mut Option<u64>,
) -> Option<(String, intake::Contribution)> {
    let Action::JjMember { member } = action else {
        return None;
    };
    let Member {
        key,
        contribution,
        step,
    } = (**member).clone();
    match step {
        MemberStep::Create {
            commit,
            title,
            description,
            wip,
            authorship,
        } => {
            *action = Action::Create {
                commit,
                target_ref: contribution.target_ref.clone(),
                title,
                description,
                wip,
                authorship,
            };
        }
        MemberStep::Upload {
            commit,
            expected_version: version,
            authorship,
        } => {
            *change_id = Some(contribution.change_id.clone());
            *expected_version = Some(version);
            *action = Action::Upload { commit, authorship };
        }
    }
    Some((key, contribution))
}

/// One observed, not yet admitted, member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Observed {
    pub change_id: String,
    pub commit: String,
    pub title: String,
    pub description: String,
    pub observation: RevisionObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Outcome {
    Created,
    Appended,
    Unchanged,
    /// Already submitted: left untouched even if jj rewrote it since.
    Merged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Planned {
    pub observed: Observed,
    pub key: String,
    pub contribution: intake::Contribution,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StackPlan {
    pub id: String,
    pub members: Vec<String>,
    /// Previous members absent from this submission (newly or still removed).
    pub removed: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Plan {
    pub members: Vec<Planned>,
    pub stack: Option<StackPlan>,
}

pub(super) fn key(change_id: &str) -> String {
    format!("{KEY_PREFIX}{change_id}")
}

/// Deterministic Kiln change id of a jj change ID in one repository. Both the
/// jj and the header source use it, so either path reaches the same change.
pub(super) fn kiln_change(repository_id: &str, change_id: &str) -> Result<String, Failure> {
    hash(&json!([
        "kiln.review.jj-change.v1",
        repository_id,
        change_id
    ]))
}

/// Published lookup (#5): the Kiln change of an exact jj change ID.
#[allow(dead_code)]
pub(super) fn change_for_jj<'a>(state: &'a State, change_id: &str) -> Option<&'a str> {
    state
        .contributions
        .get(&key(change_id))
        .map(|item| item.change_id.as_str())
}

/// Published lookup (#5): the jj facts recorded for one Kiln revision.
pub(super) fn observation<'a>(state: &'a State, revision: &str) -> Option<&'a RevisionObservation> {
    state.jj.revisions.get(revision)
}

fn jj_change_of<'a>(state: &'a State, change: &str) -> Option<&'a str> {
    state
        .contributions
        .iter()
        .find(|(key, item)| key.starts_with(KEY_PREFIX) && item.change_id == change)
        .and_then(|(key, _)| key.strip_prefix(KEY_PREFIX))
}

/// Map an adapter diagnostic to a stable `review_jj_*` API refusal.
fn refused(error: &kiln_core::types::Diagnostic) -> Failure {
    let status = match error.code.as_str() {
        "change_id_invalid" | "change_duplicate" | "stack_bound" => 400,
        "execution_failed" | "workspace_invalid" | "store_unavailable" | "observation_invalid" => {
            503
        }
        _ => 409,
    };
    Failure::Api(status, format!("review_jj_{}", error.code))
}

/// Pure request admission: configuration, principal, exactly one source,
/// full unique change IDs and the stack bound. No I/O.
// r[impl kiln.review.jj_intake.admission]
pub(super) fn admit_request<'a>(
    profile: &'a Profile,
    actor: &str,
    change_ids: &[String],
    source_ref: Option<&str>,
    stack: bool,
) -> Result<&'a JujutsuIntake, Failure> {
    let config = profile
        .intake
        .as_ref()
        .and_then(|intake| intake.jujutsu.as_ref())
        .ok_or_else(|| Failure::Api(403, "review_jj_intake_disabled".into()))?;
    if !profile.write_principals.iter().any(|p| p == actor) {
        return reject(403, "review_source_owner_denied");
    }
    match (change_ids.is_empty(), source_ref) {
        (false, None) => {
            if config.workspace_path.is_none() {
                return reject(403, "review_jj_workspace_unconfigured");
            }
            if change_ids.len() > config.max_stack {
                return reject(400, "review_jj_stack_bound");
            }
            for (index, id) in change_ids.iter().enumerate() {
                if !valid_jujutsu_change_id(id) {
                    return reject(400, "review_jj_change_id_invalid");
                }
                if change_ids.iter().take(index).any(|other| other == id) {
                    return reject(400, "review_jj_change_duplicate");
                }
            }
            if change_ids.len() > 1 && !stack {
                return reject(400, "review_jj_stack_required");
            }
        }
        (true, Some(source_ref)) => {
            if !config.change_id_header {
                return reject(403, "review_jj_header_disabled");
            }
            if source_ref.len() > MAX_IDENTITY_BYTES {
                return reject(400, "review_source_ref_invalid");
            }
        }
        _ => return reject(400, "review_jj_source_invalid"),
    }
    Ok(config)
}

/// Pure planner over every observed member. Any refusal refuses the whole
/// request before a provider effect, reservation or state change.
// r[impl kiln.review.jj_intake.scenario.rewritten]
// r[impl kiln.review.jj_intake.scenario.replay]
// r[impl kiln.review.jj_intake.stack.scenario.create]
// r[impl kiln.review.jj_intake.stack.scenario.partial]
pub(super) fn plan(
    profile: &Profile,
    state: &State,
    actor: &str,
    target_ref: &str,
    observed: Vec<Observed>,
    stack: bool,
) -> Result<Plan, Failure> {
    let mut members = Vec::with_capacity(observed.len());
    let mut creates = 0usize;
    let mut new_keys = 0usize;
    for observed in observed {
        let key = key(&observed.change_id);
        let change_id = kiln_change(&profile.provider.repository_id, &observed.change_id)?;
        let prior = state.contributions.get(&key);
        if let Some(prior) = prior {
            if prior.principal != actor {
                return reject(403, "review_source_owner_denied");
            }
            if prior.target_ref != target_ref {
                return reject(409, "review_source_mapping_changed");
            }
            if prior.change_id != change_id {
                return Err(Failure::Unavailable);
            }
        } else {
            new_keys += 1;
        }
        if state.operations.iter().any(|operation| {
            matches!(operation.state.as_str(), "pending" | "uncertain")
                && (operation.change == change_id
                    || operation.result_change.as_ref() == Some(&change_id))
        }) {
            return reject(409, "review_change_reserved");
        }
        let contribution = prior.cloned().unwrap_or_else(|| intake::Contribution {
            principal: actor.into(),
            change_id: change_id.clone(),
            target_ref: target_ref.into(),
            commit: None,
        });
        let outcome = match state.review.changes.get(&change_id) {
            None => {
                if contribution.commit.is_some() {
                    return Err(Failure::Unavailable);
                }
                if !valid_text(&observed.title, MAX_TITLE_BYTES) {
                    return reject(400, "review_jj_title_missing");
                }
                if observed.description.len() > MAX_DESCRIPTION_BYTES {
                    return reject(400, "review_action_bound");
                }
                creates += 1;
                Outcome::Created
            }
            Some(change) => {
                if prior.is_none()
                    || change.owner != actor
                    || change.project != profile.project
                    || change.repository != profile.provider.repository_id
                    || change.target_ref != target_ref
                {
                    return reject(409, "review_source_change_mismatch");
                }
                let current = change.current()?;
                match change.state {
                    domain::ChangeStatus::Submitted => Outcome::Merged,
                    domain::ChangeStatus::Abandoned if stack => {
                        return reject(409, "review_jj_change_abandoned");
                    }
                    _ if current.commit == observed.commit => Outcome::Unchanged,
                    domain::ChangeStatus::Abandoned => {
                        return reject(409, "review_source_change_not_open");
                    }
                    domain::ChangeStatus::Open => {
                        if change.revisions.iter().any(|r| r.commit == observed.commit) {
                            return reject(409, "review_source_commit_replayed");
                        }
                        if change.revisions.len() >= MAX_REVISIONS {
                            return reject(409, "review_revision_bound");
                        }
                        Outcome::Appended
                    }
                }
            }
        };
        members.push(Planned {
            observed,
            key,
            contribution,
            outcome,
        });
    }
    if state.review.changes.len().saturating_add(creates) > MAX_CHANGES {
        return reject(409, "review_change_bound");
    }
    if state.contributions.len().saturating_add(new_keys) > intake::MAX_CONTRIBUTIONS {
        return reject(409, "review_source_mapping_bound");
    }
    let stack = if stack {
        Some(stack_plan(profile, state, actor, &members)?)
    } else {
        None
    };
    Ok(Plan { members, stack })
}

// r[impl kiln.review.jj_intake.stack.scenario.removed]
/// Pure: locate the stack these members continue (the lowest member already
/// in a stack), else a new stack keyed by the bottom change, and compute the
/// members that left it.
fn stack_plan(
    profile: &Profile,
    state: &State,
    actor: &str,
    members: &[Planned],
) -> Result<StackPlan, Failure> {
    let ids: Vec<String> = members
        .iter()
        .map(|member| member.observed.change_id.clone())
        .collect();
    let existing = ids.iter().find_map(|id| {
        state
            .jj
            .stacks
            .iter()
            .find(|(_, stack)| stack.members.contains(id) || stack.removed.contains(id))
    });
    let (id, previous) = match existing {
        Some((id, stack)) => {
            if stack.principal != actor {
                return reject(403, "review_source_owner_denied");
            }
            (id.clone(), Some(stack))
        }
        None => (
            hash(&json!([
                "kiln.review.jj-stack.v1",
                profile.provider.repository_id,
                actor,
                ids.first()
            ]))?,
            None,
        ),
    };
    let mut removed: Vec<String> = Vec::new();
    if let Some(previous) = previous {
        for id in previous.removed.iter().chain(&previous.members) {
            if !ids.contains(id) && !removed.contains(id) {
                removed.push(id.clone());
            }
        }
    }
    Ok(StackPlan {
        id,
        members: ids,
        removed,
    })
}

/// Pure: apply a stack plan; members move out of any other stack.
fn apply_stack(jj: &mut JjState, plan: &StackPlan, actor: &str, now_ms: i64) {
    for (id, stack) in &mut jj.stacks {
        if *id != plan.id {
            stack
                .members
                .retain(|member| !plan.members.contains(member));
            stack
                .removed
                .retain(|member| !plan.members.contains(member));
        }
    }
    jj.stacks.retain(|id, stack| {
        *id == plan.id || !stack.members.is_empty() || !stack.removed.is_empty()
    });
    jj.stacks.insert(
        plan.id.clone(),
        Stack {
            principal: actor.into(),
            members: plan.members.clone(),
            removed: plan.removed.clone(),
            updated_ms: now_ms,
        },
    );
}

/// Pure: the dependency edges stack intake owns for member `index`.
// r[impl kiln.review.jj_intake.stack.scenario.reorder]
fn desired_dependencies(
    state: &State,
    plan: &Plan,
    index: usize,
) -> Result<Vec<domain::Dependency>, Failure> {
    let Some(previous) = index.checked_sub(1).and_then(|i| plan.members.get(i)) else {
        return Ok(Vec::new());
    };
    let change = state
        .review
        .changes
        .get(&previous.contribution.change_id)
        .ok_or(Failure::Unavailable)?;
    Ok(vec![domain::Dependency {
        change: change.id.clone(),
        revision: change.current()?.id.clone(),
    }])
}

fn author_text(value: Option<&str>) -> String {
    let mut text: String = value
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    while text.len() > MAX_AUTHOR_BYTES {
        text.pop();
    }
    text
}

/// Read title, description and author from the exact Git commit object.
fn describe<'r>(
    repository: &'r Repository,
    oid: Oid,
    change_id: &str,
    source: &str,
    now_ms: i64,
) -> Result<(git2::Commit<'r>, Observed), Failure> {
    let commit = repository
        .find_commit(oid)
        .map_err(|_| Failure::Api(409, "review_jj_commit_unreachable".into()))?;
    let signature = commit.author();
    let (author_name, author_email, author_time_seconds) = (
        author_text(signature.name()),
        author_text(signature.email()),
        signature.when().seconds(),
    );
    drop(signature);
    let observed = Observed {
        change_id: change_id.into(),
        commit: oid.to_string(),
        title: commit.summary().unwrap_or("").trim().to_owned(),
        description: commit.body().unwrap_or("").trim().to_owned(),
        observation: RevisionObservation {
            source: source.into(),
            change_id: change_id.into(),
            commit: oid.to_string(),
            operation: None,
            created_by_operation: None,
            predecessors: Vec::new(),
            author_name,
            author_email,
            author_time_seconds,
            observed_ms: now_ms,
        },
    };
    Ok((commit, observed))
}

fn header(commit: &git2::Commit<'_>, field: &str) -> Option<String> {
    commit
        .header_field_bytes(field)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
}

/// Shell: observe change IDs through the read-only jj adapter, then bind each
/// resolved commit to the review repository's exact Git object.
fn observe_jj(
    profile: &Profile,
    config: &JujutsuIntake,
    change_ids: &[String],
    now_ms: i64,
) -> Result<Vec<Observed>, Failure> {
    let (Some(workspace), Some(executable)) = (&config.workspace_path, &config.executable) else {
        return reject(403, "review_jj_workspace_unconfigured");
    };
    let observation = jj::observe_changes(
        &jj::Workspace {
            executable: executable.clone(),
            workspace: workspace.clone(),
        },
        change_ids,
    )
    .map_err(|error| refused(&error))?;
    let repository = intake::repository(&profile.provider)?;
    let expected = repository
        .path()
        .canonicalize()
        .map_err(|_| Failure::Unavailable)?;
    if observation.git_store != expected {
        return reject(409, "review_jj_store_mismatch");
    }
    let mut observed = Vec::with_capacity(observation.changes.len());
    for change in observation.changes {
        let oid = Oid::from_str(&change.commit).map_err(|_| Failure::Unavailable)?;
        let (commit, mut item) = describe(&repository, oid, &change.change_id, SOURCE_JJ, now_ms)?;
        if header(&commit, CHANGE_ID_HEADER).is_some_and(|value| value != change.change_id) {
            return reject(409, "review_jj_header_mismatch");
        }
        item.observation.operation = Some(observation.operation.clone());
        item.observation.created_by_operation = change.evolog.operation;
        item.observation.predecessors = change.evolog.predecessors;
        observed.push(item);
    }
    ancestor_order(&repository, &observed)?;
    Ok(observed)
}

/// Shell: an admitted branch whose commits carry jj `change-id` headers.
/// Single mode takes the tip; stack mode walks first parents down to the
/// merge base with the target. No jj process is involved.
// r[impl kiln.review.jj_intake.header]
// r[impl kiln.review.jj_intake.header.scenario.stack]
fn observe_header(
    profile: &Profile,
    config: &JujutsuIntake,
    source_ref: &str,
    target_ref: &str,
    stack: bool,
    now_ms: i64,
) -> Result<Vec<Observed>, Failure> {
    let tip = intake::observe(&profile.provider, source_ref)?;
    let repository = intake::repository(&profile.provider)?;
    let tip = Oid::from_str(&tip).map_err(|_| Failure::Unavailable)?;
    let mut chain = vec![tip];
    if stack {
        let target = repository
            .refname_to_id(target_ref)
            .map_err(|_| Failure::Api(409, "review_target_missing".into()))?;
        let base = repository
            .merge_base(tip, target)
            .map_err(|_| Failure::Api(409, "review_jj_header_unrelated".into()))?;
        if base == tip {
            return reject(409, "review_jj_header_empty");
        }
        chain.clear();
        let mut cursor = tip;
        while cursor != base {
            if chain.len() >= config.max_stack {
                return reject(400, "review_jj_stack_bound");
            }
            let commit = repository
                .find_commit(cursor)
                .map_err(|_| Failure::Api(409, "review_jj_commit_unreachable".into()))?;
            if commit.parent_count() != 1 {
                return reject(409, "review_jj_header_merge");
            }
            chain.push(cursor);
            cursor = commit.parent_id(0).map_err(|_| Failure::Unavailable)?;
        }
        chain.reverse();
    }
    let mut observed: Vec<Observed> = Vec::with_capacity(chain.len());
    for oid in chain {
        let commit = repository
            .find_commit(oid)
            .map_err(|_| Failure::Api(409, "review_jj_commit_unreachable".into()))?;
        let change_id = header(&commit, CHANGE_ID_HEADER)
            .filter(|value| valid_jujutsu_change_id(value))
            .ok_or_else(|| Failure::Api(409, "review_jj_header_missing".into()))?;
        if header(&commit, CONFLICT_TREES_HEADER).is_some() {
            return reject(409, "review_jj_conflicted_commit");
        }
        if observed.iter().any(|item| item.change_id == change_id) {
            return reject(409, "review_jj_divergent_change");
        }
        observed.push(describe(&repository, oid, &change_id, SOURCE_HEADER, now_ms)?.1);
    }
    Ok(observed)
}

/// Stack members must be listed bottom to top in ancestor order.
fn ancestor_order(repository: &Repository, observed: &[Observed]) -> Result<(), Failure> {
    for (below, above) in observed.iter().zip(observed.iter().skip(1)) {
        let lower = Oid::from_str(&below.commit).map_err(|_| Failure::Unavailable)?;
        let upper = Oid::from_str(&above.commit).map_err(|_| Failure::Unavailable)?;
        if !repository
            .graph_descendant_of(upper, lower)
            .map_err(|_| Failure::Unavailable)?
        {
            return reject(409, "review_jj_stack_order");
        }
    }
    Ok(())
}

fn derived(kind: &str, request_id: &str, parts: &Value) -> Result<String, Failure> {
    hash(&json!([kind, request_id, parts]))
}

/// Record the observation for the member's current revision when it is the
/// observed commit and nothing was recorded yet (back-fills on replay).
fn record(state: &mut State, planned: &Planned) {
    let Some(revision) = state
        .review
        .changes
        .get(&planned.contribution.change_id)
        .and_then(|change| change.current().ok())
        .filter(|revision| revision.commit == planned.observed.commit)
        .map(|revision| revision.id.clone())
    else {
        return;
    };
    state
        .jj
        .revisions
        .entry(revision)
        .or_insert_with(|| planned.observed.observation.clone());
}

impl Workspace {
    /// Change-ID intake orchestration. Admission of every member precedes the
    /// first provider effect; member effects reuse journaled commands.
    pub(super) fn jj_contribute(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        request: Request,
        request_context: &RequestContext<'_>,
    ) -> Result<ApiResponse, Failure> {
        let ProviderContext {
            profile,
            provider,
            binding,
        } = *context;
        let actor = request_context.principal;
        let now_ms = request_context.now_ms;
        let Action::JjContribute {
            change_ids,
            source_ref,
            stack,
            wip,
            authorship,
        } = &request.action
        else {
            return Err(Failure::Unavailable);
        };
        if request.change_id.is_some() || request.expected_version.is_some() {
            return reject(400, "review_contribution_global_required");
        }
        if domain::validate_authorship(authorship).is_err() {
            return reject(400, "review_authorship_invalid");
        }
        let config = admit_request(profile, actor, change_ids, source_ref.as_deref(), *stack)?;
        let intake = profile.intake.as_ref().ok_or(Failure::Unavailable)?;
        let (target_ref, observed) = match source_ref {
            None => (
                intake.default_target_ref.clone(),
                observe_jj(profile, config, change_ids, now_ms)?,
            ),
            Some(source_ref) => {
                let target = intake
                    .target(&profile.provider, actor, source_ref)?
                    .to_owned();
                let observed =
                    observe_header(profile, config, source_ref, &target, *stack, now_ms)?;
                (target, observed)
            }
        };
        self.require_target(profile, &target_ref)?;
        let plan = plan(profile, state, actor, &target_ref, observed, *stack)?;
        // Provider reads only: every new commit must be inspectable against
        // the target before any member effect is reserved.
        for member in &plan.members {
            if matches!(member.outcome, Outcome::Created | Outcome::Appended) {
                provider.inspect(&member.observed.commit, &target_ref)?;
            }
        }
        let digest = hash(&request)?;
        let mut results = Vec::with_capacity(plan.members.len());
        for (index, member) in plan.members.iter().enumerate() {
            let step = match member.outcome {
                Outcome::Created => Some(MemberStep::Create {
                    commit: member.observed.commit.clone(),
                    title: member.observed.title.clone(),
                    description: member.observed.description.clone(),
                    wip: *wip,
                    authorship: authorship.clone(),
                }),
                Outcome::Appended => Some(MemberStep::Upload {
                    commit: member.observed.commit.clone(),
                    expected_version: state
                        .versions
                        .get(&member.contribution.change_id)
                        .copied()
                        .unwrap_or(0),
                    authorship: authorship.clone(),
                }),
                Outcome::Unchanged | Outcome::Merged => None,
            };
            if let Some(step) = step {
                let sub = Request {
                    request_id: derived(
                        "kiln.review.jj-member.v1",
                        &request.request_id,
                        &json!([member.observed.change_id, member.observed.commit]),
                    )?,
                    change_id: None,
                    expected_version: None,
                    action: Action::JjMember {
                        member: Box::new(Member {
                            key: member.key.clone(),
                            contribution: member.contribution.clone(),
                            step,
                        }),
                    },
                };
                let response = self.command(context, store, state, sub, request_context)?;
                if response.status != 200 {
                    return Ok(response);
                }
            }
            // Persisted with the next member command or the final journal; a
            // crash before that is back-filled when the request is retried.
            record(state, member);
            let mut dependencies_updated = false;
            if plan.stack.is_some() && member.outcome != Outcome::Merged {
                let desired = desired_dependencies(state, &plan, index)?;
                let change = state
                    .review
                    .changes
                    .get(&member.contribution.change_id)
                    .ok_or(Failure::Unavailable)?;
                if change.dependencies != desired {
                    let sub = Request {
                        request_id: derived(
                            "kiln.review.jj-dependencies.v1",
                            &request.request_id,
                            &json!([change.id, desired]),
                        )?,
                        change_id: Some(change.id.clone()),
                        expected_version: Some(
                            state.versions.get(&change.id).copied().unwrap_or(0),
                        ),
                        action: Action::SetDependencies {
                            dependencies: desired,
                        },
                    };
                    let response = self.command(context, store, state, sub, request_context)?;
                    if response.status != 200 {
                        return Ok(response);
                    }
                    dependencies_updated = true;
                }
            }
            let change = state.review.changes.get(&member.contribution.change_id);
            results.push(json!({
                "jj_change_id": member.observed.change_id,
                "commit": member.observed.commit,
                "outcome": member.outcome,
                "change": change.map(|c| json!({"id": c.id, "version": state.versions.get(&c.id).copied().unwrap_or(0)})),
                "revision": change.and_then(|c| c.current().ok()).map(|r| json!({"id": r.id, "number": r.number})),
                "dependencies_updated": dependencies_updated,
            }));
        }
        let mut next = state.clone();
        let mut removed = Vec::new();
        if let Some(stack) = &plan.stack {
            apply_stack(&mut next.jj, stack, actor, now_ms);
            for id in &stack.removed {
                removed.push(json!({"jj_change_id": id, "change": change_for_jj(&next, id)}));
            }
        }
        let top = plan
            .members
            .last()
            .map(|member| member.contribution.change_id.clone());
        let body = json!({
            "change": top.filter(|id| next.review.changes.contains_key(id)).map(|id| json!({"id": id, "version": next.versions.get(&id).copied().unwrap_or(0)})),
            "replayed": false,
            "operation": null,
            "jj_intake": {
                "source": if source_ref.is_some() { SOURCE_HEADER } else { SOURCE_JJ },
                "target_ref": target_ref,
                "stack": plan.stack.as_ref().map(|stack| &stack.id),
                "members": results,
                "removed": removed,
            },
        });
        let journal = Journal {
            principal: actor.into(),
            request_id: request.request_id.clone(),
            digest,
            pending: None,
            response: Some(json!({"status": 200, "body": body})),
        };
        store.commit(binding, &next, Some(&journal))?;
        *state = next;
        json_response(200, body).map_err(Failure::from)
    }
}

/// Change-detail evolog: every revision with its jj facts and the interdiff
/// link from the previous revision (`review_interdiff=<from>..<to>`).
// r[impl kiln.review.jj_intake.evolog]
pub(super) fn evolog(state: &State, change: &domain::Change) -> Value {
    if !change
        .revisions
        .iter()
        .any(|revision| state.jj.revisions.contains_key(&revision.id))
    {
        return Value::Null;
    }
    let mut previous: Option<&domain::Revision> = None;
    let mut rows = Vec::with_capacity(change.revisions.len());
    for revision in &change.revisions {
        let observed = state.jj.revisions.get(&revision.id);
        rows.push(json!({
            "revision": revision.id,
            "number": revision.number,
            "commit": revision.commit,
            "created_ms": revision.created_ms,
            "submitted_by": revision.author,
            "authorship": revision.authorship,
            "jj": observed,
            "predecessor_matches_previous": previous.zip(observed).map(|(previous, observed)| observed.predecessors.contains(&previous.commit)),
            "interdiff": previous.map(|previous| format!("{}..{}", previous.id, revision.id)),
        }));
        previous = Some(revision);
    }
    Value::Array(rows)
}

/// Change-detail stack membership and the removed flag.
pub(super) fn stack_view(state: &State, change: &domain::Change) -> Value {
    let Some(jj_change) = jj_change_of(state, &change.id) else {
        return Value::Null;
    };
    let found = state.jj.stacks.iter().find(|(_, stack)| {
        stack.members.iter().any(|m| m == jj_change) || stack.removed.iter().any(|m| m == jj_change)
    });
    match found {
        None => json!({"jj_change_id": jj_change, "stack": null}),
        Some((id, stack)) => {
            let position = stack.members.iter().position(|m| m == jj_change);
            json!({
                "jj_change_id": jj_change,
                "stack": id,
                "position": position.map(|p| p + 1),
                "size": stack.members.len(),
                "removed": position.is_none(),
                "members": stack.members.iter().map(|m| json!({"jj_change_id": m, "change": change_for_jj(state, m)})).collect::<Vec<_>>(),
                "updated_ms": stack.updated_ms,
            })
        }
    }
}

/// Overview flag telling the console which change-ID sources are enabled.
pub(super) fn published(profile: &Profile) -> Value {
    match profile.intake.as_ref().and_then(|i| i.jujutsu.as_ref()) {
        None => Value::Null,
        Some(config) => json!({
            "jj": config.workspace_path.is_some(),
            "change_id_header": config.change_id_header,
            "max_stack": config.max_stack,
            "target_ref": profile.intake.as_ref().map(|i| &i.default_target_ref),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "wnloytsvxwxwxtpqyuwxutmotlmslszv";
    const B: &str = "kvpqzqvnyrywpssxpqtmuvqxuvvkpqnt";
    const C: &str = "onzuuxporuotqxvzkptkvvntupqmpmyw";
    const ALICE: &str = "local:author";

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn profile(jujutsu: Value) -> TestResult<Profile> {
        Ok(serde_json::from_value(json!({
            "schema":"kiln.code-review-profile.v1","database":"/tmp/review.sqlite","project":"project",
            "provider":{"repository_id":"r","repository_path":"/tmp/repo","target_refs":["refs/heads/main"],
                "capabilities":["inspect","compare","register"]},
            "policy":{"id":"p","eligible_reviewers":[],"submitters":[],"minimum_approvals":1,
                "require_ci":false,"ownership":{"teams":{},"rules":[]}},
            "read_principals":[ALICE,"local:reader"],"write_principals":[ALICE],
            "signature_name":"Review","signature_email":"review@example.invalid",
            "intake":{"default_target_ref":"refs/heads/main","mappings":[],"jujutsu":jujutsu}
        }))?)
    }

    fn code<T: core::fmt::Debug>(result: Result<T, Failure>) -> String {
        match result {
            Err(Failure::Api(status, code)) => format!("{status} {code}"),
            other => format!("{other:?}"),
        }
    }

    fn observed(change_id: &str, commit: &str) -> Observed {
        Observed {
            change_id: change_id.into(),
            commit: commit.into(),
            title: "Title".into(),
            description: String::new(),
            observation: RevisionObservation {
                source: SOURCE_JJ.into(),
                change_id: change_id.into(),
                commit: commit.into(),
                operation: None,
                created_by_operation: None,
                predecessors: Vec::new(),
                author_name: "A".into(),
                author_email: "a@example.org".into(),
                author_time_seconds: 0,
                observed_ms: 0,
            },
        }
    }

    // r[verify kiln.review.jj_intake.admission]
    #[test]
    fn request_admission_requires_configuration_principal_and_exact_ids() -> TestResult {
        let both = profile(
            json!({"workspace_path":"/w","executable":"/bin/jj","change_id_header":true,"max_stack":2}),
        )?;
        let ids = |values: &[&str]| values.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>();
        assert!(admit_request(&both, ALICE, &ids(&[A]), None, false).is_ok());
        assert!(admit_request(&both, ALICE, &ids(&[A, B]), None, true).is_ok());
        assert!(admit_request(&both, ALICE, &[], Some("refs/heads/review/x"), true).is_ok());
        for (actor, values, source, stack, expected) in [
            (
                "local:reader",
                vec![A],
                None,
                false,
                "403 review_source_owner_denied",
            ),
            (ALICE, vec![], None, false, "400 review_jj_source_invalid"),
            (
                ALICE,
                vec![A],
                Some("refs/heads/review/x"),
                false,
                "400 review_jj_source_invalid",
            ),
            (
                ALICE,
                vec!["wnloyt"],
                None,
                false,
                "400 review_jj_change_id_invalid",
            ),
            (
                ALICE,
                vec!["@-"],
                None,
                false,
                "400 review_jj_change_id_invalid",
            ),
            (
                ALICE,
                vec![A, A],
                None,
                true,
                "400 review_jj_change_duplicate",
            ),
            (
                ALICE,
                vec![A, B],
                None,
                false,
                "400 review_jj_stack_required",
            ),
            (
                ALICE,
                vec![A, B, C],
                None,
                true,
                "400 review_jj_stack_bound",
            ),
        ] {
            assert_eq!(
                code(admit_request(&both, actor, &ids(&values), source, stack)),
                expected
            );
        }
        let header_only = profile(json!({"change_id_header":true,"max_stack":2}))?;
        assert_eq!(
            code(admit_request(&header_only, ALICE, &ids(&[A]), None, false)),
            "403 review_jj_workspace_unconfigured"
        );
        let jj_only = profile(json!({"workspace_path":"/w","executable":"/bin/jj","max_stack":2}))?;
        assert_eq!(
            code(admit_request(
                &jj_only,
                ALICE,
                &[],
                Some("refs/heads/review/x"),
                false
            )),
            "403 review_jj_header_disabled"
        );
        let disabled = profile(Value::Null)?;
        assert_eq!(
            code(admit_request(&disabled, ALICE, &ids(&[A]), None, false)),
            "403 review_jj_intake_disabled"
        );
        Ok(())
    }

    // r[verify kiln.review.jj_intake.stack]
    #[test]
    fn planner_creates_then_maps_by_change_id_and_flags_removed_members() -> TestResult {
        let profile = profile(json!({"change_id_header":true,"max_stack":8}))?;
        let mut state = State::default();
        let first = plan(
            &profile,
            &state,
            ALICE,
            "refs/heads/main",
            vec![observed(A, "1"), observed(B, "2"), observed(C, "3")],
            true,
        )
        .map_err(|error| format!("{error:?}"))?;
        assert!(first.members.iter().all(|m| m.outcome == Outcome::Created));
        let stack = first.stack.clone().unwrap_or_else(|| unreachable!());
        assert_eq!(stack.members, [A, B, C]);
        assert!(stack.removed.is_empty());
        apply_stack(&mut state.jj, &stack, ALICE, 1);
        // Another principal's stack is never continued.
        assert_eq!(
            code(stack_plan(&profile, &state, "local:other", &first.members)),
            "403 review_source_owner_denied"
        );
        // Dropping B keeps the stack id and flags B; re-adding clears it.
        let second = plan(
            &profile,
            &state,
            ALICE,
            "refs/heads/main",
            vec![observed(A, "1"), observed(C, "3")],
            true,
        )
        .map_err(|error| format!("{error:?}"))?;
        let shortened = second.stack.unwrap_or_else(|| unreachable!());
        assert_eq!(shortened.id, stack.id);
        assert_eq!(shortened.removed, [B]);
        apply_stack(&mut state.jj, &shortened, ALICE, 2);
        let third = stack_plan(&profile, &state, ALICE, &first.members)
            .map_err(|error| format!("{error:?}"))?;
        assert!(third.removed.is_empty());
        // Single-change intake never touches stack membership.
        let single = plan(
            &profile,
            &state,
            ALICE,
            "refs/heads/main",
            vec![observed(B, "2")],
            false,
        )
        .map_err(|error| format!("{error:?}"))?;
        assert!(single.stack.is_none());
        // A mapping owned by another principal or bound to another target is refused.
        state.contributions.insert(
            key(A),
            intake::Contribution {
                principal: "local:other".into(),
                change_id: kiln_change("r", A).unwrap_or_default(),
                target_ref: "refs/heads/main".into(),
                commit: None,
            },
        );
        assert_eq!(
            code(plan(
                &profile,
                &state,
                ALICE,
                "refs/heads/main",
                vec![observed(A, "1")],
                false
            )),
            "403 review_source_owner_denied"
        );
        let mut untitled = observed(B, "2");
        untitled.title = String::new();
        assert_eq!(
            code(plan(
                &profile,
                &state,
                ALICE,
                "refs/heads/main",
                vec![untitled],
                false
            )),
            "400 review_jj_title_missing"
        );
        Ok(())
    }

    // r[verify kiln.review.jj_intake.observations.scenario.compatibility]
    #[test]
    fn absent_jj_fields_keep_state_profile_and_requests_byte_identical() -> TestResult {
        let encoded = serde_json::to_string(&State::default()).unwrap_or_default();
        assert!(!encoded.contains("\"jj\""));
        let decoded: State = serde_json::from_str(&encoded)?;
        assert_eq!(serde_json::to_string(&decoded).unwrap_or_default(), encoded);
        let legacy = r#"{"default_target_ref":"refs/heads/main","mappings":[]}"#;
        let intake: intake::IntakeProfile = serde_json::from_str(legacy)?;
        assert_eq!(serde_json::to_string(&intake).unwrap_or_default(), legacy);
        let request = r#"{"request_id":"r","change_id":null,"expected_version":null,"action":{"type":"jj_contribute","change_ids":["wnloytsvxwxwxtpqyuwxutmotlmslszv"],"stack":false,"wip":false}}"#;
        let parsed: Request = serde_json::from_str(request)?;
        assert_eq!(serde_json::to_string(&parsed).unwrap_or_default(), request);
        // The internal member step is never accepted from a client.
        let internal = r#"{"request_id":"r","change_id":null,"expected_version":null,"action":{"type":"jj_member","member":{}}}"#;
        assert!(serde_json::from_str::<Request>(internal).is_err());
        Ok(())
    }
}
