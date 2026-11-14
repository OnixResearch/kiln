use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::verified::{evidence::canonical_json_hash, lifecycle::validate_change_name};

// r[impl cairn.radicle_change_cob.lossless_change_state]
// r[impl cairn.radicle_change_cob.deterministic_reducer]
// r[impl cairn.radicle_change_cob.evidence_boundary]
// r[impl cairn.radicle_change_cob.materialized_view]

pub const CHANGE_COLLABORATION_SCHEMA_V1: &str = "cairn.change-collaboration.v1";
pub const RADICLE_CHANGE_COB_SCHEMA_V1: &str = "cairn.radicle-change-cob.v1";
pub const RADICLE_CHANGE_COB_TYPE_NAME: &str = "xyz.onixresearch.cairn-change";
pub const BASE_REVISION_SEPARATOR: char = ':';
pub const ARTIFACT_PROPOSAL: &str = "proposal";
pub const ARTIFACT_DESIGN: &str = "design";
pub const ARTIFACT_TASKS: &str = "tasks";
pub const ARTIFACT_METADATA: &str = "metadata";
pub const ARTIFACT_SPEC_PREFIX: &str = "spec:";

pub const MAX_CHANGE_ID_BYTES: usize = 128;
pub const MAX_REPOSITORY_ID_BYTES: usize = 512;
pub const MAX_WORKSPACE_ID_BYTES: usize = MAX_REPOSITORY_ID_BYTES;
pub const MAX_BASE_REVISION_KIND_BYTES: usize = 64;
pub const MAX_BASE_REVISION_VALUE_BYTES: usize = 4_096;
pub const MAX_OPERATION_ID_BYTES: usize = 128;
pub const MAX_ACTOR_ID_BYTES: usize = 256;
pub const MAX_ARTIFACT_ID_BYTES: usize = 256;
pub const MAX_ARTIFACTS: usize = 256;
pub const MAX_ARTIFACT_BYTES: usize = 1_048_576;
pub const MAX_ACTIONS_PER_OPERATION: usize = 64;
pub const MAX_DELEGATES: usize = 256;
pub const MAX_OPERATIONS: usize = 16_384;
pub const MAX_REVISIONS_PER_ARTIFACT: usize = 4_096;
pub const MAX_REVISION_BASIS_IDS: usize = MAX_REVISIONS_PER_ARTIFACT;
pub const MAX_LIFECYCLE_REVISIONS: usize = 4_096;
pub const MAX_LIFECYCLE_EVIDENCE_IDS: usize = 256;
pub const MAX_EVIDENCE_RECORDS: usize = 1_024;
pub const MAX_EVIDENCE_NON_CLAIMS: usize = 64;
pub const MAX_EVIDENCE_TEXT_BYTES: usize = 4_096;
pub const BLAKE3_HEX_LENGTH: usize = 64;
pub const GIT_SHA1_HEX_LENGTH: usize = 40;
pub const GIT_SHA256_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChangeArtifactId(String);

impl ChangeArtifactId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ChangeCobError> {
        let artifact = Self(value.into());
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn proposal() -> Self {
        Self(ARTIFACT_PROPOSAL.to_string())
    }

    pub fn design() -> Self {
        Self(ARTIFACT_DESIGN.to_string())
    }

    pub fn tasks() -> Self {
        Self(ARTIFACT_TASKS.to_string())
    }

    pub fn metadata() -> Self {
        Self(ARTIFACT_METADATA.to_string())
    }

    pub fn delta_spec(spec_id: &str) -> Result<Self, ChangeCobError> {
        Self::parse(format!("{ARTIFACT_SPEC_PREFIX}{spec_id}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn relative_path(&self) -> Result<String, ChangeCobError> {
        self.validate()?;
        match self.as_str() {
            ARTIFACT_PROPOSAL => Ok("proposal.md".to_string()),
            ARTIFACT_DESIGN => Ok("design.md".to_string()),
            ARTIFACT_TASKS => Ok("tasks.md".to_string()),
            ARTIFACT_METADATA => Ok("metadata.json".to_string()),
            value => {
                let spec_id = value.strip_prefix(ARTIFACT_SPEC_PREFIX).ok_or_else(|| {
                    ChangeCobError::InvalidArtifactId {
                        artifact: value.to_string(),
                    }
                })?;
                Ok(format!("specs/{spec_id}/spec.md"))
            }
        }
    }

    fn validate(&self) -> Result<(), ChangeCobError> {
        validate_text_limit("artifact_id", &self.0, MAX_ARTIFACT_ID_BYTES)?;
        if matches!(
            self.as_str(),
            ARTIFACT_PROPOSAL | ARTIFACT_DESIGN | ARTIFACT_TASKS | ARTIFACT_METADATA
        ) {
            return Ok(());
        }
        let Some(spec_id) = self.as_str().strip_prefix(ARTIFACT_SPEC_PREFIX) else {
            return Err(ChangeCobError::InvalidArtifactId {
                artifact: self.0.clone(),
            });
        };
        validate_stable_id("artifact_spec_id", spec_id).map_err(|_| {
            ChangeCobError::InvalidArtifactId {
                artifact: self.0.clone(),
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCobInit {
    pub schema: String,
    pub change_id: String,
    pub repository_id: String,
    pub base_git_oid: String,
    pub policy_hash: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub delegates: BTreeSet<String>,
    pub artifacts: BTreeMap<ChangeArtifactId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCobEvidence {
    pub blake3: String,
    pub schema: String,
    pub role: String,
    pub subject_root: String,
    pub policy_hash: String,
    pub git_oid: Option<String>,
    pub uri: Option<String>,
    pub non_claims: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeLifecycleStatus {
    Open,
    ReviewRequested,
    Approved,
    ArchiveRequested,
    Archived,
}

impl ChangeLifecycleStatus {
    fn requires_delegate(self) -> bool {
        matches!(
            self,
            Self::Approved | Self::ArchiveRequested | Self::Archived
        )
    }

    fn requires_evidence(self) -> bool {
        matches!(self, Self::Approved | Self::Archived)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeLifecycleRecord {
    pub status: ChangeLifecycleStatus,
    pub subject_root: String,
    pub evidence: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChangeCobAction {
    Init {
        state: ChangeCobInit,
    },
    SetArtifact {
        artifact: ChangeArtifactId,
        content: Option<String>,
        basis: BTreeSet<String>,
    },
    AttachEvidence {
        evidence: ChangeCobEvidence,
    },
    SetLifecycle {
        status: ChangeLifecycleStatus,
        subject_root: String,
        evidence: BTreeSet<String>,
        basis: BTreeSet<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCobOperation {
    pub id: String,
    pub actor: String,
    pub is_delegate: bool,
    pub actions: Vec<ChangeCobAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeRevision<T> {
    pub actor: String,
    pub basis: BTreeSet<String>,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeRevisionSet<T> {
    pub revisions: BTreeMap<String, ChangeRevision<T>>,
}

impl<T> ChangeRevisionSet<T> {
    fn from_root(operation_id: &str, actor: &str, value: T) -> Self {
        Self {
            revisions: BTreeMap::from([(
                operation_id.to_string(),
                ChangeRevision {
                    actor: actor.to_string(),
                    basis: BTreeSet::new(),
                    value,
                },
            )]),
        }
    }

    pub fn head_ids(&self) -> BTreeSet<String> {
        let superseded = self
            .revisions
            .values()
            .flat_map(|revision| revision.basis.iter().cloned())
            .collect::<BTreeSet<_>>();
        self.revisions
            .keys()
            .filter(|revision_id| !superseded.contains(*revision_id))
            .cloned()
            .collect()
    }

    pub fn heads(&self) -> Vec<(&str, &ChangeRevision<T>)> {
        let head_ids = self.head_ids();
        self.revisions
            .iter()
            .filter(|(revision_id, _)| head_ids.contains(*revision_id))
            .map(|(revision_id, revision)| (revision_id.as_str(), revision))
            .collect()
    }

    fn insert(
        &mut self,
        operation_id: &str,
        actor: &str,
        basis: &BTreeSet<String>,
        value: T,
        field: &str,
        revision_limit: usize,
    ) -> Result<(), ChangeCobError> {
        if self.revisions.contains_key(operation_id) {
            return Err(ChangeCobError::DuplicateRevision {
                field: field.to_string(),
                revision: operation_id.to_string(),
            });
        }
        for revision_id in basis {
            if !self.revisions.contains_key(revision_id) {
                return Err(ChangeCobError::UnknownBasis {
                    field: field.to_string(),
                    revision: revision_id.clone(),
                });
            }
        }
        let next_revision_count = self.revisions.len().saturating_add(1);
        validate_count_limit(field, next_revision_count, revision_limit)?;
        self.revisions.insert(
            operation_id.to_string(),
            ChangeRevision {
                actor: actor.to_string(),
                basis: basis.clone(),
                value,
            },
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCobState {
    pub schema: String,
    pub change_id: String,
    pub repository_id: String,
    pub base_git_oid: String,
    pub policy_hash: String,
    pub creator: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub delegates: BTreeSet<String>,
    pub artifacts: BTreeMap<ChangeArtifactId, ChangeRevisionSet<Option<String>>>,
    pub evidence: BTreeMap<String, ChangeCobEvidence>,
    pub lifecycle: ChangeRevisionSet<ChangeLifecycleRecord>,
    pub operations: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeMaterializedFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeLifecycleView {
    Current(ChangeLifecycleRecord),
    Stale {
        record: ChangeLifecycleRecord,
        current_content_root: String,
    },
    Conflicted {
        revisions: Vec<String>,
    },
    ContentConflicted {
        artifacts: Vec<String>,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ChangeCobError {
    #[error("root operation must contain exactly one init action")]
    InvalidRootShape,
    #[error("update operation must contain at least one action")]
    EmptyOperation,
    #[error("init action is only valid in the root operation")]
    InitInUpdate,
    #[error("unsupported change COB schema: {schema}")]
    UnsupportedSchema { schema: String },
    #[error("invalid {field}: {reason}")]
    InvalidField { field: String, reason: String },
    #[error("invalid artifact id: {artifact}")]
    InvalidArtifactId { artifact: String },
    #[error("{field} count {actual} exceeds protocol limit {limit}")]
    CountLimitExceeded {
        field: String,
        actual: usize,
        limit: usize,
    },
    #[error("{field} byte length {actual} exceeds protocol limit {limit}")]
    ByteLimitExceeded {
        field: String,
        actual: usize,
        limit: usize,
    },
    #[error("operation id already applied: {operation}")]
    DuplicateOperation { operation: String },
    #[error("operation repeats artifact action: {artifact}")]
    DuplicateArtifactAction { artifact: String },
    #[error("operation contains more than one lifecycle action")]
    DuplicateLifecycleAction,
    #[error("unknown basis revision {revision} for {field}")]
    UnknownBasis { field: String, revision: String },
    #[error("duplicate revision {revision} for {field}")]
    DuplicateRevision { field: String, revision: String },
    #[error("actor {actor} is not authorized for {action}")]
    Unauthorized { actor: String, action: String },
    #[error("evidence hash {blake3} resolves to conflicting declarations")]
    ConflictingEvidence { blake3: String },
    #[error("evidence does not exist: {blake3}")]
    MissingEvidence { blake3: String },
    #[error("evidence {blake3} is not current for subject {subject_root}")]
    StaleEvidence {
        blake3: String,
        subject_root: String,
    },
    #[error("lifecycle subject {subject_root} does not match current content root {current_root}")]
    StaleLifecycleSubject {
        subject_root: String,
        current_root: String,
    },
    #[error("lifecycle status {status:?} requires current evidence")]
    LifecycleEvidenceRequired { status: ChangeLifecycleStatus },
    #[error("change content has conflicting artifacts: {artifacts:?}")]
    ArtifactConflict { artifacts: Vec<String> },
    #[error("change state serialization failed: {reason}")]
    Serialization { reason: String },
}

pub fn initialize_change_cob(
    operation: ChangeCobOperation,
) -> Result<ChangeCobState, ChangeCobError> {
    validate_operation_envelope(&operation)?;
    if operation.actions.len() != 1 {
        return Err(ChangeCobError::InvalidRootShape);
    }
    let mut actions = operation.actions;
    let Some(ChangeCobAction::Init { state: init }) = actions.pop() else {
        return Err(ChangeCobError::InvalidRootShape);
    };
    validate_change_cob_init(&init)?;

    let artifacts = init
        .artifacts
        .into_iter()
        .map(|(artifact, content)| {
            (
                artifact,
                ChangeRevisionSet::from_root(&operation.id, &operation.actor, Some(content)),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut state = ChangeCobState {
        schema: init.schema,
        change_id: init.change_id,
        repository_id: init.repository_id,
        base_git_oid: init.base_git_oid,
        policy_hash: init.policy_hash,
        creator: operation.actor.clone(),
        delegates: init.delegates,
        artifacts,
        evidence: BTreeMap::new(),
        lifecycle: ChangeRevisionSet {
            revisions: BTreeMap::new(),
        },
        operations: BTreeSet::from([operation.id.clone()]),
    };
    let content_root = change_cob_content_root(&state)?;
    state.lifecycle = ChangeRevisionSet::from_root(
        &operation.id,
        &operation.actor,
        ChangeLifecycleRecord {
            status: ChangeLifecycleStatus::Open,
            subject_root: content_root,
            evidence: BTreeSet::new(),
        },
    );
    Ok(state)
}

pub fn apply_change_cob_operation(
    state: &mut ChangeCobState,
    operation: ChangeCobOperation,
) -> Result<(), ChangeCobError> {
    validate_operation_envelope(&operation)?;
    validate_state_schema(state)?;
    if operation.actions.is_empty() {
        return Err(ChangeCobError::EmptyOperation);
    }
    if state.operations.contains(&operation.id) {
        return Err(ChangeCobError::DuplicateOperation {
            operation: operation.id,
        });
    }
    let next_operation_count = state.operations.len().saturating_add(1);
    validate_count_limit("operations", next_operation_count, MAX_OPERATIONS)?;
    validate_operation_action_set(&operation.actions)?;

    let mut candidate = state.clone();
    for action in operation.actions {
        apply_action(
            &mut candidate,
            &operation.id,
            &operation.actor,
            operation.is_delegate,
            action,
        )?;
    }
    candidate.operations.insert(operation.id);
    *state = candidate;
    Ok(())
}

pub fn change_cob_content_root(state: &ChangeCobState) -> Result<String, ChangeCobError> {
    validate_state_schema(state)?;
    let conflicts = conflicted_artifacts(state);
    if !conflicts.is_empty() {
        return Err(ChangeCobError::ArtifactConflict {
            artifacts: conflicts,
        });
    }
    let artifacts = resolved_artifacts(state)?
        .into_iter()
        .filter_map(|(artifact, content)| content.map(|content| (artifact, content)))
        .collect();
    if state.schema == RADICLE_CHANGE_COB_SCHEMA_V1 {
        return hash_serializable(&LegacyChangeContentIdentity {
            schema: &state.schema,
            change_id: &state.change_id,
            repository_id: &state.repository_id,
            base_git_oid: &state.base_git_oid,
            policy_hash: &state.policy_hash,
            creator: &state.creator,
            artifacts,
        });
    }
    let base_revision = decode_base_revision(&state.base_git_oid)?;
    hash_serializable(&ChangeContentIdentity {
        schema: &state.schema,
        change_id: &state.change_id,
        workspace_id: &state.repository_id,
        base_revision,
        policy_hash: &state.policy_hash,
        creator: &state.creator,
        delegates: &state.delegates,
        artifacts,
    })
}

pub fn change_cob_state_hash(state: &ChangeCobState) -> Result<String, ChangeCobError> {
    validate_state_schema(state)?;
    if state.schema == RADICLE_CHANGE_COB_SCHEMA_V1 {
        return hash_serializable(state);
    }
    let base_revision = decode_base_revision(&state.base_git_oid)?;
    hash_serializable(&ChangeStateIdentity {
        schema: &state.schema,
        change_id: &state.change_id,
        workspace_id: &state.repository_id,
        base_revision,
        policy_hash: &state.policy_hash,
        creator: &state.creator,
        delegates: &state.delegates,
        artifacts: &state.artifacts,
        evidence: &state.evidence,
        lifecycle: &state.lifecycle,
        operations: &state.operations,
    })
}

pub fn materialize_change_cob(
    state: &ChangeCobState,
) -> Result<Vec<ChangeMaterializedFile>, ChangeCobError> {
    let _content_root = change_cob_content_root(state)?;
    let artifacts = resolved_artifacts(state)?;
    let mut files = artifacts
        .into_iter()
        .filter_map(|(artifact, content)| {
            content.map(|content| {
                artifact
                    .relative_path()
                    .map(|path| ChangeMaterializedFile { path, content })
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

pub fn current_change_cob_evidence(
    state: &ChangeCobState,
) -> Result<Vec<ChangeCobEvidence>, ChangeCobError> {
    let content_root = change_cob_content_root(state)?;
    Ok(state
        .evidence
        .values()
        .filter(|evidence| {
            evidence.subject_root == content_root && evidence.policy_hash == state.policy_hash
        })
        .cloned()
        .collect())
}

pub fn change_cob_lifecycle_view(
    state: &ChangeCobState,
) -> Result<ChangeLifecycleView, ChangeCobError> {
    let content_root = match change_cob_content_root(state) {
        Ok(content_root) => content_root,
        Err(ChangeCobError::ArtifactConflict { artifacts }) => {
            return Ok(ChangeLifecycleView::ContentConflicted { artifacts });
        }
        Err(error) => return Err(error),
    };
    let heads = state.lifecycle.heads();
    if heads.len() != 1 {
        return Ok(ChangeLifecycleView::Conflicted {
            revisions: heads
                .into_iter()
                .map(|(revision_id, _)| revision_id.to_string())
                .collect(),
        });
    }
    let Some((_, revision)) = heads.into_iter().next() else {
        return Ok(ChangeLifecycleView::Conflicted {
            revisions: Vec::new(),
        });
    };
    if revision.value.subject_root == content_root {
        return Ok(ChangeLifecycleView::Current(revision.value.clone()));
    }
    Ok(ChangeLifecycleView::Stale {
        record: revision.value.clone(),
        current_content_root: content_root,
    })
}

fn apply_action(
    state: &mut ChangeCobState,
    operation_id: &str,
    actor: &str,
    is_delegate: bool,
    action: ChangeCobAction,
) -> Result<(), ChangeCobError> {
    match action {
        ChangeCobAction::Init { .. } => Err(ChangeCobError::InitInUpdate),
        ChangeCobAction::SetArtifact {
            artifact,
            content,
            basis,
        } => apply_artifact_action(
            state,
            operation_id,
            actor,
            is_delegate,
            artifact,
            content,
            basis,
        ),
        ChangeCobAction::AttachEvidence { evidence } => apply_evidence_action(state, evidence),
        ChangeCobAction::SetLifecycle {
            status,
            subject_root,
            evidence,
            basis,
        } => apply_lifecycle_action(
            state,
            operation_id,
            actor,
            is_delegate,
            status,
            subject_root,
            evidence,
            basis,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_artifact_action(
    state: &mut ChangeCobState,
    operation_id: &str,
    actor: &str,
    is_delegate: bool,
    artifact: ChangeArtifactId,
    content: Option<String>,
    basis: BTreeSet<String>,
) -> Result<(), ChangeCobError> {
    authorize_creator_or_delegate(state, actor, is_delegate, "set_artifact")?;
    artifact.validate()?;
    if let Some(content) = &content {
        validate_artifact_content(content)?;
    }
    validate_count_limit("artifact.basis", basis.len(), MAX_REVISION_BASIS_IDS)?;
    if !state.artifacts.contains_key(&artifact) {
        let next_artifact_count = state.artifacts.len().saturating_add(1);
        validate_count_limit("artifacts", next_artifact_count, MAX_ARTIFACTS)?;
        if !basis.is_empty() {
            let revision = basis.iter().next().cloned().unwrap_or_default();
            return Err(ChangeCobError::UnknownBasis {
                field: artifact.as_str().to_string(),
                revision,
            });
        }
        state.artifacts.insert(
            artifact,
            ChangeRevisionSet::from_root(operation_id, actor, content),
        );
        return Ok(());
    }
    let Some(history) = state.artifacts.get_mut(&artifact) else {
        return Err(ChangeCobError::InvalidArtifactId {
            artifact: artifact.as_str().to_string(),
        });
    };
    history.insert(
        operation_id,
        actor,
        &basis,
        content,
        artifact.as_str(),
        MAX_REVISIONS_PER_ARTIFACT,
    )
}

fn apply_evidence_action(
    state: &mut ChangeCobState,
    evidence: ChangeCobEvidence,
) -> Result<(), ChangeCobError> {
    validate_evidence(&evidence)?;
    if let Some(existing) = state.evidence.get(&evidence.blake3) {
        if existing == &evidence {
            return Ok(());
        }
        return Err(ChangeCobError::ConflictingEvidence {
            blake3: evidence.blake3,
        });
    }
    let next_evidence_count = state.evidence.len().saturating_add(1);
    validate_count_limit("evidence", next_evidence_count, MAX_EVIDENCE_RECORDS)?;
    state.evidence.insert(evidence.blake3.clone(), evidence);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_lifecycle_action(
    state: &mut ChangeCobState,
    operation_id: &str,
    actor: &str,
    is_delegate: bool,
    status: ChangeLifecycleStatus,
    subject_root: String,
    evidence_hashes: BTreeSet<String>,
    basis: BTreeSet<String>,
) -> Result<(), ChangeCobError> {
    authorize_lifecycle(state, actor, is_delegate, status)?;
    validate_blake3("lifecycle.subject_root", &subject_root)?;
    validate_count_limit("lifecycle.basis", basis.len(), MAX_REVISION_BASIS_IDS)?;
    validate_count_limit(
        "lifecycle.evidence",
        evidence_hashes.len(),
        MAX_LIFECYCLE_EVIDENCE_IDS,
    )?;
    let current_root = change_cob_content_root(state)?;
    if subject_root != current_root {
        return Err(ChangeCobError::StaleLifecycleSubject {
            subject_root,
            current_root,
        });
    }
    if status.requires_evidence() && evidence_hashes.is_empty() {
        return Err(ChangeCobError::LifecycleEvidenceRequired { status });
    }
    for evidence_hash in &evidence_hashes {
        let evidence =
            state
                .evidence
                .get(evidence_hash)
                .ok_or_else(|| ChangeCobError::MissingEvidence {
                    blake3: evidence_hash.clone(),
                })?;
        if evidence.subject_root != current_root || evidence.policy_hash != state.policy_hash {
            return Err(ChangeCobError::StaleEvidence {
                blake3: evidence_hash.clone(),
                subject_root: current_root,
            });
        }
    }
    state.lifecycle.insert(
        operation_id,
        actor,
        &basis,
        ChangeLifecycleRecord {
            status,
            subject_root: current_root,
            evidence: evidence_hashes,
        },
        "lifecycle",
        MAX_LIFECYCLE_REVISIONS,
    )
}

fn validate_operation_envelope(operation: &ChangeCobOperation) -> Result<(), ChangeCobError> {
    validate_text_limit("operation_id", &operation.id, MAX_OPERATION_ID_BYTES)?;
    validate_text_limit("actor", &operation.actor, MAX_ACTOR_ID_BYTES)?;
    validate_count_limit(
        "operation.actions",
        operation.actions.len(),
        MAX_ACTIONS_PER_OPERATION,
    )
}

fn validate_operation_action_set(actions: &[ChangeCobAction]) -> Result<(), ChangeCobError> {
    let mut artifacts = BTreeSet::new();
    let mut lifecycle_seen = false;
    for action in actions {
        match action {
            ChangeCobAction::SetArtifact { artifact, .. } => {
                artifact.validate()?;
                if !artifacts.insert(artifact.clone()) {
                    return Err(ChangeCobError::DuplicateArtifactAction {
                        artifact: artifact.as_str().to_string(),
                    });
                }
            }
            ChangeCobAction::SetLifecycle { .. } => {
                if lifecycle_seen {
                    return Err(ChangeCobError::DuplicateLifecycleAction);
                }
                lifecycle_seen = true;
            }
            ChangeCobAction::Init { .. } | ChangeCobAction::AttachEvidence { .. } => {}
        }
    }
    Ok(())
}

pub fn validate_change_cob_init(init: &ChangeCobInit) -> Result<(), ChangeCobError> {
    if !matches!(
        init.schema.as_str(),
        CHANGE_COLLABORATION_SCHEMA_V1 | RADICLE_CHANGE_COB_SCHEMA_V1
    ) {
        return Err(ChangeCobError::UnsupportedSchema {
            schema: init.schema.clone(),
        });
    }
    validate_text_limit("change_id", &init.change_id, MAX_CHANGE_ID_BYTES)?;
    validate_change_name(&init.change_id).map_err(|reason| ChangeCobError::InvalidField {
        field: "change_id".to_string(),
        reason,
    })?;
    validate_text_limit("workspace_id", &init.repository_id, MAX_WORKSPACE_ID_BYTES)?;
    if init.schema == RADICLE_CHANGE_COB_SCHEMA_V1 {
        validate_git_oid("base_git_oid", &init.base_git_oid)?;
    } else {
        let _base_revision = decode_base_revision(&init.base_git_oid)?;
    }
    validate_blake3("policy_hash", &init.policy_hash)?;
    if init.schema == RADICLE_CHANGE_COB_SCHEMA_V1 && !init.delegates.is_empty() {
        return Err(ChangeCobError::InvalidField {
            field: "delegates".to_string(),
            reason: "legacy Radicle roots cannot embed Cairn delegate authority".to_string(),
        });
    }
    validate_count_limit("delegates", init.delegates.len(), MAX_DELEGATES)?;
    for delegate in &init.delegates {
        validate_text_limit("delegate", delegate, MAX_ACTOR_ID_BYTES)?;
    }
    validate_count_limit("artifacts", init.artifacts.len(), MAX_ARTIFACTS)?;
    for (artifact, content) in &init.artifacts {
        artifact.validate()?;
        validate_artifact_content(content)?;
    }
    Ok(())
}

pub fn validate_change_cob_state(state: &ChangeCobState) -> Result<(), ChangeCobError> {
    validate_state_schema(state)?;
    validate_change_cob_init(&ChangeCobInit {
        schema: state.schema.clone(),
        change_id: state.change_id.clone(),
        repository_id: state.repository_id.clone(),
        base_git_oid: state.base_git_oid.clone(),
        policy_hash: state.policy_hash.clone(),
        delegates: state.delegates.clone(),
        artifacts: BTreeMap::new(),
    })?;
    validate_text_limit("creator", &state.creator, MAX_ACTOR_ID_BYTES)?;
    if state.delegates.contains(&state.creator) {
        return Err(ChangeCobError::InvalidField {
            field: "delegates".to_string(),
            reason: "creator must not also be an embedded delegate".to_string(),
        });
    }
    validate_count_limit("operations", state.operations.len(), MAX_OPERATIONS)?;
    if state.operations.is_empty() {
        return Err(ChangeCobError::InvalidField {
            field: "operations".to_string(),
            reason: "must contain at least one operation".to_string(),
        });
    }
    for operation_id in &state.operations {
        validate_text_limit("operation_id", operation_id, MAX_OPERATION_ID_BYTES)?;
    }

    validate_count_limit("artifacts", state.artifacts.len(), MAX_ARTIFACTS)?;
    for (artifact, history) in &state.artifacts {
        artifact.validate()?;
        validate_revision_history(
            artifact.as_str(),
            history,
            &state.operations,
            MAX_REVISIONS_PER_ARTIFACT,
            |content| {
                if let Some(content) = content {
                    validate_artifact_content(content)?;
                }
                Ok(())
            },
        )?;
    }

    validate_count_limit("evidence", state.evidence.len(), MAX_EVIDENCE_RECORDS)?;
    for (evidence_id, evidence) in &state.evidence {
        validate_evidence(evidence)?;
        if evidence_id != &evidence.blake3 {
            return Err(ChangeCobError::InvalidField {
                field: "evidence".to_string(),
                reason: format!(
                    "evidence map key {evidence_id} does not match declared hash {}",
                    evidence.blake3
                ),
            });
        }
    }

    validate_revision_history(
        "lifecycle",
        &state.lifecycle,
        &state.operations,
        MAX_LIFECYCLE_REVISIONS,
        |record| {
            validate_blake3("lifecycle.subject_root", &record.subject_root)?;
            validate_count_limit(
                "lifecycle.evidence",
                record.evidence.len(),
                MAX_LIFECYCLE_EVIDENCE_IDS,
            )?;
            for evidence_id in &record.evidence {
                validate_blake3("lifecycle.evidence_id", evidence_id)?;
                if !state.evidence.contains_key(evidence_id) {
                    return Err(ChangeCobError::MissingEvidence {
                        blake3: evidence_id.clone(),
                    });
                }
            }
            Ok(())
        },
    )?;
    Ok(())
}

fn validate_revision_history<T, F>(
    field: &str,
    history: &ChangeRevisionSet<T>,
    operations: &BTreeSet<String>,
    revision_limit: usize,
    mut validate_value: F,
) -> Result<(), ChangeCobError>
where
    F: FnMut(&T) -> Result<(), ChangeCobError>,
{
    if history.revisions.is_empty() {
        return Err(ChangeCobError::InvalidField {
            field: field.to_string(),
            reason: "revision history must not be empty".to_string(),
        });
    }
    validate_count_limit(field, history.revisions.len(), revision_limit)?;
    for (revision_id, revision) in &history.revisions {
        validate_text_limit("revision_id", revision_id, MAX_OPERATION_ID_BYTES)?;
        if !operations.contains(revision_id) {
            return Err(ChangeCobError::InvalidField {
                field: field.to_string(),
                reason: format!("revision {revision_id} is absent from operation inventory"),
            });
        }
        validate_text_limit("revision.actor", &revision.actor, MAX_ACTOR_ID_BYTES)?;
        validate_count_limit(
            "revision.basis",
            revision.basis.len(),
            MAX_REVISION_BASIS_IDS,
        )?;
        for basis in &revision.basis {
            if !history.revisions.contains_key(basis) {
                return Err(ChangeCobError::UnknownBasis {
                    field: field.to_string(),
                    revision: basis.clone(),
                });
            }
        }
        validate_value(&revision.value)?;
    }
    Ok(())
}

fn validate_state_schema(state: &ChangeCobState) -> Result<(), ChangeCobError> {
    if !matches!(
        state.schema.as_str(),
        CHANGE_COLLABORATION_SCHEMA_V1 | RADICLE_CHANGE_COB_SCHEMA_V1
    ) {
        return Err(ChangeCobError::UnsupportedSchema {
            schema: state.schema.clone(),
        });
    }
    Ok(())
}

fn validate_artifact_content(content: &str) -> Result<(), ChangeCobError> {
    validate_byte_limit("artifact_content", content, MAX_ARTIFACT_BYTES)?;
    if content.contains('\0') {
        return Err(ChangeCobError::InvalidField {
            field: "artifact_content".to_string(),
            reason: "NUL bytes are not valid lifecycle text".to_string(),
        });
    }
    Ok(())
}

fn validate_evidence(evidence: &ChangeCobEvidence) -> Result<(), ChangeCobError> {
    validate_blake3("evidence.blake3", &evidence.blake3)?;
    validate_text_limit("evidence.schema", &evidence.schema, MAX_EVIDENCE_TEXT_BYTES)?;
    validate_text_limit("evidence.role", &evidence.role, MAX_EVIDENCE_TEXT_BYTES)?;
    validate_blake3("evidence.subject_root", &evidence.subject_root)?;
    validate_blake3("evidence.policy_hash", &evidence.policy_hash)?;
    if let Some(git_oid) = &evidence.git_oid {
        validate_git_oid("evidence.git_oid", git_oid)?;
    }
    if let Some(uri) = &evidence.uri {
        validate_text_limit("evidence.uri", uri, MAX_EVIDENCE_TEXT_BYTES)?;
    }
    if evidence.non_claims.is_empty() {
        return Err(ChangeCobError::InvalidField {
            field: "evidence.non_claims".to_string(),
            reason: "at least one explicit non-claim is required".to_string(),
        });
    }
    validate_count_limit(
        "evidence.non_claims",
        evidence.non_claims.len(),
        MAX_EVIDENCE_NON_CLAIMS,
    )?;
    for non_claim in &evidence.non_claims {
        validate_text_limit("evidence.non_claim", non_claim, MAX_EVIDENCE_TEXT_BYTES)?;
    }
    Ok(())
}

fn authorize_creator_or_delegate(
    state: &ChangeCobState,
    actor: &str,
    is_delegate: bool,
    action: &str,
) -> Result<(), ChangeCobError> {
    if is_delegate || actor == state.creator {
        return Ok(());
    }
    Err(ChangeCobError::Unauthorized {
        actor: actor.to_string(),
        action: action.to_string(),
    })
}

fn authorize_lifecycle(
    state: &ChangeCobState,
    actor: &str,
    is_delegate: bool,
    status: ChangeLifecycleStatus,
) -> Result<(), ChangeCobError> {
    if status.requires_delegate() {
        if is_delegate {
            return Ok(());
        }
        return Err(ChangeCobError::Unauthorized {
            actor: actor.to_string(),
            action: format!("set_lifecycle:{status:?}"),
        });
    }
    authorize_creator_or_delegate(state, actor, is_delegate, "set_lifecycle")
}

fn resolved_artifacts(
    state: &ChangeCobState,
) -> Result<BTreeMap<ChangeArtifactId, Option<String>>, ChangeCobError> {
    let mut resolved = BTreeMap::new();
    for (artifact, history) in &state.artifacts {
        let heads = history.heads();
        if heads.len() != 1 {
            return Err(ChangeCobError::ArtifactConflict {
                artifacts: vec![artifact.as_str().to_string()],
            });
        }
        let Some((_, revision)) = heads.into_iter().next() else {
            return Err(ChangeCobError::ArtifactConflict {
                artifacts: vec![artifact.as_str().to_string()],
            });
        };
        resolved.insert(artifact.clone(), revision.value.clone());
    }
    Ok(resolved)
}

fn conflicted_artifacts(state: &ChangeCobState) -> Vec<String> {
    state
        .artifacts
        .iter()
        .filter(|(_, history)| history.head_ids().len() != 1)
        .map(|(artifact, _)| artifact.as_str().to_string())
        .collect()
}

fn validate_count_limit(field: &str, actual: usize, limit: usize) -> Result<(), ChangeCobError> {
    if actual > limit {
        return Err(ChangeCobError::CountLimitExceeded {
            field: field.to_string(),
            actual,
            limit,
        });
    }
    Ok(())
}

fn validate_text_limit(field: &str, value: &str, limit: usize) -> Result<(), ChangeCobError> {
    if value.is_empty() {
        return Err(ChangeCobError::InvalidField {
            field: field.to_string(),
            reason: "must not be empty".to_string(),
        });
    }
    validate_byte_limit(field, value, limit)
}

fn validate_byte_limit(field: &str, value: &str, limit: usize) -> Result<(), ChangeCobError> {
    let actual = value.len();
    if actual > limit {
        return Err(ChangeCobError::ByteLimitExceeded {
            field: field.to_string(),
            actual,
            limit,
        });
    }
    Ok(())
}

fn validate_stable_id(field: &str, value: &str) -> Result<(), ChangeCobError> {
    validate_text_limit(field, value, MAX_ARTIFACT_ID_BYTES)?;
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Ok(());
    }
    Err(ChangeCobError::InvalidField {
        field: field.to_string(),
        reason: "must contain only ASCII letters, digits, underscores, dots, or hyphens"
            .to_string(),
    })
}

fn validate_blake3(field: &str, value: &str) -> Result<(), ChangeCobError> {
    validate_lower_hex(field, value, &[BLAKE3_HEX_LENGTH])
}

fn validate_git_oid(field: &str, value: &str) -> Result<(), ChangeCobError> {
    validate_lower_hex(field, value, &[GIT_SHA1_HEX_LENGTH, GIT_SHA256_HEX_LENGTH])
}

fn validate_lower_hex(
    field: &str,
    value: &str,
    allowed_lengths: &[usize],
) -> Result<(), ChangeCobError> {
    if allowed_lengths.contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Ok(());
    }
    Err(ChangeCobError::InvalidField {
        field: field.to_string(),
        reason: format!("must be lowercase hexadecimal with one of lengths {allowed_lengths:?}"),
    })
}

fn hash_serializable<T: Serialize>(value: &T) -> Result<String, ChangeCobError> {
    let json = serde_json::to_value(value).map_err(|error| ChangeCobError::Serialization {
        reason: error.to_string(),
    })?;
    Ok(canonical_json_hash(&json))
}

#[derive(Serialize)]
struct LegacyChangeContentIdentity<'a> {
    schema: &'a str,
    change_id: &'a str,
    repository_id: &'a str,
    base_git_oid: &'a str,
    policy_hash: &'a str,
    creator: &'a str,
    artifacts: BTreeMap<ChangeArtifactId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeBaseRevision {
    pub kind: String,
    pub value: String,
}

impl ChangeBaseRevision {
    pub fn new(kind: impl Into<String>, value: impl Into<String>) -> Result<Self, ChangeCobError> {
        let revision = Self {
            kind: kind.into(),
            value: value.into(),
        };
        revision.validate()?;
        Ok(revision)
    }

    pub fn encode(&self) -> Result<String, ChangeCobError> {
        self.validate()?;
        Ok(format!(
            "{}{BASE_REVISION_SEPARATOR}{}",
            self.kind, self.value
        ))
    }

    fn validate(&self) -> Result<(), ChangeCobError> {
        validate_text_limit(
            "base_revision.kind",
            &self.kind,
            MAX_BASE_REVISION_KIND_BYTES,
        )?;
        validate_stable_id("base_revision.kind", &self.kind)?;
        validate_text_limit(
            "base_revision.value",
            &self.value,
            MAX_BASE_REVISION_VALUE_BYTES,
        )?;
        if self.value.contains('\0') {
            return Err(ChangeCobError::InvalidField {
                field: "base_revision.value".to_string(),
                reason: "NUL bytes are not valid in a revision identity".to_string(),
            });
        }
        Ok(())
    }
}

pub fn decode_base_revision(value: &str) -> Result<ChangeBaseRevision, ChangeCobError> {
    let Some((kind, revision_value)) = value.split_once(BASE_REVISION_SEPARATOR) else {
        return Err(ChangeCobError::InvalidField {
            field: "base_revision".to_string(),
            reason: format!("must contain a kind and value separated by {BASE_REVISION_SEPARATOR}"),
        });
    };
    ChangeBaseRevision::new(kind, revision_value)
}

#[derive(Serialize)]
struct ChangeContentIdentity<'a> {
    schema: &'a str,
    change_id: &'a str,
    workspace_id: &'a str,
    base_revision: ChangeBaseRevision,
    policy_hash: &'a str,
    creator: &'a str,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    delegates: &'a BTreeSet<String>,
    artifacts: BTreeMap<ChangeArtifactId, String>,
}

#[derive(Serialize)]
struct ChangeStateIdentity<'a> {
    schema: &'a str,
    change_id: &'a str,
    workspace_id: &'a str,
    base_revision: ChangeBaseRevision,
    policy_hash: &'a str,
    creator: &'a str,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    delegates: &'a BTreeSet<String>,
    artifacts: &'a BTreeMap<ChangeArtifactId, ChangeRevisionSet<Option<String>>>,
    evidence: &'a BTreeMap<String, ChangeCobEvidence>,
    lifecycle: &'a ChangeRevisionSet<ChangeLifecycleRecord>,
    operations: &'a BTreeSet<String>,
}

pub fn change_cob_action_json(action: &ChangeCobAction) -> Result<Value, ChangeCobError> {
    serde_json::to_value(action).map_err(|error| ChangeCobError::Serialization {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT_OPERATION: &str = "1111111111111111111111111111111111111111";
    const DESIGN_OPERATION: &str = "2222222222222222222222222222222222222222";
    const TASKS_OPERATION: &str = "3333333333333333333333333333333333333333";
    const CONFLICT_OPERATION: &str = "4444444444444444444444444444444444444444";
    const RESOLUTION_OPERATION: &str = "5555555555555555555555555555555555555555";
    const EVIDENCE_OPERATION: &str = "6666666666666666666666666666666666666666";
    const LIFECYCLE_OPERATION: &str = "7777777777777777777777777777777777777777";
    const POST_APPROVAL_OPERATION: &str = "8888888888888888888888888888888888888888";
    const CREATOR: &str = "creator-key";
    const CONTRIBUTOR: &str = "contributor-key";
    const DELEGATE: &str = "delegate-key";
    const REPOSITORY_ID: &str = "rad:z3example";
    const BASE_GIT_OID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const POLICY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const EVIDENCE_HASH: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const INITIAL_PROPOSAL: &str = "# Proposal\n\nInitial proposal.\n";
    const INITIAL_DESIGN: &str = "# Design\n\nInitial design.\n";
    const INITIAL_TASKS: &str = "# Tasks\n\n- [ ] [serial] Implement.\n";
    const INITIAL_METADATA: &str = "{\"depends_on\":[\"other-change\"]}\n";
    const UPDATED_DESIGN: &str = "# Design\n\nUpdated design.\n";
    const UPDATED_TASKS: &str = "# Tasks\n\n- [x] [serial] Implemented.\n";
    const CONFLICTING_DESIGN: &str = "# Design\n\nConflicting design.\n";
    const RESOLVED_DESIGN: &str = "# Design\n\nResolved design.\n";
    const EXPECTED_INITIAL_FILE_COUNT: usize = 5;
    const EXPECTED_CONFLICT_HEAD_COUNT: usize = 2;
    const EXPECTED_CURRENT_EVIDENCE_COUNT: usize = 1;

    fn hash_set(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn init_action() -> ChangeCobAction {
        ChangeCobAction::Init {
            state: ChangeCobInit {
                schema: RADICLE_CHANGE_COB_SCHEMA_V1.to_string(),
                change_id: "radicle-change-cob".to_string(),
                repository_id: REPOSITORY_ID.to_string(),
                base_git_oid: BASE_GIT_OID.to_string(),
                policy_hash: POLICY_HASH.to_string(),
                delegates: BTreeSet::new(),
                artifacts: BTreeMap::from([
                    (ChangeArtifactId::proposal(), INITIAL_PROPOSAL.to_string()),
                    (ChangeArtifactId::design(), INITIAL_DESIGN.to_string()),
                    (ChangeArtifactId::tasks(), INITIAL_TASKS.to_string()),
                    (ChangeArtifactId::metadata(), INITIAL_METADATA.to_string()),
                    (
                        ChangeArtifactId::delta_spec("radicle-change-cob").unwrap(),
                        "# Delta spec\n".to_string(),
                    ),
                ]),
            },
        }
    }

    fn operation(
        id: &str,
        actor: &str,
        is_delegate: bool,
        actions: Vec<ChangeCobAction>,
    ) -> ChangeCobOperation {
        ChangeCobOperation {
            id: id.to_string(),
            actor: actor.to_string(),
            is_delegate,
            actions,
        }
    }

    fn state() -> ChangeCobState {
        initialize_change_cob(operation(
            ROOT_OPERATION,
            CREATOR,
            false,
            vec![init_action()],
        ))
        .unwrap()
    }

    fn neutral_state(delegates: BTreeSet<String>) -> ChangeCobState {
        let mut action = init_action();
        let ChangeCobAction::Init { state: init } = &mut action else {
            unreachable!("test helper constructs an init action");
        };
        init.schema = CHANGE_COLLABORATION_SCHEMA_V1.to_string();
        init.repository_id = "workspace:radicle-cob-only".to_string();
        init.base_git_oid = ChangeBaseRevision::new("git_oid", BASE_GIT_OID)
            .unwrap()
            .encode()
            .unwrap();
        init.delegates = delegates;
        initialize_change_cob(operation(ROOT_OPERATION, CREATOR, false, vec![action])).unwrap()
    }

    fn set_artifact(artifact: ChangeArtifactId, content: &str, basis: &[&str]) -> ChangeCobAction {
        ChangeCobAction::SetArtifact {
            artifact,
            content: Some(content.to_string()),
            basis: hash_set(basis),
        }
    }

    fn evidence(subject_root: &str) -> ChangeCobEvidence {
        ChangeCobEvidence {
            blake3: EVIDENCE_HASH.to_string(),
            schema: "cairn.gate.receipt.v1".to_string(),
            role: "gate_receipt".to_string(),
            subject_root: subject_root.to_string(),
            policy_hash: POLICY_HASH.to_string(),
            git_oid: Some(BASE_GIT_OID.to_string()),
            uri: None,
            non_claims: BTreeSet::from([
                "does not prove source correctness or release readiness".to_string()
            ]),
        }
    }

    // r[verify cairn.radicle_change_cob.lossless_change_state]
    // r[verify cairn.radicle_change_cob.lossless_change_state.preserve]
    // r[verify cairn.radicle_change_cob.materialized_view]
    // r[verify cairn.radicle_change_cob.materialized_view.paths]
    // r[verify cairn.radicle_change_cob.radicle_adapter.dependencies]
    #[test]
    fn root_preserves_and_materializes_exact_artifacts() {
        let state = state();
        let files = materialize_change_cob(&state).unwrap();
        assert_eq!(files.len(), EXPECTED_INITIAL_FILE_COUNT);
        assert!(files
            .iter()
            .any(|file| { file.path == "proposal.md" && file.content == INITIAL_PROPOSAL }));
        assert!(files.iter().any(|file| {
            file.path == "specs/radicle-change-cob/spec.md" && file.content == "# Delta spec\n"
        }));
        assert!(files
            .iter()
            .any(|file| { file.path == "metadata.json" && file.content == INITIAL_METADATA }));
        assert!(!change_cob_content_root(&state).unwrap().is_empty());
        assert!(!change_cob_state_hash(&state).unwrap().is_empty());
    }

    // r[verify cairn.radicle_change_cob.deterministic_reducer]
    // r[verify cairn.radicle_change_cob.deterministic_reducer.disjoint]
    #[test]
    fn disjoint_edits_converge_in_either_order() {
        let design = operation(
            DESIGN_OPERATION,
            CREATOR,
            false,
            vec![set_artifact(
                ChangeArtifactId::design(),
                UPDATED_DESIGN,
                &[ROOT_OPERATION],
            )],
        );
        let tasks = operation(
            TASKS_OPERATION,
            CREATOR,
            false,
            vec![set_artifact(
                ChangeArtifactId::tasks(),
                UPDATED_TASKS,
                &[ROOT_OPERATION],
            )],
        );
        let mut left = state();
        apply_change_cob_operation(&mut left, design.clone()).unwrap();
        apply_change_cob_operation(&mut left, tasks.clone()).unwrap();
        let mut right = state();
        apply_change_cob_operation(&mut right, tasks).unwrap();
        apply_change_cob_operation(&mut right, design).unwrap();
        assert_eq!(left, right);
        assert_eq!(
            change_cob_state_hash(&left).unwrap(),
            change_cob_state_hash(&right).unwrap()
        );
    }

    // r[verify cairn.radicle_change_cob.deterministic_reducer.conflict]
    // r[verify cairn.radicle_change_cob.materialized_view.conflict]
    #[test]
    fn same_artifact_concurrency_is_visible_and_order_independent() {
        let update = operation(
            DESIGN_OPERATION,
            CREATOR,
            false,
            vec![set_artifact(
                ChangeArtifactId::design(),
                UPDATED_DESIGN,
                &[ROOT_OPERATION],
            )],
        );
        let conflict = operation(
            CONFLICT_OPERATION,
            CREATOR,
            false,
            vec![set_artifact(
                ChangeArtifactId::design(),
                CONFLICTING_DESIGN,
                &[ROOT_OPERATION],
            )],
        );
        let mut left = state();
        apply_change_cob_operation(&mut left, update.clone()).unwrap();
        apply_change_cob_operation(&mut left, conflict.clone()).unwrap();
        let mut right = state();
        apply_change_cob_operation(&mut right, conflict).unwrap();
        apply_change_cob_operation(&mut right, update).unwrap();
        assert_eq!(left, right);
        let heads = left
            .artifacts
            .get(&ChangeArtifactId::design())
            .unwrap()
            .head_ids();
        assert_eq!(heads.len(), EXPECTED_CONFLICT_HEAD_COUNT);
        assert!(heads.contains(DESIGN_OPERATION));
        assert!(heads.contains(CONFLICT_OPERATION));
        assert!(!heads.contains(ROOT_OPERATION));
        assert!(matches!(
            materialize_change_cob(&left),
            Err(ChangeCobError::ArtifactConflict { .. })
        ));
    }

    // r[verify cairn.radicle_change_cob.deterministic_reducer.resolve]
    #[test]
    fn resolution_supersedes_every_conflict_head() {
        let mut state = state();
        apply_change_cob_operation(
            &mut state,
            operation(
                DESIGN_OPERATION,
                CREATOR,
                false,
                vec![set_artifact(
                    ChangeArtifactId::design(),
                    UPDATED_DESIGN,
                    &[ROOT_OPERATION],
                )],
            ),
        )
        .unwrap();
        apply_change_cob_operation(
            &mut state,
            operation(
                CONFLICT_OPERATION,
                CREATOR,
                false,
                vec![set_artifact(
                    ChangeArtifactId::design(),
                    CONFLICTING_DESIGN,
                    &[ROOT_OPERATION],
                )],
            ),
        )
        .unwrap();
        apply_change_cob_operation(
            &mut state,
            operation(
                RESOLUTION_OPERATION,
                CREATOR,
                false,
                vec![set_artifact(
                    ChangeArtifactId::design(),
                    RESOLVED_DESIGN,
                    &[DESIGN_OPERATION, CONFLICT_OPERATION],
                )],
            ),
        )
        .unwrap();
        let history = state.artifacts.get(&ChangeArtifactId::design()).unwrap();
        assert_eq!(history.head_ids(), hash_set(&[RESOLUTION_OPERATION]));
        assert!(history.revisions.contains_key(ROOT_OPERATION));
        assert!(history.revisions.contains_key(DESIGN_OPERATION));
        assert!(history.revisions.contains_key(CONFLICT_OPERATION));
        let files = materialize_change_cob(&state).unwrap();
        assert!(files
            .iter()
            .any(|file| { file.path == "design.md" && file.content == RESOLVED_DESIGN }));
    }

    // r[verify cairn.radicle_change_cob.deterministic_reducer.invalid]
    #[test]
    fn unauthorized_or_invalid_operations_are_atomic() {
        let mut state = state();
        let before = state.clone();
        let unauthorized = operation(
            DESIGN_OPERATION,
            CONTRIBUTOR,
            false,
            vec![set_artifact(
                ChangeArtifactId::design(),
                UPDATED_DESIGN,
                &[ROOT_OPERATION],
            )],
        );
        assert!(matches!(
            apply_change_cob_operation(&mut state, unauthorized),
            Err(ChangeCobError::Unauthorized { .. })
        ));
        assert_eq!(state, before);

        let invalid_basis = operation(
            DESIGN_OPERATION,
            CREATOR,
            false,
            vec![set_artifact(
                ChangeArtifactId::design(),
                UPDATED_DESIGN,
                &[CONFLICT_OPERATION],
            )],
        );
        assert!(matches!(
            apply_change_cob_operation(&mut state, invalid_basis),
            Err(ChangeCobError::UnknownBasis { .. })
        ));
        assert_eq!(state, before);
    }

    // r[verify cairn.radicle_change_cob.evidence_boundary]
    // r[verify cairn.radicle_change_cob.evidence_boundary.stale]
    #[test]
    fn evidence_and_lifecycle_become_stale_after_content_change() {
        let mut state = state();
        let subject_root = change_cob_content_root(&state).unwrap();
        apply_change_cob_operation(
            &mut state,
            operation(
                EVIDENCE_OPERATION,
                CONTRIBUTOR,
                false,
                vec![ChangeCobAction::AttachEvidence {
                    evidence: evidence(&subject_root),
                }],
            ),
        )
        .unwrap();
        apply_change_cob_operation(
            &mut state,
            operation(
                LIFECYCLE_OPERATION,
                DELEGATE,
                true,
                vec![ChangeCobAction::SetLifecycle {
                    status: ChangeLifecycleStatus::Approved,
                    subject_root: subject_root.clone(),
                    evidence: hash_set(&[EVIDENCE_HASH]),
                    basis: hash_set(&[ROOT_OPERATION]),
                }],
            ),
        )
        .unwrap();
        assert_eq!(
            current_change_cob_evidence(&state).unwrap().len(),
            EXPECTED_CURRENT_EVIDENCE_COUNT
        );
        assert!(matches!(
            change_cob_lifecycle_view(&state).unwrap(),
            ChangeLifecycleView::Current(ChangeLifecycleRecord {
                status: ChangeLifecycleStatus::Approved,
                ..
            })
        ));

        apply_change_cob_operation(
            &mut state,
            operation(
                POST_APPROVAL_OPERATION,
                CREATOR,
                false,
                vec![set_artifact(
                    ChangeArtifactId::tasks(),
                    UPDATED_TASKS,
                    &[ROOT_OPERATION],
                )],
            ),
        )
        .unwrap();
        assert!(current_change_cob_evidence(&state).unwrap().is_empty());
        assert!(state.evidence.contains_key(EVIDENCE_HASH));
        assert!(matches!(
            change_cob_lifecycle_view(&state).unwrap(),
            ChangeLifecycleView::Stale { .. }
        ));
    }

    // r[verify cairn.radicle_change_cob.evidence_boundary.authority]
    #[test]
    fn creator_cannot_approve_and_delegate_can_edit() {
        let mut state = state();
        let subject_root = change_cob_content_root(&state).unwrap();
        let before = state.clone();
        let creator_approval = operation(
            LIFECYCLE_OPERATION,
            CREATOR,
            false,
            vec![ChangeCobAction::SetLifecycle {
                status: ChangeLifecycleStatus::Approved,
                subject_root,
                evidence: BTreeSet::new(),
                basis: hash_set(&[ROOT_OPERATION]),
            }],
        );
        assert!(matches!(
            apply_change_cob_operation(&mut state, creator_approval),
            Err(ChangeCobError::Unauthorized { .. })
        ));
        assert_eq!(state, before);

        apply_change_cob_operation(
            &mut state,
            operation(
                DESIGN_OPERATION,
                DELEGATE,
                true,
                vec![set_artifact(
                    ChangeArtifactId::design(),
                    UPDATED_DESIGN,
                    &[ROOT_OPERATION],
                )],
            ),
        )
        .unwrap();
    }

    // r[verify cairn.radicle_change_cob.lossless_change_state.invalid]
    #[test]
    fn malformed_identifiers_hashes_duplicates_and_limits_fail() {
        assert!(ChangeArtifactId::delta_spec("../escape").is_err());
        let mut invalid_init = init_action();
        let ChangeCobAction::Init { state: init } = &mut invalid_init else {
            panic!("fixture must be init");
        };
        init.policy_hash = "not-a-hash".to_string();
        assert!(matches!(
            initialize_change_cob(operation(
                ROOT_OPERATION,
                CREATOR,
                false,
                vec![invalid_init]
            )),
            Err(ChangeCobError::InvalidField { .. })
        ));

        let mut state = state();
        let duplicate_actions = operation(
            DESIGN_OPERATION,
            CREATOR,
            false,
            vec![
                set_artifact(
                    ChangeArtifactId::design(),
                    UPDATED_DESIGN,
                    &[ROOT_OPERATION],
                ),
                set_artifact(
                    ChangeArtifactId::design(),
                    CONFLICTING_DESIGN,
                    &[ROOT_OPERATION],
                ),
            ],
        );
        assert!(matches!(
            apply_change_cob_operation(&mut state, duplicate_actions),
            Err(ChangeCobError::DuplicateArtifactAction { .. })
        ));

        let oversized = "x".repeat(MAX_ARTIFACT_BYTES.saturating_add(1));
        let oversized_action = operation(
            DESIGN_OPERATION,
            CREATOR,
            false,
            vec![set_artifact(
                ChangeArtifactId::design(),
                &oversized,
                &[ROOT_OPERATION],
            )],
        );
        assert!(matches!(
            apply_change_cob_operation(&mut state, oversized_action),
            Err(ChangeCobError::ByteLimitExceeded { .. })
        ));
    }

    // r[verify cairn.radicle_change_cob.evidence_boundary.non_claims]
    #[test]
    fn evidence_requires_explicit_non_claims() {
        let mut state = state();
        let subject_root = change_cob_content_root(&state).unwrap();
        let mut invalid_evidence = evidence(&subject_root);
        invalid_evidence.non_claims.clear();
        let before = state.clone();
        let result = apply_change_cob_operation(
            &mut state,
            operation(
                EVIDENCE_OPERATION,
                CONTRIBUTOR,
                false,
                vec![ChangeCobAction::AttachEvidence {
                    evidence: invalid_evidence,
                }],
            ),
        );
        assert!(matches!(result, Err(ChangeCobError::InvalidField { .. })));
        assert_eq!(state, before);
    }

    #[test]
    fn action_json_round_trips_and_replay_is_rejected() {
        let action = set_artifact(
            ChangeArtifactId::design(),
            UPDATED_DESIGN,
            &[ROOT_OPERATION],
        );
        let json = change_cob_action_json(&action).unwrap();
        let decoded: ChangeCobAction = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, action);

        let mut state = state();
        let update = operation(DESIGN_OPERATION, CREATOR, false, vec![action]);
        apply_change_cob_operation(&mut state, update.clone()).unwrap();
        assert!(matches!(
            apply_change_cob_operation(&mut state, update),
            Err(ChangeCobError::DuplicateOperation { .. })
        ));
    }

    // r[verify cairn.radicle_cob_only.authority.allowed]
    #[test]
    fn neutral_schema_preserves_embedded_delegate_authority() {
        let mut state = neutral_state(BTreeSet::from([DELEGATE.to_string()]));
        assert!(state.delegates.contains(DELEGATE));
        apply_change_cob_operation(
            &mut state,
            operation(
                DESIGN_OPERATION,
                DELEGATE,
                true,
                vec![set_artifact(
                    ChangeArtifactId::design(),
                    UPDATED_DESIGN,
                    &[ROOT_OPERATION],
                )],
            ),
        )
        .unwrap();
    }

    // r[verify cairn.radicle_cob_only.compatibility.legacy]
    // r[verify cairn.radicle_cob_only.authority.bounded]
    // r[verify cairn.radicle_cob_only.authority.legacy_hash]
    #[test]
    fn legacy_empty_authority_stays_omitted_and_neutral_authority_is_hashed() {
        let legacy = state();
        let serialized = serde_json::to_value(&legacy).unwrap();
        assert!(serialized.get("delegates").is_none());

        let without_delegate = neutral_state(BTreeSet::new());
        let base_revision = decode_base_revision(&without_delegate.base_git_oid).unwrap();
        let artifacts = resolved_artifacts(&without_delegate)
            .unwrap()
            .into_iter()
            .filter_map(|(artifact, content)| content.map(|content| (artifact, content)))
            .collect::<BTreeMap<_, _>>();
        let expected_content_root = canonical_json_hash(&serde_json::json!({
            "schema": without_delegate.schema,
            "change_id": without_delegate.change_id,
            "workspace_id": without_delegate.repository_id,
            "base_revision": base_revision,
            "policy_hash": without_delegate.policy_hash,
            "creator": without_delegate.creator,
            "artifacts": artifacts,
        }));
        assert_eq!(
            change_cob_content_root(&without_delegate).unwrap(),
            expected_content_root
        );
        let expected_state_hash = canonical_json_hash(&serde_json::json!({
            "schema": without_delegate.schema,
            "change_id": without_delegate.change_id,
            "workspace_id": without_delegate.repository_id,
            "base_revision": decode_base_revision(&without_delegate.base_git_oid).unwrap(),
            "policy_hash": without_delegate.policy_hash,
            "creator": without_delegate.creator,
            "artifacts": without_delegate.artifacts,
            "evidence": without_delegate.evidence,
            "lifecycle": without_delegate.lifecycle,
            "operations": without_delegate.operations,
        }));
        assert_eq!(
            change_cob_state_hash(&without_delegate).unwrap(),
            expected_state_hash
        );

        let with_delegate = neutral_state(BTreeSet::from([DELEGATE.to_string()]));
        assert_ne!(
            change_cob_content_root(&without_delegate).unwrap(),
            change_cob_content_root(&with_delegate).unwrap()
        );
        assert_ne!(
            change_cob_state_hash(&without_delegate).unwrap(),
            change_cob_state_hash(&with_delegate).unwrap()
        );

        let mut invalid_legacy = legacy;
        invalid_legacy.delegates.insert(DELEGATE.to_string());
        assert!(matches!(
            validate_change_cob_state(&invalid_legacy),
            Err(ChangeCobError::InvalidField { ref field, .. }) if field == "delegates"
        ));

        let mut oversized = without_delegate;
        oversized.delegates = (0..=MAX_DELEGATES)
            .map(|index| format!("delegate-{index}"))
            .collect();
        assert!(matches!(
            validate_change_cob_state(&oversized),
            Err(ChangeCobError::CountLimitExceeded { ref field, .. }) if field == "delegates"
        ));
    }
}
