const IMPORT_PREVIEW_OPERATION_ID: &str = "cairn-radicle-import-preview";

// r[impl cairn.radicle_import_cli.lossless_plan]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCobImportSnapshot {
    pub change_id: String,
    pub repository_id: String,
    pub base_git_oid: String,
    pub policy_hash: String,
    pub proposal: Option<String>,
    pub design: Option<String>,
    pub tasks: Option<String>,
    pub metadata: Option<String>,
    pub delta_specs: std::collections::BTreeMap<String, String>,
}

pub fn plan_change_cob_import(
    snapshot: ChangeCobImportSnapshot,
) -> Result<super::radicle_change::ChangeCobInit, super::radicle_change::ChangeCobError> {
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
    let base_revision = super::radicle_change::ChangeBaseRevision::new(
        super::collaboration_change::BASE_REVISION_KIND_GIT_OID,
        snapshot.base_git_oid,
    )?
    .encode()?;
    let init = super::radicle_change::ChangeCobInit {
        schema: super::radicle_change::CHANGE_COLLABORATION_SCHEMA_V1.to_string(),
        change_id: snapshot.change_id,
        repository_id: snapshot.repository_id,
        base_git_oid: base_revision,
        policy_hash: snapshot.policy_hash,
        delegates: std::collections::BTreeSet::new(),
        artifacts,
    };
    super::radicle_change::validate_change_cob_init(&init)?;
    Ok(init)
}

pub fn change_cob_import_content_root(
    init: &super::radicle_change::ChangeCobInit,
    actor: &str,
) -> Result<String, super::radicle_change::ChangeCobError> {
    let state =
        super::radicle_change::initialize_change_cob(super::radicle_change::ChangeCobOperation {
            id: IMPORT_PREVIEW_OPERATION_ID.to_string(),
            actor: actor.to_string(),
            is_delegate: true,
            actions: vec![super::radicle_change::ChangeCobAction::Init {
                state: init.clone(),
            }],
        })?;
    super::radicle_change::change_cob_content_root(&state)
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

#[cfg(test)]
mod tests {

    use super::*;

    const CHANGE_ID: &str = "radicle-import-cli";
    const REPOSITORY_ID: &str = "rad:z3example";
    const BASE_GIT_OID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ROOT_OPERATION_ID: &str = "1111111111111111111111111111111111111111";
    const ACTOR: &str = "did:key:z6example";
    const STANDARD_ARTIFACT_COUNT: usize = 4;
    const OVERSIZED_ARTIFACT_BYTES: usize = crate::MAX_ARTIFACT_BYTES + 1;

    fn snapshot() -> ChangeCobImportSnapshot {
        ChangeCobImportSnapshot {
            change_id: CHANGE_ID.to_string(),
            repository_id: REPOSITORY_ID.to_string(),
            base_git_oid: BASE_GIT_OID.to_string(),
            policy_hash: POLICY_HASH.to_string(),
            proposal: Some("# Proposal\n\nExact trailing bytes.  \n".to_string()),
            design: Some("# Design\n".to_string()),
            tasks: Some("# Tasks\n\n- [ ] Preserve me\n".to_string()),
            metadata: Some("{\n  \"depends_on\": []\n}\n".to_string()),
            delta_specs: std::collections::BTreeMap::from([(
                "radicle-import-cli".to_string(),
                "# Delta\n".to_string(),
            )]),
        }
    }

    // r[verify cairn.radicle_import_cli.lossless_plan.exact]
    #[test]
    fn import_plan_preserves_exact_artifact_bytes_and_materialization() {
        let source = snapshot();
        let init = plan_change_cob_import(source.clone()).unwrap();
        assert_eq!(
            init.artifacts
                .get(&super::super::radicle_change::ChangeArtifactId::proposal()),
            source.proposal.as_ref()
        );
        assert_eq!(
            init.artifacts
                .get(&super::super::radicle_change::ChangeArtifactId::metadata()),
            source.metadata.as_ref()
        );
        let spec_id =
            super::super::radicle_change::ChangeArtifactId::delta_spec("radicle-import-cli")
                .unwrap();
        assert_eq!(
            init.artifacts.get(&spec_id),
            source.delta_specs.get("radicle-import-cli")
        );

        let expected_content_root = change_cob_import_content_root(&init, ACTOR).unwrap();
        let state = super::super::radicle_change::initialize_change_cob(
            super::super::radicle_change::ChangeCobOperation {
                id: ROOT_OPERATION_ID.to_string(),
                actor: ACTOR.to_string(),
                is_delegate: false,
                actions: vec![super::super::radicle_change::ChangeCobAction::Init { state: init }],
            },
        )
        .unwrap();
        assert_eq!(
            super::super::radicle_change::change_cob_content_root(&state).unwrap(),
            expected_content_root
        );
        let files = crate::materialize_change_cob(&state).unwrap();
        assert_eq!(
            files.len(),
            source.delta_specs.len() + STANDARD_ARTIFACT_COUNT
        );
        assert_eq!(
            files
                .iter()
                .find(|file| file.path == "proposal.md")
                .map(|file| file.content.as_str()),
            source.proposal.as_deref()
        );
        assert_eq!(
            state.lifecycle.head_ids(),
            std::collections::BTreeSet::from([ROOT_OPERATION_ID.to_string()])
        );
    }

    // r[verify cairn.radicle_import_cli.lossless_plan.exact]
    #[test]
    fn import_plan_rejects_malformed_spec_ids_and_oversized_artifacts() {
        let mut malformed = snapshot();
        malformed.delta_specs =
            std::collections::BTreeMap::from([("../escape".to_string(), "bad\n".to_string())]);
        assert!(matches!(
            plan_change_cob_import(malformed),
            Err(super::super::radicle_change::ChangeCobError::InvalidArtifactId { .. })
        ));

        let mut oversized = snapshot();
        oversized.metadata = Some("x".repeat(OVERSIZED_ARTIFACT_BYTES));
        assert!(matches!(
            plan_change_cob_import(oversized),
            Err(super::super::radicle_change::ChangeCobError::ByteLimitExceeded { .. })
        ));
    }
}
