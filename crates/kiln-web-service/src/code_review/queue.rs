//! FIFO integration queue over exact reviewed sources and canonical candidate CI.
//! r[impl kiln.review.parity.queue]
use kiln_core::gating::WindowPolicy;

use super::commands::{Completion, Pending};
use super::speculation::{self, Chain, Link, Verdict};
use super::*;

const MAX_QUEUE_ENTRIES: usize = 1000;
const MAX_CANDIDATE_BUILDS: u32 = 128;
const MAX_TICK_STEPS: usize = 4;

/// `serial` (default) advances only each target's head; `speculative` stacks
/// candidates inside an adaptive window (`kiln_core::gating::GateMode`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueMode {
    #[default]
    Serial,
    Speculative,
}
impl QueueMode {
    fn is_serial(&self) -> bool {
        *self == Self::Serial
    }
}

/// Profile form of `kiln_core::gating::WindowPolicy`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueWindow {
    pub floor: usize,
    pub ceiling: usize,
    pub increase: usize,
    pub decrease: usize,
}
impl From<QueueWindow> for WindowPolicy {
    fn from(window: QueueWindow) -> Self {
        Self {
            floor: window.floor,
            ceiling: window.ceiling,
            increase: window.increase,
            decrease: window.decrease,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub max_entries: usize,
    pub max_candidate_builds: u32,
    // r[impl kiln.review.queue.speculative.profile]
    /// Absent means serial, so existing profiles replay unchanged.
    #[serde(default, skip_serializing_if = "QueueMode::is_serial")]
    pub mode: QueueMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<QueueWindow>,
}
impl Profile {
    pub(super) fn admit(&self, profile: &super::Profile) -> Result<(), String> {
        if self.schema != "kiln.review-queue.v1"
            || self.max_entries == 0
            || self.max_entries > MAX_QUEUE_ENTRIES
            || self.max_candidate_builds == 0
            || self.max_candidate_builds > MAX_CANDIDATE_BUILDS
            || !profile.policy.require_ci
            || ![ReviewCapability::BuildIntegration, ReviewCapability::Submit]
                .iter()
                .all(|capability| profile.provider.capabilities.contains(capability))
        {
            return Err(
                "review queue requires bounded exact integration, submission and required CI"
                    .into(),
            );
        }
        let speculative = match (self.mode, &self.window) {
            (QueueMode::Serial, None) => return Ok(()),
            (QueueMode::Serial, Some(_)) | (QueueMode::Speculative, None) => false,
            (QueueMode::Speculative, Some(window)) => {
                speculation::window_admissible(&(*window).into(), self.max_entries)
                    && profile
                        .provider
                        .capabilities
                        .contains(&ReviewCapability::SpeculativeIntegration)
            }
        };
        if !speculative {
            return Err(
                "speculative review queue requires a bounded window and speculative_integration; serial forbids a window"
                    .into(),
            );
        }
        Ok(())
    }
    fn window_policy(&self) -> Option<WindowPolicy> {
        (self.mode == QueueMode::Speculative)
            .then_some(self.window)
            .flatten()
            .map(WindowPolicy::from)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Entry {
    pub id: String,
    pub change: String,
    pub revision: String,
    pub actor: String,
    pub target_ref: String,
    pub source_commit: String,
    pub target_commit: String,
    pub candidate_commit: Option<String>,
    pub ci_run: Option<String>,
    /// Every candidate CI run of a monorepo review (one per affected project);
    /// `None` with `ci_run` set is a single-project entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_runs: Option<Vec<String>>,
    pub status: String,
    pub detail: String,
    pub created_ms: i64,
    pub generation: u32,
    pub policy_id: String,
    /// Assumed predecessor chain of a speculative candidate; serial entries omit it.
    // r[impl kiln.review.queue.speculative.chain]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<Chain>,
    /// Retired chain identities whose late results are rejected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub superseded_chains: Vec<String>,
}
fn active(entry: &Entry) -> bool {
    matches!(
        entry.status.as_str(),
        "queued" | "preparing" | "waiting_ci" | "submitting" | "uncertain"
    )
}

pub(super) fn enqueue(
    profile: &super::Profile,
    state: &mut State,
    change_id: &str,
    actor: &str,
    expected_target: &str,
    facts: &domain::Facts,
) -> Result<(), Failure> {
    let event = facts.event_id.as_str();
    let now_ms = facts.now_ms;
    let config = profile
        .queue
        .as_ref()
        .ok_or_else(|| Failure::Api(403, "review_queue_disabled".into()))?;
    if state.queue.len() >= config.max_entries {
        return reject(409, "review_queue_bound");
    }
    let policy = facts.policy.as_ref().ok_or(Failure::Unavailable)?;
    if !policy.submitters.contains(actor) || !profile.write_principals.iter().any(|p| p == actor) {
        return reject(403, "review_submitter_required");
    }
    let change = state
        .review
        .changes
        .get(change_id)
        .ok_or(Failure::Unavailable)?;
    let source = facts.source.as_ref().ok_or(Failure::Unavailable)?;
    if source.current_target != expected_target {
        return reject(409, "review_target_moved");
    }
    if state
        .queue
        .iter()
        .any(|entry| entry.change == change_id && active(entry))
    {
        return reject(409, "review_already_queued");
    }
    let checklist = domain::checklist(&state.review, change_id, facts)?;
    if checklist.items.iter().any(|item| {
        !item.satisfied && !matches!(item.key.as_str(), "ci" | "source_target" | "dependencies")
    }) {
        return reject(409, "review_queue_requirements_unmet");
    }
    let revision = change.current()?;
    state.queue.push(Entry { id: hash(&json!(["queue", event, change_id, &revision.id]))?, change:change_id.into(),
        revision:revision.id.clone(), actor:actor.into(), target_ref:change.target_ref.clone(),
        source_commit:revision.commit.clone(),target_commit:expected_target.into(),candidate_commit:None,ci_run:None,ci_runs:None,
        status:"queued".into(),detail:"Authorized to construct and test exact integration candidates, then submit after fresh admission".into(),
        created_ms:now_ms,generation:0,policy_id:policy.id.clone(),chain:None,superseded_chains:Vec::new() });
    Ok(())
}
pub(super) fn dequeue(state: &mut State, change: &str, actor: &str) -> Result<(), Failure> {
    let owner = &state
        .review
        .changes
        .get(change)
        .ok_or(Failure::Unavailable)?
        .owner;
    let entry = state
        .queue
        .iter_mut()
        .rev()
        .find(|entry| entry.change == change && active(entry))
        .ok_or_else(|| Failure::Api(409, "review_not_queued".into()))?;
    if actor != entry.actor && actor != owner {
        return reject(403, "review_queue_owner_required");
    }
    if matches!(
        entry.status.as_str(),
        "submitting" | "uncertain" | "preparing"
    ) {
        return reject(409, "review_queue_operation_reserved");
    }
    entry.status = "cancelled".into();
    entry.detail = "Cancelled by authenticated review participant".into();
    Ok(())
}

pub(super) fn integration(
    profile: &super::Profile,
    provider: &impl ChangeReviewPort,
    state: &State,
    change: &domain::Change,
    target: &str,
) -> Result<Option<domain::IntegrationFact>, Failure> {
    let revision = change.current()?;
    let Some(entry) = state.queue.iter().rev().find(|entry| {
        entry.change == change.id
            && entry.revision == revision.id
            && entry.target_commit == target
            && entry.candidate_commit.is_some()
            && matches!(
                entry.status.as_str(),
                "waiting_ci" | "submitting" | "uncertain"
            )
    }) else {
        return Ok(None);
    };
    let candidate = entry
        .candidate_commit
        .as_ref()
        .ok_or(Failure::Unavailable)?;
    let observed = provider.inspect(candidate, &change.target_ref)?;
    if observed.target_commit != target {
        return reject(409, "review_target_moved");
    }
    let repo = git2::Repository::open(&profile.provider.repository_path)
        .map_err(|_| Failure::Unavailable)?;
    let source = git2::Oid::from_str(&revision.commit).map_err(|_| Failure::Unavailable)?;
    let candidate_oid = git2::Oid::from_str(candidate).map_err(|_| Failure::Unavailable)?;
    let source_is_ancestor = source == candidate_oid
        || repo
            .graph_descendant_of(candidate_oid, source)
            .map_err(|_| Failure::Unavailable)?;
    Ok(Some(domain::IntegrationFact {
        source_commit: revision.commit.clone(),
        target_commit: target.into(),
        candidate_commit: candidate.clone(),
        target_is_ancestor: observed.target_is_ancestor,
        source_is_ancestor,
    }))
}

impl Workspace {
    /// Engine is locked by the caller before the review lock, matching request order.
    /// All candidate source execution is admitted internally from durable provider facts.
    pub fn advance_queue(
        &self,
        engine: &mut crate::engine::Engine,
        now_ms: i64,
    ) -> Result<(), String> {
        self.advance_queue_inner(engine, now_ms)
            .map_err(|failure| format!("review queue: {failure:?}"))
    }
    fn advance_queue_inner(
        &self,
        engine: &mut crate::engine::Engine,
        now_ms: i64,
    ) -> Result<(), Failure> {
        let Some(profile) = &self.profile else {
            return Ok(());
        };
        if profile.queue.is_none() {
            return Ok(());
        }
        let _gate = self.gate.lock().map_err(|_| Failure::Unavailable)?;
        let binding = profile.binding()?;
        let mut store = store::Store::open(&profile.database, &binding)?;
        let mut state: State = store.load()?;
        let provider = LocalGitReviewProvider::open(profile.provider.clone())?;
        let context = ProviderContext {
            profile,
            provider: &provider,
            binding: &binding,
        };
        self.recover(&context, &mut store, &mut state)?;
        if let Some(policy) = profile.queue.as_ref().and_then(Profile::window_policy) {
            let mut targets: Vec<String> = Vec::new();
            for entry in state.queue.iter().filter(|entry| active(entry)) {
                if !targets.contains(&entry.target_ref) {
                    targets.push(entry.target_ref.clone());
                }
            }
            for target in targets.into_iter().take(MAX_TICK_STEPS) {
                self.advance_speculative(
                    &context, &mut store, &mut state, &target, policy, engine, now_ms,
                )?;
            }
            return Ok(());
        }
        let mut targets = BTreeSet::new();
        let heads: Vec<_> = state
            .queue
            .iter()
            .enumerate()
            .filter(|(_, entry)| active(entry))
            .filter(|(_, entry)| targets.insert(entry.target_ref.clone()))
            .map(|(index, _)| index)
            .take(MAX_TICK_STEPS)
            .collect();
        for index in heads {
            if let Err(failure) =
                self.advance_entry(&context, &mut store, &mut state, index, engine, now_ms)
            {
                record_failure(&mut state, index, failure)?;
                store.commit(&binding, &state, None)?;
            }
        }
        Ok(())
    }
    fn advance_entry(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        index: usize,
        engine: &mut crate::engine::Engine,
        now_ms: i64,
    ) -> Result<(), Failure> {
        let profile = context.profile;
        let entry = state.queue.get(index).ok_or(Failure::Unavailable)?.clone();
        let change = state
            .review
            .changes
            .get(&entry.change)
            .ok_or(Failure::Unavailable)?;
        if change.state == domain::ChangeStatus::Submitted {
            let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
            next.status = "submitted".into();
            next.detail = "Exact candidate submission reconciled".into();
            return store
                .commit(context.binding, state, None)
                .map_err(Failure::from);
        }
        if change.current()?.id != entry.revision
            || change.current()?.commit != entry.source_commit
            || profile.effective_policy()?.id != entry.policy_id
            || !profile.write_principals.contains(&entry.actor)
            || !profile.policy.submitters.contains(&entry.actor)
        {
            return reject(409, "review_queue_authority_changed");
        }
        if state.operations.iter().any(|op| {
            op.change == entry.change && matches!(op.state.as_str(), "pending" | "uncertain")
        }) {
            let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
            next.status = "uncertain".into();
            next.detail = "Reconciling exact reserved provider operation; no redispatch".into();
            return store
                .commit(context.binding, state, None)
                .map_err(Failure::from);
        }
        let observed = context
            .provider
            .inspect(&entry.source_commit, &entry.target_ref)?;
        if observed.target_commit != entry.target_commit {
            let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
            next.target_commit = observed.target_commit;
            next.candidate_commit = None;
            next.ci_run = None;
            next.ci_runs = None;
            next.status = "queued".into();
            next.detail = "Target changed: prior candidate and CI invalidated".into();
            store.commit(context.binding, state, None)?;
            return Ok(());
        }
        if entry.candidate_commit.is_none() {
            if entry.generation
                >= profile
                    .queue
                    .as_ref()
                    .ok_or(Failure::Unavailable)?
                    .max_candidate_builds
            {
                return reject(409, "review_queue_candidate_bound");
            }
            if matches!(entry.status.as_str(), "preparing" | "uncertain") {
                return reject(409, "review_queue_candidate_not_applied");
            }
            let ci = engine.code_review_ci();
            let event = hash(&json!(["queue-build", &entry.id, entry.generation]))?;
            let facts = self.facts(context, state, change, now_ms, &ci, &event)?;
            let checks = domain::checklist(&state.review, &entry.change, &facts)?;
            if checks.items.iter().any(|item| {
                !item.satisfied
                    && !matches!(item.key.as_str(), "ci" | "source_target" | "dependencies")
            }) {
                return reject(409, "review_queue_requirements_unmet");
            }
            // Dependencies are tested only after they have landed, never as unchecked batches.
            if checks
                .items
                .iter()
                .any(|item| item.key == "dependencies" && !item.satisfied)
            {
                return Ok(());
            }
            let operation_id = hash(&json!(["queue-integration", &entry.id, entry.generation]))?;
            let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
            next.generation = entry
                .generation
                .checked_add(1)
                .ok_or(Failure::Unavailable)?;
            next.status = "preparing".into();
            let pending = Pending {
                request: self.operation(
                    profile,
                    operation_id.clone(),
                    ReviewOperation::BuildIntegration {
                        commit: entry.source_commit.clone(),
                        expected_target: entry.target_commit.clone(),
                        strategy: IntegrationStrategy::Merge,
                    },
                    entry.target_ref.clone(),
                    now_ms,
                ),
                actor: entry.actor.clone(),
                facts,
                completion: Completion::Integration {
                    queue_id: entry.id.clone(),
                    chain: None,
                },
                origin_change: entry.change.clone(),
                contribution: None,
            };
            let mut journal = store::Journal {
                principal: entry.actor.clone(),
                request_id: format!("queue-build-{operation_id}"),
                digest: hash(&pending.request)?,
                pending: None,
                response: None,
            };
            let _ = self.reserve_apply(context, store, state, &mut journal, pending)?;
            return Ok(());
        }
        let candidate = entry
            .candidate_commit
            .as_ref()
            .ok_or(Failure::Unavailable)?;
        let started = match (&entry.ci_runs, &entry.ci_run) {
            (Some(runs), _) => Some(runs.clone()),
            (None, Some(run)) => Some(vec![run.clone()]),
            (None, None) => None,
        };
        let Some(runs) = started else {
            let (runs, monorepo) =
                self.start_candidate_runs(profile, engine, state, &entry, candidate, |project| {
                    Ok(if project == profile.project {
                        format!("review-queue-{}-{}", entry.id, entry.generation)
                    } else {
                        format!(
                            "review-queue-{}",
                            hash(&json!(["queue-ci", &entry.id, entry.generation, project]))?
                        )
                    })
                })?;
            let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
            next.ci_run = runs.first().cloned();
            next.ci_runs = monorepo.then_some(runs);
            next.status = "waiting_ci".into();
            next.detail = "Canonical workflow executing exact integration commit".into();
            store.commit(context.binding, state, None)?;
            return Ok(());
        };
        for run_id in &runs {
            let Some((terminal, success)) = engine.review_ci_outcome(run_id) else {
                return reject(503, "review_queue_ci_missing");
            };
            if !terminal {
                return Ok(());
            }
            if !success {
                return reject(409, "review_queue_ci_failed");
            }
        }
        self.submit_candidate(context, store, state, index, &entry, engine, now_ms)
            .map(|_| ())
    }

    /// Canonical CI for one exact candidate commit: once per affected project
    /// at its current scope identity (monorepo), else once for the review project.
    /// Returns the run identities and whether this was a monorepo evaluation.
    fn start_candidate_runs(
        &self,
        profile: &super::Profile,
        engine: &mut crate::engine::Engine,
        state: &State,
        entry: &Entry,
        candidate: &str,
        request_id: impl Fn(&str) -> Result<String, Failure>,
    ) -> Result<(Vec<String>, bool), Failure> {
        let change = state
            .review
            .changes
            .get(&entry.change)
            .ok_or(Failure::Unavailable)?;
        // r[impl kiln.monorepo.gating]
        // Monorepo reviews test the candidate once per affected project.
        let revision = change.current()?;
        let evaluation = self.review_evaluation(profile, revision)?;
        let targets: Vec<(String, Option<crate::monorepo::RunAffected>)> = match &evaluation {
            None => vec![(profile.project.clone(), None)],
            Some(evaluation) => self
                .ci_requirements(evaluation)?
                .into_iter()
                .map(|requirement| {
                    let affected = evaluation.run_affected(&requirement.project);
                    (requirement.project, affected)
                })
                .collect(),
        };
        let mut runs = Vec::new();
        for (project, affected) in targets {
            let request_id = request_id(&project)?;
            let response =
                engine.start_review_candidate(&project, candidate, &request_id, affected)?;
            let value: Value =
                serde_json::from_slice(&response.body).map_err(|_| Failure::Unavailable)?;
            let Some(id) = value.get("id").and_then(Value::as_str) else {
                return reject(response.status, "review_queue_ci_admission_failed");
            };
            runs.push(id.to_owned());
        }
        Ok((runs, evaluation.is_some()))
    }

    /// Fresh admission, then PrepareSubmit and exact-target CAS Submit through
    /// the ordinary command path. Returns whether the candidate landed.
    #[allow(clippy::too_many_arguments)]
    fn submit_candidate(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        index: usize,
        entry: &Entry,
        engine: &mut crate::engine::Engine,
        now_ms: i64,
    ) -> Result<bool, Failure> {
        let change = state
            .review
            .changes
            .get(&entry.change)
            .ok_or(Failure::Unavailable)?;
        let ci = engine.code_review_ci();
        let facts = self.facts(context, state, change, now_ms, &ci, "queue-admission")?;
        let checklist = domain::checklist(&state.review, &entry.change, &facts)?;
        if !checklist.ready {
            return reject(409, "review_queue_requirements_unmet");
        }
        let principal = RequestContext {
            role: Role::Operator,
            principal: &entry.actor,
            now_ms,
            ci: &ci,
        };
        let prepare_id = format!("queue-prepare-{}-{}", entry.id, entry.generation);
        let request = Request {
            request_id: prepare_id,
            change_id: Some(entry.change.clone()),
            expected_version: state.versions.get(&entry.change).copied(),
            action: Action::PrepareSubmit {},
        };
        let response = self.command(context, store, state, request, &principal)?;
        if response.status != 200 {
            return reject(response.status, "review_queue_prepare_failed");
        }
        let candidate_id = state
            .review
            .changes
            .get(&entry.change)
            .and_then(|change| change.candidate.as_ref())
            .map(|candidate| candidate.id.clone())
            .ok_or(Failure::Unavailable)?;
        let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
        next.status = "submitting".into();
        next.detail = "Exact candidate passed fresh policy and canonical CI admission".into();
        store.commit(context.binding, state, None)?;
        let request = Request {
            request_id: format!("queue-submit-{}-{}", entry.id, entry.generation),
            change_id: Some(entry.change.clone()),
            expected_version: state.versions.get(&entry.change).copied(),
            action: Action::Submit { candidate_id },
        };
        let response = self.command(context, store, state, request, &principal)?;
        if response.status != 200 {
            return Ok(false);
        }
        let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
        next.status = "submitted".into();
        next.detail = "Exact integration candidate submitted and independently read back".into();
        store.commit(context.binding, state, None)?;
        Ok(true)
    }
}

/// Record a queue failure exactly as the serial queue always has: reserved
/// provider operations make the entry uncertain, anything else ejects it.
/// Returns whether the entry was ejected (`blocked`).
fn record_failure(state: &mut State, index: usize, failure: Failure) -> Result<bool, Failure> {
    let entry = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
    let uncertain = state.operations.iter().any(|op| {
        op.change == entry.change && matches!(op.state.as_str(), "pending" | "uncertain")
    });
    entry.status = if uncertain { "uncertain" } else { "blocked" }.into();
    entry.detail = match failure {
        Failure::Api(_, code) => code,
        Failure::Domain(error) => error.message,
        Failure::Unavailable => {
            "Candidate or canonical CI unavailable; fresh authorization is required".into()
        }
    };
    Ok(!uncertain)
}

/// Outcome of advancing one speculative window position.
enum Step {
    /// The entry holds an exact candidate later entries may stack on.
    Ahead(Link),
    /// Stop extending the window this tick (reserved operation, unlanded
    /// dependency, non-conclusive failure or capacity wait).
    Hold,
    /// The entry's candidate landed on the target.
    Landed,
}

const HELD_ON_PREDECESSORS: &str = "Candidate could not be built or tested on its assumed predecessors; conclusive once it reaches the head";

impl Workspace {
    /// One speculative tick for `target_ref`: walk the admitted window in
    /// queue order, stacking candidate k on candidates 0..k-1.
    // r[impl kiln.review.queue.speculative.window]
    // r[impl kiln.review.queue.speculative.failure]
    #[allow(clippy::too_many_arguments)]
    fn advance_speculative(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        target_ref: &str,
        policy: WindowPolicy,
        engine: &mut crate::engine::Engine,
        now_ms: i64,
    ) -> Result<(), Failure> {
        let queued: Vec<usize> = state
            .queue
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.target_ref == target_ref && active(entry))
            .map(|(index, _)| index)
            .collect();
        let Some(&head) = queued.first() else {
            return Ok(());
        };
        let window =
            speculation::current_window(state.queue_windows.get(target_ref).copied(), policy);
        let positions = speculation::admitted(window, queued.len())?;
        let source = state
            .queue
            .get(head)
            .ok_or(Failure::Unavailable)?
            .source_commit
            .clone();
        let observed = context.provider.inspect(&source, target_ref)?.target_commit;
        let mut ahead: Vec<Link> = Vec::new();
        for position in positions {
            let index = *queued.get(position).ok_or(Failure::Unavailable)?;
            match self.speculate_entry(
                context, store, state, index, &observed, &ahead, engine, now_ms,
            ) {
                Ok(Step::Ahead(link)) => ahead.push(link),
                Ok(Step::Hold) => break,
                Ok(Step::Landed) => {
                    self.adapt_window(context, store, state, target_ref, policy, true)?;
                    // The target moved; the next tick observes it afresh.
                    break;
                }
                Err(failure) => {
                    let ejected = record_failure(state, index, failure)?;
                    store.commit(context.binding, state, None)?;
                    if !ejected {
                        break;
                    }
                    // Later entries see a changed chain ahead and rebuild without it.
                    self.adapt_window(context, store, state, target_ref, policy, false)?;
                }
            }
        }
        Ok(())
    }

    fn adapt_window(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        target_ref: &str,
        policy: WindowPolicy,
        passed: bool,
    ) -> Result<(), Failure> {
        let current =
            speculation::current_window(state.queue_windows.get(target_ref).copied(), policy);
        let next = speculation::next_window(current, passed, policy)?;
        state.queue_windows.insert(target_ref.to_owned(), next);
        store.commit(context.binding, state, None)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn speculate_entry(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        index: usize,
        observed: &str,
        ahead: &[Link],
        engine: &mut crate::engine::Engine,
        now_ms: i64,
    ) -> Result<Step, Failure> {
        let profile = context.profile;
        let entry = state.queue.get(index).ok_or(Failure::Unavailable)?.clone();
        let change = state
            .review
            .changes
            .get(&entry.change)
            .ok_or(Failure::Unavailable)?;
        if change.state == domain::ChangeStatus::Submitted {
            let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
            next.status = "submitted".into();
            next.detail = "Exact candidate submission reconciled".into();
            store.commit(context.binding, state, None)?;
            return Ok(Step::Landed);
        }
        if change.current()?.id != entry.revision
            || change.current()?.commit != entry.source_commit
            || profile.effective_policy()?.id != entry.policy_id
            || !profile.write_principals.contains(&entry.actor)
            || !profile.policy.submitters.contains(&entry.actor)
        {
            return reject(409, "review_queue_authority_changed");
        }
        // r[impl kiln.review.queue.speculative.chain]
        // A reserved provider operation is reconciled, never redispatched, and
        // nothing stacks on it until it resolves.
        if state.operations.iter().any(|op| {
            op.change == entry.change && matches!(op.state.as_str(), "pending" | "uncertain")
        }) {
            let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
            next.status = "uncertain".into();
            next.detail = "Reconciling exact reserved provider operation; no redispatch".into();
            store.commit(context.binding, state, None)?;
            return Ok(Step::Hold);
        }
        let head = ahead.is_empty();
        let stale = match &entry.chain {
            Some(chain) => !speculation::chain_valid(chain, observed, ahead),
            // A candidate without a chain predates speculative mode.
            None => entry.candidate_commit.is_some() || entry.ci_run.is_some(),
        };
        if stale {
            self.supersede(engine, state, index)?;
            store.commit(context.binding, state, None)?;
        }
        let entry = state.queue.get(index).ok_or(Failure::Unavailable)?.clone();
        let Some(candidate) = entry.candidate_commit.clone() else {
            if entry.chain.is_some() {
                // The build on this still-valid chain was not applied.
                if head {
                    return reject(409, "review_queue_candidate_not_applied");
                }
                self.hold(context, store, state, index)?;
                return Ok(Step::Hold);
            }
            if !self.build_speculative(
                context, store, state, index, observed, ahead, engine, now_ms,
            )? {
                return Ok(Step::Hold);
            }
            return self.speculate_entry(
                context, store, state, index, observed, ahead, engine, now_ms,
            );
        };
        let chain = entry.chain.clone().ok_or(Failure::Unavailable)?;
        let link = Link {
            queue_id: entry.id.clone(),
            generation: entry.generation,
            candidate_commit: candidate.clone(),
        };
        let started = match (&entry.ci_runs, &entry.ci_run) {
            (Some(runs), _) => Some(runs.clone()),
            (None, Some(run)) => Some(vec![run.clone()]),
            (None, None) => None,
        };
        let Some(runs) = started else {
            // Deterministic per entry, generation, chain and project, so a
            // restart replays the engine admission instead of dispatching again.
            let started =
                self.start_candidate_runs(profile, engine, state, &entry, &candidate, |project| {
                    Ok(format!(
                        "review-queue-{}",
                        hash(&json!([
                            "queue-ci-speculative",
                            &entry.id,
                            entry.generation,
                            &chain.id,
                            project
                        ]))?
                    ))
                });
            let (runs, monorepo) = match started {
                Ok(started) => started,
                Err(failure) if head => return Err(failure),
                Err(_) => {
                    self.hold(context, store, state, index)?;
                    return Ok(Step::Hold);
                }
            };
            let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
            next.ci_run = runs.first().cloned();
            next.ci_runs = monorepo.then_some(runs);
            next.status = "waiting_ci".into();
            next.detail =
                "Canonical workflow executing exact speculative candidate on its assumed predecessors"
                    .into();
            store.commit(context.binding, state, None)?;
            return Ok(Step::Ahead(link));
        };
        let outcomes: Vec<_> = runs
            .iter()
            .map(|run| engine.review_ci_outcome(run))
            .collect();
        match speculation::ci_verdict(&outcomes) {
            Verdict::Pending => Ok(Step::Ahead(link)),
            Verdict::Passed if head => {
                // Every predecessor landed: fresh admission and exact-target CAS.
                let landed =
                    self.submit_candidate(context, store, state, index, &entry, engine, now_ms)?;
                Ok(if landed { Step::Landed } else { Step::Hold })
            }
            Verdict::Passed => Ok(Step::Ahead(link)),
            Verdict::Missing if head => reject(503, "review_queue_ci_missing"),
            Verdict::Failed if head => reject(409, "review_queue_ci_failed"),
            Verdict::Missing | Verdict::Failed => {
                self.hold(context, store, state, index)?;
                Ok(Step::Hold)
            }
        }
    }

    /// Build the exact candidate on the observed target and the candidates
    /// ahead. Returns false when the entry must wait (unlanded dependency).
    #[allow(clippy::too_many_arguments)]
    fn build_speculative(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        index: usize,
        observed: &str,
        ahead: &[Link],
        engine: &mut crate::engine::Engine,
        now_ms: i64,
    ) -> Result<bool, Failure> {
        let profile = context.profile;
        let entry = state.queue.get(index).ok_or(Failure::Unavailable)?.clone();
        let config = profile.queue.as_ref().ok_or(Failure::Unavailable)?;
        if entry.generation >= config.max_candidate_builds {
            return reject(409, "review_queue_candidate_bound");
        }
        if matches!(entry.status.as_str(), "preparing" | "uncertain") {
            return reject(409, "review_queue_candidate_not_applied");
        }
        let change = state
            .review
            .changes
            .get(&entry.change)
            .ok_or(Failure::Unavailable)?;
        let ci = engine.code_review_ci();
        let event = hash(&json!(["queue-build", &entry.id, entry.generation]))?;
        let facts = self.facts(context, state, change, now_ms, &ci, &event)?;
        let checks = domain::checklist(&state.review, &entry.change, &facts)?;
        // r[impl kiln.review.queue.speculative.serial]
        // Same authority gate as serial: approvals, owners and policy are the
        // source's own, never transferred, and advisory approvals never count.
        if checks.items.iter().any(|item| {
            !item.satisfied && !matches!(item.key.as_str(), "ci" | "source_target" | "dependencies")
        }) {
            return reject(409, "review_queue_requirements_unmet");
        }
        // Dependencies are tested only after they have landed, never as unchecked batches.
        if checks
            .items
            .iter()
            .any(|item| item.key == "dependencies" && !item.satisfied)
        {
            return Ok(false);
        }
        let generation = entry
            .generation
            .checked_add(1)
            .ok_or(Failure::Unavailable)?;
        let chain = Chain::new(&entry.id, generation, observed, ahead.to_vec())?;
        let operation_id = hash(&json!([
            "queue-speculative-integration",
            &entry.id,
            generation,
            &chain.id
        ]))?;
        let operation = match ahead.last() {
            None => ReviewOperation::BuildIntegration {
                commit: entry.source_commit.clone(),
                expected_target: observed.to_owned(),
                strategy: IntegrationStrategy::Merge,
            },
            Some(predecessor) => ReviewOperation::BuildSpeculativeIntegration {
                commit: entry.source_commit.clone(),
                expected_target: observed.to_owned(),
                predecessor: predecessor.candidate_commit.clone(),
                strategy: IntegrationStrategy::Merge,
            },
        };
        let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
        next.generation = generation;
        next.status = "preparing".into();
        next.detail = "Constructing exact speculative candidate on its assumed predecessors".into();
        // Submission fast-forwards the last predecessor once it has landed.
        next.target_commit = chain.base().to_owned();
        let completion = Completion::Integration {
            queue_id: entry.id.clone(),
            chain: Some(chain.id.clone()),
        };
        next.chain = Some(chain);
        let pending = Pending {
            request: self.operation(
                profile,
                operation_id.clone(),
                operation,
                entry.target_ref.clone(),
                now_ms,
            ),
            actor: entry.actor.clone(),
            facts,
            completion,
            origin_change: entry.change.clone(),
            contribution: None,
        };
        let mut journal = store::Journal {
            principal: entry.actor.clone(),
            request_id: format!("queue-build-{operation_id}"),
            digest: hash(&pending.request)?,
            pending: None,
            response: None,
        };
        let _ = self.reserve_apply(context, store, state, &mut journal, pending)?;
        Ok(state
            .queue
            .get(index)
            .is_some_and(|entry| entry.candidate_commit.is_some()))
    }

    /// Retire the entry's chain: cancel its undispatched CI runs, ignore any
    /// dispatched ones, and clear the candidate so it is rebuilt.
    // r[impl kiln.review.queue.speculative.failure]
    fn supersede(
        &self,
        engine: &mut crate::engine::Engine,
        state: &mut State,
        index: usize,
    ) -> Result<(), Failure> {
        let entry = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
        let runs = match (&entry.ci_runs, &entry.ci_run) {
            (Some(runs), _) => runs.clone(),
            (None, run) => run.iter().cloned().collect(),
        };
        for run in &runs {
            engine.cancel_review_candidate(run)?;
        }
        if let Some(chain) = entry.chain.take() {
            speculation::retire(&mut entry.superseded_chains, &chain.id);
        }
        entry.candidate_commit = None;
        entry.ci_run = None;
        entry.ci_runs = None;
        entry.status = "queued".into();
        entry.detail =
            "Assumed predecessor chain superseded: candidate and CI results retired; rebuilding"
                .into();
        Ok(())
    }

    fn hold(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        index: usize,
    ) -> Result<(), Failure> {
        let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
        if next.detail == HELD_ON_PREDECESSORS
            && matches!(next.status.as_str(), "queued" | "waiting_ci")
        {
            return Ok(());
        }
        if next.candidate_commit.is_none() {
            next.status = "queued".into();
        }
        next.detail = HELD_ON_PREDECESSORS.into();
        store.commit(context.binding, state, None)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replayed<T: Serialize + for<'de> Deserialize<'de>>(bytes: &str) -> String {
        let value: T = serde_json::from_str(bytes)
            .unwrap_or_else(|error| unreachable!("legacy record rejected: {error}"));
        serde_json::to_string(&value).unwrap_or_else(|error| unreachable!("{error}"))
    }

    // r[verify kiln.review.queue.speculative.serial]
    // r[verify kiln.review.queue.speculative.serial.scenario.replay]
    #[test]
    fn serial_profiles_entries_and_journals_replay_byte_identically() {
        let profile =
            r#"{"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16}"#;
        assert_eq!(replayed::<Profile>(profile), profile);
        let parsed: Profile =
            serde_json::from_str(profile).unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(parsed.mode, QueueMode::Serial);
        assert!(parsed.window_policy().is_none());
        for entry in [
            r#"{"id":"q","change":"c","revision":"r","actor":"a","target_ref":"refs/heads/main","source_commit":"s","target_commit":"t","candidate_commit":null,"ci_run":null,"status":"queued","detail":"d","created_ms":1,"generation":0,"policy_id":"p"}"#,
            r#"{"id":"q","change":"c","revision":"r","actor":"a","target_ref":"refs/heads/main","source_commit":"s","target_commit":"t","candidate_commit":"k","ci_run":"run","ci_runs":["run","docs"],"status":"waiting_ci","detail":"d","created_ms":1,"generation":1,"policy_id":"p"}"#,
        ] {
            assert_eq!(replayed::<Entry>(entry), entry);
        }
        let completion = r#"{"type":"integration","queue_id":"q"}"#;
        assert_eq!(replayed::<Completion>(completion), completion);
        let state =
            serde_json::to_value(State::default()).unwrap_or_else(|error| unreachable!("{error}"));
        assert!(state.get("queue_windows").is_none());
    }

    #[test]
    fn speculative_records_round_trip_and_reject_unknown_window_fields() {
        let profile = r#"{"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16,"mode":"speculative","window":{"floor":1,"ceiling":3,"increase":1,"decrease":1}}"#;
        assert_eq!(replayed::<Profile>(profile), profile);
        let parsed: Profile =
            serde_json::from_str(profile).unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(
            parsed.window_policy(),
            Some(WindowPolicy {
                floor: 1,
                ceiling: 3,
                increase: 1,
                decrease: 1
            })
        );
        for invalid in [
            r#"{"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16,"mode":"batch"}"#,
            r#"{"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16,"mode":"speculative","window":{"floor":1,"ceiling":3,"increase":1,"decrease":1,"batch":true}}"#,
        ] {
            assert!(
                serde_json::from_str::<Profile>(invalid).is_err(),
                "{invalid}"
            );
        }
        let completion = r#"{"type":"integration","queue_id":"q","chain":"c"}"#;
        assert_eq!(replayed::<Completion>(completion), completion);
    }
}
