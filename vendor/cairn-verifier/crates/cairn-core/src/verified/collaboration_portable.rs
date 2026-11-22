pub const PORTABLE_COLLABORATION_SCHEMA_V1: &str = "cairn.portable-change-collaboration.v1";
pub const PORTABLE_COLLABORATION_BACKEND_V1: &str = "portable-json.v1";

// r[impl cairn.collaboration_backend.portable_state]

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Envelope {
    pub schema: String,
    pub backend: String,
    pub state: super::collaboration_change::State,
    pub content_root: Option<String>,
    pub state_hash: String,
    pub envelope_hash: String,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Failure {
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
    State(#[from] super::collaboration_change::Error),
    #[error("portable collaboration serialization failed: {reason}")]
    Serialization { reason: String },
}

pub fn build(state: super::collaboration_change::State) -> Result<Envelope, Failure> {
    require_neutral_state(&state)?;
    super::collaboration_change::validate_change_collaboration_state(&state)?;
    let content_root = derive_content_root(&state)?;
    let state_hash = super::collaboration_change::change_collaboration_state_hash(&state)?;
    let envelope_hash = hash_envelope(&state, &content_root, &state_hash)?;
    Ok(Envelope {
        schema: PORTABLE_COLLABORATION_SCHEMA_V1.to_string(),
        backend: PORTABLE_COLLABORATION_BACKEND_V1.to_string(),
        state,
        content_root,
        state_hash,
        envelope_hash,
    })
}

pub fn validate(envelope: &Envelope) -> Result<super::collaboration_change::State, Failure> {
    if envelope.schema != PORTABLE_COLLABORATION_SCHEMA_V1 {
        return Err(Failure::UnsupportedEnvelopeSchema {
            schema: envelope.schema.clone(),
        });
    }
    if envelope.backend != PORTABLE_COLLABORATION_BACKEND_V1 {
        return Err(Failure::UnsupportedBackend {
            backend: envelope.backend.clone(),
        });
    }
    require_neutral_state(&envelope.state)?;
    super::collaboration_change::validate_change_collaboration_state(&envelope.state)?;
    let content_root = derive_content_root(&envelope.state)?;
    if content_root != envelope.content_root {
        return Err(Failure::ContentRootMismatch {
            stated: envelope.content_root.clone(),
            computed: content_root,
        });
    }
    let state_hash = super::collaboration_change::change_collaboration_state_hash(&envelope.state)?;
    if state_hash != envelope.state_hash {
        return Err(Failure::StateHashMismatch {
            stated: envelope.state_hash.clone(),
            computed: state_hash,
        });
    }
    let envelope_hash = hash_envelope(
        &envelope.state,
        &envelope.content_root,
        &envelope.state_hash,
    )?;
    if envelope_hash != envelope.envelope_hash {
        return Err(Failure::EnvelopeHashMismatch {
            stated: envelope.envelope_hash.clone(),
            computed: envelope_hash,
        });
    }
    Ok(envelope.state.clone())
}

fn derive_content_root(
    state: &super::collaboration_change::State,
) -> Result<Option<String>, Failure> {
    match super::collaboration_change::change_collaboration_content_root(state) {
        Ok(content_root) => Ok(Some(content_root)),
        Err(super::radicle_change::ChangeCobError::ArtifactConflict { .. }) => Ok(None),
        Err(error) => Err(Failure::State(error)),
    }
}

fn require_neutral_state(state: &super::collaboration_change::State) -> Result<(), Failure> {
    if state.schema == super::radicle_change::CHANGE_COLLABORATION_SCHEMA_V1 {
        return Ok(());
    }
    Err(Failure::NonNeutralStateSchema {
        schema: state.schema.clone(),
    })
}

fn hash_envelope(
    state: &super::collaboration_change::State,
    content_root: &Option<String>,
    state_hash: &str,
) -> Result<String, Failure> {
    let identity = EnvelopeIdentity {
        schema: PORTABLE_COLLABORATION_SCHEMA_V1,
        backend: PORTABLE_COLLABORATION_BACKEND_V1,
        state,
        content_root,
        state_hash,
    };
    let value = serde_json::to_value(identity).map_err(|error| Failure::Serialization {
        reason: error.to_string(),
    })?;
    Ok(super::evidence::canonical_json_hash(&value))
}

#[derive(serde::Serialize)]
struct EnvelopeIdentity<'a> {
    schema: &'a str,
    backend: &'a str,
    state: &'a super::collaboration_change::State,
    content_root: &'a Option<String>,
    state_hash: &'a str,
}

#[cfg(test)]
mod tests {

    use super::*;

    const CHANGE_ID: &str = "portable-roundtrip";
    const WORKSPACE_ID: &str = "workspace:portable";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ACTOR: &str = "actor:portable";
    const OPERATION_ID: &str = "operation:portable-root";
    const LEFT_OPERATION_ID: &str = "operation:portable-left";
    const RIGHT_OPERATION_ID: &str = "operation:portable-right";

    fn state() -> super::super::collaboration_change::State {
        crate::initialize_change_collaboration(crate::ChangeCollaborationOperation {
            id: OPERATION_ID.to_string(),
            actor: ACTOR.to_string(),
            is_delegate: false,
            actions: vec![crate::ChangeCollaborationAction::Init {
                state: crate::ChangeCollaborationInit::new(
                    CHANGE_ID,
                    WORKSPACE_ID,
                    crate::ChangeBaseRevision::new("opaque", "revision-one").unwrap(),
                    POLICY_HASH,
                    std::collections::BTreeMap::from([(
                        crate::ChangeArtifactId::proposal(),
                        "# Proposal\n".to_string(),
                    )]),
                ),
            }],
        })
        .unwrap()
    }

    // r[verify cairn.collaboration_backend.portable_state.roundtrip]
    #[test]
    fn round_trip_preserves_exact_state_and_hashes() {
        let state = state();
        let envelope = build(state.clone()).unwrap();
        let legacy_identity = serde_json::json!({
            "schema": PORTABLE_COLLABORATION_SCHEMA_V1,
            "backend": PORTABLE_COLLABORATION_BACKEND_V1,
            "state": &state,
            "content_root": super::super::collaboration_change::change_collaboration_content_root(&state).unwrap(),
            "state_hash": super::super::collaboration_change::change_collaboration_state_hash(&state).unwrap(),
        });
        assert_eq!(
            envelope.envelope_hash,
            super::super::evidence::canonical_json_hash(&legacy_identity)
        );
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let decoded: Envelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(validate(&decoded).unwrap(), state);
        assert_eq!(
            decoded.content_root,
            Some(
                super::super::collaboration_change::change_collaboration_content_root(&state)
                    .unwrap()
            )
        );
        assert_eq!(
            decoded.state_hash,
            super::super::collaboration_change::change_collaboration_state_hash(&state).unwrap()
        );
    }

    #[test]
    fn round_trip_preserves_conflicted_state_without_claiming_content_root() {
        let mut state = state();
        let basis = std::collections::BTreeSet::from([OPERATION_ID.to_string()]);
        for (operation_id, content) in [
            (LEFT_OPERATION_ID, "# Left\n"),
            (RIGHT_OPERATION_ID, "# Right\n"),
        ] {
            crate::apply_change_collaboration_operation(
                &mut state,
                crate::ChangeCollaborationOperation {
                    id: operation_id.to_string(),
                    actor: ACTOR.to_string(),
                    is_delegate: false,
                    actions: vec![crate::ChangeCollaborationAction::SetArtifact {
                        artifact: crate::ChangeArtifactId::proposal(),
                        content: Some(content.to_string()),
                        basis: basis.clone(),
                    }],
                },
            )
            .unwrap();
        }
        let envelope = build(state.clone()).unwrap();
        assert!(envelope.content_root.is_none());
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let decoded: Envelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(validate(&decoded).unwrap(), state);
    }

    // r[verify cairn.collaboration_backend.portable_state.tamper]
    #[test]
    fn validation_rejects_labels_hashes_and_state_tampering() {
        let envelope = build(state()).unwrap();
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
            assert!(validate(&case).is_err());
        }

        let mut recomputed = build(state()).unwrap();
        recomputed.state.operations.clear();
        recomputed.content_root = Some(
            super::super::collaboration_change::change_collaboration_content_root(
                &recomputed.state,
            )
            .unwrap(),
        );
        recomputed.state_hash =
            super::super::collaboration_change::change_collaboration_state_hash(&recomputed.state)
                .unwrap();
        recomputed.envelope_hash = hash_envelope(
            &recomputed.state,
            &recomputed.content_root,
            &recomputed.state_hash,
        )
        .unwrap();
        assert!(matches!(validate(&recomputed), Err(Failure::State(_))));
    }
}
