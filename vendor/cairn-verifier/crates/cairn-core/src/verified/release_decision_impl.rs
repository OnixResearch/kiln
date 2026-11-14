// r[impl cairn.release_decision_cob.profile.root]
// r[impl cairn.release_decision_cob.profile.kind]
pub fn validate_release_init(init: &ReleaseDecisionInit) -> Result<(), ReleaseDecisionError> {
    if init.schema != RELEASE_DECISION_SCHEMA_V1 {
        return Err(ReleaseDecisionError::SchemaMismatch {
            expected: RELEASE_DECISION_SCHEMA_V1.to_string(),
            actual: init.schema.clone(),
        });
    }
    if init.kind != "release-decision" {
        return Err(ReleaseDecisionError::KindMismatch {
            expected: "release-decision".to_string(),
            actual: init.kind.clone(),
        });
    }
    if init.decision_id.is_empty()
        || init.workspace_id.is_empty()
        || init.candidate_root.len() != BLAKE3_HEX_LENGTH
        || init.policy_identity.len() != BLAKE3_HEX_LENGTH
        || init.candidate.source_revision.is_empty()
        || init.candidate.artifact_manifest.is_empty()
        || init.candidate.artifact_identity.is_empty()
        || init.candidate.target_label.is_empty()
        || init.candidate.release_label.is_empty()
    {
        return Err(ReleaseDecisionError::InvalidField {
            field: "identity".to_string(),
            reason: "decision id, candidate root, policy identity, and candidate fields must be present".to_string(),
        });
    }
    let base = super::radicle_change::decode_base_revision(&init.workspace_id)
        .ok();
    let _ = base;
    if init.proposers.is_empty()
        || init.reviewers.is_empty()
        || init.approvers.is_empty()
        || init.withdrawers.is_empty()
        || init.invalidators.is_empty()
    {
        return Err(ReleaseDecisionError::InvalidField {
            field: "roles".to_string(),
            reason: "each release-decision role set must be non-empty".to_string(),
        });
    }
    Ok(())
}

// r[impl cairn.release_decision_cob.candidate]
// r[impl cairn.release_decision_cob.candidate.provider]
// r[impl cairn.release_decision_cob.candidate.immutable]
pub fn initialize_release_decision(
    operation: ReleaseDecisionOperation,
) -> Result<ReleaseDecisionState, ReleaseDecisionError> {
    if operation.actions.is_empty() {
        return Err(ReleaseDecisionError::EmptyOperation);
    }
    let [ReleaseDecisionAction::Init { state }] = operation.actions.as_slice() else {
        return Err(ReleaseDecisionError::InvalidRootShape);
    };
    validate_release_init(state)?;
    let candidate_root = release_candidate_root(&state.candidate, &state.policy_identity)?;
    if candidate_root != state.candidate_root {
        return Err(ReleaseDecisionError::CandidateSubstitution);
    }
    Ok(ReleaseDecisionState {
        schema: RELEASE_DECISION_SCHEMA_V1.to_string(),
        kind: "release-decision".to_string(),
        decision_id: state.decision_id.clone(),
        workspace_id: state.workspace_id.clone(),
        candidate_root: state.candidate_root.clone(),
        policy_identity: state.policy_identity.clone(),
        candidate: state.candidate.clone(),
        proposers: state.proposers.clone(),
        reviewers: state.reviewers.clone(),
        approvers: state.approvers.clone(),
        withdrawers: state.withdrawers.clone(),
        invalidators: state.invalidators.clone(),
        evidence: BTreeMap::new(),
        findings: BTreeMap::new(),
        votes: BTreeMap::new(),
        lifecycle: "draft".to_string(),
        review_basis: String::new(),
        operations: BTreeSet::from([operation.id]),
    })
}

pub fn release_candidate_root(
    candidate: &ReleaseCandidate,
    policy_identity: &str,
) -> Result<String, ReleaseDecisionError> {
    Ok(canonical_json_hash(&json!({
        "candidate": candidate,
        "policy": policy_identity,
    })))
}

// r[impl cairn.release_decision_cob.authority]
// r[impl cairn.release_decision_cob.evidence]
// r[impl cairn.release_decision_cob.blockers]
// r[impl cairn.release_decision_cob.decision]
// r[impl cairn.release_decision_cob.lifecycle]
pub fn apply_release_operation(
    state: &mut ReleaseDecisionState,
    operation: ReleaseDecisionOperation,
) -> Result<(), ReleaseDecisionError> {
    if operation.actions.is_empty() {
        return Err(ReleaseDecisionError::EmptyOperation);
    }
    if !state.operations.insert(operation.id.clone()) {
        return Err(ReleaseDecisionError::DuplicateOperation { id: operation.id });
    }
    let mut snapshot = state.clone();
    for action in &operation.actions {
        apply_release_action(&mut snapshot, &operation, action)?;
    }
    *state = snapshot;
    Ok(())
}

// r[impl cairn.release_decision_cob.decision.current]
pub fn derive_review_basis(state: &ReleaseDecisionState) -> Result<String, ReleaseDecisionError> {
    let mut evidence = Vec::new();
    for entry in state.evidence.values() {
        if entry.verification_role == "property" {
            evidence.push(json!({ "id": entry.id, "identity": entry.evidence_identity }));
        }
    }
    let mut blockers = Vec::new();
    for finding in state.findings.values() {
        if finding.blocker_class == "blocking"
            && !finding.dispositions.iter().any(|d| d == "resolved")
        {
            blockers.push(json!(finding.id));
        }
    }
    Ok(canonical_json_hash(&json!({
        "candidate_root": state.candidate_root,
        "policy": state.policy_identity,
        "evidence": evidence,
        "blockers": blockers,
    })))
}

pub fn release_state_hash(state: &ReleaseDecisionState) -> Result<String, ReleaseDecisionError> {
    Ok(canonical_json_hash(&json!(state)))
}

pub fn release_content_root(state: &ReleaseDecisionState) -> Result<String, ReleaseDecisionError> {
    Ok(canonical_json_hash(&json!({
        "kind": state.kind,
        "decision_id": state.decision_id,
        "candidate_root": state.candidate_root,
        "policy_identity": state.policy_identity,
        "candidate": state.candidate,
        "evidence": state.evidence,
        "findings": state.findings,
        "review_basis": state.review_basis,
        "verdict": state.lifecycle,
    })))
}

fn apply_release_action(
    state: &mut ReleaseDecisionState,
    operation: &ReleaseDecisionOperation,
    action: &ReleaseDecisionAction,
) -> Result<(), ReleaseDecisionError> {
    let required = required_role(action);
    if let Some(required) = required {
        if !release_actor_has_role(state, &operation.actor, required.clone()) {
            return Err(ReleaseDecisionError::UnauthorizedRole {
                required: required.as_str(),
                actor: operation.actor.clone(),
            });
        }
    }
    match action {
        ReleaseDecisionAction::Init { .. } => Err(ReleaseDecisionError::InvalidRootShape),
        ReleaseDecisionAction::AttachEvidence { evidence } => attach_evidence(state, evidence),
        ReleaseDecisionAction::AddFinding { finding } => add_finding(state, finding),
        ReleaseDecisionAction::RespondFinding { finding_id, response } => {
            let f = state.findings.get_mut(finding_id).ok_or_else(|| ReleaseDecisionError::InvalidField {
                field: "finding".to_string(),
                reason: "unknown finding id".to_string(),
            })?;
            if !f.description.starts_with(&format!("{response}::")) {
                f.description = format!("{response}::{}", f.description);
            }
            Ok(())
        }
        ReleaseDecisionAction::DisposeFinding { finding_id, disposition } => {
            let f = state.findings.get_mut(finding_id).ok_or_else(|| ReleaseDecisionError::InvalidField {
                field: "finding".to_string(),
                reason: "unknown finding id".to_string(),
            })?;
            f.dispositions.push(disposition.clone());
            Ok(())
        }
        ReleaseDecisionAction::Vote { vote } => apply_release_vote(state, vote),
        ReleaseDecisionAction::Withdraw { .. } => {
            state.lifecycle = "withdrawn".to_string();
            Ok(())
        }
        ReleaseDecisionAction::Invalidate { .. } => {
            state.lifecycle = "invalidated".to_string();
            Ok(())
        }
        ReleaseDecisionAction::Supersede { .. } => {
            state.lifecycle = "superseded".to_string();
            Ok(())
        }
        ReleaseDecisionAction::Resolve { finding_id, disposition } => {
            let f = state.findings.get_mut(finding_id).ok_or_else(|| ReleaseDecisionError::InvalidField {
                field: "finding".to_string(),
                reason: "unknown finding id".to_string(),
            })?;
            f.dispositions.push(disposition.clone());
            Ok(())
        }
    }
}

fn required_role(action: &ReleaseDecisionAction) -> Option<ReleaseRole> {
    match action {
        ReleaseDecisionAction::Init { .. } => None,
        ReleaseDecisionAction::AttachEvidence { .. } | ReleaseDecisionAction::AddFinding { .. } => {
            Some(ReleaseRole::Reviewer)
        }
        ReleaseDecisionAction::RespondFinding { .. }
        | ReleaseDecisionAction::DisposeFinding { .. }
        | ReleaseDecisionAction::Resolve { .. } => Some(ReleaseRole::Reviewer),
        ReleaseDecisionAction::Vote { .. } => None, // verified in vote()
        ReleaseDecisionAction::Withdraw { .. } => Some(ReleaseRole::Withdrawer),
        ReleaseDecisionAction::Invalidate { .. } => Some(ReleaseRole::Invalidator),
        ReleaseDecisionAction::Supersede { .. } => Some(ReleaseRole::Proposer),
    }
}

fn attach_evidence(
    state: &mut ReleaseDecisionState,
    evidence: &ReleaseEvidenceSummary,
) -> Result<(), ReleaseDecisionError> {
    if evidence.verification_role == "property"
        && (evidence.verdict != "pass" || evidence.receipt_identity.is_empty())
    {
        return Err(ReleaseDecisionError::EvidenceRolePromotion {
            role: evidence.verification_role.clone(),
        });
    }
    if state.evidence.contains_key(&evidence.id) {
        return Err(ReleaseDecisionError::DuplicateOperation {
            id: evidence.id.clone(),
        });
    }
    state.evidence.insert(evidence.id.clone(), evidence.clone());
    Ok(())
}

fn add_finding(
    state: &mut ReleaseDecisionState,
    finding: &ReleaseFinding,
) -> Result<(), ReleaseDecisionError> {
    if state.findings.contains_key(&finding.id) {
        return Err(ReleaseDecisionError::DuplicateOperation {
            id: finding.id.clone(),
        });
    }
    state.findings.insert(finding.id.clone(), finding.clone());
    Ok(())
}

fn apply_release_vote(
    state: &mut ReleaseDecisionState,
    vote: &ReleaseVote,
) -> Result<(), ReleaseDecisionError> {
    let basis = state.review_basis.clone();
    if !basis.is_empty() && vote.review_basis != basis {
        return Err(ReleaseDecisionError::StaleReviewBasis {
            expected: basis,
            observed: vote.review_basis.clone(),
        });
    }
    // Approval votes require the approver role; rejection votes require reviewer.
    let required = if vote.kind == "approve" {
        ReleaseRole::Approver
    } else {
        ReleaseRole::Reviewer
    };
    if !release_actor_has_role(state, &vote.actor, required.clone()) {
        return Err(ReleaseDecisionError::UnauthorizedRole {
            required: required.as_str(),
            actor: vote.actor.clone(),
        });
    }
    if state.votes.contains_key(&vote.id) {
        return Err(ReleaseDecisionError::DuplicateOperation {
            id: vote.id.clone(),
        });
    }
    state.votes.insert(vote.id.clone(), vote.clone());
    Ok(())
}

// r[impl cairn.release_decision_cob.decision.conflict]
// r[impl cairn.release_decision_cob.blockers.unresolved]
pub fn evaluate_decision(
    state: &mut ReleaseDecisionState,
) -> Result<String, ReleaseDecisionError> {
    let basis = derive_review_basis(state)?;
    if state.review_basis.is_empty() {
        state.review_basis = basis.clone();
    }
    // unresolved blockers
    for finding in state.findings.values() {
        if finding.blocker_class == "blocking"
            && !finding.dispositions.iter().any(|d| d == "resolved")
        {
            return Err(ReleaseDecisionError::UnresolvedBlocker {
                finding_id: finding.id.clone(),
            });
        }
    }
    let approve_votes = state
        .votes
        .values()
        .filter(|v| v.kind == "approve" && v.review_basis == basis)
        .count();
    let reject_votes = state
        .votes
        .values()
        .filter(|v| v.kind == "reject" && v.review_basis == basis)
        .count();
    let verdict = if approve_votes > 0 && reject_votes == 0 {
        "approved"
    } else if reject_votes > 0 && approve_votes == 0 {
        "rejected"
    } else if approve_votes > 0 && reject_votes > 0 {
        return Err(ReleaseDecisionError::ConflictedDecision);
    } else {
        "in-review"
    };
    if verdict != "in-review" && state.lifecycle == "draft" {
        state.lifecycle = verdict.to_string();
    }
    Ok(verdict.to_string())
}

pub fn release_actor_has_role(state: &ReleaseDecisionState, actor: &str, role: ReleaseRole) -> bool {
    match role {
        ReleaseRole::Proposer => state.proposers.contains(actor),
        ReleaseRole::Reviewer => state.reviewers.contains(actor),
        ReleaseRole::Approver => state.approvers.contains(actor),
        ReleaseRole::Withdrawer => state.withdrawers.contains(actor),
        ReleaseRole::Invalidator => state.invalidators.contains(actor),
    }
}

// r[impl cairn.release_decision_cob.backends.roundtrip]
// r[impl cairn.release_decision_cob.backends.roundtrip]
pub fn serialize_release_portable(state: &ReleaseDecisionState) -> Result<String, ReleaseDecisionError> {
    let mut value = json!({
        "schema": RELEASE_PORTABLE_SCHEMA_V1,
        "backend": "portable",
        "state": state,
    });
    value["state_identity"] = json!(release_state_hash(state)?);
    value["content_identity"] = json!(release_content_root(state)?);
    value["portable_identity"] = json!(portable_identity(&value));
    serde_json::to_string(&value).map_err(release_json_error)
}

pub fn parse_release_portable(text: &str) -> Result<ReleaseDecisionState, ReleaseDecisionError> {
    let value: Value = serde_json::from_str(text).map_err(release_json_error)?;
    if value.get("schema").and_then(Value::as_str) != Some(RELEASE_PORTABLE_SCHEMA_V1) {
        return Err(ReleaseDecisionError::PortableTamper {
            field: "schema".to_string(),
        });
    }
    let state: ReleaseDecisionState =
        serde_json::from_value(value.get("state").cloned().unwrap_or(Value::Null))
            .map_err(release_json_error)?;
    if value.get("state_identity").and_then(Value::as_str) != Some(&release_state_hash(&state)?) {
        return Err(ReleaseDecisionError::PortableTamper {
            field: "state_identity".to_string(),
        });
    }
    Ok(state)
}

fn portable_identity(value: &Value) -> String {
    let mut material = value.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove("portable_identity");
    }
    canonical_json_hash(&material)
}

fn release_json_error<E: std::fmt::Display>(error: E) -> ReleaseDecisionError {
    ReleaseDecisionError::Factory(format!("release JSON error: {error}"))
}
