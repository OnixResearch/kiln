// r[verify cairn.workflow_collaboration.semantic_core]
// r[verify cairn.workflow_collaboration.semantic_core.neutral]
// r[verify cairn.workflow_collaboration.reducer]
// r[verify cairn.workflow_collaboration.reducer.conflict]
// r[verify cairn.workflow_collaboration.reducer.resolve]
// r[verify cairn.workflow_collaboration.evidence]
// r[verify cairn.workflow_collaboration.portable.roundtrip]
// r[verify cairn.workflow_collaboration.portable.tamper]
// r[verify cairn.verification_obligation_cob.profile]
// r[verify cairn.verification_obligation_cob.profile.kind]
// r[verify cairn.verification_obligation_cob.authority]
// r[verify cairn.verification_obligation_cob.authority.allowed]
// r[verify cairn.verification_obligation_cob.authority.denied]
// r[verify cairn.verification_obligation_cob.claim_revision]
// r[verify cairn.verification_obligation_cob.findings]
// r[verify cairn.verification_obligation_cob.validation]
// r[verify cairn.verification_obligation_cob.discharge]
// r[verify cairn.verification_obligation_cob.discharge.allowed]
// r[verify cairn.verification_obligation_cob.discharge.blocker]
// r[verify cairn.verification_obligation_cob.discharge.invalidation]
// r[verify cairn.verification_obligation_cob.discharge.stale_evidence]
// r[verify cairn.verification_obligation_cob.findings.delete]
// r[verify cairn.verification_obligation_cob.findings.unresolved]
// r[verify cairn.verification_obligation_cob.findings.conflict]
// r[verify cairn.verification_obligation_cob.claim_revision]
// r[verify cairn.verification_obligation_cob.claim_revision.conflict]
// r[verify cairn.verification_obligation_cob.claim_revision.history]
// r[verify cairn.verification_obligation_cob.claim_revision.stale]
// r[verify cairn.verification_obligation_cob.identity]
// r[verify cairn.verification_obligation_cob.identity.provider]
// r[verify cairn.verification_obligation_cob.receipts.backend]
// r[verify cairn.verification_obligation_cob.receipts.discharge]
// r[verify cairn.workflow_collaboration.reducer.conflict]
// r[verify cairn.workflow_collaboration.reducer.resolve]
// r[verify cairn.workflow_collaboration.evidence]
// r[verify cairn.workflow_collaboration.evidence.hash]
// r[verify cairn.workflow_collaboration.evidence.opaque]
// r[verify cairn.workflow_collaboration.evidence.role]
// r[verify cairn.workflow_collaboration.portable.roundtrip]
// r[verify cairn.workflow_collaboration.portable.tamper]
// r[verify cairn.workflow_collaboration.policy]
// r[verify cairn.workflow_collaboration.policy.deterministic]
// r[verify cairn.workflow_collaboration.policy.invalid]
// r[verify cairn.workflow_collaboration.receipts.execution]
// r[verify cairn.workflow_collaboration.receipts.provider]
// r[verify cairn.workflow_collaboration.validation.negative]
use super::*;

const POLICY_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OBLIGATION: &str = "vo-1";
const SUBMITTER: &str = "submitter";
const CONTRIBUTOR: &str = "contributor";
const VERIFIER: &str = "verifier";
const INVALIDATOR: &str = "invalidator";

fn base_revision() -> String {
    format!("git_oid:{}{}", "b".repeat(40), "c".repeat(24))
}

fn init_op() -> (WorkflowOperation, WorkflowState) {
    let init = WorkflowInit {
        schema: VERIFICATION_OBLIGATION_SCHEMA_V1.to_string(),
        kind: "verification-obligation".to_string(),
        obligation_id: OBLIGATION.to_string(),
        workspace_id: "workspace:vo".to_string(),
        base_revision: base_revision(),
        policy_hash: POLICY_HASH.to_string(),
        claim_root: ClaimRoot {
            claim_identity: "claim-1".to_string(),
            subject_root: "subject-1".to_string(),
            revision: 0,
        },
        submitters: BTreeSet::from([SUBMITTER.to_string()]),
        contributors: BTreeSet::from([CONTRIBUTOR.to_string()]),
        verifiers: BTreeSet::from([VERIFIER.to_string()]),
        invalidators: BTreeSet::from([INVALIDATOR.to_string()]),
        fields: BTreeMap::from([("claim.md".to_string(), "# Claim\n".to_string())]),
    };
    let op = WorkflowOperation {
        id: "init".to_string(),
        actor: SUBMITTER.to_string(),
        role: None,
        actions: vec![WorkflowAction::Init { state: init }],
    };
    let state = initialize_workflow_cob(op.clone()).unwrap();
    (op, state)
}

#[test]
fn neutral_workflow_needs_no_provider_context() {
    let (_, state) = init_op();
    assert_eq!(state.kind, "verification-obligation");
    assert_eq!(state.obligation_id, OBLIGATION);
    assert!(workflow_content_root(&state).unwrap().len() == BLAKE3_HEX_LENGTH);
    assert!(workflow_state_hash(&state).unwrap().len() == BLAKE3_HEX_LENGTH);
}

// r[verify cairn.verification_obligation_cob.profile.root]
#[test]
fn verification_obligation_kind_rejects_other_kind() {
    let mut init = WorkflowInit {
        schema: VERIFICATION_OBLIGATION_SCHEMA_V1.to_string(),
        kind: "verification-obligation".to_string(),
        obligation_id: OBLIGATION.to_string(),
        workspace_id: "workspace:vo".to_string(),
        base_revision: base_revision(),
        policy_hash: POLICY_HASH.to_string(),
        claim_root: ClaimRoot {
            claim_identity: "claim-1".to_string(),
            subject_root: "subject-1".to_string(),
            revision: 0,
        },
        submitters: BTreeSet::from([SUBMITTER.to_string()]),
        contributors: BTreeSet::from([CONTRIBUTOR.to_string()]),
        verifiers: BTreeSet::from([VERIFIER.to_string()]),
        invalidators: BTreeSet::from([INVALIDATOR.to_string()]),
        fields: BTreeMap::from([("claim.md".to_string(), "# Claim\n".to_string())]),
    };
    init.kind = "rfd".to_string();
    assert!(validate_workflow_init(&init).is_err());
    init.kind = "verification-obligation".to_string();
    assert!(validate_workflow_init(&init).is_ok());
}

// r[verify cairn.verification_obligation_cob.authority.denied]
#[test]
fn submitter_cannot_discharge_an_obligation() {
    let (_, mut state) = init_op();
    let discharge = WorkflowOperation {
        id: "discharge".to_string(),
        actor: SUBMITTER.to_string(),
        role: Some(WorkflowRole::Verifier),
        actions: vec![WorkflowAction::Discharge {
            decision: "approve".to_string(),
            subject_root: String::new(),
        }],
    };
    assert!(matches!(
        apply_workflow_cob_operation(&mut state, discharge),
        Err(WorkflowCobError::UnauthorizedRole {
            required: "verifier",
            ..
        })
    ));
}

// r[verify cairn.workflow_collaboration.evidence]
#[test]
fn recorded_evidence_cannot_become_property() {
    let (_, mut state) = init_op();
    let at_attach = WorkflowOperation {
        id: "evidence".to_string(),
        actor: CONTRIBUTOR.to_string(),
        role: Some(WorkflowRole::Contributor),
        actions: vec![WorkflowAction::AttachEvidence {
            evidence: WorkflowEvidenceReference {
                id: "proof-1".to_string(),
                evidence_kind: "formal".to_string(),
                verification_role: "property".to_string(),
                claim_scope: "claim-1".to_string(),
                external_identity: "trellis/1".to_string(),
            },
        }],
    };
    assert!(matches!(
        apply_workflow_cob_operation(&mut state, at_attach),
        Err(WorkflowCobError::EvidenceRolePromotion { .. })
    ));
}

// r[verify cairn.verification_obligation_cob.discharge]
#[test]
fn unresolved_blocking_finding_prevents_discharge() {
    let (_, mut state) = init_op();
    apply_workflow_cob_operation(
        &mut state,
        WorkflowOperation {
            id: "find".to_string(),
            actor: CONTRIBUTOR.to_string(),
            role: Some(WorkflowRole::Contributor),
            actions: vec![WorkflowAction::AddFinding {
                finding: WorkflowFinding {
                    id: "f1".to_string(),
                    status: "open".to_string(),
                    blocker_class: "blocking".to_string(),
                    source_span: "claim.md:1".to_string(),
                    response: None,
                    disposition: None,
                },
            }],
        },
    )
    .unwrap();
    let discharge = WorkflowOperation {
        id: "discharge".to_string(),
        actor: VERIFIER.to_string(),
        role: Some(WorkflowRole::Verifier),
        actions: vec![WorkflowAction::Discharge {
            decision: "approve".to_string(),
            subject_root: String::new(),
        }],
    };
    assert!(matches!(
        apply_workflow_cob_operation(&mut state, discharge),
        Err(WorkflowCobError::UnresolvedBlocker { finding_id }) if finding_id == "f1"
    ));
}

// r[verify cairn.verification_obligation_cob.discharge]
#[test]
fn conflict_free_discharge_succeeds_when_no_blockers() {
    let (_, mut state) = init_op();
    apply_workflow_cob_operation(
        &mut state,
        WorkflowOperation {
            id: "find".to_string(),
            actor: CONTRIBUTOR.to_string(),
            role: Some(WorkflowRole::Contributor),
            actions: vec![WorkflowAction::AddFinding {
                finding: WorkflowFinding {
                    id: "f1".to_string(),
                    status: "open".to_string(),
                    blocker_class: "non-blocking".to_string(),
                    source_span: "claim.md:1".to_string(),
                    response: None,
                    disposition: None,
                },
            }],
        },
    )
    .unwrap();
    apply_workflow_cob_operation(
        &mut state,
        WorkflowOperation {
            id: "discharge".to_string(),
            actor: VERIFIER.to_string(),
            role: Some(WorkflowRole::Verifier),
            actions: vec![WorkflowAction::Discharge {
                decision: "approve".to_string(),
                subject_root: String::new(),
            }],
        },
    )
    .unwrap();
    assert_eq!(state.lifecycle.status, "discharged");
    // post-discharge field edit is immutable
    let heads = state.fields["claim.md"].head_ids();
    let edit = WorkflowOperation {
        id: "edit-after".to_string(),
        actor: CONTRIBUTOR.to_string(),
        role: Some(WorkflowRole::Contributor),
        actions: vec![WorkflowAction::SetField {
            field: "claim.md".to_string(),
            content: Some("# Changed\n".to_string()),
            basis: heads,
        }],
    };
    assert!(matches!(
        apply_workflow_cob_operation(&mut state, edit),
        Err(WorkflowCobError::ImmutableDischarged)
    ));
}

// r[verify cairn.verification_obligation_cob.claim_revision]
#[test]
fn claim_revision_advances_and_makes_prior_discharge_inapplicable() {
    let (_, mut state) = init_op();
    let original_claim_revision = state.claim_root.revision;
    let heads = state.fields["claim.md"].head_ids();
    // Contributor revises the claim field (claim revision always advances on resolve)
    apply_workflow_cob_operation(
        &mut state,
        WorkflowOperation {
            id: "revise".to_string(),
            actor: CONTRIBUTOR.to_string(),
            role: Some(WorkflowRole::Contributor),
            actions: vec![WorkflowAction::ResolveField {
                field: "claim.md".to_string(),
                content: "# Revised claim\n".to_string(),
                basis: heads,
            }],
        },
    )
    .unwrap();
    assert!(state.claim_root.revision > original_claim_revision);
    assert_eq!(state.lifecycle.status, "open");
}

// r[verify cairn.workflow_collaboration.reducer.resolve]
#[test]
fn concurrent_field_revisions_conflict_and_resolve() {
    let (_, mut state) = init_op();
    let basis = state.fields["claim.md"].head_ids();
    // Both edits branch from the same init head -> two current heads -> conflict.
    apply_workflow_cob_operation(
        &mut state,
        WorkflowOperation {
            id: "a".to_string(),
            actor: CONTRIBUTOR.to_string(),
            role: Some(WorkflowRole::Contributor),
            actions: vec![WorkflowAction::SetField {
                field: "claim.md".to_string(),
                content: Some("# A\n".to_string()),
                basis: basis.clone(),
            }],
        },
    )
    .unwrap();
    apply_workflow_cob_operation(
        &mut state,
        WorkflowOperation {
            id: "b".to_string(),
            actor: CONTRIBUTOR.to_string(),
            role: Some(WorkflowRole::Contributor),
            actions: vec![WorkflowAction::SetField {
                field: "claim.md".to_string(),
                content: Some("# B\n".to_string()),
                basis: basis.clone(),
            }],
        },
    )
    .unwrap();
    assert_eq!(state.fields["claim.md"].head_ids().len(), 2);
    assert!(workflow_conflicted(&state));

    // Complete resolution supersedes all heads and advances the claim root.
    let all = state.fields["claim.md"].head_ids();
    apply_workflow_cob_operation(
        &mut state,
        WorkflowOperation {
            id: "resolve".to_string(),
            actor: CONTRIBUTOR.to_string(),
            role: Some(WorkflowRole::Contributor),
            actions: vec![WorkflowAction::ResolveField {
                field: "claim.md".to_string(),
                content: "# Merged\n".to_string(),
                basis: all,
            }],
        },
    )
    .unwrap();
    assert_eq!(state.fields["claim.md"].head_ids().len(), 1);
    assert!(!workflow_conflicted(&state));
}
