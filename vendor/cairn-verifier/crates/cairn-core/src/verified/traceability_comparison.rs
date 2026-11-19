//! Declared comparison qualification for structural traceability.
//!
//! This module validates one versioned comparison fixture set and builds a
//! report-only receipt. Upstream observations are declared from a pinned
//! source identity. They are never treated as an execution result, and they
//! never become a second production authority. Native observations are
//! supplied by the shell after it executes the ordinary trace-index commands
//! on saved fixture copies.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const BUOY_COMPARISON_MANIFEST_SCHEMA_V1: &str = "cairn.buoy-comparison.fixtures.v1";
pub const BUOY_COMPARISON_RECEIPT_SCHEMA_V1: &str = "cairn.buoy-comparison.receipt.v1";
pub const BUOY_COMPARISON_BOUNDARY: &str = "a valid comparison receipt proves only that executed Cairn trace-index observations match independently declared expectations for saved fixture copies and that declared upstream observations satisfy the pinned projection contract; it does not prove Buoy execution, cross-implementation agreement, requirement satisfaction, implementation correctness, test adequacy, proof soundness, release eligibility, lifecycle readiness, or behavioral correctness";
pub const BUOY_COMPARISON_NON_CLAIMS: &[&str] = &[
    "not upstream execution",
    "not cross-implementation agreement proof",
    "not requirement satisfaction",
    "not implementation correctness",
    "not test adequacy",
    "not proof soundness",
    "not release eligibility",
    "not behavioral correctness",
];

pub const BUOY_COMPARISON_SEMANTIC_CURRENT: &str = "current";
pub const BUOY_COMPARISON_SEMANTIC_STALE_REQUIREMENT: &str = "stale_requirement";
pub const BUOY_COMPARISON_SEMANTIC_STALE_ITEM: &str = "stale_item";
pub const BUOY_COMPARISON_SEMANTIC_MISSING_OBJECT: &str = "missing_object";
pub const BUOY_COMPARISON_SEMANTIC_AMBIGUOUS: &str = "ambiguous";
pub const BUOY_COMPARISON_SEMANTIC_UNSUPPORTED: &str = "unsupported";
pub const BUOY_COMPARISON_SEMANTIC_INVALID_OUTPUT: &str = "invalid_output";

pub const BUOY_COMPARISON_MODE_EXPORT_AND_VERIFY: &str = "export_and_verify";
pub const BUOY_COMPARISON_MODE_VERIFY_BASE_INDEX: &str = "verify_base_index";
pub const BUOY_COMPARISON_MODE_REJECT_BASE_INDEX: &str = "reject_base_index";
pub const BUOY_COMPARISON_MODE_NOT_EXECUTED: &str = "not_executed";

pub const BUOY_COMPARISON_DISPOSITION_AGREEMENT: &str = "agreement";
pub const BUOY_COMPARISON_DISPOSITION_EXPECTED_DIVERGENCE: &str = "expected_divergence";
pub const BUOY_COMPARISON_DISPOSITION_UNSUPPORTED: &str = "comparison_unsupported";

pub const BUOY_OBSERVATION_DECLARED_NOT_EXECUTED: &str = "declared_not_executed";
pub const BUOY_OBSERVATION_DECLARED_UNSUPPORTED: &str = "declared_unsupported";

pub const BUOY_COMPARISON_INDEX_MUTATION_SCHEMA_VERSION: &str = "schema_version";

const REVISION_HEX_LENGTH: usize = 40;
const IDENTITY_DOMAIN: &str = "cairn.buoy-comparison.identity.v1";
const MANIFEST_DOMAIN: &str = "cairn.buoy-comparison.manifest.v1";
const NATIVE_REPORT_DOMAIN: &str = "cairn.buoy-comparison.native-report.v1";
const BUOY_REPORT_DOMAIN: &str = "cairn.buoy-comparison.buoy-report.v1";
const FIXTURE_TREE_DOMAIN: &str = "cairn.buoy-comparison.fixture-tree.v1";
const NATIVE_RECEIPT_SCHEMA: &str = "cairn.trace-index.verify-receipt.v1";
const MAX_CASE_COUNT: usize = 32;
const MAX_STRING_BYTES: usize = 4_096;

const ISSUE_SCHEMA_UNSUPPORTED: &str = "comparison.schema_unsupported";
const ISSUE_UPSTREAM_INVALID: &str = "comparison.upstream_invalid";
const ISSUE_MAPPING_INVALID: &str = "comparison.mapping_invalid";
const ISSUE_DECISION_INVALID: &str = "comparison.decision_invalid";
const ISSUE_CASE_INVALID: &str = "comparison.case_invalid";
const ISSUE_CASE_DUPLICATE: &str = "comparison.case_duplicate";
const ISSUE_FIXTURE_UNSAFE: &str = "comparison.fixture_path_unsafe";
const ISSUE_FIXTURE_MISSING: &str = "comparison.fixture_identity_missing";
const ISSUE_EXPECTATION_INVALID: &str = "comparison.native_expectation_invalid";
const ISSUE_EXECUTION_MISMATCH: &str = "comparison.native_execution_mismatch";
const ISSUE_OBSERVATION_MISSING: &str = "comparison.native_observation_missing";
const ISSUE_OBSERVATION_UNEXPECTED: &str = "comparison.native_observation_unexpected";
const ISSUE_BUOY_EXECUTED_CLAIM: &str = "comparison.upstream_execution_claimed";
const ISSUE_DISPOSITION_INVALID: &str = "comparison.disposition_invalid";
const ISSUE_DIVERGENCE_INVALID: &str = "comparison.divergence_invalid";
const ISSUE_MUTATION_INVALID: &str = "comparison.mutation_invalid";
const ISSUE_MUTATION_RESTATED: &str = "comparison.mutation_restated";
const ISSUE_COVERAGE_MISSING: &str = "comparison.coverage_missing";
const ISSUE_EMPTY_EXECUTION: &str = "comparison.execution_empty";
const ISSUE_RESTATED_OBSERVATION: &str = "comparison.restated_observation_accepted";
const ISSUE_RESTATED_NATIVE: &str = "comparison.restated_native_expectation_accepted";
const ISSUE_NOT_EXECUTED_PASS: &str = "comparison.not_executed_passed";

const NATIVE_ISSUE_REGISTRY_STALE: &str = "trace_index.registry_stale";
const NATIVE_ISSUE_POLICY_STALE: &str = "trace_index.policy_stale";
const NATIVE_ISSUE_SOURCE_STALE: &str = "trace_index.source_stale";
const NATIVE_ISSUE_SOURCE_UNEXPECTED: &str = "trace_index.source_unexpected";
const NATIVE_ISSUE_LINK_STALE: &str = "trace_index.link_stale";
const NATIVE_ISSUE_SOURCE_MISSING: &str = "trace_index.source_missing";
const NATIVE_ISSUE_MARKER_DANGLING: &str = "trace_index.marker_dangling";
const NATIVE_ISSUE_LINK_DUPLICATE: &str = "trace_index.link_duplicate";
const NATIVE_ISSUE_SOURCE_DUPLICATE: &str = "trace_index.source_duplicate";
const NATIVE_ISSUE_SCHEMA_UNSUPPORTED: &str = "trace_index.schema_unsupported";
const NATIVE_ISSUE_PROFILE_UNSUPPORTED: &str = "trace_index.profile_unsupported";
const NATIVE_ISSUE_PROFILE_STALE: &str = "trace_index.profile_stale";
const NATIVE_ISSUE_ORDER_INVALID: &str = "trace_index.order_invalid";

/// One pinned upstream identity. The comparison never fetches, builds, or
/// executes this identity at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuoyUpstreamPin {
    pub producer: String,
    pub repository: String,
    pub revision: String,
    pub source_content_hash: String,
    pub source_content_hash_algorithm: String,
    pub source_content_hash_mode: String,
    pub consumer_package: String,
    pub rust_toolchain: String,
    pub digest_feature: String,
    pub commands: Vec<String>,
    #[serde(default)]
    pub command_behavior: Vec<String>,
    pub report_schemas: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub artifact: BuoyUpstreamArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuoyUpstreamArtifact {
    pub available: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuoyComparisonMapping {
    pub id: String,
    pub semantic_subset: String,
    pub projection_version: String,
    pub original_authority: String,
    pub preserves_original_identities: bool,
    pub external_projection_is_authority: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuoyComparisonDecision {
    pub disposition: String,
    pub runtime_adoption: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuoyComparisonCaseExecution {
    pub mode: String,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub registry_source: Option<String>,
    #[serde(default)]
    pub index_mutation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuoyComparisonMutation {
    pub of: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuoyNativeExpectation {
    pub exit: u8,
    pub valid: bool,
    #[serde(default)]
    pub issue_codes: Vec<String>,
    #[serde(default)]
    pub stderr_contains: Option<String>,
    pub semantic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuoyDeclaredObservation {
    pub disposition: String,
    pub semantic: String,
    pub report_schema: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuoyComparisonCase {
    pub id: String,
    #[serde(default)]
    pub fixture: Option<String>,
    pub execution: BuoyComparisonCaseExecution,
    #[serde(default)]
    pub mutation: Option<BuoyComparisonMutation>,
    #[serde(default)]
    pub native_expectation: Option<BuoyNativeExpectation>,
    pub buoy_observation: BuoyDeclaredObservation,
    pub comparison: String,
    #[serde(default)]
    pub divergence: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuoyComparisonManifest {
    pub schema: String,
    pub upstream: BuoyUpstreamPin,
    pub mapping: BuoyComparisonMapping,
    pub decision: BuoyComparisonDecision,
    pub cases: Vec<BuoyComparisonCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuoyComparisonIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

impl BuoyComparisonIssue {
    fn new(code: &str, field_path: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            field_path: field_path.to_string(),
            message: message.to_string(),
        }
    }
}

/// One native execution captured by the shell for one executed case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuoyComparisonObservation {
    pub case_id: String,
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuoyComparisonInputs {
    pub manifest_blake3: String,
    pub policy_blake3: String,
    pub fixture_blake3: BTreeMap<String, String>,
    pub observations: Vec<BuoyComparisonObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuoyComparisonEvaluation {
    pub valid: bool,
    pub receipt: Value,
    pub issues: Vec<BuoyComparisonIssue>,
}

const REQUIRED_MUTATION_KINDS: &[&str] = &[
    "missing_marker",
    "duplicate_marker",
    "moved_span",
    "changed_requirement_body",
    "schema_version_drift",
];

const MUTATION_ALLOWED_SEMANTICS: &[(&str, &[&str])] = &[
    (
        "missing_marker",
        &[
            BUOY_COMPARISON_SEMANTIC_STALE_ITEM,
            BUOY_COMPARISON_SEMANTIC_MISSING_OBJECT,
        ],
    ),
    (
        "duplicate_marker",
        &[
            BUOY_COMPARISON_SEMANTIC_STALE_ITEM,
            BUOY_COMPARISON_SEMANTIC_AMBIGUOUS,
        ],
    ),
    ("moved_span", &[BUOY_COMPARISON_SEMANTIC_STALE_ITEM]),
    (
        "changed_requirement_body",
        &[BUOY_COMPARISON_SEMANTIC_STALE_REQUIREMENT],
    ),
    (
        "schema_version_drift",
        &[
            BUOY_COMPARISON_SEMANTIC_UNSUPPORTED,
            BUOY_COMPARISON_SEMANTIC_INVALID_OUTPUT,
        ],
    ),
];

fn issue(code: &str, field_path: &str, message: &str) -> BuoyComparisonIssue {
    BuoyComparisonIssue::new(code, field_path, message)
}

pub fn parse_buoy_comparison_manifest_value(
    value: &Value,
) -> Result<BuoyComparisonManifest, BuoyComparisonIssue> {
    let manifest: BuoyComparisonManifest =
        serde_json::from_value(value.clone()).map_err(|error| {
            issue(
                ISSUE_SCHEMA_UNSUPPORTED,
                "manifest",
                &format!("comparison manifest does not match the closed schema: {error}"),
            )
        })?;
    if manifest.schema != BUOY_COMPARISON_MANIFEST_SCHEMA_V1 {
        return Err(issue(
            ISSUE_SCHEMA_UNSUPPORTED,
            "schema",
            "comparison manifest schema version is unsupported",
        ));
    }
    Ok(manifest)
}

pub fn buoy_comparison_manifest_blake3(bytes: &[u8]) -> String {
    domain_hash(MANIFEST_DOMAIN, bytes)
}

pub fn buoy_comparison_fixture_tree_blake3(files: &[(String, Vec<u8>)]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(FIXTURE_TREE_DOMAIN.as_bytes());
    hasher.update(&[0]);
    let mut sorted = files.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.0.cmp(&right.0));
    for (path, bytes) in sorted {
        update_length_prefixed(&mut hasher, path.as_bytes());
        update_length_prefixed(&mut hasher, bytes);
    }
    hasher.finalize().to_hex().to_string()
}

pub fn validate_buoy_comparison_manifest(
    manifest: &BuoyComparisonManifest,
) -> Vec<BuoyComparisonIssue> {
    let mut issues = Vec::new();
    validate_upstream(&manifest.upstream, &mut issues);
    validate_mapping(&manifest.mapping, &mut issues);
    validate_decision(&manifest.decision, &mut issues);
    validate_cases(manifest, &mut issues);

    // r[verify cairn.trace_index.validation]
    //
    // The comparator must reject a manifest that restates a base observation
    // or the base native expectation for a declared mutation. This is the
    // executable mutation qualification for the comparison itself.
    if issues.is_empty() {
        let mut restated_observations = manifest.clone();
        let mut restated_native = manifest.clone();
        for case in &manifest.cases {
            let Some(mutation) = &case.mutation else {
                continue;
            };
            let Some(base) = manifest
                .cases
                .iter()
                .find(|candidate| candidate.id == mutation.of)
            else {
                continue;
            };
            if let Some(target) = restated_observations
                .cases
                .iter_mut()
                .find(|candidate| candidate.id == case.id)
            {
                target.buoy_observation.semantic = base.buoy_observation.semantic.clone();
                target.comparison = BUOY_COMPARISON_DISPOSITION_EXPECTED_DIVERGENCE.to_string();
                target.divergence = Some("restated_observation".to_string());
            }
            if let (Some(target), Some(base_expectation)) = (
                restated_native
                    .cases
                    .iter_mut()
                    .find(|candidate| candidate.id == case.id),
                base.native_expectation.as_ref(),
            ) {
                target.native_expectation = Some(base_expectation.clone());
                target.buoy_observation.semantic = base.buoy_observation.semantic.clone();
                target.comparison = BUOY_COMPARISON_DISPOSITION_AGREEMENT.to_string();
                target.divergence = None;
            }
        }
        if validate_cases_only(&restated_observations).is_empty() {
            issues.push(issue(
                ISSUE_RESTATED_OBSERVATION,
                "cases",
                "comparison accepted a mutation observation restated from its base case",
            ));
        }
        if validate_cases_only(&restated_native).is_empty() {
            issues.push(issue(
                ISSUE_RESTATED_NATIVE,
                "cases",
                "comparison accepted a mutation native expectation restated from its base case",
            ));
        }
    }
    issues
}

fn validate_upstream(pin: &BuoyUpstreamPin, issues: &mut Vec<BuoyComparisonIssue>) {
    if pin.producer != "buoy" {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.producer",
            "comparison upstream producer must be buoy",
        ));
    }
    if !value_is_nonempty(&pin.repository) {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.repository",
            "comparison upstream repository must be nonempty",
        ));
    }
    if pin.revision.len() != REVISION_HEX_LENGTH
        || !pin
            .revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.revision",
            "comparison upstream revision must be one lowercase 40-hex commit identity",
        ));
    }
    if !pin.source_content_hash.starts_with("sha256-")
        || pin.source_content_hash.len() <= "sha256-".len()
    {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.source_content_hash",
            "comparison upstream source content hash must be one sha256 SRI value",
        ));
    }
    if pin.source_content_hash_algorithm != "sha256" {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.source_content_hash_algorithm",
            "comparison upstream source content hash algorithm must be sha256",
        ));
    }
    if pin.source_content_hash_mode != "recursive-nar" {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.source_content_hash_mode",
            "comparison upstream source content hash mode must be recursive-nar",
        ));
    }
    if !value_is_nonempty(&pin.consumer_package) {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.consumer_package",
            "comparison upstream consumer package must be named",
        ));
    }
    if !value_is_nonempty(&pin.rust_toolchain) {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.rust_toolchain",
            "comparison upstream Rust toolchain must be pinned",
        ));
    }
    if pin.digest_feature != "blake3-256" {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.digest_feature",
            "comparison upstream digest feature must select blake3-256",
        ));
    }
    if pin.commands.is_empty()
        || pin
            .commands
            .iter()
            .any(|command| !matches!(command.as_str(), "static" | "trace" | "package-identity"))
    {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.commands",
            "comparison upstream commands must stay within read-only static, trace, and package-identity inspection",
        ));
    }
    if pin.command_behavior.is_empty()
        || pin
            .command_behavior
            .iter()
            .any(|value| !value_is_nonempty(value))
    {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.command_behavior",
            "comparison upstream read-only command behavior must be stated",
        ));
    }
    if pin.report_schemas.is_empty()
        || pin
            .report_schemas
            .iter()
            .any(|schema| !value_is_nonempty(schema))
    {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.report_schemas",
            "comparison upstream report schemas must be named",
        ));
    }
    if pin.evidence.is_empty() || pin.evidence.iter().any(|value| !value_is_nonempty(value)) {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.evidence",
            "comparison upstream source identity evidence must be retained",
        ));
    }
    if !value_is_nonempty(&pin.artifact.reason) {
        issues.push(issue(
            ISSUE_UPSTREAM_INVALID,
            "upstream.artifact.reason",
            "comparison upstream artifact availability reason must be stated",
        ));
    }
}

fn validate_mapping(mapping: &BuoyComparisonMapping, issues: &mut Vec<BuoyComparisonIssue>) {
    for (field, value) in [
        ("mapping.id", mapping.id.as_str()),
        ("mapping.semantic_subset", mapping.semantic_subset.as_str()),
        (
            "mapping.projection_version",
            mapping.projection_version.as_str(),
        ),
        (
            "mapping.original_authority",
            mapping.original_authority.as_str(),
        ),
    ] {
        if !value_is_nonempty(value) {
            issues.push(issue(
                ISSUE_MAPPING_INVALID,
                field,
                "comparison mapping field must be nonempty",
            ));
        }
    }
    if !mapping.preserves_original_identities {
        issues.push(issue(
            ISSUE_MAPPING_INVALID,
            "mapping.preserves_original_identities",
            "comparison projection must preserve original Cairn identities",
        ));
    }
    if mapping.external_projection_is_authority {
        issues.push(issue(
            ISSUE_MAPPING_INVALID,
            "mapping.external_projection_is_authority",
            "comparison projection must not become a second production authority",
        ));
    }
}

fn validate_decision(decision: &BuoyComparisonDecision, issues: &mut Vec<BuoyComparisonIssue>) {
    if !value_is_nonempty(&decision.disposition) {
        issues.push(issue(
            ISSUE_DECISION_INVALID,
            "decision.disposition",
            "comparison decision disposition must be stated",
        ));
    }
    if decision.runtime_adoption {
        issues.push(issue(
            ISSUE_DECISION_INVALID,
            "decision.runtime_adoption",
            "comparison decision must not assume permanent runtime adoption",
        ));
    }
    if !value_is_nonempty(&decision.reason) {
        issues.push(issue(
            ISSUE_DECISION_INVALID,
            "decision.reason",
            "comparison decision reason must be stated",
        ));
    }
}

fn validate_cases(manifest: &BuoyComparisonManifest, issues: &mut Vec<BuoyComparisonIssue>) {
    issues.extend(validate_cases_only(manifest));
}

fn validate_cases_only(manifest: &BuoyComparisonManifest) -> Vec<BuoyComparisonIssue> {
    let mut issues = Vec::new();
    if manifest.cases.is_empty() {
        issues.push(issue(
            ISSUE_CASE_INVALID,
            "cases",
            "comparison case inventory is empty",
        ));
        return issues;
    }
    if manifest.cases.len() > MAX_CASE_COUNT {
        issues.push(issue(
            ISSUE_CASE_INVALID,
            "cases",
            "comparison case inventory exceeds the bounded case count",
        ));
    }

    let mut seen = BTreeSet::new();
    for (index, case) in manifest.cases.iter().enumerate() {
        let path = format!("cases[{index}]");
        if !seen.insert(case.id.as_str()) {
            issues.push(issue(
                ISSUE_CASE_DUPLICATE,
                &format!("{path}.id"),
                "comparison case IDs must be unique",
            ));
            continue;
        }
        validate_case_shape(case, &path, &manifest.upstream, &mut issues);
    }

    for (index, case) in manifest.cases.iter().enumerate() {
        let path = format!("cases[{index}]");
        if let Some(mutation) = &case.mutation {
            validate_mutation(case, mutation, index, manifest, &path, &mut issues);
        }
        validate_case_comparison(case, &path, &mut issues);
        if case.execution.mode == BUOY_COMPARISON_MODE_NOT_EXECUTED
            && case.comparison != BUOY_COMPARISON_DISPOSITION_UNSUPPORTED
        {
            issues.push(issue(
                ISSUE_NOT_EXECUTED_PASS,
                &format!("{path}.comparison"),
                "a case without an executed native result must use an explicit unsupported comparison disposition",
            ));
        }
        if case.execution.mode != BUOY_COMPARISON_MODE_NOT_EXECUTED {
            if case.fixture.is_none() {
                issues.push(issue(
                    ISSUE_FIXTURE_UNSAFE,
                    &format!("{path}.fixture"),
                    "an executed comparison case must name a fixture",
                ));
            }
            if case.native_expectation.is_none() {
                issues.push(issue(
                    ISSUE_EXPECTATION_INVALID,
                    &format!("{path}.native_expectation"),
                    "an executed comparison case must declare a native expectation",
                ));
            }
        }
    }

    let executed = manifest
        .cases
        .iter()
        .filter(|case| case.execution.mode != BUOY_COMPARISON_MODE_NOT_EXECUTED)
        .count();
    if executed == 0 {
        issues.push(issue(
            ISSUE_EMPTY_EXECUTION,
            "cases",
            "comparison fixture set must execute at least one native case",
        ));
    }

    let kinds = manifest
        .cases
        .iter()
        .filter_map(|case| {
            case.mutation
                .as_ref()
                .map(|mutation| mutation.kind.as_str())
        })
        .collect::<BTreeSet<_>>();
    for required in REQUIRED_MUTATION_KINDS {
        if !kinds.contains(required) {
            issues.push(issue(
                ISSUE_COVERAGE_MISSING,
                "cases",
                &format!("comparison fixture set is missing required mutation kind {required}"),
            ));
        }
    }

    let has_current_base = manifest.cases.iter().any(|case| {
        case.mutation.is_none()
            && case.execution.mode == BUOY_COMPARISON_MODE_EXPORT_AND_VERIFY
            && case
                .native_expectation
                .as_ref()
                .is_some_and(|expectation| expectation.semantic == BUOY_COMPARISON_SEMANTIC_CURRENT)
    });
    if !has_current_base {
        issues.push(issue(
            ISSUE_COVERAGE_MISSING,
            "cases",
            "comparison fixture set must retain one clean export-and-verify base case",
        ));
    }

    issues
}

fn validate_case_shape(
    case: &BuoyComparisonCase,
    path: &str,
    upstream: &BuoyUpstreamPin,
    issues: &mut Vec<BuoyComparisonIssue>,
) {
    if !value_is_nonempty(&case.id) || !safe_case_id(&case.id) {
        issues.push(issue(
            ISSUE_CASE_INVALID,
            &format!("{path}.id"),
            "comparison case ID must be a safe nonempty path segment",
        ));
    }
    if !value_is_nonempty(&case.reason) {
        issues.push(issue(
            ISSUE_CASE_INVALID,
            &format!("{path}.reason"),
            "comparison case reason must be stated",
        ));
    }
    if let Some(fixture) = &case.fixture {
        if !safe_relative_path(fixture) {
            issues.push(issue(
                ISSUE_FIXTURE_UNSAFE,
                &format!("{path}.fixture"),
                "comparison fixture path must be a safe relative path",
            ));
        }
    }

    let mode = case.execution.mode.as_str();
    if !matches!(
        mode,
        BUOY_COMPARISON_MODE_EXPORT_AND_VERIFY
            | BUOY_COMPARISON_MODE_VERIFY_BASE_INDEX
            | BUOY_COMPARISON_MODE_REJECT_BASE_INDEX
            | BUOY_COMPARISON_MODE_NOT_EXECUTED
    ) {
        issues.push(issue(
            ISSUE_CASE_INVALID,
            &format!("{path}.execution.mode"),
            "comparison execution mode is unsupported",
        ));
        return;
    }
    match mode {
        BUOY_COMPARISON_MODE_EXPORT_AND_VERIFY => {
            if case.execution.base.is_some()
                || case.execution.registry_source.is_some()
                || case.execution.index_mutation.is_some()
            {
                issues.push(issue(
                    ISSUE_CASE_INVALID,
                    &format!("{path}.execution"),
                    "export-and-verify execution must not select a base, registry source, or index mutation",
                ));
            }
        }
        BUOY_COMPARISON_MODE_VERIFY_BASE_INDEX | BUOY_COMPARISON_MODE_REJECT_BASE_INDEX => {
            if case.execution.base.is_none() {
                issues.push(issue(
                    ISSUE_CASE_INVALID,
                    &format!("{path}.execution.base"),
                    "base-index execution must name a base case",
                ));
            }
            if !matches!(
                case.execution.registry_source.as_deref(),
                Some("base" | "case")
            ) {
                issues.push(issue(
                    ISSUE_CASE_INVALID,
                    &format!("{path}.execution.registry_source"),
                    "base-index execution must select the base or case registry",
                ));
            }
            let mutation = case.execution.index_mutation.as_deref();
            if mode == BUOY_COMPARISON_MODE_REJECT_BASE_INDEX {
                if mutation != Some(BUOY_COMPARISON_INDEX_MUTATION_SCHEMA_VERSION) {
                    issues.push(issue(
                        ISSUE_CASE_INVALID,
                        &format!("{path}.execution.index_mutation"),
                        "schema drift execution must name the schema_version index mutation",
                    ));
                }
            } else if mutation.is_some() {
                issues.push(issue(
                    ISSUE_CASE_INVALID,
                    &format!("{path}.execution.index_mutation"),
                    "verify-base execution must not mutate the saved index",
                ));
            }
        }
        BUOY_COMPARISON_MODE_NOT_EXECUTED => {
            if case.fixture.is_some() || case.native_expectation.is_some() {
                issues.push(issue(
                    ISSUE_CASE_INVALID,
                    path,
                    "a not-executed case must not present a fixture or native expectation",
                ));
            }
        }
        _ => {}
    }

    if case.buoy_observation.disposition == "executed" && !upstream.artifact.available {
        issues.push(issue(
            ISSUE_BUOY_EXECUTED_CLAIM,
            &format!("{path}.buoy_observation.disposition"),
            "comparison must not claim an upstream execution that is not available",
        ));
    }
    if !matches!(
        case.buoy_observation.disposition.as_str(),
        BUOY_OBSERVATION_DECLARED_NOT_EXECUTED | BUOY_OBSERVATION_DECLARED_UNSUPPORTED
    ) {
        issues.push(issue(
            ISSUE_DISPOSITION_INVALID,
            &format!("{path}.buoy_observation.disposition"),
            "comparison upstream observation must be declared, not executed",
        ));
    }
    if !semantic_is_supported(&case.buoy_observation.semantic) {
        issues.push(issue(
            ISSUE_DISPOSITION_INVALID,
            &format!("{path}.buoy_observation.semantic"),
            "comparison upstream semantic class is unsupported",
        ));
    }
    if !value_is_nonempty(&case.buoy_observation.report_schema)
        || !upstream
            .report_schemas
            .iter()
            .any(|schema| schema == &case.buoy_observation.report_schema)
    {
        issues.push(issue(
            ISSUE_DISPOSITION_INVALID,
            &format!("{path}.buoy_observation.report_schema"),
            "comparison upstream report schema must be pinned by the manifest",
        ));
    }
    if !value_is_nonempty(&case.buoy_observation.reason) {
        issues.push(issue(
            ISSUE_DISPOSITION_INVALID,
            &format!("{path}.buoy_observation.reason"),
            "comparison upstream observation reason must be stated",
        ));
    }

    if let Some(expectation) = &case.native_expectation {
        validate_native_expectation(expectation, &format!("{path}.native_expectation"), issues);
    }
}

fn validate_native_expectation(
    expectation: &BuoyNativeExpectation,
    path: &str,
    issues: &mut Vec<BuoyComparisonIssue>,
) {
    if expectation.exit > 2 {
        issues.push(issue(
            ISSUE_EXPECTATION_INVALID,
            &format!("{path}.exit"),
            "native expectation exit disposition is unsupported",
        ));
    }
    if expectation.valid && expectation.exit != 0 {
        issues.push(issue(
            ISSUE_EXPECTATION_INVALID,
            &format!("{path}.valid"),
            "a valid native expectation must use exit 0",
        ));
    }
    if expectation.exit == 0 && !expectation.valid {
        issues.push(issue(
            ISSUE_EXPECTATION_INVALID,
            &format!("{path}.valid"),
            "an exit 0 native expectation must be valid",
        ));
    }
    if expectation
        .issue_codes
        .iter()
        .any(|code| !value_is_nonempty(code))
    {
        issues.push(issue(
            ISSUE_EXPECTATION_INVALID,
            &format!("{path}.issue_codes"),
            "native expectation issue codes must be nonempty",
        ));
    }
    if expectation
        .stderr_contains
        .as_ref()
        .is_some_and(|value| value.is_empty())
    {
        issues.push(issue(
            ISSUE_EXPECTATION_INVALID,
            &format!("{path}.stderr_contains"),
            "native expectation stderr matcher must be nonempty",
        ));
    }

    let derived = derive_expectation_semantic(expectation);
    match derived {
        Some(semantic) if semantic == expectation.semantic.as_str() => {}
        Some(semantic) => issues.push(issue(
            ISSUE_EXPECTATION_INVALID,
            &format!("{path}.semantic"),
            &format!(
                "native expectation semantic must be {semantic} for the declared exit and issue codes"
            ),
        )),
        None => issues.push(issue(
            ISSUE_EXPECTATION_INVALID,
            &format!("{path}.semantic"),
            "native expectation semantic cannot be derived from its exit, validity, issue codes, and stderr matcher",
        )),
    }

    if expectation.semantic == BUOY_COMPARISON_SEMANTIC_INVALID_OUTPUT
        && expectation.stderr_contains.is_none()
    {
        issues.push(issue(
            ISSUE_EXPECTATION_INVALID,
            &format!("{path}.stderr_contains"),
            "invalid output expectations must name the retained stderr discriminator",
        ));
    }
    if expectation.semantic != BUOY_COMPARISON_SEMANTIC_INVALID_OUTPUT
        && expectation.stderr_contains.is_some()
    {
        issues.push(issue(
            ISSUE_EXPECTATION_INVALID,
            &format!("{path}.stderr_contains"),
            "stderr matchers belong only to invalid output expectations",
        ));
    }
}

fn validate_mutation(
    case: &BuoyComparisonCase,
    mutation: &BuoyComparisonMutation,
    index: usize,
    manifest: &BuoyComparisonManifest,
    path: &str,
    issues: &mut Vec<BuoyComparisonIssue>,
) {
    if !REQUIRED_MUTATION_KINDS.contains(&mutation.kind.as_str()) {
        issues.push(issue(
            ISSUE_MUTATION_INVALID,
            &format!("{path}.mutation.kind"),
            "comparison mutation kind is unsupported",
        ));
        return;
    }
    let Some(base_index) = manifest
        .cases
        .iter()
        .position(|candidate| candidate.id == mutation.of)
    else {
        issues.push(issue(
            ISSUE_MUTATION_INVALID,
            &format!("{path}.mutation.of"),
            "comparison mutation must name an existing base case",
        ));
        return;
    };
    if base_index >= index {
        issues.push(issue(
            ISSUE_MUTATION_INVALID,
            &format!("{path}.mutation.of"),
            "comparison mutation base case must appear before the mutated case",
        ));
        return;
    }
    let base = &manifest.cases[base_index];
    if base.mutation.is_some() {
        issues.push(issue(
            ISSUE_MUTATION_INVALID,
            &format!("{path}.mutation.of"),
            "comparison mutation base case must not itself be a mutation",
        ));
    }
    if base.execution.mode != BUOY_COMPARISON_MODE_EXPORT_AND_VERIFY {
        issues.push(issue(
            ISSUE_MUTATION_INVALID,
            &format!("{path}.mutation.of"),
            "comparison mutation base case must export its own base artifacts",
        ));
    }
    if case.execution.base.as_deref() != Some(mutation.of.as_str()) {
        issues.push(issue(
            ISSUE_MUTATION_INVALID,
            &format!("{path}.execution.base"),
            "comparison mutation execution must use the declared base case",
        ));
    }
    let expected_mode = if mutation.kind == "schema_version_drift" {
        BUOY_COMPARISON_MODE_REJECT_BASE_INDEX
    } else {
        BUOY_COMPARISON_MODE_VERIFY_BASE_INDEX
    };
    if case.execution.mode != expected_mode {
        issues.push(issue(
            ISSUE_MUTATION_INVALID,
            &format!("{path}.execution.mode"),
            "comparison mutation execution mode does not match its planted drift kind",
        ));
    }

    let Some(expectation) = &case.native_expectation else {
        return;
    };
    let allowed = MUTATION_ALLOWED_SEMANTICS
        .iter()
        .find(|(kind, _)| *kind == mutation.kind.as_str())
        .map(|(_, semantics)| *semantics)
        .unwrap_or(&[]);
    if !allowed.contains(&expectation.semantic.as_str()) {
        issues.push(issue(
            ISSUE_MUTATION_INVALID,
            &format!("{path}.native_expectation.semantic"),
            "comparison mutation native semantic does not match its planted drift kind",
        ));
    }
    if let Some(base_expectation) = &base.native_expectation {
        if base_expectation.semantic == expectation.semantic
            && base_expectation.issue_codes == expectation.issue_codes
            && base_expectation.valid == expectation.valid
        {
            issues.push(issue(
                ISSUE_MUTATION_RESTATED,
                &format!("{path}.native_expectation"),
                "comparison mutation must change the native expectation from its base case",
            ));
        }
    }
    if base.buoy_observation.semantic == case.buoy_observation.semantic {
        issues.push(issue(
            ISSUE_MUTATION_RESTATED,
            &format!("{path}.buoy_observation.semantic"),
            "comparison mutation must change the declared upstream observation from its base case",
        ));
    }
}

fn validate_case_comparison(
    case: &BuoyComparisonCase,
    path: &str,
    issues: &mut Vec<BuoyComparisonIssue>,
) {
    match case.comparison.as_str() {
        BUOY_COMPARISON_DISPOSITION_AGREEMENT => {
            if case.divergence.is_some() {
                issues.push(issue(
                    ISSUE_DIVERGENCE_INVALID,
                    &format!("{path}.divergence"),
                    "an agreement case must not declare a divergence",
                ));
            }
            if case.buoy_observation.disposition == BUOY_OBSERVATION_DECLARED_UNSUPPORTED {
                issues.push(issue(
                    ISSUE_DISPOSITION_INVALID,
                    &format!("{path}.buoy_observation.disposition"),
                    "an unsupported upstream observation cannot be declared as agreement",
                ));
            }
            if let Some(expectation) = &case.native_expectation {
                if expectation.semantic != case.buoy_observation.semantic {
                    issues.push(issue(
                        ISSUE_DISPOSITION_INVALID,
                        &format!("{path}.buoy_observation.semantic"),
                        "an agreement case must declare the executed native semantic class",
                    ));
                }
            }
        }
        BUOY_COMPARISON_DISPOSITION_EXPECTED_DIVERGENCE => {
            if case.divergence.as_deref().is_none_or(str::is_empty) {
                issues.push(issue(
                    ISSUE_DIVERGENCE_INVALID,
                    &format!("{path}.divergence"),
                    "an expected divergence must name the divergence dimension",
                ));
            }
            if let Some(expectation) = &case.native_expectation {
                if expectation.semantic == case.buoy_observation.semantic {
                    issues.push(issue(
                        ISSUE_DIVERGENCE_INVALID,
                        &format!("{path}.buoy_observation.semantic"),
                        "an expected divergence must not declare equal semantic classes",
                    ));
                }
            }
        }
        BUOY_COMPARISON_DISPOSITION_UNSUPPORTED => {
            if case.buoy_observation.disposition != BUOY_OBSERVATION_DECLARED_UNSUPPORTED {
                issues.push(issue(
                    ISSUE_DISPOSITION_INVALID,
                    &format!("{path}.buoy_observation.disposition"),
                    "a comparison-unsupported case must declare an unsupported upstream disposition",
                ));
            }
            if case.divergence.is_some() {
                issues.push(issue(
                    ISSUE_DIVERGENCE_INVALID,
                    &format!("{path}.divergence"),
                    "a comparison-unsupported case must not declare a divergence result",
                ));
            }
        }
        _ => issues.push(issue(
            ISSUE_DISPOSITION_INVALID,
            &format!("{path}.comparison"),
            "comparison disposition is unsupported",
        )),
    }
}

fn derive_expectation_semantic(expectation: &BuoyNativeExpectation) -> Option<&'static str> {
    if expectation.exit == 0 {
        if expectation.valid && expectation.issue_codes.is_empty() {
            return Some(BUOY_COMPARISON_SEMANTIC_CURRENT);
        }
        return None;
    }
    if expectation.valid {
        return None;
    }
    if expectation
        .stderr_contains
        .as_ref()
        .is_some_and(|value| !value.is_empty())
        && expectation.issue_codes.is_empty()
    {
        return Some(BUOY_COMPARISON_SEMANTIC_INVALID_OUTPUT);
    }
    let has = |code: &str| expectation.issue_codes.iter().any(|value| value == code);
    if has(NATIVE_ISSUE_REGISTRY_STALE) || has(NATIVE_ISSUE_POLICY_STALE) {
        return Some(BUOY_COMPARISON_SEMANTIC_STALE_REQUIREMENT);
    }
    if has(NATIVE_ISSUE_SOURCE_STALE)
        || has(NATIVE_ISSUE_SOURCE_UNEXPECTED)
        || has(NATIVE_ISSUE_LINK_STALE)
    {
        return Some(BUOY_COMPARISON_SEMANTIC_STALE_ITEM);
    }
    if has(NATIVE_ISSUE_SOURCE_MISSING) || has(NATIVE_ISSUE_MARKER_DANGLING) {
        return Some(BUOY_COMPARISON_SEMANTIC_MISSING_OBJECT);
    }
    if has(NATIVE_ISSUE_LINK_DUPLICATE) || has(NATIVE_ISSUE_SOURCE_DUPLICATE) {
        return Some(BUOY_COMPARISON_SEMANTIC_AMBIGUOUS);
    }
    if has(NATIVE_ISSUE_SCHEMA_UNSUPPORTED)
        || has(NATIVE_ISSUE_PROFILE_UNSUPPORTED)
        || has(NATIVE_ISSUE_PROFILE_STALE)
        || has(NATIVE_ISSUE_ORDER_INVALID)
    {
        return Some(BUOY_COMPARISON_SEMANTIC_UNSUPPORTED);
    }
    None
}

fn semantic_is_supported(semantic: &str) -> bool {
    matches!(
        semantic,
        BUOY_COMPARISON_SEMANTIC_CURRENT
            | BUOY_COMPARISON_SEMANTIC_STALE_REQUIREMENT
            | BUOY_COMPARISON_SEMANTIC_STALE_ITEM
            | BUOY_COMPARISON_SEMANTIC_MISSING_OBJECT
            | BUOY_COMPARISON_SEMANTIC_AMBIGUOUS
            | BUOY_COMPARISON_SEMANTIC_UNSUPPORTED
            | BUOY_COMPARISON_SEMANTIC_INVALID_OUTPUT
    )
}

pub fn evaluate_buoy_comparison(
    manifest: &BuoyComparisonManifest,
    inputs: &BuoyComparisonInputs,
) -> BuoyComparisonEvaluation {
    let mut issues = validate_buoy_comparison_manifest(manifest);
    if !is_hash(&inputs.manifest_blake3) {
        issues.push(issue(
            ISSUE_SCHEMA_UNSUPPORTED,
            "manifest_blake3",
            "comparison manifest identity must be one BLAKE3 value",
        ));
    }
    if !is_hash(&inputs.policy_blake3) {
        issues.push(issue(
            ISSUE_SCHEMA_UNSUPPORTED,
            "policy_blake3",
            "comparison policy identity must be one BLAKE3 value",
        ));
    }

    let mut observations = BTreeMap::new();
    for observation in &inputs.observations {
        if observations
            .insert(observation.case_id.as_str(), observation)
            .is_some()
        {
            issues.push(issue(
                ISSUE_OBSERVATION_UNEXPECTED,
                "observations",
                "comparison native observations must be unique per case",
            ));
        }
    }
    for case in &manifest.cases {
        match case.execution.mode.as_str() {
            BUOY_COMPARISON_MODE_NOT_EXECUTED => {
                if observations.contains_key(case.id.as_str()) {
                    issues.push(issue(
                        ISSUE_OBSERVATION_UNEXPECTED,
                        &format!("cases.{}.observations", case.id),
                        "a not-executed comparison case must not supply a native observation",
                    ));
                }
            }
            _ => {
                if !observations.contains_key(case.id.as_str()) {
                    issues.push(issue(
                        ISSUE_OBSERVATION_MISSING,
                        &format!("cases.{}.observations", case.id),
                        "an executed comparison case must supply one native observation",
                    ));
                }
                if !inputs.fixture_blake3.contains_key(case.id.as_str()) {
                    issues.push(issue(
                        ISSUE_FIXTURE_MISSING,
                        &format!("cases.{}.fixture", case.id),
                        "an executed comparison case must supply one fixture identity",
                    ));
                }
            }
        }
    }
    for case_id in inputs.fixture_blake3.keys() {
        if !manifest.cases.iter().any(|case| &case.id == case_id) {
            issues.push(issue(
                ISSUE_OBSERVATION_UNEXPECTED,
                &format!("fixtures.{case_id}"),
                "fixture identity has no matching comparison case",
            ));
        }
    }

    let mut case_receipts = Vec::with_capacity(manifest.cases.len());
    for case in &manifest.cases {
        case_receipts.push(evaluate_case(case, inputs, &observations, &mut issues));
    }

    let executed = manifest
        .cases
        .iter()
        .filter(|case| case.execution.mode != BUOY_COMPARISON_MODE_NOT_EXECUTED)
        .count();
    let declared_only = manifest.cases.len().saturating_sub(executed);
    let observed_kinds = manifest
        .cases
        .iter()
        .filter_map(|case| case.mutation.as_ref().map(|mutation| mutation.kind.clone()))
        .collect::<BTreeSet<_>>();
    let required_kinds = REQUIRED_MUTATION_KINDS
        .iter()
        .map(|kind| (*kind).to_string())
        .collect::<Vec<_>>();
    let mutation_detected = issues.is_empty()
        && REQUIRED_MUTATION_KINDS
            .iter()
            .all(|kind| observed_kinds.contains(*kind));

    let known_disagreements = case_receipts
        .iter()
        .filter(|case| case["comparison"] == BUOY_COMPARISON_DISPOSITION_EXPECTED_DIVERGENCE)
        .map(|case| case["id"].clone())
        .collect::<Vec<_>>();
    let unsupported_cases = case_receipts
        .iter()
        .filter(|case| case["comparison"] == BUOY_COMPARISON_DISPOSITION_UNSUPPORTED)
        .map(|case| case["id"].clone())
        .collect::<Vec<_>>();

    let producer_identities = producer_identities(manifest, inputs, &case_receipts);
    let comparison_identity = comparison_identity(manifest, inputs);
    let artifact_available = manifest.upstream.artifact.available;
    let valid = issues.is_empty() && executed > 0;
    let complete = unsupported_cases.is_empty() && declared_only == 0;
    // These report whether the comparison *rejected* a restated input. The paired
    // codes record the opposite (an accepted restatement), so their absence is the
    // positive result.
    let restated_observation_rejected = !issues
        .iter()
        .any(|issue| issue.code == ISSUE_RESTATED_OBSERVATION);
    let restated_native_rejected = !issues
        .iter()
        .any(|issue| issue.code == ISSUE_RESTATED_NATIVE);

    let receipt = json!({
        "schema": BUOY_COMPARISON_RECEIPT_SCHEMA_V1,
        "command": "traceability compare",
        "upstream": {
            "producer": manifest.upstream.producer,
            "repository": manifest.upstream.repository,
            "revision": manifest.upstream.revision,
            "source_content_hash": manifest.upstream.source_content_hash,
            "source_content_hash_algorithm": manifest.upstream.source_content_hash_algorithm,
            "source_content_hash_mode": manifest.upstream.source_content_hash_mode,
            "consumer_package": manifest.upstream.consumer_package,
            "rust_toolchain": manifest.upstream.rust_toolchain,
            "digest_feature": manifest.upstream.digest_feature,
            "commands": manifest.upstream.commands,
            "command_behavior": manifest.upstream.command_behavior,
            "report_schemas": manifest.upstream.report_schemas,
            "evidence": manifest.upstream.evidence,
            "executed": false,
            "artifact_available": artifact_available,
            "disposition": if artifact_available { "artifact_available_not_executed" } else { "blocked" },
            "reason": manifest.upstream.artifact.reason,
        },
        "mapping": manifest.mapping,
        "decision": manifest.decision,
        "manifest_blake3": inputs.manifest_blake3,
        "policy_blake3": inputs.policy_blake3,
        "comparison_identity": comparison_identity,
        "execution": {
            "native_executed": true,
            "upstream_executed": false,
            "case_count": manifest.cases.len(),
            "executed_case_count": executed,
            "declared_only_case_count": declared_only,
        },
        "cases": case_receipts,
        "mutation_qualification": {
            "required_kinds": required_kinds,
            "observed_kinds": observed_kinds,
            "planted_drift_detected": mutation_detected,
            "restated_observation_rejected": restated_observation_rejected,
            "restated_native_expectation_rejected": restated_native_rejected,
        },
        "known_disagreements": known_disagreements,
        "unsupported_cases": unsupported_cases,
        "producer_identities": producer_identities,
        "valid": valid,
        "complete": complete,
        "verdict": if valid { "pass" } else { "fail" },
        "boundary": BUOY_COMPARISON_BOUNDARY,
        "non_claims": BUOY_COMPARISON_NON_CLAIMS,
        "issues": issues,
    });

    BuoyComparisonEvaluation {
        valid,
        receipt,
        issues,
    }
}

fn evaluate_case(
    case: &BuoyComparisonCase,
    inputs: &BuoyComparisonInputs,
    observations: &BTreeMap<&str, &BuoyComparisonObservation>,
    issues: &mut Vec<BuoyComparisonIssue>,
) -> Value {
    let fixture_blake3 = inputs.fixture_blake3.get(case.id.as_str()).cloned();
    let mut native = json!({
        "executed": false,
        "exit": Value::Null,
        "valid": Value::Null,
        "issue_codes": [],
        "stdout_blake3": Value::Null,
        "stderr_blake3": Value::Null,
    });
    let mut actual_semantic: Option<&'static str> = None;
    if case.execution.mode != BUOY_COMPARISON_MODE_NOT_EXECUTED {
        if let Some(observation) = observations.get(case.id.as_str()) {
            let (semantic, codes, valid) = classify_observation(observation);
            actual_semantic = semantic;
            native = json!({
                "executed": true,
                "exit": observation.exit_code,
                "valid": valid,
                "issue_codes": codes.clone(),
                "stdout_blake3": domain_hash(NATIVE_REPORT_DOMAIN, &observation.stdout),
                "stderr_blake3": domain_hash(NATIVE_REPORT_DOMAIN, &observation.stderr),
            });

            if let Some(expectation) = &case.native_expectation {
                if i32::from(expectation.exit) != observation.exit_code {
                    issues.push(issue(
                        ISSUE_EXECUTION_MISMATCH,
                        &format!("cases.{}.exit", case.id),
                        &format!(
                            "native exit disposition {} does not match declared exit {}",
                            observation.exit_code, expectation.exit
                        ),
                    ));
                }
                if expectation.valid != valid.unwrap_or(false) {
                    issues.push(issue(
                        ISSUE_EXECUTION_MISMATCH,
                        &format!("cases.{}.valid", case.id),
                        "native validity does not match the declared expectation",
                    ));
                }
                for expected_code in &expectation.issue_codes {
                    if !codes.iter().any(|code| code == expected_code) {
                        issues.push(issue(
                            ISSUE_EXECUTION_MISMATCH,
                            &format!("cases.{}.issue_codes", case.id),
                            &format!("native observation does not report {expected_code}"),
                        ));
                    }
                }
                if let Some(matcher) = &expectation.stderr_contains {
                    let stderr = String::from_utf8_lossy(&observation.stderr);
                    if !stderr.contains(matcher) {
                        issues.push(issue(
                            ISSUE_EXECUTION_MISMATCH,
                            &format!("cases.{}.stderr", case.id),
                            "native stderr does not contain the declared discriminator",
                        ));
                    }
                }
                let Some(semantic) = semantic else {
                    issues.push(issue(
                        ISSUE_EXECUTION_MISMATCH,
                        &format!("cases.{}.semantic", case.id),
                        "native observation cannot be classified into the declared semantic subset",
                    ));
                    return case_receipt(case, fixture_blake3, native, None, "fail");
                };
                if semantic != expectation.semantic.as_str() {
                    issues.push(issue(
                        ISSUE_EXECUTION_MISMATCH,
                        &format!("cases.{}.semantic", case.id),
                        &format!(
                            "native semantic {semantic} does not match declared {}",
                            expectation.semantic
                        ),
                    ));
                }
            }
        }
    }

    let outcome = match case.comparison.as_str() {
        BUOY_COMPARISON_DISPOSITION_AGREEMENT => {
            if actual_semantic != Some(case.buoy_observation.semantic.as_str()) {
                issues.push(issue(
                    ISSUE_DISPOSITION_INVALID,
                    &format!("cases.{}.comparison", case.id),
                    "native and declared upstream semantic classes disagree",
                ));
                "fail"
            } else {
                "agreement"
            }
        }
        BUOY_COMPARISON_DISPOSITION_EXPECTED_DIVERGENCE => {
            if actual_semantic == Some(case.buoy_observation.semantic.as_str()) {
                issues.push(issue(
                    ISSUE_DIVERGENCE_INVALID,
                    &format!("cases.{}.comparison", case.id),
                    "an expected divergence case now declares equal semantic classes",
                ));
                "fail"
            } else {
                "expected_divergence"
            }
        }
        BUOY_COMPARISON_DISPOSITION_UNSUPPORTED => "unsupported",
        _ => "fail",
    };

    case_receipt(case, fixture_blake3, native, actual_semantic, outcome)
}

fn case_receipt(
    case: &BuoyComparisonCase,
    fixture_blake3: Option<String>,
    native: Value,
    native_semantic: Option<&str>,
    outcome: &str,
) -> Value {
    json!({
        "id": case.id,
        "fixture_blake3": fixture_blake3,
        "execution_mode": case.execution.mode,
        "base": case.execution.base,
        "registry_source": case.execution.registry_source,
        "index_mutation": case.execution.index_mutation,
        "mutation": case.mutation,
        "native": native,
        "native_semantic": native_semantic,
        "declared_upstream": {
            "disposition": case.buoy_observation.disposition,
            "semantic": case.buoy_observation.semantic,
            "report_schema": case.buoy_observation.report_schema,
            "report_blake3": domain_hash_json(BUOY_REPORT_DOMAIN, &json!({
                "schema": case.buoy_observation.report_schema,
                "semantic": case.buoy_observation.semantic,
                "disposition": case.buoy_observation.disposition,
            })),
            "reason": case.buoy_observation.reason,
        },
        "comparison": case.comparison,
        "divergence": case.divergence,
        "outcome": outcome,
        "reason": case.reason,
    })
}

fn classify_observation(
    observation: &BuoyComparisonObservation,
) -> (Option<&'static str>, Vec<String>, Option<bool>) {
    if observation.exit_code != 0 && observation.stdout.is_empty() {
        return (
            Some(BUOY_COMPARISON_SEMANTIC_INVALID_OUTPUT),
            Vec::new(),
            Some(false),
        );
    }
    let Ok(value) = serde_json::from_slice::<Value>(&observation.stdout) else {
        return (
            Some(BUOY_COMPARISON_SEMANTIC_INVALID_OUTPUT),
            Vec::new(),
            Some(false),
        );
    };
    let valid = value.get("valid").and_then(Value::as_bool);
    let mut codes = Vec::new();
    for field in ["issues", "registry_issues"] {
        if let Some(items) = value.get(field).and_then(Value::as_array) {
            for item in items {
                if let Some(code) = item.get("code").and_then(Value::as_str) {
                    codes.push(code.to_string());
                }
            }
        }
    }
    codes.sort();
    codes.dedup();
    let semantic = match valid {
        Some(true) => Some(BUOY_COMPARISON_SEMANTIC_CURRENT),
        Some(false) => Some(classify_issue_codes(&codes)),
        None => Some(BUOY_COMPARISON_SEMANTIC_INVALID_OUTPUT),
    };
    (semantic, codes, valid)
}

fn classify_issue_codes(codes: &[String]) -> &'static str {
    let has = |code: &str| codes.iter().any(|value| value == code);
    if has(NATIVE_ISSUE_REGISTRY_STALE) || has(NATIVE_ISSUE_POLICY_STALE) {
        BUOY_COMPARISON_SEMANTIC_STALE_REQUIREMENT
    } else if has(NATIVE_ISSUE_SOURCE_STALE)
        || has(NATIVE_ISSUE_SOURCE_UNEXPECTED)
        || has(NATIVE_ISSUE_LINK_STALE)
    {
        BUOY_COMPARISON_SEMANTIC_STALE_ITEM
    } else if has(NATIVE_ISSUE_SOURCE_MISSING) || has(NATIVE_ISSUE_MARKER_DANGLING) {
        BUOY_COMPARISON_SEMANTIC_MISSING_OBJECT
    } else if has(NATIVE_ISSUE_LINK_DUPLICATE) || has(NATIVE_ISSUE_SOURCE_DUPLICATE) {
        BUOY_COMPARISON_SEMANTIC_AMBIGUOUS
    } else {
        BUOY_COMPARISON_SEMANTIC_UNSUPPORTED
    }
}

fn producer_identities(
    manifest: &BuoyComparisonManifest,
    inputs: &BuoyComparisonInputs,
    case_receipts: &[Value],
) -> Value {
    let native_material = case_receipts
        .iter()
        .map(|case| {
            json!({
                "id": case["id"],
                "schema": NATIVE_RECEIPT_SCHEMA,
                "stdout_blake3": case["native"]["stdout_blake3"],
                "stderr_blake3": case["native"]["stderr_blake3"],
                "exit": case["native"]["exit"],
            })
        })
        .collect::<Vec<_>>();
    let buoy_material = manifest
        .cases
        .iter()
        .map(|case| {
            json!({
                "id": case.id,
                "schema": case.buoy_observation.report_schema,
                "semantic": case.buoy_observation.semantic,
                "disposition": case.buoy_observation.disposition,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "native": {
            "producer": "cairn",
            "schema": NATIVE_RECEIPT_SCHEMA,
            "report_blake3": domain_hash_json(NATIVE_REPORT_DOMAIN, &json!(native_material)),
        },
        "declared_upstream": {
            "producer": manifest.upstream.producer,
            "schemas": manifest.upstream.report_schemas,
            "report_blake3": domain_hash_json(BUOY_REPORT_DOMAIN, &json!(buoy_material)),
        },
        "fixture_set_blake3": domain_hash_json(IDENTITY_DOMAIN, &json!({
            "manifest_blake3": inputs.manifest_blake3,
            "policy_blake3": inputs.policy_blake3,
        })),
    })
}

fn comparison_identity(manifest: &BuoyComparisonManifest, inputs: &BuoyComparisonInputs) -> String {
    let fixtures = inputs
        .fixture_blake3
        .iter()
        .map(|(case_id, blake3)| json!({"case": case_id, "fixture_blake3": blake3}))
        .collect::<Vec<_>>();
    domain_hash_json(
        IDENTITY_DOMAIN,
        &json!({
            "schema": BUOY_COMPARISON_RECEIPT_SCHEMA_V1,
            "manifest_blake3": inputs.manifest_blake3,
            "policy_blake3": inputs.policy_blake3,
            "upstream": manifest.upstream,
            "mapping": manifest.mapping,
            "fixtures": fixtures,
        }),
    )
}

fn safe_case_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_STRING_BYTES
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\\')
}

fn safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_STRING_BYTES
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.split(['/', '\\']).any(|part| part == "..")
}

fn value_is_nonempty(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_STRING_BYTES
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn domain_hash(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain.as_bytes());
    hasher.update(&[0]);
    hasher.update(bytes);
    hasher.finalize().to_hex().to_string()
}

fn domain_hash_json(domain: &str, value: &Value) -> String {
    domain_hash(
        domain,
        crate::verified::evidence::canonical_json(value).as_bytes(),
    )
}

fn update_length_prefixed(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> BuoyComparisonManifest {
        serde_json::from_value(json!({
            "schema": BUOY_COMPARISON_MANIFEST_SCHEMA_V1,
            "upstream": {
                "producer": "buoy",
                "repository": "https://github.com/Chelis-Lang/buoy",
                "revision": "01096fbdd0b9bf39b2fec468f90e7b1db958f615",
                "source_content_hash": "sha256-1+/x0hY3+YMYgLVXs0uoUmA1VPPlg5i/7qt19NrBruc=",
                "source_content_hash_algorithm": "sha256",
                "source_content_hash_mode": "recursive-nar",
                "consumer_package": "nix/consumer-package-v1.nix",
                "rust_toolchain": "nightly-2026-04-16",
                "digest_feature": "blake3-256",
                "commands": ["static", "trace", "package-identity"],
                "report_schemas": ["buoy/trace-report/v1"],
                "artifact": {"available": false, "reason": "not realized"}
            },
            "mapping": {
                "id": "cairn.buoy-comparison.mapping.v1",
                "semantic_subset": "binding-semantics",
                "projection_version": "projection.v1",
                "original_authority": "cairn-content-bound-trace-index",
                "preserves_original_identities": true,
                "external_projection_is_authority": false
            },
            "decision": {
                "disposition": "retain_until_pinned_artifact_available",
                "runtime_adoption": false,
                "reason": "bounded substitute"
            },
            "cases": [
                {
                    "id": "clean",
                    "fixture": "cases/clean",
                    "execution": {"mode": "export_and_verify"},
                    "native_expectation": {"exit": 0, "valid": true, "issue_codes": [], "semantic": "current"},
                    "buoy_observation": {"disposition": "declared_not_executed", "semantic": "current", "report_schema": "buoy/trace-report/v1", "reason": "declared"},
                    "comparison": "agreement",
                    "reason": "clean"
                },
                {
                    "id": "missing",
                    "fixture": "cases/missing",
                    "execution": {"mode": "verify_base_index", "base": "clean", "registry_source": "base"},
                    "mutation": {"of": "clean", "kind": "missing_marker"},
                    "native_expectation": {"exit": 1, "valid": false, "issue_codes": ["trace_index.source_stale"], "semantic": "stale_item"},
                    "buoy_observation": {"disposition": "declared_not_executed", "semantic": "stale_item", "report_schema": "buoy/trace-report/v1", "reason": "declared"},
                    "comparison": "agreement",
                    "reason": "missing"
                }
            ]
        }))
        .expect("manifest")
    }

    #[test]
    fn manifest_requires_the_complete_mutation_inventory() {
        let issues = validate_buoy_comparison_manifest(&manifest());
        assert!(issues
            .iter()
            .any(|entry| entry.code == ISSUE_COVERAGE_MISSING));
    }

    #[test]
    fn producer_identities_separate_schema_namespaces() {
        let left = domain_hash_json(
            NATIVE_REPORT_DOMAIN,
            &json!({"schema": NATIVE_RECEIPT_SCHEMA, "semantic": "current"}),
        );
        let right = domain_hash_json(
            BUOY_REPORT_DOMAIN,
            &json!({"schema": "buoy/trace-report/v1", "semantic": "current"}),
        );
        assert_ne!(left, right);
        assert_eq!(left.len(), 64);
    }

    #[test]
    fn fixture_tree_identity_is_order_independent() {
        let left = buoy_comparison_fixture_tree_blake3(&[
            ("b.rs".to_string(), b"b".to_vec()),
            ("a.rs".to_string(), b"a".to_vec()),
        ]);
        let right = buoy_comparison_fixture_tree_blake3(&[
            ("a.rs".to_string(), b"a".to_vec()),
            ("b.rs".to_string(), b"b".to_vec()),
        ]);
        assert_eq!(left, right);
        let changed = buoy_comparison_fixture_tree_blake3(&[
            ("a.rs".to_string(), b"a".to_vec()),
            ("b.rs".to_string(), b"c".to_vec()),
        ]);
        assert_ne!(left, changed);
    }
}
