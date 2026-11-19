//! Root API controls for collaboration naming repairs.

const CHANGE_ID: &str = "compatibility-change";
const WORKSPACE_ID: &str = "workspace:compatibility";
const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ACTOR: &str = "actor:compatibility";
const UNAUTHORIZED_ACTOR: &str = "actor:unauthorized";
const PROPOSAL: &str = "# Proposal\n";

fn state() -> cairn_core::ChangeCollaborationState {
    let init = cairn_core::plan_change_collaboration_import(
        cairn_core::ChangeCollaborationImportSnapshot {
            change_id: CHANGE_ID.to_string(),
            workspace_id: WORKSPACE_ID.to_string(),
            base_revision: cairn_core::ChangeBaseRevision::new("opaque", "revision-one")
                .expect("valid base revision"),
            policy_hash: POLICY_HASH.to_string(),
            proposal: Some(PROPOSAL.to_string()),
            design: None,
            tasks: None,
            metadata: None,
            delta_specs: std::collections::BTreeMap::new(),
        },
    )
    .expect("valid import plan");
    cairn_core::initialize_change_collaboration_import(init, ACTOR).expect("valid imported state")
}

fn snapshot(actor: &str) -> cairn_core::ChangeCollaborationUpdateSnapshot {
    cairn_core::ChangeCollaborationUpdateSnapshot {
        change_id: CHANGE_ID.to_string(),
        policy_hash: POLICY_HASH.to_string(),
        actor: actor.to_string(),
        is_delegate: false,
        proposal: Some(PROPOSAL.to_string()),
        design: None,
        tasks: None,
        metadata: None,
        delta_specs: std::collections::BTreeMap::new(),
    }
}

#[test]
fn root_api_preserves_noop_update_contract() {
    let current = state();
    let plan = cairn_core::plan_change_collaboration_update(&current, snapshot(ACTOR))
        .expect("valid no-op plan");
    let _: &cairn_core::ChangeCollaborationUpdatePlan = &plan;
    assert!(plan.is_noop());
    assert!(
        cairn_core::change_collaboration_update_plan_hash(&plan).is_ok(),
        "update plan hash must remain callable"
    );
    let next = cairn_core::apply_planned_change_collaboration_update(&current, &plan)
        .expect("valid no-op apply");
    assert_eq!(next, current);
    let _: fn(&cairn_core::ChangeCobUpdatePlan) -> Result<String, cairn_core::ChangeCobError> =
        cairn_core::change_cob_update_plan_hash;
}

#[test]
fn root_api_rejects_unauthorized_update_actor() {
    let error =
        cairn_core::plan_change_collaboration_update(&state(), snapshot(UNAUTHORIZED_ACTOR))
            .expect_err("unauthorized actor");
    assert!(matches!(
        error,
        cairn_core::ChangeCobError::Unauthorized { .. }
    ));
}

#[test]
fn resolution_root_api_names_remain_callable() {
    let _: fn(
        &cairn_core::ChangeCollaborationState,
    ) -> Result<
        cairn_core::ChangeCollaborationConflictInspection,
        cairn_core::ChangeCobError,
    > = cairn_core::inspect_change_collaboration_conflicts;
    let _: fn(
        &cairn_core::ChangeCollaborationState,
        &cairn_core::ChangeArtifactId,
        &str,
    ) -> Result<cairn_core::ChangeArtifactConflictValue, cairn_core::ChangeCobError> =
        cairn_core::inspect_change_collaboration_conflict_value;
    let _: fn(
        &cairn_core::ChangeCollaborationState,
        cairn_core::ChangeCollaborationResolutionSnapshot,
    )
        -> Result<cairn_core::ChangeCollaborationResolutionPlan, cairn_core::ChangeCobError> =
        cairn_core::plan_change_collaboration_resolution;
    let _: fn(
        &cairn_core::ChangeCollaborationState,
        &cairn_core::ChangeCollaborationResolutionPlan,
    ) -> Result<cairn_core::ChangeCollaborationState, cairn_core::ChangeCobError> =
        cairn_core::apply_planned_change_collaboration_resolution;
    let _: fn(
        &cairn_core::ChangeCollaborationResolutionPlan,
    ) -> Result<String, cairn_core::ChangeCobError> =
        cairn_core::change_collaboration_resolution_plan_hash;
    let _: fn(&cairn_core::ChangeCobResolutionPlan) -> Result<String, cairn_core::ChangeCobError> =
        cairn_core::change_cob_resolution_plan_hash;
}
