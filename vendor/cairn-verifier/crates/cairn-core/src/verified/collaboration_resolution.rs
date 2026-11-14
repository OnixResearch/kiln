use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    collaboration_change::{
        action_into_compatibility, apply_change_collaboration_operation,
        change_collaboration_content_root, change_collaboration_lifecycle_view,
        change_collaboration_state_hash, state_from_compatibility,
        validate_change_collaboration_state, ChangeCollaborationAction,
        ChangeCollaborationOperation, ChangeCollaborationState,
    },
    evidence::canonical_json_hash,
    nominal_identity::{
        ActorId, Blake3Digest, ChangeId, ContentRootDigestRole, DigestRoleMarker, IdentityIssue,
        OperationId, PlanDigest, PolicyDigest, StateDigestRole, RAW_BLAKE3_PROFILE,
    },
    radicle_change::{
        ChangeArtifactId, ChangeCobAction, ChangeCobError, ChangeCobState, ChangeLifecycleRecord,
        ChangeLifecycleStatus, ChangeLifecycleView, MAX_ACTOR_ID_BYTES,
    },
};

const RESOLUTION_OPERATION_DOMAIN: &str = "cairn-change-collaboration-resolution-v1";
const RESOLUTION_SIMULATION_OPERATION: &str = "cairn-resolution-simulation";

// r[impl cairn.collaboration_resolution.inspection]
// r[impl cairn.collaboration_resolution.plan]

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeArtifactConflictHead {
    pub revision_id: String,
    pub actor: String,
    pub deleted: bool,
    pub content_blake3: Option<String>,
    pub content_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeArtifactConflict {
    pub artifact: ChangeArtifactId,
    pub heads: Vec<ChangeArtifactConflictHead>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeLifecycleConflictHead {
    pub revision_id: String,
    pub actor: String,
    pub record: ChangeLifecycleRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCollaborationConflictInspection {
    pub change_id: String,
    pub state_hash: String,
    pub artifact_conflicts: Vec<ChangeArtifactConflict>,
    pub lifecycle_heads: Vec<ChangeLifecycleConflictHead>,
}

impl ChangeCollaborationConflictInspection {
    pub fn has_conflicts(&self) -> bool {
        !self.artifact_conflicts.is_empty() || !self.lifecycle_heads.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeArtifactConflictValue {
    pub artifact: ChangeArtifactId,
    pub revision_id: String,
    pub actor: String,
    pub deleted: bool,
    pub content: Option<String>,
    pub content_blake3: Option<String>,
    pub content_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeArtifactHeadSelection {
    pub artifact: ChangeArtifactId,
    pub revision_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCollaborationResolutionSnapshot {
    pub change_id: String,
    pub policy_hash: String,
    pub actor: String,
    pub is_delegate: bool,
    pub artifact_selections: Vec<ChangeArtifactHeadSelection>,
    pub lifecycle_status: Option<ChangeLifecycleStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCollaborationResolutionPlan {
    pub change_id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub input_state_hash: String,
    pub operation_id: String,
    pub actions: Vec<ChangeCollaborationAction>,
    pub selected_artifacts: Vec<ChangeArtifactHeadSelection>,
    pub lifecycle_reset: bool,
    pub expected_state_hash: String,
    pub expected_content_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCobResolutionPlan {
    pub change_id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub input_state_hash: String,
    pub operation_id: String,
    pub actions: Vec<ChangeCobAction>,
    pub selected_artifacts: Vec<ChangeArtifactHeadSelection>,
    pub lifecycle_reset: bool,
    pub expected_content_root: String,
}

pub fn inspect_change_collaboration_conflicts(
    state: &ChangeCollaborationState,
) -> Result<ChangeCollaborationConflictInspection, ChangeCobError> {
    validate_change_collaboration_state(state)?;
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
    Ok(ChangeCollaborationConflictInspection {
        change_id: state.change_id.clone(),
        state_hash: change_collaboration_state_hash(state)?,
        artifact_conflicts,
        lifecycle_heads,
    })
}

pub fn inspect_change_cob_conflicts(
    state: &ChangeCobState,
) -> Result<ChangeCollaborationConflictInspection, ChangeCobError> {
    inspect_change_collaboration_conflicts(&state_from_compatibility(state.clone())?)
}

pub fn inspect_change_collaboration_conflict_value(
    state: &ChangeCollaborationState,
    artifact: &ChangeArtifactId,
    revision_id: &str,
) -> Result<ChangeArtifactConflictValue, ChangeCobError> {
    let inspection = inspect_change_collaboration_conflicts(state)?;
    let Some(conflict) = inspection
        .artifact_conflicts
        .iter()
        .find(|conflict| &conflict.artifact == artifact)
    else {
        return Err(invalid_resolution(
            "resolution.show.artifact",
            "selected artifact is not currently conflicted",
        ));
    };
    if !conflict
        .heads
        .iter()
        .any(|head| head.revision_id == revision_id)
    {
        return Err(invalid_resolution(
            "resolution.show.revision",
            "selected revision is not a current conflict head",
        ));
    }
    let history = state.artifacts.get(artifact).ok_or_else(|| {
        invalid_resolution(
            "resolution.show.artifact",
            "selected artifact history is missing",
        )
    })?;
    let revision = history.revisions.get(revision_id).ok_or_else(|| {
        invalid_resolution(
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
    state: &ChangeCobState,
    artifact: &ChangeArtifactId,
    revision_id: &str,
) -> Result<ChangeArtifactConflictValue, ChangeCobError> {
    inspect_change_collaboration_conflict_value(
        &state_from_compatibility(state.clone())?,
        artifact,
        revision_id,
    )
}

pub fn plan_change_collaboration_resolution(
    state: &ChangeCollaborationState,
    snapshot: ChangeCollaborationResolutionSnapshot,
) -> Result<ChangeCollaborationResolutionPlan, ChangeCobError> {
    validate_change_collaboration_state(state)?;
    validate_resolution_context(state, &snapshot)?;
    let inspection = inspect_change_collaboration_conflicts(state)?;
    if !inspection.has_conflicts() {
        return Err(invalid_resolution(
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
            invalid_resolution(
                "resolution.artifact",
                "selected artifact history is missing",
            )
        })?;
        let basis = history.head_ids();
        let selected_revision = basis
            .iter()
            .find(|revision| same_operation_id(revision, &selection.revision_id))
            .ok_or_else(|| {
                invalid_resolution(
                    "resolution.revision",
                    "selected artifact revision is not a current head",
                )
            })?;
        let revision = history.revisions.get(selected_revision).ok_or_else(|| {
            invalid_resolution(
                "resolution.revision",
                "selected artifact revision is missing",
            )
        })?;
        artifact_actions.push(ChangeCollaborationAction::SetArtifact {
            artifact: selection.artifact.clone(),
            content: revision.value.clone(),
            basis,
        });
    }

    let resolved_content_root = simulate_resolved_content(state, &snapshot, &artifact_actions)?;
    let lifecycle_reset = !inspection.lifecycle_heads.is_empty();
    let mut actions = artifact_actions;
    if lifecycle_reset {
        actions.push(ChangeCollaborationAction::SetLifecycle {
            status: ChangeLifecycleStatus::Open,
            subject_root: resolved_content_root,
            evidence: BTreeSet::new(),
            basis: state.lifecycle.head_ids(),
        });
    }
    let input_state_hash = inspection.state_hash;
    let operation_id = resolution_operation_id(
        &input_state_hash,
        &snapshot.actor,
        snapshot.is_delegate,
        &actions,
    )?;
    let mut simulated = state.clone();
    apply_change_collaboration_operation(
        &mut simulated,
        ChangeCollaborationOperation {
            id: operation_id.clone(),
            actor: snapshot.actor.clone(),
            is_delegate: snapshot.is_delegate,
            actions: actions.clone(),
        },
    )?;
    require_conflict_free_result(&simulated)?;
    Ok(ChangeCollaborationResolutionPlan {
        change_id: state.change_id.clone(),
        actor: snapshot.actor,
        is_delegate: snapshot.is_delegate,
        input_state_hash,
        operation_id,
        actions,
        selected_artifacts,
        lifecycle_reset,
        expected_state_hash: change_collaboration_state_hash(&simulated)?,
        expected_content_root: change_collaboration_content_root(&simulated)?,
    })
}

pub fn plan_change_cob_resolution(
    state: &ChangeCobState,
    snapshot: ChangeCollaborationResolutionSnapshot,
) -> Result<ChangeCobResolutionPlan, ChangeCobError> {
    let neutral = state_from_compatibility(state.clone())?;
    let plan = plan_change_collaboration_resolution(&neutral, snapshot)?;
    let actions = plan
        .actions
        .into_iter()
        .map(action_into_compatibility)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ChangeCobResolutionPlan {
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

pub fn apply_planned_change_collaboration_resolution(
    state: &ChangeCollaborationState,
    plan: &ChangeCollaborationResolutionPlan,
) -> Result<ChangeCollaborationState, ChangeCobError> {
    let current_state_hash = change_collaboration_state_hash(state)?;
    if !same_digest::<StateDigestRole>(&current_state_hash, &plan.input_state_hash) {
        return Err(invalid_resolution(
            "resolution.input_state_hash",
            "collaboration state changed after planning",
        ));
    }
    let mut resolved = state.clone();
    apply_change_collaboration_operation(
        &mut resolved,
        ChangeCollaborationOperation {
            id: plan.operation_id.clone(),
            actor: plan.actor.clone(),
            is_delegate: plan.is_delegate,
            actions: plan.actions.clone(),
        },
    )?;
    require_conflict_free_result(&resolved)?;
    let state_hash = change_collaboration_state_hash(&resolved)?;
    if !same_digest::<StateDigestRole>(&state_hash, &plan.expected_state_hash) {
        return Err(invalid_resolution(
            "resolution.expected_state_hash",
            "resolved state does not match the reviewed plan",
        ));
    }
    let content_root = change_collaboration_content_root(&resolved)?;
    if !same_digest::<ContentRootDigestRole>(&content_root, &plan.expected_content_root) {
        return Err(invalid_resolution(
            "resolution.expected_content_root",
            "resolved content does not match the reviewed plan",
        ));
    }
    Ok(resolved)
}

pub fn change_collaboration_resolution_plan_hash(
    plan: &ChangeCollaborationResolutionPlan,
) -> Result<String, ChangeCobError> {
    resolution_plan_hash(plan)
}

pub fn change_cob_resolution_plan_hash(
    plan: &ChangeCobResolutionPlan,
) -> Result<String, ChangeCobError> {
    resolution_plan_hash(plan)
}

fn resolution_plan_hash<T: Serialize>(plan: &T) -> Result<String, ChangeCobError> {
    let value = serde_json::to_value(plan).map_err(|error| ChangeCobError::Serialization {
        reason: error.to_string(),
    })?;
    let hash = canonical_json_hash(&value);
    let digest = PlanDigest::parse(&hash, RAW_BLAKE3_PROFILE, "resolution.plan_hash")
        .map_err(nominal_error)?;
    Ok(digest.as_hex().to_owned())
}

fn validate_resolution_context(
    state: &ChangeCollaborationState,
    snapshot: &ChangeCollaborationResolutionSnapshot,
) -> Result<(), ChangeCobError> {
    let state_change =
        ChangeId::parse(&state.change_id, "resolution.state_change_id").map_err(nominal_error)?;
    let snapshot_change = ChangeId::parse(&snapshot.change_id, "resolution.snapshot_change_id")
        .map_err(nominal_error)?;
    if state_change != snapshot_change {
        return Err(invalid_resolution(
            "resolution.change_id",
            "snapshot and collaboration state name different changes",
        ));
    }
    let state_policy = PolicyDigest::parse_compatible(
        &state.policy_hash,
        RAW_BLAKE3_PROFILE,
        "resolution.state_policy_hash",
    )
    .map_err(nominal_error)?;
    let snapshot_policy = PolicyDigest::parse_compatible(
        &snapshot.policy_hash,
        RAW_BLAKE3_PROFILE,
        "resolution.snapshot_policy_hash",
    )
    .map_err(nominal_error)?;
    if state_policy != snapshot_policy {
        return Err(invalid_resolution(
            "resolution.policy_hash",
            "current policy does not match collaboration state",
        ));
    }
    let actor_bytes = snapshot.actor.len();
    if actor_bytes > MAX_ACTOR_ID_BYTES {
        return Err(ChangeCobError::ByteLimitExceeded {
            field: "actor".to_string(),
            actual: actor_bytes,
            limit: MAX_ACTOR_ID_BYTES,
        });
    }
    let actor = ActorId::parse(&snapshot.actor, "resolution.actor").map_err(nominal_error)?;
    let creator = ActorId::parse(&state.creator, "resolution.creator").map_err(nominal_error)?;
    if actor != creator && !snapshot.is_delegate {
        return Err(ChangeCobError::Unauthorized {
            actor: snapshot.actor.clone(),
            action: "plan_collaboration_resolution".to_string(),
        });
    }
    Ok(())
}

fn unique_selections(
    selections: &[ChangeArtifactHeadSelection],
) -> Result<BTreeMap<ChangeArtifactId, ChangeArtifactHeadSelection>, ChangeCobError> {
    let mut unique = BTreeMap::new();
    for selection in selections {
        if unique
            .insert(selection.artifact.clone(), selection.clone())
            .is_some()
        {
            return Err(invalid_resolution(
                "resolution.selections",
                "an artifact has more than one selected head",
            ));
        }
    }
    Ok(unique)
}

fn validate_complete_selections(
    inspection: &ChangeCollaborationConflictInspection,
    selections: &BTreeMap<ChangeArtifactId, ChangeArtifactHeadSelection>,
) -> Result<(), ChangeCobError> {
    let conflicted = inspection
        .artifact_conflicts
        .iter()
        .map(|conflict| conflict.artifact.clone())
        .collect::<BTreeSet<_>>();
    let selected = selections.keys().cloned().collect::<BTreeSet<_>>();
    if conflicted != selected {
        let missing = conflicted
            .difference(&selected)
            .cloned()
            .collect::<Vec<_>>();
        let extra = selected
            .difference(&conflicted)
            .cloned()
            .collect::<Vec<_>>();
        return Err(invalid_resolution(
            "resolution.selections",
            format!("selections must cover exactly every conflict; missing {missing:?}, extra {extra:?}"),
        ));
    }
    for conflict in &inspection.artifact_conflicts {
        let selection = selections.get(&conflict.artifact).ok_or_else(|| {
            invalid_resolution(
                "resolution.selections",
                "conflicted artifact selection is missing",
            )
        })?;
        if !conflict
            .heads
            .iter()
            .any(|head| same_operation_id(&head.revision_id, &selection.revision_id))
        {
            return Err(invalid_resolution(
                "resolution.revision",
                "selected artifact revision is not a current conflict head",
            ));
        }
    }
    Ok(())
}

fn validate_lifecycle_selection(
    inspection: &ChangeCollaborationConflictInspection,
    lifecycle_status: Option<ChangeLifecycleStatus>,
) -> Result<(), ChangeCobError> {
    if inspection.lifecycle_heads.is_empty() {
        if lifecycle_status.is_some() {
            return Err(invalid_resolution(
                "resolution.lifecycle",
                "lifecycle selection was provided without a lifecycle conflict",
            ));
        }
        return Ok(());
    }
    match lifecycle_status {
        Some(ChangeLifecycleStatus::Open) => Ok(()),
        Some(_) => Err(invalid_resolution(
            "resolution.lifecycle",
            "conflicted lifecycle may only be reset explicitly to open",
        )),
        None => Err(invalid_resolution(
            "resolution.lifecycle",
            "lifecycle conflict requires explicit open reset",
        )),
    }
}

fn simulate_resolved_content(
    state: &ChangeCollaborationState,
    snapshot: &ChangeCollaborationResolutionSnapshot,
    artifact_actions: &[ChangeCollaborationAction],
) -> Result<String, ChangeCobError> {
    if artifact_actions.is_empty() {
        return change_collaboration_content_root(state);
    }
    let mut simulated = state.clone();
    apply_change_collaboration_operation(
        &mut simulated,
        ChangeCollaborationOperation {
            id: RESOLUTION_SIMULATION_OPERATION.to_string(),
            actor: snapshot.actor.clone(),
            is_delegate: snapshot.is_delegate,
            actions: artifact_actions.to_vec(),
        },
    )?;
    change_collaboration_content_root(&simulated)
}

fn require_conflict_free_result(state: &ChangeCollaborationState) -> Result<(), ChangeCobError> {
    let _content_root = change_collaboration_content_root(state)?;
    match change_collaboration_lifecycle_view(state)? {
        ChangeLifecycleView::Conflicted { revisions } => Err(invalid_resolution(
            "resolution.result.lifecycle",
            format!("resolution left lifecycle heads conflicted: {revisions:?}"),
        )),
        ChangeLifecycleView::ContentConflicted { artifacts } => {
            Err(ChangeCobError::ArtifactConflict { artifacts })
        }
        ChangeLifecycleView::Current(_) | ChangeLifecycleView::Stale { .. } => Ok(()),
    }
}

fn same_operation_id(left: &str, right: &str) -> bool {
    let left = OperationId::parse(left, "resolution.left_operation_id");
    let right = OperationId::parse(right, "resolution.right_operation_id");
    matches!((left, right), (Ok(left), Ok(right)) if left == right)
}

fn same_digest<Role: DigestRoleMarker>(left: &str, right: &str) -> bool {
    let left =
        Blake3Digest::<Role>::parse_compatible(left, RAW_BLAKE3_PROFILE, "resolution.left_digest");
    let right = Blake3Digest::<Role>::parse_compatible(
        right,
        RAW_BLAKE3_PROFILE,
        "resolution.right_digest",
    );
    matches!(
        (left, right),
        (Ok(left), Ok(right)) if left.as_hex() == right.as_hex()
    )
}

fn nominal_error(issue: IdentityIssue) -> ChangeCobError {
    ChangeCobError::InvalidField {
        field: issue.field.to_owned(),
        reason: issue.code.to_owned(),
    }
}

fn resolution_operation_id(
    input_state_hash: &str,
    actor: &str,
    is_delegate: bool,
    actions: &[ChangeCollaborationAction],
) -> Result<String, ChangeCobError> {
    let value = serde_json::to_value(ResolutionOperationIdentity {
        domain: RESOLUTION_OPERATION_DOMAIN,
        input_state_hash,
        actor,
        is_delegate,
        actions,
    })
    .map_err(|error| ChangeCobError::Serialization {
        reason: error.to_string(),
    })?;
    let hash = canonical_json_hash(&value);
    let operation = OperationId::parse(&hash, "resolution.operation_id").map_err(nominal_error)?;
    Ok(operation.as_str().to_owned())
}

fn invalid_resolution(field: &str, reason: impl Into<String>) -> ChangeCobError {
    ChangeCobError::InvalidField {
        field: field.to_string(),
        reason: reason.into(),
    }
}

#[derive(Serialize)]
struct ResolutionOperationIdentity<'a> {
    domain: &'a str,
    input_state_hash: &'a str,
    actor: &'a str,
    is_delegate: bool,
    actions: &'a [ChangeCollaborationAction],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apply_change_collaboration_operation, initialize_change_collaboration_import,
        plan_change_collaboration_import, ChangeBaseRevision, ChangeCollaborationImportSnapshot,
    };

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

    fn initial_state() -> ChangeCollaborationState {
        let init = plan_change_collaboration_import(ChangeCollaborationImportSnapshot {
            change_id: CHANGE_ID.to_string(),
            workspace_id: WORKSPACE_ID.to_string(),
            base_revision: ChangeBaseRevision::new("opaque", "revision-one").unwrap(),
            policy_hash: POLICY_HASH.to_string(),
            proposal: Some("# Proposal\n".to_string()),
            design: Some("# Design\n".to_string()),
            tasks: None,
            metadata: None,
            delta_specs: BTreeMap::new(),
        })
        .unwrap();
        initialize_change_collaboration_import(init, CREATOR).unwrap()
    }

    fn add_artifact_conflict(
        state: &mut ChangeCollaborationState,
        artifact: ChangeArtifactId,
        operations: [(&str, Option<&str>); CONCURRENT_HEAD_COUNT],
    ) {
        let basis = state.artifacts.get(&artifact).unwrap().head_ids();
        for (operation_id, content) in operations {
            apply_change_collaboration_operation(
                state,
                ChangeCollaborationOperation {
                    id: operation_id.to_string(),
                    actor: CREATOR.to_string(),
                    is_delegate: false,
                    actions: vec![ChangeCollaborationAction::SetArtifact {
                        artifact: artifact.clone(),
                        content: content.map(str::to_string),
                        basis: basis.clone(),
                    }],
                },
            )
            .unwrap();
        }
    }

    fn conflicted_state() -> ChangeCollaborationState {
        let mut state = initial_state();
        add_artifact_conflict(
            &mut state,
            ChangeArtifactId::proposal(),
            [
                (LEFT_OPERATION, Some("# Left proposal\n")),
                (RIGHT_OPERATION, Some("# Right proposal\n")),
            ],
        );
        add_artifact_conflict(
            &mut state,
            ChangeArtifactId::design(),
            [
                (LEFT_DESIGN_OPERATION, Some("# Left design\n")),
                (RIGHT_DESIGN_OPERATION, None),
            ],
        );
        state
    }

    fn snapshot() -> ChangeCollaborationResolutionSnapshot {
        ChangeCollaborationResolutionSnapshot {
            change_id: CHANGE_ID.to_string(),
            policy_hash: POLICY_HASH.to_string(),
            actor: CREATOR.to_string(),
            is_delegate: false,
            artifact_selections: vec![
                ChangeArtifactHeadSelection {
                    artifact: ChangeArtifactId::proposal(),
                    revision_id: LEFT_OPERATION.to_string(),
                },
                ChangeArtifactHeadSelection {
                    artifact: ChangeArtifactId::design(),
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
        let inspection = inspect_change_collaboration_conflicts(&state).unwrap();
        assert_eq!(inspection.artifact_conflicts.len(), CONFLICT_ARTIFACT_COUNT);
        assert!(inspection
            .artifact_conflicts
            .iter()
            .all(|conflict| conflict.heads.len() == CONCURRENT_HEAD_COUNT));
        let shown = inspect_change_collaboration_conflict_value(
            &state,
            &ChangeArtifactId::proposal(),
            LEFT_OPERATION,
        )
        .unwrap();
        assert_eq!(shown.content.as_deref(), Some("# Left proposal\n"));
        assert!(inspect_change_collaboration_conflict_value(
            &state,
            &ChangeArtifactId::tasks(),
            LEFT_OPERATION,
        )
        .is_err());
    }

    // r[verify cairn.collaboration_resolution.plan.complete]
    #[test]
    fn complete_selection_supersedes_all_heads_and_preserves_history() {
        let state = conflicted_state();
        let first = plan_change_collaboration_resolution(&state, snapshot()).unwrap();
        let second = plan_change_collaboration_resolution(&state, snapshot()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.actions.len(), RESOLUTION_ACTION_COUNT);
        assert_eq!(
            first.actions[0],
            ChangeCollaborationAction::SetArtifact {
                artifact: ChangeArtifactId::design(),
                content: None,
                basis: BTreeSet::from([
                    LEFT_DESIGN_OPERATION.to_string(),
                    RIGHT_DESIGN_OPERATION.to_string(),
                ]),
            }
        );
        let resolved = apply_planned_change_collaboration_resolution(&state, &first).unwrap();
        assert_eq!(
            change_collaboration_state_hash(&resolved).unwrap(),
            first.expected_state_hash
        );
        assert_eq!(
            change_collaboration_content_root(&resolved).unwrap(),
            first.expected_content_root
        );
        assert_eq!(
            resolved.artifacts[&ChangeArtifactId::proposal()]
                .revisions
                .len(),
            RESOLVED_PROPOSAL_REVISION_COUNT
        );
        assert!(!inspect_change_collaboration_conflicts(&resolved)
            .unwrap()
            .has_conflicts());
    }

    // r[verify cairn.collaboration_resolution.plan.lifecycle]
    #[test]
    fn lifecycle_conflict_requires_explicit_open_reset_over_every_head() {
        let mut state = initial_state();
        let basis = state.lifecycle.head_ids();
        let content_root = change_collaboration_content_root(&state).unwrap();
        for operation_id in [LIFECYCLE_LEFT_OPERATION, LIFECYCLE_RIGHT_OPERATION] {
            apply_change_collaboration_operation(
                &mut state,
                ChangeCollaborationOperation {
                    id: operation_id.to_string(),
                    actor: DELEGATE.to_string(),
                    is_delegate: true,
                    actions: vec![ChangeCollaborationAction::SetLifecycle {
                        status: ChangeLifecycleStatus::ReviewRequested,
                        subject_root: content_root.clone(),
                        evidence: BTreeSet::new(),
                        basis: basis.clone(),
                    }],
                },
            )
            .unwrap();
        }
        let mut resolution = ChangeCollaborationResolutionSnapshot {
            change_id: CHANGE_ID.to_string(),
            policy_hash: POLICY_HASH.to_string(),
            actor: DELEGATE.to_string(),
            is_delegate: true,
            artifact_selections: Vec::new(),
            lifecycle_status: None,
        };
        assert!(plan_change_collaboration_resolution(&state, resolution.clone()).is_err());
        resolution.lifecycle_status = Some(ChangeLifecycleStatus::Approved);
        assert!(plan_change_collaboration_resolution(&state, resolution.clone()).is_err());
        resolution.lifecycle_status = Some(ChangeLifecycleStatus::Open);
        let plan = plan_change_collaboration_resolution(&state, resolution).unwrap();
        assert!(plan.lifecycle_reset);
        let resolved = apply_planned_change_collaboration_resolution(&state, &plan).unwrap();
        assert!(matches!(
            change_collaboration_lifecycle_view(&resolved).unwrap(),
            ChangeLifecycleView::Current(ChangeLifecycleRecord {
                status: ChangeLifecycleStatus::Open,
                evidence,
                ..
            }) if evidence.is_empty()
        ));
    }

    // r[verify cairn.collaboration_resolution.plan.blocked]
    #[test]
    fn incomplete_duplicate_stale_unauthorized_and_drifted_resolution_fail_closed() {
        let state = conflicted_state();
        let mut missing = snapshot();
        missing.artifact_selections.pop();
        assert!(plan_change_collaboration_resolution(&state, missing).is_err());

        let mut duplicate = snapshot();
        duplicate
            .artifact_selections
            .push(duplicate.artifact_selections[0].clone());
        assert!(plan_change_collaboration_resolution(&state, duplicate).is_err());

        let mut stale = snapshot();
        stale.artifact_selections[0].revision_id = "operation:stale".to_string();
        assert!(plan_change_collaboration_resolution(&state, stale).is_err());

        let mut unauthorized = snapshot();
        unauthorized.actor = OUTSIDER.to_string();
        assert!(matches!(
            plan_change_collaboration_resolution(&state, unauthorized),
            Err(ChangeCobError::Unauthorized { .. })
        ));

        let mut stale_policy = snapshot();
        stale_policy.policy_hash = "c".repeat(crate::BLAKE3_HEX_LENGTH);
        assert!(plan_change_collaboration_resolution(&state, stale_policy).is_err());
        assert!(plan_change_collaboration_resolution(&initial_state(), snapshot()).is_err());

        let plan = plan_change_collaboration_resolution(&state, snapshot()).unwrap();
        let mut drifted = state;
        drifted.policy_hash = "d".repeat(crate::BLAKE3_HEX_LENGTH);
        assert!(apply_planned_change_collaboration_resolution(&drifted, &plan).is_err());
    }
}
