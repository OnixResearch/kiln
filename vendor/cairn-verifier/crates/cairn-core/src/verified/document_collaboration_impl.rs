// r[impl cairn.document_collaboration.semantic_core]
// r[impl cairn.document_collaboration.semantic_core.neutral]
// r[impl cairn.document_collaboration.semantic_core.identity]
// r[impl cairn.rfd_cob.identity]
// r[impl cairn.rfd_cob.identity.provider]
// r[impl cairn.rfd_cob.identity.registry]
// r[impl cairn.rfd_cob.identity.non_claims]
// r[impl cairn.rfd_cob.receipts]
// r[impl cairn.rfd_cob.receipts.acceptance]
// r[impl cairn.rfd_cob.receipts.backend]
// r[impl cairn.rfd_cob.validation]
// r[impl cairn.rfd_cob.validation.release]
// r[impl cairn.document_collaboration.radicle_adapter.boundary]
// r[impl cairn.adr_cob.profile.kind]
// r[impl cairn.adr_cob.authority]
// r[impl cairn.adr_cob.identity.registry]
// r[impl cairn.adr_cob.materialization.conflict]
// r[impl cairn.rfd_cob.profile]
pub fn validate_document_init(init: &DocumentInit) -> Result<(), DocumentCobError> {
    let kind = DocumentKind::try_from(init.kind.as_str())?;
    if init.schema != kind.schema_v1() {
        return Err(DocumentCobError::SchemaMismatch {
            expected: kind.schema_v1().to_string(),
            actual: init.schema.clone(),
        });
    }
    if init.document_id.is_empty()
        || init.workspace_id.is_empty()
        || init.policy_hash.len() != BLAKE3_HEX_LENGTH
    {
        return Err(DocumentCobError::InvalidField {
            field: "identity".to_string(),
            reason: "document id, workspace id, and policy hash must be present".to_string(),
        });
    }
    let base = decode_base_revision(&init.base_revision)
        .map_err(|error| DocumentCobError::Factory(format!("{error}")))?;
    if base.kind != BASE_REVISION_KIND_GIT_OID {
        return Err(DocumentCobError::InvalidField {
            field: "base_revision".to_string(),
            reason: "COB document backend requires a git_oid base revision".to_string(),
        });
    }
    if init.authors.is_empty()
        || init.participants.is_empty()
        || init.reviewers.is_empty()
        || init.approvers.is_empty()
        || init.publishers.is_empty()
    {
        return Err(DocumentCobError::InvalidField {
            field: "roles".to_string(),
            reason: "each document role set must be non-empty and immutable".to_string(),
        });
    }
    if init.artifacts.is_empty()
        || init
            .artifacts
            .values()
            .any(|value| value.is_empty())
        || init
            .artifacts
            .keys()
            .any(|key| !safe_document_path(key))
    {
        return Err(DocumentCobError::InvalidField {
            field: "artifacts".to_string(),
            reason: "artifacts must have safe non-empty ids and content".to_string(),
        });
    }
    Ok(())
}

// r[impl cairn.document_collaboration.semantic_core]
// r[impl cairn.rfd_cob.profile.root]
pub fn initialize_document_cob(
    operation: DocumentOperation,
) -> Result<DocumentState, DocumentCobError> {
    if operation.actions.is_empty() {
        return Err(DocumentCobError::EmptyOperation);
    }
    let [DocumentAction::Init { state }] = operation.actions.as_slice() else {
        return Err(DocumentCobError::InvalidRootShape);
    };
    validate_document_init(state)?;
    let kind = DocumentKind::try_from(state.kind.as_str())?;
    let mut artifacts = BTreeMap::new();
    for (artifact_id, content) in &state.artifacts {
        let revision_id = format!("init:{artifact_id}");
        let mut set = DocumentRevisionSet::default();
        set.revisions.insert(revision_id.clone(), content.clone());
        artifacts.insert(artifact_id.clone(), set);
    }
    Ok(DocumentState {
        schema: DOCUMENT_COLLABORATION_SCHEMA_V1.to_string(),
        kind: kind.as_str().to_string(),
        document_id: state.document_id.clone(),
        workspace_id: state.workspace_id.clone(),
        base_revision: state.base_revision.clone(),
        policy_hash: state.policy_hash.clone(),
        authors: state.authors.clone(),
        participants: state.participants.clone(),
        reviewers: state.reviewers.clone(),
        approvers: state.approvers.clone(),
        publishers: state.publishers.clone(),
        artifacts,
        links: BTreeMap::new(),
        discussion: BTreeMap::new(),
        lifecycle: DocumentLifecycleRecord {
            status: DocumentLifecycleStatus::Draft,
            subject_root: String::new(),
            basis: BTreeSet::new(),
        },
        evidence: BTreeMap::new(),
        publication: None,
        operations: BTreeSet::from([operation.id]),
    })
}

// r[impl cairn.document_collaboration.reducer]
// r[impl cairn.document_collaboration.reducer.disjoint]
// r[impl cairn.document_collaboration.reducer.conflict]
// r[impl cairn.document_collaboration.reducer.resolve]
// r[impl cairn.document_collaboration.reducer.invalid]
// r[impl cairn.rfd_cob.authority]
pub fn apply_document_cob_operation(
    state: &mut DocumentState,
    operation: DocumentOperation,
) -> Result<(), DocumentCobError> {
    validate_state_schema(state)?;
    if operation.actions.is_empty() {
        return Err(DocumentCobError::EmptyOperation);
    }
    if !state.operations.insert(operation.id.clone()) {
        return Err(DocumentCobError::DuplicateOperation { id: operation.id });
    }
    let mut snapshot = state.clone();
    for action in &operation.actions {
        apply_document_action(&mut snapshot, &operation, action)?;
    }
    *state = snapshot;
    Ok(())
}

fn apply_document_action(
    state: &mut DocumentState,
    operation: &DocumentOperation,
    action: &DocumentAction,
) -> Result<(), DocumentCobError> {
    let required_role = required_role(action);
    if let Some(required) = required_role {
        if !document_actor_has_role(state, operation, required.clone()) {
            return Err(DocumentCobError::UnauthorizedRole {
                required: required.as_str(),
                actor: operation.actor.clone(),
            });
        }
    }
    match action {
        DocumentAction::Init { .. } => Err(DocumentCobError::MultiStateAction),
        DocumentAction::SetArtifact {
            artifact,
            content,
            basis,
        } => apply_artifact(state, operation, artifact, content, basis),
        DocumentAction::AddDiscussion { record, basis } => {
            apply_discussion(state, operation, record, basis)
        }
        DocumentAction::AddLink { link, basis } => apply_link(state, operation, link, basis),
        DocumentAction::SetLifecycle {
            status,
            subject_root,
            basis,
        } => apply_lifecycle(state, operation, status, subject_root, basis),
        DocumentAction::SetPublication { publication, basis } => {
            apply_publication(state, operation, publication, basis)
        }
    }
}

fn apply_artifact(
    state: &mut DocumentState,
    operation: &DocumentOperation,
    artifact: &str,
    content: &Option<String>,
    basis: &BTreeSet<String>,
) -> Result<(), DocumentCobError> {
    // r[impl cairn.adr_cob.immutability]
    // An accepted (or later) decision freezes its decision source artifacts.
    if matches!(
        state.lifecycle.status,
        DocumentLifecycleStatus::Accepted
            | DocumentLifecycleStatus::Published
            | DocumentLifecycleStatus::Superseded
    ) {
        return Err(DocumentCobError::ImmutableDecision);
    }
    let set = state
        .artifacts
        .get_mut(artifact)
        .ok_or_else(|| DocumentCobError::InvalidField {
            field: artifact.to_string(),
            reason: "unknown artifact id".to_string(),
        })?;
    ensure_basis(set, basis)?;
    let Some(content) = content else {
        return Ok(());
    };
    let revision_id = format!("{}:artifact:{}", operation.id, artifact);
    set.revisions.insert(revision_id.clone(), content.clone());
    set.supersedes.insert(revision_id.clone(), basis.clone());
    Ok(())
}

fn apply_discussion(
    state: &mut DocumentState,
    operation: &DocumentOperation,
    record: &DiscussionRecord,
    _basis: &BTreeSet<String>,
) -> Result<(), DocumentCobError> {
    if record.text.is_empty()
        || record.text.len() > 4096
        || !record.id.starts_with(&format!("{}:", operation.id))
    {
        return Err(DocumentCobError::InvalidField {
            field: "discussion".to_string(),
            reason: "discussion record id or text is invalid".to_string(),
        });
    }
    if let Some(reply_to) = &record.reply_to {
        if !state.discussion.contains_key(reply_to) {
            return Err(DocumentCobError::MissingBasis {
                operation: operation.id.clone(),
                basis: reply_to.clone(),
            });
        }
    }
    if state.discussion.contains_key(&record.id) {
        return Err(DocumentCobError::DuplicateOperation { id: record.id.clone() });
    }
    state.discussion.insert(record.id.clone(), record.clone());
    Ok(())
}

fn apply_link(
    state: &mut DocumentState,
    _operation: &DocumentOperation,
    link: &DocumentLink,
    _basis: &BTreeSet<String>,
) -> Result<(), DocumentCobError> {
    if link.target_id.is_empty() || link.relation.is_empty() {
        return Err(DocumentCobError::InvalidField {
            field: "link".to_string(),
            reason: "link target and relation must be present".to_string(),
        });
    }
    if state.links.contains_key(&link.id) {
        return Err(DocumentCobError::DuplicateOperation { id: link.id.clone() });
    }
    state.links.insert(link.id.clone(), link.clone());
    Ok(())
}

fn apply_lifecycle(
    state: &mut DocumentState,
    operation: &DocumentOperation,
    status: &DocumentLifecycleStatus,
    subject_root: &str,
    basis: &BTreeSet<String>,
) -> Result<(), DocumentCobError> {
    if !state.lifecycle.basis.iter().all(|base| basis.contains(base)) {
        return Err(DocumentCobError::MissingBasis {
            operation: operation.id.clone(),
            basis: "lifecycle basis".to_string(),
        });
    }
    state.lifecycle = DocumentLifecycleRecord {
        status: status.clone(),
        subject_root: subject_root.to_string(),
        basis: basis.clone(),
    };
    Ok(())
}

fn apply_publication(
    state: &mut DocumentState,
    _operation: &DocumentOperation,
    publication: &PublicationNumber,
    _basis: &BTreeSet<String>,
) -> Result<(), DocumentCobError> {
    if state.publication.is_some() {
        return Err(DocumentCobError::DuplicateOperation {
            id: "publication".to_string(),
        });
    }
    state.publication = Some(publication.clone());
    Ok(())
}

fn required_role(action: &DocumentAction) -> Option<DocumentRole> {
    match action {
        DocumentAction::Init { .. } => None,
        DocumentAction::SetArtifact { .. } => Some(DocumentRole::Author),
        DocumentAction::AddDiscussion { .. } => Some(DocumentRole::Participant),
        DocumentAction::AddLink { .. } => Some(DocumentRole::Author),
        DocumentAction::SetLifecycle { status: DocumentLifecycleStatus::Accepted, .. } => {
            Some(DocumentRole::Approver)
        }
        DocumentAction::SetLifecycle { .. } => Some(DocumentRole::Reviewer),
        DocumentAction::SetPublication { .. } => Some(DocumentRole::Publisher),
    }
}

// r[impl cairn.rfd_cob.authority.allowed]
// r[impl cairn.rfd_cob.authority.denied]
// r[impl cairn.rfd_cob.authority.ambient]
pub fn document_actor_has_role(
    state: &DocumentState,
    operation: &DocumentOperation,
    role: DocumentRole,
) -> bool {
    // r[impl cairn.rfd_cob.authority.ambient]
    // Role authority comes only from immutable root role sets bound to the actor and kind.
    let actor = operation.actor.as_str();
    let (kind_ok, has_role) = match role {
        DocumentRole::Author => (operation_kind_matches(state, operation), state.authors.contains(actor)),
        DocumentRole::Participant => (operation_kind_matches(state, operation), state.participants.contains(actor)),
        DocumentRole::Reviewer => (operation_kind_matches(state, operation), state.reviewers.contains(actor)),
        DocumentRole::Approver => (operation_kind_matches(state, operation), state.approvers.contains(actor)),
        DocumentRole::Publisher => (operation_kind_matches(state, operation), state.publishers.contains(actor)),
    };
    kind_ok && has_role
}

fn operation_kind_matches(state: &DocumentState, operation: &DocumentOperation) -> bool {
    let mut ok = true;
    for action in &operation.actions {
        if let DocumentAction::Init { state: init } = action {
            ok = ok && init.kind == state.kind;
        }
    }
    ok
}

fn ensure_basis(
    set: &DocumentRevisionSet,
    basis: &BTreeSet<String>,
) -> Result<(), DocumentCobError> {
    let heads = set.head_ids();
    if basis.is_empty() {
        if !heads.is_empty() {
            return Err(DocumentCobError::MissingBasis {
                operation: "artifact".to_string(),
                basis: "current heads".to_string(),
            });
        }
        return Ok(());
    }
    if !basis.is_subset(&heads) && !basis.iter().all(|base| set.revisions.contains_key(base)) {
        return Err(DocumentCobError::UnknownBase {
            field: "artifact".to_string(),
            base: basis
                .iter()
                .find(|base| !set.revisions.contains_key(*base))
                .cloned()
                .unwrap_or_default(),
        });
    }
    Ok(())
}

// r[impl cairn.document_collaboration.semantic_core.identity]
// r[impl cairn.document_collaboration.semantic_core.identity]
pub fn document_init_content_root(init: &DocumentInit) -> Result<String, DocumentCobError> {
    // The canonical init operation uses a fixed ID and actor so the content root is
    // transport-neutral and independent of provider identity or the signer.
    let operation = DocumentOperation {
        id: "init".to_string(),
        actor: String::new(),
        role: None,
        actions: vec![DocumentAction::Init { state: init.clone() }],
    };
    document_content_root(&initialize_document_cob(operation)?)
}

pub fn document_content_root(state: &DocumentState) -> Result<String, DocumentCobError> {
    validate_state_schema(state)?;
    let mut artifact_contents = BTreeMap::new();
    for (artifact, set) in &state.artifacts {
        let mut contents = set
            .head_ids()
            .into_iter()
            .filter_map(|head| set.content(&head).map(str::to_string))
            .collect::<Vec<_>>();
        contents.sort();
        contents.dedup();
        artifact_contents.insert(artifact.clone(), contents);
    }
    Ok(canonical_json_hash(&json!({
        "kind": state.kind,
        "document_id": state.document_id,
        "workspace_id": state.workspace_id,
        "policy_hash": state.policy_hash,
        "authors": state.authors,
        "participants": state.participants,
        "reviewers": state.reviewers,
        "approvers": state.approvers,
        "publishers": state.publishers,
        "artifacts": artifact_contents,
        "links": state.links,
        "discussion": state.discussion,
        "lifecycle": state.lifecycle,
        "evidence": state.evidence,
        "publication": state.publication,
    })))
}

pub fn document_state_hash(state: &DocumentState) -> Result<String, DocumentCobError> {
    validate_state_schema(state)?;
    Ok(canonical_json_hash(&json!(state)))
}

pub fn validate_state_schema(state: &DocumentState) -> Result<(), DocumentCobError> {
    if state.schema != DOCUMENT_COLLABORATION_SCHEMA_V1 {
        return Err(DocumentCobError::SchemaMismatch {
            expected: DOCUMENT_COLLABORATION_SCHEMA_V1.to_string(),
            actual: state.schema.clone(),
        });
    }
    Ok(())
}

// r[impl cairn.rfd_cob.profile.kind]
pub fn assert_state_kind(state: &DocumentState, expected: DocumentKind) -> Result<(), DocumentCobError> {
    let actual = DocumentKind::try_from(state.kind.as_str())?;
    if actual != expected {
        return Err(DocumentCobError::KindMismatch {
            expected: expected.as_str().to_string(),
            actual: state.kind.clone(),
        });
    }
    Ok(())
}

// r[impl cairn.rfd_cob.materialization.conflict]
pub fn document_conflict_view(state: &DocumentState) -> BTreeMap<String, Vec<String>> {
    let mut conflicts = BTreeMap::new();
    for (artifact, set) in &state.artifacts {
        let heads = set.head_ids();
        if heads.len() > 1 {
            conflicts.insert(artifact.clone(), heads.into_iter().collect());
        }
    }
    conflicts
}

pub fn document_conflicted(state: &DocumentState) -> bool {
    !document_conflict_view(state).is_empty()
}

// r[impl cairn.rfd_cob.materialization]
// r[impl cairn.rfd_cob.materialization.exact]
// r[impl cairn.rfd_cob.materialization.unsafe]
// r[impl cairn.adr_cob.materialization.exact]
pub fn plan_document_materialization(
    state: &DocumentState,
    base_destination: &str,
    max_path_bytes: usize,
) -> Result<Vec<DocumentPlannedFile>, DocumentCobError> {
    assert_state_kind(state, DocumentKind::Rfd)?;
    if document_conflicted(state) {
        return Err(DocumentCobError::ConflictedField {
            field: "artifacts".to_string(),
            heads: document_conflict_view(state)
                .values()
                .flatten()
                .cloned()
                .collect(),
        });
    }
    let mut files = Vec::new();
    for (artifact, set) in &state.artifacts {
        let heads = set.head_ids();
        let Some(head) = heads.iter().next() else {
            continue;
        };
        let Some(content) = set.content(head) else {
            continue;
        };
        let destination = format!("{base_destination}/{artifact}");
        if !safe_document_path(&destination) || destination.len() > max_path_bytes {
            return Err(DocumentCobError::InvalidField {
                field: "destination".to_string(),
                reason: "unsafe or oversized publication path".to_string(),
            });
        }
        files.push(DocumentPlannedFile {
            artifact: artifact.clone(),
            destination,
            content: content.to_string(),
            content_blake3: canonical_json_hash(&json!(content)),
        });
    }
    files.sort_by(|left, right| left.destination.cmp(&right.destination));
    Ok(files)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentPlannedFile {
    pub artifact: String,
    pub destination: String,
    pub content: String,
    pub content_blake3: String,
}

// r[impl cairn.rfd_cob.identity.registry]
// r[impl cairn.rfd_cob.identity.non_claims]
pub fn plan_publication_number(
    state: &DocumentState,
    selected_number: u64,
    registry_snapshot_identity: &str,
    publisher: &str,
) -> Result<PublicationNumber, DocumentCobError> {
    if document_conflicted(state) {
        return Err(DocumentCobError::ConflictedField {
            field: "conflicts".to_string(),
            heads: vec!["publication requires conflict-free state".to_string()],
        });
    }
    if selected_number == 0 {
        return Err(DocumentCobError::InvalidField {
            field: "number".to_string(),
            reason: "RFD publication numbers start at 1".to_string(),
        });
    }
    if registry_snapshot_identity.len() != BLAKE3_HEX_LENGTH {
        return Err(DocumentCobError::InvalidField {
            field: "registry_snapshot_identity".to_string(),
            reason: "registry snapshot identity must be a BLAKE3 hash".to_string(),
        });
    }
    let content_root = document_content_root(state)?;
    Ok(PublicationNumber {
        number: selected_number,
        registry_snapshot_identity: registry_snapshot_identity.to_string(),
        content_root,
        policy_hash: state.policy_hash.clone(),
        publisher: publisher.to_string(),
    })
}

// r[impl cairn.document_collaboration.portable]
// r[impl cairn.document_collaboration.portable.roundtrip]
// r[impl cairn.document_collaboration.portable.tamper]
pub fn serialize_document_portable(state: &DocumentState) -> Result<String, DocumentCobError> {
    let mut value = json!({
        "schema": DOCUMENT_PORTABLE_SCHEMA_V1,
        "backend": "portable",
        "state": state,
    });
    value["state_identity"] = json!(document_state_hash(state)?);
    value["content_identity"] = json!(document_content_root(state)?);
    value["portable_identity"] = json!(portable_identity(&value));
    serde_json::to_string(&value).map_err(document_json_error)
}

pub fn parse_document_portable(text: &str) -> Result<DocumentState, DocumentCobError> {
    let value: Value = serde_json::from_str(text).map_err(document_json_error)?;
    validate_portable(&value)?;
    let state: DocumentState = serde_json::from_value(value.get("state").cloned().unwrap_or(Value::Null))
        .map_err(document_json_error)?;
    validate_state_schema(&state)?;
    Ok(state)
}

fn validate_portable(value: &Value) -> Result<(), DocumentCobError> {
    if value.get("schema").and_then(Value::as_str) != Some(DOCUMENT_PORTABLE_SCHEMA_V1) {
        return Err(DocumentCobError::PortableTamper {
            field: "schema".to_string(),
        });
    }
    if value.get("backend").and_then(Value::as_str) != Some("portable") {
        return Err(DocumentCobError::PortableTamper {
            field: "backend".to_string(),
        });
    }
    let state: DocumentState =
        serde_json::from_value(value.get("state").cloned().unwrap_or(Value::Null))
            .map_err(document_json_error)?;
    if value.get("state_identity").and_then(Value::as_str) != Some(&document_state_hash(&state)?) {
        return Err(DocumentCobError::PortableTamper {
            field: "state_identity".to_string(),
        });
    }
    if value.get("content_identity").and_then(Value::as_str) != Some(&document_content_root(&state)?) {
        return Err(DocumentCobError::PortableTamper {
            field: "content_identity".to_string(),
        });
    }
    if value.get("portable_identity").and_then(Value::as_str) != Some(&portable_identity(value)) {
        return Err(DocumentCobError::PortableTamper {
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

fn safe_document_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains("..")
        && !path.contains("//")
        && !path.contains('\\')
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

fn document_json_error<E: std::fmt::Display>(error: E) -> DocumentCobError {
    DocumentCobError::Factory(format!("document JSON error: {error}"))
}
