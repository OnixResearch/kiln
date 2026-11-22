use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::evidence::canonical_json_hash;
use super::radicle_change::decode_base_revision;

pub const DOCUMENT_COLLABORATION_SCHEMA_V1: &str = "cairn.document-collaboration.v1";
pub const BASE_REVISION_KIND_GIT_OID: &str = "git_oid";
pub const RFD_SCHEMA_V1: &str = "xyz.onixresearch.rfd.v1";
pub const RFD_COB_TYPE_NAME: &str = "xyz.onixresearch.rfd";
pub const ADR_COB_TYPE_NAME: &str = "xyz.onixresearch.adr";
pub const ADR_SCHEMA_V1: &str = "xyz.onixresearch.adr.v1";
pub const DOCUMENT_PORTABLE_SCHEMA_V1: &str = "cairn.document-portable.v1";
pub const DOCUMENT_ROLE_AUTHOR: &str = "author";
pub const DOCUMENT_ROLE_PARTICIPANT: &str = "participant";
pub const DOCUMENT_ROLE_REVIEWER: &str = "reviewer";
pub const DOCUMENT_ROLE_PUBLISHER: &str = "publisher";
pub const DOCUMENT_ROLE_APPROVER: &str = "approver";
const BLAKE3_HEX_LENGTH: usize = 64;

// r[impl cairn.document_collaboration.semantic_core]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Rfd,
    Adr,
}

impl DocumentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rfd => "rfd",
            Self::Adr => "adr",
        }
    }
    pub fn cob_type_name(&self) -> &'static str {
        match self {
            Self::Rfd => RFD_COB_TYPE_NAME,
            Self::Adr => ADR_COB_TYPE_NAME,
        }
    }
    pub fn schema_v1(&self) -> &'static str {
        match self {
            Self::Rfd => RFD_SCHEMA_V1,
            Self::Adr => ADR_SCHEMA_V1,
        }
    }
}

impl TryFrom<&str> for DocumentKind {
    type Error = DocumentCobError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "rfd" | RFD_COB_TYPE_NAME => Ok(Self::Rfd),
            "adr" | ADR_COB_TYPE_NAME => Ok(Self::Adr),
            other => Err(DocumentCobError::UnsupportedKind {
                kind: other.to_string(),
            }),
        }
    }
}

// r[impl cairn.document_collaboration.semantic_core]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentRole {
    Author,
    Participant,
    Reviewer,
    Approver,
    Publisher,
}

impl DocumentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Author => DOCUMENT_ROLE_AUTHOR,
            Self::Participant => DOCUMENT_ROLE_PARTICIPANT,
            Self::Reviewer => DOCUMENT_ROLE_REVIEWER,
            Self::Approver => DOCUMENT_ROLE_APPROVER,
            Self::Publisher => DOCUMENT_ROLE_PUBLISHER,
        }
    }
}

// r[impl cairn.document_collaboration.semantic_core]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionRecord {
    pub id: String,
    pub actor: String,
    pub text: String,
    pub reply_to: Option<String>,
}

// r[impl cairn.document_collaboration.semantic_core]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentLink {
    pub id: String,
    pub target_kind: String,
    pub target_id: String,
    pub relation: String,
    pub expected_content_root: Option<String>,
}

// r[impl cairn.rfd_cob.identity]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationNumber {
    pub number: u64,
    pub registry_snapshot_identity: String,
    pub content_root: String,
    pub policy_hash: String,
    pub publisher: String,
}

// r[impl cairn.document_collaboration.semantic_core]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentState {
    pub schema: String,
    pub kind: String,
    pub document_id: String,
    pub workspace_id: String,
    pub base_revision: String,
    pub policy_hash: String,
    pub authors: std::collections::BTreeSet<String>,
    pub participants: std::collections::BTreeSet<String>,
    pub reviewers: std::collections::BTreeSet<String>,
    pub approvers: std::collections::BTreeSet<String>,
    pub publishers: std::collections::BTreeSet<String>,
    pub artifacts: std::collections::BTreeMap<String, DocumentRevisionSet>,
    pub links: std::collections::BTreeMap<String, DocumentLink>,
    pub discussion: std::collections::BTreeMap<String, DiscussionRecord>,
    pub lifecycle: DocumentLifecycleRecord,
    pub evidence: std::collections::BTreeMap<String, String>,
    pub publication: Option<PublicationNumber>,
    pub operations: std::collections::BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DocumentRevisionSet {
    pub revisions: std::collections::BTreeMap<String, String>,
    pub supersedes: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
}

impl DocumentRevisionSet {
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentLifecycleStatus {
    Draft,
    Proposed,
    Review,
    Accepted,
    Deprecated,
    Published,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentLifecycleRecord {
    pub status: DocumentLifecycleStatus,
    pub subject_root: String,
    #[serde(default)]
    pub basis: std::collections::BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentAction {
    Init {
        state: DocumentInit,
    },
    SetArtifact {
        artifact: String,
        content: Option<String>,
        basis: std::collections::BTreeSet<String>,
    },
    AddDiscussion {
        record: DiscussionRecord,
        basis: std::collections::BTreeSet<String>,
    },
    AddLink {
        link: DocumentLink,
        basis: std::collections::BTreeSet<String>,
    },
    SetLifecycle {
        status: DocumentLifecycleStatus,
        subject_root: String,
        basis: std::collections::BTreeSet<String>,
    },
    SetPublication {
        publication: PublicationNumber,
        basis: std::collections::BTreeSet<String>,
    },
}

// r[impl cairn.rfd_cob.profile.root]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentInit {
    pub schema: String,
    pub kind: String,
    pub document_id: String,
    pub workspace_id: String,
    pub base_revision: String,
    pub policy_hash: String,
    pub authors: std::collections::BTreeSet<String>,
    pub participants: std::collections::BTreeSet<String>,
    pub reviewers: std::collections::BTreeSet<String>,
    pub approvers: std::collections::BTreeSet<String>,
    pub publishers: std::collections::BTreeSet<String>,
    pub artifacts: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentOperation {
    pub id: String,
    pub actor: String,
    pub role: Option<DocumentRole>,
    pub actions: Vec<DocumentAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentCobError {
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
    MixedKind {
        expected: String,
        actual: String,
    },
    ConflictedField {
        field: String,
        heads: Vec<String>,
    },
    AtomicConflict,
    ImmutableDecision,
    InvalidRootShape,
    MultiStateAction,
    MissingRelatedBase {
        base: String,
    },
    PortableTamper {
        field: String,
    },
    Factory(String),
}

impl std::fmt::Display for DocumentCobError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidField { field, reason } => {
                write!(formatter, "invalid document field {field}: {reason}")
            }
            Self::UnsupportedKind { kind } => write!(formatter, "unsupported document kind {kind}"),
            Self::SchemaMismatch { expected, actual } => {
                write!(
                    formatter,
                    "document schema mismatch; expected {expected}, got {actual}"
                )
            }
            Self::KindMismatch { expected, actual } => {
                write!(
                    formatter,
                    "document kind mismatch; expected {expected}, got {actual}"
                )
            }
            Self::EmptyOperation => write!(formatter, "document operation has no actions"),
            Self::DuplicateOperation { id } => {
                write!(formatter, "duplicate document operation {id}")
            }
            Self::UnknownBase { field, base } => {
                write!(formatter, "unknown base revision {base} for {field}")
            }
            Self::MissingBasis { operation, basis } => {
                write!(
                    formatter,
                    "operation {operation} is missing basis revision {basis}"
                )
            }
            Self::UnauthorizedRole { required, actor } => {
                write!(formatter, "actor {actor} is not a required {required}")
            }
            Self::MixedKind { expected, actual } => {
                write!(
                    formatter,
                    "operation mixes document kind {actual} with {expected}"
                )
            }
            Self::ConflictedField { field, heads } => {
                write!(
                    formatter,
                    "field {field} has multiple current heads: {heads:?}"
                )
            }
            Self::AtomicConflict => write!(
                formatter,
                "document operation is conflict-preserving and atomic"
            ),
            Self::ImmutableDecision => write!(formatter, "accepted decision content is immutable"),
            Self::InvalidRootShape => write!(
                formatter,
                "document root operation must be exactly one Init"
            ),
            Self::MultiStateAction => write!(
                formatter,
                "Init cannot appear in a non-root document operation"
            ),
            Self::MissingRelatedBase { base } => {
                write!(
                    formatter,
                    "root base object {base} is missing from related objects"
                )
            }
            Self::PortableTamper { field } => {
                write!(formatter, "portable envelope field {field} is tampered")
            }
            Self::Factory(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for DocumentCobError {}

include!("document_collaboration_impl.rs");

#[cfg(test)]
#[path = "document_collaboration_tests.rs"]
mod tests;
