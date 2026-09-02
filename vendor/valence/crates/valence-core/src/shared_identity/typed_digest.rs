use std::marker::PhantomData;

use super::B3_PREFIX;
use super::BLAKE3_PREFIX;
use super::DigestProfile;
use super::DigestRole;
use super::DigestSpelling;
use super::SharedIdentityIssue;

mod sealed {
    pub trait Sealed {}
}

/// Marker implemented by each closed digest role.
pub trait DigestRoleMarker: sealed::Sealed + Clone + Copy + std::fmt::Debug + Eq + Ord + 'static {
    const ROLE: DigestRole;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BinaryRole;
impl sealed::Sealed for BinaryRole {}
impl DigestRoleMarker for BinaryRole {
    const ROLE: DigestRole = DigestRole::BinaryBytes;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SidecarRole;
impl sealed::Sealed for SidecarRole {}
impl DigestRoleMarker for SidecarRole {
    const ROLE: DigestRole = DigestRole::SidecarBytes;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArtifactIdentityRole;
impl sealed::Sealed for ArtifactIdentityRole {}
impl DigestRoleMarker for ArtifactIdentityRole {
    const ROLE: DigestRole = DigestRole::ArtifactIdentity;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReceiptRole;
impl sealed::Sealed for ReceiptRole {}
impl DigestRoleMarker for ReceiptRole {
    const ROLE: DigestRole = DigestRole::ReceiptIdentity;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PolicyRole;
impl sealed::Sealed for PolicyRole {}
impl DigestRoleMarker for PolicyRole {
    const ROLE: DigestRole = DigestRole::PolicyIdentity;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphRole;
impl sealed::Sealed for GraphRole {}
impl DigestRoleMarker for GraphRole {
    const ROLE: DigestRole = DigestRole::GraphIdentity;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceRole;
impl sealed::Sealed for SourceRole {}
impl DigestRoleMarker for SourceRole {
    const ROLE: DigestRole = DigestRole::SourceIdentity;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PreservesMetadataRole;
impl sealed::Sealed for PreservesMetadataRole {}
impl DigestRoleMarker for PreservesMetadataRole {
    const ROLE: DigestRole = DigestRole::PreservesMetadata;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompatibilityReferenceRole;
impl sealed::Sealed for CompatibilityReferenceRole {}
impl DigestRoleMarker for CompatibilityReferenceRole {
    const ROLE: DigestRole = DigestRole::CompatibilityReference;
}

/// An owned, normalized BLAKE3 digest for one compile-time role.
///
/// The private field prevents unchecked construction. Use [`Digest::parse`] for
/// current Valence fields or [`Digest::parse_compatible`] at an existing
/// case-insensitive compatibility boundary.
///
/// Same-role values compile:
///
/// ```
/// # use valence_core::shared_identity::{BinaryDigest, RAW_BLAKE3_PROFILE};
///
/// let digest = BinaryDigest::parse(
///     "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
///     RAW_BLAKE3_PROFILE,
///     "binary_hash",
/// )?;
/// let same_role: BinaryDigest = digest;
/// assert_eq!(same_role.as_hex(), "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
/// # Ok::<(), valence_core::shared_identity::SharedIdentityIssue>(())
/// ```
///
/// Receipt and binary roles are not interchangeable:
///
/// ```compile_fail
/// # use valence_core::shared_identity::{BinaryDigest, ReceiptDigest, RAW_BLAKE3_PROFILE};
/// fn requires_binary(_: BinaryDigest) {}
/// let value = ReceiptDigest::parse("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f", RAW_BLAKE3_PROFILE, "digest").unwrap();
/// requires_binary(value);
/// ```
///
/// Binary and receipt roles are not interchangeable:
///
/// ```compile_fail
/// # use valence_core::shared_identity::{BinaryDigest, ReceiptDigest, RAW_BLAKE3_PROFILE};
/// fn requires_receipt(_: ReceiptDigest) {}
/// let value = BinaryDigest::parse("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f", RAW_BLAKE3_PROFILE, "digest").unwrap();
/// requires_receipt(value);
/// ```
///
/// Policy and artifact roles are not interchangeable:
///
/// ```compile_fail
/// # use valence_core::shared_identity::{ArtifactIdentityDigest, PolicyDigest, RAW_BLAKE3_PROFILE};
/// fn requires_artifact(_: ArtifactIdentityDigest) {}
/// let value = PolicyDigest::parse("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f", RAW_BLAKE3_PROFILE, "digest").unwrap();
/// requires_artifact(value);
/// ```
///
/// Artifact and policy roles are not interchangeable:
///
/// ```compile_fail
/// # use valence_core::shared_identity::{ArtifactIdentityDigest, PolicyDigest, RAW_BLAKE3_PROFILE};
/// fn requires_policy(_: PolicyDigest) {}
/// let value = ArtifactIdentityDigest::parse("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f", RAW_BLAKE3_PROFILE, "digest").unwrap();
/// requires_policy(value);
/// ```
///
/// Graph and sidecar roles are not interchangeable:
///
/// ```compile_fail
/// # use valence_core::shared_identity::{GraphDigest, SidecarDigest, RAW_BLAKE3_PROFILE};
/// fn requires_sidecar(_: SidecarDigest) {}
/// let value = GraphDigest::parse("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f", RAW_BLAKE3_PROFILE, "digest").unwrap();
/// requires_sidecar(value);
/// ```
///
/// Sidecar and graph roles are not interchangeable:
///
/// ```compile_fail
/// # use valence_core::shared_identity::{GraphDigest, SidecarDigest, RAW_BLAKE3_PROFILE};
/// fn requires_graph(_: GraphDigest) {}
/// let value = SidecarDigest::parse("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f", RAW_BLAKE3_PROFILE, "digest").unwrap();
/// requires_graph(value);
/// ```
///
/// Source and artifact roles are not interchangeable:
///
/// ```compile_fail
/// # use valence_core::shared_identity::{ArtifactIdentityDigest, SourceDigest, RAW_BLAKE3_PROFILE};
/// fn requires_artifact(_: ArtifactIdentityDigest) {}
/// let value = SourceDigest::parse("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f", RAW_BLAKE3_PROFILE, "digest").unwrap();
/// requires_artifact(value);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Digest<Role: DigestRoleMarker> {
    normalized_hex: String,
    role: PhantomData<fn() -> Role>,
}

impl<Role: DigestRoleMarker> Digest<Role> {
    pub fn parse(value: &str, profile: DigestProfile, field: &'static str) -> Result<Self, SharedIdentityIssue> {
        let (_, normalized_hex) = parse_parts(value, profile, field, HexCase::LowercaseOnly)?;
        Ok(Self {
            normalized_hex: normalized_hex.to_owned(),
            role: PhantomData,
        })
    }

    pub fn parse_compatible(
        value: &str,
        profile: DigestProfile,
        field: &'static str,
    ) -> Result<Self, SharedIdentityIssue> {
        let (_, normalized_hex) = parse_parts(value, profile, field, HexCase::Insensitive)?;
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
    pub fn as_ref(&self) -> DigestRef<'_, Role> {
        DigestRef {
            normalized_hex: &self.normalized_hex,
            role: PhantomData,
        }
    }
}

/// A borrowed, validated BLAKE3 digest view for one compile-time role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DigestRef<'a, Role: DigestRoleMarker> {
    normalized_hex: &'a str,
    role: PhantomData<fn() -> Role>,
}

impl<'a, Role: DigestRoleMarker> DigestRef<'a, Role> {
    pub fn parse(value: &'a str, profile: DigestProfile, field: &'static str) -> Result<Self, SharedIdentityIssue> {
        let (_, normalized_hex) = parse_parts(value, profile, field, HexCase::LowercaseOnly)?;
        Ok(Self {
            normalized_hex,
            role: PhantomData,
        })
    }

    pub fn parse_compatible(
        value: &'a str,
        profile: DigestProfile,
        field: &'static str,
    ) -> Result<Self, SharedIdentityIssue> {
        let (_, normalized_hex) = parse_parts(value, profile, field, HexCase::Insensitive)?;
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
        self.normalized_hex.eq_ignore_ascii_case(other.normalized_hex)
    }

    #[must_use]
    pub fn to_owned_digest(self) -> Digest<Role> {
        Digest {
            normalized_hex: self.normalized_hex.to_ascii_lowercase(),
            role: PhantomData,
        }
    }
}

/// A typed digest plus compatibility metadata retained from its wire form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedDigest<Role: DigestRoleMarker> {
    digest: Digest<Role>,
    original: String,
    spelling: DigestSpelling,
}

impl<Role: DigestRoleMarker> AdmittedDigest<Role> {
    pub fn parse(value: &str, profile: DigestProfile, field: &'static str) -> Result<Self, SharedIdentityIssue> {
        let (spelling, normalized_hex) = parse_parts(value, profile, field, HexCase::LowercaseOnly)?;
        Ok(Self {
            digest: Digest {
                normalized_hex: normalized_hex.to_owned(),
                role: PhantomData,
            },
            original: value.to_owned(),
            spelling,
        })
    }

    pub fn parse_compatible(
        value: &str,
        profile: DigestProfile,
        field: &'static str,
    ) -> Result<Self, SharedIdentityIssue> {
        let (spelling, normalized_hex) = parse_parts(value, profile, field, HexCase::Insensitive)?;
        Ok(Self {
            digest: Digest {
                normalized_hex: normalized_hex.to_ascii_lowercase(),
                role: PhantomData,
            },
            original: value.to_owned(),
            spelling,
        })
    }

    pub(super) fn from_wire(
        original: String,
        normalized_hex: String,
        spelling: DigestSpelling,
        field: &'static str,
    ) -> Result<Self, SharedIdentityIssue> {
        let profile = exact_spelling_profile(spelling);
        let (parsed_spelling, parsed_hex) = parse_parts(&original, profile, field, HexCase::LowercaseOnly)?;
        if parsed_spelling != spelling {
            return Err(SharedIdentityIssue::new(field, "digest_spelling_mismatch"));
        }
        if parsed_hex != normalized_hex {
            return Err(SharedIdentityIssue::new(field, "normalized_digest_mismatch"));
        }
        let digest = Digest {
            normalized_hex,
            role: PhantomData,
        };
        Ok(Self {
            digest,
            original,
            spelling,
        })
    }

    #[must_use]
    pub fn digest(&self) -> &Digest<Role> {
        &self.digest
    }

    #[must_use]
    pub fn original(&self) -> &str {
        &self.original
    }

    #[must_use]
    pub const fn spelling(&self) -> DigestSpelling {
        self.spelling
    }

    pub(super) fn into_wire_parts(self) -> (String, String, DigestSpelling) {
        (self.original, self.digest.normalized_hex, self.spelling)
    }
}

pub type BinaryDigest = Digest<BinaryRole>;
pub type BinaryDigestRef<'a> = DigestRef<'a, BinaryRole>;
pub type AdmittedBinaryDigest = AdmittedDigest<BinaryRole>;
pub type SidecarDigest = Digest<SidecarRole>;
pub type SidecarDigestRef<'a> = DigestRef<'a, SidecarRole>;
pub type ArtifactIdentityDigest = Digest<ArtifactIdentityRole>;
pub type ArtifactIdentityDigestRef<'a> = DigestRef<'a, ArtifactIdentityRole>;
pub type ReceiptDigest = Digest<ReceiptRole>;
pub type ReceiptDigestRef<'a> = DigestRef<'a, ReceiptRole>;
pub type PolicyDigest = Digest<PolicyRole>;
pub type PolicyDigestRef<'a> = DigestRef<'a, PolicyRole>;
pub type GraphDigest = Digest<GraphRole>;
pub type GraphDigestRef<'a> = DigestRef<'a, GraphRole>;
pub type SourceDigest = Digest<SourceRole>;
pub type SourceDigestRef<'a> = DigestRef<'a, SourceRole>;
pub type PreservesMetadataDigest = Digest<PreservesMetadataRole>;
pub type PreservesMetadataDigestRef<'a> = DigestRef<'a, PreservesMetadataRole>;
pub type CompatibilityDigest = Digest<CompatibilityReferenceRole>;
pub type CompatibilityDigestRef<'a> = DigestRef<'a, CompatibilityReferenceRole>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HexCase {
    LowercaseOnly,
    Insensitive,
}

pub(super) fn parse_lower_parts<'a>(
    value: &'a str,
    profile: DigestProfile,
    field: &'static str,
) -> Result<(DigestSpelling, &'a str), SharedIdentityIssue> {
    parse_parts(value, profile, field, HexCase::LowercaseOnly)
}

fn parse_parts<'a>(
    value: &'a str,
    profile: DigestProfile,
    field: &'static str,
    hex_case: HexCase,
) -> Result<(DigestSpelling, &'a str), SharedIdentityIssue> {
    let (spelling, hex) = if let Some(hex) = value.strip_prefix(B3_PREFIX) {
        (DigestSpelling::B3Prefix, hex)
    } else if let Some(hex) = value.strip_prefix(BLAKE3_PREFIX) {
        (DigestSpelling::Blake3Prefix, hex)
    } else {
        (DigestSpelling::RawHex, value)
    };

    if !profile_allows_spelling(profile, spelling) {
        return Err(SharedIdentityIssue::new(field, "unsupported_digest_spelling"));
    }
    if hex.len() != super::BLAKE3_HEX_LENGTH || !hex.chars().all(|character| valid_hex(character, hex_case)) {
        return Err(SharedIdentityIssue::new(field, "malformed_blake3"));
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

fn valid_hex(character: char, hex_case: HexCase) -> bool {
    match hex_case {
        HexCase::LowercaseOnly => character.is_ascii_digit() || ('a'..='f').contains(&character),
        HexCase::Insensitive => character.is_ascii_hexdigit(),
    }
}

fn exact_spelling_profile(spelling: DigestSpelling) -> DigestProfile {
    DigestProfile {
        name: "wire-exact-spelling",
        allow_raw_hex: spelling == DigestSpelling::RawHex,
        allow_b3_prefix: spelling == DigestSpelling::B3Prefix,
        allow_blake3_prefix: spelling == DigestSpelling::Blake3Prefix,
    }
}
