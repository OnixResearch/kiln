const UPDATE_OPERATION_DOMAIN: &str = "cairn-change-collaboration-update-v1";

// r[impl cairn.collaboration_update.plan]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub change_id: String,
    pub policy_hash: String,
    pub actor: String,
    pub is_delegate: bool,
    pub proposal: Option<String>,
    pub design: Option<String>,
    pub tasks: Option<String>,
    pub metadata: Option<String>,
    pub delta_specs: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Plan {
    pub change_id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub input_state_hash: String,
    pub input_content_root: String,
    pub operation_id: String,
    pub actions: Vec<super::collaboration_change::Action>,
    pub changed_artifacts: Vec<String>,
    pub expected_content_root: String,
}

impl Plan {
    pub fn is_noop(&self) -> bool {
        self.actions.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CobPlan {
    pub change_id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub input_state_hash: String,
    pub input_content_root: String,
    pub operation_id: String,
    pub actions: Vec<super::radicle_change::ChangeCobAction>,
    pub changed_artifacts: Vec<String>,
    pub expected_content_root: String,
}

impl CobPlan {
    pub fn is_noop(&self) -> bool {
        self.actions.is_empty()
    }
}

pub fn build_plan(
    state: &super::collaboration_change::State,
    snapshot: Snapshot,
) -> Result<Plan, super::radicle_change::ChangeCobError> {
    super::collaboration_change::validate_change_collaboration_state(state)?;
    validate_snapshot_context(state, &snapshot)?;
    let input_state_hash = super::collaboration_change::change_collaboration_state_hash(state)?;
    let input_content_root = require_conflict_free_state(state)?;
    let desired = desired_artifacts(&snapshot)?;
    let actions = artifact_actions(state, desired)?;
    let operation_id = derive_operation_id(
        &input_state_hash,
        &snapshot.actor,
        snapshot.is_delegate,
        &actions,
    )?;
    let changed_artifacts = actions
        .iter()
        .filter_map(|action| match action {
            super::collaboration_change::Action::SetArtifact { artifact, .. } => {
                Some(artifact.as_str().to_string())
            }
            super::collaboration_change::Action::Init { .. }
            | super::collaboration_change::Action::AttachEvidence { .. }
            | super::collaboration_change::Action::SetLifecycle { .. } => None,
        })
        .collect::<Vec<_>>();
    let mut simulated = state.clone();
    if !actions.is_empty() {
        super::collaboration_change::apply_change_collaboration_operation(
            &mut simulated,
            super::collaboration_change::Operation {
                id: operation_id.clone(),
                actor: snapshot.actor.clone(),
                is_delegate: snapshot.is_delegate,
                actions: actions.clone(),
            },
        )?;
    }
    let expected_content_root =
        super::collaboration_change::change_collaboration_content_root(&simulated)?;
    Ok(Plan {
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
    state: &super::radicle_change::ChangeCobState,
    snapshot: Snapshot,
) -> Result<CobPlan, super::radicle_change::ChangeCobError> {
    let neutral = super::collaboration_change::state_from_compatibility(state.clone())?;
    let plan = build_plan(&neutral, snapshot)?;
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
        input_content_root: plan.input_content_root,
        operation_id: plan.operation_id,
        actions,
        changed_artifacts: plan.changed_artifacts,
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
        return Err(super::radicle_change::ChangeCobError::InvalidField {
            field: "update.input_state_hash".to_string(),
            reason: "collaboration state changed after planning".to_string(),
        });
    }
    if plan.actions.is_empty() {
        return Ok(state.clone());
    }
    let mut updated = state.clone();
    super::collaboration_change::apply_change_collaboration_operation(
        &mut updated,
        super::collaboration_change::Operation {
            id: plan.operation_id.clone(),
            actor: plan.actor.clone(),
            is_delegate: plan.is_delegate,
            actions: plan.actions.clone(),
        },
    )?;
    let content_root = super::collaboration_change::change_collaboration_content_root(&updated)?;
    if !same_digest::<super::nominal_identity::ContentRootDigestRole>(
        &content_root,
        &plan.expected_content_root,
    ) {
        return Err(super::radicle_change::ChangeCobError::InvalidField {
            field: "update.expected_content_root".to_string(),
            reason: "applied update content does not match the reviewed plan".to_string(),
        });
    }
    Ok(updated)
}

pub fn plan_hash(plan: &Plan) -> Result<String, super::radicle_change::ChangeCobError> {
    hash_plan(plan)
}

pub fn cob_plan_hash(plan: &CobPlan) -> Result<String, super::radicle_change::ChangeCobError> {
    hash_plan(plan)
}

fn hash_plan<T: serde::Serialize>(
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
        "update.plan_hash",
    )
    .map_err(nominal_error)?;
    Ok(digest.as_hex().to_owned())
}

fn validate_snapshot_context(
    state: &super::collaboration_change::State,
    snapshot: &Snapshot,
) -> Result<(), super::radicle_change::ChangeCobError> {
    let state_change =
        super::nominal_identity::ChangeId::parse(&state.change_id, "update.state_change_id")
            .map_err(nominal_error)?;
    let snapshot_change =
        super::nominal_identity::ChangeId::parse(&snapshot.change_id, "update.snapshot_change_id")
            .map_err(nominal_error)?;
    if state_change != snapshot_change {
        return Err(super::radicle_change::ChangeCobError::InvalidField {
            field: "update.change_id".to_string(),
            reason: format!(
                "snapshot names {}, state names {}",
                snapshot.change_id, state.change_id
            ),
        });
    }
    let state_policy = super::nominal_identity::PolicyDigest::parse_compatible(
        &state.policy_hash,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "update.state_policy_hash",
    )
    .map_err(nominal_error)?;
    let snapshot_policy = super::nominal_identity::PolicyDigest::parse_compatible(
        &snapshot.policy_hash,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "update.snapshot_policy_hash",
    )
    .map_err(nominal_error)?;
    if state_policy != snapshot_policy {
        return Err(super::radicle_change::ChangeCobError::InvalidField {
            field: "update.policy_hash".to_string(),
            reason: "current policy does not match collaboration state".to_string(),
        });
    }
    let actor_bytes = snapshot.actor.len();
    if actor_bytes > super::radicle_change::MAX_ACTOR_ID_BYTES {
        return Err(super::radicle_change::ChangeCobError::ByteLimitExceeded {
            field: "actor".to_string(),
            actual: actor_bytes,
            limit: super::radicle_change::MAX_ACTOR_ID_BYTES,
        });
    }
    let actor = super::nominal_identity::ActorId::parse(&snapshot.actor, "update.actor")
        .map_err(nominal_error)?;
    let creator = super::nominal_identity::ActorId::parse(&state.creator, "update.creator")
        .map_err(nominal_error)?;
    if actor != creator && !snapshot.is_delegate {
        return Err(super::radicle_change::ChangeCobError::Unauthorized {
            actor: snapshot.actor.clone(),
            action: "plan_collaboration_update".to_string(),
        });
    }
    Ok(())
}

fn require_conflict_free_state(
    state: &super::collaboration_change::State,
) -> Result<String, super::radicle_change::ChangeCobError> {
    let content_root = super::collaboration_change::change_collaboration_content_root(state)?;
    match super::collaboration_change::change_collaboration_lifecycle_view(state)? {
        super::radicle_change::ChangeLifecycleView::Conflicted { revisions } => {
            Err(super::radicle_change::ChangeCobError::InvalidField {
                field: "update.lifecycle".to_string(),
                reason: format!("lifecycle has conflicting revisions: {revisions:?}"),
            })
        }
        super::radicle_change::ChangeLifecycleView::ContentConflicted { artifacts } => {
            Err(super::radicle_change::ChangeCobError::ArtifactConflict { artifacts })
        }
        super::radicle_change::ChangeLifecycleView::Current(_)
        | super::radicle_change::ChangeLifecycleView::Stale { .. } => Ok(content_root),
    }
}

fn desired_artifacts(
    snapshot: &Snapshot,
) -> Result<
    std::collections::BTreeMap<super::radicle_change::ChangeArtifactId, String>,
    super::radicle_change::ChangeCobError,
> {
    let mut artifacts = std::collections::BTreeMap::new();
    insert_present_artifact(
        &mut artifacts,
        super::radicle_change::ChangeArtifactId::proposal(),
        snapshot.proposal.clone(),
    );
    insert_present_artifact(
        &mut artifacts,
        super::radicle_change::ChangeArtifactId::design(),
        snapshot.design.clone(),
    );
    insert_present_artifact(
        &mut artifacts,
        super::radicle_change::ChangeArtifactId::tasks(),
        snapshot.tasks.clone(),
    );
    insert_present_artifact(
        &mut artifacts,
        super::radicle_change::ChangeArtifactId::metadata(),
        snapshot.metadata.clone(),
    );
    for (spec_id, content) in &snapshot.delta_specs {
        artifacts.insert(
            super::radicle_change::ChangeArtifactId::delta_spec(spec_id)?,
            content.clone(),
        );
    }
    Ok(artifacts)
}

fn insert_present_artifact(
    artifacts: &mut std::collections::BTreeMap<super::radicle_change::ChangeArtifactId, String>,
    artifact: super::radicle_change::ChangeArtifactId,
    content: Option<String>,
) {
    if let Some(content) = content {
        artifacts.insert(artifact, content);
    }
}

fn artifact_actions(
    state: &super::collaboration_change::State,
    desired: std::collections::BTreeMap<super::radicle_change::ChangeArtifactId, String>,
) -> Result<Vec<super::collaboration_change::Action>, super::radicle_change::ChangeCobError> {
    let artifact_ids = state
        .artifacts
        .keys()
        .chain(desired.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut actions = Vec::new();
    for artifact in artifact_ids {
        let (current, basis) = match state.artifacts.get(&artifact) {
            Some(history) => {
                let heads = history.heads();
                if heads.len() != 1 {
                    return Err(super::radicle_change::ChangeCobError::ArtifactConflict {
                        artifacts: vec![artifact.as_str().to_string()],
                    });
                }
                let Some((_, revision)) = heads.first() else {
                    return Err(super::radicle_change::ChangeCobError::ArtifactConflict {
                        artifacts: vec![artifact.as_str().to_string()],
                    });
                };
                (revision.value.clone(), history.head_ids())
            }
            None => (None, std::collections::BTreeSet::new()),
        };
        let next = desired.get(&artifact).cloned();
        if current != next {
            actions.push(super::collaboration_change::Action::SetArtifact {
                artifact,
                content: next,
                basis,
            });
        }
    }
    Ok(actions)
}

fn same_digest<Role: super::nominal_identity::DigestRoleMarker>(left: &str, right: &str) -> bool {
    let left = super::nominal_identity::Blake3Digest::<Role>::parse_compatible(
        left,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "update.left_digest",
    );
    let right = super::nominal_identity::Blake3Digest::<Role>::parse_compatible(
        right,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        "update.right_digest",
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

fn derive_operation_id(
    input_state_hash: &str,
    actor: &str,
    is_delegate: bool,
    actions: &[super::collaboration_change::Action],
) -> Result<String, super::radicle_change::ChangeCobError> {
    let identity = OperationIdentity {
        domain: UPDATE_OPERATION_DOMAIN,
        input_state_hash,
        actor,
        is_delegate,
        actions,
    };
    let value = serde_json::to_value(identity).map_err(|error| {
        super::radicle_change::ChangeCobError::Serialization {
            reason: error.to_string(),
        }
    })?;
    Ok(super::evidence::canonical_json_hash(&value))
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

    const CHANGE_ID: &str = "collaboration-update";
    const WORKSPACE_ID: &str = "workspace:update";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const CREATOR: &str = "actor:creator";
    const DELEGATE: &str = "actor:delegate";
    const OUTSIDER: &str = "actor:outsider";
    const EDIT_OPERATION: &str = "edit-operation";
    const CONCURRENT_OPERATION: &str = "concurrent-operation";

    fn state() -> super::super::collaboration_change::State {
        let init =
            crate::plan_change_collaboration_import(crate::ChangeCollaborationImportSnapshot {
                change_id: CHANGE_ID.to_string(),
                workspace_id: WORKSPACE_ID.to_string(),
                base_revision: crate::ChangeBaseRevision::new("opaque", "revision-one").unwrap(),
                policy_hash: POLICY_HASH.to_string(),
                proposal: Some("# Proposal\n".to_string()),
                design: None,
                tasks: None,
                metadata: None,
                delta_specs: std::collections::BTreeMap::from([(
                    CHANGE_ID.to_string(),
                    "# Delta\n".to_string(),
                )]),
            })
            .unwrap();
        crate::initialize_change_collaboration_import(init, CREATOR).unwrap()
    }

    fn snapshot(actor: &str) -> Snapshot {
        Snapshot {
            change_id: CHANGE_ID.to_string(),
            policy_hash: POLICY_HASH.to_string(),
            actor: actor.to_string(),
            is_delegate: actor == DELEGATE,
            proposal: Some("# Updated proposal\n".to_string()),
            design: Some("# Design\n".to_string()),
            tasks: None,
            metadata: None,
            delta_specs: std::collections::BTreeMap::new(),
        }
    }

    // r[verify cairn.collaboration_update.plan.exact]
    #[test]
    fn plans_and_applies_sorted_add_change_remove_actions() {
        let state = state();
        let first = build_plan(&state, snapshot(CREATOR)).unwrap();
        let second = build_plan(&state, snapshot(CREATOR)).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.changed_artifacts,
            vec![
                "design".to_string(),
                "proposal".to_string(),
                format!("spec:{CHANGE_ID}"),
            ]
        );
        assert_eq!(plan_hash(&first).unwrap(), plan_hash(&second).unwrap());
        let updated = apply(&state, &first).unwrap();
        assert_eq!(
            super::super::collaboration_change::change_collaboration_content_root(&updated)
                .unwrap(),
            first.expected_content_root
        );
        let files = crate::materialize_change_collaboration(&updated).unwrap();
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
    fn byte_equivalent_change_is_noop() {
        let state = state();
        let plan = build_plan(
            &state,
            Snapshot {
                change_id: CHANGE_ID.to_string(),
                policy_hash: POLICY_HASH.to_string(),
                actor: CREATOR.to_string(),
                is_delegate: false,
                proposal: Some("# Proposal\n".to_string()),
                design: None,
                tasks: None,
                metadata: None,
                delta_specs: std::collections::BTreeMap::from([(
                    CHANGE_ID.to_string(),
                    "# Delta\n".to_string(),
                )]),
            },
        )
        .unwrap();
        assert!(plan.is_noop());
        assert_eq!(plan.input_content_root, plan.expected_content_root);
        assert_eq!(apply(&state, &plan).unwrap(), state);
    }

    // r[verify cairn.collaboration_update.plan.blocked]
    #[test]
    fn rejects_wrong_identity_policy_actor_and_protocol_inputs() {
        let state = state();
        let mut wrong_change = snapshot(CREATOR);
        wrong_change.change_id = "other-change".to_string();
        assert!(build_plan(&state, wrong_change).is_err());

        let mut stale_policy = snapshot(CREATOR);
        stale_policy.policy_hash = "c".repeat(crate::BLAKE3_HEX_LENGTH);
        assert!(build_plan(&state, stale_policy).is_err());

        assert!(matches!(
            build_plan(&state, snapshot(OUTSIDER)),
            Err(super::super::radicle_change::ChangeCobError::Unauthorized { .. })
        ));

        let mut malformed_spec = snapshot(CREATOR);
        malformed_spec
            .delta_specs
            .insert("../escape".to_string(), "# Bad\n".to_string());
        assert!(build_plan(&state, malformed_spec).is_err());

        let mut oversized = snapshot(CREATOR);
        oversized.design = Some("x".repeat(crate::MAX_ARTIFACT_BYTES.saturating_add(1)));
        assert!(matches!(
            build_plan(&state, oversized),
            Err(super::super::radicle_change::ChangeCobError::ByteLimitExceeded { .. })
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
            crate::apply_change_collaboration_operation(
                &mut content_conflict,
                super::super::collaboration_change::Operation {
                    id: id.to_string(),
                    actor: CREATOR.to_string(),
                    is_delegate: false,
                    actions: vec![super::super::collaboration_change::Action::SetArtifact {
                        artifact: super::super::radicle_change::ChangeArtifactId::proposal(),
                        content: Some(content.to_string()),
                        basis: std::collections::BTreeSet::from([root.clone()]),
                    }],
                },
            )
            .unwrap();
        }
        assert!(matches!(
            build_plan(&content_conflict, snapshot(CREATOR)),
            Err(super::super::radicle_change::ChangeCobError::ArtifactConflict { .. })
        ));

        let mut lifecycle_conflict = state();
        let root = lifecycle_conflict.operations.iter().next().unwrap().clone();
        let subject_root = super::super::collaboration_change::change_collaboration_content_root(
            &lifecycle_conflict,
        )
        .unwrap();
        for (id, status) in [
            (EDIT_OPERATION, crate::ChangeLifecycleStatus::Open),
            (CONCURRENT_OPERATION, crate::ChangeLifecycleStatus::Open),
        ] {
            crate::apply_change_collaboration_operation(
                &mut lifecycle_conflict,
                super::super::collaboration_change::Operation {
                    id: id.to_string(),
                    actor: DELEGATE.to_string(),
                    is_delegate: true,
                    actions: vec![super::super::collaboration_change::Action::SetLifecycle {
                        status,
                        subject_root: subject_root.clone(),
                        evidence: std::collections::BTreeSet::new(),
                        basis: std::collections::BTreeSet::from([root.clone()]),
                    }],
                },
            )
            .unwrap();
        }
        assert!(matches!(
            build_plan(&lifecycle_conflict, snapshot(DELEGATE)),
            Err(super::super::radicle_change::ChangeCobError::InvalidField { field, .. }) if field == "update.lifecycle"
        ));
    }
}
