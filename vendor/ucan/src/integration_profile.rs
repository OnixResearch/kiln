//! Pure support-catalog identity and integration-profile admission.
//!
//! This module consumes explicit values only. It performs no file access,
//! process execution, environment inspection, clock reads, or Nickel
//! evaluation. The readiness checker binary owns those shell operations.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Write as _;

use serde::Deserialize;
use serde::Serialize;

use crate::protocol_profile::ProtocolProfileAdmissionError;
use crate::readiness::ProductionReadinessReport;
use crate::readiness::ReadinessOption;
use crate::readiness::SupportLevel;
use crate::readiness::production_readiness_report;
use crate::token::VerificationLimits;

// r[impl ucan.integration_support_profiles.support_catalog]
// r[impl ucan.integration_support_profiles.support_levels]
// r[impl ucan.integration_support_profiles.profile_contract]
// r[impl ucan.integration_support_profiles.subset_admission]
// r[impl ucan.integration_support_profiles.fixture_manifests]
// r[verify ucan.integration_support_profiles.fixtures]
// r[verify ucan.integration_support_profiles.freshness]
// r[impl ucan.integration_support_profiles.runtime_boundary]
/// Canonical support-catalog schema.
pub const SUPPORT_CATALOG_SCHEMA: &str = "ucan.support-catalog.v1";
/// Generated JSON projection schema.
pub const SUPPORT_CATALOG_PROJECTION_SCHEMA: &str = "ucan.support-catalog-projection.v1";
/// Generated Nickel vocabulary schema.
pub const SUPPORT_VOCABULARY_SCHEMA: &str = "ucan.support-vocabulary.v1";
/// Reviewed integration-profile schema.
pub const INTEGRATION_PROFILE_SCHEMA: &str = "ucan.integration-support-profile.v1";
/// Public fixture-manifest schema.
pub const FIXTURE_MANIFEST_SCHEMA: &str = "ucan.integration-fixture-manifest.v1";
/// Profile-admission report schema.
pub const PROFILE_ADMISSION_SCHEMA: &str = "ucan.integration-profile-admission.v1";
/// Fixture-validation report schema.
pub const FIXTURE_VALIDATION_SCHEMA: &str = "ucan.integration-fixture-validation.v1";
/// Name bound to the crate's default verification ceilings.
pub const DEFAULT_LIMIT_PROFILE_ID: &str = "ucan-default-ceilings";
/// Start marker for the generated operator documentation section.
pub const GENERATED_DOC_START: &str = "<!-- BEGIN GENERATED UCAN SUPPORT CATALOG -->";
/// End marker for the generated operator documentation section.
pub const GENERATED_DOC_END: &str = "<!-- END GENERATED UCAN SUPPORT CATALOG -->";

const REPORT_SCHEMA_VERSION: u8 = 1;
const BLAKE3_HEX_LENGTH: usize = blake3::OUT_LEN * 2;
const MAXIMUM_IDENTIFIER_BYTES: usize = 128;
const SUPPORT_STRENGTH_UNSUPPORTED: u8 = 0;
const SUPPORT_STRENGTH_PREVIEW: u8 = 1;
const SUPPORT_STRENGTH_CONTROLLED: u8 = 2;
const SUPPORT_STRENGTH_PRODUCTION: u8 = 3;
const EXPECTED_OPTION_GROUP_COUNT: usize = 3;

const REQUIRED_NON_CLAIMS: &[&str] = &[
    "Profile conformance does not prove resolver truth.",
    "Profile conformance does not prove key custody or cryptographic correctness.",
    "Profile conformance does not prove revocation or replay freshness.",
    "Profile conformance does not prove backend durability.",
    "Profile conformance does not prove application authorization completeness.",
];

const REQUIRED_PROMOTION_EVIDENCE: &[&str] = &[
    "positive-tests",
    "negative-fail-closed-tests",
    "operator-evidence",
    "migration-review",
];

const REQUIRED_INTEGRATION_MODES: &[(&str, &str)] = &[
    ("key_resolution", "caller-supplied"),
    ("proof_loading", "caller-supplied"),
    ("revocation", "caller-supplied"),
    ("replay", "caller-supplied"),
    ("verification_time", "explicit"),
    ("caveat_policy", "caller-supplied"),
    ("signer_custody", "caller-owned"),
    ("backend_failure", "fail-closed"),
];

const BASELINE_CONTROLLED_SURFACES: &[&str] = &[
    "ed25519-did-key-compact-token",
    "caller-supplied-key-resolution",
    "caller-supplied-proof-store",
    "caller-supplied-revocation",
    "generic-consumer-integration-hooks",
    "caller-supplied-replay-admission",
    "holder-signed-invocation-admission",
    "verified-replay-admission-transcript",
    "verified-proof-traversal-orchestration",
    "verified-compact-token-codec-structure",
    "verified-core-structural-validators",
    "verified-ability-helper-predicates",
    "verified-backend-transcript-aggregation",
    "opaque-caveat-policy-evaluation",
];

const BASELINE_PREVIEW_SURFACES: &[&str] = &["finite-typed-caveat-helpers", "web-of-trust-helpers"];

const BASELINE_UNSUPPORTED_SURFACES: &[&str] = &[
    "remote-did-resolution",
    "proof-transport-backends",
    "registry-backed-revocation",
    "replay-persistence-store",
    "non-ed25519-key-algorithms",
];

const NOSTR_ADAPTER_BOUNDARY_FRAGMENTS: &[&str] = &[
    "relay access",
    "DNS",
    "signing",
    "clocks",
    "replay storage",
    "subject resolution",
    "authorization",
];

const SECRET_LIKE_FRAGMENTS: &[&str] = &[
    "private_key",
    "private-key",
    "secret=",
    "bearer ",
    "bearer-token",
    "credential=",
    "token=",
    "backend://",
];

/// One deterministic validation issue.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessIssue {
    /// Stable issue code.
    pub code: String,
    /// Stable field or artifact path.
    pub path: String,
    /// Human-readable bounded explanation.
    pub message: String,
}

/// Error collection returned when canonical catalog material cannot be built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadinessIssues {
    /// Deterministically ordered issues.
    pub issues: Vec<ReadinessIssue>,
}

/// One support-catalog surface.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogSurface {
    /// Stable surface identifier.
    pub id: String,
    /// Current support level.
    pub level: SupportLevel,
    /// Evidence summary.
    pub evidence: String,
    /// Retained caller-owned boundary.
    pub boundary: String,
    /// Baseline level used to detect unreviewed promotion.
    pub baseline_level: Option<SupportLevel>,
    /// Explicit evidence for a promotion above the baseline.
    pub promotion_review: Option<PromotionReview>,
}

/// One closed DID, signature, or key choice.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogOption {
    /// Stable option identifier.
    pub id: String,
    /// Current support level.
    pub level: SupportLevel,
    /// Evidence summary.
    pub evidence: String,
    /// Retained caller-owned boundary.
    pub boundary: String,
}

/// Explicit support-level promotion evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionReview {
    /// Previous support level.
    pub from: SupportLevel,
    /// Promoted support level.
    pub to: SupportLevel,
    /// Stable reviewed migration identifier.
    pub migration_id: String,
    /// Required evidence classes.
    pub evidence: Vec<String>,
}

/// Catalog-wide promotion policy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionPolicy {
    /// Ordered closed support levels from weakest to strongest.
    pub ordered_levels: Vec<SupportLevel>,
    /// Evidence classes required for every promotion.
    pub required_evidence: Vec<String>,
}

/// One required integration-mode vocabulary entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogIntegrationMode {
    /// Stable integration-mode field identifier.
    pub id: String,
    /// Closed admitted value.
    pub admitted_value: String,
}

/// Finite verification ceilings bound to the catalog.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationLimitProfile {
    /// Stable named limit profile.
    pub id: String,
    /// Maximum proof depth.
    pub max_proof_depth: u64,
    /// Maximum number of resolved proofs.
    pub max_proof_count: u64,
    /// Maximum compact-token input bytes.
    pub max_input_bytes: u64,
}

/// Canonical owned support catalog derived from the Rust readiness report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupportCatalog {
    /// Exact catalog schema.
    pub schema: String,
    /// Readiness-report schema version.
    pub report_schema_version: u8,
    /// Crate-level support label.
    pub crate_level: SupportLevel,
    /// Production-relevant surfaces.
    pub surfaces: Vec<CatalogSurface>,
    /// Closed DID method choices.
    pub did_methods: Vec<CatalogOption>,
    /// Closed signature algorithm choices.
    pub signing_algorithms: Vec<CatalogOption>,
    /// Closed key class choices.
    pub key_classes: Vec<CatalogOption>,
    /// Required caller-owned integration modes.
    pub integration_modes: Vec<CatalogIntegrationMode>,
    /// Maximum admitted verification limits.
    pub verification_limit_ceiling: VerificationLimitProfile,
    /// Support promotion policy.
    pub promotion_policy: PromotionPolicy,
    /// Required bounded non-claims.
    pub required_non_claims: Vec<String>,
}

/// Generated support-catalog JSON envelope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupportCatalogProjection {
    /// Exact projection schema.
    pub projection_schema: String,
    /// BLAKE3 of compact canonical catalog bytes.
    pub catalog_blake3: String,
    /// Canonical catalog value.
    pub catalog: SupportCatalog,
}

/// Explicit caller-owned integration choices.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationModes {
    /// Key resolver integration mode.
    pub key_resolution: String,
    /// Proof loading integration mode.
    pub proof_loading: String,
    /// Revocation integration mode.
    pub revocation: String,
    /// Replay integration mode.
    pub replay: String,
    /// Verification-time source mode.
    pub verification_time: String,
    /// Caveat-policy integration mode.
    pub caveat_policy: String,
    /// Signer custody mode.
    pub signer_custody: String,
    /// Backend failure disposition.
    pub backend_failure: String,
}

impl IntegrationModes {
    fn pairs(&self) -> [(&'static str, &str); REQUIRED_INTEGRATION_MODES.len()] {
        [
            ("key_resolution", &self.key_resolution),
            ("proof_loading", &self.proof_loading),
            ("revocation", &self.revocation),
            ("replay", &self.replay),
            ("verification_time", &self.verification_time),
            ("caveat_policy", &self.caveat_policy),
            ("signer_custody", &self.signer_custody),
            ("backend_failure", &self.backend_failure),
        ]
    }
}

/// Human-authored integration support profile exported from Nickel.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationSupportProfile {
    /// Exact profile schema.
    pub schema: String,
    /// Stable profile identifier.
    pub id: String,
    /// Bound support-catalog BLAKE3.
    pub catalog_blake3: String,
    /// Required support surfaces.
    pub required_surfaces: Vec<String>,
    /// Required preview surfaces admitted explicitly.
    pub admitted_preview_surfaces: Vec<String>,
    /// Selected DID methods.
    pub did_methods: Vec<String>,
    /// Selected compact-token signature algorithms.
    pub signing_algorithms: Vec<String>,
    /// Selected signing and verification key classes.
    pub key_classes: Vec<String>,
    /// Finite named verification limits.
    pub verification_limits: VerificationLimitProfile,
    /// Explicit caller-owned integration choices.
    pub integrations: IntegrationModes,
    /// Whether all backend and policy failures deny by default.
    pub fail_closed: bool,
    /// Explicit bounded non-claims.
    pub non_claims: Vec<String>,
    /// Selected nominal protocol-profile ID (defaults to the local compact profile).
    #[serde(default = "default_local_protocol_profile")]
    pub protocol_profile: String,
}

/// Default integration-profile protocol selection: the local compact profile.
fn default_local_protocol_profile() -> String {
    crate::protocol_profile::ONIX_COMPACT_ED25519_V1.to_owned()
}

/// Deterministic profile-admission report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileAdmissionReport {
    /// Exact report schema.
    pub schema: String,
    /// Profile identifier.
    pub profile_id: String,
    /// Current support-catalog identity when available.
    pub catalog_blake3: String,
    /// Whether every admission check passed.
    pub valid: bool,
    /// Deterministically ordered issues.
    pub issues: Vec<ReadinessIssue>,
    /// Preserved profile non-claims.
    pub non_claims: Vec<String>,
}

/// Public bounded integration fixture manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationFixtureManifest {
    /// Exact fixture schema.
    pub schema: String,
    /// Stable fixture identifier.
    pub id: String,
    /// Bound reviewed profile identifier.
    pub profile_id: String,
    /// BLAKE3 of public fixture input bytes.
    pub public_input_blake3: String,
    /// BLAKE3 of the public reference artifact.
    pub public_reference_blake3: String,
    /// Expected result class: allow, deny, or error.
    pub expected_result: String,
    /// Support surfaces exercised by the fixture.
    pub required_surfaces: Vec<String>,
    /// Explicit bounded non-claims.
    pub non_claims: Vec<String>,
}

/// Deterministic fixture-validation report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureValidationReport {
    /// Exact report schema.
    pub schema: String,
    /// Fixture identifier.
    pub fixture_id: String,
    /// Whether every fixture check passed.
    pub valid: bool,
    /// Deterministically ordered issues.
    pub issues: Vec<ReadinessIssue>,
}

/// Build the canonical owned support catalog from the Rust readiness authority.
#[must_use]
pub fn support_catalog() -> SupportCatalog {
    support_catalog_from_report(production_readiness_report())
}

/// Build an owned support catalog from an explicit readiness report.
#[must_use]
pub fn support_catalog_from_report(report: ProductionReadinessReport) -> SupportCatalog {
    let limits = VerificationLimits::default();
    SupportCatalog {
        schema: SUPPORT_CATALOG_SCHEMA.to_owned(),
        report_schema_version: report.schema_version,
        crate_level: report.crate_level,
        surfaces: report
            .surfaces
            .iter()
            .map(|surface| CatalogSurface {
                id: surface.id.to_owned(),
                level: surface.level,
                evidence: surface.evidence.to_owned(),
                boundary: surface.boundary.to_owned(),
                baseline_level: baseline_surface_level(surface.id),
                promotion_review: promotion_review(surface.id),
            })
            .collect(),
        did_methods: catalog_options(report.did_methods),
        signing_algorithms: catalog_options(report.signing_algorithms),
        key_classes: catalog_options(report.key_classes),
        integration_modes: REQUIRED_INTEGRATION_MODES
            .iter()
            .map(|(id, admitted_value)| CatalogIntegrationMode {
                id: (*id).to_owned(),
                admitted_value: (*admitted_value).to_owned(),
            })
            .collect(),
        verification_limit_ceiling: VerificationLimitProfile {
            id: DEFAULT_LIMIT_PROFILE_ID.to_owned(),
            max_proof_depth: usize_to_u64(limits.max_proof_depth()),
            max_proof_count: usize_to_u64(limits.max_proof_count()),
            max_input_bytes: usize_to_u64(limits.max_compact_token_bytes()),
        },
        promotion_policy: PromotionPolicy {
            ordered_levels: vec![
                SupportLevel::Unsupported,
                SupportLevel::Preview,
                SupportLevel::ControlledIntegrationReady,
                SupportLevel::ProductionSupported,
            ],
            required_evidence: REQUIRED_PROMOTION_EVIDENCE.iter().map(ToString::to_string).collect(),
        },
        required_non_claims: REQUIRED_NON_CLAIMS.iter().map(ToString::to_string).collect(),
    }
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn catalog_options(options: &[ReadinessOption]) -> Vec<CatalogOption> {
    options
        .iter()
        .map(|option| CatalogOption {
            id: option.id.to_owned(),
            level: option.level,
            evidence: option.evidence.to_owned(),
            boundary: option.boundary.to_owned(),
        })
        .collect()
}

fn baseline_surface_level(id: &str) -> Option<SupportLevel> {
    if BASELINE_CONTROLLED_SURFACES.contains(&id) {
        return Some(SupportLevel::ControlledIntegrationReady);
    }
    if BASELINE_PREVIEW_SURFACES.contains(&id) {
        return Some(SupportLevel::Preview);
    }
    if BASELINE_UNSUPPORTED_SURFACES.contains(&id) {
        return Some(SupportLevel::Unsupported);
    }
    None
}

// r[impl ucan.nostr_identity_adapter.support_profile]
fn promotion_review(id: &str) -> Option<PromotionReview> {
    let (to, migration_id) = match id {
        "nostr-identity-adapter" => (SupportLevel::Preview, "add-nostr-identity-adapter"),
        "ucan-wg-1.0.0-authorization" => (SupportLevel::ControlledIntegrationReady, "implement-ucan-1-authorization"),
        _ => return None,
    };
    Some(PromotionReview {
        from: SupportLevel::Unsupported,
        to,
        migration_id: migration_id.to_owned(),
        evidence: REQUIRED_PROMOTION_EVIDENCE.iter().map(ToString::to_string).collect(),
    })
}

/// Validate catalog shape, baseline support levels, and promotion evidence.
#[must_use]
pub fn validate_support_catalog(catalog: &SupportCatalog) -> Vec<ReadinessIssue> {
    let mut issues = Vec::new();
    if catalog.schema != SUPPORT_CATALOG_SCHEMA {
        issues.push(issue("catalog-schema", "catalog.schema", "unsupported support-catalog schema"));
    }
    if catalog.report_schema_version != REPORT_SCHEMA_VERSION {
        issues.push(issue(
            "catalog-report-schema",
            "catalog.report_schema_version",
            "support catalog does not bind the current readiness-report schema",
        ));
    }
    if catalog.crate_level != SupportLevel::ControlledIntegrationReady {
        issues.push(issue(
            "crate-support-level",
            "catalog.crate_level",
            "crate support must remain controlled-integration-ready in this profile",
        ));
    }
    validate_surfaces(catalog, &mut issues);
    validate_catalog_options("did_methods", &catalog.did_methods, &mut issues);
    validate_catalog_options("signing_algorithms", &catalog.signing_algorithms, &mut issues);
    validate_catalog_options("key_classes", &catalog.key_classes, &mut issues);
    validate_integration_vocabulary(catalog, &mut issues);
    validate_limit_ceiling(catalog, &mut issues);
    validate_promotion_policy(catalog, &mut issues);
    validate_required_non_claims(&catalog.required_non_claims, "catalog.required_non_claims", &mut issues);
    issues
}

fn validate_surfaces(catalog: &SupportCatalog, issues: &mut Vec<ReadinessIssue>) {
    validate_unique_ids(
        catalog.surfaces.iter().map(|surface| surface.id.as_str()),
        "catalog.surfaces",
        "duplicate-surface",
        issues,
    );
    for surface in &catalog.surfaces {
        let path = format!("catalog.surfaces.{}", surface.id);
        if !valid_identifier(&surface.id) {
            issues.push(issue("surface-id", &path, "surface id is empty, oversized, or malformed"));
        }
        if surface.evidence.trim().is_empty() || surface.boundary.trim().is_empty() {
            issues.push(issue("surface-evidence", &path, "surface evidence and boundary must be non-empty"));
        }
        validate_surface_transition(surface, &path, &catalog.promotion_policy, issues);
        validate_nostr_adapter_surface(surface, &path, issues);
    }
    for id in BASELINE_CONTROLLED_SURFACES
        .iter()
        .chain(BASELINE_PREVIEW_SURFACES)
        .chain(BASELINE_UNSUPPORTED_SURFACES)
    {
        if !catalog.surfaces.iter().any(|surface| surface.id == *id) {
            issues.push(issue(
                "missing-baseline-surface",
                "catalog.surfaces",
                &format!("baseline surface {id} is missing"),
            ));
        }
    }
}

fn validate_nostr_adapter_surface(surface: &CatalogSurface, path: &str, issues: &mut Vec<ReadinessIssue>) {
    if surface.id != "nostr-identity-adapter" {
        return;
    }
    let evidence_complete = ["NIP-01", "NIP-42", "NIP-98"].iter().all(|required| surface.evidence.contains(required));
    let boundary_complete = NOSTR_ADAPTER_BOUNDARY_FRAGMENTS.iter().all(|required| surface.boundary.contains(required));
    if surface.level != SupportLevel::Preview || !evidence_complete || !boundary_complete {
        issues.push(issue(
            "nostr-adapter-overclaim",
            path,
            "Nostr adapter support must stay preview with exact caller-owned effects",
        ));
    }
}

fn validate_surface_transition(
    surface: &CatalogSurface,
    path: &str,
    policy: &PromotionPolicy,
    issues: &mut Vec<ReadinessIssue>,
) {
    let baseline = surface.baseline_level.unwrap_or(SupportLevel::Unsupported);
    let current_strength = support_strength(surface.level);
    let baseline_strength = support_strength(baseline);
    if current_strength < baseline_strength {
        issues.push(issue("support-regression", path, "surface support is weaker than the recorded baseline"));
        return;
    }
    if current_strength == baseline_strength {
        if surface.promotion_review.is_some() {
            issues.push(issue(
                "unexpected-promotion-review",
                path,
                "unchanged support level must not carry promotion evidence",
            ));
        }
        return;
    }
    let Some(review) = &surface.promotion_review else {
        issues.push(issue(
            "unreviewed-promotion",
            path,
            "stronger support requires explicit migration and evidence review",
        ));
        return;
    };
    if review.from != baseline || review.to != surface.level || !valid_identifier(&review.migration_id) {
        issues.push(issue(
            "promotion-review-binding",
            path,
            "promotion review must bind the baseline, target, and a stable migration id",
        ));
    }
    let supplied = review.evidence.iter().map(String::as_str).collect::<BTreeSet<_>>();
    for required in &policy.required_evidence {
        if !supplied.contains(required.as_str()) {
            issues.push(issue(
                "promotion-evidence",
                path,
                &format!("promotion review is missing evidence class {required}"),
            ));
        }
    }
}

const fn support_strength(level: SupportLevel) -> u8 {
    match level {
        SupportLevel::Unsupported => SUPPORT_STRENGTH_UNSUPPORTED,
        SupportLevel::Preview => SUPPORT_STRENGTH_PREVIEW,
        SupportLevel::ControlledIntegrationReady => SUPPORT_STRENGTH_CONTROLLED,
        SupportLevel::ProductionSupported => SUPPORT_STRENGTH_PRODUCTION,
    }
}

fn validate_catalog_options(group: &str, options: &[CatalogOption], issues: &mut Vec<ReadinessIssue>) {
    validate_unique_ids(
        options.iter().map(|option| option.id.as_str()),
        &format!("catalog.{group}"),
        "duplicate-option",
        issues,
    );
    if options.is_empty() {
        issues.push(issue("empty-option-group", &format!("catalog.{group}"), "option group must not be empty"));
    }
    for option in options {
        let path = format!("catalog.{group}.{}", option.id);
        if option.id.trim().is_empty() || option.evidence.trim().is_empty() || option.boundary.trim().is_empty() {
            issues.push(issue("option-shape", &path, "option id, evidence, and boundary must be non-empty"));
        }
        if option.level == SupportLevel::ProductionSupported {
            issues.push(issue(
                "option-production-overclaim",
                &path,
                "no DID, signature, or key option is production-supported by this release",
            ));
        }
    }
}

fn validate_integration_vocabulary(catalog: &SupportCatalog, issues: &mut Vec<ReadinessIssue>) {
    validate_unique_ids(
        catalog.integration_modes.iter().map(|mode| mode.id.as_str()),
        "catalog.integration_modes",
        "duplicate-integration-mode",
        issues,
    );
    let actual = catalog
        .integration_modes
        .iter()
        .map(|mode| (mode.id.as_str(), mode.admitted_value.as_str()))
        .collect::<BTreeMap<_, _>>();
    for (id, expected) in REQUIRED_INTEGRATION_MODES {
        if actual.get(id).copied() != Some(*expected) {
            issues.push(issue(
                "integration-vocabulary",
                &format!("catalog.integration_modes.{id}"),
                "integration mode is missing or has an unrecognized admitted value",
            ));
        }
    }
}

fn validate_limit_ceiling(catalog: &SupportCatalog, issues: &mut Vec<ReadinessIssue>) {
    let ceiling = &catalog.verification_limit_ceiling;
    if ceiling.id != DEFAULT_LIMIT_PROFILE_ID {
        issues.push(issue(
            "limit-profile-id",
            "catalog.verification_limit_ceiling.id",
            "verification ceiling must retain its stable named profile",
        ));
    }
    if ceiling.max_proof_depth == 0 || ceiling.max_proof_count == 0 || ceiling.max_input_bytes == 0 {
        issues.push(issue(
            "invalid-limit-ceiling",
            "catalog.verification_limit_ceiling",
            "verification ceilings must be finite and non-zero",
        ));
    }
}

fn validate_promotion_policy(catalog: &SupportCatalog, issues: &mut Vec<ReadinessIssue>) {
    let expected_levels = [
        SupportLevel::Unsupported,
        SupportLevel::Preview,
        SupportLevel::ControlledIntegrationReady,
        SupportLevel::ProductionSupported,
    ];
    if catalog.promotion_policy.ordered_levels != expected_levels {
        issues.push(issue(
            "promotion-level-order",
            "catalog.promotion_policy.ordered_levels",
            "promotion policy must preserve the closed ordered support levels",
        ));
    }
    let supplied = catalog.promotion_policy.required_evidence.iter().map(String::as_str).collect::<BTreeSet<_>>();
    for required in REQUIRED_PROMOTION_EVIDENCE {
        if !supplied.contains(required) {
            issues.push(issue(
                "promotion-policy-evidence",
                "catalog.promotion_policy.required_evidence",
                &format!("promotion policy is missing evidence class {required}"),
            ));
        }
    }
}

/// Return compact canonical JSON bytes for a valid support catalog.
///
/// # Errors
///
/// Returns deterministic issues when the catalog is invalid or cannot be serialized.
pub fn canonical_support_catalog_bytes(catalog: &SupportCatalog) -> Result<Vec<u8>, ReadinessIssues> {
    let issues = validate_support_catalog(catalog);
    if !issues.is_empty() {
        return Err(ReadinessIssues { issues });
    }
    serde_json::to_vec(catalog).map_err(|error| ReadinessIssues {
        issues: vec![issue("catalog-serialization", "catalog", &error.to_string())],
    })
}

/// Return the BLAKE3 identity of compact canonical catalog bytes.
///
/// # Errors
///
/// Returns deterministic issues when the catalog is invalid or cannot be serialized.
pub fn support_catalog_blake3(catalog: &SupportCatalog) -> Result<String, ReadinessIssues> {
    canonical_support_catalog_bytes(catalog).map(|bytes| blake3::hash(&bytes).to_hex().to_string())
}

/// Render the generated JSON projection with a trailing newline.
///
/// # Errors
///
/// Returns deterministic issues when the catalog is invalid or cannot be serialized.
pub fn render_support_catalog_json(catalog: &SupportCatalog) -> Result<String, ReadinessIssues> {
    let identity = support_catalog_blake3(catalog)?;
    let projection = SupportCatalogProjection {
        projection_schema: SUPPORT_CATALOG_PROJECTION_SCHEMA.to_owned(),
        catalog_blake3: identity,
        catalog: catalog.clone(),
    };
    serde_json::to_string_pretty(&projection)
        .map(|rendered| format!("{rendered}\n"))
        .map_err(|error| ReadinessIssues {
            issues: vec![issue(
                "projection-serialization",
                "docs/production-readiness.json",
                &error.to_string(),
            )],
        })
}

fn append_formatted(output: &mut String, arguments: fmt::Arguments<'_>) {
    let result = output.write_fmt(arguments);
    debug_assert!(result.is_ok(), "writing to a String must not fail");
}

/// Render the compact generated Nickel support vocabulary.
///
/// # Errors
///
/// Returns deterministic issues when the catalog is invalid.
pub fn render_support_vocabulary(catalog: &SupportCatalog) -> Result<String, ReadinessIssues> {
    let identity = support_catalog_blake3(catalog)?;
    let mut output = String::from("# @generated by check-production-readiness; do not edit.\n{\n");
    append_formatted(&mut output, format_args!("  schema = {SUPPORT_VOCABULARY_SCHEMA:?},\n"));
    append_formatted(&mut output, format_args!("  catalog_blake3 = {identity:?},\n"));
    output.push_str("  support_levels = [\"unsupported\", \"preview\", \"controlled-integration-ready\", \"production-supported\"],\n");
    render_surface_vocabulary(&mut output, &catalog.surfaces);
    render_option_vocabulary(&mut output, "did_methods", &catalog.did_methods);
    render_option_vocabulary(&mut output, "signing_algorithms", &catalog.signing_algorithms);
    render_option_vocabulary(&mut output, "key_classes", &catalog.key_classes);
    append_formatted(
        &mut output,
        format_args!(
            "  verification_limit_ceiling = {{ id = {:?}, max_proof_depth = {}, max_proof_count = {}, max_input_bytes = {} }},\n",
            catalog.verification_limit_ceiling.id,
            catalog.verification_limit_ceiling.max_proof_depth,
            catalog.verification_limit_ceiling.max_proof_count,
            catalog.verification_limit_ceiling.max_input_bytes,
        ),
    );
    output.push_str("  integration_modes = [\n");
    for mode in &catalog.integration_modes {
        append_formatted(
            &mut output,
            format_args!("    {{ id = {:?}, admitted_value = {:?} }},\n", mode.id, mode.admitted_value),
        );
    }
    output.push_str("  ],\n}\n");
    Ok(output)
}

fn render_surface_vocabulary(output: &mut String, surfaces: &[CatalogSurface]) {
    output.push_str("  surfaces = [\n");
    for surface in surfaces {
        append_formatted(
            output,
            format_args!("    {{ id = {:?}, level = {:?} }},\n", surface.id, surface.level.as_str()),
        );
    }
    output.push_str("  ],\n");
}

fn render_option_vocabulary(output: &mut String, name: &str, options: &[CatalogOption]) {
    append_formatted(output, format_args!("  {name} = [\n"));
    for option in options {
        append_formatted(
            output,
            format_args!("    {{ id = {:?}, level = {:?} }},\n", option.id, option.level.as_str()),
        );
    }
    output.push_str("  ],\n");
}

/// Render the exact generated section embedded in the operator readiness document.
///
/// # Errors
///
/// Returns deterministic issues when the catalog is invalid.
pub fn render_operator_support_section(catalog: &SupportCatalog) -> Result<String, ReadinessIssues> {
    let identity = support_catalog_blake3(catalog)?;
    let mut output = format!(
        "{GENERATED_DOC_START}\n\nCatalog BLAKE3: `{identity}`\n\nCatalog schema: `{SUPPORT_CATALOG_SCHEMA}`\n\n"
    );
    output.push_str("| Surface ID | Label | Evidence | Retained boundary |\n");
    output.push_str("| --- | --- | --- | --- |\n");
    for surface in &catalog.surfaces {
        append_formatted(
            &mut output,
            format_args!(
                "| `{}` | `{}` | {} | {} |\n",
                surface.id,
                surface.level.as_str(),
                surface.evidence,
                surface.boundary
            ),
        );
    }
    render_operator_options(&mut output, "DID methods", &catalog.did_methods);
    render_operator_options(&mut output, "Signature algorithms", &catalog.signing_algorithms);
    render_operator_options(&mut output, "Key classes", &catalog.key_classes);
    output.push_str("\n### Generated verification ceiling\n\n");
    append_formatted(
        &mut output,
        format_args!(
            "`{}`: proof depth `{}`, proof count `{}`, compact-token bytes `{}`.\n\n",
            catalog.verification_limit_ceiling.id,
            catalog.verification_limit_ceiling.max_proof_depth,
            catalog.verification_limit_ceiling.max_proof_count,
            catalog.verification_limit_ceiling.max_input_bytes,
        ),
    );
    output.push_str("### Required non-claims\n\n");
    for non_claim in &catalog.required_non_claims {
        append_formatted(&mut output, format_args!("- {non_claim}\n"));
    }
    append_formatted(&mut output, format_args!("\n{GENERATED_DOC_END}"));
    Ok(output)
}

fn render_operator_options(output: &mut String, heading: &str, options: &[CatalogOption]) {
    append_formatted(output, format_args!("\n### {heading}\n\n"));
    output.push_str("| Option | Label | Retained boundary |\n");
    output.push_str("| --- | --- | --- |\n");
    for option in options {
        append_formatted(
            output,
            format_args!("| `{}` | `{}` | {} |\n", option.id, option.level.as_str(), option.boundary),
        );
    }
}

fn validate_profile_protocol(profile: &IntegrationSupportProfile, issues: &mut Vec<ReadinessIssue>) {
    match crate::protocol_profile::admit_protocol_profile_id(profile.protocol_profile.as_str()) {
        Ok(()) => {}
        Err(error) => issues.push(ReadinessIssue {
            code: match error {
                ProtocolProfileAdmissionError::UnknownProfile { .. } => "unknown-protocol-profile".to_owned(),
                ProtocolProfileAdmissionError::UnsupportedProfile { .. } => "unsupported-protocol-profile".to_owned(),
            },
            path: "profile.protocol_profile".to_owned(),
            message: error.to_string(),
        }),
    }
}

/// Admit a reviewed integration profile as a pure subset of the support catalog.
#[must_use]
pub fn admit_integration_profile(
    catalog: &SupportCatalog,
    profile: &IntegrationSupportProfile,
) -> ProfileAdmissionReport {
    let mut issues = validate_support_catalog(catalog);
    let catalog_identity = match support_catalog_blake3(catalog) {
        Ok(identity) => identity,
        Err(error) => {
            issues.extend(error.issues);
            String::new()
        }
    };
    validate_profile_shape(profile, &catalog_identity, &mut issues);
    validate_profile_surfaces(catalog, profile, &mut issues);
    validate_profile_options(catalog, profile, &mut issues);
    validate_profile_limits(catalog, profile, &mut issues);
    validate_profile_integrations(catalog, profile, &mut issues);
    validate_required_non_claims(&profile.non_claims, "profile.non_claims", &mut issues);
    validate_overclaims(&profile.non_claims, "profile.non_claims", &mut issues);
    validate_profile_protocol(profile, &mut issues);
    ProfileAdmissionReport {
        schema: PROFILE_ADMISSION_SCHEMA.to_owned(),
        profile_id: profile.id.clone(),
        catalog_blake3: catalog_identity,
        valid: issues.is_empty(),
        issues,
        non_claims: profile.non_claims.clone(),
    }
}

fn validate_profile_shape(profile: &IntegrationSupportProfile, identity: &str, issues: &mut Vec<ReadinessIssue>) {
    if profile.schema != INTEGRATION_PROFILE_SCHEMA {
        issues.push(issue("profile-schema", "profile.schema", "unsupported integration-profile schema"));
    }
    if !valid_identifier(&profile.id) {
        issues.push(issue("profile-id", "profile.id", "profile id is empty, oversized, or malformed"));
    }
    if !is_blake3_hex(&profile.catalog_blake3) || profile.catalog_blake3 != identity {
        issues.push(issue(
            "catalog-identity",
            "profile.catalog_blake3",
            "profile does not bind the current canonical support catalog",
        ));
    }
    if profile.required_surfaces.is_empty() {
        issues.push(issue(
            "empty-required-surfaces",
            "profile.required_surfaces",
            "profile must select at least one support surface",
        ));
    }
    validate_unique_ids(
        profile.required_surfaces.iter().map(String::as_str),
        "profile.required_surfaces",
        "duplicate-surface",
        issues,
    );
    validate_unique_ids(
        profile.admitted_preview_surfaces.iter().map(String::as_str),
        "profile.admitted_preview_surfaces",
        "duplicate-preview-surface",
        issues,
    );
    if !profile.fail_closed {
        issues.push(issue("fail-open-profile", "profile.fail_closed", "integration profile must fail closed"));
    }
}

fn validate_profile_surfaces(
    catalog: &SupportCatalog,
    profile: &IntegrationSupportProfile,
    issues: &mut Vec<ReadinessIssue>,
) {
    let surfaces = catalog.surfaces.iter().map(|surface| (surface.id.as_str(), surface)).collect::<BTreeMap<_, _>>();
    let selected = profile.required_surfaces.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let previews = profile.admitted_preview_surfaces.iter().map(String::as_str).collect::<BTreeSet<_>>();
    for surface_id in &profile.required_surfaces {
        let path = format!("profile.required_surfaces.{surface_id}");
        let Some(surface) = surfaces.get(surface_id.as_str()) else {
            issues.push(issue("absent-surface", &path, "required surface is absent from the support catalog"));
            continue;
        };
        match surface.level {
            SupportLevel::Unsupported => issues.push(issue(
                "unsupported-surface",
                &path,
                &format!("required surface {surface_id} is unsupported"),
            )),
            SupportLevel::Preview if !previews.contains(surface_id.as_str()) => issues.push(issue(
                "implicit-preview",
                &path,
                &format!("preview surface {surface_id} requires explicit admission"),
            )),
            SupportLevel::Preview | SupportLevel::ControlledIntegrationReady | SupportLevel::ProductionSupported => {}
        }
    }
    for preview_id in &profile.admitted_preview_surfaces {
        let path = format!("profile.admitted_preview_surfaces.{preview_id}");
        if !selected.contains(preview_id.as_str()) {
            issues.push(issue("unused-preview-admission", &path, "preview admission must name a required surface"));
            continue;
        }
        if surfaces.get(preview_id.as_str()).map(|surface| surface.level) != Some(SupportLevel::Preview) {
            issues.push(issue(
                "invalid-preview-admission",
                &path,
                "preview admission must name a catalog preview surface",
            ));
        }
    }
}

fn validate_profile_options(
    catalog: &SupportCatalog,
    profile: &IntegrationSupportProfile,
    issues: &mut Vec<ReadinessIssue>,
) {
    let selections = [
        ("did_methods", profile.did_methods.as_slice(), catalog.did_methods.as_slice()),
        ("signing_algorithms", profile.signing_algorithms.as_slice(), catalog.signing_algorithms.as_slice()),
        ("key_classes", profile.key_classes.as_slice(), catalog.key_classes.as_slice()),
    ];
    debug_assert_eq!(selections.len(), EXPECTED_OPTION_GROUP_COUNT);
    for (group, selected, catalog_options) in selections {
        validate_option_selection(group, selected, catalog_options, issues);
    }
}

fn validate_option_selection(
    group: &str,
    selected: &[String],
    catalog_options: &[CatalogOption],
    issues: &mut Vec<ReadinessIssue>,
) {
    if selected.is_empty() {
        issues.push(issue(
            "empty-option-selection",
            &format!("profile.{group}"),
            "profile option selection must not be empty",
        ));
    }
    validate_unique_ids(
        selected.iter().map(String::as_str),
        &format!("profile.{group}"),
        "duplicate-option-selection",
        issues,
    );
    let options = catalog_options.iter().map(|option| (option.id.as_str(), option.level)).collect::<BTreeMap<_, _>>();
    for selected_id in selected {
        let path = format!("profile.{group}.{selected_id}");
        match options.get(selected_id.as_str()) {
            None => issues.push(issue("unknown-option", &path, "selected option is absent from the support catalog")),
            Some(SupportLevel::Unsupported) => {
                issues.push(issue("unsupported-option", &path, "selected option is unsupported"));
            }
            Some(SupportLevel::Preview) => {
                issues.push(issue("preview-option", &path, "preview DID, signature, and key options are not admitted"));
            }
            Some(SupportLevel::ControlledIntegrationReady | SupportLevel::ProductionSupported) => {}
        }
    }
}

fn validate_profile_limits(
    catalog: &SupportCatalog,
    profile: &IntegrationSupportProfile,
    issues: &mut Vec<ReadinessIssue>,
) {
    let selected = &profile.verification_limits;
    let ceiling = &catalog.verification_limit_ceiling;
    if !valid_identifier(&selected.id) {
        issues.push(issue("limit-profile-id", "profile.verification_limits.id", "limit profile id is malformed"));
    }
    if selected.max_proof_depth == 0 || selected.max_proof_count == 0 || selected.max_input_bytes == 0 {
        issues.push(issue(
            "invalid-limit",
            "profile.verification_limits",
            "all verification limits must be finite and non-zero",
        ));
    }
    if selected.max_proof_depth > ceiling.max_proof_depth
        || selected.max_proof_count > ceiling.max_proof_count
        || selected.max_input_bytes > ceiling.max_input_bytes
    {
        issues.push(issue(
            "limit-exceeds-catalog",
            "profile.verification_limits",
            "profile verification limits exceed the bound catalog ceiling",
        ));
    }
}

fn validate_profile_integrations(
    catalog: &SupportCatalog,
    profile: &IntegrationSupportProfile,
    issues: &mut Vec<ReadinessIssue>,
) {
    let admitted = catalog
        .integration_modes
        .iter()
        .map(|mode| (mode.id.as_str(), mode.admitted_value.as_str()))
        .collect::<BTreeMap<_, _>>();
    for (id, actual) in profile.integrations.pairs() {
        if admitted.get(id).copied() != Some(actual) {
            issues.push(issue(
                "missing-integration",
                &format!("profile.integrations.{id}"),
                "profile omits or weakens an explicit caller-owned integration mode",
            ));
        }
    }
}

fn validate_required_non_claims(non_claims: &[String], path: &str, issues: &mut Vec<ReadinessIssue>) {
    validate_unique_ids(non_claims.iter().map(String::as_str), path, "duplicate-non-claim", issues);
    let supplied = non_claims.iter().map(String::as_str).collect::<BTreeSet<_>>();
    for required in REQUIRED_NON_CLAIMS {
        if !supplied.contains(required) {
            issues.push(issue("weakened-non-claims", path, &format!("missing required non-claim: {required}")));
        }
    }
}

fn validate_overclaims(non_claims: &[String], path: &str, issues: &mut Vec<ReadinessIssue>) {
    for claim in non_claims {
        let normalized = claim.to_ascii_lowercase();
        if (normalized.contains("proves") || normalized.contains("production-ready"))
            && !normalized.contains("does not prove")
        {
            issues.push(issue(
                "production-overclaim",
                path,
                "profile contains an affirmative correctness or production claim",
            ));
        }
    }
}

/// Detect secret-like field names or string values in an exported fixture value.
#[must_use]
pub fn fixture_value_has_secret_like(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(fields) => fields
            .iter()
            .any(|(key, nested)| contains_secret_fragment(key) || fixture_value_has_secret_like(nested)),
        serde_json::Value::Array(items) => items.iter().any(fixture_value_has_secret_like),
        serde_json::Value::String(text) => contains_secret_fragment(text),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => false,
    }
}

fn contains_secret_fragment(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    SECRET_LIKE_FRAGMENTS.iter().any(|fragment| normalized.contains(fragment))
}

/// Validate a public fixture manifest against an admitted profile and catalog.
#[must_use]
pub fn validate_fixture_manifest(
    catalog: &SupportCatalog,
    profile: &IntegrationSupportProfile,
    manifest: &IntegrationFixtureManifest,
) -> FixtureValidationReport {
    let mut issues = Vec::new();
    if manifest.schema != FIXTURE_MANIFEST_SCHEMA {
        issues.push(issue("fixture-schema", "fixture.schema", "unsupported fixture-manifest schema"));
    }
    if !valid_identifier(&manifest.id) {
        issues.push(issue("fixture-id", "fixture.id", "fixture id is empty, oversized, or malformed"));
    }
    if manifest.profile_id != profile.id {
        issues.push(issue("fixture-profile", "fixture.profile_id", "fixture does not bind the admitted profile"));
    }
    if !is_blake3_hex(&manifest.public_input_blake3) {
        issues.push(issue("fixture-input-identity", "fixture.public_input_blake3", "public input BLAKE3 is malformed"));
    }
    if !is_blake3_hex(&manifest.public_reference_blake3) {
        issues.push(issue(
            "fixture-reference-identity",
            "fixture.public_reference_blake3",
            "public reference BLAKE3 is malformed",
        ));
    }
    if !matches!(manifest.expected_result.as_str(), "allow" | "deny" | "error") {
        issues.push(issue(
            "fixture-result-class",
            "fixture.expected_result",
            "expected result must be allow, deny, or error",
        ));
    }
    validate_fixture_surfaces(catalog, profile, manifest, &mut issues);
    validate_required_non_claims(&manifest.non_claims, "fixture.non_claims", &mut issues);
    validate_fixture_secrets(manifest, &mut issues);
    FixtureValidationReport {
        schema: FIXTURE_VALIDATION_SCHEMA.to_owned(),
        fixture_id: manifest.id.clone(),
        valid: issues.is_empty(),
        issues,
    }
}

fn validate_fixture_surfaces(
    catalog: &SupportCatalog,
    profile: &IntegrationSupportProfile,
    manifest: &IntegrationFixtureManifest,
    issues: &mut Vec<ReadinessIssue>,
) {
    validate_unique_ids(
        manifest.required_surfaces.iter().map(String::as_str),
        "fixture.required_surfaces",
        "duplicate-fixture-surface",
        issues,
    );
    let catalog_ids = catalog.surfaces.iter().map(|surface| surface.id.as_str()).collect::<BTreeSet<_>>();
    let profile_ids = profile.required_surfaces.iter().map(String::as_str).collect::<BTreeSet<_>>();
    for surface in &manifest.required_surfaces {
        if !catalog_ids.contains(surface.as_str()) || !profile_ids.contains(surface.as_str()) {
            issues.push(issue(
                "fixture-surface",
                &format!("fixture.required_surfaces.{surface}"),
                "fixture surface must be present in both the catalog and bound profile",
            ));
        }
    }
}

fn validate_fixture_secrets(manifest: &IntegrationFixtureManifest, issues: &mut Vec<ReadinessIssue>) {
    let values = [
        manifest.id.as_str(),
        manifest.profile_id.as_str(),
        manifest.expected_result.as_str(),
    ];
    for value in values {
        if contains_secret_fragment(value) {
            issues.push(issue("fixture-secret", "fixture", "fixture metadata contains a secret-like field or value"));
        }
    }
}

fn validate_unique_ids<'a>(
    values: impl IntoIterator<Item = &'a str>,
    path: &str,
    code: &str,
    issues: &mut Vec<ReadinessIssue>,
) {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            issues.push(issue(code, path, &format!("duplicate value {value}")));
        }
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'/'))
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH && value.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn issue(code: &str, path: &str, message: &str) -> ReadinessIssue {
    ReadinessIssue {
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LIMIT: u64 = 1;
    const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    const PUBLIC_INPUT_DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const PUBLIC_REFERENCE_DIGEST: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    fn valid_profile(catalog: &SupportCatalog) -> IntegrationSupportProfile {
        let catalog_blake3 = support_catalog_blake3(catalog).unwrap_or_default();
        IntegrationSupportProfile {
            schema: INTEGRATION_PROFILE_SCHEMA.to_owned(),
            id: "tests.controlled".to_owned(),
            catalog_blake3,
            required_surfaces: vec![
                "ed25519-did-key-compact-token".to_owned(),
                "caller-supplied-key-resolution".to_owned(),
                "caller-supplied-proof-store".to_owned(),
                "caller-supplied-revocation".to_owned(),
                "caller-supplied-replay-admission".to_owned(),
                "holder-signed-invocation-admission".to_owned(),
                "opaque-caveat-policy-evaluation".to_owned(),
            ],
            admitted_preview_surfaces: Vec::new(),
            did_methods: vec!["did:key".to_owned()],
            signing_algorithms: vec!["EdDSA".to_owned()],
            key_classes: vec!["Ed25519".to_owned()],
            verification_limits: VerificationLimitProfile {
                id: "tests-bounded".to_owned(),
                max_proof_depth: TEST_LIMIT,
                max_proof_count: TEST_LIMIT,
                max_input_bytes: TEST_LIMIT,
            },
            integrations: IntegrationModes {
                key_resolution: "caller-supplied".to_owned(),
                proof_loading: "caller-supplied".to_owned(),
                revocation: "caller-supplied".to_owned(),
                replay: "caller-supplied".to_owned(),
                verification_time: "explicit".to_owned(),
                caveat_policy: "caller-supplied".to_owned(),
                signer_custody: "caller-owned".to_owned(),
                backend_failure: "fail-closed".to_owned(),
            },
            fail_closed: true,
            non_claims: REQUIRED_NON_CLAIMS.iter().map(ToString::to_string).collect(),
            protocol_profile: crate::protocol_profile::ONIX_COMPACT_ED25519_V1.to_owned(),
        }
    }

    fn issue_codes(report: &ProfileAdmissionReport) -> BTreeSet<&str> {
        report.issues.iter().map(|entry| entry.code.as_str()).collect()
    }

    #[test]
    fn canonical_catalog_bytes_and_identity_are_stable() {
        let catalog = support_catalog();
        let first = canonical_support_catalog_bytes(&catalog);
        let second = canonical_support_catalog_bytes(&catalog);
        assert_eq!(first, second);
        let identity = support_catalog_blake3(&catalog).unwrap_or_default();
        assert_eq!(identity.len(), BLAKE3_HEX_LENGTH);
    }

    #[test]
    fn catalog_rejects_duplicate_surface_and_unreviewed_promotion() {
        let mut catalog = support_catalog();
        catalog.surfaces.push(catalog.surfaces[0].clone());
        let controlled = catalog.surfaces.iter_mut().find(|surface| surface.id == "remote-did-resolution");
        if let Some(surface) = controlled {
            surface.level = SupportLevel::ControlledIntegrationReady;
        }
        let issues = validate_support_catalog(&catalog);
        assert!(issues.iter().any(|entry| entry.code == "duplicate-surface"));
        assert!(issues.iter().any(|entry| entry.code == "unreviewed-promotion"));
    }

    #[test]
    fn controlled_profile_is_admitted() {
        let catalog = support_catalog();
        let report = admit_integration_profile(&catalog, &valid_profile(&catalog));
        assert!(report.valid, "{:#?}", report.issues);
    }

    #[test]
    fn profile_with_local_protocol_profile_is_admitted() {
        let catalog = support_catalog();
        let mut profile = valid_profile(&catalog);
        profile.protocol_profile = crate::protocol_profile::ONIX_COMPACT_ED25519_V1.to_owned();
        let report = admit_integration_profile(&catalog, &profile);
        assert!(report.valid, "{:#?}", report.issues);
    }

    #[test]
    fn profile_with_final_protocol_profile_is_admitted() {
        let catalog = support_catalog();
        let mut profile = valid_profile(&catalog);
        profile.protocol_profile = crate::protocol_profile::UCAN_WG_1_0_0.to_owned();
        let report = admit_integration_profile(&catalog, &profile);
        assert!(report.valid, "{:#?}", report.issues);
    }

    #[test]
    fn profile_with_unknown_protocol_profile_is_rejected() {
        let catalog = support_catalog();
        let mut profile = valid_profile(&catalog);
        profile.protocol_profile = "hummingbird-v42".to_owned();
        let report = admit_integration_profile(&catalog, &profile);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "unknown-protocol-profile"));
    }

    #[test]
    fn explicit_preview_profile_is_admitted() {
        let catalog = support_catalog();
        let mut profile = valid_profile(&catalog);
        profile.required_surfaces.push("finite-typed-caveat-helpers".to_owned());
        profile.admitted_preview_surfaces.push("finite-typed-caveat-helpers".to_owned());
        let report = admit_integration_profile(&catalog, &profile);
        assert!(report.valid, "{:#?}", report.issues);
    }

    #[test]
    fn unsupported_surface_and_option_fail_closed() {
        let catalog = support_catalog();
        let mut profile = valid_profile(&catalog);
        profile.required_surfaces.push("proof-transport-backends".to_owned());
        profile.did_methods = vec!["remote-did-method".to_owned()];
        let report = admit_integration_profile(&catalog, &profile);
        let codes = issue_codes(&report);
        assert!(codes.contains("unsupported-surface"));
        assert!(codes.contains("unsupported-option"));
    }

    #[test]
    fn implicit_preview_stale_identity_and_duplicate_surface_are_rejected() {
        let catalog = support_catalog();
        let mut profile = valid_profile(&catalog);
        profile.catalog_blake3 = ZERO_DIGEST.to_owned();
        profile.required_surfaces.push("finite-typed-caveat-helpers".to_owned());
        profile.required_surfaces.push("finite-typed-caveat-helpers".to_owned());
        let report = admit_integration_profile(&catalog, &profile);
        let codes = issue_codes(&report);
        assert!(codes.contains("catalog-identity"));
        assert!(codes.contains("duplicate-surface"));
        assert!(codes.contains("implicit-preview"));
    }

    #[test]
    fn invalid_limits_missing_integration_and_weakened_nonclaims_are_rejected() {
        let catalog = support_catalog();
        let mut profile = valid_profile(&catalog);
        profile.verification_limits.max_proof_depth = 0;
        profile.integrations.replay = "ambient".to_owned();
        profile.non_claims.clear();
        let report = admit_integration_profile(&catalog, &profile);
        let codes = issue_codes(&report);
        assert!(codes.contains("invalid-limit"));
        assert!(codes.contains("missing-integration"));
        assert!(codes.contains("weakened-non-claims"));
    }

    // r[verify ucan.nostr_identity_adapter.support_profile]
    #[test]
    fn nostr_surface_rejects_effect_and_support_overclaim() {
        let mut catalog = support_catalog();
        let surface = catalog
            .surfaces
            .iter_mut()
            .find(|surface| surface.id == "nostr-identity-adapter")
            .expect("Nostr surface must exist");
        surface.level = SupportLevel::ControlledIntegrationReady;
        surface.boundary = "built-in relay, DNS, replay, and authorization".to_owned();
        let issues = validate_support_catalog(&catalog);

        assert!(issues.iter().any(|issue| issue.code == "nostr-adapter-overclaim"));
    }

    #[test]
    fn production_overclaim_is_rejected() {
        let catalog = support_catalog();
        let mut profile = valid_profile(&catalog);
        profile.non_claims.push("This profile proves production readiness.".to_owned());
        let report = admit_integration_profile(&catalog, &profile);
        assert!(issue_codes(&report).contains("production-overclaim"));
    }

    #[test]
    fn public_fixture_is_admitted_and_secret_like_fixture_is_rejected() {
        let catalog = support_catalog();
        let profile = valid_profile(&catalog);
        let mut manifest = IntegrationFixtureManifest {
            schema: FIXTURE_MANIFEST_SCHEMA.to_owned(),
            id: "tests.public-allow".to_owned(),
            profile_id: profile.id.clone(),
            public_input_blake3: PUBLIC_INPUT_DIGEST.to_owned(),
            public_reference_blake3: PUBLIC_REFERENCE_DIGEST.to_owned(),
            expected_result: "allow".to_owned(),
            required_surfaces: vec!["ed25519-did-key-compact-token".to_owned()],
            non_claims: REQUIRED_NON_CLAIMS.iter().map(ToString::to_string).collect(),
        };
        let accepted = validate_fixture_manifest(&catalog, &profile, &manifest);
        assert!(accepted.valid, "{:#?}", accepted.issues);
        manifest.id = "tests.bearer-token".to_owned();
        let rejected = validate_fixture_manifest(&catalog, &profile, &manifest);
        assert!(rejected.issues.iter().any(|entry| entry.code == "fixture-secret"));
        let unknown_secret_field = serde_json::json!({
            "schema": FIXTURE_MANIFEST_SCHEMA,
            "private_key": "not-public",
        });
        assert!(fixture_value_has_secret_like(&unknown_secret_field));
    }

    #[test]
    fn rendered_artifacts_bind_the_same_catalog_identity() {
        let catalog = support_catalog();
        let identity = support_catalog_blake3(&catalog).unwrap_or_default();
        let json = render_support_catalog_json(&catalog).unwrap_or_default();
        let vocabulary = render_support_vocabulary(&catalog).unwrap_or_default();
        let docs = render_operator_support_section(&catalog).unwrap_or_default();
        assert!(json.contains(&identity));
        assert!(vocabulary.contains(&identity));
        assert!(docs.contains(&identity));
    }
}
