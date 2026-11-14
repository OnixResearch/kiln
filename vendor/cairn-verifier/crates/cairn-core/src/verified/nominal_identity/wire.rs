use serde::Deserialize;
use serde::Serialize;

use super::types::{
    ActorId, ArchiveDigest, ArchiveId, ArtifactDigest, BinaryDigest, ChainDigest, ChangeId,
    ContentRootDigest, EvidenceKind, GitOidSha1Digest, GraphDigest, ImportedDigest, InputDigest,
    LedgerDigest, MantleDigest, OperationId, PackDigest, PlanDigest, PolicyDigest, PolicyKind,
    ProjectionRole, ReceiptDigest, Sha256Digest, SidecarDigest, SourceDigest, StateDigest,
    ValenceDigest, WorkspaceId, MIXED_BLAKE3_PROFILE,
};
use super::IdentityIssue;

const FIELD_VALUE: &str = "value";
const FIELD_ROLE: &str = "role";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdentityRole {
    ChangeId,
    WorkspaceId,
    ActorId,
    OperationId,
    ArchiveId,
    PolicyKind,
    EvidenceKind,
    ProjectionRole,
    PolicyDigest,
    InputDigest,
    PlanDigest,
    ReceiptDigest,
    ChainDigest,
    StateDigest,
    ContentRootDigest,
    ArtifactDigest,
    ValenceDigest,
    MantleDigest,
    GraphDigest,
    SourceDigest,
    BinaryDigest,
    SidecarDigest,
    ArchiveDigest,
    PackDigest,
    LedgerDigest,
    ImportedSha256,
    ImportedGitOidSha1,
}

/// Closed compatibility DTO for heterogeneous identity records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityWire {
    pub role: IdentityRole,
    pub value: String,
}

/// Closed admitted representation for heterogeneous identity records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnyIdentity {
    ChangeId(ChangeId),
    WorkspaceId(WorkspaceId),
    ActorId(ActorId),
    OperationId(OperationId),
    ArchiveId(ArchiveId),
    PolicyKind(PolicyKind),
    EvidenceKind(EvidenceKind),
    ProjectionRole(ProjectionRole),
    PolicyDigest(PolicyDigest),
    InputDigest(InputDigest),
    PlanDigest(PlanDigest),
    ReceiptDigest(ReceiptDigest),
    ChainDigest(ChainDigest),
    StateDigest(StateDigest),
    ContentRootDigest(ContentRootDigest),
    ArtifactDigest(ArtifactDigest),
    ValenceDigest(ValenceDigest),
    MantleDigest(MantleDigest),
    GraphDigest(GraphDigest),
    SourceDigest(SourceDigest),
    BinaryDigest(BinaryDigest),
    SidecarDigest(SidecarDigest),
    ArchiveDigest(ArchiveDigest),
    PackDigest(PackDigest),
    LedgerDigest(LedgerDigest),
    ImportedSha256(Sha256Digest),
    ImportedGitOidSha1(GitOidSha1Digest),
}

impl AnyIdentity {
    #[must_use]
    pub const fn role(&self) -> IdentityRole {
        match self {
            Self::ChangeId(_) => IdentityRole::ChangeId,
            Self::WorkspaceId(_) => IdentityRole::WorkspaceId,
            Self::ActorId(_) => IdentityRole::ActorId,
            Self::OperationId(_) => IdentityRole::OperationId,
            Self::ArchiveId(_) => IdentityRole::ArchiveId,
            Self::PolicyKind(_) => IdentityRole::PolicyKind,
            Self::EvidenceKind(_) => IdentityRole::EvidenceKind,
            Self::ProjectionRole(_) => IdentityRole::ProjectionRole,
            Self::PolicyDigest(_) => IdentityRole::PolicyDigest,
            Self::InputDigest(_) => IdentityRole::InputDigest,
            Self::PlanDigest(_) => IdentityRole::PlanDigest,
            Self::ReceiptDigest(_) => IdentityRole::ReceiptDigest,
            Self::ChainDigest(_) => IdentityRole::ChainDigest,
            Self::StateDigest(_) => IdentityRole::StateDigest,
            Self::ContentRootDigest(_) => IdentityRole::ContentRootDigest,
            Self::ArtifactDigest(_) => IdentityRole::ArtifactDigest,
            Self::ValenceDigest(_) => IdentityRole::ValenceDigest,
            Self::MantleDigest(_) => IdentityRole::MantleDigest,
            Self::GraphDigest(_) => IdentityRole::GraphDigest,
            Self::SourceDigest(_) => IdentityRole::SourceDigest,
            Self::BinaryDigest(_) => IdentityRole::BinaryDigest,
            Self::SidecarDigest(_) => IdentityRole::SidecarDigest,
            Self::ArchiveDigest(_) => IdentityRole::ArchiveDigest,
            Self::PackDigest(_) => IdentityRole::PackDigest,
            Self::LedgerDigest(_) => IdentityRole::LedgerDigest,
            Self::ImportedSha256(_) => IdentityRole::ImportedSha256,
            Self::ImportedGitOidSha1(_) => IdentityRole::ImportedGitOidSha1,
        }
    }

    pub fn try_into_policy_digest(self) -> Result<PolicyDigest, IdentityIssue> {
        match self {
            Self::PolicyDigest(value) => Ok(value),
            _ => Err(IdentityIssue::new(FIELD_ROLE, "identity_role_mismatch")),
        }
    }

    pub fn try_into_receipt_digest(self) -> Result<ReceiptDigest, IdentityIssue> {
        match self {
            Self::ReceiptDigest(value) => Ok(value),
            _ => Err(IdentityIssue::new(FIELD_ROLE, "identity_role_mismatch")),
        }
    }

    pub fn try_into_operation_id(self) -> Result<OperationId, IdentityIssue> {
        match self {
            Self::OperationId(value) => Ok(value),
            _ => Err(IdentityIssue::new(FIELD_ROLE, "identity_role_mismatch")),
        }
    }
}

impl TryFrom<IdentityWire> for AnyIdentity {
    type Error = IdentityIssue;

    fn try_from(wire: IdentityWire) -> Result<Self, Self::Error> {
        let value = wire.value;
        match wire.role {
            IdentityRole::ChangeId => ChangeId::parse(&value, FIELD_VALUE).map(Self::ChangeId),
            IdentityRole::WorkspaceId => {
                WorkspaceId::parse(&value, FIELD_VALUE).map(Self::WorkspaceId)
            }
            IdentityRole::ActorId => ActorId::parse(&value, FIELD_VALUE).map(Self::ActorId),
            IdentityRole::OperationId => {
                OperationId::parse(&value, FIELD_VALUE).map(Self::OperationId)
            }
            IdentityRole::ArchiveId => ArchiveId::parse(&value, FIELD_VALUE).map(Self::ArchiveId),
            IdentityRole::PolicyKind => {
                PolicyKind::parse(&value, FIELD_VALUE).map(Self::PolicyKind)
            }
            IdentityRole::EvidenceKind => {
                EvidenceKind::parse(&value, FIELD_VALUE).map(Self::EvidenceKind)
            }
            IdentityRole::ProjectionRole => {
                ProjectionRole::parse(&value, FIELD_VALUE).map(Self::ProjectionRole)
            }
            IdentityRole::PolicyDigest => parse_digest(&value).map(Self::PolicyDigest),
            IdentityRole::InputDigest => parse_digest(&value).map(Self::InputDigest),
            IdentityRole::PlanDigest => parse_digest(&value).map(Self::PlanDigest),
            IdentityRole::ReceiptDigest => parse_digest(&value).map(Self::ReceiptDigest),
            IdentityRole::ChainDigest => parse_digest(&value).map(Self::ChainDigest),
            IdentityRole::StateDigest => parse_digest(&value).map(Self::StateDigest),
            IdentityRole::ContentRootDigest => parse_digest(&value).map(Self::ContentRootDigest),
            IdentityRole::ArtifactDigest => parse_digest(&value).map(Self::ArtifactDigest),
            IdentityRole::ValenceDigest => parse_digest(&value).map(Self::ValenceDigest),
            IdentityRole::MantleDigest => parse_digest(&value).map(Self::MantleDigest),
            IdentityRole::GraphDigest => parse_digest(&value).map(Self::GraphDigest),
            IdentityRole::SourceDigest => parse_digest(&value).map(Self::SourceDigest),
            IdentityRole::BinaryDigest => parse_digest(&value).map(Self::BinaryDigest),
            IdentityRole::SidecarDigest => parse_digest(&value).map(Self::SidecarDigest),
            IdentityRole::ArchiveDigest => parse_digest(&value).map(Self::ArchiveDigest),
            IdentityRole::PackDigest => parse_digest(&value).map(Self::PackDigest),
            IdentityRole::LedgerDigest => parse_digest(&value).map(Self::LedgerDigest),
            IdentityRole::ImportedSha256 => {
                ImportedDigest::parse(&value, FIELD_VALUE).map(Self::ImportedSha256)
            }
            IdentityRole::ImportedGitOidSha1 => {
                ImportedDigest::parse(&value, FIELD_VALUE).map(Self::ImportedGitOidSha1)
            }
        }
    }
}

fn parse_digest<Role: super::DigestRoleMarker>(
    value: &str,
) -> Result<super::Blake3Digest<Role>, IdentityIssue> {
    super::Blake3Digest::<Role>::parse_compatible(value, MIXED_BLAKE3_PROFILE, FIELD_VALUE)
}
