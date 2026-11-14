use super::{
    collaboration_change::{state_to_compatibility, ChangeCollaborationState},
    radicle_change::{ChangeBaseRevision, ChangeCobError, ChangeLifecycleView},
    radicle_checkout::{
        plan_change_cob_checkout, ChangeCheckoutDestination, ChangeCheckoutFile,
        ChangeCobLocalSnapshot,
    },
};

// r[impl cairn.collaboration_backend.semantic_core]

pub type ChangeCollaborationLocalSnapshot = ChangeCobLocalSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCollaborationCheckoutPlan {
    pub change_id: String,
    pub workspace_id: String,
    pub base_revision: ChangeBaseRevision,
    pub policy_hash: String,
    pub state_hash: String,
    pub content_root: Option<String>,
    pub content_conflicts: Vec<String>,
    pub lifecycle: ChangeLifecycleView,
    pub evidence_count: usize,
    pub current_evidence_count: usize,
    pub files: Vec<ChangeCheckoutFile>,
    pub destination: ChangeCheckoutDestination,
    pub blockers: Vec<String>,
}

impl ChangeCollaborationCheckoutPlan {
    pub fn checkout_ready(&self) -> bool {
        self.blockers.is_empty()
            && matches!(
                self.destination,
                ChangeCheckoutDestination::Absent | ChangeCheckoutDestination::Current
            )
    }

    pub fn already_current(&self) -> bool {
        self.destination == ChangeCheckoutDestination::Current && self.blockers.is_empty()
    }
}

pub fn plan_change_collaboration_checkout(
    state: &ChangeCollaborationState,
    local: ChangeCollaborationLocalSnapshot,
) -> Result<ChangeCollaborationCheckoutPlan, ChangeCobError> {
    let compatibility = state_to_compatibility(state)?;
    let plan = plan_change_cob_checkout(&compatibility, local)?;
    Ok(ChangeCollaborationCheckoutPlan {
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
    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        initialize_change_collaboration, ChangeArtifactId, ChangeCollaborationAction,
        ChangeCollaborationInit, ChangeCollaborationOperation,
    };

    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn state() -> ChangeCollaborationState {
        initialize_change_collaboration(ChangeCollaborationOperation {
            id: "operation:root".to_string(),
            actor: "actor:portable".to_string(),
            is_delegate: false,
            actions: vec![ChangeCollaborationAction::Init {
                state: ChangeCollaborationInit::new(
                    "portable-checkout",
                    "workspace:portable",
                    ChangeBaseRevision::new("opaque", "revision-one").unwrap(),
                    POLICY_HASH,
                    BTreeMap::from([(ChangeArtifactId::proposal(), "# Proposal\n".to_string())]),
                ),
            }],
        })
        .unwrap()
    }

    // r[verify cairn.collaboration_backend.portable_shell.positive]
    #[test]
    fn neutral_checkout_plan_supports_absent_and_exact_destinations() {
        let state = state();
        let absent =
            plan_change_collaboration_checkout(&state, ChangeCollaborationLocalSnapshot::absent())
                .unwrap();
        assert!(absent.checkout_ready());
        assert_eq!(absent.destination, ChangeCheckoutDestination::Absent);
        assert_eq!(absent.base_revision.kind, "opaque");

        let exact = plan_change_collaboration_checkout(
            &state,
            ChangeCollaborationLocalSnapshot {
                exists: true,
                files: BTreeMap::from([("proposal.md".to_string(), "# Proposal\n".to_string())]),
                issues: Vec::new(),
            },
        )
        .unwrap();
        assert!(exact.already_current());
    }

    // r[verify cairn.collaboration_backend.portable_shell.negative]
    #[test]
    fn neutral_checkout_plan_blocks_divergent_destination() {
        let plan = plan_change_collaboration_checkout(
            &state(),
            ChangeCollaborationLocalSnapshot {
                exists: true,
                files: BTreeMap::from([("proposal.md".to_string(), "# Divergent\n".to_string())]),
                issues: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(plan.destination, ChangeCheckoutDestination::Divergent);
        assert!(!plan.checkout_ready());
    }
}
