use super::store::Journal;
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Pending {
    pub(super) request: ReviewOperationRequest,
    pub(super) actor: String,
    pub(super) facts: domain::Facts,
    pub(super) completion: Completion,
    pub(super) origin_change: String,
    #[serde(default)]
    pub(super) contribution: Option<(String, intake::Contribution)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Completion {
    Register {
        command: domain::Command,
        change: String,
    },
    Submit {
        change: String,
        revision: String,
        candidate: String,
        target: String,
    },
    Maintenance {
        change: String,
        source_revision: String,
        result_change: String,
        result_revision: String,
        title: String,
        target_ref: String,
        onto: String,
        kind: domain::MaintenanceKind,
    },
    Suggestions {
        change: String,
        source_revision: String,
        result_revision: String,
        base: String,
    },
    Integration {
        queue_id: String,
    },
}

impl Workspace {
    pub(super) fn command(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        mut request: Request,
        request_context: &RequestContext<'_>,
    ) -> Result<ApiResponse, Failure> {
        let ProviderContext {
            profile,
            provider,
            binding,
        } = *context;
        let RequestContext {
            principal: actor,
            now_ms,
            ci,
            ..
        } = *request_context;
        if !valid_text(&request.request_id, MAX_SHORT_TEXT_BYTES) {
            return reject(400, "review_request_id_invalid");
        }
        validate_action(&request.action)?;
        let digest = hash(&request)?;
        if let Some(journal) = store.request(actor, &request.request_id)? {
            if journal.digest != digest {
                return reject(409, "review_request_conflict");
            }
            return replay(&journal, true);
        }
        let private = domain::project(&state.review, actor);
        match &request.action {
            Action::SaveView { id, .. }
                if private.saved_queries.len() >= MAX_SAVED_VIEWS
                    && !private
                        .saved_queries
                        .iter()
                        .any(|saved| Some(&saved.id) == id.as_ref()) =>
            {
                return reject(409, "review_saved_view_bound");
            }
            Action::SaveDraft { id, .. }
                if private.drafts.len() >= MAX_DRAFTS
                    && !private
                        .drafts
                        .iter()
                        .any(|draft| Some(&draft.id) == id.as_ref()) =>
            {
                return reject(409, "review_draft_bound");
            }
            _ => {}
        }
        let event = hash(&json!(["event", actor, &request.request_id, &digest]))?;
        let identify = |kind: &str| hash(&json!([kind, actor, &request.request_id, &digest]));
        let mut journal = Journal {
            principal: actor.into(),
            request_id: request.request_id.clone(),
            digest: digest.clone(),
            pending: None,
            response: None,
        };
        let mut facts = domain::Facts {
            now_ms,
            event_id: event.clone(),
            policy: Some(profile.effective_policy()?),
            source: None,
            ci: Vec::new(),
            anchors: Vec::new(),
            integration: None,
        };
        let mut contribution = None;
        if let Action::Contribute {
            source_ref,
            expected_commit,
            title,
            description,
            wip,
        } = &request.action
        {
            if request.change_id.is_some() || request.expected_version.is_some() {
                return reject(400, "review_contribution_global_required");
            }
            let plan = intake::plan(
                profile,
                state,
                actor,
                source_ref,
                expected_commit.as_deref(),
            )?;
            let source_ref = source_ref.clone();
            let title = title.clone();
            let description = description.clone();
            let wip = *wip;
            match plan {
                intake::Plan::Create {
                    contribution: item,
                    commit,
                } => {
                    request.action = Action::Create {
                        commit,
                        target_ref: item.target_ref.clone(),
                        title,
                        description,
                        wip,
                    };
                    contribution = Some((source_ref, item));
                }
                intake::Plan::Upload {
                    contribution: item,
                    commit,
                    expected_version,
                } => {
                    request.change_id = Some(item.change_id.clone());
                    request.expected_version = Some(expected_version);
                    request.action = Action::Upload { commit };
                    contribution = Some((source_ref, item));
                }
                intake::Plan::Replay { change_id } => {
                    let result = command_result(state, Some(&change_id), true, None);
                    journal.response = Some(json!({"status":200,"body":result}));
                    store.commit(binding, state, Some(&journal))?;
                    return replay(&journal, true);
                }
            }
        }
        let global = matches!(
            &request.action,
            Action::Create { .. }
                | Action::SaveView { .. }
                | Action::DeleteView { .. }
                | Action::ReadNotification { .. }
                | Action::DeleteDraft { .. }
        );
        let current = match (&request.change_id, request.expected_version) {
            (Some(id), Some(version)) => {
                let change = state
                    .review
                    .changes
                    .get(id)
                    .ok_or_else(|| Failure::Api(404, "review_change_missing".into()))?;
                if state.operations.iter().any(|op| {
                    matches!(op.state.as_str(), "pending" | "uncertain")
                        && (op.change == *id || op.result_change.as_ref() == Some(id))
                }) {
                    return reject(409, "review_change_reserved");
                }
                if state.versions.get(id).copied().unwrap_or(0) != version {
                    return reject(409, "review_version_conflict");
                }
                Some(change)
            }
            (None, None) if global => None,
            _ => return reject(400, "review_expected_state_required"),
        };
        if matches!(&request.action, Action::Create { .. }) && current.is_some() {
            return reject(400, "review_create_state_invalid");
        }
        if !global && current.is_none() {
            return reject(400, "review_change_required");
        }
        if let Some(change) = &current {
            facts = self.facts(context, state, change, now_ms, ci, &event)?;
        }
        let change_id = current.as_ref().map(|c| c.id.clone()).unwrap_or_default();
        let revision_id = match &current {
            Some(change) => change.current()?.id.clone(),
            None => String::new(),
        };
        let mut effect: Option<(ReviewOperation, Completion, String, String)> = None;
        let mut result_change = current.as_ref().map(|c| c.id.clone());
        let command = match request.action {
            Action::Contribute { .. } => return Err(Failure::Unavailable),
            Action::Subscribe { subscribed } => {
                let mut next = state.clone();
                delivery::subscribe(
                    &mut next.delivery,
                    &next.review,
                    actor,
                    &change_id,
                    subscribed,
                    &profile.read_principals,
                )?;
                journal.response = Some(
                    json!({"status":200,"body":command_result(&next, Some(&change_id), false, None)}),
                );
                store.commit(binding, &next, Some(&journal))?;
                *state = next;
                return replay(&journal, false);
            }
            Action::Enqueue { expected_target } => {
                let mut next = state.clone();
                queue::enqueue(
                    profile,
                    &mut next,
                    &change_id,
                    actor,
                    &expected_target,
                    &facts,
                )?;
                journal.response = Some(
                    json!({"status":200,"body":command_result(&next, Some(&change_id), false, None)}),
                );
                store.commit(binding, &next, Some(&journal))?;
                *state = next;
                return replay(&journal, false);
            }
            Action::Dequeue {} => {
                let mut next = state.clone();
                queue::dequeue(&mut next, &change_id, actor)?;
                journal.response = Some(
                    json!({"status":200,"body":command_result(&next, Some(&change_id), false, None)}),
                );
                store.commit(binding, &next, Some(&journal))?;
                *state = next;
                return replay(&journal, false);
            }
            Action::ApplySuggestions { thread_ids } => {
                let change = current.as_ref().ok_or(Failure::Unavailable)?;
                if change.owner != actor || change.state != domain::ChangeStatus::Open {
                    return reject(403, "review_owner_required");
                }
                if change.revisions.len() >= MAX_REVISIONS {
                    return reject(409, "review_revision_bound");
                }
                let edits = suggestions::edits(profile, change, &thread_ids)?;
                self.require_capability(provider, ReviewCapability::ApplySuggestions)?;
                let pending = Pending {
                    request: self.operation(
                        profile,
                        identify("operation")?,
                        ReviewOperation::ApplySuggestions {
                            commit: change.current()?.commit.clone(),
                            suggestions: edits,
                        },
                        change.target_ref.clone(),
                        now_ms,
                    ),
                    actor: actor.into(),
                    facts,
                    completion: Completion::Suggestions {
                        change: change_id.clone(),
                        source_revision: revision_id,
                        result_revision: identify("revision")?,
                        base: change.current()?.base.clone(),
                    },
                    origin_change: change_id,
                    contribution: None,
                };
                return self.reserve_apply(context, store, state, &mut journal, pending);
            }
            Action::Create {
                commit,
                target_ref,
                title,
                description,
                wip,
            } => {
                if state.review.changes.len() >= MAX_CHANGES {
                    return reject(409, "review_change_bound");
                }
                self.require_target(profile, &target_ref)?;
                let id = match &contribution {
                    Some((_, item)) => item.change_id.clone(),
                    None => identify("change")?,
                };
                let inspection = provider.inspect(&commit, &target_ref)?;
                let revision = self.revision(
                    provider,
                    &commit,
                    &inspection.target_commit,
                    identify("revision")?,
                    1,
                    (actor, now_ms),
                )?;
                facts.source = Some(source_fact(profile, &inspection, revision.base.clone()));
                self.populate_changed_files(provider, &revision, &mut facts)?;
                let command = domain::Command::Create {
                    id: id.clone(),
                    project: profile.project.clone(),
                    repository: profile.provider.repository_id.clone(),
                    target_ref: target_ref.clone(),
                    title,
                    description,
                    wip,
                    revision,
                };
                effect = Some((
                    ReviewOperation::Register { commit },
                    Completion::Register {
                        command: command.clone(),
                        change: id.clone(),
                    },
                    target_ref,
                    id.clone(),
                ));
                result_change = Some(id);
                command
            }
            Action::Upload { commit } => {
                let change = current.as_ref().ok_or(Failure::Unavailable)?;
                if change.revisions.len() >= MAX_REVISIONS {
                    return reject(409, "review_revision_bound");
                }
                let inspection = provider.inspect(&commit, &change.target_ref)?;
                let revision = self.revision(
                    provider,
                    &commit,
                    &inspection.target_commit,
                    identify("revision")?,
                    change.revisions.len() as u32 + 1,
                    (actor, now_ms),
                )?;
                facts.source = Some(source_fact(profile, &inspection, revision.base.clone()));
                self.populate_changed_files(provider, &revision, &mut facts)?;
                let command = domain::Command::Upload {
                    change: change_id.clone(),
                    expected_revision: revision_id.clone(),
                    revision,
                };
                effect = Some((
                    ReviewOperation::Register { commit },
                    Completion::Register {
                        command: command.clone(),
                        change: change_id.clone(),
                    },
                    change.target_ref.clone(),
                    change_id.clone(),
                ));
                command
            }
            Action::Edit { title, description } => domain::Command::Edit {
                change: change_id.clone(),
                expected_revision: revision_id,
                title,
                description,
            },
            Action::SetWip { wip } => domain::Command::SetWip {
                change: change_id.clone(),
                expected_revision: revision_id,
                wip,
            },
            Action::Abandon {} => domain::Command::Abandon {
                change: change_id.clone(),
                expected_revision: revision_id,
            },
            Action::Restore {} => domain::Command::Restore {
                change: change_id.clone(),
                expected_revision: revision_id,
            },
            Action::SetReviewer { principal, role } => {
                if !profile.read_principals.contains(&principal) {
                    return reject(403, "review_participant_denied");
                }
                domain::Command::SetReviewer {
                    change: change_id.clone(),
                    expected_revision: revision_id,
                    principal,
                    role,
                }
            }
            Action::Vote { value } => domain::Command::Vote {
                change: change_id.clone(),
                expected_revision: revision_id,
                value,
            },
            Action::StartThread {
                anchor,
                body,
                blocking,
            } => {
                self.admit_anchor(
                    provider,
                    current.as_ref().ok_or(Failure::Unavailable)?,
                    &anchor,
                    &mut facts,
                )?;
                domain::Command::StartThread {
                    change: change_id.clone(),
                    expected_revision: revision_id,
                    id: identify("thread")?,
                    comment_id: identify("comment")?,
                    anchor,
                    body,
                    blocking,
                }
            }
            Action::Reply { thread, body } => domain::Command::Reply {
                change: change_id.clone(),
                expected_revision: revision_id,
                thread,
                comment_id: identify("comment")?,
                body,
            },
            Action::Resolve { thread, resolved } => domain::Command::Resolve {
                change: change_id.clone(),
                expected_revision: revision_id,
                thread,
                resolved,
            },
            Action::SaveDraft {
                id,
                anchor,
                thread,
                body,
                blocking,
            } => {
                self.admit_anchor(
                    provider,
                    current.as_ref().ok_or(Failure::Unavailable)?,
                    &anchor,
                    &mut facts,
                )?;
                domain::Command::SaveDraft {
                    change: change_id.clone(),
                    expected_revision: revision_id,
                    draft: domain::Draft {
                        id: match id {
                            Some(id) => id,
                            None => identify("draft")?,
                        },
                        change: change_id.clone(),
                        author: actor.into(),
                        anchor,
                        thread,
                        body,
                        blocking,
                    },
                }
            }
            Action::DeleteDraft { id } => domain::Command::DeleteDraft { id },
            Action::Publish { draft_ids, vote } => {
                for draft in domain::project(&state.review, actor)
                    .drafts
                    .iter()
                    .filter(|d| draft_ids.contains(&d.id))
                {
                    self.admit_anchor(
                        provider,
                        current.as_ref().ok_or(Failure::Unavailable)?,
                        &draft.anchor,
                        &mut facts,
                    )?;
                }
                domain::Command::Publish {
                    change: change_id.clone(),
                    expected_revision: revision_id,
                    draft_ids,
                    vote,
                }
            }
            Action::MarkReviewed { file, reviewed } => domain::Command::MarkReviewed {
                change: change_id.clone(),
                expected_revision: revision_id,
                file,
                reviewed,
            },
            Action::SetAttention {
                principal,
                attention,
            } => domain::Command::SetAttention {
                change: change_id.clone(),
                expected_revision: revision_id,
                principal,
                attention,
            },
            Action::SetDependencies { dependencies } => domain::Command::SetDependencies {
                change: change_id.clone(),
                expected_revision: revision_id,
                dependencies,
            },
            Action::SaveView { id, name, query } => domain::Command::SaveQuery {
                query: domain::SavedQuery {
                    id: match id {
                        Some(id) => id,
                        None => identify("view")?,
                    },
                    name,
                    query,
                },
            },
            Action::DeleteView { id } => domain::Command::DeleteQuery { id },
            Action::ReadNotification { id, read } => {
                let mut next = state.clone();
                if delivery::set_read(&mut next.delivery, actor, &id, read) {
                    journal.response =
                        Some(json!({"status":200,"body":command_result(&next,None,false,None)}));
                    store.commit(binding, &next, Some(&journal))?;
                    *state = next;
                    return replay(&journal, false);
                }
                domain::Command::ReadNotification { id, read }
            }
            Action::PrepareSubmit {} => domain::Command::PrepareSubmit {
                change: change_id.clone(),
                expected_revision: revision_id,
                candidate_id: identify("candidate")?,
            },
            Action::Submit { candidate_id } => {
                let change = current.as_ref().ok_or(Failure::Unavailable)?;
                let candidate = change
                    .candidate
                    .as_ref()
                    .ok_or_else(|| Failure::Api(409, "review_candidate_required".into()))?;
                domain::validate_candidate(
                    &state.review,
                    &change_id,
                    &revision_id,
                    &candidate_id,
                    actor,
                    &facts,
                )?;
                if state.operations.iter().any(|op| {
                    matches!(op.state.as_str(), "pending" | "uncertain")
                        && op.target_ref == change.target_ref
                        && op.kind == "submit"
                }) {
                    return reject(409, "review_target_reserved");
                }
                self.require_capability(provider, ReviewCapability::Submit)?;
                let operation = ReviewOperation::Submit {
                    commit: candidate.commit.clone(),
                    expected_target: candidate.target.clone(),
                };
                let completion = Completion::Submit {
                    change: change_id.clone(),
                    revision: revision_id,
                    candidate: candidate_id,
                    target: candidate.target.clone(),
                };
                let pending = Pending {
                    request: self.operation(
                        profile,
                        identify("operation")?,
                        operation,
                        change.target_ref.clone(),
                        now_ms,
                    ),
                    actor: actor.into(),
                    facts,
                    completion,
                    origin_change: change_id,
                    contribution: None,
                };
                return self.reserve_apply(context, store, state, &mut journal, pending);
            }
            action
            @ (Action::Rebase { .. } | Action::Revert { .. } | Action::CherryPick { .. }) => {
                let change = current.as_ref().ok_or(Failure::Unavailable)?;
                if change.owner != actor {
                    return reject(403, "review_owner_required");
                }
                let (kind, target, title, expected_target) = match action {
                    Action::Rebase { expected_target } => (
                        domain::MaintenanceKind::Rebase,
                        change.target_ref.clone(),
                        change.title.clone(),
                        expected_target,
                    ),
                    Action::Revert {
                        expected_target,
                        title,
                    } => (
                        domain::MaintenanceKind::Revert,
                        change.target_ref.clone(),
                        title,
                        expected_target,
                    ),
                    Action::CherryPick {
                        expected_target,
                        target_ref,
                        title,
                    } => (
                        domain::MaintenanceKind::CherryPick,
                        target_ref,
                        title,
                        expected_target,
                    ),
                    _ => return Err(Failure::Unavailable),
                };
                if (kind == domain::MaintenanceKind::Rebase
                    && change.state != domain::ChangeStatus::Open)
                    || (kind == domain::MaintenanceKind::Revert
                        && change.state != domain::ChangeStatus::Submitted)
                    || (kind == domain::MaintenanceKind::CherryPick
                        && (target == change.target_ref
                            || change.state == domain::ChangeStatus::Abandoned))
                    || !valid_text(&title, MAX_TITLE_BYTES)
                {
                    return reject(409, "review_maintenance_state");
                }
                if kind == domain::MaintenanceKind::Rebase
                    && change.revisions.len() >= MAX_REVISIONS
                {
                    return reject(409, "review_revision_bound");
                }
                self.require_target(profile, &target)?;
                let source_revision = change.current()?;
                let inspection = provider.inspect(&source_revision.commit, &target)?;
                if inspection.target_commit != expected_target {
                    return reject(409, "review_target_moved");
                }
                let operation = match kind {
                    domain::MaintenanceKind::Rebase => ReviewOperation::Rebase {
                        commit: source_revision.commit.clone(),
                        onto: expected_target.clone(),
                    },
                    domain::MaintenanceKind::Revert => ReviewOperation::Revert {
                        commit: source_revision.commit.clone(),
                        onto: expected_target.clone(),
                    },
                    domain::MaintenanceKind::CherryPick => ReviewOperation::CherryPick {
                        commit: source_revision.commit.clone(),
                        onto: expected_target.clone(),
                    },
                };
                self.require_capability(provider, operation.capability())?;
                self.require_capability(provider, ReviewCapability::Register)?;
                let result_change = if kind == domain::MaintenanceKind::Rebase {
                    change_id.clone()
                } else {
                    identify("change")?
                };
                if result_change != change_id && state.review.changes.len() >= MAX_CHANGES {
                    return reject(409, "review_change_bound");
                }
                let completion = Completion::Maintenance {
                    change: change_id.clone(),
                    source_revision: revision_id,
                    result_change,
                    result_revision: identify("revision")?,
                    title,
                    target_ref: target.clone(),
                    onto: expected_target,
                    kind,
                };
                let pending = Pending {
                    request: self.operation(
                        profile,
                        identify("operation")?,
                        operation,
                        target,
                        now_ms,
                    ),
                    actor: actor.into(),
                    facts,
                    completion,
                    origin_change: change_id,
                    contribution: None,
                };
                return self.reserve_apply(context, store, state, &mut journal, pending);
            }
        };
        let mut next = state.clone();
        domain::apply(&mut next.review, actor, command, &facts)?;
        if let Some((operation, completion, target, origin)) = effect {
            self.require_capability(provider, operation.capability())?;
            let pending = Pending {
                request: self.operation(profile, identify("operation")?, operation, target, now_ms),
                actor: actor.into(),
                facts,
                completion,
                origin_change: origin,
                contribution,
            };
            return self.reserve_apply(context, store, state, &mut journal, pending);
        }
        if let Some(id) = &result_change {
            bump(&mut next, id)?;
        }
        let result = command_result(&next, result_change.as_deref(), false, None);
        journal.response = Some(json!({"status":200,"body":result}));
        self.record_transition(profile, state, &mut next, actor, &facts)?;
        store.commit(binding, &next, Some(&journal))?;
        *state = next;
        replay(&journal, false)
    }

    fn require_capability(
        &self,
        provider: &impl ChangeReviewPort,
        capability: ReviewCapability,
    ) -> Result<(), Failure> {
        if !provider.info().capabilities.contains(&capability) {
            return reject(403, "review_capability_denied");
        }
        Ok(())
    }
    pub(super) fn operation(
        &self,
        profile: &Profile,
        id: String,
        operation: ReviewOperation,
        target_ref: String,
        now_ms: i64,
    ) -> ReviewOperationRequest {
        ReviewOperationRequest {
            version: REVIEW_PORT_VERSION,
            operation_id: id,
            repository_id: profile.provider.repository_id.clone(),
            target_ref,
            operation,
            signature: ReviewSignature {
                name: profile.signature_name.clone(),
                email: profile.signature_email.clone(),
                time_seconds: now_ms / MILLIS_PER_SECOND,
            },
        }
    }
    fn revision(
        &self,
        provider: &impl ChangeReviewPort,
        commit: &str,
        base: &str,
        id: String,
        number: u32,
        (actor, now_ms): (&str, i64),
    ) -> Result<domain::Revision, Failure> {
        let comparison = provider.compare(&CompareRequest {
            old_commit: base.into(),
            new_commit: commit.into(),
            limits: DiffLimits::default(),
            whitespace: WhitespaceMode::Exact,
        })?;
        if comparison.truncated || comparison.files.iter().any(|file| file.truncated) {
            return reject(409, "review_diff_bound");
        }
        let mut files = BTreeSet::new();
        for file in comparison.files {
            if let Some(path) = file.new_path.or(file.old_path) {
                files.insert(path);
            }
        }
        Ok(domain::Revision {
            id,
            number,
            commit: commit.into(),
            base: base.into(),
            files: files.into_iter().collect(),
            author: actor.into(),
            created_ms: now_ms,
        })
    }
    fn admit_anchor(
        &self,
        provider: &impl ChangeReviewPort,
        change: &domain::Change,
        anchor: &domain::Anchor,
        facts: &mut domain::Facts,
    ) -> Result<(), Failure> {
        let revision = change
            .revisions
            .iter()
            .find(|r| r.id == anchor.revision)
            .ok_or_else(|| Failure::Api(400, "review_anchor_revision".into()))?;
        if anchor.base_commit != revision.base
            && !change
                .revisions
                .iter()
                .any(|old| old.number < revision.number && old.commit == anchor.base_commit)
        {
            return reject(400, "review_anchor_base");
        }
        let diff = provider.compare(&CompareRequest {
            old_commit: anchor.base_commit.clone(),
            new_commit: revision.commit.clone(),
            limits: DiffLimits::default(),
            whitespace: WhitespaceMode::Exact,
        })?;
        let valid = !diff.truncated
            && diff.files.iter().any(|file| {
                if file.binary || file.truncated {
                    return false;
                }
                let (path, count) = match anchor.side {
                    domain::Side::Base => (&file.old_path, file.old_line_count),
                    domain::Side::Revision => (&file.new_path, file.new_line_count),
                };
                path.as_ref() == Some(&anchor.file)
                    && anchor.start_line > 0
                    && anchor.end_line >= anchor.start_line
                    && anchor.end_line - anchor.start_line <= MAX_ANCHOR_SPAN
                    && count.is_some_and(|count| anchor.end_line <= count)
            });
        if !valid {
            return reject(400, "review_anchor_invalid");
        }
        facts.anchors.push(anchor.clone());
        Ok(())
    }

    pub(super) fn reserve_apply(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        journal: &mut Journal,
        pending: Pending,
    ) -> Result<ApiResponse, Failure> {
        let ProviderContext {
            provider, binding, ..
        } = *context;
        if state.operations.len() >= MAX_OPERATIONS {
            return reject(409, "review_operation_bound");
        }
        if serde_json::to_vec(state)
            .map_err(|_| Failure::Unavailable)?
            .len()
            > RESERVATION_STATE_BYTES
        {
            return reject(409, "review_state_bound");
        }
        journal.pending = Some(serde_json::to_value(&pending).map_err(|_| Failure::Unavailable)?);
        upsert_operation(state, pending_view(&pending, "pending", None, None));
        if let Some((source_ref, item)) = &pending.contribution {
            state.contributions.insert(source_ref.clone(), item.clone());
        }
        // A FULL-synchronous commit is the authority boundary before any provider write.
        store.commit(binding, state, Some(journal))?;
        match provider.execute(&pending.request) {
            Ok(effect) => self.finish(context, store, state, journal, pending, effect),
            Err(error) if error.kind == ReviewErrorKind::Uncertain => self.finish(
                context,
                store,
                state,
                journal,
                pending,
                ReviewEffect::Uncertain {
                    message: "Exact provider outcome requires reconciliation".into(),
                },
            ),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn recover(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
    ) -> Result<(), Failure> {
        let provider = context.provider;
        for mut journal in store.pending()? {
            let pending: Pending =
                serde_json::from_value(journal.pending.clone().ok_or(Failure::Unavailable)?)
                    .map_err(|_| Failure::Unavailable)?;
            // Recovery only observes durable exact outcomes; it never re-executes a
            // previously authorized effect after policy/CI/actor eligibility may change.
            let effect = match provider.reconcile(&pending.request) {
                Ok(effect) => effect,
                Err(error) if error.kind == ReviewErrorKind::Uncertain => ReviewEffect::Uncertain {
                    message: "Exact provider outcome requires reconciliation".into(),
                },
                Err(error) => return Err(error.into()),
            };
            let _ = self.finish(context, store, state, &mut journal, pending, effect)?;
        }
        Ok(())
    }

    fn finish(
        &self,
        context: &ProviderContext<'_, impl ChangeReviewPort>,
        store: &mut store::Store,
        state: &mut State,
        journal: &mut Journal,
        pending: Pending,
        effect: ReviewEffect,
    ) -> Result<ApiResponse, Failure> {
        let ProviderContext {
            profile,
            provider,
            binding,
        } = *context;
        let (commit, target) = match effect {
            ReviewEffect::Uncertain { .. } => {
                let operation = pending_view(&pending, "uncertain", None, None);
                let changed = upsert_operation(state, operation.clone());
                journal.response = Some(
                    json!({"status":503,"body":{"code":"review_operation_uncertain","message":"The exact provider outcome is ambiguous. History remains available; mutations to this operation's changes are reserved until reconciliation.","operation":operation,"replayed":false}}),
                );
                if changed {
                    store.commit(binding, state, Some(journal))?;
                }
                return replay(journal, false);
            }
            ReviewEffect::NotApplied { target_commit } => {
                let operation = pending_view(&pending, "not_applied", None, Some(target_commit));
                upsert_operation(state, operation);
                journal.pending = None;
                journal.response = Some(
                    json!({"status":409,"body":{"code":"review_operation_not_applied","message":"The reserved provider operation was not applied. Prepare a fresh authorized request.","replayed":false}}),
                );
                store.commit(binding, state, Some(journal))?;
                return replay(journal, false);
            }
            ReviewEffect::Applied {
                commit,
                target_commit,
            } => (commit, target_commit),
        };
        let operation = pending_view(
            &pending,
            "applied",
            Some(commit.clone()),
            Some(target.clone()),
        );
        let mut next = state.clone();
        let mut facts = pending.facts.clone();
        let result_change;
        match pending.completion {
            Completion::Register { command, change } => {
                let expected = match &pending.request.operation {
                    ReviewOperation::Register { commit } => commit,
                    _ => return Err(Failure::Unavailable),
                };
                if &commit != expected {
                    return reject(503, "review_effect_mismatch");
                }
                domain::apply(&mut next.review, &pending.actor, command, &facts)?;
                bump(&mut next, &change)?;
                result_change = change;
            }
            Completion::Submit {
                change,
                revision,
                candidate,
                target: expected_target,
            } => {
                let current = next
                    .review
                    .changes
                    .get(&change)
                    .ok_or(Failure::Unavailable)?;
                let submitted = provider.inspect(&commit, &pending.request.target_ref)?;
                let source_revision = current.current()?;
                let expected_commit = current
                    .candidate
                    .as_ref()
                    .map(|candidate| candidate.commit.as_str())
                    .ok_or(Failure::Unavailable)?;
                if !submitted.ancestor_of_target || expected_commit != commit {
                    return reject(503, "review_submit_readback_mismatch");
                }
                let inspection =
                    provider.inspect(&source_revision.commit, &pending.request.target_ref)?;
                let mut observed = source_fact(profile, &inspection, source_revision.base.clone());
                observed.target_ancestors = facts
                    .source
                    .as_ref()
                    .map(|source| source.target_ancestors.clone())
                    .unwrap_or_default();
                observed.changed_files = facts
                    .source
                    .as_ref()
                    .and_then(|source| source.changed_files.clone());
                facts.source = Some(observed);
                domain::apply(
                    &mut next.review,
                    &pending.actor,
                    domain::Command::RecordSubmit {
                        change: change.clone(),
                        expected_revision: revision,
                        candidate_id: candidate,
                        expected_target,
                        result_commit: commit.clone(),
                    },
                    &facts,
                )?;
                bump(&mut next, &change)?;
                result_change = change;
            }
            Completion::Maintenance {
                change,
                source_revision,
                result_change: result,
                result_revision,
                title,
                target_ref,
                onto,
                kind,
            } => {
                let inspection = provider.inspect(&commit, &target_ref)?;
                facts.source = Some(source_fact(profile, &inspection, onto.clone()));
                let number = if kind == domain::MaintenanceKind::Rebase {
                    next.review
                        .changes
                        .get(&change)
                        .ok_or(Failure::Unavailable)?
                        .revisions
                        .len() as u32
                        + 1
                } else {
                    1
                };
                let revision = self.revision(
                    provider,
                    &commit,
                    &onto,
                    result_revision.clone(),
                    number,
                    (&pending.actor, facts.now_ms),
                )?;
                facts.integration = None;
                self.populate_changed_files(provider, &revision, &mut facts)?;
                let command = if kind == domain::MaintenanceKind::Rebase {
                    domain::Command::Upload {
                        change: change.clone(),
                        expected_revision: source_revision.clone(),
                        revision,
                    }
                } else {
                    domain::Command::Create {
                        id: result.clone(),
                        project: profile.project.clone(),
                        repository: profile.provider.repository_id.clone(),
                        target_ref,
                        title,
                        description: format!(
                            "Service-created {kind:?} of change {change} revision {source_revision}"
                        ),
                        wip: true,
                        revision,
                    }
                };
                domain::apply(&mut next.review, &pending.actor, command, &facts)?;
                facts.event_id =
                    hash(&json!(["maintenance-event", &pending.request.operation_id]))?;
                let expected_current = if kind == domain::MaintenanceKind::Rebase {
                    result_revision.clone()
                } else {
                    source_revision.clone()
                };
                let outcome = domain::MaintenanceOutcome {
                    operation_id: pending.request.operation_id.clone(),
                    kind,
                    source_revision: source_revision.clone(),
                    result_change: result.clone(),
                    result_revision,
                    result_commit: commit.clone(),
                    actor: pending.actor.clone(),
                    created_ms: facts.now_ms,
                };
                domain::apply(
                    &mut next.review,
                    &pending.actor,
                    domain::Command::RecordMaintenance {
                        change: change.clone(),
                        expected_revision: expected_current,
                        outcome,
                    },
                    &facts,
                )?;
                bump(&mut next, &change)?;
                if result != change {
                    bump(&mut next, &result)?;
                }
                result_change = result;
            }
            Completion::Suggestions {
                change,
                source_revision,
                result_revision,
                base,
            } => {
                let current = next
                    .review
                    .changes
                    .get(&change)
                    .ok_or(Failure::Unavailable)?;
                let revision = self.revision(
                    provider,
                    &commit,
                    &base,
                    result_revision,
                    current.revisions.len() as u32 + 1,
                    (&pending.actor, facts.now_ms),
                )?;
                let inspection = provider.inspect(&commit, &current.target_ref)?;
                facts.source = Some(source_fact(profile, &inspection, base));
                facts.integration = None;
                self.populate_changed_files(provider, &revision, &mut facts)?;
                domain::apply(
                    &mut next.review,
                    &pending.actor,
                    domain::Command::Upload {
                        change: change.clone(),
                        expected_revision: source_revision,
                        revision,
                    },
                    &facts,
                )?;
                bump(&mut next, &change)?;
                result_change = change;
            }
            Completion::Integration { queue_id } => {
                let entry = next
                    .queue
                    .iter_mut()
                    .find(|entry| entry.id == queue_id)
                    .ok_or(Failure::Unavailable)?;
                entry.candidate_commit = Some(commit.clone());
                entry.status = "waiting_ci".into();
                entry.detail = "Exact integration candidate awaits canonical CI".into();
                result_change = entry.change.clone();
            }
        }
        upsert_operation(&mut next, operation.clone());
        if let Some((source_ref, mut item)) = pending.contribution {
            item.commit = Some(commit.clone());
            next.contributions.insert(source_ref, item);
        }
        self.record_transition(profile, state, &mut next, &pending.actor, &facts)?;
        let result = command_result(&next, Some(&result_change), false, Some(&operation));
        journal.pending = None;
        journal.response = Some(json!({"status":200,"body":result}));
        store.commit(binding, &next, Some(journal))?;
        *state = next;
        replay(journal, false)
    }
}
fn bump(state: &mut State, id: &str) -> Result<(), Failure> {
    let version = state.versions.entry(id.into()).or_default();
    *version = version.checked_add(1).ok_or(Failure::Unavailable)?;
    Ok(())
}
fn operation_kind(operation: &ReviewOperation) -> &'static str {
    match operation {
        ReviewOperation::Register { .. } => "register",
        ReviewOperation::Submit { .. } => "submit",
        ReviewOperation::Rebase { .. } => "rebase",
        ReviewOperation::Revert { .. } => "revert",
        ReviewOperation::CherryPick { .. } => "cherry_pick",
        ReviewOperation::ApplySuggestions { .. } => "apply_suggestions",
        ReviewOperation::BuildIntegration { .. } => "build_integration",
    }
}
fn command_result(
    state: &State,
    change: Option<&str>,
    replayed: bool,
    operation: Option<&OperationView>,
) -> Value {
    json!({"change":change.filter(|id|state.review.changes.contains_key(*id)).map(|id|json!({"id":id,"version":state.versions.get(id).copied().unwrap_or(0)})),"replayed":replayed,"operation":operation})
}
fn replay(journal: &Journal, replayed: bool) -> Result<ApiResponse, Failure> {
    let result = journal
        .response
        .as_ref()
        .ok_or_else(|| Failure::Api(503, "review_operation_pending".into()))?;
    let status = result
        .get("status")
        .and_then(Value::as_u64)
        .and_then(|n| u16::try_from(n).ok())
        .ok_or(Failure::Unavailable)?;
    let mut body = result.get("body").cloned().ok_or(Failure::Unavailable)?;
    if let Value::Object(map) = &mut body {
        map.insert("replayed".into(), json!(replayed));
    }
    json_response(status, body).map_err(Failure::from)
}
fn validate_action(action: &Action) -> Result<(), Failure> {
    let valid = match action {
        Action::Contribute {
            title,
            description,
            source_ref,
            ..
        } => {
            valid_text(title, MAX_TITLE_BYTES)
                && description.len() <= MAX_DESCRIPTION_BYTES
                && source_ref.len() <= MAX_IDENTITY_BYTES
        }
        Action::ApplySuggestions { thread_ids } => !thread_ids.is_empty() && thread_ids.len() <= 32,
        Action::Create {
            title, description, ..
        }
        | Action::Edit { title, description } => {
            valid_text(title, MAX_TITLE_BYTES) && description.len() <= MAX_DESCRIPTION_BYTES
        }
        Action::StartThread { body, .. }
        | Action::SaveDraft { body, .. }
        | Action::Reply { body, .. } => !body.trim().is_empty() && body.len() <= MAX_COMMENT_BYTES,
        Action::Publish { draft_ids, .. } => {
            draft_ids.len() <= MAX_DRAFTS
                && draft_ids
                    .iter()
                    .all(|id| valid_text(id, MAX_SHORT_TEXT_BYTES))
        }
        Action::SetDependencies { dependencies } => dependencies.len() <= MAX_DEPENDENCIES,
        Action::SaveView { name, query, .. } => {
            valid_text(name, MAX_SHORT_TEXT_BYTES) && query.text.len() <= MAX_QUERY_BYTES
        }
        Action::Revert { title, .. } | Action::CherryPick { title, .. } => {
            valid_text(title, MAX_TITLE_BYTES)
        }
        _ => true,
    };
    if !valid {
        return reject(400, "review_action_bound");
    }
    Ok(())
}
fn pending_view(
    pending: &Pending,
    state: &str,
    commit: Option<String>,
    target_commit: Option<String>,
) -> OperationView {
    let result_change = match &pending.completion {
        Completion::Maintenance { result_change, .. } => Some(result_change.clone()),
        Completion::Register { change, .. } => Some(change.clone()),
        Completion::Submit { change, .. } => Some(change.clone()),
        Completion::Suggestions { change, .. } => Some(change.clone()),
        Completion::Integration { .. } => Some(pending.origin_change.clone()),
    };
    OperationView {
        change: pending.origin_change.clone(),
        operation_id: pending.request.operation_id.clone(),
        kind: operation_kind(&pending.request.operation).into(),
        state: state.into(),
        result_change,
        target_ref: pending.request.target_ref.clone(),
        commit,
        target_commit,
    }
}
fn upsert_operation(state: &mut State, operation: OperationView) -> bool {
    if let Some(existing) = state
        .operations
        .iter_mut()
        .find(|op| op.operation_id == operation.operation_id)
    {
        if existing.state == operation.state
            && existing.commit == operation.commit
            && existing.target_commit == operation.target_commit
        {
            return false;
        }
        *existing = operation;
    } else {
        state.operations.push(operation);
    }
    true
}
