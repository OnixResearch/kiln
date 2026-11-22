const RESOLUTION_OPERATION_DOMAIN: &str = "cairn-change-collaboration-resolution-v1";
const RESOLUTION_SIMULATION_OPERATION: &str = "cairn-resolution-simulation";

// r[impl cairn.collaboration_resolution.inspection]
// r[impl cairn.collaboration_resolution.plan]

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChangeArtifactConflictHead {
    pub revision_id: String,
    pub actor: String,
    pub deleted: bool,
    pub content_blake3: Option<String>,
    pub content_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChangeArtifactConflict {
    pub artifact: super::radicle_change::ChangeArtifactId,
    pub heads: Vec<ChangeArtifactConflictHead>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChangeLifecycleConflictHead {
    pub revision_id: String,
    pub actor: String,
    pub record: super::radicle_change::ChangeLifecycleRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Inspection {
    pub change_id: String,
    pub state_hash: String,
    pub artifact_conflicts: Vec<ChangeArtifactConflict>,
    pub lifecycle_heads: Vec<ChangeLifecycleConflictHead>,
}

impl Inspection {
    pub fn has_conflicts(&self) -> bool {
        !self.artifact_conflicts.is_empty() || !self.lifecycle_heads.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChangeArtifactConflictValue {
    pub artifact: super::radicle_change::ChangeArtifactId,
    pub revision_id: String,
    pub actor: String,
    pub deleted: bool,
    pub content: Option<String>,
    pub content_blake3: Option<String>,
    pub content_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChangeArtifactHeadSelection {
    pub artifact: super::radicle_change::ChangeArtifactId,
    pub revision_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub change_id: String,
    pub policy_hash: String,
    pub actor: String,
    pub is_delegate: bool,
    pub artifact_selections: Vec<ChangeArtifactHeadSelection>,
    pub lifecycle_status: Option<super::radicle_change::ChangeLifecycleStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Plan {
    pub change_id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub input_state_hash: String,
    pub operation_id: String,
    pub actions: Vec<super::collaboration_change::Action>,
    pub selected_artifacts: Vec<ChangeArtifactHeadSelection>,
    pub lifecycle_reset: bool,
    pub expected_state_hash: String,
    pub expected_content_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CobPlan {
    pub change_id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub input_state_hash: String,
    pub operation_id: String,
    pub actions: Vec<super::radicle_change::ChangeCobAction>,
    pub selected_artifacts: Vec<ChangeArtifactHeadSelection>,
    pub lifecycle_reset: bool,
    pub expected_content_root: String,
}

pub fn inspect(
    state: &super::collaboration_change::State,
) -> Result<Inspection, super::radicle_change::ChangeCobError> {
    super::collaboration_change::validate_change_collaboration_state(state)?;
    let artifact_conflicts = state
        .artifacts
        .iter()
        .filter_map(|(artifact, history)| {
            let heads = history.heads();
            (heads.len() > 1).then(|| ChangeArtifactConflict {
                artifact: artifact.clone(),
                heads: heads
                    .into_iter()
                    .map(|(revision_id, revision)| ChangeArtifactConflictHead {
                        revision_id: revision_id.to_string(),
                        actor: revision.actor.clone(),
                        deleted: revision.value.is_none(),
                        content_blake3: revision
                            .value
                            .as_ref()
                            .map(|content| super::evidence::blake3_hex(content.as_bytes())),
                        content_bytes: revision.value.as_ref().map(String::len),
                    })
                    .collect(),
            })
        })
        .collect();
    let lifecycle = state.lifecycle.heads();
    let lifecycle_heads = if lifecycle.len() > 1 {
        lifecycle
            .into_iter()
            .map(|(revision_id, revision)| ChangeLifecycleConflictHead {
                revision_id: revision_id.to_string(),
                actor: revision.actor.clone(),
                record: revision.value.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(Inspection {
        change_id: state.change_id.clone(),
        state_hash: super::collaboration_change::change_collaboration_state_hash(state)?,
        artifact_conflicts,
        lifecycle_heads,
    })
}

pub fn inspect_change_cob_conflicts(
    state: &super::radicle_change::ChangeCobState,
) -> Result<Inspection, super::radicle_change::ChangeCobError> {
    inspect(&super::collaboration_change::state_from_compatibility(
        state.clone(),
    )?)
}

pub fn inspect_value(
    state: &super::collaboration_change::State,
    artifact: &super::radicle_change::ChangeArtifactId,
    revision_id: &str,
) -> Result<ChangeArtifactConflictValue, super::radicle_change::ChangeCobError> {
    let inspection = inspect(state)?;
    let Some(conflict) = inspection
        .artifact_conflicts
        .iter()
        .find(|conflict| &conflict.artifact == artifact)
    else {
        return Err(invalid(
            "resolution.show.artifact",
            "selected artifact is not currently conflicted",
        ));
    };
    if !conflict
        .heads
        .iter()
        .any(|head| head.revision_id == revision_id)
    {
        return Err(invalid(
            "resolution.show.revision",
            "selected revision is not a current conflict head",
        ));
    }
    let history = state.artifacts.get(artifact).ok_or_else(|| {
        invalid(
            "resolution.show.artifact",
            "selected artifact history is missing",
        )
    })?;
    let revision = history.revisions.get(revision_id).ok_or_else(|| {
        invalid(
            "resolution.show.revision",
            "selected revision history is missing",
        )
    })?;
    Ok(ChangeArtifactConflictValue {
        artifact: artifact.clone(),
        revision_id: revision_id.to_string(),
        actor: revision.actor.clone(),
        deleted: revision.value.is_none(),
        content: revision.value.clone(),
        content_blake3: revision
            .value
            .as_ref()
            .map(|content| super::evidence::blake3_hex(content.as_bytes())),
        content_bytes: revision.value.as_ref().map(String::len),
    })
}

pub fn inspect_change_cob_conflict_value(
    state: &super::radicle_change::ChangeCobState,
    artifact: &super::radicle_change::ChangeArtifactId,
    revision_id: &str,
) -> Result<ChangeArtifactConflictValue, super::radicle_change::ChangeCobError> {
    inspect_value(
        &super::collaboration_change::state_from_compatibility(state.clone())?,
        artifact,
        revision_id,
    )
}

pub fn plan(
    state: &super::collaboration_change::State,
    snapshot: Snapshot,
) -> Result<Plan, super::radicle_change::ChangeCobError> {
    super::collaboration_change::validate_change_collaboration_state(state)?;
    validate_resolution_context(state, &snapshot)?;
    let inspection = inspect(state)?;
    if !inspection.has_conflicts() {
        return Err(invalid(
            "resolution.conflicts",
            "collaboration state has no current conflicts",
        ));
    }
    let selections = unique_selections(&snapshot.artifact_selections)?;
    validate_complete_selections(&inspection, &selections)?;
    validate_lifecycle_selection(&inspection, snapshot.lifecycle_status)?;

    let selected_artifacts = selections.values().cloned().collect::<Vec<_>>();
    let mut artifact_actions = Vec::with_capacity(selected_artifacts.len());
    for selection in &selected_artifacts {
        let history = state.artifacts.get(&selection.artifact).ok_or_else(|| {
            invalid(
                "resolution.artifact",
                "selected artifact history is missing",
            )
        })?;
        let basis = history.head_ids();
        let selected_revision = basis
            .iter()
            .find(|revision| same_operation_id(revision, &selection.revision_id))
            .ok_or_else(|| {
                invalid(
                    "resolution.revision",
                    "selected artifact revision is not a current head",
                )
            })?;
        let revision = history.revisions.get(selected_revision).ok_or_else(|| {
            invalid(
                "resolution.revision",
                "selected artifact revision is missing",
            )
        })?;
        artifact_actions.push(super::collaboration_change::Action::SetArtifact {
            artifact: selection.artifact.clone(),
            content: revision.value.clone(),
            basis,
        });
    }

    let resolved_content_root = simulate_resolved_content(state, &snapshot, &artifact_actions)?;
    let lifecycle_reset = !inspection.lifecycle_heads.is_empty();
    let mut actions = artifact_actions;
    if lifecycle_reset {
        actions.push(super::collaboration_change::Action::SetLifecycle {
            status: super::radicle_change::ChangeLifecycleStatus::Open,
            subject_root: resolved_content_root,
            evidence: std::collections::BTreeSet::new(),
            basis: state.lifecycle.head_ids(),
        });
    }
    let input_state_hash = inspection.state_hash;
    let operation_id = operation_id(
        &input_state_hash,
        &snapshot.actor,
        snapshot.is_delegate,
        &actions,
    )?;
    let mut simulated = state.clone();
    super::collaboration_change::apply_change_collaboration_operation(
        &mut simulated,
        super::collaboration_change::Operation {
            id: operation_id.clone(),
            actor: snapshot.actor.clone(),
            is_delegate: snapshot.is_delegate,
            actions: actions.clone(),
        },
    )?;
    require_conflict_free_result(&simulated)?;
    Ok(Plan {
        change_id: state.change_id.clone(),
        actor: snapshot.actor,
        is_delegate: snapshot.is_delegate,
        input_state_hash,
        operation_id,
        actions,
        selected_artifacts,
        lifecycle_reset,
        expected_state_hash: super::collaboration_change::change_collaboration_state_hash(
            &simulated,
        )?,
        expected_content_root: super::collaboration_change::change_collaboration_content_root(
            &simulated,
        )?,
    })
}

pub fn plan_change_cob_resolution(
    state: &super::radicle_change::ChangeCobState,
    snapshot: Snapshot,
) -> Result<CobPlan, super::radicle_change::ChangeCobError> {
    let neutral = super::collaboration_change::state_from_compatibility(state.clone())?;
    let plan = plan(&neutral, snapshot)?;
    let actions = plan
        .actions
        .into_iter()
        .map(super::collaboration_change::action_into_compatibility)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CobPlan {
        change_id: plan.change_id,
        actor: plan.actor,
        is_delegate: plan.is_delegate,
        input_state_hash: plan.input_state_hash,
        operation_id: plan.operation_id,
        actions,
        selected_artifacts: plan.selected_artifacts,
        lifecycle_reset: plan.lifecycle_reset,
        expected_content_root: plan.expected_content_root,
    })
}

pub fn apply(
    state: &super::collaboration_change::State,
    plan: &Plan,
) -> Result<super::collaboration_change::State, super::radicle_change::ChangeCobError> {
    let current_state_hash = super::collaboration_change::change_collaboration_state_hash(state)?;
    if !same_digest::<super::nominal_identity::StateDigestRole>(
        &current_state_hash,
        &plan.input_state_hash,
    ) {
        return Err(invalid(
            "resolution.input_state_hash",
            "collaboration state changed after planning",
        ));
    }
    let mut resolved = state.clone();
    super::collaboration_change::apply_change_collaboration_operation(
        &mut resolved,
        super::collaboration_change::Operation {
            id: plan.operation_id.clone(),
            actor: plan.actor.clone(),
            is_delegate: plan.is_delegate,
            actions: plan.actions.clone(),
        },
    )?;
    require_conflict_free_result(&resolved)?;
    let state_hash = super::collaboration_change::change_collaboration_state_hash(&resolved)?;
    if !same_digest::<super::nominal_identity::StateDigestRole>(
        &state_hash,
        &plan.expected_state_hash,
    ) {
        return Err(invalid(
            "resolution.expected_state_hash",
            "resolved state does not match the reviewed plan",
        ));
    }
    let content_root = super::collaboration_change::change_collaboration_content_root(&resolved)?;
    if !same_digest::<super::nominal_identity::ContentRootDigestRole>(
        &content_root,
        &plan.expected_content_root,
    ) {
        return Err(invalid(
            "resolution.expected_content_root",
            "resolved content does not match the reviewed plan",
        ));
    }
    Ok(resolved)
}

pub fn hash_plan(plan: &Plan) -> Result<String, super::radicle_change::ChangeCobError> {
    plan_hash(plan)
}

pub fn hash_cob_plan(plan: &CobPlan) -> Result<String, super::radicle_change::ChangeCobError> {
    plan_hash(plan)
}

fn plan_hash<T: serde::Serialize>(
    plan: &T,
) -> Result<String, super::radicle_change::ChangeCobError> {
    let value = serde_json::to_value(plan).map_err(|error| {
        super::radicle_change::ChangeCobError::Serialization {
            reason: error.to_string(),
        }
    })?;
    let hash = super::evidence::canonical_json_hash(&value);
    let digest = super::nominal_identity::PlanDigest::parse(
        &hash,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "resolution.plan_hash",
    )
    .map_err(nominal_error)?;
    Ok(digest.as_hex().to_owned())
}

fn validate_resolution_context(
    state: &super::collaboration_change::State,
    snapshot: &Snapshot,
) -> Result<(), super::radicle_change::ChangeCobError> {
    let state_change =
        super::nominal_identity::ChangeId::parse(&state.change_id, "resolution.state_change_id")
            .map_err(nominal_error)?;
    let snapshot_change = super::nominal_identity::ChangeId::parse(
        &snapshot.change_id,
        "resolution.snapshot_change_id",
    )
    .map_err(nominal_error)?;
    if state_change != snapshot_change {
        return Err(invalid(
            "resolution.change_id",
            "snapshot and collaboration state name different changes",
        ));
    }
    let state_policy = super::nominal_identity::PolicyDigest::parse_compatible(
        &state.policy_hash,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "resolution.state_policy_hash",
    )
    .map_err(nominal_error)?;
    let snapshot_policy = super::nominal_identity::PolicyDigest::parse_compatible(
        &snapshot.policy_hash,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "resolution.snapshot_policy_hash",
    )
    .map_err(nominal_error)?;
    if state_policy != snapshot_policy {
        return Err(invalid(
            "resolution.policy_hash",
            "current policy does not match collaboration state",
        ));
    }
    let actor_bytes = snapshot.actor.len();
    if actor_bytes > super::radicle_change::MAX_ACTOR_ID_BYTES {
        return Err(super::radicle_change::ChangeCobError::ByteLimitExceeded {
            field: "actor".to_string(),
            actual: actor_bytes,
            limit: super::radicle_change::MAX_ACTOR_ID_BYTES,
        });
    }
    let actor = super::nominal_identity::ActorId::parse(&snapshot.actor, "resolution.actor")
        .map_err(nominal_error)?;
    let creator = super::nominal_identity::ActorId::parse(&state.creator, "resolution.creator")
        .map_err(nominal_error)?;
    if actor != creator && !snapshot.is_delegate {
        return Err(super::radicle_change::ChangeCobError::Unauthorized {
            actor: snapshot.actor.clone(),
            action: "plan_collaboration_resolution".to_string(),
        });
    }
    Ok(())
}

fn unique_selections(
    selections: &[ChangeArtifactHeadSelection],
) -> Result<
    std::collections::BTreeMap<
        super::radicle_change::ChangeArtifactId,
        ChangeArtifactHeadSelection,
    >,
    super::radicle_change::ChangeCobError,
> {
    let mut unique = std::collections::BTreeMap::new();
    for selection in selections {
        if unique
            .insert(selection.artifact.clone(), selection.clone())
            .is_some()
        {
            return Err(invalid(
                "resolution.selections",
                "an artifact has more than one selected head",
            ));
        }
    }
    Ok(unique)
}

fn validate_complete_selections(
    inspection: &Inspection,
    selections: &std::collections::BTreeMap<
        super::radicle_change::ChangeArtifactId,
        ChangeArtifactHeadSelection,
    >,
) -> Result<(), super::radicle_change::ChangeCobError> {
    let conflicted = inspection
        .artifact_conflicts
        .iter()
        .map(|conflict| conflict.artifact.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let selected = selections
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if conflicted != selected {
        let missing = conflicted
            .difference(&selected)
            .cloned()
            .collect::<Vec<_>>();
        let extra = selected
            .difference(&conflicted)
            .cloned()
            .collect::<Vec<_>>();
        return Err(invalid(
            "resolution.selections",
            format!("selections must cover exactly every conflict; missing {missing:?}, extra {extra:?}"),
        ));
    }
    for conflict in &inspection.artifact_conflicts {
        let selection = selections.get(&conflict.artifact).ok_or_else(|| {
            invalid(
                "resolution.selections",
                "conflicted artifact selection is missing",
            )
        })?;
        if !conflict
            .heads
            .iter()
            .any(|head| same_operation_id(&head.revision_id, &selection.revision_id))
        {
            return Err(invalid(
                "resolution.revision",
                "selected artifact revision is not a current conflict head",
            ));
        }
    }
    Ok(())
}

fn validate_lifecycle_selection(
    inspection: &Inspection,
    lifecycle_status: Option<super::radicle_change::ChangeLifecycleStatus>,
) -> Result<(), super::radicle_change::ChangeCobError> {
    if inspection.lifecycle_heads.is_empty() {
        if lifecycle_status.is_some() {
            return Err(invalid(
                "resolution.lifecycle",
                "lifecycle selection was provided without a lifecycle conflict",
            ));
        }
        return Ok(());
    }
    match lifecycle_status {
        Some(super::radicle_change::ChangeLifecycleStatus::Open) => Ok(()),
        Some(_) => Err(invalid(
            "resolution.lifecycle",
            "conflicted lifecycle may only be reset explicitly to open",
        )),
        None => Err(invalid(
            "resolution.lifecycle",
            "lifecycle conflict requires explicit open reset",
        )),
    }
}

fn simulate_resolved_content(
    state: &super::collaboration_change::State,
    snapshot: &Snapshot,
    artifact_actions: &[super::collaboration_change::Action],
) -> Result<String, super::radicle_change::ChangeCobError> {
    if artifact_actions.is_empty() {
        return super::collaboration_change::change_collaboration_content_root(state);
    }
    let mut simulated = state.clone();
    super::collaboration_change::apply_change_collaboration_operation(
        &mut simulated,
        super::collaboration_change::Operation {
            id: RESOLUTION_SIMULATION_OPERATION.to_string(),
            actor: snapshot.actor.clone(),
            is_delegate: snapshot.is_delegate,
            actions: artifact_actions.to_vec(),
        },
    )?;
    super::collaboration_change::change_collaboration_content_root(&simulated)
}

fn require_conflict_free_result(
    state: &super::collaboration_change::State,
) -> Result<(), super::radicle_change::ChangeCobError> {
    let _content_root = super::collaboration_change::change_collaboration_content_root(state)?;
    match super::collaboration_change::change_collaboration_lifecycle_view(state)? {
        super::radicle_change::ChangeLifecycleView::Conflicted { revisions } => Err(invalid(
            "resolution.result.lifecycle",
            format!("resolution left lifecycle heads conflicted: {revisions:?}"),
        )),
        super::radicle_change::ChangeLifecycleView::ContentConflicted { artifacts } => {
            Err(super::radicle_change::ChangeCobError::ArtifactConflict { artifacts })
        }
        super::radicle_change::ChangeLifecycleView::Current(_)
        | super::radicle_change::ChangeLifecycleView::Stale { .. } => Ok(()),
    }
}

fn same_operation_id(left: &str, right: &str) -> bool {
    let left = super::nominal_identity::OperationId::parse(left, "resolution.left_operation_id");
    let right = super::nominal_identity::OperationId::parse(right, "resolution.right_operation_id");
    matches!((left, right), (Ok(left), Ok(right)) if left == right)
}

fn same_digest<Role: super::nominal_identity::DigestRoleMarker>(left: &str, right: &str) -> bool {
    let left = super::nominal_identity::Blake3Digest::<Role>::parse_compatible(
        left,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "resolution.left_digest",
    );
    let right = super::nominal_identity::Blake3Digest::<Role>::parse_compatible(
        right,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "resolution.right_digest",
    );
    matches!(
        (left, right),
        (Ok(left), Ok(right)) if left.as_hex() == right.as_hex()
    )
}

fn nominal_error(
    issue: super::nominal_identity::IdentityIssue,
) -> super::radicle_change::ChangeCobError {
    super::radicle_change::ChangeCobError::InvalidField {
        field: issue.field.to_owned(),
        reason: issue.code.to_owned(),
    }
}

fn operation_id(
    input_state_hash: &str,
    actor: &str,
    is_delegate: bool,
    actions: &[super::collaboration_change::Action],
) -> Result<String, super::radicle_change::ChangeCobError> {
    let value = serde_json::to_value(OperationIdentity {
        domain: RESOLUTION_OPERATION_DOMAIN,
        input_state_hash,
        actor,
        is_delegate,
        actions,
    })
    .map_err(
        |error| super::radicle_change::ChangeCobError::Serialization {
            reason: error.to_string(),
        },
    )?;
    let hash = super::evidence::canonical_json_hash(&value);
    let operation = super::nominal_identity::OperationId::parse(&hash, "resolution.operation_id")
        .map_err(nominal_error)?;
    Ok(operation.as_str().to_owned())
}

fn invalid(field: &str, reason: impl Into<String>) -> super::radicle_change::ChangeCobError {
    super::radicle_change::ChangeCobError::InvalidField {
        field: field.to_string(),
        reason: reason.into(),
    }
}

#[derive(serde::Serialize)]
struct OperationIdentity<'a> {
    domain: &'a str,
    input_state_hash: &'a str,
    actor: &'a str,
    is_delegate: bool,
    actions: &'a [super::collaboration_change::Action],
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANGE_ID: &str = "collaboration-resolution";
    const WORKSPACE_ID: &str = "workspace:resolution";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const CREATOR: &str = "actor:creator";
    const DELEGATE: &str = "actor:delegate";
    const OUTSIDER: &str = "actor:outsider";
    const LEFT_OPERATION: &str = "operation:left";
    const RIGHT_OPERATION: &str = "operation:right";
    const LEFT_DESIGN_OPERATION: &str = "operation:design-left";
    const RIGHT_DESIGN_OPERATION: &str = "operation:design-right";
    const LIFECYCLE_LEFT_OPERATION: &str = "operation:lifecycle-left";
    const LIFECYCLE_RIGHT_OPERATION: &str = "operation:lifecycle-right";
    const CONCURRENT_HEAD_COUNT: usize = 2;
    const CONFLICT_ARTIFACT_COUNT: usize = 2;
    const RESOLUTION_ACTION_COUNT: usize = 2;
    const RESOLVED_PROPOSAL_REVISION_COUNT: usize = 4;

    fn initial_state() -> super::super::collaboration_change::State {
        let init =
            crate::plan_change_collaboration_import(crate::ChangeCollaborationImportSnapshot {
                change_id: CHANGE_ID.to_string(),
                workspace_id: WORKSPACE_ID.to_string(),
                base_revision: crate::ChangeBaseRevision::new("opaque", "revision-one").unwrap(),
                policy_hash: POLICY_HASH.to_string(),
                proposal: Some("# Proposal\n".to_string()),
                design: Some("# Design\n".to_string()),
                tasks: None,
                metadata: None,
                delta_specs: std::collections::BTreeMap::new(),
            })
            .unwrap();
        crate::initialize_change_collaboration_import(init, CREATOR).unwrap()
    }

    fn add_artifact_conflict(
        state: &mut super::super::collaboration_change::State,
        artifact: super::super::radicle_change::ChangeArtifactId,
        operations: [(&str, Option<&str>); CONCURRENT_HEAD_COUNT],
    ) {
        let basis = state.artifacts.get(&artifact).unwrap().head_ids();
        for (operation_id, content) in operations {
            crate::apply_change_collaboration_operation(
                state,
                super::super::collaboration_change::Operation {
                    id: operation_id.to_string(),
                    actor: CREATOR.to_string(),
                    is_delegate: false,
                    actions: vec![super::super::collaboration_change::Action::SetArtifact {
                        artifact: artifact.clone(),
                        content: content.map(str::to_string),
                        basis: basis.clone(),
                    }],
                },
            )
            .unwrap();
        }
    }

    fn conflicted_state() -> super::super::collaboration_change::State {
        let mut state = initial_state();
        add_artifact_conflict(
            &mut state,
            super::super::radicle_change::ChangeArtifactId::proposal(),
            [
                (LEFT_OPERATION, Some("# Left proposal\n")),
                (RIGHT_OPERATION, Some("# Right proposal\n")),
            ],
        );
        add_artifact_conflict(
            &mut state,
            super::super::radicle_change::ChangeArtifactId::design(),
            [
                (LEFT_DESIGN_OPERATION, Some("# Left design\n")),
                (RIGHT_DESIGN_OPERATION, None),
            ],
        );
        state
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            change_id: CHANGE_ID.to_string(),
            policy_hash: POLICY_HASH.to_string(),
            actor: CREATOR.to_string(),
            is_delegate: false,
            artifact_selections: vec![
                ChangeArtifactHeadSelection {
                    artifact: super::super::radicle_change::ChangeArtifactId::proposal(),
                    revision_id: LEFT_OPERATION.to_string(),
                },
                ChangeArtifactHeadSelection {
                    artifact: super::super::radicle_change::ChangeArtifactId::design(),
                    revision_id: RIGHT_DESIGN_OPERATION.to_string(),
                },
            ],
            lifecycle_status: None,
        }
    }

    // r[verify cairn.collaboration_resolution.inspection.complete]
    // r[verify cairn.collaboration_resolution.inspection.show]
    #[test]
    fn inspection_lists_every_head_and_discloses_only_selected_value() {
        let state = conflicted_state();
        let inspection = inspect(&state).unwrap();
        assert_eq!(inspection.artifact_conflicts.len(), CONFLICT_ARTIFACT_COUNT);
        assert!(inspection
            .artifact_conflicts
            .iter()
            .all(|conflict| conflict.heads.len() == CONCURRENT_HEAD_COUNT));
        let shown = inspect_value(
            &state,
            &super::super::radicle_change::ChangeArtifactId::proposal(),
            LEFT_OPERATION,
        )
        .unwrap();
        assert_eq!(shown.content.as_deref(), Some("# Left proposal\n"));
        assert!(inspect_value(
            &state,
            &super::super::radicle_change::ChangeArtifactId::tasks(),
            LEFT_OPERATION,
        )
        .is_err());
    }

    // r[verify cairn.collaboration_resolution.plan.complete]
    #[test]
    fn complete_selection_supersedes_all_heads_and_preserves_history() {
        let state = conflicted_state();
        let first = plan(&state, snapshot()).unwrap();
        let second = plan(&state, snapshot()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.actions.len(), RESOLUTION_ACTION_COUNT);
        assert_eq!(
            first.actions[0],
            super::super::collaboration_change::Action::SetArtifact {
                artifact: super::super::radicle_change::ChangeArtifactId::design(),
                content: None,
                basis: std::collections::BTreeSet::from([
                    LEFT_DESIGN_OPERATION.to_string(),
                    RIGHT_DESIGN_OPERATION.to_string(),
                ]),
            }
        );
        let resolved = apply(&state, &first).unwrap();
        assert_eq!(
            super::super::collaboration_change::change_collaboration_state_hash(&resolved).unwrap(),
            first.expected_state_hash
        );
        assert_eq!(
            super::super::collaboration_change::change_collaboration_content_root(&resolved)
                .unwrap(),
            first.expected_content_root
        );
        assert_eq!(
            resolved.artifacts[&super::super::radicle_change::ChangeArtifactId::proposal()]
                .revisions
                .len(),
            RESOLVED_PROPOSAL_REVISION_COUNT
        );
        assert!(!inspect(&resolved).unwrap().has_conflicts());
    }

    // r[verify cairn.collaboration_resolution.plan.lifecycle]
    #[test]
    fn lifecycle_conflict_requires_explicit_open_reset_over_every_head() {
        let mut state = initial_state();
        let basis = state.lifecycle.head_ids();
        let content_root =
            super::super::collaboration_change::change_collaboration_content_root(&state).unwrap();
        for operation_id in [LIFECYCLE_LEFT_OPERATION, LIFECYCLE_RIGHT_OPERATION] {
            crate::apply_change_collaboration_operation(
                &mut state,
                super::super::collaboration_change::Operation {
                    id: operation_id.to_string(),
                    actor: DELEGATE.to_string(),
                    is_delegate: true,
                    actions: vec![super::super::collaboration_change::Action::SetLifecycle {
                        status:
                            super::super::radicle_change::ChangeLifecycleStatus::ReviewRequested,
                        subject_root: content_root.clone(),
                        evidence: std::collections::BTreeSet::new(),
                        basis: basis.clone(),
                    }],
                },
            )
            .unwrap();
        }
        let mut resolution = Snapshot {
            change_id: CHANGE_ID.to_string(),
            policy_hash: POLICY_HASH.to_string(),
            actor: DELEGATE.to_string(),
            is_delegate: true,
            artifact_selections: Vec::new(),
            lifecycle_status: None,
        };
        assert!(plan(&state, resolution.clone()).is_err());
        resolution.lifecycle_status =
            Some(super::super::radicle_change::ChangeLifecycleStatus::Approved);
        assert!(plan(&state, resolution.clone()).is_err());
        resolution.lifecycle_status =
            Some(super::super::radicle_change::ChangeLifecycleStatus::Open);
        let plan = plan(&state, resolution).unwrap();
        assert!(plan.lifecycle_reset);
        let resolved = apply(&state, &plan).unwrap();
        assert!(matches!(
            super::super::collaboration_change::change_collaboration_lifecycle_view(&resolved).unwrap(),
            super::super::radicle_change::ChangeLifecycleView::Current(super::super::radicle_change::ChangeLifecycleRecord {
                status: super::super::radicle_change::ChangeLifecycleStatus::Open,
                evidence,
                ..
            }) if evidence.is_empty()
        ));
    }

    // r[verify cairn.collaboration_resolution.plan.blocked]
    #[test]
    fn incomplete_duplicate_stale_unauthorized_and_drifted_input_fails_closed() {
        let state = conflicted_state();
        let mut missing = snapshot();
        missing.artifact_selections.pop();
        assert!(plan(&state, missing).is_err());

        let mut duplicate = snapshot();
        duplicate
            .artifact_selections
            .push(duplicate.artifact_selections[0].clone());
        assert!(plan(&state, duplicate).is_err());

        let mut stale = snapshot();
        stale.artifact_selections[0].revision_id = "operation:stale".to_string();
        assert!(plan(&state, stale).is_err());

        let mut unauthorized = snapshot();
        unauthorized.actor = OUTSIDER.to_string();
        assert!(matches!(
            plan(&state, unauthorized),
            Err(super::super::radicle_change::ChangeCobError::Unauthorized { .. })
        ));

        let mut stale_policy = snapshot();
        stale_policy.policy_hash = "c".repeat(crate::BLAKE3_HEX_LENGTH);
        assert!(plan(&state, stale_policy).is_err());
        assert!(plan(&initial_state(), snapshot()).is_err());

        let plan = plan(&state, snapshot()).unwrap();
        let mut drifted = state;
        drifted.policy_hash = "d".repeat(crate::BLAKE3_HEX_LENGTH);
        assert!(apply(&drifted, &plan).is_err());
    }
}
