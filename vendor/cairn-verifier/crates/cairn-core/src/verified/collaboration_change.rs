// r[impl cairn.nominal_evidence_identities.lifecycle]
// r[verify cairn.nominal_evidence_identities.lifecycle.validation]

use std::collections::BTreeSet;

pub const BASE_REVISION_KIND_GIT_OID: &str = "git_oid";

// r[impl cairn.collaboration_backend.semantic_core]

pub type Evidence = super::radicle_change::ChangeCobEvidence;
pub type Error = super::radicle_change::ChangeCobError;

#[derive(Debug)]
struct AdmittedInit {
    change_id: super::nominal_identity::ChangeId,
    workspace_id: super::nominal_identity::WorkspaceId,
    policy_hash: super::nominal_identity::PolicyDigest,
    delegates: std::collections::BTreeSet<super::nominal_identity::ActorId>,
}

#[derive(Debug)]
struct AdmittedOperation {
    id: super::nominal_identity::OperationId,
    actor: super::nominal_identity::ActorId,
}

#[derive(Debug)]
struct AdmittedState {
    change_id: super::nominal_identity::ChangeId,
    workspace_id: super::nominal_identity::WorkspaceId,
    policy_hash: super::nominal_identity::PolicyDigest,
    creator: super::nominal_identity::ActorId,
    delegates: std::collections::BTreeSet<super::nominal_identity::ActorId>,
    operations: std::collections::BTreeSet<super::nominal_identity::OperationId>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Init {
    pub schema: String,
    pub change_id: String,
    pub workspace_id: String,
    pub base_revision: super::radicle_change::ChangeBaseRevision,
    pub policy_hash: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub delegates: std::collections::BTreeSet<String>,
    pub artifacts: std::collections::BTreeMap<super::radicle_change::ChangeArtifactId, String>,
}

impl Init {
    pub fn new(
        change_id: impl Into<String>,
        workspace_id: impl Into<String>,
        base_revision: super::radicle_change::ChangeBaseRevision,
        policy_hash: impl Into<String>,
        artifacts: std::collections::BTreeMap<super::radicle_change::ChangeArtifactId, String>,
    ) -> Self {
        Self {
            schema: super::radicle_change::CHANGE_COLLABORATION_SCHEMA_V1.to_string(),
            change_id: change_id.into(),
            workspace_id: workspace_id.into(),
            base_revision,
            policy_hash: policy_hash.into(),
            delegates: std::collections::BTreeSet::new(),
            artifacts,
        }
    }

    pub fn with_delegates(mut self, delegates: std::collections::BTreeSet<String>) -> Self {
        self.delegates = delegates;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Init {
        state: Init,
    },
    SetArtifact {
        artifact: super::radicle_change::ChangeArtifactId,
        content: Option<String>,
        basis: std::collections::BTreeSet<String>,
    },
    AttachEvidence {
        evidence: Evidence,
    },
    SetLifecycle {
        status: super::radicle_change::ChangeLifecycleStatus,
        subject_root: String,
        evidence: std::collections::BTreeSet<String>,
        basis: std::collections::BTreeSet<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct State {
    pub schema: String,
    pub change_id: String,
    pub workspace_id: String,
    pub base_revision: super::radicle_change::ChangeBaseRevision,
    pub policy_hash: String,
    pub creator: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub delegates: std::collections::BTreeSet<String>,
    pub artifacts: std::collections::BTreeMap<
        super::radicle_change::ChangeArtifactId,
        super::radicle_change::ChangeRevisionSet<Option<String>>,
    >,
    pub evidence: std::collections::BTreeMap<String, Evidence>,
    pub lifecycle:
        super::radicle_change::ChangeRevisionSet<super::radicle_change::ChangeLifecycleRecord>,
    pub operations: std::collections::BTreeSet<String>,
}

pub fn initialize_change_collaboration(operation: Operation) -> Result<State, Error> {
    let enforce_nominal = operation.actions.iter().any(|action| {
        matches!(
            action,
            Action::Init { state }
                if state.schema == super::radicle_change::CHANGE_COLLABORATION_SCHEMA_V1
        )
    });
    let state = super::radicle_change::initialize_change_cob(operation_into_compatibility(
        operation,
        enforce_nominal,
    )?)?;
    state_from_compatibility(state)
}

pub fn apply_change_collaboration_operation(
    state: &mut State,
    operation: Operation,
) -> Result<(), Error> {
    let mut compatibility = state_to_compatibility(state)?;
    let enforce_nominal = state.schema == super::radicle_change::CHANGE_COLLABORATION_SCHEMA_V1;
    super::radicle_change::apply_change_cob_operation(
        &mut compatibility,
        operation_into_compatibility(operation, enforce_nominal)?,
    )?;
    *state = state_from_compatibility(compatibility)?;
    Ok(())
}

pub fn validate_change_collaboration_init(init: &Init) -> Result<(), Error> {
    super::radicle_change::validate_change_cob_init(&init_to_compatibility(init.clone())?)
}

pub fn validate_change_collaboration_state(state: &State) -> Result<(), Error> {
    super::radicle_change::validate_change_cob_state(&state_to_compatibility(state)?)
}

pub fn change_collaboration_content_root(state: &State) -> Result<String, Error> {
    super::radicle_change::change_cob_content_root(&state_to_compatibility(state)?)
}

pub fn change_collaboration_state_hash(state: &State) -> Result<String, Error> {
    super::radicle_change::change_cob_state_hash(&state_to_compatibility(state)?)
}

pub fn materialize_change_collaboration(
    state: &State,
) -> Result<Vec<super::radicle_change::ChangeMaterializedFile>, Error> {
    super::radicle_change::materialize_change_cob(&state_to_compatibility(state)?)
}

pub fn current_change_collaboration_evidence(state: &State) -> Result<Vec<Evidence>, Error> {
    super::radicle_change::current_change_cob_evidence(&state_to_compatibility(state)?)
}

pub fn change_collaboration_lifecycle_view(
    state: &State,
) -> Result<super::radicle_change::ChangeLifecycleView, Error> {
    super::radicle_change::change_cob_lifecycle_view(&state_to_compatibility(state)?)
}

pub fn action_json(action: &Action) -> Result<serde_json::Value, Error> {
    serde_json::to_value(action).map_err(|error| {
        super::radicle_change::ChangeCobError::Serialization {
            reason: error.to_string(),
        }
    })
}

pub(crate) fn state_to_compatibility(
    state: &State,
) -> Result<super::radicle_change::ChangeCobState, Error> {
    if state.schema != super::radicle_change::CHANGE_COLLABORATION_SCHEMA_V1 {
        return Ok(super::radicle_change::ChangeCobState {
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
    Ok(super::radicle_change::ChangeCobState {
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
    state: super::radicle_change::ChangeCobState,
) -> Result<State, Error> {
    let base_revision = if state.schema == super::radicle_change::RADICLE_CHANGE_COB_SCHEMA_V1 {
        super::radicle_change::ChangeBaseRevision::new(
            BASE_REVISION_KIND_GIT_OID,
            &state.base_git_oid,
        )?
    } else {
        super::radicle_change::decode_base_revision(&state.base_git_oid)?
    };
    let result = State {
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
    if result.schema == super::radicle_change::CHANGE_COLLABORATION_SCHEMA_V1 {
        let _ = admit_collaboration_state(&result)?;
    }
    Ok(result)
}

fn operation_into_compatibility(
    operation: Operation,
    enforce_nominal: bool,
) -> Result<super::radicle_change::ChangeCobOperation, Error> {
    let identities = if enforce_nominal {
        Some(admit_operation(&operation)?)
    } else {
        None
    };
    let actions = operation
        .actions
        .into_iter()
        .map(action_into_compatibility)
        .collect::<Result<Vec<_>, _>>()?;
    let Some(identities) = identities else {
        return Ok(super::radicle_change::ChangeCobOperation {
            id: operation.id,
            actor: operation.actor,
            is_delegate: operation.is_delegate,
            actions,
        });
    };
    Ok(super::radicle_change::ChangeCobOperation {
        id: identities.id.as_str().to_owned(),
        actor: identities.actor.as_str().to_owned(),
        is_delegate: operation.is_delegate,
        actions,
    })
}

pub(crate) fn action_into_compatibility(
    action: Action,
) -> Result<super::radicle_change::ChangeCobAction, Error> {
    Ok(match action {
        Action::Init { state } => super::radicle_change::ChangeCobAction::Init {
            state: init_to_compatibility(state)?,
        },
        Action::SetArtifact {
            artifact,
            content,
            basis,
        } => super::radicle_change::ChangeCobAction::SetArtifact {
            artifact,
            content,
            basis,
        },
        Action::AttachEvidence { evidence } => {
            super::radicle_change::ChangeCobAction::AttachEvidence { evidence }
        }
        Action::SetLifecycle {
            status,
            subject_root,
            evidence,
            basis,
        } => super::radicle_change::ChangeCobAction::SetLifecycle {
            status,
            subject_root,
            evidence,
            basis,
        },
    })
}

fn init_to_compatibility(init: Init) -> Result<super::radicle_change::ChangeCobInit, Error> {
    if init.schema != super::radicle_change::CHANGE_COLLABORATION_SCHEMA_V1 {
        return Ok(super::radicle_change::ChangeCobInit {
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
    Ok(super::radicle_change::ChangeCobInit {
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

fn admit_collaboration_init(init: &Init) -> Result<AdmittedInit, Error> {
    Ok(AdmittedInit {
        change_id: super::nominal_identity::ChangeId::parse(
            &init.change_id,
            "collaboration.change_id",
        )
        .map_err(nominal_error)?,
        workspace_id: super::nominal_identity::WorkspaceId::parse(
            &init.workspace_id,
            "collaboration.workspace_id",
        )
        .map_err(nominal_error)?,
        policy_hash: super::nominal_identity::PolicyDigest::parse_compatible(
            &init.policy_hash,
            super::nominal_identity::RAW_BLAKE3_PROFILE,
            "collaboration.policy_hash",
        )
        .map_err(nominal_error)?,
        delegates: admit_actor_set(&init.delegates, "collaboration.delegates")?,
    })
}

fn admit_operation(operation: &Operation) -> Result<AdmittedOperation, Error> {
    Ok(AdmittedOperation {
        id: super::nominal_identity::OperationId::parse(
            &operation.id,
            "collaboration.operation_id",
        )
        .map_err(nominal_error)?,
        actor: super::nominal_identity::ActorId::parse(&operation.actor, "collaboration.actor_id")
            .map_err(nominal_error)?,
    })
}

fn admit_collaboration_state(state: &State) -> Result<AdmittedState, Error> {
    Ok(AdmittedState {
        change_id: super::nominal_identity::ChangeId::parse(
            &state.change_id,
            "collaboration.change_id",
        )
        .map_err(nominal_error)?,
        workspace_id: super::nominal_identity::WorkspaceId::parse(
            &state.workspace_id,
            "collaboration.workspace_id",
        )
        .map_err(nominal_error)?,
        policy_hash: super::nominal_identity::PolicyDigest::parse_compatible(
            &state.policy_hash,
            super::nominal_identity::RAW_BLAKE3_PROFILE,
            "collaboration.policy_hash",
        )
        .map_err(nominal_error)?,
        creator: super::nominal_identity::ActorId::parse(&state.creator, "collaboration.creator")
            .map_err(nominal_error)?,
        delegates: admit_actor_set(&state.delegates, "collaboration.delegates")?,
        operations: state
            .operations
            .iter()
            .map(|value| {
                super::nominal_identity::OperationId::parse(value, "collaboration.operations")
                    .map_err(nominal_error)
            })
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?,
    })
}

fn admit_actor_set(
    values: &std::collections::BTreeSet<String>,
    field: &'static str,
) -> Result<std::collections::BTreeSet<super::nominal_identity::ActorId>, Error> {
    values
        .iter()
        .map(|value| super::nominal_identity::ActorId::parse(value, field).map_err(nominal_error))
        .collect()
}

fn nominal_error(issue: super::nominal_identity::IdentityIssue) -> Error {
    super::radicle_change::ChangeCobError::InvalidField {
        field: issue.field.to_owned(),
        reason: issue.code.to_owned(),
    }
}

fn compatibility_base_revision(
    schema: &str,
    revision: &super::radicle_change::ChangeBaseRevision,
) -> Result<String, Error> {
    if schema == super::radicle_change::RADICLE_CHANGE_COB_SCHEMA_V1 {
        if revision.kind != BASE_REVISION_KIND_GIT_OID {
            return Err(super::radicle_change::ChangeCobError::InvalidField {
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

    const CHANGE_ID: &str = "backend-neutral";
    const WORKSPACE_ID: &str = "workspace:example";
    const BASE_REVISION_KIND: &str = "opaque";
    const BASE_REVISION_VALUE: &str = "revision-one";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ACTOR: &str = "actor:example";
    const ROOT_OPERATION: &str = "operation:root";
    const LEGACY_GIT_OID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn neutral_init() -> Init {
        Init::new(
            CHANGE_ID,
            WORKSPACE_ID,
            super::super::radicle_change::ChangeBaseRevision::new(
                BASE_REVISION_KIND,
                BASE_REVISION_VALUE,
            )
            .unwrap(),
            POLICY_HASH,
            std::collections::BTreeMap::from([(
                super::super::radicle_change::ChangeArtifactId::proposal(),
                "# Proposal\n".to_string(),
            )]),
        )
    }

    fn neutral_state() -> State {
        initialize_change_collaboration(Operation {
            id: ROOT_OPERATION.to_string(),
            actor: ACTOR.to_string(),
            is_delegate: false,
            actions: vec![Action::Init {
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
            .get(&super::super::radicle_change::ChangeArtifactId::proposal())
            .unwrap()
            .head_ids();
        for (id, content) in [
            ("operation:left", "# Left\n"),
            ("operation:right", "# Right\n"),
        ] {
            apply_change_collaboration_operation(
                &mut state,
                Operation {
                    id: id.to_string(),
                    actor: ACTOR.to_string(),
                    is_delegate: false,
                    actions: vec![Action::SetArtifact {
                        artifact: super::super::radicle_change::ChangeArtifactId::proposal(),
                        content: Some(content.to_string()),
                        basis: root_head.clone(),
                    }],
                },
            )
            .unwrap();
        }
        assert!(matches!(
            materialize_change_collaboration(&state),
            Err(super::super::radicle_change::ChangeCobError::ArtifactConflict { .. })
        ));
    }

    // r[verify cairn.collaboration_backend.semantic_core.legacy]
    // r[verify cairn.radicle_cob_only.compatibility.legacy]
    // r[verify cairn.radicle_cob_only.authority.legacy_hash]
    #[test]
    fn legacy_radicle_hashes_use_the_original_canonical_shape() {
        let legacy = super::super::radicle_change::ChangeCobState {
            schema: crate::RADICLE_CHANGE_COB_SCHEMA_V1.to_string(),
            change_id: CHANGE_ID.to_string(),
            repository_id: "rad:z3example".to_string(),
            base_git_oid: LEGACY_GIT_OID.to_string(),
            policy_hash: POLICY_HASH.to_string(),
            creator: ACTOR.to_string(),
            delegates: std::collections::BTreeSet::new(),
            artifacts: std::collections::BTreeMap::from([(
                super::super::radicle_change::ChangeArtifactId::proposal(),
                super::super::radicle_change::ChangeRevisionSet {
                    revisions: std::collections::BTreeMap::from([(
                        LEGACY_GIT_OID.to_string(),
                        super::super::radicle_change::ChangeRevision {
                            actor: ACTOR.to_string(),
                            basis: std::collections::BTreeSet::new(),
                            value: Some("# Proposal\n".to_string()),
                        },
                    )]),
                },
            )]),
            evidence: std::collections::BTreeMap::new(),
            lifecycle: super::super::radicle_change::ChangeRevisionSet {
                revisions: std::collections::BTreeMap::new(),
            },
            operations: std::collections::BTreeSet::from([LEGACY_GIT_OID.to_string()]),
        };
        let expected_state_hash =
            crate::canonical_json_hash(&serde_json::to_value(&legacy).unwrap());
        assert_eq!(
            super::super::radicle_change::change_cob_state_hash(&legacy).unwrap(),
            expected_state_hash
        );

        let expected_content_root = crate::canonical_json_hash(&serde_json::json!({
            "schema": crate::RADICLE_CHANGE_COB_SCHEMA_V1,
            "change_id": CHANGE_ID,
            "repository_id": "rad:z3example",
            "base_git_oid": LEGACY_GIT_OID,
            "policy_hash": POLICY_HASH,
            "creator": ACTOR,
            "artifacts": { "proposal": "# Proposal\n" },
        }));
        assert_eq!(
            super::super::radicle_change::change_cob_content_root(&legacy).unwrap(),
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
        assert!(
            super::super::radicle_change::ChangeBaseRevision::new("bad:kind", "value").is_err()
        );
        let mut init = neutral_init();
        init.schema = crate::RADICLE_CHANGE_COB_SCHEMA_V1.to_string();
        assert!(validate_change_collaboration_init(&init).is_err());
    }

    #[test]
    fn generic_action_json_uses_transport_neutral_fields() {
        let value = action_json(&Action::Init {
            state: neutral_init(),
        })
        .unwrap();
        assert!(value["state"].get("workspace_id").is_some());
        assert!(value["state"].get("base_revision").is_some());
        assert!(value.to_string().find("radicle").is_none());
        assert!(value.to_string().find("base_git_oid").is_none());
    }
}
