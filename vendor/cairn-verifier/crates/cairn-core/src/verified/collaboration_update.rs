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
        PlanDigest, PolicyDigest, StateDigestRole, RAW_BLAKE3_PROFILE,
    },
    radicle_change::{
        ChangeArtifactId, ChangeCobAction, ChangeCobError, ChangeCobState, ChangeLifecycleView,
        MAX_ACTOR_ID_BYTES,
    },
};

const UPDATE_OPERATION_DOMAIN: &str = "cairn-change-collaboration-update-v1";

// r[impl cairn.collaboration_update.plan]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCollaborationUpdateSnapshot {
    pub change_id: String,
    pub policy_hash: String,
    pub actor: String,
    pub is_delegate: bool,
    pub proposal: Option<String>,
    pub design: Option<String>,
    pub tasks: Option<String>,
    pub metadata: Option<String>,
    pub delta_specs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCollaborationUpdatePlan {
    pub change_id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub input_state_hash: String,
    pub input_content_root: String,
    pub operation_id: String,
    pub actions: Vec<ChangeCollaborationAction>,
    pub changed_artifacts: Vec<String>,
    pub expected_content_root: String,
}

impl ChangeCollaborationUpdatePlan {
    pub fn is_noop(&self) -> bool {
        self.actions.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCobUpdatePlan {
    pub change_id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub input_state_hash: String,
    pub input_content_root: String,
    pub operation_id: String,
    pub actions: Vec<ChangeCobAction>,
    pub changed_artifacts: Vec<String>,
    pub expected_content_root: String,
}

impl ChangeCobUpdatePlan {
    pub fn is_noop(&self) -> bool {
        self.actions.is_empty()
    }
}

pub fn plan_change_collaboration_update(
    state: &ChangeCollaborationState,
    snapshot: ChangeCollaborationUpdateSnapshot,
) -> Result<ChangeCollaborationUpdatePlan, ChangeCobError> {
    validate_change_collaboration_state(state)?;
    validate_snapshot_context(state, &snapshot)?;
    let input_state_hash = change_collaboration_state_hash(state)?;
    let input_content_root = require_conflict_free_state(state)?;
    let desired = desired_artifacts(&snapshot)?;
    let actions = artifact_update_actions(state, desired)?;
    let operation_id = update_operation_id(
        &input_state_hash,
        &snapshot.actor,
        snapshot.is_delegate,
        &actions,
    )?;
    let changed_artifacts = actions
        .iter()
        .filter_map(|action| match action {
            ChangeCollaborationAction::SetArtifact { artifact, .. } => {
                Some(artifact.as_str().to_string())
            }
            ChangeCollaborationAction::Init { .. }
            | ChangeCollaborationAction::AttachEvidence { .. }
            | ChangeCollaborationAction::SetLifecycle { .. } => None,
        })
        .collect::<Vec<_>>();
    let mut simulated = state.clone();
    if !actions.is_empty() {
        apply_change_collaboration_operation(
            &mut simulated,
            ChangeCollaborationOperation {
                id: operation_id.clone(),
                actor: snapshot.actor.clone(),
                is_delegate: snapshot.is_delegate,
                actions: actions.clone(),
            },
        )?;
    }
    let expected_content_root = change_collaboration_content_root(&simulated)?;
    Ok(ChangeCollaborationUpdatePlan {
        change_id: state.change_id.clone(),
        actor: snapshot.actor,
        is_delegate: snapshot.is_delegate,
        input_state_hash,
        input_content_root,
        operation_id,
        actions,
        changed_artifacts,
        expected_content_root,
    })
}

pub fn plan_change_cob_update(
    state: &ChangeCobState,
    snapshot: ChangeCollaborationUpdateSnapshot,
) -> Result<ChangeCobUpdatePlan, ChangeCobError> {
    let neutral = state_from_compatibility(state.clone())?;
    let plan = plan_change_collaboration_update(&neutral, snapshot)?;
    let actions = plan
        .actions
        .into_iter()
        .map(action_into_compatibility)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ChangeCobUpdatePlan {
        change_id: plan.change_id,
        actor: plan.actor,
        is_delegate: plan.is_delegate,
        input_state_hash: plan.input_state_hash,
        input_content_root: plan.input_content_root,
        operation_id: plan.operation_id,
        actions,
        changed_artifacts: plan.changed_artifacts,
        expected_content_root: plan.expected_content_root,
    })
}

pub fn apply_planned_change_collaboration_update(
    state: &ChangeCollaborationState,
    plan: &ChangeCollaborationUpdatePlan,
) -> Result<ChangeCollaborationState, ChangeCobError> {
    let current_state_hash = change_collaboration_state_hash(state)?;
    if !same_digest::<StateDigestRole>(&current_state_hash, &plan.input_state_hash) {
        return Err(ChangeCobError::InvalidField {
            field: "update.input_state_hash".to_string(),
            reason: "collaboration state changed after planning".to_string(),
        });
    }
    if plan.actions.is_empty() {
        return Ok(state.clone());
    }
    let mut updated = state.clone();
    apply_change_collaboration_operation(
        &mut updated,
        ChangeCollaborationOperation {
            id: plan.operation_id.clone(),
            actor: plan.actor.clone(),
            is_delegate: plan.is_delegate,
            actions: plan.actions.clone(),
        },
    )?;
    let content_root = change_collaboration_content_root(&updated)?;
    if !same_digest::<ContentRootDigestRole>(&content_root, &plan.expected_content_root) {
        return Err(ChangeCobError::InvalidField {
            field: "update.expected_content_root".to_string(),
            reason: "applied update content does not match the reviewed plan".to_string(),
        });
    }
    Ok(updated)
}

pub fn change_collaboration_update_plan_hash(
    plan: &ChangeCollaborationUpdatePlan,
) -> Result<String, ChangeCobError> {
    update_plan_hash(plan)
}

pub fn change_cob_update_plan_hash(plan: &ChangeCobUpdatePlan) -> Result<String, ChangeCobError> {
    update_plan_hash(plan)
}

fn update_plan_hash<T: Serialize>(plan: &T) -> Result<String, ChangeCobError> {
    let value = serde_json::to_value(plan).map_err(|error| ChangeCobError::Serialization {
        reason: error.to_string(),
    })?;
    let hash = canonical_json_hash(&value);
    let digest =
        PlanDigest::parse(&hash, RAW_BLAKE3_PROFILE, "update.plan_hash").map_err(nominal_error)?;
    Ok(digest.as_hex().to_owned())
}

fn validate_snapshot_context(
    state: &ChangeCollaborationState,
    snapshot: &ChangeCollaborationUpdateSnapshot,
) -> Result<(), ChangeCobError> {
    let state_change =
        ChangeId::parse(&state.change_id, "update.state_change_id").map_err(nominal_error)?;
    let snapshot_change =
        ChangeId::parse(&snapshot.change_id, "update.snapshot_change_id").map_err(nominal_error)?;
    if state_change != snapshot_change {
        return Err(ChangeCobError::InvalidField {
            field: "update.change_id".to_string(),
            reason: format!(
                "snapshot names {}, state names {}",
                snapshot.change_id, state.change_id
            ),
        });
    }
    let state_policy = PolicyDigest::parse_compatible(
        &state.policy_hash,
        RAW_BLAKE3_PROFILE,
        "update.state_policy_hash",
    )
    .map_err(nominal_error)?;
    let snapshot_policy = PolicyDigest::parse_compatible(
        &snapshot.policy_hash,
        RAW_BLAKE3_PROFILE,
        "update.snapshot_policy_hash",
    )
    .map_err(nominal_error)?;
    if state_policy != snapshot_policy {
        return Err(ChangeCobError::InvalidField {
            field: "update.policy_hash".to_string(),
            reason: "current policy does not match collaboration state".to_string(),
        });
    }
    let actor_bytes = snapshot.actor.len();
    if actor_bytes > MAX_ACTOR_ID_BYTES {
        return Err(ChangeCobError::ByteLimitExceeded {
            field: "actor".to_string(),
            actual: actor_bytes,
            limit: MAX_ACTOR_ID_BYTES,
        });
    }
    let actor = ActorId::parse(&snapshot.actor, "update.actor").map_err(nominal_error)?;
    let creator = ActorId::parse(&state.creator, "update.creator").map_err(nominal_error)?;
    if actor != creator && !snapshot.is_delegate {
        return Err(ChangeCobError::Unauthorized {
            actor: snapshot.actor.clone(),
            action: "plan_collaboration_update".to_string(),
        });
    }
    Ok(())
}

fn require_conflict_free_state(state: &ChangeCollaborationState) -> Result<String, ChangeCobError> {
    let content_root = change_collaboration_content_root(state)?;
    match change_collaboration_lifecycle_view(state)? {
        ChangeLifecycleView::Conflicted { revisions } => Err(ChangeCobError::InvalidField {
            field: "update.lifecycle".to_string(),
            reason: format!("lifecycle has conflicting revisions: {revisions:?}"),
        }),
        ChangeLifecycleView::ContentConflicted { artifacts } => {
            Err(ChangeCobError::ArtifactConflict { artifacts })
        }
        ChangeLifecycleView::Current(_) | ChangeLifecycleView::Stale { .. } => Ok(content_root),
    }
}

fn desired_artifacts(
    snapshot: &ChangeCollaborationUpdateSnapshot,
) -> Result<BTreeMap<ChangeArtifactId, String>, ChangeCobError> {
    let mut artifacts = BTreeMap::new();
    insert_present_artifact(
        &mut artifacts,
        ChangeArtifactId::proposal(),
        snapshot.proposal.clone(),
    );
    insert_present_artifact(
        &mut artifacts,
        ChangeArtifactId::design(),
        snapshot.design.clone(),
    );
    insert_present_artifact(
        &mut artifacts,
        ChangeArtifactId::tasks(),
        snapshot.tasks.clone(),
    );
    insert_present_artifact(
        &mut artifacts,
        ChangeArtifactId::metadata(),
        snapshot.metadata.clone(),
    );
    for (spec_id, content) in &snapshot.delta_specs {
        artifacts.insert(ChangeArtifactId::delta_spec(spec_id)?, content.clone());
    }
    Ok(artifacts)
}

fn insert_present_artifact(
    artifacts: &mut BTreeMap<ChangeArtifactId, String>,
    artifact: ChangeArtifactId,
    content: Option<String>,
) {
    if let Some(content) = content {
        artifacts.insert(artifact, content);
    }
}

fn artifact_update_actions(
    state: &ChangeCollaborationState,
    desired: BTreeMap<ChangeArtifactId, String>,
) -> Result<Vec<ChangeCollaborationAction>, ChangeCobError> {
    let artifact_ids = state
        .artifacts
        .keys()
        .chain(desired.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut actions = Vec::new();
    for artifact in artifact_ids {
        let (current, basis) = match state.artifacts.get(&artifact) {
            Some(history) => {
                let heads = history.heads();
                if heads.len() != 1 {
                    return Err(ChangeCobError::ArtifactConflict {
                        artifacts: vec![artifact.as_str().to_string()],
                    });
                }
                let Some((_, revision)) = heads.first() else {
                    return Err(ChangeCobError::ArtifactConflict {
                        artifacts: vec![artifact.as_str().to_string()],
                    });
                };
                (revision.value.clone(), history.head_ids())
            }
            None => (None, BTreeSet::new()),
        };
        let next = desired.get(&artifact).cloned();
        if current != next {
            actions.push(ChangeCollaborationAction::SetArtifact {
                artifact,
                content: next,
                basis,
            });
        }
    }
    Ok(actions)
}

fn same_digest<Role: DigestRoleMarker>(left: &str, right: &str) -> bool {
    let left =
        Blake3Digest::<Role>::parse_compatible(left, RAW_BLAKE3_PROFILE, "update.left_digest");
    let right =
        Blake3Digest::<Role>::parse_compatible(right, RAW_BLAKE3_PROFILE, "update.right_digest");
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

fn update_operation_id(
    input_state_hash: &str,
    actor: &str,
    is_delegate: bool,
    actions: &[ChangeCollaborationAction],
) -> Result<String, ChangeCobError> {
    let identity = UpdateOperationIdentity {
        domain: UPDATE_OPERATION_DOMAIN,
        input_state_hash,
        actor,
        is_delegate,
        actions,
    };
    let value = serde_json::to_value(identity).map_err(|error| ChangeCobError::Serialization {
        reason: error.to_string(),
    })?;
    Ok(canonical_json_hash(&value))
}

#[derive(Serialize)]
struct UpdateOperationIdentity<'a> {
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
        materialize_change_collaboration, plan_change_collaboration_import, ChangeBaseRevision,
        ChangeCollaborationImportSnapshot, ChangeLifecycleStatus, MAX_ARTIFACT_BYTES,
    };

    const CHANGE_ID: &str = "collaboration-update";
    const WORKSPACE_ID: &str = "workspace:update";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const CREATOR: &str = "actor:creator";
    const DELEGATE: &str = "actor:delegate";
    const OUTSIDER: &str = "actor:outsider";
    const EDIT_OPERATION: &str = "edit-operation";
    const CONCURRENT_OPERATION: &str = "concurrent-operation";

    fn state() -> ChangeCollaborationState {
        let init = plan_change_collaboration_import(ChangeCollaborationImportSnapshot {
            change_id: CHANGE_ID.to_string(),
            workspace_id: WORKSPACE_ID.to_string(),
            base_revision: ChangeBaseRevision::new("opaque", "revision-one").unwrap(),
            policy_hash: POLICY_HASH.to_string(),
            proposal: Some("# Proposal\n".to_string()),
            design: None,
            tasks: None,
            metadata: None,
            delta_specs: BTreeMap::from([(CHANGE_ID.to_string(), "# Delta\n".to_string())]),
        })
        .unwrap();
        initialize_change_collaboration_import(init, CREATOR).unwrap()
    }

    fn snapshot(actor: &str) -> ChangeCollaborationUpdateSnapshot {
        ChangeCollaborationUpdateSnapshot {
            change_id: CHANGE_ID.to_string(),
            policy_hash: POLICY_HASH.to_string(),
            actor: actor.to_string(),
            is_delegate: actor == DELEGATE,
            proposal: Some("# Updated proposal\n".to_string()),
            design: Some("# Design\n".to_string()),
            tasks: None,
            metadata: None,
            delta_specs: BTreeMap::new(),
        }
    }

    // r[verify cairn.collaboration_update.plan.exact]
    #[test]
    fn plans_and_applies_sorted_add_change_remove_actions() {
        let state = state();
        let first = plan_change_collaboration_update(&state, snapshot(CREATOR)).unwrap();
        let second = plan_change_collaboration_update(&state, snapshot(CREATOR)).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.changed_artifacts,
            vec![
                "design".to_string(),
                "proposal".to_string(),
                format!("spec:{CHANGE_ID}"),
            ]
        );
        assert_eq!(
            change_collaboration_update_plan_hash(&first).unwrap(),
            change_collaboration_update_plan_hash(&second).unwrap()
        );
        let updated = apply_planned_change_collaboration_update(&state, &first).unwrap();
        assert_eq!(
            change_collaboration_content_root(&updated).unwrap(),
            first.expected_content_root
        );
        let files = materialize_change_collaboration(&updated).unwrap();
        assert!(files
            .iter()
            .any(|file| file.path == "proposal.md" && file.content == "# Updated proposal\n"));
        assert!(files
            .iter()
            .any(|file| file.path == "design.md" && file.content == "# Design\n"));
        assert!(!files.iter().any(|file| file.path.starts_with("specs/")));
    }

    // r[verify cairn.collaboration_update.plan.noop]
    #[test]
    fn byte_equivalent_update_is_noop() {
        let state = state();
        let plan = plan_change_collaboration_update(
            &state,
            ChangeCollaborationUpdateSnapshot {
                change_id: CHANGE_ID.to_string(),
                policy_hash: POLICY_HASH.to_string(),
                actor: CREATOR.to_string(),
                is_delegate: false,
                proposal: Some("# Proposal\n".to_string()),
                design: None,
                tasks: None,
                metadata: None,
                delta_specs: BTreeMap::from([(CHANGE_ID.to_string(), "# Delta\n".to_string())]),
            },
        )
        .unwrap();
        assert!(plan.is_noop());
        assert_eq!(plan.input_content_root, plan.expected_content_root);
        assert_eq!(
            apply_planned_change_collaboration_update(&state, &plan).unwrap(),
            state
        );
    }

    // r[verify cairn.collaboration_update.plan.blocked]
    #[test]
    fn rejects_wrong_identity_policy_actor_and_protocol_inputs() {
        let state = state();
        let mut wrong_change = snapshot(CREATOR);
        wrong_change.change_id = "other-change".to_string();
        assert!(plan_change_collaboration_update(&state, wrong_change).is_err());

        let mut stale_policy = snapshot(CREATOR);
        stale_policy.policy_hash = "c".repeat(crate::BLAKE3_HEX_LENGTH);
        assert!(plan_change_collaboration_update(&state, stale_policy).is_err());

        assert!(matches!(
            plan_change_collaboration_update(&state, snapshot(OUTSIDER)),
            Err(ChangeCobError::Unauthorized { .. })
        ));

        let mut malformed_spec = snapshot(CREATOR);
        malformed_spec
            .delta_specs
            .insert("../escape".to_string(), "# Bad\n".to_string());
        assert!(plan_change_collaboration_update(&state, malformed_spec).is_err());

        let mut oversized = snapshot(CREATOR);
        oversized.design = Some("x".repeat(MAX_ARTIFACT_BYTES.saturating_add(1)));
        assert!(matches!(
            plan_change_collaboration_update(&state, oversized),
            Err(ChangeCobError::ByteLimitExceeded { .. })
        ));
    }

    // r[verify cairn.collaboration_update.plan.blocked]
    #[test]
    fn rejects_content_and_lifecycle_conflicts() {
        let mut content_conflict = state();
        let root = content_conflict.operations.iter().next().unwrap().clone();
        for (id, content) in [
            (EDIT_OPERATION, "# Edit one\n"),
            (CONCURRENT_OPERATION, "# Edit two\n"),
        ] {
            apply_change_collaboration_operation(
                &mut content_conflict,
                ChangeCollaborationOperation {
                    id: id.to_string(),
                    actor: CREATOR.to_string(),
                    is_delegate: false,
                    actions: vec![ChangeCollaborationAction::SetArtifact {
                        artifact: ChangeArtifactId::proposal(),
                        content: Some(content.to_string()),
                        basis: BTreeSet::from([root.clone()]),
                    }],
                },
            )
            .unwrap();
        }
        assert!(matches!(
            plan_change_collaboration_update(&content_conflict, snapshot(CREATOR)),
            Err(ChangeCobError::ArtifactConflict { .. })
        ));

        let mut lifecycle_conflict = state();
        let root = lifecycle_conflict.operations.iter().next().unwrap().clone();
        let subject_root = change_collaboration_content_root(&lifecycle_conflict).unwrap();
        for (id, status) in [
            (EDIT_OPERATION, ChangeLifecycleStatus::Open),
            (CONCURRENT_OPERATION, ChangeLifecycleStatus::Open),
        ] {
            apply_change_collaboration_operation(
                &mut lifecycle_conflict,
                ChangeCollaborationOperation {
                    id: id.to_string(),
                    actor: DELEGATE.to_string(),
                    is_delegate: true,
                    actions: vec![ChangeCollaborationAction::SetLifecycle {
                        status,
                        subject_root: subject_root.clone(),
                        evidence: BTreeSet::new(),
                        basis: BTreeSet::from([root.clone()]),
                    }],
                },
            )
            .unwrap();
        }
        assert!(matches!(
            plan_change_collaboration_update(&lifecycle_conflict, snapshot(DELEGATE)),
            Err(ChangeCobError::InvalidField { field, .. }) if field == "update.lifecycle"
        ));
    }
}
