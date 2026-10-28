//! FIFO integration queue over exact reviewed sources and canonical candidate CI.
//! r[impl kiln.review.parity.queue]
use super::commands::{Completion, Pending};
use super::*;

const MAX_QUEUE_ENTRIES: usize = 1000;
const MAX_CANDIDATE_BUILDS: u32 = 128;
const MAX_TICK_STEPS: usize = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub max_entries: usize,
    pub max_candidate_builds: u32,
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
        Ok(())
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
    pub status: String,
    pub detail: String,
    pub created_ms: i64,
    pub generation: u32,
    pub policy_id: String,
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
        source_commit:revision.commit.clone(),target_commit:expected_target.into(),candidate_commit:None,ci_run:None,
        status:"queued".into(),detail:"Authorized to construct and test exact integration candidates, then submit after fresh admission".into(),
        created_ms:now_ms,generation:0,policy_id:policy.id.clone() });
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
                let entry = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
                let uncertain = state.operations.iter().any(|op| {
                    op.change == entry.change
                        && matches!(op.state.as_str(), "pending" | "uncertain")
                });
                entry.status = if uncertain { "uncertain" } else { "blocked" }.into();
                entry.detail = match failure {
                    Failure::Api(_, code) => code,
                    Failure::Domain(error) => error.message,
                    Failure::Unavailable => {
                        "Candidate or canonical CI unavailable; fresh authorization is required"
                            .into()
                    }
                };
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
        if entry.ci_run.is_none() {
            let request_id = format!("review-queue-{}-{}", entry.id, entry.generation);
            let response =
                engine.start_review_candidate(&profile.project, candidate, &request_id)?;
            let value: Value =
                serde_json::from_slice(&response.body).map_err(|_| Failure::Unavailable)?;
            if let Some(id) = value.get("id").and_then(Value::as_str) {
                let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
                next.ci_run = Some(id.into());
                next.status = "waiting_ci".into();
                next.detail = "Canonical workflow executing exact integration commit".into();
                store.commit(context.binding, state, None)?;
                return Ok(());
            }
            return reject(response.status, "review_queue_ci_admission_failed");
        }
        let run_id = entry.ci_run.as_deref().ok_or(Failure::Unavailable)?;
        let Some((terminal, success)) = engine.review_ci_outcome(run_id) else {
            return reject(503, "review_queue_ci_missing");
        };
        if !terminal {
            return Ok(());
        }
        if !success {
            return reject(409, "review_queue_ci_failed");
        }
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
        if response.status == 200 {
            let next = state.queue.get_mut(index).ok_or(Failure::Unavailable)?;
            next.status = "submitted".into();
            next.detail =
                "Exact integration candidate submitted and independently read back".into();
            store.commit(context.binding, state, None)?;
        }
        Ok(())
    }
}
