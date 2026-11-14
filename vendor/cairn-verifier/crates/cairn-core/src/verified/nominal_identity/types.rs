use std::marker::PhantomData;

use serde::Deserialize;
use serde::Serialize;

const BLAKE3_HEX_LENGTH: usize = 64;
const SHA256_HEX_LENGTH: usize = 64;
const GIT_OID_SHA1_HEX_LENGTH: usize = 40;
const DEFAULT_DOMAIN_ID_MAX_BYTES: usize = 128;
const ACTOR_ID_MAX_BYTES: usize = 256;
const DOMAIN_SEPARATOR: char = ':';
const B3_PREFIX: &str = "b3:";
const BLAKE3_PREFIX: &str = "blake3:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DigestSpelling {
    RawHex,
    B3Prefix,
    Blake3Prefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DigestProfile {
    pub id: &'static str,
    pub allow_raw_hex: bool,
    pub allow_b3_prefix: bool,
    pub allow_blake3_prefix: bool,
}

pub const RAW_BLAKE3_PROFILE: DigestProfile = DigestProfile {
    id: "raw-blake3",
    allow_raw_hex: true,
    allow_b3_prefix: false,
    allow_blake3_prefix: false,
};

pub const STRICT_B3_PROFILE: DigestProfile = DigestProfile {
    id: "strict-b3",
    allow_raw_hex: false,
    allow_b3_prefix: true,
    allow_blake3_prefix: false,
};

pub const MIXED_BLAKE3_PROFILE: DigestProfile = DigestProfile {
    id: "mixed-blake3",
    allow_raw_hex: true,
    allow_b3_prefix: true,
    allow_blake3_prefix: true,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdentityIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl IdentityIssue {
    #[must_use]
    pub const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

mod sealed {
    pub trait Sealed {}
}

pub trait DomainRoleMarker:
    sealed::Sealed + Clone + Copy + std::fmt::Debug + Eq + Ord + 'static
{
    const ROLE: &'static str;
    const REQUIRED_PREFIX: Option<&'static str>;
    const ALLOW_UNPREFIXED_COMPATIBILITY: bool = false;
    const MAX_BYTES: usize;
}

pub trait DigestRoleMarker:
    sealed::Sealed + Clone + Copy + std::fmt::Debug + Eq + Ord + 'static
{
    const ROLE: &'static str;
}

pub trait ImportedDigestRoleMarker:
    sealed::Sealed + Clone + Copy + std::fmt::Debug + Eq + Ord + 'static
{
    const ROLE: &'static str;
    const ALGORITHM: ImportedDigestAlgorithm;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChangeIdRole;
impl sealed::Sealed for ChangeIdRole {}
impl DomainRoleMarker for ChangeIdRole {
    const ROLE: &'static str = "change-id";
    const REQUIRED_PREFIX: Option<&'static str> = None;
    const MAX_BYTES: usize = DEFAULT_DOMAIN_ID_MAX_BYTES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkspaceIdRole;
impl sealed::Sealed for WorkspaceIdRole {}
impl DomainRoleMarker for WorkspaceIdRole {
    const ROLE: &'static str = "workspace-id";
    const REQUIRED_PREFIX: Option<&'static str> = Some("workspace:");
    const MAX_BYTES: usize = DEFAULT_DOMAIN_ID_MAX_BYTES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActorIdRole;
impl sealed::Sealed for ActorIdRole {}
impl DomainRoleMarker for ActorIdRole {
    const ROLE: &'static str = "actor-id";
    const REQUIRED_PREFIX: Option<&'static str> = Some("actor:");
    const MAX_BYTES: usize = ACTOR_ID_MAX_BYTES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationIdRole;
impl sealed::Sealed for OperationIdRole {}
impl DomainRoleMarker for OperationIdRole {
    const ROLE: &'static str = "operation-id";
    const REQUIRED_PREFIX: Option<&'static str> = Some("operation:");
    const ALLOW_UNPREFIXED_COMPATIBILITY: bool = true;
    const MAX_BYTES: usize = DEFAULT_DOMAIN_ID_MAX_BYTES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArchiveIdRole;
impl sealed::Sealed for ArchiveIdRole {}
impl DomainRoleMarker for ArchiveIdRole {
    const ROLE: &'static str = "archive-id";
    const REQUIRED_PREFIX: Option<&'static str> = None;
    const MAX_BYTES: usize = DEFAULT_DOMAIN_ID_MAX_BYTES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PolicyKindRole;
impl sealed::Sealed for PolicyKindRole {}
impl DomainRoleMarker for PolicyKindRole {
    const ROLE: &'static str = "policy-kind";
    const REQUIRED_PREFIX: Option<&'static str> = None;
    const MAX_BYTES: usize = DEFAULT_DOMAIN_ID_MAX_BYTES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvidenceKindRole;
impl sealed::Sealed for EvidenceKindRole {}
impl DomainRoleMarker for EvidenceKindRole {
    const ROLE: &'static str = "evidence-kind";
    const REQUIRED_PREFIX: Option<&'static str> = None;
    const MAX_BYTES: usize = DEFAULT_DOMAIN_ID_MAX_BYTES;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectionRoleMarker;
impl sealed::Sealed for ProjectionRoleMarker {}
impl DomainRoleMarker for ProjectionRoleMarker {
    const ROLE: &'static str = "projection-role";
    const REQUIRED_PREFIX: Option<&'static str> = None;
    const MAX_BYTES: usize = DEFAULT_DOMAIN_ID_MAX_BYTES;
}

macro_rules! digest_role {
    ($name:ident, $role:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name;
        impl sealed::Sealed for $name {}
        impl DigestRoleMarker for $name {
            const ROLE: &'static str = $role;
        }
    };
}

digest_role!(PolicyDigestRole, "policy-digest");
digest_role!(InputDigestRole, "input-digest");
digest_role!(PlanDigestRole, "plan-digest");
digest_role!(ReceiptDigestRole, "receipt-digest");
digest_role!(ChainDigestRole, "chain-digest");
digest_role!(StateDigestRole, "state-digest");
digest_role!(ContentRootDigestRole, "content-root-digest");
digest_role!(ArtifactDigestRole, "artifact-digest");
digest_role!(ValenceDigestRole, "valence-digest");
digest_role!(MantleDigestRole, "mantle-digest");
digest_role!(GraphDigestRole, "graph-digest");
digest_role!(SourceDigestRole, "source-digest");
digest_role!(BinaryDigestRole, "binary-digest");
digest_role!(SidecarDigestRole, "sidecar-digest");
digest_role!(ArchiveDigestRole, "archive-digest");
digest_role!(PackDigestRole, "pack-digest");
digest_role!(LedgerDigestRole, "ledger-digest");

/// An owned domain identifier whose role is part of its Rust type.
///
/// The inner value is private. The type has no `Deref` implementation and no
/// unchecked primitive conversion.
///
/// Change and workspace identities are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{ChangeId, WorkspaceId};
/// fn requires_change(_: ChangeId) {}
/// let value = WorkspaceId::parse("workspace:example", "workspace_id").unwrap();
/// requires_change(value);
/// ```
///
/// Operation and actor identities are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{ActorId, OperationId};
/// fn requires_operation(_: OperationId) {}
/// let value = ActorId::parse("actor:operator", "actor_id").unwrap();
/// requires_operation(value);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DomainId<Role: DomainRoleMarker> {
    value: String,
    role: PhantomData<fn() -> Role>,
}

impl<Role: DomainRoleMarker> DomainId<Role> {
    pub fn parse(value: &str, field: &'static str) -> Result<Self, IdentityIssue> {
        validate_domain_id::<Role>(value, field)?;
        Ok(Self {
            value: value.to_owned(),
            role: PhantomData,
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn as_ref(&self) -> DomainIdRef<'_, Role> {
        DomainIdRef {
            value: &self.value,
            role: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DomainIdRef<'a, Role: DomainRoleMarker> {
    value: &'a str,
    role: PhantomData<fn() -> Role>,
}

impl<'a, Role: DomainRoleMarker> DomainIdRef<'a, Role> {
    pub fn parse(value: &'a str, field: &'static str) -> Result<Self, IdentityIssue> {
        validate_domain_id::<Role>(value, field)?;
        Ok(Self {
            value,
            role: PhantomData,
        })
    }

    #[must_use]
    pub fn as_str(self) -> &'a str {
        self.value
    }

    #[must_use]
    pub fn to_owned_id(self) -> DomainId<Role> {
        DomainId {
            value: self.value.to_owned(),
            role: PhantomData,
        }
    }
}

fn validate_domain_id<Role: DomainRoleMarker>(
    value: &str,
    field: &'static str,
) -> Result<(), IdentityIssue> {
    if value.is_empty() {
        return Err(IdentityIssue::new(field, "identity_empty"));
    }
    if value.len() > Role::MAX_BYTES {
        return Err(IdentityIssue::new(field, "identity_oversized"));
    }
    let suffix = match Role::REQUIRED_PREFIX {
        Some(prefix) => match value.strip_prefix(prefix) {
            Some(suffix) => suffix,
            None if Role::ALLOW_UNPREFIXED_COMPATIBILITY => {
                if value.contains(DOMAIN_SEPARATOR) {
                    return Err(IdentityIssue::new(field, "identity_prefix_unsupported"));
                }
                value
            }
            None => return Err(IdentityIssue::new(field, "identity_prefix_unsupported")),
        },
        None => {
            if value.contains(DOMAIN_SEPARATOR) {
                return Err(IdentityIssue::new(field, "identity_prefix_unsupported"));
            }
            value
        }
    };
    if suffix.is_empty() {
        return Err(IdentityIssue::new(field, "identity_suffix_empty"));
    }
    if !suffix.bytes().all(|byte| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'-' | b'_' | b'.' | b'/')
    }) {
        return Err(IdentityIssue::new(field, "identity_spelling_noncanonical"));
    }
    let first = suffix.as_bytes()[0];
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(IdentityIssue::new(field, "identity_start_invalid"));
    }
    Ok(())
}

/// An owned normalized BLAKE3 digest whose role is part of its Rust type.
///
/// Same-role values compile:
///
/// ```
/// use cairn_core::{PolicyDigest, RAW_BLAKE3_PROFILE};
/// let digest = PolicyDigest::parse(
///     "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
///     RAW_BLAKE3_PROFILE,
///     "policy_hash",
/// )?;
/// let same_role: PolicyDigest = digest;
/// assert_eq!(same_role.as_hex().len(), 64);
/// # Ok::<(), cairn_core::IdentityIssue>(())
/// ```
///
/// Policy and input digests are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{InputDigest, PolicyDigest, RAW_BLAKE3_PROFILE};
/// fn requires_policy(_: PolicyDigest) {}
/// let value = InputDigest::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", RAW_BLAKE3_PROFILE, "input_hash").unwrap();
/// requires_policy(value);
/// ```
///
/// Plan and receipt digests are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{PlanDigest, ReceiptDigest, RAW_BLAKE3_PROFILE};
/// fn requires_plan(_: PlanDigest) {}
/// let value = ReceiptDigest::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", RAW_BLAKE3_PROFILE, "receipt_hash").unwrap();
/// requires_plan(value);
/// ```
///
/// Receipt and chain digests are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{ChainDigest, ReceiptDigest, RAW_BLAKE3_PROFILE};
/// fn requires_chain(_: ChainDigest) {}
/// let value = ReceiptDigest::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", RAW_BLAKE3_PROFILE, "receipt_hash").unwrap();
/// requires_chain(value);
/// ```
///
/// Graph and sidecar digests are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{GraphDigest, SidecarDigest, RAW_BLAKE3_PROFILE};
/// fn requires_graph(_: GraphDigest) {}
/// let value = SidecarDigest::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", RAW_BLAKE3_PROFILE, "sidecar_hash").unwrap();
/// requires_graph(value);
/// ```
///
/// Source and binary digests are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{BinaryDigest, SourceDigest, RAW_BLAKE3_PROFILE};
/// fn requires_binary(_: BinaryDigest) {}
/// let value = SourceDigest::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", RAW_BLAKE3_PROFILE, "source_hash").unwrap();
/// requires_binary(value);
/// ```
///
/// Valence and Mantle digests are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{MantleDigest, ValenceDigest, RAW_BLAKE3_PROFILE};
/// fn requires_valence(_: ValenceDigest) {}
/// let value = MantleDigest::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", RAW_BLAKE3_PROFILE, "mantle_hash").unwrap();
/// requires_valence(value);
/// ```
///
/// State and content-root digests are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{ContentRootDigest, StateDigest, RAW_BLAKE3_PROFILE};
/// fn requires_state(_: StateDigest) {}
/// let value = ContentRootDigest::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", RAW_BLAKE3_PROFILE, "content_root").unwrap();
/// requires_state(value);
/// ```
///
/// Archive and pack digests are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{ArchiveDigest, PackDigest, RAW_BLAKE3_PROFILE};
/// fn requires_archive(_: ArchiveDigest) {}
/// let value = PackDigest::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", RAW_BLAKE3_PROFILE, "pack_root").unwrap();
/// requires_archive(value);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Blake3Digest<Role: DigestRoleMarker> {
    normalized_hex: String,
    role: PhantomData<fn() -> Role>,
}

impl<Role: DigestRoleMarker> Blake3Digest<Role> {
    pub fn parse(
        value: &str,
        profile: DigestProfile,
        field: &'static str,
    ) -> Result<Self, IdentityIssue> {
        let (_, normalized_hex) = parse_blake3_parts(value, profile, field, false)?;
        Ok(Self {
            normalized_hex: normalized_hex.to_owned(),
            role: PhantomData,
        })
    }

    pub fn parse_compatible(
        value: &str,
        profile: DigestProfile,
        field: &'static str,
    ) -> Result<Self, IdentityIssue> {
        let (_, normalized_hex) = parse_blake3_parts(value, profile, field, true)?;
        Ok(Self {
            normalized_hex: normalized_hex.to_ascii_lowercase(),
            role: PhantomData,
        })
    }

    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.normalized_hex
    }

    #[must_use]
    pub fn as_ref(&self) -> Blake3DigestRef<'_, Role> {
        Blake3DigestRef {
            normalized_hex: &self.normalized_hex,
            role: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Blake3DigestRef<'a, Role: DigestRoleMarker> {
    normalized_hex: &'a str,
    role: PhantomData<fn() -> Role>,
}

impl<'a, Role: DigestRoleMarker> Blake3DigestRef<'a, Role> {
    pub fn parse(
        value: &'a str,
        profile: DigestProfile,
        field: &'static str,
    ) -> Result<Self, IdentityIssue> {
        let (_, normalized_hex) = parse_blake3_parts(value, profile, field, false)?;
        Ok(Self {
            normalized_hex,
            role: PhantomData,
        })
    }

    #[must_use]
    pub fn as_hex(self) -> &'a str {
        self.normalized_hex
    }

    #[must_use]
    pub fn matches(self, other: Self) -> bool {
        self.normalized_hex == other.normalized_hex
    }

    #[must_use]
    pub fn to_owned_digest(self) -> Blake3Digest<Role> {
        Blake3Digest {
            normalized_hex: self.normalized_hex.to_owned(),
            role: PhantomData,
        }
    }
}

fn parse_blake3_parts<'a>(
    value: &'a str,
    profile: DigestProfile,
    field: &'static str,
    allow_uppercase: bool,
) -> Result<(DigestSpelling, &'a str), IdentityIssue> {
    let (spelling, hex) = if let Some(hex) = value.strip_prefix(B3_PREFIX) {
        (DigestSpelling::B3Prefix, hex)
    } else if let Some(hex) = value.strip_prefix(BLAKE3_PREFIX) {
        (DigestSpelling::Blake3Prefix, hex)
    } else {
        (DigestSpelling::RawHex, value)
    };
    if !profile_allows_spelling(profile, spelling) {
        return Err(IdentityIssue::new(field, "digest_prefix_unsupported"));
    }
    if hex.len() != BLAKE3_HEX_LENGTH {
        return Err(IdentityIssue::new(field, "digest_length_invalid"));
    }
    let valid = if allow_uppercase {
        hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    } else {
        hex.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    if !valid {
        return Err(IdentityIssue::new(field, "digest_spelling_noncanonical"));
    }
    Ok((spelling, hex))
}

fn profile_allows_spelling(profile: DigestProfile, spelling: DigestSpelling) -> bool {
    match spelling {
        DigestSpelling::RawHex => profile.allow_raw_hex,
        DigestSpelling::B3Prefix => profile.allow_b3_prefix,
        DigestSpelling::Blake3Prefix => profile.allow_blake3_prefix,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportedDigestAlgorithm {
    Sha256,
    GitOidSha1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sha256DigestRole;
impl sealed::Sealed for Sha256DigestRole {}
impl ImportedDigestRoleMarker for Sha256DigestRole {
    const ROLE: &'static str = "imported-sha256-digest";
    const ALGORITHM: ImportedDigestAlgorithm = ImportedDigestAlgorithm::Sha256;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitOidSha1DigestRole;
impl sealed::Sealed for GitOidSha1DigestRole {}
impl ImportedDigestRoleMarker for GitOidSha1DigestRole {
    const ROLE: &'static str = "imported-git-oid-sha1";
    const ALGORITHM: ImportedDigestAlgorithm = ImportedDigestAlgorithm::GitOidSha1;
}

/// An imported digest whose algorithm is part of its Rust type.
///
/// SHA-256 and Git object identities are not interchangeable:
///
/// ```compile_fail
/// use cairn_core::{GitOidSha1Digest, Sha256Digest};
/// fn requires_sha256(_: Sha256Digest) {}
/// let value = GitOidSha1Digest::parse("0123456789abcdef0123456789abcdef01234567", "git_oid").unwrap();
/// requires_sha256(value);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImportedDigest<Role: ImportedDigestRoleMarker> {
    normalized_hex: String,
    role: PhantomData<fn() -> Role>,
}

impl<Role: ImportedDigestRoleMarker> ImportedDigest<Role> {
    pub fn parse(value: &str, field: &'static str) -> Result<Self, IdentityIssue> {
        let expected_length = imported_digest_hex_length(Role::ALGORITHM);
        if value.len() != expected_length {
            return Err(IdentityIssue::new(field, "imported_digest_length_invalid"));
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(IdentityIssue::new(
                field,
                "imported_digest_spelling_noncanonical",
            ));
        }
        Ok(Self {
            normalized_hex: value.to_owned(),
            role: PhantomData,
        })
    }

    #[must_use]
    pub fn algorithm(&self) -> ImportedDigestAlgorithm {
        Role::ALGORITHM
    }

    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.normalized_hex
    }
}

const fn imported_digest_hex_length(algorithm: ImportedDigestAlgorithm) -> usize {
    match algorithm {
        ImportedDigestAlgorithm::Sha256 => SHA256_HEX_LENGTH,
        ImportedDigestAlgorithm::GitOidSha1 => GIT_OID_SHA1_HEX_LENGTH,
    }
}

pub type ChangeId = DomainId<ChangeIdRole>;
pub type WorkspaceId = DomainId<WorkspaceIdRole>;
pub type ActorId = DomainId<ActorIdRole>;
pub type OperationId = DomainId<OperationIdRole>;
pub type ArchiveId = DomainId<ArchiveIdRole>;
pub type PolicyKind = DomainId<PolicyKindRole>;
pub type EvidenceKind = DomainId<EvidenceKindRole>;
pub type ProjectionRole = DomainId<ProjectionRoleMarker>;

pub type PolicyDigest = Blake3Digest<PolicyDigestRole>;
pub type InputDigest = Blake3Digest<InputDigestRole>;
pub type PlanDigest = Blake3Digest<PlanDigestRole>;
pub type ReceiptDigest = Blake3Digest<ReceiptDigestRole>;
pub type ChainDigest = Blake3Digest<ChainDigestRole>;
pub type StateDigest = Blake3Digest<StateDigestRole>;
pub type ContentRootDigest = Blake3Digest<ContentRootDigestRole>;
pub type ArtifactDigest = Blake3Digest<ArtifactDigestRole>;
pub type ValenceDigest = Blake3Digest<ValenceDigestRole>;
pub type MantleDigest = Blake3Digest<MantleDigestRole>;
pub type GraphDigest = Blake3Digest<GraphDigestRole>;
pub type SourceDigest = Blake3Digest<SourceDigestRole>;
pub type BinaryDigest = Blake3Digest<BinaryDigestRole>;
pub type SidecarDigest = Blake3Digest<SidecarDigestRole>;
pub type ArchiveDigest = Blake3Digest<ArchiveDigestRole>;
pub type PackDigest = Blake3Digest<PackDigestRole>;
pub type LedgerDigest = Blake3Digest<LedgerDigestRole>;
pub type Sha256Digest = ImportedDigest<Sha256DigestRole>;
pub type GitOidSha1Digest = ImportedDigest<GitOidSha1DigestRole>;
