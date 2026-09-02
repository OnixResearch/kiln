//! Pure artifact reference contracts for Valence-owned evidence identity plumbing.
//!
//! The artifact-ref core validates already-loaded metadata only. It does not read
//! files, resolve repository paths, measure bytes, fetch network content, mutate
//! stores, decide whether an artifact is true evidence, or claim release
//! readiness.
//!
//! r[impl valence.evidence_ir.artifact_ref_core.contract]
//! r[impl valence.evidence_ir.artifact_ref_core.roles]
//! r[impl valence.evidence_ir.artifact_ref_core.validation]
//! r[impl valence.evidence_ir.artifact_ref_core.adapters]

use serde::Deserialize;
use serde::Serialize;

use crate::shared_identity::ArtifactIdentityDigest;
use crate::shared_identity::DigestProfile as SharedDigestProfile;

const B3_PREFIX: &str = "b3:";
const BLAKE3_PREFIX: &str = "blake3:";
const CURRENT_DIRECTORY_SEGMENT: &str = ".";
const PARENT_DIRECTORY_SEGMENT: &str = "..";
const WINDOWS_DRIVE_SEPARATOR: char = ':';
const WINDOWS_SEPARATOR: char = '\\';
const UNIX_SEPARATOR: char = '/';

/// Required visible non-claim boundary for artifact reference reports.
pub const ARTIFACT_REF_REQUIRED_NON_CLAIM: &str = "artifact ref validation proves only artifact ref syntax, role separation, metadata confinement, and declared hash/size consistency";

const ARTIFACT_REF_OVERCLAIM_MARKERS: &[&str] = &[
    "proves artifact truth",
    "proves behavioral correctness",
    "proves semantic equivalence",
    "proves verifier soundness",
    "proves release eligibility",
    "proves lifecycle readiness",
    "grants runtime authority",
    "proves whole-program safety",
];

const FIELD_DIGEST: &str = "digest";
const FIELD_EXPECTED_ROLE: &str = "expected_role";
const FIELD_LABEL: &str = "label";
const FIELD_MEASURED_DIGEST: &str = "measured_digest";
const FIELD_METADATA: &str = "metadata";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_PATH: &str = "path";
const FIELD_PUBLIC_FIELD: &str = "public_field";
const FIELD_SIZE: &str = "size_bytes";

/// Role-specific artifact reference wrappers keep equal digest bytes from erasing evidence roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRole {
    BinaryArtifact,
    Sidecar,
    Receipt,
    SourceArtifact,
    ReleaseBundleMember,
    CompatibilityRef,
}

impl ArtifactRole {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BinaryArtifact => "binary_artifact",
            Self::Sidecar => "sidecar",
            Self::Receipt => "receipt",
            Self::SourceArtifact => "source_artifact",
            Self::ReleaseBundleMember => "release_bundle_member",
            Self::CompatibilityRef => "compatibility_ref",
        }
    }
}

/// Digest spellings accepted by a compatibility profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigestSpelling {
    RawHex,
    B3Prefix,
    Blake3Prefix,
}

impl DigestSpelling {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RawHex => "raw_hex",
            Self::B3Prefix => "b3_prefix",
            Self::Blake3Prefix => "blake3_prefix",
        }
    }
}

/// Profile-scoped spelling policy for existing public fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DigestSpellingProfile {
    pub name: &'static str,
    pub allow_raw_hex: bool,
    pub allow_b3_prefix: bool,
    pub allow_blake3_prefix: bool,
}

/// Strict new Valence-owned profile: require `b3:` for displayed artifact refs.
pub const STRICT_B3_PROFILE: DigestSpellingProfile = DigestSpellingProfile {
    name: "strict-b3",
    allow_raw_hex: false,
    allow_b3_prefix: true,
    allow_blake3_prefix: false,
};

/// Legacy raw BLAKE3 profile for already-public fields that omit a prefix.
pub const RAW_BLAKE3_PROFILE: DigestSpellingProfile = DigestSpellingProfile {
    name: "raw-blake3",
    allow_raw_hex: true,
    allow_b3_prefix: false,
    allow_blake3_prefix: false,
};

/// Compatibility profile for stack surfaces that already expose mixed BLAKE3 spellings.
pub const MIXED_BLAKE3_PROFILE: DigestSpellingProfile = DigestSpellingProfile {
    name: "mixed-blake3",
    allow_raw_hex: true,
    allow_b3_prefix: true,
    allow_blake3_prefix: true,
};

/// Parsed digest with both public spelling and normalized BLAKE3 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDigest {
    pub original: String,
    pub normalized_hex: String,
    pub spelling: DigestSpelling,
}

/// Validated role-specific artifact reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub role: ArtifactRole,
    pub digest: ArtifactDigest,
    pub path: Option<String>,
    pub label: Option<String>,
    pub size_bytes: Option<u64>,
}

/// Pure in-memory candidate for artifact reference validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRefCandidate {
    pub role: ArtifactRole,
    pub expected_role: Option<ArtifactRole>,
    pub digest: String,
    pub digest_profile: DigestSpellingProfile,
    pub path: Option<String>,
    pub label: Option<String>,
    pub size_bytes: Option<u64>,
    pub measured_digest: Option<String>,
    pub measured_size_bytes: Option<u64>,
    pub non_claims: Vec<String>,
}

/// Deterministic field-scoped issue emitted by artifact reference validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactRefIssue {
    pub field: &'static str,
    pub code: &'static str,
}

impl ArtifactRefIssue {
    #[must_use]
    pub const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

/// Validation report for one candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactRefValidationReport {
    pub valid: bool,
    pub issues: Vec<ArtifactRefIssue>,
    pub artifact: Option<ArtifactRef>,
    pub supported_claim: String,
    pub non_claims: Vec<String>,
}

/// Adapter input for current public fields that cannot be renamed yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicArtifactFieldCandidate {
    pub public_field: String,
    pub candidate: ArtifactRefCandidate,
}

/// Adapter report preserving the public field name alongside the core report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicArtifactFieldReport {
    pub public_field: String,
    pub report: ArtifactRefValidationReport,
}

/// Validate an artifact reference candidate without I/O or byte measurement.
#[must_use]
pub fn validate_artifact_ref(candidate: &ArtifactRefCandidate) -> ArtifactRefValidationReport {
    let mut issues = Vec::new();

    if let Some(expected_role) = candidate.expected_role
        && expected_role != candidate.role
    {
        issues.push(ArtifactRefIssue::new(FIELD_EXPECTED_ROLE, "role_mismatch"));
    }

    let digest = parse_digest(&candidate.digest, candidate.digest_profile, FIELD_DIGEST, &mut issues);
    validate_path(candidate.path.as_deref(), &mut issues);
    validate_label(candidate.label.as_deref(), &mut issues);
    validate_metadata_pairing(candidate, &mut issues);
    validate_measured_digest(candidate, digest.as_ref(), &mut issues);
    validate_measured_size(candidate, &mut issues);
    validate_non_claims(&candidate.non_claims, &mut issues);

    sort_and_dedup_issues(&mut issues);
    let valid = issues.is_empty();
    let artifact = if valid {
        digest.map(|parsed_digest| ArtifactRef {
            role: candidate.role,
            digest: parsed_digest,
            path: candidate.path.clone(),
            label: candidate.label.clone(),
            size_bytes: candidate.size_bytes,
        })
    } else {
        None
    };

    ArtifactRefValidationReport {
        valid,
        issues,
        artifact,
        supported_claim: ARTIFACT_REF_REQUIRED_NON_CLAIM.to_owned(),
        non_claims: candidate.non_claims.clone(),
    }
}

/// Validate a currently public artifact field while preserving its field name in the report.
#[must_use]
pub fn validate_public_artifact_field(candidate: &PublicArtifactFieldCandidate) -> PublicArtifactFieldReport {
    let mut report = validate_artifact_ref(&candidate.candidate);
    if candidate.public_field.trim().is_empty() {
        report.issues.push(ArtifactRefIssue::new(FIELD_PUBLIC_FIELD, "public_field_empty"));
        sort_and_dedup_issues(&mut report.issues);
        report.valid = false;
        report.artifact = None;
    }

    PublicArtifactFieldReport {
        public_field: candidate.public_field.clone(),
        report,
    }
}

fn parse_digest(
    value: &str,
    profile: DigestSpellingProfile,
    field: &'static str,
    issues: &mut Vec<ArtifactRefIssue>,
) -> Option<ArtifactDigest> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed != value {
        issues.push(ArtifactRefIssue::new(field, "digest_malformed"));
        return None;
    }

    let (spelling, _hex) = if let Some(stripped) = trimmed.strip_prefix(B3_PREFIX) {
        (DigestSpelling::B3Prefix, stripped)
    } else if let Some(stripped) = trimmed.strip_prefix(BLAKE3_PREFIX) {
        (DigestSpelling::Blake3Prefix, stripped)
    } else if trimmed.contains(WINDOWS_DRIVE_SEPARATOR) {
        issues.push(ArtifactRefIssue::new(field, "digest_unsupported_prefix"));
        return None;
    } else {
        (DigestSpelling::RawHex, trimmed)
    };

    if !profile_allows_spelling(profile, spelling) {
        issues.push(ArtifactRefIssue::new(field, "digest_unsupported_prefix"));
        return None;
    }
    let typed = match ArtifactIdentityDigest::parse_compatible(trimmed, shared_digest_profile(profile), field) {
        Ok(digest) => digest,
        Err(_) => {
            issues.push(ArtifactRefIssue::new(field, "digest_malformed"));
            return None;
        }
    };

    Some(ArtifactDigest {
        original: trimmed.to_owned(),
        normalized_hex: typed.as_hex().to_owned(),
        spelling,
    })
}

fn profile_allows_spelling(profile: DigestSpellingProfile, spelling: DigestSpelling) -> bool {
    match spelling {
        DigestSpelling::RawHex => profile.allow_raw_hex,
        DigestSpelling::B3Prefix => profile.allow_b3_prefix,
        DigestSpelling::Blake3Prefix => profile.allow_blake3_prefix,
    }
}

fn shared_digest_profile(profile: DigestSpellingProfile) -> SharedDigestProfile {
    SharedDigestProfile {
        name: profile.name,
        allow_raw_hex: profile.allow_raw_hex,
        allow_b3_prefix: profile.allow_b3_prefix,
        allow_blake3_prefix: profile.allow_blake3_prefix,
    }
}

fn validate_path(path: Option<&str>, issues: &mut Vec<ArtifactRefIssue>) {
    let Some(path) = path else {
        return;
    };
    if path.trim().is_empty() || path.trim() != path {
        issues.push(ArtifactRefIssue::new(FIELD_PATH, "path_empty_or_padded"));
        return;
    }
    if path.starts_with(UNIX_SEPARATOR) || path.starts_with(WINDOWS_SEPARATOR) || has_windows_drive_prefix(path) {
        issues.push(ArtifactRefIssue::new(FIELD_PATH, "path_not_relative"));
    }
    if path_segments(path).any(|segment| segment == PARENT_DIRECTORY_SEGMENT) {
        issues.push(ArtifactRefIssue::new(FIELD_PATH, "path_escape"));
    }
    if path_segments(path).any(|segment| segment.is_empty() || segment == CURRENT_DIRECTORY_SEGMENT) {
        issues.push(ArtifactRefIssue::new(FIELD_PATH, "path_unstable_segment"));
    }
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let mut chars = path.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let Some(second) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic() && second == WINDOWS_DRIVE_SEPARATOR
}

fn path_segments(path: &str) -> impl Iterator<Item = &str> {
    path.split([UNIX_SEPARATOR, WINDOWS_SEPARATOR])
}

fn validate_label(label: Option<&str>, issues: &mut Vec<ArtifactRefIssue>) {
    let Some(label) = label else {
        return;
    };
    if label.trim().is_empty() || label.trim() != label {
        issues.push(ArtifactRefIssue::new(FIELD_LABEL, "label_empty_or_padded"));
        return;
    }
    if label.contains(UNIX_SEPARATOR) || label.contains(WINDOWS_SEPARATOR) || label.contains(PARENT_DIRECTORY_SEGMENT) {
        issues.push(ArtifactRefIssue::new(FIELD_LABEL, "label_escape"));
    }
}

fn validate_metadata_pairing(candidate: &ArtifactRefCandidate, issues: &mut Vec<ArtifactRefIssue>) {
    let has_measured_digest = candidate.measured_digest.is_some();
    let has_measured_size = candidate.measured_size_bytes.is_some();
    if has_measured_digest != has_measured_size {
        issues.push(ArtifactRefIssue::new(FIELD_METADATA, "hash_size_metadata_incomplete"));
    }
    if candidate.measured_size_bytes.is_some() && candidate.size_bytes.is_none() {
        issues.push(ArtifactRefIssue::new(FIELD_SIZE, "declared_size_missing"));
    }
}

fn validate_measured_digest(
    candidate: &ArtifactRefCandidate,
    declared_digest: Option<&ArtifactDigest>,
    issues: &mut Vec<ArtifactRefIssue>,
) {
    let Some(measured) = candidate.measured_digest.as_deref() else {
        return;
    };
    let measured_digest = parse_digest(measured, MIXED_BLAKE3_PROFILE, FIELD_MEASURED_DIGEST, issues);
    if let (Some(declared), Some(measured)) = (declared_digest, measured_digest)
        && declared.normalized_hex != measured.normalized_hex
    {
        issues.push(ArtifactRefIssue::new(FIELD_MEASURED_DIGEST, "hash_metadata_mismatch"));
    }
}

fn validate_measured_size(candidate: &ArtifactRefCandidate, issues: &mut Vec<ArtifactRefIssue>) {
    match (candidate.size_bytes, candidate.measured_size_bytes) {
        (Some(declared), Some(measured)) if declared != measured => {
            issues.push(ArtifactRefIssue::new(FIELD_SIZE, "size_metadata_mismatch"));
        }
        _ => {}
    }
}

fn validate_non_claims(non_claims: &[String], issues: &mut Vec<ArtifactRefIssue>) {
    if non_claims.is_empty() {
        issues.push(ArtifactRefIssue::new(FIELD_NON_CLAIMS, "non_claims_missing"));
        return;
    }
    let joined = non_claims.join(" ");
    if !joined.contains(ARTIFACT_REF_REQUIRED_NON_CLAIM) {
        issues.push(ArtifactRefIssue::new(FIELD_NON_CLAIMS, "non_claims_weakened"));
    }
    if ARTIFACT_REF_OVERCLAIM_MARKERS.iter().any(|marker| joined.contains(marker)) {
        issues.push(ArtifactRefIssue::new(FIELD_NON_CLAIMS, "overclaim_detected"));
    }
}

fn sort_and_dedup_issues(issues: &mut Vec<ArtifactRefIssue>) {
    issues.sort_by(|left, right| left.field.cmp(right.field).then_with(|| left.code.cmp(right.code)));
    issues.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST_A_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const DIGEST_A_B3: &str = "b3:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const DIGEST_A_BLAKE3: &str = "blake3:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const DIGEST_B_B3: &str = "b3:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
    const EXPECTED_FIXTURE_SIZE_BYTES: u64 = 4096;
    const STALE_FIXTURE_SIZE_BYTES: u64 = 8192;

    fn non_claims() -> Vec<String> {
        vec![
            ARTIFACT_REF_REQUIRED_NON_CLAIM.to_owned(),
            "artifact refs are identity metadata, not evidence truth".to_owned(),
        ]
    }

    fn candidate(role: ArtifactRole, digest: &str, profile: DigestSpellingProfile) -> ArtifactRefCandidate {
        ArtifactRefCandidate {
            role,
            expected_role: Some(role),
            digest: digest.to_owned(),
            digest_profile: profile,
            path: Some("artifacts/valence/ref.json".to_owned()),
            label: Some("valence-ref".to_owned()),
            size_bytes: Some(EXPECTED_FIXTURE_SIZE_BYTES),
            measured_digest: Some(digest.to_owned()),
            measured_size_bytes: Some(EXPECTED_FIXTURE_SIZE_BYTES),
            non_claims: non_claims(),
        }
    }

    fn assert_issue(report: &ArtifactRefValidationReport, field: &str, code: &str) {
        assert!(
            report.issues.iter().any(|issue| issue.field == field && issue.code == code),
            "expected issue {field}.{code}, got {:?}",
            report.issues
        );
    }

    fn valid_fixture(candidate: ArtifactRefCandidate) -> ArtifactRef {
        let report = validate_artifact_ref(&candidate);
        assert!(report.valid, "expected valid report, got {:?}", report.issues);
        assert_eq!(report.supported_claim, ARTIFACT_REF_REQUIRED_NON_CLAIM);
        report.artifact.expect("valid report returns artifact")
    }

    #[test]
    fn positive_stack_fixtures_accept_current_digest_spellings() {
        // r[verify valence.evidence_ir.artifact_ref_core.fixtures.positive]
        let fixtures = [
            ("valence", ArtifactRole::SourceArtifact, DIGEST_A_HEX, RAW_BLAKE3_PROFILE),
            ("cairn", ArtifactRole::Receipt, DIGEST_A_B3, STRICT_B3_PROFILE),
            ("mantle", ArtifactRole::ReleaseBundleMember, DIGEST_A_BLAKE3, MIXED_BLAKE3_PROFILE),
            ("basalt", ArtifactRole::BinaryArtifact, DIGEST_A_B3, MIXED_BLAKE3_PROFILE),
            ("octet", ArtifactRole::Sidecar, DIGEST_A_B3, STRICT_B3_PROFILE),
            ("molten-aspen", ArtifactRole::CompatibilityRef, DIGEST_A_HEX, MIXED_BLAKE3_PROFILE),
        ];

        for (label, role, digest, profile) in fixtures {
            let mut input = candidate(role, digest, profile);
            input.label = Some(label.to_owned());
            input.path = Some(format!("artifacts/{label}/ref.json"));
            let artifact = valid_fixture(input);

            assert_eq!(artifact.role, role);
            assert_eq!(artifact.digest.normalized_hex, DIGEST_A_HEX);
            assert_eq!(artifact.label.as_deref(), Some(label));
        }
    }

    #[test]
    fn role_mismatch_fails_closed_and_same_digest_preserves_role() {
        // r[verify valence.evidence_ir.artifact_ref_core.roles.same_digest]
        // r[verify valence.evidence_ir.artifact_ref_core.fixtures.negative]
        let mut input = candidate(ArtifactRole::Receipt, DIGEST_A_B3, STRICT_B3_PROFILE);
        input.expected_role = Some(ArtifactRole::Sidecar);
        let report = validate_artifact_ref(&input);

        assert!(!report.valid);
        assert_issue(&report, FIELD_EXPECTED_ROLE, "role_mismatch");

        let receipt = valid_fixture(candidate(ArtifactRole::Receipt, DIGEST_A_B3, STRICT_B3_PROFILE));
        let sidecar = valid_fixture(candidate(ArtifactRole::Sidecar, DIGEST_A_B3, STRICT_B3_PROFILE));
        assert_ne!(receipt, sidecar);
        assert_eq!(receipt.digest.normalized_hex, sidecar.digest.normalized_hex);
        assert_ne!(receipt.role, sidecar.role);
    }

    #[test]
    fn malformed_and_unsupported_digest_spellings_fail_closed() {
        // r[verify valence.evidence_ir.artifact_ref_core.fixtures.negative]
        let malformed = validate_artifact_ref(&candidate(ArtifactRole::Receipt, "b3:short", STRICT_B3_PROFILE));
        assert!(!malformed.valid);
        assert_issue(&malformed, FIELD_DIGEST, "digest_malformed");

        let unsupported_profile =
            validate_artifact_ref(&candidate(ArtifactRole::Receipt, DIGEST_A_BLAKE3, STRICT_B3_PROFILE));
        assert!(!unsupported_profile.valid);
        assert_issue(&unsupported_profile, FIELD_DIGEST, "digest_unsupported_prefix");

        let unsupported_prefix =
            validate_artifact_ref(&candidate(ArtifactRole::Receipt, "sha256:0123456789abcdef", MIXED_BLAKE3_PROFILE));
        assert!(!unsupported_prefix.valid);
        assert_issue(&unsupported_prefix, FIELD_DIGEST, "digest_unsupported_prefix");
    }

    #[test]
    fn path_and_label_confinement_rejects_escape_patterns() {
        // r[verify valence.evidence_ir.artifact_ref_core.validation]
        // r[verify valence.evidence_ir.artifact_ref_core.validation.shell]
        let mut path_escape = candidate(ArtifactRole::SourceArtifact, DIGEST_A_HEX, RAW_BLAKE3_PROFILE);
        path_escape.path = Some("../outside/ref.json".to_owned());
        let path_report = validate_artifact_ref(&path_escape);
        assert!(!path_report.valid);
        assert_issue(&path_report, FIELD_PATH, "path_escape");

        let mut absolute_path = candidate(ArtifactRole::SourceArtifact, DIGEST_A_HEX, RAW_BLAKE3_PROFILE);
        absolute_path.path = Some("/tmp/ref.json".to_owned());
        let absolute_report = validate_artifact_ref(&absolute_path);
        assert!(!absolute_report.valid);
        assert_issue(&absolute_report, FIELD_PATH, "path_not_relative");

        let mut label_escape = candidate(ArtifactRole::Receipt, DIGEST_A_B3, STRICT_B3_PROFILE);
        label_escape.label = Some("bad/label".to_owned());
        let label_report = validate_artifact_ref(&label_escape);
        assert!(!label_report.valid);
        assert_issue(&label_report, FIELD_LABEL, "label_escape");
    }

    #[test]
    fn stale_hash_and_size_metadata_fail_closed() {
        // r[verify valence.evidence_ir.artifact_ref_core.fixtures.negative]
        let mut stale = candidate(ArtifactRole::BinaryArtifact, DIGEST_A_B3, STRICT_B3_PROFILE);
        stale.measured_digest = Some(DIGEST_B_B3.to_owned());
        stale.measured_size_bytes = Some(STALE_FIXTURE_SIZE_BYTES);
        let report = validate_artifact_ref(&stale);

        assert!(!report.valid);
        assert_issue(&report, FIELD_MEASURED_DIGEST, "hash_metadata_mismatch");
        assert_issue(&report, FIELD_SIZE, "size_metadata_mismatch");
    }

    #[test]
    fn metadata_pairing_requires_measured_hash_and_size_together() {
        // r[verify valence.evidence_ir.artifact_ref_core.fixtures.negative]
        let mut missing_measured_size = candidate(ArtifactRole::Receipt, DIGEST_A_B3, STRICT_B3_PROFILE);
        missing_measured_size.measured_size_bytes = None;
        let missing_size_report = validate_artifact_ref(&missing_measured_size);
        assert!(!missing_size_report.valid);
        assert_issue(&missing_size_report, FIELD_METADATA, "hash_size_metadata_incomplete");

        let mut missing_declared_size = candidate(ArtifactRole::Receipt, DIGEST_A_B3, STRICT_B3_PROFILE);
        missing_declared_size.size_bytes = None;
        let missing_declared_report = validate_artifact_ref(&missing_declared_size);
        assert!(!missing_declared_report.valid);
        assert_issue(&missing_declared_report, FIELD_SIZE, "declared_size_missing");
    }

    #[test]
    fn missing_boundary_and_overclaim_fail_closed() {
        // r[verify valence.evidence_ir.artifact_ref_core.docs]
        // r[verify valence.evidence_ir.artifact_ref_core.final_validation]
        let mut missing = candidate(ArtifactRole::Receipt, DIGEST_A_B3, STRICT_B3_PROFILE);
        missing.non_claims = Vec::new();
        let missing_report = validate_artifact_ref(&missing);
        assert!(!missing_report.valid);
        assert_issue(&missing_report, FIELD_NON_CLAIMS, "non_claims_missing");

        let mut overclaim = candidate(ArtifactRole::Receipt, DIGEST_A_B3, STRICT_B3_PROFILE);
        overclaim.non_claims = vec![
            ARTIFACT_REF_REQUIRED_NON_CLAIM.to_owned(),
            "this proves release eligibility".to_owned(),
        ];
        let overclaim_report = validate_artifact_ref(&overclaim);
        assert!(!overclaim_report.valid);
        assert_issue(&overclaim_report, FIELD_NON_CLAIMS, "overclaim_detected");
    }

    #[test]
    fn public_field_adapter_preserves_field_name_and_validates() {
        // r[verify valence.evidence_ir.artifact_ref_core.adapters.shape]
        let report = validate_public_artifact_field(&PublicArtifactFieldCandidate {
            public_field: "canonical_hash".to_owned(),
            candidate: candidate(ArtifactRole::CompatibilityRef, DIGEST_A_HEX, RAW_BLAKE3_PROFILE),
        });

        assert!(report.report.valid);
        assert_eq!(report.public_field, "canonical_hash");
        assert_eq!(report.report.artifact.expect("valid adapter artifact").digest.spelling, DigestSpelling::RawHex);
    }

    #[test]
    fn public_field_adapter_rejects_empty_field_name() {
        // r[verify valence.evidence_ir.artifact_ref_core.fixtures.negative]
        let report = validate_public_artifact_field(&PublicArtifactFieldCandidate {
            public_field: "".to_owned(),
            candidate: candidate(ArtifactRole::CompatibilityRef, DIGEST_A_HEX, RAW_BLAKE3_PROFILE),
        });

        assert!(!report.report.valid);
        assert_issue(&report.report, FIELD_PUBLIC_FIELD, "public_field_empty");
    }
}
