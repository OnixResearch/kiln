// r[impl cairn.nominal_evidence_identities.lifecycle]
// r[verify cairn.nominal_evidence_identities.lifecycle.validation]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::nominal_identity::{
    ActorId, ChangeId, IdentityIssue, OperationId, PolicyDigest, WorkspaceId, RAW_BLAKE3_PROFILE,
};
use super::radicle_change::{
    apply_change_cob_operation, change_cob_content_root, change_cob_lifecycle_view,
    change_cob_state_hash, current_change_cob_evidence, decode_base_revision,
    initialize_change_cob, materialize_change_cob, validate_change_cob_init,
    validate_change_cob_state, ChangeArtifactId, ChangeBaseRevision, ChangeCobAction,
    ChangeCobError, ChangeCobEvidence, ChangeCobInit, ChangeCobOperation, ChangeCobState,
    ChangeLifecycleRecord, ChangeLifecycleStatus, ChangeLifecycleView, ChangeMaterializedFile,
    ChangeRevisionSet, CHANGE_COLLABORATION_SCHEMA_V1, RADICLE_CHANGE_COB_SCHEMA_V1,
};

pub const BASE_REVISION_KIND_GIT_OID: &str = "git_oid";

// r[impl cairn.collaboration_backend.semantic_core]

pub type ChangeCollaborationEvidence = ChangeCobEvidence;
pub type ChangeCollaborationError = ChangeCobError;

#[derive(Debug)]
struct AdmittedCollaborationInit {
    change_id: ChangeId,
    workspace_id: WorkspaceId,
    policy_hash: PolicyDigest,
    delegates: BTreeSet<ActorId>,
}

#[derive(Debug)]
struct AdmittedCollaborationOperation {
    id: OperationId,
    actor: ActorId,
}

#[derive(Debug)]
struct AdmittedCollaborationState {
    change_id: ChangeId,
    workspace_id: WorkspaceId,
    policy_hash: PolicyDigest,
    creator: ActorId,
    delegates: BTreeSet<ActorId>,
    operations: BTreeSet<OperationId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCollaborationInit {
    pub schema: String,
    pub change_id: String,
    pub workspace_id: String,
    pub base_revision: ChangeBaseRevision,
    pub policy_hash: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub delegates: BTreeSet<String>,
    pub artifacts: BTreeMap<ChangeArtifactId, String>,
}

impl ChangeCollaborationInit {
    pub fn new(
        change_id: impl Into<String>,
        workspace_id: impl Into<String>,
        base_revision: ChangeBaseRevision,
        policy_hash: impl Into<String>,
        artifacts: BTreeMap<ChangeArtifactId, String>,
    ) -> Self {
        Self {
            schema: CHANGE_COLLABORATION_SCHEMA_V1.to_string(),
            change_id: change_id.into(),
            workspace_id: workspace_id.into(),
            base_revision,
            policy_hash: policy_hash.into(),
            delegates: BTreeSet::new(),
            artifacts,
        }
    }

    pub fn with_delegates(mut self, delegates: BTreeSet<String>) -> Self {
        self.delegates = delegates;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChangeCollaborationAction {
    Init {
        state: ChangeCollaborationInit,
    },
    SetArtifact {
        artifact: ChangeArtifactId,
        content: Option<String>,
        basis: BTreeSet<String>,
    },
    AttachEvidence {
        evidence: ChangeCollaborationEvidence,
    },
    SetLifecycle {
        status: ChangeLifecycleStatus,
        subject_root: String,
        evidence: BTreeSet<String>,
        basis: BTreeSet<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCollaborationOperation {
    pub id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub actions: Vec<ChangeCollaborationAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCollaborationState {
    pub schema: String,
    pub change_id: String,
    pub workspace_id: String,
    pub base_revision: ChangeBaseRevision,
    pub policy_hash: String,
    pub creator: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub delegates: BTreeSet<String>,
    pub artifacts: BTreeMap<ChangeArtifactId, ChangeRevisionSet<Option<String>>>,
    pub evidence: BTreeMap<String, ChangeCollaborationEvidence>,
    pub lifecycle: ChangeRevisionSet<ChangeLifecycleRecord>,
    pub operations: BTreeSet<String>,
}

pub fn initialize_change_collaboration(
    operation: ChangeCollaborationOperation,
) -> Result<ChangeCollaborationState, ChangeCollaborationError> {
    let enforce_nominal = operation.actions.iter().any(|action| {
        matches!(
            action,
            ChangeCollaborationAction::Init { state }
                if state.schema == CHANGE_COLLABORATION_SCHEMA_V1
        )
    });
    let state = initialize_change_cob(operation_into_compatibility(operation, enforce_nominal)?)?;
    state_from_compatibility(state)
}

pub fn apply_change_collaboration_operation(
    state: &mut ChangeCollaborationState,
    operation: ChangeCollaborationOperation,
) -> Result<(), ChangeCollaborationError> {
    let mut compatibility = state_to_compatibility(state)?;
    let enforce_nominal = state.schema == CHANGE_COLLABORATION_SCHEMA_V1;
    apply_change_cob_operation(
        &mut compatibility,
        operation_into_compatibility(operation, enforce_nominal)?,
    )?;
    *state = state_from_compatibility(compatibility)?;
    Ok(())
}

pub fn validate_change_collaboration_init(
    init: &ChangeCollaborationInit,
) -> Result<(), ChangeCollaborationError> {
    validate_change_cob_init(&init_to_compatibility(init.clone())?)
}

pub fn validate_change_collaboration_state(
    state: &ChangeCollaborationState,
) -> Result<(), ChangeCollaborationError> {
    validate_change_cob_state(&state_to_compatibility(state)?)
}

pub fn change_collaboration_content_root(
    state: &ChangeCollaborationState,
) -> Result<String, ChangeCollaborationError> {
    change_cob_content_root(&state_to_compatibility(state)?)
}

pub fn change_collaboration_state_hash(
    state: &ChangeCollaborationState,
) -> Result<String, ChangeCollaborationError> {
    change_cob_state_hash(&state_to_compatibility(state)?)
}

pub fn materialize_change_collaboration(
    state: &ChangeCollaborationState,
) -> Result<Vec<ChangeMaterializedFile>, ChangeCollaborationError> {
    materialize_change_cob(&state_to_compatibility(state)?)
}

pub fn current_change_collaboration_evidence(
    state: &ChangeCollaborationState,
) -> Result<Vec<ChangeCollaborationEvidence>, ChangeCollaborationError> {
    current_change_cob_evidence(&state_to_compatibility(state)?)
}

pub fn change_collaboration_lifecycle_view(
    state: &ChangeCollaborationState,
) -> Result<ChangeLifecycleView, ChangeCollaborationError> {
    change_cob_lifecycle_view(&state_to_compatibility(state)?)
}

pub fn change_collaboration_action_json(
    action: &ChangeCollaborationAction,
) -> Result<Value, ChangeCollaborationError> {
    serde_json::to_value(action).map_err(|error| ChangeCobError::Serialization {
        reason: error.to_string(),
    })
}

pub(crate) fn state_to_compatibility(
    state: &ChangeCollaborationState,
) -> Result<ChangeCobState, ChangeCollaborationError> {
    if state.schema != CHANGE_COLLABORATION_SCHEMA_V1 {
        return Ok(ChangeCobState {
            schema: state.schema.clone(),
            change_id: state.change_id.clone(),
            repository_id: state.workspace_id.clone(),
            base_git_oid: compatibility_base_revision(&state.schema, &state.base_revision)?,
            policy_hash: state.policy_hash.clone(),
            creator: state.creator.clone(),
            delegates: state.delegates.clone(),
            artifacts: state.artifacts.clone(),
            evidence: state.evidence.clone(),
            lifecycle: state.lifecycle.clone(),
            operations: state.operations.clone(),
        });
    }
    let identities = admit_collaboration_state(state)?;
    Ok(ChangeCobState {
        schema: state.schema.clone(),
        change_id: identities.change_id.as_str().to_owned(),
        repository_id: identities.workspace_id.as_str().to_owned(),
        base_git_oid: compatibility_base_revision(&state.schema, &state.base_revision)?,
        policy_hash: identities.policy_hash.as_hex().to_owned(),
        creator: identities.creator.as_str().to_owned(),
        delegates: identities
            .delegates
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect(),
        artifacts: state.artifacts.clone(),
        evidence: state.evidence.clone(),
        lifecycle: state.lifecycle.clone(),
        operations: identities
            .operations
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect(),
    })
}

pub(crate) fn state_from_compatibility(
    state: ChangeCobState,
) -> Result<ChangeCollaborationState, ChangeCollaborationError> {
    let base_revision = if state.schema == RADICLE_CHANGE_COB_SCHEMA_V1 {
        ChangeBaseRevision::new(BASE_REVISION_KIND_GIT_OID, &state.base_git_oid)?
    } else {
        decode_base_revision(&state.base_git_oid)?
    };
    let result = ChangeCollaborationState {
        schema: state.schema,
        change_id: state.change_id,
        workspace_id: state.repository_id,
        base_revision,
        policy_hash: state.policy_hash,
        creator: state.creator,
        delegates: state.delegates,
        artifacts: state.artifacts,
        evidence: state.evidence,
        lifecycle: state.lifecycle,
        operations: state.operations,
    };
    if result.schema == CHANGE_COLLABORATION_SCHEMA_V1 {
        let _ = admit_collaboration_state(&result)?;
    }
    Ok(result)
}

fn operation_into_compatibility(
    operation: ChangeCollaborationOperation,
    enforce_nominal: bool,
) -> Result<ChangeCobOperation, ChangeCollaborationError> {
    let identities = if enforce_nominal {
        Some(admit_collaboration_operation(&operation)?)
    } else {
        None
    };
    let actions = operation
        .actions
        .into_iter()
        .map(action_into_compatibility)
        .collect::<Result<Vec<_>, _>>()?;
    let Some(identities) = identities else {
        return Ok(ChangeCobOperation {
            id: operation.id,
            actor: operation.actor,
            is_delegate: operation.is_delegate,
            actions,
        });
    };
    Ok(ChangeCobOperation {
        id: identities.id.as_str().to_owned(),
        actor: identities.actor.as_str().to_owned(),
        is_delegate: operation.is_delegate,
        actions,
    })
}

pub(crate) fn action_into_compatibility(
    action: ChangeCollaborationAction,
) -> Result<ChangeCobAction, ChangeCollaborationError> {
    Ok(match action {
        ChangeCollaborationAction::Init { state } => ChangeCobAction::Init {
            state: init_to_compatibility(state)?,
        },
        ChangeCollaborationAction::SetArtifact {
            artifact,
            content,
            basis,
        } => ChangeCobAction::SetArtifact {
            artifact,
            content,
            basis,
        },
        ChangeCollaborationAction::AttachEvidence { evidence } => {
            ChangeCobAction::AttachEvidence { evidence }
        }
        ChangeCollaborationAction::SetLifecycle {
            status,
            subject_root,
            evidence,
            basis,
        } => ChangeCobAction::SetLifecycle {
            status,
            subject_root,
            evidence,
            basis,
        },
    })
}

fn init_to_compatibility(
    init: ChangeCollaborationInit,
) -> Result<ChangeCobInit, ChangeCollaborationError> {
    if init.schema != CHANGE_COLLABORATION_SCHEMA_V1 {
        return Ok(ChangeCobInit {
            schema: init.schema.clone(),
            change_id: init.change_id,
            repository_id: init.workspace_id,
            base_git_oid: compatibility_base_revision(&init.schema, &init.base_revision)?,
            policy_hash: init.policy_hash,
            delegates: init.delegates,
            artifacts: init.artifacts,
        });
    }
    let identities = admit_collaboration_init(&init)?;
    Ok(ChangeCobInit {
        schema: init.schema.clone(),
        change_id: identities.change_id.as_str().to_owned(),
        repository_id: identities.workspace_id.as_str().to_owned(),
        base_git_oid: compatibility_base_revision(&init.schema, &init.base_revision)?,
        policy_hash: identities.policy_hash.as_hex().to_owned(),
        delegates: identities
            .delegates
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect(),
        artifacts: init.artifacts,
    })
}

fn admit_collaboration_init(
    init: &ChangeCollaborationInit,
) -> Result<AdmittedCollaborationInit, ChangeCollaborationError> {
    Ok(AdmittedCollaborationInit {
        change_id: ChangeId::parse(&init.change_id, "collaboration.change_id")
            .map_err(nominal_error)?,
        workspace_id: WorkspaceId::parse(&init.workspace_id, "collaboration.workspace_id")
            .map_err(nominal_error)?,
        policy_hash: PolicyDigest::parse_compatible(
            &init.policy_hash,
            RAW_BLAKE3_PROFILE,
            "collaboration.policy_hash",
        )
        .map_err(nominal_error)?,
        delegates: admit_actor_set(&init.delegates, "collaboration.delegates")?,
    })
}

fn admit_collaboration_operation(
    operation: &ChangeCollaborationOperation,
) -> Result<AdmittedCollaborationOperation, ChangeCollaborationError> {
    Ok(AdmittedCollaborationOperation {
        id: OperationId::parse(&operation.id, "collaboration.operation_id")
            .map_err(nominal_error)?,
        actor: ActorId::parse(&operation.actor, "collaboration.actor_id").map_err(nominal_error)?,
    })
}

fn admit_collaboration_state(
    state: &ChangeCollaborationState,
) -> Result<AdmittedCollaborationState, ChangeCollaborationError> {
    Ok(AdmittedCollaborationState {
        change_id: ChangeId::parse(&state.change_id, "collaboration.change_id")
            .map_err(nominal_error)?,
        workspace_id: WorkspaceId::parse(&state.workspace_id, "collaboration.workspace_id")
            .map_err(nominal_error)?,
        policy_hash: PolicyDigest::parse_compatible(
            &state.policy_hash,
            RAW_BLAKE3_PROFILE,
            "collaboration.policy_hash",
        )
        .map_err(nominal_error)?,
        creator: ActorId::parse(&state.creator, "collaboration.creator").map_err(nominal_error)?,
        delegates: admit_actor_set(&state.delegates, "collaboration.delegates")?,
        operations: state
            .operations
            .iter()
            .map(|value| {
                OperationId::parse(value, "collaboration.operations").map_err(nominal_error)
            })
            .collect::<Result<BTreeSet<_>, _>>()?,
    })
}

fn admit_actor_set(
    values: &BTreeSet<String>,
    field: &'static str,
) -> Result<BTreeSet<ActorId>, ChangeCollaborationError> {
    values
        .iter()
        .map(|value| ActorId::parse(value, field).map_err(nominal_error))
        .collect()
}

fn nominal_error(issue: IdentityIssue) -> ChangeCollaborationError {
    ChangeCobError::InvalidField {
        field: issue.field.to_owned(),
        reason: issue.code.to_owned(),
    }
}

fn compatibility_base_revision(
    schema: &str,
    revision: &ChangeBaseRevision,
) -> Result<String, ChangeCollaborationError> {
    if schema == RADICLE_CHANGE_COB_SCHEMA_V1 {
        if revision.kind != BASE_REVISION_KIND_GIT_OID {
            return Err(ChangeCobError::InvalidField {
                field: "base_revision.kind".to_string(),
                reason: format!(
                    "legacy Radicle state requires {BASE_REVISION_KIND_GIT_OID}, observed {}",
                    revision.kind
                ),
            });
        }
        return Ok(revision.value.clone());
    }
    revision.encode()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{canonical_json_hash, RADICLE_CHANGE_COB_SCHEMA_V1};
    use serde_json::json;

    const CHANGE_ID: &str = "backend-neutral";
    const WORKSPACE_ID: &str = "workspace:example";
    const BASE_REVISION_KIND: &str = "opaque";
    const BASE_REVISION_VALUE: &str = "revision-one";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ACTOR: &str = "actor:example";
    const ROOT_OPERATION: &str = "operation:root";
    const LEGACY_GIT_OID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn neutral_init() -> ChangeCollaborationInit {
        ChangeCollaborationInit::new(
            CHANGE_ID,
            WORKSPACE_ID,
            ChangeBaseRevision::new(BASE_REVISION_KIND, BASE_REVISION_VALUE).unwrap(),
            POLICY_HASH,
            BTreeMap::from([(ChangeArtifactId::proposal(), "# Proposal\n".to_string())]),
        )
    }

    fn neutral_state() -> ChangeCollaborationState {
        initialize_change_collaboration(ChangeCollaborationOperation {
            id: ROOT_OPERATION.to_string(),
            actor: ACTOR.to_string(),
            is_delegate: false,
            actions: vec![ChangeCollaborationAction::Init {
                state: neutral_init(),
            }],
        })
        .unwrap()
    }

    // r[verify cairn.collaboration_backend.semantic_core.neutral]
    #[test]
    fn neutral_schema_accepts_non_git_revision_and_reduces_conflicts() {
        let mut state = neutral_state();
        assert_eq!(state.base_revision.kind, BASE_REVISION_KIND);
        assert_eq!(materialize_change_collaboration(&state).unwrap().len(), 1);
        let root_head = state
            .artifacts
            .get(&ChangeArtifactId::proposal())
            .unwrap()
            .head_ids();
        for (id, content) in [
            ("operation:left", "# Left\n"),
            ("operation:right", "# Right\n"),
        ] {
            apply_change_collaboration_operation(
                &mut state,
                ChangeCollaborationOperation {
                    id: id.to_string(),
                    actor: ACTOR.to_string(),
                    is_delegate: false,
                    actions: vec![ChangeCollaborationAction::SetArtifact {
                        artifact: ChangeArtifactId::proposal(),
                        content: Some(content.to_string()),
                        basis: root_head.clone(),
                    }],
                },
            )
            .unwrap();
        }
        assert!(matches!(
            materialize_change_collaboration(&state),
            Err(ChangeCobError::ArtifactConflict { .. })
        ));
    }

    // r[verify cairn.collaboration_backend.semantic_core.legacy]
    // r[verify cairn.radicle_cob_only.compatibility.legacy]
    // r[verify cairn.radicle_cob_only.authority.legacy_hash]
    #[test]
    fn legacy_radicle_hashes_use_the_original_canonical_shape() {
        let legacy = ChangeCobState {
            schema: RADICLE_CHANGE_COB_SCHEMA_V1.to_string(),
            change_id: CHANGE_ID.to_string(),
            repository_id: "rad:z3example".to_string(),
            base_git_oid: LEGACY_GIT_OID.to_string(),
            policy_hash: POLICY_HASH.to_string(),
            creator: ACTOR.to_string(),
            delegates: BTreeSet::new(),
            artifacts: BTreeMap::from([(
                ChangeArtifactId::proposal(),
                ChangeRevisionSet {
                    revisions: BTreeMap::from([(
                        LEGACY_GIT_OID.to_string(),
                        super::super::radicle_change::ChangeRevision {
                            actor: ACTOR.to_string(),
                            basis: BTreeSet::new(),
                            value: Some("# Proposal\n".to_string()),
                        },
                    )]),
                },
            )]),
            evidence: BTreeMap::new(),
            lifecycle: ChangeRevisionSet {
                revisions: BTreeMap::new(),
            },
            operations: BTreeSet::from([LEGACY_GIT_OID.to_string()]),
        };
        let expected_state_hash = canonical_json_hash(&serde_json::to_value(&legacy).unwrap());
        assert_eq!(change_cob_state_hash(&legacy).unwrap(), expected_state_hash);

        let expected_content_root = canonical_json_hash(&json!({
            "schema": RADICLE_CHANGE_COB_SCHEMA_V1,
            "change_id": CHANGE_ID,
            "repository_id": "rad:z3example",
            "base_git_oid": LEGACY_GIT_OID,
            "policy_hash": POLICY_HASH,
            "creator": ACTOR,
            "artifacts": { "proposal": "# Proposal\n" },
        }));
        assert_eq!(
            change_cob_content_root(&legacy).unwrap(),
            expected_content_root
        );

        let neutral = state_from_compatibility(legacy.clone()).unwrap();
        assert_eq!(
            change_collaboration_state_hash(&neutral).unwrap(),
            expected_state_hash
        );
        assert_eq!(
            change_collaboration_content_root(&neutral).unwrap(),
            expected_content_root
        );
    }

    #[test]
    fn malformed_revision_and_legacy_non_git_revision_fail_closed() {
        assert!(ChangeBaseRevision::new("bad:kind", "value").is_err());
        let mut init = neutral_init();
        init.schema = RADICLE_CHANGE_COB_SCHEMA_V1.to_string();
        assert!(validate_change_collaboration_init(&init).is_err());
    }

    #[test]
    fn generic_action_json_uses_transport_neutral_fields() {
        let value = change_collaboration_action_json(&ChangeCollaborationAction::Init {
            state: neutral_init(),
        })
        .unwrap();
        assert!(value["state"].get("workspace_id").is_some());
        assert!(value["state"].get("base_revision").is_some());
        assert!(value.to_string().find("radicle").is_none());
        assert!(value.to_string().find("base_git_oid").is_none());
    }
}
