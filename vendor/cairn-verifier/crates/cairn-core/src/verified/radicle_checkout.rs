const BLOCKER_CONTENT_CONFLICT: &str =
    "typed change content is conflicted and cannot be materialized";
const BLOCKER_LIFECYCLE_CONFLICT: &str =
    "typed change lifecycle is conflicted and checkout is not safe";
const BLOCKER_LOCAL_DIVERGENCE: &str =
    "local active-change destination differs from the complete typed materialization";
const BLOCKER_INVALID_ABSENT_SNAPSHOT: &str =
    "local snapshot marks the destination absent but includes observed content";

// r[impl cairn.radicle_status_checkout.checkout_plan]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCobLocalSnapshot {
    pub exists: bool,
    pub files: std::collections::BTreeMap<String, String>,
    pub issues: Vec<String>,
}

impl ChangeCobLocalSnapshot {
    pub fn absent() -> Self {
        Self {
            exists: false,
            files: std::collections::BTreeMap::new(),
            issues: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeCheckoutDestination {
    Absent,
    Current,
    Divergent,
}

impl ChangeCheckoutDestination {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Current => "current",
            Self::Divergent => "divergent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCheckoutFile {
    pub path: String,
    pub content: String,
    pub blake3: String,
    pub byte_length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCobCheckoutPlan {
    pub change_id: String,
    pub repository_id: String,
    pub base_git_oid: String,
    pub policy_hash: String,
    pub state_hash: String,
    pub content_root: Option<String>,
    pub content_conflicts: Vec<String>,
    pub lifecycle: super::radicle_change::ChangeLifecycleView,
    pub evidence_count: usize,
    pub current_evidence_count: usize,
    pub files: Vec<ChangeCheckoutFile>,
    pub destination: ChangeCheckoutDestination,
    pub blockers: Vec<String>,
}

impl ChangeCobCheckoutPlan {
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

pub fn plan_change_cob_checkout(
    state: &super::radicle_change::ChangeCobState,
    mut local: ChangeCobLocalSnapshot,
) -> Result<ChangeCobCheckoutPlan, super::radicle_change::ChangeCobError> {
    local.issues.sort();
    local.issues.dedup();
    let state_hash = super::radicle_change::change_cob_state_hash(state)?;
    let lifecycle = super::radicle_change::change_cob_lifecycle_view(state)?;
    let evidence_count = state.evidence.len();
    let mut blockers = local.issues.clone();
    if matches!(
        &lifecycle,
        super::radicle_change::ChangeLifecycleView::Conflicted { .. }
    ) {
        blockers.push(BLOCKER_LIFECYCLE_CONFLICT.to_string());
    }

    let (content_root, content_conflicts, current_evidence_count, materialized) =
        match super::radicle_change::materialize_change_cob(state) {
            Ok(materialized) => (
                Some(super::radicle_change::change_cob_content_root(state)?),
                Vec::new(),
                super::radicle_change::current_change_cob_evidence(state)?.len(),
                materialized,
            ),
            Err(super::radicle_change::ChangeCobError::ArtifactConflict { mut artifacts }) => {
                artifacts.sort();
                artifacts.dedup();
                blockers.push(BLOCKER_CONTENT_CONFLICT.to_string());
                (None, artifacts, 0, Vec::new())
            }
            Err(error) => return Err(error),
        };

    let files = materialized
        .into_iter()
        .map(|file| ChangeCheckoutFile {
            blake3: super::evidence::blake3_hex(file.content.as_bytes()),
            byte_length: file.content.len(),
            path: file.path,
            content: file.content,
        })
        .collect::<Vec<_>>();
    let expected = files
        .iter()
        .map(|file| (file.path.clone(), file.content.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();

    let destination = if !local.exists {
        if local.files.is_empty() && local.issues.is_empty() {
            ChangeCheckoutDestination::Absent
        } else {
            blockers.push(BLOCKER_INVALID_ABSENT_SNAPSHOT.to_string());
            ChangeCheckoutDestination::Divergent
        }
    } else if content_conflicts.is_empty() && local.issues.is_empty() && local.files == expected {
        ChangeCheckoutDestination::Current
    } else {
        blockers.push(BLOCKER_LOCAL_DIVERGENCE.to_string());
        ChangeCheckoutDestination::Divergent
    };

    blockers.sort();
    blockers.dedup();
    Ok(ChangeCobCheckoutPlan {
        change_id: state.change_id.clone(),
        repository_id: state.repository_id.clone(),
        base_git_oid: state.base_git_oid.clone(),
        policy_hash: state.policy_hash.clone(),
        state_hash,
        content_root,
        content_conflicts,
        lifecycle,
        evidence_count,
        current_evidence_count,
        files,
        destination,
        blockers,
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    const CHANGE_ID: &str = "radicle-status-checkout-cli";
    const REPOSITORY_ID: &str = "rad:z3example";
    const BASE_GIT_OID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ROOT_OPERATION: &str = "1111111111111111111111111111111111111111";
    const UPDATE_OPERATION: &str = "2222222222222222222222222222222222222222";
    const CONFLICT_OPERATION: &str = "3333333333333333333333333333333333333333";
    const LIFECYCLE_OPERATION: &str = "4444444444444444444444444444444444444444";
    const LIFECYCLE_CONFLICT_OPERATION: &str = "5555555555555555555555555555555555555555";
    const CREATOR: &str = "z6Mkcreator";
    const PROPOSAL: &str = "# Proposal\n\nExact.  \n";
    const UPDATED_PROPOSAL: &str = "# Proposal\n\nUpdated.\n";
    const CONFLICTING_PROPOSAL: &str = "# Proposal\n\nConcurrent.\n";
    const EXPECTED_FILE_COUNT: usize = 1;

    fn state() -> super::super::radicle_change::ChangeCobState {
        crate::initialize_change_cob(crate::ChangeCobOperation {
            id: ROOT_OPERATION.to_string(),
            actor: CREATOR.to_string(),
            is_delegate: true,
            actions: vec![crate::ChangeCobAction::Init {
                state: crate::ChangeCobInit {
                    schema: crate::RADICLE_CHANGE_COB_SCHEMA_V1.to_string(),
                    change_id: CHANGE_ID.to_string(),
                    repository_id: REPOSITORY_ID.to_string(),
                    base_git_oid: BASE_GIT_OID.to_string(),
                    policy_hash: POLICY_HASH.to_string(),
                    delegates: std::collections::BTreeSet::new(),
                    artifacts: std::collections::BTreeMap::from([(
                        crate::ChangeArtifactId::proposal(),
                        PROPOSAL.to_string(),
                    )]),
                },
            }],
        })
        .expect("valid test state")
    }

    fn set_proposal(operation_id: &str, content: &str) -> crate::ChangeCobOperation {
        crate::ChangeCobOperation {
            id: operation_id.to_string(),
            actor: CREATOR.to_string(),
            is_delegate: false,
            actions: vec![crate::ChangeCobAction::SetArtifact {
                artifact: crate::ChangeArtifactId::proposal(),
                content: Some(content.to_string()),
                basis: std::collections::BTreeSet::from([ROOT_OPERATION.to_string()]),
            }],
        }
    }

    // r[verify cairn.radicle_status_checkout.checkout_plan.safe]
    #[test]
    fn absent_and_exact_destinations_produce_create_and_noop_plans() {
        let state = state();
        let absent = plan_change_cob_checkout(&state, ChangeCobLocalSnapshot::absent())
            .expect("absent checkout plan");
        assert_eq!(absent.destination, ChangeCheckoutDestination::Absent);
        assert!(absent.checkout_ready());
        assert!(!absent.already_current());
        assert_eq!(absent.files.len(), EXPECTED_FILE_COUNT);
        assert_eq!(absent.files[0].content, PROPOSAL);
        assert_eq!(
            absent.content_root,
            Some(
                super::super::radicle_change::change_cob_content_root(&state)
                    .expect("content root")
            )
        );

        let exact = plan_change_cob_checkout(
            &state,
            ChangeCobLocalSnapshot {
                exists: true,
                files: std::collections::BTreeMap::from([(
                    "proposal.md".to_string(),
                    PROPOSAL.to_string(),
                )]),
                issues: Vec::new(),
            },
        )
        .expect("exact checkout plan");
        assert_eq!(exact.destination, ChangeCheckoutDestination::Current);
        assert!(exact.checkout_ready());
        assert!(exact.already_current());
    }

    // r[verify cairn.radicle_status_checkout.checkout_plan.divergent]
    #[test]
    fn divergent_and_unsupported_local_content_is_blocked() {
        let local = ChangeCobLocalSnapshot {
            exists: true,
            files: std::collections::BTreeMap::from([(
                "proposal.md".to_string(),
                UPDATED_PROPOSAL.to_string(),
            )]),
            issues: vec!["unsupported local entry: notes.txt".to_string()],
        };
        let plan = plan_change_cob_checkout(&state(), local).expect("divergent plan");
        assert_eq!(plan.destination, ChangeCheckoutDestination::Divergent);
        assert!(!plan.checkout_ready());
        assert!(plan
            .blockers
            .iter()
            .any(|blocker| blocker.contains("unsupported local entry")));
        assert!(plan
            .blockers
            .iter()
            .any(|blocker| blocker == BLOCKER_LOCAL_DIVERGENCE));
    }

    #[test]
    fn lifecycle_conflict_is_visible_and_blocks_checkout() {
        let mut conflicted = state();
        let content_root = super::super::radicle_change::change_cob_content_root(&conflicted)
            .expect("content root");
        for (operation_id, status) in [
            (
                LIFECYCLE_OPERATION,
                crate::ChangeLifecycleStatus::ReviewRequested,
            ),
            (
                LIFECYCLE_CONFLICT_OPERATION,
                crate::ChangeLifecycleStatus::Open,
            ),
        ] {
            crate::apply_change_cob_operation(
                &mut conflicted,
                crate::ChangeCobOperation {
                    id: operation_id.to_string(),
                    actor: CREATOR.to_string(),
                    is_delegate: false,
                    actions: vec![crate::ChangeCobAction::SetLifecycle {
                        status,
                        subject_root: content_root.clone(),
                        evidence: std::collections::BTreeSet::new(),
                        basis: std::collections::BTreeSet::from([ROOT_OPERATION.to_string()]),
                    }],
                },
            )
            .expect("concurrent lifecycle update");
        }

        let plan = plan_change_cob_checkout(&conflicted, ChangeCobLocalSnapshot::absent())
            .expect("lifecycle conflict status plan");
        assert!(matches!(
            plan.lifecycle,
            super::super::radicle_change::ChangeLifecycleView::Conflicted { .. }
        ));
        assert!(!plan.files.is_empty());
        assert!(!plan.checkout_ready());
        assert!(plan
            .blockers
            .iter()
            .any(|blocker| blocker == BLOCKER_LIFECYCLE_CONFLICT));
    }

    #[test]
    fn content_conflict_is_visible_without_partial_materialization() {
        let mut conflicted = state();
        crate::apply_change_cob_operation(
            &mut conflicted,
            set_proposal(UPDATE_OPERATION, UPDATED_PROPOSAL),
        )
        .expect("first update");
        crate::apply_change_cob_operation(
            &mut conflicted,
            set_proposal(CONFLICT_OPERATION, CONFLICTING_PROPOSAL),
        )
        .expect("concurrent update");

        let plan = plan_change_cob_checkout(&conflicted, ChangeCobLocalSnapshot::absent())
            .expect("conflict status plan");
        assert!(plan.content_root.is_none());
        assert_eq!(plan.content_conflicts, vec!["proposal".to_string()]);
        assert!(plan.files.is_empty());
        assert!(!plan.checkout_ready());
        assert!(matches!(
            plan.lifecycle,
            super::super::radicle_change::ChangeLifecycleView::ContentConflicted { .. }
        ));
    }
}
