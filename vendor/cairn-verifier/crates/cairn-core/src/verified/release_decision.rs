use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::evidence::canonical_json_hash;

pub const RELEASE_DECISION_SCHEMA_V1: &str = "cairn.release-decision.v1";
pub const RELEASE_DECISION_COB_TYPE_NAME: &str = "xyz.onixresearch.release-decision";
pub const RELEASE_PORTABLE_SCHEMA_V1: &str = "cairn.release-decision-portable.v1";
const BLAKE3_HEX_LENGTH: usize = 64;

// r[impl cairn.release_decision_cob.profile]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCandidate {
    pub source_revision: String,
    pub artifact_manifest: String,
    pub artifact_identity: String,
    pub target_label: String,
    pub release_label: String,
}

// r[impl cairn.release_decision_cob.evidence]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseEvidenceSummary {
    pub id: String,
    pub evidence_identity: String,
    pub schema: String,
    pub verification_role: String,
    pub claim_scope: String,
    pub validator_identity: String,
    pub verdict: String,
    pub policy_identity: String,
    pub receipt_identity: String,
}

// r[impl cairn.release_decision_cob.blockers]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseFinding {
    pub id: String,
    pub category: String,
    pub severity: String,
    pub blocker_class: String,
    pub description: String,
    pub source_span: String,
    pub dispositions: Vec<String>,
}

// r[impl cairn.release_decision_cob.decision]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseVote {
    pub id: String,
    pub actor: String,
    pub kind: String,
    pub review_basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseDecisionState {
    pub schema: String,
    pub kind: String,
    pub decision_id: String,
    pub workspace_id: String,
    pub candidate_root: String,
    pub policy_identity: String,
    pub candidate: ReleaseCandidate,
    pub proposers: BTreeSet<String>,
    pub reviewers: BTreeSet<String>,
    pub approvers: BTreeSet<String>,
    pub withdrawers: BTreeSet<String>,
    pub invalidators: BTreeSet<String>,
    pub evidence: BTreeMap<String, ReleaseEvidenceSummary>,
    pub findings: BTreeMap<String, ReleaseFinding>,
    pub votes: BTreeMap<String, ReleaseVote>,
    pub lifecycle: String,
    pub review_basis: String,
    pub operations: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseDecisionInit {
    pub schema: String,
    pub kind: String,
    pub decision_id: String,
    pub workspace_id: String,
    pub candidate_root: String,
    pub policy_identity: String,
    pub candidate: ReleaseCandidate,
    pub proposers: BTreeSet<String>,
    pub reviewers: BTreeSet<String>,
    pub approvers: BTreeSet<String>,
    pub withdrawers: BTreeSet<String>,
    pub invalidators: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReleaseDecisionAction {
    Init {
        state: ReleaseDecisionInit,
    },
    AttachEvidence {
        evidence: ReleaseEvidenceSummary,
    },
    AddFinding {
        finding: ReleaseFinding,
    },
    RespondFinding {
        finding_id: String,
        response: String,
    },
    DisposeFinding {
        finding_id: String,
        disposition: String,
    },
    Vote {
        vote: ReleaseVote,
    },
    Withdraw {
        reason: String,
    },
    Invalidate {
        reason: String,
        evidence_id: String,
    },
    Supersede {
        successor: String,
    },
    Resolve {
        finding_id: String,
        disposition: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseDecisionOperation {
    pub id: String,
    pub actor: String,
    pub role: Option<ReleaseRole>,
    pub actions: Vec<ReleaseDecisionAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseRole {
    Proposer,
    Reviewer,
    Approver,
    Withdrawer,
    Invalidator,
}

impl ReleaseRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Proposer => "proposer",
            Self::Reviewer => "reviewer",
            Self::Approver => "approver",
            Self::Withdrawer => "withdrawer",
            Self::Invalidator => "invalidator",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseDecisionError {
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
    UnauthorizedRole {
        required: &'static str,
        actor: String,
    },
    CandidateImmutable,
    UnresolvedBlocker {
        finding_id: String,
    },
    StaleReviewBasis {
        expected: String,
        observed: String,
    },
    EvidenceStale {
        evidence_id: String,
    },
    EvidenceRolePromotion {
        role: String,
    },
    ConflictedDecision,
    InvalidRootShape,
    CandidateSubstitution,
    MissingRelatedBase {
        base: String,
    },
    PortableTamper {
        field: String,
    },
    Factory(String),
}

impl std::fmt::Display for ReleaseDecisionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidField { field, reason } => write!(
                formatter,
                "invalid release-decision field {field}: {reason}"
            ),
            Self::UnsupportedKind { kind } => {
                write!(formatter, "unsupported release-decision kind {kind}")
            }
            Self::SchemaMismatch { expected, actual } => write!(
                formatter,
                "schema mismatch; expected {expected}, got {actual}"
            ),
            Self::KindMismatch { expected, actual } => write!(
                formatter,
                "kind mismatch; expected {expected}, got {actual}"
            ),
            Self::EmptyOperation => write!(formatter, "operation has no actions"),
            Self::DuplicateOperation { id } => write!(formatter, "duplicate operation {id}"),
            Self::UnauthorizedRole { required, actor } => {
                write!(formatter, "actor {actor} is not a required {required}")
            }
            Self::CandidateImmutable => write!(formatter, "release candidate is immutable"),
            Self::UnresolvedBlocker { finding_id } => write!(
                formatter,
                "unresolved blocker {finding_id} prevents approval"
            ),
            Self::StaleReviewBasis { expected, observed } => write!(
                formatter,
                "stale review basis; expected {expected}, observed {observed}"
            ),
            Self::EvidenceStale { evidence_id } => {
                write!(formatter, "evidence {evidence_id} is missing or stale")
            }
            Self::EvidenceRolePromotion { role } => write!(
                formatter,
                "recorded-only evidence cannot satisfy property evidence (role {role})"
            ),
            Self::ConflictedDecision => {
                write!(formatter, "conflicting terminal decisions remain visible")
            }
            Self::InvalidRootShape => write!(formatter, "root operation must be one Init"),
            Self::CandidateSubstitution => write!(
                formatter,
                "candidate substitution is rejected; use supersession"
            ),
            Self::MissingRelatedBase { base } => write!(
                formatter,
                "root base object {base} is missing from related objects"
            ),
            Self::PortableTamper { field } => {
                write!(formatter, "portable envelope field {field} is tampered")
            }
            Self::Factory(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for ReleaseDecisionError {}

include!("release_decision_impl.rs");

#[cfg(test)]
#[path = "release_decision_tests.rs"]
mod tests;
