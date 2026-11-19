use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use super::evidence::canonical_json_hash;
use super::radicle_change::decode_base_revision;

pub const WORKFLOW_COLLABORATION_SCHEMA_V1: &str = "cairn.workflow-collaboration.v1";
pub const VERIFICATION_OBLIGATION_SCHEMA_V1: &str = "xyz.onixresearch.verification-obligation.v1";
pub const VERIFICATION_OBLIGATION_COB_TYPE_NAME: &str = "xyz.onixresearch.verification-obligation";
pub const WORKFLOW_PORTABLE_SCHEMA_V1: &str = "cairn.workflow-portable.v1";
pub const WORKFLOW_ROLE_SUBMITTER: &str = "submitter";
pub const WORKFLOW_ROLE_CONTRIBUTOR: &str = "contributor";
pub const WORKFLOW_ROLE_VERIFIER: &str = "verifier";
pub const WORKFLOW_ROLE_INVALIDATOR: &str = "invalidator";
const BLAKE3_HEX_LENGTH: usize = 64;
pub const BASE_REVISION_KIND_GIT_OID: &str = "git_oid";

// r[impl cairn.workflow_collaboration.semantic_core]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowKind {
    VerificationObligation,
}

impl WorkflowKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VerificationObligation => "verification-obligation",
        }
    }
    pub fn cob_type_name(&self) -> &'static str {
        match self {
            Self::VerificationObligation => VERIFICATION_OBLIGATION_COB_TYPE_NAME,
        }
    }
    pub fn schema_v1(&self) -> &'static str {
        match self {
            Self::VerificationObligation => VERIFICATION_OBLIGATION_SCHEMA_V1,
        }
    }
}

impl TryFrom<&str> for WorkflowKind {
    type Error = WorkflowCobError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "verification-obligation" | VERIFICATION_OBLIGATION_COB_TYPE_NAME => {
                Ok(Self::VerificationObligation)
            }
            other => Err(WorkflowCobError::UnsupportedKind {
                kind: other.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRole {
    Submitter,
    Contributor,
    Verifier,
    Invalidator,
}

impl WorkflowRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Submitter => WORKFLOW_ROLE_SUBMITTER,
            Self::Contributor => WORKFLOW_ROLE_CONTRIBUTOR,
            Self::Verifier => WORKFLOW_ROLE_VERIFIER,
            Self::Invalidator => WORKFLOW_ROLE_INVALIDATOR,
        }
    }
}

// r[impl cairn.workflow_collaboration.evidence]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvidenceReference {
    pub id: String,
    pub evidence_kind: String,
    pub verification_role: String,
    pub claim_scope: String,
    pub external_identity: String,
}

// r[impl cairn.verification_obligation_cob.findings]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowFinding {
    pub id: String,
    pub status: String,
    pub blocker_class: String,
    pub source_span: String,
    pub response: Option<String>,
    pub disposition: Option<String>,
}

// r[impl cairn.verification_obligation_cob.claim_revision]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRoot {
    pub claim_identity: String,
    pub subject_root: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowLifecycleRecord {
    pub status: String,
    pub subject_root: String,
}

// r[impl cairn.workflow_collaboration.semantic_core]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowState {
    pub schema: String,
    pub kind: String,
    pub obligation_id: String,
    pub workspace_id: String,
    pub base_revision: String,
    pub policy_hash: String,
    pub claim_root: ClaimRoot,
    pub submitters: std::collections::BTreeSet<String>,
    pub contributors: std::collections::BTreeSet<String>,
    pub verifiers: std::collections::BTreeSet<String>,
    pub invalidators: std::collections::BTreeSet<String>,
    pub fields: std::collections::BTreeMap<String, WorkflowRevisionSet>,
    pub findings: std::collections::BTreeMap<String, WorkflowFinding>,
    pub evidence: std::collections::BTreeMap<String, WorkflowEvidenceReference>,
    pub lifecycle: WorkflowLifecycleRecord,
    pub operations: std::collections::BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRevisionSet {
    pub revisions: std::collections::BTreeMap<String, String>,
    pub supersedes: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
}

impl WorkflowRevisionSet {
    pub fn head_ids(&self) -> std::collections::BTreeSet<String> {
        let superseded = self
            .supersedes
            .values()
            .flatten()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        self.revisions
            .keys()
            .filter(|id| !superseded.contains(*id))
            .cloned()
            .collect()
    }
    pub fn content(&self, revision_id: &str) -> Option<&str> {
        self.revisions.get(revision_id).map(String::as_str)
    }
}

// r[impl cairn.workflow_collaboration.semantic_core]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowInit {
    pub schema: String,
    pub kind: String,
    pub obligation_id: String,
    pub workspace_id: String,
    pub base_revision: String,
    pub policy_hash: String,
    pub claim_root: ClaimRoot,
    pub submitters: std::collections::BTreeSet<String>,
    pub contributors: std::collections::BTreeSet<String>,
    pub verifiers: std::collections::BTreeSet<String>,
    pub invalidators: std::collections::BTreeSet<String>,
    pub fields: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowAction {
    Init {
        state: WorkflowInit,
    },
    SetField {
        field: String,
        content: Option<String>,
        basis: std::collections::BTreeSet<String>,
    },
    AttachEvidence {
        evidence: WorkflowEvidenceReference,
    },
    AddFinding {
        finding: WorkflowFinding,
    },
    RespondFinding {
        finding_id: String,
        response: String,
    },
    SetDisposition {
        finding_id: String,
        disposition: String,
        basis: Vec<String>,
    },
    Discharge {
        decision: String,
        subject_root: String,
    },
    Reject {
        decision: String,
        subject_root: String,
    },
    Invalidate {
        decision: String,
        subject_root: String,
    },
    ResolveField {
        field: String,
        content: String,
        basis: std::collections::BTreeSet<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowOperation {
    pub id: String,
    pub actor: String,
    pub role: Option<WorkflowRole>,
    pub actions: Vec<WorkflowAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowCobError {
    InvalidField {
        field: String,
        reason: String,
    },
    UnsupportedKind {
        kind: String,
    },
    SchemaMismatch {
        expected: String,
        actual: String,
    },
    KindMismatch {
        expected: String,
        actual: String,
    },
    EmptyOperation,
    DuplicateOperation {
        id: String,
    },
    UnknownBase {
        field: String,
        base: String,
    },
    MissingBasis {
        operation: String,
        basis: String,
    },
    UnauthorizedRole {
        required: &'static str,
        actor: String,
    },
    ConflictedField {
        field: String,
        heads: Vec<String>,
    },
    ImmutableDischarged,
    UnresolvedBlocker {
        finding_id: String,
    },
    StaleClaimRoot {
        expected: u64,
        observed: u64,
    },
    EvidenceRolePromotion {
        role: String,
    },
    InvalidRootShape,
    MissingRelatedBase {
        base: String,
    },
    PortableTamper {
        field: String,
    },
    Factory(String),
}

impl std::fmt::Display for WorkflowCobError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidField { field, reason } => {
                write!(formatter, "invalid workflow field {field}: {reason}")
            }
            Self::UnsupportedKind { kind } => write!(formatter, "unsupported workflow kind {kind}"),
            Self::SchemaMismatch { expected, actual } => write!(
                formatter,
                "workflow schema mismatch; expected {expected}, got {actual}"
            ),
            Self::KindMismatch { expected, actual } => write!(
                formatter,
                "workflow kind mismatch; expected {expected}, got {actual}"
            ),
            Self::EmptyOperation => write!(formatter, "workflow operation has no actions"),
            Self::DuplicateOperation { id } => {
                write!(formatter, "duplicate workflow operation {id}")
            }
            Self::UnknownBase { field, base } => {
                write!(formatter, "unknown base revision {base} for {field}")
            }
            Self::MissingBasis { operation, basis } => write!(
                formatter,
                "operation {operation} is missing basis revision {basis}"
            ),
            Self::UnauthorizedRole { required, actor } => {
                write!(formatter, "actor {actor} is not a required {required}")
            }
            Self::ConflictedField { field, heads } => write!(
                formatter,
                "field {field} has multiple current heads: {heads:?}"
            ),
            Self::ImmutableDischarged => {
                write!(formatter, "a discharged obligation cannot change fields")
            }
            Self::UnresolvedBlocker { finding_id } => write!(
                formatter,
                "unresolved blocking finding {finding_id} prevents discharge"
            ),
            Self::StaleClaimRoot { expected, observed } => write!(
                formatter,
                "claim root revision stale; expected {expected}, observed {observed}"
            ),
            Self::EvidenceRolePromotion { role } => write!(
                formatter,
                "recorded-only evidence cannot become property evidence (role {role})"
            ),
            Self::InvalidRootShape => write!(
                formatter,
                "workflow root operation must be exactly one Init"
            ),
            Self::MissingRelatedBase { base } => write!(
                formatter,
                "root base object {base} is missing from related objects"
            ),
            Self::PortableTamper { field } => write!(
                formatter,
                "workflow portable envelope field {field} is tampered"
            ),
            Self::Factory(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for WorkflowCobError {}

include!("workflow_collaboration_impl.rs");

#[cfg(test)]
#[path = "workflow_collaboration_tests.rs"]
mod tests;
