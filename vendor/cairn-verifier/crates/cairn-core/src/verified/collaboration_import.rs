const IMPORT_OPERATION_DOMAIN: &str = "cairn-change-collaboration-import-v1";

// r[impl cairn.collaboration_backend.semantic_core]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub change_id: String,
    pub workspace_id: String,
    pub base_revision: super::radicle_change::ChangeBaseRevision,
    pub policy_hash: String,
    pub proposal: Option<String>,
    pub design: Option<String>,
    pub tasks: Option<String>,
    pub metadata: Option<String>,
    pub delta_specs: std::collections::BTreeMap<String, String>,
}

pub fn plan(
    snapshot: Snapshot,
) -> Result<super::collaboration_change::Init, super::collaboration_change::Error> {
    let mut artifacts = std::collections::BTreeMap::new();
    insert_present_artifact(
        &mut artifacts,
        super::radicle_change::ChangeArtifactId::proposal(),
        snapshot.proposal,
    );
    insert_present_artifact(
        &mut artifacts,
        super::radicle_change::ChangeArtifactId::design(),
        snapshot.design,
    );
    insert_present_artifact(
        &mut artifacts,
        super::radicle_change::ChangeArtifactId::tasks(),
        snapshot.tasks,
    );
    insert_present_artifact(
        &mut artifacts,
        super::radicle_change::ChangeArtifactId::metadata(),
        snapshot.metadata,
    );
    for (spec_id, content) in snapshot.delta_specs {
        artifacts.insert(
            super::radicle_change::ChangeArtifactId::delta_spec(&spec_id)?,
            content,
        );
    }
    let init = super::collaboration_change::Init::new(
        snapshot.change_id,
        snapshot.workspace_id,
        snapshot.base_revision,
        snapshot.policy_hash,
        artifacts,
    );
    super::collaboration_change::validate_change_collaboration_init(&init)?;
    Ok(init)
}

pub fn initialize(
    init: super::collaboration_change::Init,
    actor: impl Into<String>,
) -> Result<super::collaboration_change::State, super::collaboration_change::Error> {
    let actor = actor.into();
    let id = operation_id(&init, &actor)?;
    super::collaboration_change::initialize_change_collaboration(
        super::collaboration_change::Operation {
            id,
            actor,
            is_delegate: false,
            actions: vec![super::collaboration_change::Action::Init { state: init }],
        },
    )
}

pub fn operation_id(
    init: &super::collaboration_change::Init,
    actor: &str,
) -> Result<String, super::collaboration_change::Error> {
    let identity = OperationIdentity {
        domain: IMPORT_OPERATION_DOMAIN,
        actor,
        init,
    };
    let value = serde_json::to_value(identity).map_err(|error| {
        super::collaboration_change::Error::Serialization {
            reason: error.to_string(),
        }
    })?;
    let hash = super::evidence::canonical_json_hash(&value);
    let operation =
        super::nominal_identity::OperationId::parse(&hash, "collaboration.import_operation_id")
            .map_err(|issue| super::collaboration_change::Error::InvalidField {
                field: issue.field.to_owned(),
                reason: issue.code.to_owned(),
            })?;
    debug_assert_eq!(operation.as_str(), hash);
    Ok(operation.as_str().to_owned())
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

#[derive(serde::Serialize)]
struct OperationIdentity<'a> {
    domain: &'a str,
    actor: &'a str,
    init: &'a super::collaboration_change::Init,
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANGE_ID: &str = "portable-import";
    const WORKSPACE_ID: &str = "workspace:portable";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ACTOR: &str = "actor:portable";
    const OVERSIZED_ARTIFACT_BYTES: usize = crate::MAX_ARTIFACT_BYTES + 1;

    fn snapshot() -> Snapshot {
        Snapshot {
            change_id: CHANGE_ID.to_string(),
            workspace_id: WORKSPACE_ID.to_string(),
            base_revision: super::super::radicle_change::ChangeBaseRevision::new(
                "opaque",
                "revision-one",
            )
            .unwrap(),
            policy_hash: POLICY_HASH.to_string(),
            proposal: Some("# Proposal\n".to_string()),
            design: Some("# Design\n".to_string()),
            tasks: None,
            metadata: None,
            delta_specs: std::collections::BTreeMap::from([(
                "portable-import".to_string(),
                "# Delta\n".to_string(),
            )]),
        }
    }

    // r[verify cairn.collaboration_backend.semantic_core.neutral]
    #[test]
    fn neutral_plan_preserves_exact_artifacts_and_has_stable_operation_identity() {
        let init = plan(snapshot()).unwrap();
        let first = operation_id(&init, ACTOR).unwrap();
        let second = operation_id(&init, ACTOR).unwrap();
        assert_eq!(first, second);
        let state = initialize(init, ACTOR).unwrap();
        let files = crate::materialize_change_collaboration(&state).unwrap();
        assert!(files
            .iter()
            .any(|file| file.path == "proposal.md" && file.content == "# Proposal\n"));
        assert!(files.iter().any(|file| {
            file.path == "specs/portable-import/spec.md" && file.content == "# Delta\n"
        }));
    }

    #[test]
    fn neutral_plan_rejects_unsafe_ids_and_oversized_artifacts() {
        let mut invalid_spec = snapshot();
        invalid_spec
            .delta_specs
            .insert("../escape".to_string(), "# Bad\n".to_string());
        assert!(plan(invalid_spec).is_err());

        let mut oversized = snapshot();
        oversized.proposal = Some("x".repeat(OVERSIZED_ARTIFACT_BYTES));
        assert!(plan(oversized).is_err());
    }
}
