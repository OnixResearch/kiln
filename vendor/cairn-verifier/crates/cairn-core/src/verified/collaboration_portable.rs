use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    collaboration_change::{
        change_collaboration_content_root, change_collaboration_state_hash,
        validate_change_collaboration_state, ChangeCollaborationError, ChangeCollaborationState,
    },
    evidence::canonical_json_hash,
    radicle_change::ChangeCobError,
    radicle_change::CHANGE_COLLABORATION_SCHEMA_V1,
};

pub const PORTABLE_COLLABORATION_SCHEMA_V1: &str = "cairn.portable-change-collaboration.v1";
pub const PORTABLE_COLLABORATION_BACKEND_V1: &str = "portable-json.v1";

// r[impl cairn.collaboration_backend.portable_state]

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableChangeCollaboration {
    pub schema: String,
    pub backend: String,
    pub state: ChangeCollaborationState,
    pub content_root: Option<String>,
    pub state_hash: String,
    pub envelope_hash: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PortableChangeCollaborationError {
    #[error("unsupported portable collaboration schema: {schema}")]
    UnsupportedEnvelopeSchema { schema: String },
    #[error("unsupported portable collaboration backend: {backend}")]
    UnsupportedBackend { backend: String },
    #[error("portable collaboration contains non-neutral state schema: {schema}")]
    NonNeutralStateSchema { schema: String },
    #[error(
        "portable collaboration content root mismatch: stated {stated:?}, computed {computed:?}"
    )]
    ContentRootMismatch {
        stated: Option<String>,
        computed: Option<String>,
    },
    #[error("portable collaboration state hash mismatch: stated {stated}, computed {computed}")]
    StateHashMismatch { stated: String, computed: String },
    #[error("portable collaboration envelope hash mismatch: stated {stated}, computed {computed}")]
    EnvelopeHashMismatch { stated: String, computed: String },
    #[error("collaboration state is invalid: {0}")]
    State(#[from] ChangeCollaborationError),
    #[error("portable collaboration serialization failed: {reason}")]
    Serialization { reason: String },
}

pub fn build_portable_change_collaboration(
    state: ChangeCollaborationState,
) -> Result<PortableChangeCollaboration, PortableChangeCollaborationError> {
    require_neutral_state(&state)?;
    validate_change_collaboration_state(&state)?;
    let content_root = portable_content_root(&state)?;
    let state_hash = change_collaboration_state_hash(&state)?;
    let envelope_hash = portable_envelope_hash(&state, &content_root, &state_hash)?;
    Ok(PortableChangeCollaboration {
        schema: PORTABLE_COLLABORATION_SCHEMA_V1.to_string(),
        backend: PORTABLE_COLLABORATION_BACKEND_V1.to_string(),
        state,
        content_root,
        state_hash,
        envelope_hash,
    })
}

pub fn validate_portable_change_collaboration(
    envelope: &PortableChangeCollaboration,
) -> Result<ChangeCollaborationState, PortableChangeCollaborationError> {
    if envelope.schema != PORTABLE_COLLABORATION_SCHEMA_V1 {
        return Err(
            PortableChangeCollaborationError::UnsupportedEnvelopeSchema {
                schema: envelope.schema.clone(),
            },
        );
    }
    if envelope.backend != PORTABLE_COLLABORATION_BACKEND_V1 {
        return Err(PortableChangeCollaborationError::UnsupportedBackend {
            backend: envelope.backend.clone(),
        });
    }
    require_neutral_state(&envelope.state)?;
    validate_change_collaboration_state(&envelope.state)?;
    let content_root = portable_content_root(&envelope.state)?;
    if content_root != envelope.content_root {
        return Err(PortableChangeCollaborationError::ContentRootMismatch {
            stated: envelope.content_root.clone(),
            computed: content_root,
        });
    }
    let state_hash = change_collaboration_state_hash(&envelope.state)?;
    if state_hash != envelope.state_hash {
        return Err(PortableChangeCollaborationError::StateHashMismatch {
            stated: envelope.state_hash.clone(),
            computed: state_hash,
        });
    }
    let envelope_hash = portable_envelope_hash(
        &envelope.state,
        &envelope.content_root,
        &envelope.state_hash,
    )?;
    if envelope_hash != envelope.envelope_hash {
        return Err(PortableChangeCollaborationError::EnvelopeHashMismatch {
            stated: envelope.envelope_hash.clone(),
            computed: envelope_hash,
        });
    }
    Ok(envelope.state.clone())
}

fn portable_content_root(
    state: &ChangeCollaborationState,
) -> Result<Option<String>, PortableChangeCollaborationError> {
    match change_collaboration_content_root(state) {
        Ok(content_root) => Ok(Some(content_root)),
        Err(ChangeCobError::ArtifactConflict { .. }) => Ok(None),
        Err(error) => Err(PortableChangeCollaborationError::State(error)),
    }
}

fn require_neutral_state(
    state: &ChangeCollaborationState,
) -> Result<(), PortableChangeCollaborationError> {
    if state.schema == CHANGE_COLLABORATION_SCHEMA_V1 {
        return Ok(());
    }
    Err(PortableChangeCollaborationError::NonNeutralStateSchema {
        schema: state.schema.clone(),
    })
}

fn portable_envelope_hash(
    state: &ChangeCollaborationState,
    content_root: &Option<String>,
    state_hash: &str,
) -> Result<String, PortableChangeCollaborationError> {
    let identity = PortableEnvelopeIdentity {
        schema: PORTABLE_COLLABORATION_SCHEMA_V1,
        backend: PORTABLE_COLLABORATION_BACKEND_V1,
        state,
        content_root,
        state_hash,
    };
    let value = serde_json::to_value(identity).map_err(|error| {
        PortableChangeCollaborationError::Serialization {
            reason: error.to_string(),
        }
    })?;
    Ok(canonical_json_hash(&value))
}

#[derive(Serialize)]
struct PortableEnvelopeIdentity<'a> {
    schema: &'a str,
    backend: &'a str,
    state: &'a ChangeCollaborationState,
    content_root: &'a Option<String>,
    state_hash: &'a str,
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::{
        apply_change_collaboration_operation, initialize_change_collaboration, ChangeArtifactId,
        ChangeBaseRevision, ChangeCollaborationAction, ChangeCollaborationInit,
        ChangeCollaborationOperation,
    };

    const CHANGE_ID: &str = "portable-roundtrip";
    const WORKSPACE_ID: &str = "workspace:portable";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ACTOR: &str = "actor:portable";
    const OPERATION_ID: &str = "operation:portable-root";
    const LEFT_OPERATION_ID: &str = "operation:portable-left";
    const RIGHT_OPERATION_ID: &str = "operation:portable-right";

    fn state() -> ChangeCollaborationState {
        initialize_change_collaboration(ChangeCollaborationOperation {
            id: OPERATION_ID.to_string(),
            actor: ACTOR.to_string(),
            is_delegate: false,
            actions: vec![ChangeCollaborationAction::Init {
                state: ChangeCollaborationInit::new(
                    CHANGE_ID,
                    WORKSPACE_ID,
                    ChangeBaseRevision::new("opaque", "revision-one").unwrap(),
                    POLICY_HASH,
                    BTreeMap::from([(ChangeArtifactId::proposal(), "# Proposal\n".to_string())]),
                ),
            }],
        })
        .unwrap()
    }

    // r[verify cairn.collaboration_backend.portable_state.roundtrip]
    #[test]
    fn portable_round_trip_preserves_exact_state_and_hashes() {
        let state = state();
        let envelope = build_portable_change_collaboration(state.clone()).unwrap();
        let legacy_identity = serde_json::json!({
            "schema": PORTABLE_COLLABORATION_SCHEMA_V1,
            "backend": PORTABLE_COLLABORATION_BACKEND_V1,
            "state": &state,
            "content_root": change_collaboration_content_root(&state).unwrap(),
            "state_hash": change_collaboration_state_hash(&state).unwrap(),
        });
        assert_eq!(
            envelope.envelope_hash,
            canonical_json_hash(&legacy_identity)
        );
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let decoded: PortableChangeCollaboration = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            validate_portable_change_collaboration(&decoded).unwrap(),
            state
        );
        assert_eq!(
            decoded.content_root,
            Some(change_collaboration_content_root(&state).unwrap())
        );
        assert_eq!(
            decoded.state_hash,
            change_collaboration_state_hash(&state).unwrap()
        );
    }

    #[test]
    fn portable_round_trip_preserves_conflicted_state_without_claiming_content_root() {
        let mut state = state();
        let basis = BTreeSet::from([OPERATION_ID.to_string()]);
        for (operation_id, content) in [
            (LEFT_OPERATION_ID, "# Left\n"),
            (RIGHT_OPERATION_ID, "# Right\n"),
        ] {
            apply_change_collaboration_operation(
                &mut state,
                ChangeCollaborationOperation {
                    id: operation_id.to_string(),
                    actor: ACTOR.to_string(),
                    is_delegate: false,
                    actions: vec![ChangeCollaborationAction::SetArtifact {
                        artifact: ChangeArtifactId::proposal(),
                        content: Some(content.to_string()),
                        basis: basis.clone(),
                    }],
                },
            )
            .unwrap();
        }
        let envelope = build_portable_change_collaboration(state.clone()).unwrap();
        assert!(envelope.content_root.is_none());
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let decoded: PortableChangeCollaboration = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            validate_portable_change_collaboration(&decoded).unwrap(),
            state
        );
    }

    // r[verify cairn.collaboration_backend.portable_state.tamper]
    #[test]
    fn portable_validation_rejects_labels_hashes_and_state_tampering() {
        let envelope = build_portable_change_collaboration(state()).unwrap();
        let mut cases = Vec::new();

        let mut schema = envelope.clone();
        schema.schema = "unsupported".to_string();
        cases.push(schema);

        let mut backend = envelope.clone();
        backend.backend = "unsupported".to_string();
        cases.push(backend);

        let mut content = envelope.clone();
        content.content_root = Some(POLICY_HASH.to_string());
        cases.push(content);

        let mut absent_content = envelope.clone();
        absent_content.content_root = None;
        cases.push(absent_content);

        let mut state_hash = envelope.clone();
        state_hash.state_hash = POLICY_HASH.to_string();
        cases.push(state_hash);

        let mut envelope_hash = envelope.clone();
        envelope_hash.envelope_hash = POLICY_HASH.to_string();
        cases.push(envelope_hash);

        let mut state_value = envelope;
        state_value.state.workspace_id = "workspace:tampered".to_string();
        cases.push(state_value);

        for case in cases {
            assert!(validate_portable_change_collaboration(&case).is_err());
        }

        let mut recomputed = build_portable_change_collaboration(state()).unwrap();
        recomputed.state.operations.clear();
        recomputed.content_root =
            Some(change_collaboration_content_root(&recomputed.state).unwrap());
        recomputed.state_hash = change_collaboration_state_hash(&recomputed.state).unwrap();
        recomputed.envelope_hash = portable_envelope_hash(
            &recomputed.state,
            &recomputed.content_root,
            &recomputed.state_hash,
        )
        .unwrap();
        assert!(matches!(
            validate_portable_change_collaboration(&recomputed),
            Err(PortableChangeCollaborationError::State(_))
        ));
    }
}
