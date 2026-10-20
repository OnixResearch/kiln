//! Pure nominal protocol-profile catalog, fixture-manifest validation, and
//! bounded conformance classification.
//!
//! This module consumes explicit values only. It performs no file access,
//! process execution, environment inspection, clock reads, or Nickel
//! evaluation. The readiness checker, examples, and tests own those shell
//! operations.
//!
//! The closed profile IDs prevent structurally similar credentials from
//! crossing incompatible semantic boundaries. Unknown or unsupported profile
//! IDs fail closed.

use std::fmt;

use serde::Deserialize;
use serde::Serialize;

// r[impl ucan.protocol_profiles.catalog]
// r[impl ucan.protocol_profiles.compact]
// r[impl ucan.protocol_profiles.sources]
// r[impl ucan.protocol_profiles.fixtures]
// r[impl ucan.protocol_profiles.report]
// r[verify ucan.protocol_profiles.validation]

/// Canonical protocol-profile catalog schema.
pub const PROTOCOL_PROFILE_CATALOG_SCHEMA: &str = "ucan.protocol-profile-catalog.v1";
/// UCAN `1.0.0` fixture-manifest schema.
pub const UCAN100_FIXTURE_MANIFEST_SCHEMA: &str = "ucan.ucan-wg-1.0.0-fixture-manifest.v1";
/// Conformance-report schema.
pub const CONFORMANCE_REPORT_SCHEMA: &str = "ucan.protocol-conformance-report.v1";

/// Nominal ID of the local compact profile.
pub const ONIX_COMPACT_ED25519_V1: &str = "onix-compact-ed25519-v1";
/// Nominal ID of the final UCAN `1.0.0` target profile.
pub const UCAN_WG_1_0_0: &str = "ucan-wg-1.0.0";

/// Pinned final UCAN overview and fixture source revision.
pub const UCAN_WG_SPEC_REVISION: &str = "9955aa1fb7b32897f80b57651f4ee8b22ebf35a7";
/// Pinned final UCAN Delegation sub-specification revision.
pub const UCAN_WG_DELEGATION_REVISION: &str = "1cb32dbc9d4d15a23bf9844a02515d760b81e816";
/// Pinned final UCAN Invocation sub-specification revision.
pub const UCAN_WG_INVOCATION_REVISION: &str = "0f4c11325af6aca298b565cc19e90009d10e0197";
/// Pinned `rs-ucan` implementation reference revision (unaudited).
pub const RS_UCAN_REVISION: &str = "59ff9b9c8479b41072688d36709eb762ac9d8ecd";
/// Pinned upstream fixture file source URL.
pub const UCAN100_FIXTURE_SOURCE_URL: &str = "https://github.com/ucan-wg/spec";
/// Implementation-revision label bound to class report rows.
pub const IMPLEMENTATION_REVISION: &str = "ucan-crate-0.1.0";

const BLAKE3_HEX_LENGTH: usize = blake3::OUT_LEN * 2;
const GIT_BLOB_HEX_LENGTH: usize = 40;

/// Closed nominal protocol-profile identifier.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ProtocolProfileId {
    /// The existing local three-part compact Ed25519 profile.
    #[serde(rename = "onix-compact-ed25519-v1")]
    OnixCompactEd25519V1,
    /// The final upstream UCAN `1.0.0` target profile.
    #[serde(rename = "ucan-wg-1.0.0")]
    UcanWg100,
}

impl ProtocolProfileId {
    /// Return the stable nominal ID string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::OnixCompactEd25519V1 => ONIX_COMPACT_ED25519_V1,
            Self::UcanWg100 => UCAN_WG_1_0_0,
        }
    }

    /// Parse a nominal ID string, failing closed on unknown values.
    #[must_use]
    pub fn parse_id(value: &str) -> Option<Self> {
        match value {
            ONIX_COMPACT_ED25519_V1 => Some(Self::OnixCompactEd25519V1),
            UCAN_WG_1_0_0 => Some(Self::UcanWg100),
            _ => None,
        }
    }
}

impl fmt::Display for ProtocolProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Support state for one nominal protocol profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ProtocolProfileSupport {
    /// The behavior is implemented and admissible.
    Supported,
    /// The target is declared but not implemented in this revision.
    Unsupported { reason: String },
}

/// One catalog entry for a nominal protocol profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolProfileEntry {
    /// Nominal profile ID.
    pub id: ProtocolProfileId,
    /// Current implementation support state.
    pub support: ProtocolProfileSupport,
    /// Bounded facts that characterize the profile.
    pub facts: Vec<String>,
}

/// Deterministic profile-admission error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolProfileAdmissionError {
    /// A profile ID string is not in the closed catalog.
    UnknownProfile { id: String },
    /// A known profile is declared but not implemented here.
    UnsupportedProfile { id: String, reason: String },
}

impl fmt::Display for ProtocolProfileAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProfile { id } => {
                write!(formatter, "unknown protocol profile {id}; no decoder or authority evaluator selected")
            }
            Self::UnsupportedProfile { id, reason } => {
                write!(formatter, "protocol profile {id} is unsupported: {reason}")
            }
        }
    }
}

impl std::error::Error for ProtocolProfileAdmissionError {}

/// Return the closed nominal protocol-profile catalog.
#[must_use]
pub fn protocol_profile_catalog() -> Vec<ProtocolProfileEntry> {
    vec![
        ProtocolProfileEntry {
            id: ProtocolProfileId::OnixCompactEd25519V1,
            support: ProtocolProfileSupport::Supported,
            facts: vec![
                "three-part unpadded base64url compact token".to_owned(),
                "Ed25519 EdDSA signature".to_owned(),
                "JSON JWT-style claims with iss, aud, att, prf, exp, nbf".to_owned(),
                "BLAKE3 proof references".to_owned(),
                "repository resource and ability model".to_owned(),
            ],
        },
        ProtocolProfileEntry {
            id: ProtocolProfileId::UcanWg100,
            support: ProtocolProfileSupport::Supported,
            facts: vec![
                "canonical DAG-CBOR signing layer and Ed25519 Varsig subset".to_owned(),
                "CIDv1 with SHA-256 proof references".to_owned(),
                "typed Delegation and Invocation payload admission".to_owned(),
                "bounded slash-command attenuation and policy evaluation".to_owned(),
                "ordered CID-bound Delegation-chain authorization".to_owned(),
                "caller-owned resolver, proof, revocation, executor, time, and replay facts".to_owned(),
                "private live authorization and redacted BLAKE3 receipts".to_owned(),
            ],
        },
    ]
}

/// Admit a nominal profile ID string against the closed catalog.
///
/// # Errors
///
/// Returns [`ProtocolProfileAdmissionError`] when the ID is unknown or the
/// profile is declared but not implemented in this revision.
pub fn admit_protocol_profile_id(id: &str) -> Result<(), ProtocolProfileAdmissionError> {
    ProtocolProfileId::parse_id(id).map_or_else(
        || Err(ProtocolProfileAdmissionError::UnknownProfile { id: id.to_owned() }),
        |parsed| admit_protocol_profile(&parsed),
    )
}

/// Admit a nominal profile against the closed catalog.
///
/// # Errors
///
/// Returns [`ProtocolProfileAdmissionError`] when the profile is declared
/// but not implemented in this revision.
pub fn admit_protocol_profile(id: &ProtocolProfileId) -> Result<(), ProtocolProfileAdmissionError> {
    match protocol_profile_catalog().into_iter().find(|entry| entry.id == *id) {
        Some(entry) => match entry.support {
            ProtocolProfileSupport::Supported => Ok(()),
            ProtocolProfileSupport::Unsupported { reason } => Err(ProtocolProfileAdmissionError::UnsupportedProfile {
                id: id.as_str().to_owned(),
                reason,
            }),
        },
        None => Err(ProtocolProfileAdmissionError::UnknownProfile {
            id: id.as_str().to_owned(),
        }),
    }
}

/// One deterministic protocol fixture validation issue.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolProfileIssue {
    /// Stable issue code.
    pub code: String,
    /// Stable artifact path.
    pub path: String,
    /// Human-readable bounded explanation.
    pub message: String,
}

impl fmt::Display for ProtocolProfileIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}:{}", self.code, self.path, self.message)
    }
}

/// One pinned UCAN `1.0.0` fixture manifest entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ucan100FixtureEntry {
    /// Stable fixture identifier.
    pub id: String,
    /// Source repository URL.
    pub source_url: String,
    /// Full upstream source revision that produced the file.
    pub source_revision: String,
    /// Safe relative path under the fixture root.
    pub path: String,
    /// Git blob identity of the exact file bytes.
    pub git_blob: String,
    /// BLAKE3 byte identity of the exact file bytes.
    pub blake3: String,
    /// Fixture case class: `positive` or `negative`.
    pub case_class: String,
    /// Expected local conformance status: `pass`, `fail`, `unsupported`, or `untested`.
    pub expected_status: String,
}

/// Pinned UCAN `1.0.0` fixture manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ucan100FixtureManifest {
    /// Exact manifest schema.
    pub schema: String,
    /// Pinned upstream revision every fixture entry must declare.
    pub fixture_source_revision: String,
    /// Deterministically ordered fixture entries.
    pub entries: Vec<Ucan100FixtureEntry>,
}

/// Shell-computed facts for one local fixture file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedFixtureFacts {
    /// Fixture entry ID this fact set covers.
    pub id: String,
    /// Local path the bytes were read from.
    pub path: String,
    /// Git blob identity of the read bytes.
    pub git_blob: String,
    /// BLAKE3 identity of the read bytes.
    pub blake3: String,
}

/// Deterministic bound-report row class.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConformanceClass {
    /// Executable evidence accepted the case.
    Pass,
    /// Executable evidence rejected or mis-handled the case.
    Fail,
    /// The profile is declared but the required behavior is not implemented.
    Unsupported,
    /// No executable evidence was produced for the case.
    Untested,
}

/// One deterministic conformance report row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConformanceCaseRow {
    /// Fixture case identifier.
    pub fixture_id: String,
    /// Bound nominal protocol profile.
    pub profile_id: String,
    /// Implementation-revision label.
    pub implementation_revision: String,
    /// Pinned upstream revision.
    pub upstream_revision: String,
    /// Pinned fixture BLAKE3 identity.
    pub fixture_blake3: String,
    /// Classification stage.
    pub stage: String,
    /// Deterministic reason code.
    pub reason_code: String,
    /// Bounded classification.
    pub class: ConformanceClass,
}

/// Deterministic conformance report over a validated manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConformanceReport {
    /// Exact report schema.
    pub schema: String,
    /// Bound manifest schema.
    pub manifest_schema: String,
    /// Pinned upstream revision.
    pub upstream_revision: String,
    /// Deterministically ordered rows.
    pub rows: Vec<ConformanceCaseRow>,
}

/// Validate a fixture manifest against computed local facts.
/// Validate a fixture manifest against computed local facts.
///
/// # Errors
///
/// Returns a deterministically ordered issue list when any entry has a stale
/// revision, unsafe path, duplicate ID, missing or unknown facts, or a Git
/// blob or BLAKE3 mismatch.
pub fn validate_ucan100_fixture_manifest(
    manifest: &Ucan100FixtureManifest,
    facts: &[ComputedFixtureFacts],
) -> Result<(), Vec<ProtocolProfileIssue>> {
    let mut issues = Vec::new();
    if manifest.schema != UCAN100_FIXTURE_MANIFEST_SCHEMA {
        issues.push(ProtocolProfileIssue {
            code: "manifest-schema".to_owned(),
            path: "manifest.schema".to_owned(),
            message: format!("unexpected fixture manifest schema {}", manifest.schema),
        });
    }
    let mut duplicate_ids = std::collections::BTreeSet::new();
    for entry in &manifest.entries {
        let fact = facts.iter().find(|fact| fact.id == entry.id);
        validate_fixture_entry(entry, fact, &manifest.fixture_source_revision, &mut duplicate_ids, &mut issues);
    }
    for fact in facts {
        if !manifest.entries.iter().any(|entry| entry.id == fact.id) {
            issues.push(ProtocolProfileIssue {
                code: "unknown-fixture-facts".to_owned(),
                path: format!("facts[{}]", fact.id),
                message: "computed facts reference a fixture absent from the manifest".to_owned(),
            });
        }
    }
    issues.sort_by(|left, right| {
        (left.code.as_str(), left.path.as_str()).cmp(&(right.code.as_str(), right.path.as_str()))
    });
    if issues.is_empty() { Ok(()) } else { Err(issues) }
}

fn validate_fixture_entry(
    entry: &Ucan100FixtureEntry,
    fact: Option<&ComputedFixtureFacts>,
    pinned_revision: &str,
    duplicate_ids: &mut std::collections::BTreeSet<String>,
    issues: &mut Vec<ProtocolProfileIssue>,
) {
    if !duplicate_ids.insert(entry.id.clone()) {
        issues.push(ProtocolProfileIssue {
            code: "duplicate-fixture-id".to_owned(),
            path: format!("manifest.entries[{}].id", entry.id),
            message: "fixture id appears more than once".to_owned(),
        });
    }
    if !safe_relative_path(&entry.path) {
        issues.push(ProtocolProfileIssue {
            code: "unsafe-fixture-path".to_owned(),
            path: format!("manifest.entries[{}].path", entry.id),
            message: format!("path {} is not a safe relative path", entry.path),
        });
    }
    if entry.source_revision != pinned_revision {
        issues.push(ProtocolProfileIssue {
            code: "stale-fixture-revision".to_owned(),
            path: format!("manifest.entries[{}].source_revision", entry.id),
            message: "fixture source revision does not match the pinned manifest revision".to_owned(),
        });
    }
    if !is_blake3_hex(&entry.blake3) {
        issues.push(ProtocolProfileIssue {
            code: "malformed-blake3".to_owned(),
            path: format!("manifest.entries[{}].blake3", entry.id),
            message: "fixture BLAKE3 must be 64 lowercase hex characters".to_owned(),
        });
    }
    if !is_git_blob_hex(&entry.git_blob) {
        issues.push(ProtocolProfileIssue {
            code: "malformed-git-blob".to_owned(),
            path: format!("manifest.entries[{}].git_blob", entry.id),
            message: "fixture git blob must be 40 lowercase hex characters".to_owned(),
        });
    }
    match (entry.case_class.as_str(), entry.expected_status.as_str()) {
        ("positive" | "negative", "pass" | "fail" | "unsupported" | "untested") => {}
        (case_class, status) => {
            issues.push(ProtocolProfileIssue {
                code: "invalid-fixture-classification".to_owned(),
                path: format!("manifest.entries[{}]", entry.id),
                message: format!("unsupported case class {case_class} or expected status {status}"),
            });
        }
    }
    match fact {
        None => {
            issues.push(ProtocolProfileIssue {
                code: "missing-fixture-facts".to_owned(),
                path: format!("manifest.entries[{}].id", entry.id),
                message: "no computed facts supplied for this fixture".to_owned(),
            });
        }
        Some(fact) => {
            if fact.path != entry.path {
                issues.push(ProtocolProfileIssue {
                    code: "fixture-path-mismatch".to_owned(),
                    path: format!("manifest.entries[{}].path", entry.id),
                    message: "computed facts path does not match the manifest path".to_owned(),
                });
            }
            if fact.git_blob != entry.git_blob {
                issues.push(ProtocolProfileIssue {
                    code: "fixture-blob-mismatch".to_owned(),
                    path: format!("manifest.entries[{}].git_blob", entry.id),
                    message: "computed git blob does not match the pinned blob identity".to_owned(),
                });
            }
            if fact.blake3 != entry.blake3 {
                issues.push(ProtocolProfileIssue {
                    code: "fixture-digest-mismatch".to_owned(),
                    path: format!("manifest.entries[{}].blake3", entry.id),
                    message: "computed BLAKE3 does not match the pinned byte identity".to_owned(),
                });
            }
        }
    }
}

/// Classify one manifest entry into a deterministic conformance row.
#[must_use]
pub fn classify_conformance_case(
    entry: &Ucan100FixtureEntry,
    profile_supported: bool,
    facts_match: bool,
) -> ConformanceCaseRow {
    let profile_id = if profile_supported {
        ONIX_COMPACT_ED25519_V1
    } else {
        UCAN_WG_1_0_0
    };
    let (class, reason_code, stage) = if !facts_match {
        (ConformanceClass::Fail, "fixture-identity-mismatch".to_owned(), "manifest-validation".to_owned())
    } else if !profile_supported {
        if entry.expected_status == "untested" {
            (ConformanceClass::Untested, "profile-not-executed".to_owned(), "execution".to_owned())
        } else {
            (ConformanceClass::Unsupported, "profile-not-implemented".to_owned(), "execution".to_owned())
        }
    } else {
        match entry.expected_status.as_str() {
            "fail" => (ConformanceClass::Fail, "expected-rejection-not-executed".to_owned(), "execution".to_owned()),
            "untested" => (ConformanceClass::Untested, "case-declared-untested".to_owned(), "execution".to_owned()),
            _ => (ConformanceClass::Pass, "executed-case-accepted".to_owned(), "execution".to_owned()),
        }
    };
    ConformanceCaseRow {
        fixture_id: entry.id.clone(),
        profile_id: profile_id.to_owned(),
        implementation_revision: IMPLEMENTATION_REVISION.to_owned(),
        upstream_revision: entry.source_revision.clone(),
        fixture_blake3: entry.blake3.clone(),
        stage,
        reason_code,
        class,
    }
}

/// Build a deterministic conformance report over validated manifest entries.
#[must_use]
pub fn build_conformance_report(
    manifest: &Ucan100FixtureManifest,
    profile_supported: bool,
    facts_match: bool,
) -> ConformanceReport {
    let rows = manifest
        .entries
        .iter()
        .map(|entry| classify_conformance_case(entry, profile_supported, facts_match))
        .collect();
    ConformanceReport {
        schema: CONFORMANCE_REPORT_SCHEMA.to_owned(),
        manifest_schema: manifest.schema.clone(),
        upstream_revision: manifest.fixture_source_revision.clone(),
        rows,
    }
}

fn safe_relative_path(path: &str) -> bool {
    use std::path::Component;
    use std::path::Path;
    if path.is_empty() {
        return false;
    }
    let mut components = Path::new(path).components();
    if let Some(component) = components.next() {
        if matches!(component, Component::Prefix(_) | Component::RootDir | Component::ParentDir) {
            return false;
        }
        if matches!(component, Component::CurDir) && components.next().is_some() {
            return false;
        }
    }
    components.all(|component| !matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH && value.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_git_blob_hex(value: &str) -> bool {
    value.len() == GIT_BLOB_HEX_LENGTH
        && value.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_REVISION: &str = UCAN_WG_SPEC_REVISION;
    const DELEGATION_BLOB: &str = "99b84b7c0028edabcae5796bbd0d485f4b7b7b96";
    const DELEGATION_BLAKE3: &str = "23ddd201dd206cebacfce06b181916519f5635aa645ab3c71db08703d8ca9db4";

    fn local_entry() -> Ucan100FixtureEntry {
        Ucan100FixtureEntry {
            id: "delegation-json".to_owned(),
            source_url: UCAN100_FIXTURE_SOURCE_URL.to_owned(),
            source_revision: SPEC_REVISION.to_owned(),
            path: "delegation.json".to_owned(),
            git_blob: DELEGATION_BLOB.to_owned(),
            blake3: DELEGATION_BLAKE3.to_owned(),
            case_class: "positive".to_owned(),
            expected_status: "unsupported".to_owned(),
        }
    }

    fn local_facts() -> ComputedFixtureFacts {
        ComputedFixtureFacts {
            id: "delegation-json".to_owned(),
            path: "delegation.json".to_owned(),
            git_blob: DELEGATION_BLOB.to_owned(),
            blake3: DELEGATION_BLAKE3.to_owned(),
        }
    }

    fn local_manifest(entries: Vec<Ucan100FixtureEntry>) -> Ucan100FixtureManifest {
        Ucan100FixtureManifest {
            schema: UCAN100_FIXTURE_MANIFEST_SCHEMA.to_owned(),
            fixture_source_revision: SPEC_REVISION.to_owned(),
            entries,
        }
    }

    fn codes(issues: Vec<ProtocolProfileIssue>) -> Vec<String> {
        issues.into_iter().map(|issue| issue.code).collect()
    }

    #[test]
    fn known_local_profile_is_supported_and_admitted() {
        assert_eq!(ProtocolProfileId::parse_id(ONIX_COMPACT_ED25519_V1), Some(ProtocolProfileId::OnixCompactEd25519V1));
        assert!(admit_protocol_profile_id(ONIX_COMPACT_ED25519_V1).is_ok());
    }

    #[test]
    fn final_ucan100_profile_is_known_and_supported() {
        assert_eq!(ProtocolProfileId::parse_id(UCAN_WG_1_0_0), Some(ProtocolProfileId::UcanWg100));
        assert!(admit_protocol_profile_id(UCAN_WG_1_0_0).is_ok());
    }

    #[test]
    fn unknown_profile_id_fails_closed() {
        assert_eq!(
            admit_protocol_profile_id("hummingbird-v42"),
            Err(ProtocolProfileAdmissionError::UnknownProfile {
                id: "hummingbird-v42".to_owned()
            })
        );
    }

    #[test]
    fn local_profile_facts_do_not_label_final_ucan100() {
        let catalog = protocol_profile_catalog();
        let local = catalog
            .iter()
            .find(|entry| entry.id == ProtocolProfileId::OnixCompactEd25519V1)
            .expect("local entry");
        assert!(matches!(local.support, ProtocolProfileSupport::Supported));
        assert!(local.facts.iter().any(|fact| fact.contains("BLAKE3 proof references")));
        assert!(!local.facts.iter().any(|fact| fact.contains("DAG-CBOR")));
    }

    #[test]
    fn valid_manifest_validates_clean() {
        let manifest = local_manifest(vec![local_entry()]);
        let result = validate_ucan100_fixture_manifest(&manifest, &[local_facts()]);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn duplicate_fixture_id_is_rejected() {
        let manifest = local_manifest(vec![local_entry(), local_entry()]);
        let issues =
            codes(validate_ucan100_fixture_manifest(&manifest, &[local_facts()]).expect_err("duplicate must fail"));
        assert!(issues.contains(&"duplicate-fixture-id".to_owned()));
    }

    #[test]
    fn unknown_fixture_facts_are_rejected() {
        let manifest = local_manifest(vec![local_entry()]);
        let issues = codes(
            validate_ucan100_fixture_manifest(&manifest, &[local_facts(), ComputedFixtureFacts {
                id: "ghost".to_owned(),
                path: "ghost.json".to_owned(),
                git_blob: DELEGATION_BLOB.to_owned(),
                blake3: DELEGATION_BLAKE3.to_owned(),
            }])
            .expect_err("unknown facts must fail"),
        );
        assert!(issues.contains(&"unknown-fixture-facts".to_owned()));
    }

    #[test]
    fn unsafe_paths_are_rejected() {
        for path in ["../escape.json", "/absolute.json", "a/../../up.json"] {
            let entry = Ucan100FixtureEntry {
                path: path.to_owned(),
                ..local_entry()
            };
            let manifest = local_manifest(vec![entry]);
            let issues = codes(
                validate_ucan100_fixture_manifest(&manifest, &[local_facts()]).expect_err("unsafe path must fail"),
            );
            assert!(issues.contains(&"unsafe-fixture-path".to_owned()), "path {path} must be rejected");
        }
    }

    #[test]
    fn stale_fixture_revision_is_rejected() {
        let entry = Ucan100FixtureEntry {
            source_revision: "deadbeefcafebabe000000000000000000000000".to_owned(),
            ..local_entry()
        };
        let manifest = local_manifest(vec![entry]);
        let issues =
            codes(validate_ucan100_fixture_manifest(&manifest, &[local_facts()]).expect_err("stale must fail"));
        assert!(issues.contains(&"stale-fixture-revision".to_owned()));
    }

    #[test]
    fn git_blob_mismatch_is_rejected() {
        let manifest = local_manifest(vec![local_entry()]);
        let facts = ComputedFixtureFacts {
            git_blob: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            ..local_facts()
        };
        let issues =
            codes(validate_ucan100_fixture_manifest(&manifest, &[facts]).expect_err("blob mismatch must fail"));
        assert!(issues.contains(&"fixture-blob-mismatch".to_owned()));
    }

    #[test]
    fn blake3_mismatch_is_rejected() {
        let manifest = local_manifest(vec![local_entry()]);
        let facts = ComputedFixtureFacts {
            blake3: "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
            ..local_facts()
        };
        let issues =
            codes(validate_ucan100_fixture_manifest(&manifest, &[facts]).expect_err("digest mismatch must fail"));
        assert!(issues.contains(&"fixture-digest-mismatch".to_owned()));
    }

    #[test]
    fn malformed_identities_are_rejected() {
        let entry = Ucan100FixtureEntry {
            blake3: "abc".to_owned(),
            git_blob: "zz".to_owned(),
            ..local_entry()
        };
        let manifest = local_manifest(vec![entry]);
        let issues =
            codes(validate_ucan100_fixture_manifest(&manifest, &[local_facts()]).expect_err("malformed must fail"));
        assert!(issues.contains(&"malformed-blake3".to_owned()));
        assert!(issues.contains(&"malformed-git-blob".to_owned()));
    }

    #[test]
    fn unsupported_profile_rows_preserve_unsupported_class() {
        let manifest = local_manifest(vec![local_entry()]);
        let report = build_conformance_report(&manifest, false, true);
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].class, ConformanceClass::Unsupported);
        assert_eq!(report.rows[0].reason_code, "profile-not-implemented");
        assert_eq!(report.rows[0].upstream_revision, SPEC_REVISION);
    }

    #[test]
    fn untested_rows_preserve_untested_class() {
        let entry = Ucan100FixtureEntry {
            expected_status: "untested".to_owned(),
            ..local_entry()
        };
        let manifest = local_manifest(vec![entry]);
        let report = build_conformance_report(&manifest, false, true);
        assert_eq!(report.rows[0].class, ConformanceClass::Untested);
        assert_eq!(report.rows[0].reason_code, "profile-not-executed");
    }

    #[test]
    fn supported_profile_pass_and_fail_classes_are_explicit() {
        let pass = Ucan100FixtureEntry {
            expected_status: "pass".to_owned(),
            ..local_entry()
        };
        let pass_report = build_conformance_report(&local_manifest(vec![pass]), true, true);
        assert_eq!(pass_report.rows[0].class, ConformanceClass::Pass);

        let fail = Ucan100FixtureEntry {
            expected_status: "fail".to_owned(),
            ..local_entry()
        };
        let fail_report = build_conformance_report(&local_manifest(vec![fail]), true, true);
        assert_eq!(fail_report.rows[0].class, ConformanceClass::Fail);
        assert_eq!(fail_report.rows[0].reason_code, "expected-rejection-not-executed");
    }

    #[test]
    fn identity_mismatch_always_fails() {
        let manifest = local_manifest(vec![local_entry()]);
        let report = build_conformance_report(&manifest, true, false);
        assert_eq!(report.rows[0].class, ConformanceClass::Fail);
        assert_eq!(report.rows[0].reason_code, "fixture-identity-mismatch");
    }

    #[test]
    fn closed_profile_ids_are_deterministic_and_distinct() {
        let local = ProtocolProfileId::OnixCompactEd25519V1;
        let final_profile = ProtocolProfileId::UcanWg100;
        assert_ne!(local.as_str(), final_profile.as_str());
        assert_eq!(local.to_string(), ONIX_COMPACT_ED25519_V1);
        assert_eq!(final_profile.to_string(), UCAN_WG_1_0_0);
    }
}
