// r[impl cairn.collaboration_backend.semantic_core]

pub type LocalSnapshot = super::radicle_checkout::ChangeCobLocalSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub change_id: String,
    pub workspace_id: String,
    pub base_revision: super::radicle_change::ChangeBaseRevision,
    pub policy_hash: String,
    pub state_hash: String,
    pub content_root: Option<String>,
    pub content_conflicts: Vec<String>,
    pub lifecycle: super::radicle_change::ChangeLifecycleView,
    pub evidence_count: usize,
    pub current_evidence_count: usize,
    pub files: Vec<super::radicle_checkout::ChangeCheckoutFile>,
    pub destination: super::radicle_checkout::ChangeCheckoutDestination,
    pub blockers: Vec<String>,
}

impl Plan {
    pub fn checkout_ready(&self) -> bool {
        self.blockers.is_empty()
            && matches!(
                self.destination,
                super::radicle_checkout::ChangeCheckoutDestination::Absent
                    | super::radicle_checkout::ChangeCheckoutDestination::Current
            )
    }

    pub fn already_current(&self) -> bool {
        self.destination == super::radicle_checkout::ChangeCheckoutDestination::Current
            && self.blockers.is_empty()
    }
}

pub fn plan_change_collaboration_checkout(
    state: &super::collaboration_change::State,
    local: LocalSnapshot,
) -> Result<Plan, super::radicle_change::ChangeCobError> {
    let compatibility = super::collaboration_change::state_to_compatibility(state)?;
    let plan = super::radicle_checkout::plan_change_cob_checkout(&compatibility, local)?;
    Ok(Plan {
        change_id: plan.change_id,
        workspace_id: state.workspace_id.clone(),
        base_revision: state.base_revision.clone(),
        policy_hash: plan.policy_hash,
        state_hash: plan.state_hash,
        content_root: plan.content_root,
        content_conflicts: plan.content_conflicts,
        lifecycle: plan.lifecycle,
        evidence_count: plan.evidence_count,
        current_evidence_count: plan.current_evidence_count,
        files: plan.files,
        destination: plan.destination,
        blockers: plan.blockers,
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn state() -> super::super::collaboration_change::State {
        crate::initialize_change_collaboration(crate::ChangeCollaborationOperation {
            id: "operation:root".to_string(),
            actor: "actor:portable".to_string(),
            is_delegate: false,
            actions: vec![crate::ChangeCollaborationAction::Init {
                state: crate::ChangeCollaborationInit::new(
                    "portable-checkout",
                    "workspace:portable",
                    super::super::radicle_change::ChangeBaseRevision::new("opaque", "revision-one")
                        .unwrap(),
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

    // r[verify cairn.collaboration_backend.portable_shell.positive]
    #[test]
    fn neutral_plan_supports_absent_and_exact_destinations() {
        let state = state();
        let absent = plan_change_collaboration_checkout(&state, LocalSnapshot::absent()).unwrap();
        assert!(absent.checkout_ready());
        assert_eq!(
            absent.destination,
            super::super::radicle_checkout::ChangeCheckoutDestination::Absent
        );
        assert_eq!(absent.base_revision.kind, "opaque");

        let exact = plan_change_collaboration_checkout(
            &state,
            LocalSnapshot {
                exists: true,
                files: std::collections::BTreeMap::from([(
                    "proposal.md".to_string(),
                    "# Proposal\n".to_string(),
                )]),
                issues: Vec::new(),
            },
        )
        .unwrap();
        assert!(exact.already_current());
    }

    // r[verify cairn.collaboration_backend.portable_shell.negative]
    #[test]
    fn neutral_plan_blocks_divergent_destination() {
        let plan = plan_change_collaboration_checkout(
            &state(),
            LocalSnapshot {
                exists: true,
                files: std::collections::BTreeMap::from([(
                    "proposal.md".to_string(),
                    "# Divergent\n".to_string(),
                )]),
                issues: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(
            plan.destination,
            super::super::radicle_checkout::ChangeCheckoutDestination::Divergent
        );
        assert!(!plan.checkout_ready());
    }
}
