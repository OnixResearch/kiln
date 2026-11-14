//! Nominal lifecycle and evidence identities for Cairn's pure core.
//!
//! Stable JSON and fixture structs remain wire models. Callers admit their raw
//! values through this module before role-sensitive evaluation. Equal text in
//! different roles remains different at compile time.
//!
//! The module proves checked local syntax and role separation only. It does not
//! prove evidence truth, authority, policy correctness, verifier soundness,
//! mutation safety outside checked plans, or release eligibility.
//!
//! r[impl cairn.nominal_evidence_identities.inventory]
//! r[verify cairn.nominal_evidence_identities.inventory.exclusions]
//! r[impl cairn.nominal_evidence_identities.types]
//! r[impl cairn.nominal_evidence_identities.validation]
//! r[impl cairn.nominal_evidence_identities.compile_time]
//! r[impl cairn.nominal_evidence_identities.wire_boundary]
//! r[impl cairn.nominal_evidence_identities.octet]
//! r[impl cairn.nominal_evidence_identities.docs]
//! r[verify cairn.nominal_evidence_identities.docs.pure]
//! r[impl cairn.nominal_evidence_identities.final_checks]
//! r[verify cairn.nominal_evidence_identities.final_checks.boundary]

mod catalog;
mod types;
mod wire;

pub use catalog::{
    nominal_identity_policy_diagnostics, nominal_octet_projection_hash,
    nominal_runtime_projection_hash, NominalIdentityDomainPolicy, NominalIdentityOctetDomain,
    NominalIdentityOctetProjection, NominalIdentityPolicy, NominalIdentityRuntimeProjection,
    NOMINAL_IDENTITY_OCTET_SCHEMA_V1, NOMINAL_IDENTITY_RUNTIME_SCHEMA_V1,
};
pub use types::{
    ActorId, ActorIdRole, ArchiveDigest, ArchiveDigestRole, ArchiveId, ArchiveIdRole,
    ArtifactDigest, ArtifactDigestRole, BinaryDigest, BinaryDigestRole, Blake3Digest,
    Blake3DigestRef, ChainDigest, ChainDigestRole, ChangeId, ChangeIdRole, ContentRootDigest,
    ContentRootDigestRole, DigestProfile, DigestRoleMarker, DigestSpelling, DomainId, DomainIdRef,
    DomainRoleMarker, EvidenceKind, EvidenceKindRole, GitOidSha1Digest, GitOidSha1DigestRole,
    GraphDigest, GraphDigestRole, IdentityIssue, ImportedDigest, ImportedDigestAlgorithm,
    ImportedDigestRoleMarker, InputDigest, InputDigestRole, LedgerDigest, LedgerDigestRole,
    MantleDigest, MantleDigestRole, OperationId, OperationIdRole, PackDigest, PackDigestRole,
    PlanDigest, PlanDigestRole, PolicyDigest, PolicyDigestRole, PolicyKind, PolicyKindRole,
    ProjectionRole, ProjectionRoleMarker, ReceiptDigest, ReceiptDigestRole, Sha256Digest,
    Sha256DigestRole, SidecarDigest, SidecarDigestRole, SourceDigest, SourceDigestRole,
    StateDigest, StateDigestRole, ValenceDigest, ValenceDigestRole, WorkspaceId, WorkspaceIdRole,
    MIXED_BLAKE3_PROFILE, RAW_BLAKE3_PROFILE, STRICT_B3_PROFILE,
};
pub use wire::{AnyIdentity, IdentityRole, IdentityWire};

#[cfg(test)]
mod tests;
