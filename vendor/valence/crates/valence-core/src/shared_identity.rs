//! Shared pure evidence-identity primitives for stack-owned BLAKE3, canonical
//! content hashes, self-hash exclusion, digest roles, and non-claim boundaries.
//!
//! The shared identity core validates already-loaded strings, bytes, and JSON
//! values only. It does not read files, inspect repositories, execute tools,
//! contact networks, decide release eligibility, or claim evidence truth.
//!
//! r[impl valence.evidence_ir.shared_identity.contract]
//! r[impl valence.evidence_ir.shared_identity.validation]
//! r[impl valence.evidence_ir.shared_identity.compatibility_adapters]

use std::marker::PhantomData;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;

/// Number of lowercase hexadecimal characters in a BLAKE3 digest.
const BLAKE3_HEX_LENGTH: usize = 64;

const B3_PREFIX: &str = "b3:";
const BLAKE3_PREFIX: &str = "blake3:";

const FIELD_CANONICAL_JSON: &str = "canonical_json";
const FIELD_DIGEST: &str = "digest";
const FIELD_EXPECTED_ROLE: &str = "expected_role";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_ROLE: &str = "role";
const FIELD_SELF_HASH_EXCLUSION: &str = "self_hash_exclusion";

/// Required visible non-claim for shared identity reports.
pub const SHARED_IDENTITY_REQUIRED_NON_CLAIM: &str = "shared identity proves only syntax, canonical content identity, role separation, and non-claim boundary conformance for the selected profile";

const SHARED_IDENTITY_SUPPORTED_CLAIM: &str = "shared identity validation proves only syntax, canonical content identity, role separation, and non-claim boundary conformance for the selected profile";

const OVERCLAIM_MARKERS: &[&str] = &[
    "proves evidence truth",
    "proves behavioral correctness",
    "proves semantic equivalence",
    "proves verifier soundness",
    "proves release eligibility",
    "proves lifecycle readiness",
    "proves whole-program safety",
];

const RECEIPT_SELF_HASH_FIELDS: &[&str] = &["receipt_hash", "preserves_content_hash"];
const ARTIFACT_SELF_HASH_FIELDS: &[&str] = &["artifact_hash", "canonical_hash"];
const BUNDLE_SELF_HASH_FIELDS: &[&str] = &["bundle_hash", "preserves_content_hash"];

/// Role that makes equal BLAKE3 bytes non-interchangeable across evidence surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigestRole {
    BinaryBytes,
    SidecarBytes,
    ArtifactIdentity,
    ReceiptIdentity,
    PolicyIdentity,
    GraphIdentity,
    SourceIdentity,
    PreservesMetadata,
    CompatibilityReference,
}

impl DigestRole {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BinaryBytes => "binary_bytes",
            Self::SidecarBytes => "sidecar_bytes",
            Self::ArtifactIdentity => "artifact_identity",
            Self::ReceiptIdentity => "receipt_identity",
            Self::PolicyIdentity => "policy_identity",
            Self::GraphIdentity => "graph_identity",
            Self::SourceIdentity => "source_identity",
            Self::PreservesMetadata => "preserves_metadata",
            Self::CompatibilityReference => "compatibility_reference",
        }
    }
}

/// Accepted digest spelling for a parsed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigestSpelling {
    RawHex,
    B3Prefix,
    Blake3Prefix,
}

/// Profile-scoped spelling policy for public compatibility fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DigestProfile {
    pub name: &'static str,
    pub allow_raw_hex: bool,
    pub allow_b3_prefix: bool,
    pub allow_blake3_prefix: bool,
}

/// Strict new stack-owned display profile: require `b3:`.
pub const STRICT_B3_PROFILE: DigestProfile = DigestProfile {
    name: "strict-b3",
    allow_raw_hex: false,
    allow_b3_prefix: true,
    allow_blake3_prefix: false,
};

/// Legacy raw BLAKE3 profile for surfaces that already expose bare hex.
pub const RAW_BLAKE3_PROFILE: DigestProfile = DigestProfile {
    name: "raw-blake3",
    allow_raw_hex: true,
    allow_b3_prefix: false,
    allow_blake3_prefix: false,
};

/// Compatibility profile for surfaces that already accept multiple BLAKE3 spellings.
pub const MIXED_BLAKE3_PROFILE: DigestProfile = DigestProfile {
    name: "mixed-blake3",
    allow_raw_hex: true,
    allow_b3_prefix: true,
    allow_blake3_prefix: true,
};

#[path = "shared_identity/typed_digest.rs"]
mod typed_digest;
pub use typed_digest::AdmittedBinaryDigest;
pub use typed_digest::AdmittedDigest;
pub use typed_digest::ArtifactIdentityDigest;
pub use typed_digest::ArtifactIdentityDigestRef;
pub use typed_digest::ArtifactIdentityRole;
pub use typed_digest::BinaryDigest;
pub use typed_digest::BinaryDigestRef;
pub use typed_digest::BinaryRole;
pub use typed_digest::CompatibilityDigest;
pub use typed_digest::CompatibilityDigestRef;
pub use typed_digest::CompatibilityReferenceRole;
pub use typed_digest::Digest;
pub use typed_digest::DigestRef;
pub use typed_digest::DigestRoleMarker;
pub use typed_digest::GraphDigest;
pub use typed_digest::GraphDigestRef;
pub use typed_digest::GraphRole;
pub use typed_digest::PolicyDigest;
pub use typed_digest::PolicyDigestRef;
pub use typed_digest::PolicyRole;
pub use typed_digest::PreservesMetadataDigest;
pub use typed_digest::PreservesMetadataDigestRef;
pub use typed_digest::PreservesMetadataRole;
pub use typed_digest::ReceiptDigest;
pub use typed_digest::ReceiptDigestRef;
pub use typed_digest::ReceiptRole;
pub use typed_digest::SidecarDigest;
pub use typed_digest::SidecarDigestRef;
pub use typed_digest::SidecarRole;
pub use typed_digest::SourceDigest;
pub use typed_digest::SourceDigestRef;
pub use typed_digest::SourceRole;

#[path = "shared_identity/canonical_hash_profile.rs"]
mod canonical_hash_profile;
pub use canonical_hash_profile::CanonicalHashProfile;

/// Named field-exclusion profile for receipt-shaped self-referential hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SelfHashExclusionProfile {
    pub name: &'static str,
    pub excluded_fields: &'static [&'static str],
}

pub const RECEIPT_SELF_HASH_EXCLUSION: SelfHashExclusionProfile = SelfHashExclusionProfile {
    name: "receipt-self-hash",
    excluded_fields: RECEIPT_SELF_HASH_FIELDS,
};

pub const ARTIFACT_SELF_HASH_EXCLUSION: SelfHashExclusionProfile = SelfHashExclusionProfile {
    name: "artifact-self-hash",
    excluded_fields: ARTIFACT_SELF_HASH_FIELDS,
};

pub const BUNDLE_SELF_HASH_EXCLUSION: SelfHashExclusionProfile = SelfHashExclusionProfile {
    name: "bundle-self-hash",
    excluded_fields: BUNDLE_SELF_HASH_FIELDS,
};

/// Closed wire DTO preserving the public spelling and declared role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedDigest {
    pub role: DigestRole,
    pub original: String,
    pub normalized_hex: String,
    pub spelling: DigestSpelling,
}

/// Explicit name for the compatibility wire DTO.
pub type SharedDigestWire = SharedDigest;

/// Closed admitted representation for heterogeneous digest wire records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnyDigest {
    Binary(AdmittedDigest<BinaryRole>),
    Sidecar(AdmittedDigest<SidecarRole>),
    Artifact(AdmittedDigest<ArtifactIdentityRole>),
    Receipt(AdmittedDigest<ReceiptRole>),
    Policy(AdmittedDigest<PolicyRole>),
    Graph(AdmittedDigest<GraphRole>),
    Source(AdmittedDigest<SourceRole>),
    PreservesMetadata(AdmittedDigest<PreservesMetadataRole>),
    CompatibilityReference(AdmittedDigest<CompatibilityReferenceRole>),
}

impl AnyDigest {
    #[must_use]
    pub fn role(&self) -> DigestRole {
        match self {
            Self::Binary(_) => DigestRole::BinaryBytes,
            Self::Sidecar(_) => DigestRole::SidecarBytes,
            Self::Artifact(_) => DigestRole::ArtifactIdentity,
            Self::Receipt(_) => DigestRole::ReceiptIdentity,
            Self::Policy(_) => DigestRole::PolicyIdentity,
            Self::Graph(_) => DigestRole::GraphIdentity,
            Self::Source(_) => DigestRole::SourceIdentity,
            Self::PreservesMetadata(_) => DigestRole::PreservesMetadata,
            Self::CompatibilityReference(_) => DigestRole::CompatibilityReference,
        }
    }

    #[must_use]
    pub fn normalized_hex(&self) -> &str {
        match self {
            Self::Binary(value) => value.digest().as_hex(),
            Self::Sidecar(value) => value.digest().as_hex(),
            Self::Artifact(value) => value.digest().as_hex(),
            Self::Receipt(value) => value.digest().as_hex(),
            Self::Policy(value) => value.digest().as_hex(),
            Self::Graph(value) => value.digest().as_hex(),
            Self::Source(value) => value.digest().as_hex(),
            Self::PreservesMetadata(value) => value.digest().as_hex(),
            Self::CompatibilityReference(value) => value.digest().as_hex(),
        }
    }

    pub fn try_into_role<Role: DigestRoleMarker>(self) -> Result<Digest<Role>, SharedIdentityIssue> {
        if self.role() != Role::ROLE {
            return Err(SharedIdentityIssue::new(FIELD_ROLE, "role_mismatch"));
        }
        Digest::<Role>::parse(self.normalized_hex(), RAW_BLAKE3_PROFILE, FIELD_DIGEST)
    }
}

impl TryFrom<SharedDigest> for AnyDigest {
    type Error = SharedIdentityIssue;

    fn try_from(wire: SharedDigest) -> Result<Self, Self::Error> {
        let role = wire.role;
        let original = wire.original;
        let normalized_hex = wire.normalized_hex;
        let spelling = wire.spelling;
        match role {
            DigestRole::BinaryBytes => {
                AdmittedDigest::<BinaryRole>::from_wire(original, normalized_hex, spelling, FIELD_DIGEST)
                    .map(Self::Binary)
            }
            DigestRole::SidecarBytes => {
                AdmittedDigest::<SidecarRole>::from_wire(original, normalized_hex, spelling, FIELD_DIGEST)
                    .map(Self::Sidecar)
            }
            DigestRole::ArtifactIdentity => {
                AdmittedDigest::<ArtifactIdentityRole>::from_wire(original, normalized_hex, spelling, FIELD_DIGEST)
                    .map(Self::Artifact)
            }
            DigestRole::ReceiptIdentity => {
                AdmittedDigest::<ReceiptRole>::from_wire(original, normalized_hex, spelling, FIELD_DIGEST)
                    .map(Self::Receipt)
            }
            DigestRole::PolicyIdentity => {
                AdmittedDigest::<PolicyRole>::from_wire(original, normalized_hex, spelling, FIELD_DIGEST)
                    .map(Self::Policy)
            }
            DigestRole::GraphIdentity => {
                AdmittedDigest::<GraphRole>::from_wire(original, normalized_hex, spelling, FIELD_DIGEST)
                    .map(Self::Graph)
            }
            DigestRole::SourceIdentity => {
                AdmittedDigest::<SourceRole>::from_wire(original, normalized_hex, spelling, FIELD_DIGEST)
                    .map(Self::Source)
            }
            DigestRole::PreservesMetadata => {
                AdmittedDigest::<PreservesMetadataRole>::from_wire(original, normalized_hex, spelling, FIELD_DIGEST)
                    .map(Self::PreservesMetadata)
            }
            DigestRole::CompatibilityReference => AdmittedDigest::<CompatibilityReferenceRole>::from_wire(
                original,
                normalized_hex,
                spelling,
                FIELD_DIGEST,
            )
            .map(Self::CompatibilityReference),
        }
    }
}

impl From<AnyDigest> for SharedDigest {
    fn from(digest: AnyDigest) -> Self {
        let role = digest.role();
        let (original, normalized_hex, spelling) = match digest {
            AnyDigest::Binary(value) => value.into_wire_parts(),
            AnyDigest::Sidecar(value) => value.into_wire_parts(),
            AnyDigest::Artifact(value) => value.into_wire_parts(),
            AnyDigest::Receipt(value) => value.into_wire_parts(),
            AnyDigest::Policy(value) => value.into_wire_parts(),
            AnyDigest::Graph(value) => value.into_wire_parts(),
            AnyDigest::Source(value) => value.into_wire_parts(),
            AnyDigest::PreservesMetadata(value) => value.into_wire_parts(),
            AnyDigest::CompatibilityReference(value) => value.into_wire_parts(),
        };
        Self {
            role,
            original,
            normalized_hex,
            spelling,
        }
    }
}

/// Deterministic field-scoped issue emitted by shared identity validation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SharedIdentityIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl SharedIdentityIssue {
    #[must_use]
    pub const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

/// Pure typed validation candidate for a statically known digest role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedDigestCandidate<Role: DigestRoleMarker> {
    pub digest: String,
    pub profile: DigestProfile,
    pub non_claims: Vec<String>,
    role: PhantomData<fn() -> Role>,
}

impl<Role: DigestRoleMarker> TypedDigestCandidate<Role> {
    #[must_use]
    pub fn new(digest: String, profile: DigestProfile, non_claims: Vec<String>) -> Self {
        Self {
            digest,
            profile,
            non_claims,
            role: PhantomData,
        }
    }
}

/// Validation report whose admitted digest retains its compile-time role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedIdentityReport<Role: DigestRoleMarker> {
    pub valid: bool,
    pub digest: Option<Digest<Role>>,
    pub issues: Vec<SharedIdentityIssue>,
    pub supported_claim: String,
    pub non_claims: Vec<String>,
}

/// Pure validation candidate for a compatibility wire digest field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedDigestCandidate {
    pub role: DigestRole,
    pub expected_role: Option<DigestRole>,
    pub digest: String,
    pub profile: DigestProfile,
    pub non_claims: Vec<String>,
}

/// Validation report for shared digest and non-claim checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SharedIdentityReport {
    pub valid: bool,
    pub digest: Option<SharedDigest>,
    pub issues: Vec<SharedIdentityIssue>,
    pub supported_claim: String,
    pub non_claims: Vec<String>,
}

/// Validate a statically known digest role without a runtime expected-role argument.
#[must_use]
pub fn validate_typed_digest<Role: DigestRoleMarker>(
    candidate: &TypedDigestCandidate<Role>,
) -> TypedIdentityReport<Role> {
    let mut issues = Vec::new();
    let digest = match Digest::<Role>::parse(&candidate.digest, candidate.profile, FIELD_DIGEST) {
        Ok(digest) => Some(digest),
        Err(issue) => {
            issues.push(issue);
            None
        }
    };
    issues.extend(validate_non_claims(&candidate.non_claims));
    issues.sort();
    issues.dedup();
    TypedIdentityReport {
        valid: issues.is_empty(),
        digest,
        issues,
        supported_claim: SHARED_IDENTITY_SUPPORTED_CLAIM.to_owned(),
        non_claims: candidate.non_claims.clone(),
    }
}

/// Validate a compatibility wire digest candidate without I/O.
#[must_use]
pub fn validate_shared_digest(candidate: &SharedDigestCandidate) -> SharedIdentityReport {
    let mut issues = Vec::new();
    if let Some(expected_role) = candidate.expected_role
        && expected_role != candidate.role
    {
        issues.push(SharedIdentityIssue::new(FIELD_EXPECTED_ROLE, "role_mismatch"));
    }
    let digest = match parse_digest(&candidate.digest, candidate.role, candidate.profile, FIELD_DIGEST) {
        Ok(digest) => Some(digest),
        Err(issue) => {
            issues.push(issue);
            None
        }
    };
    issues.extend(validate_non_claims(&candidate.non_claims));
    finish_report(digest, candidate.non_claims.clone(), issues)
}

/// Parse a profile-scoped BLAKE3 digest while preserving the declared role.
pub fn parse_digest(
    value: &str,
    role: DigestRole,
    profile: DigestProfile,
    field: &'static str,
) -> Result<SharedDigest, SharedIdentityIssue> {
    let (spelling, hex) = typed_digest::parse_lower_parts(value, profile, field)?;
    Ok(SharedDigest {
        role,
        original: value.to_owned(),
        normalized_hex: hex.to_owned(),
        spelling,
    })
}

/// Encode JSON with recursively sorted object keys independently of `serde_json` map features.
///
/// r[impl valence.evidence_ir.complete_envelope.canonical_hash]
pub fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, SharedIdentityIssue> {
    let mut bytes = Vec::new();
    write_canonical_json(value, &mut bytes)?;
    Ok(bytes)
}

/// Compute a BLAKE3 identity over explicit canonical JSON material.
pub fn canonical_json_hash(value: &Value) -> Result<String, SharedIdentityIssue> {
    canonical_json_bytes(value).map(|bytes| blake3_hex(&bytes))
}

/// Compute a BLAKE3 identity over explicit canonical bytes for the selected profile.
#[must_use]
pub fn canonical_bytes_hash(profile: CanonicalHashProfile, bytes: &[u8]) -> String {
    match profile {
        CanonicalHashProfile::JsonValue | CanonicalHashProfile::PreservesText => blake3_hex(bytes),
    }
}

/// Compute JSON hash material after applying a named self-hash exclusion profile.
pub fn canonical_json_hash_with_exclusion(
    value: &Value,
    profile: Option<SelfHashExclusionProfile>,
) -> Result<String, SharedIdentityIssue> {
    let Some(profile) = profile else {
        return Err(SharedIdentityIssue::new(FIELD_SELF_HASH_EXCLUSION, "missing_self_hash_exclusion_profile"));
    };
    if profile.excluded_fields.is_empty() {
        return Err(SharedIdentityIssue::new(FIELD_SELF_HASH_EXCLUSION, "unknown_self_hash_exclusion_profile"));
    }
    let stripped = strip_excluded_fields(value, profile.excluded_fields);
    canonical_json_hash(&stripped)
}

/// Validate reusable non-claim boundary text.
#[must_use]
pub fn validate_non_claims(non_claims: &[String]) -> Vec<SharedIdentityIssue> {
    let mut issues = Vec::new();
    if non_claims.is_empty() {
        issues.push(SharedIdentityIssue::new(FIELD_NON_CLAIMS, "non_claims_missing"));
        return issues;
    }
    if !non_claims.iter().any(|claim| claim.contains(SHARED_IDENTITY_REQUIRED_NON_CLAIM)) {
        issues.push(SharedIdentityIssue::new(FIELD_NON_CLAIMS, "non_claims_weakened"));
    }
    if non_claims.iter().any(|claim| contains_overclaim(claim)) {
        issues.push(SharedIdentityIssue::new(FIELD_NON_CLAIMS, "overclaim_detected"));
    }
    issues
}

/// Render the bounded supported claim for docs and reports.
#[must_use]
pub fn shared_identity_supported_claim() -> &'static str {
    SHARED_IDENTITY_SUPPORTED_CLAIM
}

/// Compute lower-case BLAKE3 hex for bytes.
#[must_use]
pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Check whether text contains a known overclaim marker.
#[must_use]
pub fn contains_overclaim(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    OVERCLAIM_MARKERS.iter().any(|marker| lower.contains(marker))
}

fn finish_report(
    digest: Option<SharedDigest>,
    non_claims: Vec<String>,
    mut issues: Vec<SharedIdentityIssue>,
) -> SharedIdentityReport {
    issues.sort();
    issues.dedup();
    SharedIdentityReport {
        valid: issues.is_empty(),
        digest,
        issues,
        supported_claim: SHARED_IDENTITY_SUPPORTED_CLAIM.to_owned(),
        non_claims,
    }
}

fn write_canonical_json(value: &Value, bytes: &mut Vec<u8>) -> Result<(), SharedIdentityIssue> {
    const ARRAY_OPEN: u8 = b'[';
    const ARRAY_CLOSE: u8 = b']';
    const OBJECT_OPEN: u8 = b'{';
    const OBJECT_CLOSE: u8 = b'}';
    const ITEM_SEPARATOR: u8 = b',';
    const FIELD_SEPARATOR: u8 = b':';

    match value {
        Value::Array(items) => {
            bytes.push(ARRAY_OPEN);
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    bytes.push(ITEM_SEPARATOR);
                }
                write_canonical_json(item, bytes)?;
            }
            bytes.push(ARRAY_CLOSE);
        }
        Value::Object(entries) => {
            bytes.push(OBJECT_OPEN);
            let mut sorted = entries.iter().collect::<Vec<_>>();
            sorted.sort_by(|left, right| left.0.cmp(right.0));
            for (index, (key, item)) in sorted.into_iter().enumerate() {
                if index > 0 {
                    bytes.push(ITEM_SEPARATOR);
                }
                let key_bytes = serde_json::to_vec(key)
                    .map_err(|_| SharedIdentityIssue::new(FIELD_CANONICAL_JSON, "serialization_failed"))?;
                bytes.extend_from_slice(&key_bytes);
                bytes.push(FIELD_SEPARATOR);
                write_canonical_json(item, bytes)?;
            }
            bytes.push(OBJECT_CLOSE);
        }
        scalar => {
            let scalar_bytes = serde_json::to_vec(scalar)
                .map_err(|_| SharedIdentityIssue::new(FIELD_CANONICAL_JSON, "serialization_failed"))?;
            bytes.extend_from_slice(&scalar_bytes);
        }
    }
    Ok(())
}

fn strip_excluded_fields(value: &Value, excluded_fields: &[&str]) -> Value {
    match value {
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| strip_excluded_fields(item, excluded_fields)).collect())
        }
        Value::Object(entries) => {
            let mut sorted = entries.iter().collect::<Vec<_>>();
            sorted.sort_by(|left, right| left.0.cmp(right.0));
            let map = sorted
                .into_iter()
                .filter_map(|(key, item)| {
                    if excluded_fields.contains(&key.as_str()) {
                        None
                    } else {
                        Some((key.clone(), strip_excluded_fields(item, excluded_fields)))
                    }
                })
                .collect::<Map<String, Value>>();
            Value::Object(map)
        }
        scalar => scalar.clone(),
    }
}

#[cfg(test)]
#[path = "shared_identity/tests.rs"]
mod tests;
