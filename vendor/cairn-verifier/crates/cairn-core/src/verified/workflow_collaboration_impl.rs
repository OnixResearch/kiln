// r[impl cairn.workflow_collaboration.semantic_core.neutral]
// r[impl cairn.workflow_collaboration.evidence]
pub fn validate_workflow_init(init: &WorkflowInit) -> Result<(), WorkflowCobError> {
    let kind = WorkflowKind::try_from(init.kind.as_str())?;
    if init.schema != kind.schema_v1() {
        return Err(WorkflowCobError::SchemaMismatch {
            expected: kind.schema_v1().to_string(),
            actual: init.schema.clone(),
        });
    }
    if init.obligation_id.is_empty()
        || init.workspace_id.is_empty()
        || init.policy_hash.len() != BLAKE3_HEX_LENGTH
        || init.claim_root.claim_identity.is_empty()
        || init.claim_root.subject_root.is_empty()
    {
        return Err(WorkflowCobError::InvalidField {
            field: "identity".to_string(),
            reason: "obligation id, workspace id, policy hash, and claim root must be present".to_string(),
        });
    }
    let base = decode_base_revision(&init.base_revision)
        .map_err(|error| WorkflowCobError::Factory(format!("{error}")))?;
    if base.kind != BASE_REVISION_KIND_GIT_OID {
        return Err(WorkflowCobError::InvalidField {
            field: "base_revision".to_string(),
            reason: "COB workflow requires a git_oid base revision".to_string(),
        });
    }
    if init.submitters.is_empty()
        || init.contributors.is_empty()
        || init.verifiers.is_empty()
        || init.invalidators.is_empty()
    {
        return Err(WorkflowCobError::InvalidField {
            field: "roles".to_string(),
            reason: "each workflow role set must be non-empty".to_string(),
        });
    }
    if init.fields.is_empty()
        || init.fields.values().any(|value| value.is_empty())
        || init.fields.keys().any(|key| !safe_path(key))
    {
        return Err(WorkflowCobError::InvalidField {
            field: "fields".to_string(),
            reason: "fields must have safe non-empty ids and content".to_string(),
        });
    }
    Ok(())
}

// r[impl cairn.workflow_collaboration.semantic_core]
// r[impl cairn.verification_obligation_cob.profile]
pub fn initialize_workflow_cob(
    operation: WorkflowOperation,
) -> Result<WorkflowState, WorkflowCobError> {
    if operation.actions.is_empty() {
        return Err(WorkflowCobError::EmptyOperation);
    }
    let [WorkflowAction::Init { state }] = operation.actions.as_slice() else {
        return Err(WorkflowCobError::InvalidRootShape);
    };
    validate_workflow_init(state)?;
    let kind = WorkflowKind::try_from(state.kind.as_str())?;
    let mut fields = BTreeMap::new();
    for (field_id, content) in &state.fields {
        let mut set = WorkflowRevisionSet::default();
        set.revisions.insert(format!("init:{field_id}"), content.clone());
        fields.insert(field_id.clone(), set);
    }
    Ok(WorkflowState {
        schema: WORKFLOW_COLLABORATION_SCHEMA_V1.to_string(),
        kind: kind.as_str().to_string(),
        obligation_id: state.obligation_id.clone(),
        workspace_id: state.workspace_id.clone(),
        base_revision: state.base_revision.clone(),
        policy_hash: state.policy_hash.clone(),
        claim_root: state.claim_root.clone(),
        submitters: state.submitters.clone(),
        contributors: state.contributors.clone(),
        verifiers: state.verifiers.clone(),
        invalidators: state.invalidators.clone(),
        fields,
        findings: BTreeMap::new(),
        evidence: BTreeMap::new(),
        lifecycle: WorkflowLifecycleRecord {
            status: "open".to_string(),
            subject_root: String::new(),
        },
        operations: BTreeSet::from([operation.id]),
    })
}

// r[impl cairn.workflow_collaboration.reducer]
// r[impl cairn.workflow_collaboration.reducer.disjoint]
// r[impl cairn.workflow_collaboration.reducer.conflict]
// r[impl cairn.workflow_collaboration.reducer.resolve]
// r[impl cairn.workflow_collaboration.reducer.invalid]
// r[impl cairn.verification_obligation_cob.authority]
pub fn apply_workflow_cob_operation(
    state: &mut WorkflowState,
    operation: WorkflowOperation,
) -> Result<(), WorkflowCobError> {
    validate_workflow_state_schema(state)?;
    if operation.actions.is_empty() {
        return Err(WorkflowCobError::EmptyOperation);
    }
    if !state.operations.insert(operation.id.clone()) {
        return Err(WorkflowCobError::DuplicateOperation { id: operation.id });
    }
    let mut snapshot = state.clone();
    for action in &operation.actions {
        apply_workflow_action(&mut snapshot, &operation, action)?;
    }
    *state = snapshot;
    Ok(())
}

fn apply_workflow_action(
    state: &mut WorkflowState,
    operation: &WorkflowOperation,
    action: &WorkflowAction,
) -> Result<(), WorkflowCobError> {
    let required = required_role(action);
    if let Some(required) = required {
        if !workflow_actor_has_role(state, required.clone(), &operation.actor) {
            return Err(WorkflowCobError::UnauthorizedRole {
                required: required.as_str(),
                actor: operation.actor.clone(),
            });
        }
    }
    match action {
        WorkflowAction::Init { .. } => Err(WorkflowCobError::InvalidRootShape),
        WorkflowAction::SetField { field, content, basis } => {
            if state.lifecycle.status == "discharged" || state.lifecycle.status == "invalidated" {
                return Err(WorkflowCobError::ImmutableDischarged);
            }
            set_field(state, operation, field, content, basis)
        }
        WorkflowAction::AttachEvidence { evidence } => attach_evidence(state, evidence),
        WorkflowAction::AddFinding { finding } => add_finding(state, finding),
        WorkflowAction::RespondFinding { finding_id, response } => {
            respond_finding(state, finding_id, response)
        }
        WorkflowAction::SetDisposition { finding_id, disposition, basis } => {
            set_disposition(state, finding_id, disposition, basis)
        }
        WorkflowAction::Discharge { .. } => discharge(state),
        WorkflowAction::Reject { .. } => reject(state),
        WorkflowAction::Invalidate { .. } => invalidate(state),
        WorkflowAction::ResolveField { field, content, basis } => {
            resolve_field(state, operation, field, content, basis)
        }
    }
}

fn set_field(
    state: &mut WorkflowState,
    operation: &WorkflowOperation,
    field: &str,
    content: &Option<String>,
    basis: &BTreeSet<String>,
) -> Result<(), WorkflowCobError> {
    let set = state.fields.get_mut(field).ok_or_else(|| WorkflowCobError::InvalidField {
        field: field.to_string(),
        reason: "unknown field id".to_string(),
    })?;
    ensure_basis(set, basis)?;
    let Some(content) = content else {
        return Ok(());
    };
    let revision_id = format!("{}:{field}", operation.id);
    set.revisions.insert(revision_id.clone(), content.clone());
    set.supersedes.insert(revision_id.clone(), basis.clone());
    Ok(())
}

fn resolve_field(
    state: &mut WorkflowState,
    operation: &WorkflowOperation,
    field: &str,
    content: &str,
    basis: &BTreeSet<String>,
) -> Result<(), WorkflowCobError> {
    if state.lifecycle.status == "discharged" || state.lifecycle.status == "invalidated" {
        return Err(WorkflowCobError::ImmutableDischarged);
    }
    let set = state.fields.get_mut(field).ok_or_else(|| WorkflowCobError::InvalidField {
        field: field.to_string(),
        reason: "unknown field id".to_string(),
    })?;
    ensure_basis(set, basis)?;
    let revision_id = format!("{}:{field}", operation.id);
    set.revisions.insert(revision_id.clone(), content.to_string());
    set.supersedes.insert(revision_id.clone(), basis.clone());
    // A claim revision makes earlier review facts (discharges) inapplicable.
    state.claim_root.revision += 1;
    state.lifecycle.status = "open".to_string();
    Ok(())
}

fn attach_evidence(
    state: &mut WorkflowState,
    evidence: &WorkflowEvidenceReference,
) -> Result<(), WorkflowCobError> {
    // recorded-only evidence cannot claim property/proof authority.
    if evidence.verification_role == "property" || evidence.verification_role == "proof" {
        return Err(WorkflowCobError::EvidenceRolePromotion {
            role: evidence.verification_role.clone(),
        });
    }
    if state.evidence.contains_key(&evidence.id) {
        return Err(WorkflowCobError::DuplicateOperation {
            id: evidence.id.clone(),
        });
    }
    state.evidence.insert(evidence.id.clone(), evidence.clone());
    Ok(())
}

fn add_finding(
    state: &mut WorkflowState,
    finding: &WorkflowFinding,
) -> Result<(), WorkflowCobError> {
    if state.findings.contains_key(&finding.id) {
        return Err(WorkflowCobError::DuplicateOperation {
            id: finding.id.clone(),
        });
    }
    if finding.status != "open" {
        return Err(WorkflowCobError::InvalidField {
            field: "finding".to_string(),
            reason: "new findings must start open".to_string(),
        });
    }
    state.findings.insert(finding.id.clone(), finding.clone());
    Ok(())
}

fn respond_finding(
    state: &mut WorkflowState,
    finding_id: &str,
    response: &str,
) -> Result<(), WorkflowCobError> {
    let finding = state.findings.get_mut(finding_id).ok_or_else(|| WorkflowCobError::InvalidField {
        field: "finding".to_string(),
        reason: "unknown finding id".to_string(),
    })?;
    finding.response = Some(response.to_string());
    Ok(())
}

fn set_disposition(
    state: &mut WorkflowState,
    finding_id: &str,
    disposition: &str,
    _basis: &[String],
) -> Result<(), WorkflowCobError> {
    let finding = state.findings.get_mut(finding_id).ok_or_else(|| WorkflowCobError::InvalidField {
        field: "finding".to_string(),
        reason: "unknown finding id".to_string(),
    })?;
    finding.disposition = Some(disposition.to_string());
    if disposition == "resolved" {
        finding.status = "resolved".to_string();
    }
    Ok(())
}

fn discharge(state: &mut WorkflowState) -> Result<(), WorkflowCobError> {
    for (id, finding) in &state.findings {
        if finding.blocker_class != "non-blocking" && finding.status != "resolved" {
            return Err(WorkflowCobError::UnresolvedBlocker {
                finding_id: id.clone(),
            });
        }
    }
    if workflow_conflicted(state) {
        return Err(WorkflowCobError::ConflictedField {
            field: "fields".to_string(),
            heads: workflow_conflict_view(state)
                .values()
                .flatten()
                .cloned()
                .collect(),
        });
    }
    state.lifecycle.status = "discharged".to_string();
    let subject_root = workflow_content_root(state)?;
    state.lifecycle.subject_root = subject_root;
    Ok(())
}

fn reject(state: &mut WorkflowState) -> Result<(), WorkflowCobError> {
    state.lifecycle.status = "rejected".to_string();
    Ok(())
}

fn invalidate(state: &mut WorkflowState) -> Result<(), WorkflowCobError> {
    state.lifecycle.status = "invalidated".to_string();
    Ok(())
}

fn required_role(action: &WorkflowAction) -> Option<WorkflowRole> {
    match action {
        WorkflowAction::Init { .. } => None,
        WorkflowAction::SetField { .. } | WorkflowAction::ResolveField { .. } => {
            Some(WorkflowRole::Contributor)
        }
        WorkflowAction::AttachEvidence { .. } | WorkflowAction::AddFinding { .. } => {
            Some(WorkflowRole::Contributor)
        }
        WorkflowAction::RespondFinding { .. } | WorkflowAction::SetDisposition { .. } => {
            Some(WorkflowRole::Verifier)
        }
        WorkflowAction::Discharge { .. } | WorkflowAction::Reject { .. } => {
            Some(WorkflowRole::Verifier)
        }
        WorkflowAction::Invalidate { .. } => Some(WorkflowRole::Invalidator),
    }
}

// r[impl cairn.verification_obligation_cob.authority.allowed]
// r[impl cairn.verification_obligation_cob.authority.denied]
// r[impl cairn.verification_obligation_cob.authority.ambient]
pub fn workflow_actor_has_role(state: &WorkflowState, role: WorkflowRole, actor: &str) -> bool {
    match role {
        WorkflowRole::Submitter => state.submitters.contains(actor),
        WorkflowRole::Contributor => state.contributors.contains(actor),
        WorkflowRole::Verifier => state.verifiers.contains(actor),
        WorkflowRole::Invalidator => state.invalidators.contains(actor),
    }
}

pub fn workflow_state_hash(state: &WorkflowState) -> Result<String, WorkflowCobError> {
    validate_workflow_state_schema(state)?;
    Ok(canonical_json_hash(&json!(state)))
}

pub fn workflow_content_root(state: &WorkflowState) -> Result<String, WorkflowCobError> {
    validate_workflow_state_schema(state)?;
    let mut fields = BTreeMap::new();
    for (field, set) in &state.fields {
        let mut contents = set
            .head_ids()
            .into_iter()
            .filter_map(|head| set.content(&head).map(str::to_string))
            .collect::<Vec<_>>();
        contents.sort();
        contents.dedup();
        fields.insert(field.clone(), contents);
    }
    Ok(canonical_json_hash(&json!({
        "kind": state.kind,
        "obligation_id": state.obligation_id,
        "workspace_id": state.workspace_id,
        "policy_hash": state.policy_hash,
        "claim_root": state.claim_root,
        "submitters": state.submitters,
        "contributors": state.contributors,
        "verifiers": state.verifiers,
        "invalidators": state.invalidators,
        "fields": fields,
        "findings": state.findings,
        "evidence": state.evidence,
        "lifecycle": state.lifecycle,
    })))
}

pub fn validate_workflow_state_schema(state: &WorkflowState) -> Result<(), WorkflowCobError> {
    if state.schema != WORKFLOW_COLLABORATION_SCHEMA_V1 {
        return Err(WorkflowCobError::SchemaMismatch {
            expected: WORKFLOW_COLLABORATION_SCHEMA_V1.to_string(),
            actual: state.schema.clone(),
        });
    }
    Ok(())
}

pub fn assert_workflow_state_kind(
    state: &WorkflowState,
    expected: WorkflowKind,
) -> Result<(), WorkflowCobError> {
    let actual = WorkflowKind::try_from(state.kind.as_str())?;
    if actual != expected {
        return Err(WorkflowCobError::KindMismatch {
            expected: expected.as_str().to_string(),
            actual: state.kind.clone(),
        });
    }
    Ok(())
}

pub fn workflow_conflict_view(state: &WorkflowState) -> BTreeMap<String, Vec<String>> {
    let mut conflicts = BTreeMap::new();
    for (field, set) in &state.fields {
        let heads = set.head_ids();
        if heads.len() > 1 {
            conflicts.insert(field.clone(), heads.into_iter().collect());
        }
    }
    conflicts
}

pub fn workflow_conflicted(state: &WorkflowState) -> bool {
    !workflow_conflict_view(state).is_empty()
}

// r[impl cairn.workflow_collaboration.portable]
// r[impl cairn.workflow_collaboration.portable.roundtrip]
// r[impl cairn.workflow_collaboration.portable.tamper]
pub fn serialize_workflow_portable(state: &WorkflowState) -> Result<String, WorkflowCobError> {
    let mut value = json!({
        "schema": WORKFLOW_PORTABLE_SCHEMA_V1,
        "backend": "portable",
        "state": state,
    });
    value["state_identity"] = json!(workflow_state_hash(state)?);
    value["content_identity"] = json!(workflow_content_root(state)?);
    value["portable_identity"] = json!(portable_identity(&value));
    serde_json::to_string(&value).map_err(workflow_json_error)
}

pub fn parse_workflow_portable(text: &str) -> Result<WorkflowState, WorkflowCobError> {
    let value: Value = serde_json::from_str(text).map_err(workflow_json_error)?;
    validate_portable(&value)?;
    let state: WorkflowState = serde_json::from_value(value.get("state").cloned().unwrap_or(Value::Null))
        .map_err(workflow_json_error)?;
    validate_workflow_state_schema(&state)?;
    Ok(state)
}

fn validate_portable(value: &Value) -> Result<(), WorkflowCobError> {
    if value.get("schema").and_then(Value::as_str) != Some(WORKFLOW_PORTABLE_SCHEMA_V1) {
        return Err(WorkflowCobError::PortableTamper {
            field: "schema".to_string(),
        });
    }
    let state: WorkflowState =
        serde_json::from_value(value.get("state").cloned().unwrap_or(Value::Null))
            .map_err(workflow_json_error)?;
    if value.get("state_identity").and_then(Value::as_str) != Some(&workflow_state_hash(&state)?) {
        return Err(WorkflowCobError::PortableTamper {
            field: "state_identity".to_string(),
        });
    }
    if value.get("content_identity").and_then(Value::as_str) != Some(&workflow_content_root(&state)?) {
        return Err(WorkflowCobError::PortableTamper {
            field: "content_identity".to_string(),
        });
    }
    if value.get("portable_identity").and_then(Value::as_str) != Some(&portable_identity(value)) {
        return Err(WorkflowCobError::PortableTamper {
            field: "portable_identity".to_string(),
        });
    }
    Ok(())
}

fn portable_identity(value: &Value) -> String {
    let mut material = value.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove("portable_identity");
    }
    canonical_json_hash(&material)
}

fn ensure_basis(set: &WorkflowRevisionSet, basis: &BTreeSet<String>) -> Result<(), WorkflowCobError> {
    let heads = set.head_ids();
    if basis.is_empty() {
        if !heads.is_empty() {
            return Err(WorkflowCobError::MissingBasis {
                operation: "field".to_string(),
                basis: "current heads".to_string(),
            });
        }
        return Ok(());
    }
    if !basis.is_subset(&heads) && !basis.iter().all(|base| set.revisions.contains_key(base)) {
        return Err(WorkflowCobError::UnknownBase {
            field: "field".to_string(),
            base: basis
                .iter()
                .find(|b| !set.revisions.contains_key(*b))
                .cloned()
                .unwrap_or_default(),
        });
    }
    Ok(())
}

fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains("..")
        && !path.contains("//")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

fn workflow_json_error<E: std::fmt::Display>(error: E) -> WorkflowCobError {
    WorkflowCobError::Factory(format!("workflow JSON error: {error}"))
}
