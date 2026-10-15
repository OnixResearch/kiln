//! Deterministic testing-harness support for Evidence IR.
//!
//! This module is the functional core for Valence's transcript fixture runner,
//! malformed-input corpus, golden report normalization, requirement coverage
//! rows, capability-enforcement planning, and suite lane summaries. It only
//! consumes in-memory values or caller-provided strings. Filesystem discovery,
//! fixture refresh writes, process execution, and CI orchestration remain in
//! tests, `valence-cli`, or downstream shells.
//!
//! # Boundary
//!
//! Harness reports prove deterministic fixture evaluation, traceability marker
//! coverage, and suite scheduling only. They do not prove behavioral
//! correctness, semantic equivalence, verifier soundness, requirement
//! satisfaction, release eligibility, sandbox effectiveness, or whole-program
//! safety.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use crate::evidence_model::CapabilityPolicy;
use crate::evidence_model::DurableEvidenceEnvelope;
use crate::evidence_model::DurableStorageCodec;
use crate::evidence_model::EvidenceDefinition;
use crate::evidence_model::EvidenceGraph;
use crate::evidence_model::NameBindingHistory;
use crate::evidence_model::ReceiptTranscript;
use crate::evidence_model::TranscriptExpectedReport;
use crate::evidence_model::VerifierCapabilityKind;
use crate::evidence_model::VerifierCapabilityLabel;
use crate::evidence_model::VerifierCapabilitySet;
use crate::evidence_model::decode_durable_value;
use crate::evidence_model::evaluate_capability_policy;
use crate::evidence_model::evaluate_receipt_transcript;
use crate::evidence_model::validate_evidence_definition;
use crate::evidence_model::validate_evidence_graph;
use crate::evidence_model::validate_name_bindings;

/// Non-claim emitted by transcript fixture reports.
pub const TRANSCRIPT_FIXTURE_RUNNER_NON_CLAIM: &str = "transcript fixture reports prove checked fixture behavior only, not general correctness, verifier soundness, semantic equivalence, or requirement satisfaction";
/// Non-claim emitted by malformed corpus reports.
pub const MALFORMED_CORPUS_NON_CLAIM: &str = "malformed corpus reports prove fail-closed behavior for checked fixtures only, not parser completeness or general correctness";
/// Non-claim emitted by golden normalization reports.
pub const GOLDEN_NORMALIZATION_NON_CLAIM: &str =
    "normalized golden reports prove stable comparison identity only, not semantic correctness or verifier soundness";
/// Non-claim emitted by requirement coverage reports.
pub const REQUIREMENT_COVERAGE_NON_CLAIM: &str = "requirement marker coverage proves traceability only, not correctness, verifier soundness, semantic equivalence, or requirement satisfaction";
/// Non-claim emitted by capability enforcement reports.
pub const CAPABILITY_ENFORCEMENT_NON_CLAIM: &str = "capability labels are metadata unless an enforcement backend actually blocks effects; labels do not prove verifier soundness or requirement satisfaction";
/// Non-claim emitted by suite partition reports.
pub const TEST_SUITE_PARTITION_NON_CLAIM: &str = "test suite lanes schedule evidence gates only and do not prove correctness, verifier soundness, semantic equivalence, or requirement satisfaction";

/// Stable command for the hermetic fast lane.
pub const FAST_LANE_COMMAND: &str = "nix develop --option builders '' -c cargo test -p valence-core evidence_harness";
/// Stable command for the opt-in deep lane.
pub const DEEP_LANE_COMMAND: &str =
    "nix develop --option builders '' -c cargo test --workspace stack_provenance -- --nocapture";
/// Stable command for the expensive nightly lane.
pub const NIGHTLY_LANE_COMMAND: &str = "nix develop --option builders '' -c cargo clippy --workspace --all-targets -- -D warnings && nix flake check --option builders '' -L";

const BLAKE3_IDENTITY_PREFIX: &str = "b3:";
const FIELD_SEPARATOR: &str = ":";
const JSON_ARRAY_START: &str = "[";
const JSON_ARRAY_END: &str = "]";
const JSON_OBJECT_START: &str = "{";
const JSON_OBJECT_END: &str = "}";
const JSON_COMMA: &str = ",";
const MARKER_IMPL_PREFIX: &str = "r[impl ";
const MARKER_VERIFY_PREFIX: &str = "r[verify ";
const REQUIREMENT_PREFIX: &str = "r[";
const REQUIREMENT_SUFFIX: &str = "]";
const ROOT_DIFF_FIELD: &str = "<root>";
const ISSUE_BACKEND_UNAVAILABLE: &str = "enforcement_backend_unavailable";
const ISSUE_EXPECTATION_MISMATCH: &str = "fixture_expectation_mismatch";
const ISSUE_EXPECTED_FAILURE_SUCCEEDED: &str = "expected_failure_succeeded";
const ISSUE_EXPECTED_PASS_FAILED: &str = "expected_pass_failed";
const ISSUE_GOLDEN_HASH_MISMATCH: &str = "golden_hash_mismatch";
const ISSUE_INPUT_MISSING: &str = "input_missing";
const ISSUE_MALFORMED_FIXTURE: &str = "malformed_fixture";
const ISSUE_MALFORMED_INPUT: &str = "malformed_input";
const ISSUE_MISSING_IMPLEMENTATION_MARKER: &str = "missing_implementation_marker";
const ISSUE_MISSING_VERIFICATION_MARKER: &str = "missing_verification_marker";
const ISSUE_PROBE_NOT_BLOCKED: &str = "denied_effect_not_blocked";
const ISSUE_PROBE_NOT_RUN: &str = "denied_effect_probe_not_run";
const ISSUE_REPORT_UNNORMALIZABLE: &str = "report_unnormalizable";
const ISSUE_SUITE_CHECK_FAILED: &str = "suite_check_failed";
const ISSUE_UNKNOWN_REQUIREMENT_MARKER: &str = "unknown_requirement_marker";

/// Whether a transcript fixture is expected to observe success or failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureExpectation {
    Pass,
    Fail,
}

/// Deterministic outcome for a harness row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessOutcome {
    Passed,
    Failed,
}

/// A transcript fixture already loaded by an imperative shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptFixture {
    pub path: String,
    pub expectation: FixtureExpectation,
    pub transcript: ReceiptTranscript,
}

/// Report returned when parsing caller-supplied fixture text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptFixtureParseReport {
    pub valid: bool,
    pub fixture: Option<TranscriptFixture>,
    pub issue_codes: Vec<String>,
}

/// Result row for one transcript fixture.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TranscriptFixtureResult {
    pub path: String,
    pub scenario_identity: String,
    pub expectation: FixtureExpectation,
    pub outcome: HarnessOutcome,
    pub observed_valid: bool,
    pub observed_issue_codes: Vec<String>,
    pub actual_hashes: BTreeMap<String, String>,
    pub issue_codes: Vec<String>,
}

/// Explicit refresh entry for transcript fixture review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptRefreshEntry {
    pub path: String,
    pub scenario_identity: String,
    pub expected_report: TranscriptExpectedReport,
    pub expected_hashes: BTreeMap<String, String>,
}

/// Aggregate transcript suite report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptFixtureSuiteReport {
    pub valid: bool,
    pub fixture_count: usize,
    pub pass_fixture_count: usize,
    pub fail_fixture_count: usize,
    pub results: Vec<TranscriptFixtureResult>,
    pub refresh_plan: Vec<TranscriptRefreshEntry>,
    pub issue_codes: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Parse a typed transcript fixture from JSON supplied by the shell.
///
/// r[impl valence.evidence_ir.transcript_fixture_runner]
#[must_use]
pub fn parse_transcript_fixture_json(
    path: impl Into<String>,
    expectation: FixtureExpectation,
    json: &str,
) -> TranscriptFixtureParseReport {
    match serde_json::from_str::<ReceiptTranscript>(json) {
        Ok(transcript) => TranscriptFixtureParseReport {
            valid: true,
            fixture: Some(TranscriptFixture {
                path: path.into(),
                expectation,
                transcript,
            }),
            issue_codes: Vec::new(),
        },
        Err(_) => TranscriptFixtureParseReport {
            valid: false,
            fixture: None,
            issue_codes: vec![ISSUE_MALFORMED_FIXTURE.to_owned()],
        },
    }
}

/// Run transcript fixtures through pure transcript evaluators.
///
/// r[impl valence.evidence_ir.transcript_fixture_runner.pure]
/// r[impl valence.evidence_ir.transcript_fixture_runner.positive]
/// r[impl valence.evidence_ir.transcript_fixture_runner.negative]
/// r[impl valence.evidence_ir.transcript_fixture_runner.stability]
#[must_use]
pub fn run_transcript_fixture_suite(fixtures: Vec<TranscriptFixture>) -> TranscriptFixtureSuiteReport {
    let mut fixtures = fixtures;
    fixtures.sort_by(|left, right| left.path.cmp(&right.path));

    let pass_fixture_count = fixtures.iter().filter(|fixture| fixture.expectation == FixtureExpectation::Pass).count();
    let fail_fixture_count = fixtures.iter().filter(|fixture| fixture.expectation == FixtureExpectation::Fail).count();
    let mut results = Vec::new();
    let mut refresh_plan = Vec::new();

    for fixture in fixtures {
        let evaluation = evaluate_receipt_transcript(&fixture.transcript);
        let mut issue_codes = evaluation.issue_codes.iter().map(|issue| (*issue).to_owned()).collect::<Vec<_>>();
        if !evaluation.valid {
            issue_codes.push(ISSUE_EXPECTATION_MISMATCH.to_owned());
        }
        match fixture.expectation {
            FixtureExpectation::Pass if !evaluation.observed_valid => {
                issue_codes.push(ISSUE_EXPECTED_PASS_FAILED.to_owned());
            }
            FixtureExpectation::Fail if evaluation.observed_valid => {
                issue_codes.push(ISSUE_EXPECTED_FAILURE_SUCCEEDED.to_owned());
            }
            FixtureExpectation::Pass | FixtureExpectation::Fail => {}
        }
        let issue_codes = sorted_dedup(issue_codes);
        let outcome = if issue_codes.is_empty() {
            HarnessOutcome::Passed
        } else {
            HarnessOutcome::Failed
        };
        refresh_plan.push(TranscriptRefreshEntry {
            path: fixture.path.clone(),
            scenario_identity: fixture.transcript.scenario_identity.clone(),
            expected_report: TranscriptExpectedReport {
                valid: evaluation.observed_valid,
                issue_codes: evaluation.observed_issue_codes.clone(),
            },
            expected_hashes: evaluation.actual_hashes.clone(),
        });
        results.push(TranscriptFixtureResult {
            path: fixture.path,
            scenario_identity: fixture.transcript.scenario_identity,
            expectation: fixture.expectation,
            outcome,
            observed_valid: evaluation.observed_valid,
            observed_issue_codes: evaluation.observed_issue_codes,
            actual_hashes: evaluation.actual_hashes,
            issue_codes,
        });
    }

    let issue_codes = sorted_dedup(results.iter().flat_map(|result| result.issue_codes.iter().cloned()).collect());
    TranscriptFixtureSuiteReport {
        valid: issue_codes.is_empty(),
        fixture_count: results.len(),
        pass_fixture_count,
        fail_fixture_count,
        results,
        refresh_plan: sorted_refresh_plan(refresh_plan),
        issue_codes,
        non_claims: vec![TRANSCRIPT_FIXTURE_RUNNER_NON_CLAIM.to_owned()],
    }
}

/// Return a reviewable transcript refresh plan without mutating fixture files.
///
/// r[impl valence.evidence_ir.transcript_fixture_runner.refresh]
#[must_use]
pub fn plan_transcript_fixture_refresh(fixtures: Vec<TranscriptFixture>) -> Vec<TranscriptRefreshEntry> {
    run_transcript_fixture_suite(fixtures).refresh_plan
}

/// Pure malformed-corpus actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MalformedCorpusAction {
    EvidenceDefinition,
    EvidenceGraph,
    NameBindingHistory,
    CapabilityPolicy,
    DurableEnvelope,
}

/// One malformed input case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MalformedCorpusCase {
    pub identity: String,
    pub action: MalformedCorpusAction,
    pub input: serde_json::Value,
    pub expected_issue_codes: Vec<String>,
    pub non_claims: Vec<String>,
}

impl MalformedCorpusCase {
    /// Build a deterministic malformed input case.
    #[must_use]
    pub fn new(
        identity: impl Into<String>,
        action: MalformedCorpusAction,
        input: serde_json::Value,
        expected_issue_codes: Vec<String>,
    ) -> Self {
        Self {
            identity: identity.into(),
            action,
            input,
            expected_issue_codes: sorted_vec(expected_issue_codes),
            non_claims: vec![MALFORMED_CORPUS_NON_CLAIM.to_owned()],
        }
    }
}

/// Malformed corpus row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MalformedCorpusResult {
    pub identity: String,
    pub action: MalformedCorpusAction,
    pub outcome: HarnessOutcome,
    pub observed_valid: bool,
    pub observed_issue_codes: Vec<String>,
    pub issue_codes: Vec<String>,
}

/// Aggregate malformed corpus report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MalformedCorpusReport {
    pub valid: bool,
    pub case_count: usize,
    pub results: Vec<MalformedCorpusResult>,
    pub issue_codes: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Run malformed corpus cases over pure validation cores when possible.
///
/// r[impl valence.evidence_ir.malformed_input_corpus.runner]
/// r[impl valence.evidence_ir.malformed_input_corpus.diagnostics]
#[must_use]
pub fn run_malformed_input_corpus(cases: Vec<MalformedCorpusCase>) -> MalformedCorpusReport {
    let mut cases = cases;
    cases.sort_by(|left, right| left.identity.cmp(&right.identity));
    let results = cases.into_iter().map(evaluate_malformed_case).collect::<Vec<_>>();
    let issue_codes = sorted_dedup(results.iter().flat_map(|result| result.issue_codes.iter().cloned()).collect());
    MalformedCorpusReport {
        valid: issue_codes.is_empty(),
        case_count: results.len(),
        results,
        issue_codes,
        non_claims: vec![MALFORMED_CORPUS_NON_CLAIM.to_owned()],
    }
}

/// Normalized report suitable for golden comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedReport {
    pub canonical_json: String,
    pub content_hash: String,
    pub non_claims: Vec<String>,
}

/// Report normalization result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportNormalizationReport {
    pub valid: bool,
    pub normalized: Option<NormalizedReport>,
    pub issue_codes: Vec<String>,
}

/// Semantic diff between two normalized reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSemanticDiff {
    pub equal: bool,
    pub baseline_hash: String,
    pub candidate_hash: String,
    pub changed_fields: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Report for a checked golden expectation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldenExpectationReport {
    pub valid: bool,
    pub expected_hash: String,
    pub actual_hash: String,
    pub diff: ReportSemanticDiff,
    pub issue_codes: Vec<String>,
}

/// Normalize a JSON report value by sorting fields and removing volatile fields.
///
/// r[impl valence.evidence_ir.golden_report_normalization]
/// r[impl valence.evidence_ir.golden_report_normalization.raw_stdout_boundary]
#[must_use]
pub fn normalize_report_value(value: &serde_json::Value, volatile_fields: &[String]) -> ReportNormalizationReport {
    if !value.is_object() {
        return ReportNormalizationReport {
            valid: false,
            normalized: None,
            issue_codes: vec![ISSUE_REPORT_UNNORMALIZABLE.to_owned()],
        };
    }
    let volatile_fields = volatile_fields.iter().cloned().collect::<BTreeSet<_>>();
    let canonical_json = render_canonical_json(value, &volatile_fields);
    ReportNormalizationReport {
        valid: true,
        normalized: Some(NormalizedReport {
            content_hash: hash_bytes(canonical_json.as_bytes()),
            canonical_json,
            non_claims: vec![GOLDEN_NORMALIZATION_NON_CLAIM.to_owned()],
        }),
        issue_codes: Vec::new(),
    }
}

/// Render a semantic diff between normalized report values.
///
/// r[impl valence.evidence_ir.golden_report_normalization.semantic_diff]
#[must_use]
pub fn diff_normalized_reports(baseline: &NormalizedReport, candidate: &NormalizedReport) -> ReportSemanticDiff {
    let changed_fields = if baseline.content_hash == candidate.content_hash {
        Vec::new()
    } else {
        changed_report_fields(&baseline.canonical_json, &candidate.canonical_json)
    };
    ReportSemanticDiff {
        equal: changed_fields.is_empty(),
        baseline_hash: baseline.content_hash.clone(),
        candidate_hash: candidate.content_hash.clone(),
        changed_fields,
        non_claims: vec![GOLDEN_NORMALIZATION_NON_CLAIM.to_owned()],
    }
}

/// Compare a normalized report to a checked-in hash expectation.
///
/// r[impl valence.evidence_ir.golden_report_normalization.fail_closed]
#[must_use]
pub fn check_golden_expectation(
    expected_hash: &str,
    baseline: &NormalizedReport,
    candidate: &NormalizedReport,
) -> GoldenExpectationReport {
    let diff = diff_normalized_reports(baseline, candidate);
    let mut issue_codes = Vec::new();
    if candidate.content_hash != expected_hash {
        issue_codes.push(ISSUE_GOLDEN_HASH_MISMATCH.to_owned());
    }
    GoldenExpectationReport {
        valid: issue_codes.is_empty(),
        expected_hash: expected_hash.to_owned(),
        actual_hash: candidate.content_hash.clone(),
        diff,
        issue_codes,
    }
}

/// Marker kind for requirement traceability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementMarkerKind {
    Implementation,
    Verification,
}

/// Requirement marker discovered by an imperative shell.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RequirementMarker {
    pub requirement_id: String,
    pub kind: RequirementMarkerKind,
    pub location: String,
}

impl RequirementMarker {
    /// Build a deterministic requirement marker row.
    #[must_use]
    pub fn new(requirement_id: impl Into<String>, kind: RequirementMarkerKind, location: impl Into<String>) -> Self {
        Self {
            requirement_id: requirement_id.into(),
            kind,
            location: location.into(),
        }
    }
}

/// Requirement coverage policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementCoveragePolicy {
    pub fail_on_missing: bool,
    pub fail_on_unknown_markers: bool,
}

/// Coverage status for one requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementCoverageStatus {
    Covered,
    MissingImplementation,
    MissingVerification,
    MissingBoth,
}

/// Requirement coverage row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RequirementCoverageRow {
    pub requirement_id: String,
    pub status: RequirementCoverageStatus,
    pub implementation_markers: Vec<String>,
    pub verification_markers: Vec<String>,
    pub issue_codes: Vec<String>,
}

/// Deterministic requirement coverage report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementCoverageReport {
    pub valid: bool,
    pub rows: Vec<RequirementCoverageRow>,
    pub unknown_markers: Vec<RequirementMarker>,
    pub issue_codes: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Parse implementation/verification markers from caller-supplied text.
///
/// r[impl valence.evidence_ir.requirement_coverage_report.markers]
#[must_use]
pub fn parse_requirement_markers_from_text(location: &str, text: &str) -> Vec<RequirementMarker> {
    let mut markers = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let line_location = format!("{location}:{}", line_index + 1);
        markers.extend(extract_markers(
            line,
            &line_location,
            MARKER_IMPL_PREFIX,
            RequirementMarkerKind::Implementation,
        ));
        markers.extend(extract_markers(
            line,
            &line_location,
            MARKER_VERIFY_PREFIX,
            RequirementMarkerKind::Verification,
        ));
    }
    sorted_vec(markers)
}

/// Compute requirement-to-marker coverage from in-memory values.
///
/// r[impl valence.evidence_ir.requirement_coverage_report]
/// r[impl valence.evidence_ir.requirement_coverage_report.shell]
/// r[impl valence.evidence_ir.requirement_coverage_report.positive]
/// r[impl valence.evidence_ir.requirement_coverage_report.missing]
#[must_use]
pub fn compute_requirement_coverage(
    requirements: Vec<String>,
    markers: Vec<RequirementMarker>,
    policy: RequirementCoveragePolicy,
) -> RequirementCoverageReport {
    let requirements = sorted_dedup(requirements);
    let requirement_set = requirements.iter().cloned().collect::<BTreeSet<_>>();
    let mut markers_by_requirement: BTreeMap<String, Vec<RequirementMarker>> = BTreeMap::new();
    let mut unknown_markers = Vec::new();
    for marker in markers {
        if requirement_set.contains(&marker.requirement_id) {
            markers_by_requirement.entry(marker.requirement_id.clone()).or_default().push(marker);
        } else {
            unknown_markers.push(marker);
        }
    }

    let mut rows = Vec::new();
    for requirement_id in requirements {
        let markers = markers_by_requirement.remove(&requirement_id).unwrap_or_default();
        let implementation_markers = markers
            .iter()
            .filter(|marker| marker.kind == RequirementMarkerKind::Implementation)
            .map(|marker| marker.location.clone())
            .collect::<Vec<_>>();
        let verification_markers = markers
            .iter()
            .filter(|marker| marker.kind == RequirementMarkerKind::Verification)
            .map(|marker| marker.location.clone())
            .collect::<Vec<_>>();
        let status = coverage_status(&implementation_markers, &verification_markers);
        let mut issue_codes = Vec::new();
        match status {
            RequirementCoverageStatus::Covered => {}
            RequirementCoverageStatus::MissingImplementation => {
                issue_codes.push(ISSUE_MISSING_IMPLEMENTATION_MARKER.to_owned());
            }
            RequirementCoverageStatus::MissingVerification => {
                issue_codes.push(ISSUE_MISSING_VERIFICATION_MARKER.to_owned());
            }
            RequirementCoverageStatus::MissingBoth => {
                issue_codes.push(ISSUE_MISSING_IMPLEMENTATION_MARKER.to_owned());
                issue_codes.push(ISSUE_MISSING_VERIFICATION_MARKER.to_owned());
            }
        }
        rows.push(RequirementCoverageRow {
            requirement_id,
            status,
            implementation_markers: sorted_vec(implementation_markers),
            verification_markers: sorted_vec(verification_markers),
            issue_codes: sorted_dedup(issue_codes),
        });
    }

    let mut issue_codes = Vec::new();
    if policy.fail_on_missing {
        issue_codes.extend(rows.iter().flat_map(|row| row.issue_codes.iter().cloned()));
    }
    if policy.fail_on_unknown_markers && !unknown_markers.is_empty() {
        issue_codes.push(ISSUE_UNKNOWN_REQUIREMENT_MARKER.to_owned());
    }
    RequirementCoverageReport {
        valid: issue_codes.is_empty(),
        rows,
        unknown_markers: sorted_vec(unknown_markers),
        issue_codes: sorted_dedup(issue_codes),
        non_claims: vec![REQUIREMENT_COVERAGE_NON_CLAIM.to_owned()],
    }
}

/// Denied effect classes for opt-in sandbox probes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeniedEffectKind {
    Filesystem,
    Process,
    Network,
    Clock,
    RemoteBuilder,
    Secret,
}

/// Enforcement backend capability summary supplied by the shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnforcementBackend {
    pub name: String,
    pub available: bool,
    pub supported_effects: Vec<DeniedEffectKind>,
}

impl EnforcementBackend {
    /// Create a deterministic backend availability row.
    #[must_use]
    pub fn new(name: impl Into<String>, available: bool, supported_effects: Vec<DeniedEffectKind>) -> Self {
        Self {
            name: name.into(),
            available,
            supported_effects: sorted_dedup(supported_effects),
        }
    }
}

/// One denied-effect probe requested by an opt-in deep/nightly lane.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeniedEffectProbe {
    pub identity: String,
    pub effect: DeniedEffectKind,
    pub opt_in: bool,
}

/// Observed result for a denied-effect probe supplied by a sandbox shell.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObservedDeniedEffect {
    pub identity: String,
    pub blocked: bool,
}

/// Stable status labels shared by suite and enforcement reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessStatus {
    Pass,
    Fail,
    Skipped,
    Unavailable,
}

/// Denied-effect probe row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeniedEffectProbeResult {
    pub identity: String,
    pub effect: DeniedEffectKind,
    pub status: HarnessStatus,
    pub issue_codes: Vec<String>,
}

/// Capability enforcement harness report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEnforcementReport {
    pub valid: bool,
    pub backend_name: String,
    pub backend_available: bool,
    pub results: Vec<DeniedEffectProbeResult>,
    pub issue_codes: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Evaluate opt-in denied-effect probes without claiming missing sandbox support as success.
///
/// r[impl valence.evidence_ir.capability_enforcement_harness.availability]
/// r[impl valence.evidence_ir.capability_enforcement_harness.denied_effects]
/// r[impl valence.evidence_ir.capability_enforcement_harness.no_false_success]
#[must_use]
pub fn evaluate_denied_effect_probes(
    backend: EnforcementBackend,
    probes: Vec<DeniedEffectProbe>,
    observed: Vec<ObservedDeniedEffect>,
) -> CapabilityEnforcementReport {
    let observed = observed.into_iter().map(|result| (result.identity, result.blocked)).collect::<BTreeMap<_, _>>();
    let supported = backend.supported_effects.iter().copied().collect::<BTreeSet<_>>();
    let mut results = probes
        .into_iter()
        .map(|probe| evaluate_denied_effect_probe(&backend, &supported, &observed, probe))
        .collect::<Vec<_>>();
    results.sort();
    let issue_codes = sorted_dedup(results.iter().flat_map(|result| result.issue_codes.iter().cloned()).collect());
    CapabilityEnforcementReport {
        valid: !issue_codes.iter().any(|issue| issue == ISSUE_PROBE_NOT_BLOCKED),
        backend_name: backend.name,
        backend_available: backend.available,
        results,
        issue_codes,
        non_claims: vec![CAPABILITY_ENFORCEMENT_NON_CLAIM.to_owned()],
    }
}

/// Default policy used by pure label-policy harness tests.
///
/// r[impl valence.evidence_ir.capability_enforcement_harness.policy]
#[must_use]
pub fn default_denied_effect_policy() -> CapabilityPolicy {
    CapabilityPolicy::new(
        vec![
            VerifierCapabilityLabel::new(VerifierCapabilityKind::Filesystem),
            VerifierCapabilityLabel::new(VerifierCapabilityKind::Clock),
            VerifierCapabilityLabel::new(VerifierCapabilityKind::Process),
            VerifierCapabilityLabel::new(VerifierCapabilityKind::Network),
            VerifierCapabilityLabel::new(VerifierCapabilityKind::RemoteBuilder),
            VerifierCapabilityLabel::new(VerifierCapabilityKind::Secret),
        ],
        Vec::new(),
    )
}

/// Test suite lane kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestSuiteLaneKind {
    Fast,
    Deep,
    Nightly,
}

/// Stable lane definition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TestSuiteLaneDefinition {
    pub lane: TestSuiteLaneKind,
    pub command: String,
    pub hermetic: bool,
    pub includes_optional_integrations: bool,
    pub requires_sibling_repositories: bool,
    pub requires_host_sandbox: bool,
    pub non_claims: Vec<String>,
}

/// Suite check result supplied by a shell/CI harness.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SuiteCheckResult {
    pub lane: TestSuiteLaneKind,
    pub name: String,
    pub status: HarnessStatus,
}

/// Stable suite lane summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuiteLaneSummary {
    pub valid: bool,
    pub results: Vec<SuiteCheckResult>,
    pub issue_codes: Vec<String>,
    pub non_claims: Vec<String>,
}

/// Define fast, deep, and nightly lanes with stable documented commands.
///
/// r[impl valence.evidence_ir.test_suite_partition]
/// r[impl valence.evidence_ir.test_suite_partition.fast]
/// r[impl valence.evidence_ir.test_suite_partition.deep]
/// r[impl valence.evidence_ir.test_suite_partition.ci]
#[must_use]
pub fn default_test_suite_lanes() -> Vec<TestSuiteLaneDefinition> {
    vec![
        TestSuiteLaneDefinition {
            lane: TestSuiteLaneKind::Fast,
            command: FAST_LANE_COMMAND.to_owned(),
            hermetic: true,
            includes_optional_integrations: false,
            requires_sibling_repositories: false,
            requires_host_sandbox: false,
            non_claims: vec![TEST_SUITE_PARTITION_NON_CLAIM.to_owned()],
        },
        TestSuiteLaneDefinition {
            lane: TestSuiteLaneKind::Deep,
            command: DEEP_LANE_COMMAND.to_owned(),
            hermetic: false,
            includes_optional_integrations: true,
            requires_sibling_repositories: false,
            requires_host_sandbox: true,
            non_claims: vec![TEST_SUITE_PARTITION_NON_CLAIM.to_owned()],
        },
        TestSuiteLaneDefinition {
            lane: TestSuiteLaneKind::Nightly,
            command: NIGHTLY_LANE_COMMAND.to_owned(),
            hermetic: false,
            includes_optional_integrations: true,
            requires_sibling_repositories: true,
            requires_host_sandbox: true,
            non_claims: vec![TEST_SUITE_PARTITION_NON_CLAIM.to_owned()],
        },
    ]
}

/// Summarize suite check results with deterministic ordering and status labels.
///
/// r[impl valence.evidence_ir.test_suite_partition.reporting]
#[must_use]
pub fn summarize_suite_lane_results(results: Vec<SuiteCheckResult>) -> SuiteLaneSummary {
    let mut results = results;
    results.sort();
    let mut issue_codes = Vec::new();
    if results.iter().any(|result| result.status == HarnessStatus::Fail) {
        issue_codes.push(ISSUE_SUITE_CHECK_FAILED.to_owned());
    }
    SuiteLaneSummary {
        valid: issue_codes.is_empty(),
        results,
        issue_codes,
        non_claims: vec![TEST_SUITE_PARTITION_NON_CLAIM.to_owned()],
    }
}

fn evaluate_malformed_case(case: MalformedCorpusCase) -> MalformedCorpusResult {
    let (observed_valid, observed_issue_codes) = run_malformed_case_action(&case);
    let expected_issue_codes = sorted_vec(case.expected_issue_codes);
    let mut issue_codes = Vec::new();
    if observed_valid {
        issue_codes.push(ISSUE_EXPECTED_FAILURE_SUCCEEDED.to_owned());
    }
    if observed_issue_codes != expected_issue_codes {
        issue_codes.push(ISSUE_EXPECTATION_MISMATCH.to_owned());
    }
    let issue_codes = sorted_dedup(issue_codes);
    MalformedCorpusResult {
        identity: case.identity,
        action: case.action,
        outcome: if issue_codes.is_empty() {
            HarnessOutcome::Passed
        } else {
            HarnessOutcome::Failed
        },
        observed_valid,
        observed_issue_codes,
        issue_codes,
    }
}

fn run_malformed_case_action(case: &MalformedCorpusCase) -> (bool, Vec<String>) {
    match case.action {
        MalformedCorpusAction::EvidenceDefinition => {
            match serde_json::from_value::<EvidenceDefinition>(case.input.clone()) {
                Ok(definition) => {
                    let report = validate_evidence_definition(&definition);
                    report_to_strings(report.valid, report.issue_codes)
                }
                Err(_) => (false, vec![ISSUE_MALFORMED_INPUT.to_owned()]),
            }
        }
        MalformedCorpusAction::EvidenceGraph => match serde_json::from_value::<EvidenceGraph>(case.input.clone()) {
            Ok(graph) => {
                let report = validate_evidence_graph(&graph);
                (report.valid, report.issue_codes.into_iter().map(ToOwned::to_owned).collect())
            }
            Err(_) => (false, vec![ISSUE_MALFORMED_INPUT.to_owned()]),
        },
        MalformedCorpusAction::NameBindingHistory => run_name_binding_case(case),
        MalformedCorpusAction::CapabilityPolicy => run_capability_policy_case(case),
        MalformedCorpusAction::DurableEnvelope => run_durable_envelope_case(case),
    }
}

fn run_name_binding_case(case: &MalformedCorpusCase) -> (bool, Vec<String>) {
    let Some(history_value) = case.input.get("history") else {
        return (false, vec![ISSUE_INPUT_MISSING.to_owned()]);
    };
    let Some(known_value) = case.input.get("known_identities") else {
        return (false, vec![ISSUE_INPUT_MISSING.to_owned()]);
    };
    let Ok(history) = serde_json::from_value::<NameBindingHistory>(history_value.clone()) else {
        return (false, vec![ISSUE_MALFORMED_INPUT.to_owned()]);
    };
    let Ok(known_identities) = serde_json::from_value::<BTreeSet<String>>(known_value.clone()) else {
        return (false, vec![ISSUE_MALFORMED_INPUT.to_owned()]);
    };
    let report = validate_name_bindings(&history, &known_identities);
    (report.valid, report.issue_codes.into_iter().map(ToOwned::to_owned).collect())
}

fn run_capability_policy_case(case: &MalformedCorpusCase) -> (bool, Vec<String>) {
    let Some(capabilities_value) = case.input.get("capabilities") else {
        return (false, vec![ISSUE_INPUT_MISSING.to_owned()]);
    };
    let Some(policy_value) = case.input.get("policy") else {
        return (false, vec![ISSUE_INPUT_MISSING.to_owned()]);
    };
    let Ok(capabilities) = serde_json::from_value::<VerifierCapabilitySet>(capabilities_value.clone()) else {
        return (false, vec![ISSUE_MALFORMED_INPUT.to_owned()]);
    };
    let Ok(policy) = serde_json::from_value::<CapabilityPolicy>(policy_value.clone()) else {
        return (false, vec![ISSUE_MALFORMED_INPUT.to_owned()]);
    };
    let report = evaluate_capability_policy(&capabilities, &policy);
    (report.valid, report.issue_codes.into_iter().map(ToOwned::to_owned).collect())
}

fn run_durable_envelope_case(case: &MalformedCorpusCase) -> (bool, Vec<String>) {
    let Some(envelope_value) = case.input.get("envelope") else {
        return (false, vec![ISSUE_INPUT_MISSING.to_owned()]);
    };
    let Some(expected_value_type) = case.input.get("expected_value_type").and_then(serde_json::Value::as_str) else {
        return (false, vec![ISSUE_INPUT_MISSING.to_owned()]);
    };
    let Some(expected_codec_value) = case.input.get("expected_codec") else {
        return (false, vec![ISSUE_INPUT_MISSING.to_owned()]);
    };
    let migration_required = case.input.get("migration_required").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let Ok(envelope) = serde_json::from_value::<DurableEvidenceEnvelope>(envelope_value.clone()) else {
        return (false, vec![ISSUE_MALFORMED_INPUT.to_owned()]);
    };
    let Ok(expected_codec) = serde_json::from_value::<DurableStorageCodec>(expected_codec_value.clone()) else {
        return (false, vec![ISSUE_MALFORMED_INPUT.to_owned()]);
    };
    let report = decode_durable_value(&envelope, expected_value_type, expected_codec, migration_required);
    (report.valid, report.issue_codes.into_iter().map(ToOwned::to_owned).collect())
}

fn report_to_strings(valid: bool, issue_codes: Vec<&'static str>) -> (bool, Vec<String>) {
    (valid, issue_codes.into_iter().map(ToOwned::to_owned).collect())
}

fn render_canonical_json(value: &serde_json::Value, volatile_fields: &BTreeSet<String>) -> String {
    match value {
        serde_json::Value::Null => "null".to_owned(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => serde_json::to_string(value).expect("serialize string value"),
        serde_json::Value::Array(values) => render_canonical_array(values, volatile_fields),
        serde_json::Value::Object(fields) => render_canonical_object(fields, volatile_fields),
    }
}

fn render_canonical_array(values: &[serde_json::Value], volatile_fields: &BTreeSet<String>) -> String {
    let rendered = values
        .iter()
        .map(|value| render_canonical_json(value, volatile_fields))
        .collect::<Vec<_>>()
        .join(JSON_COMMA);
    format!("{JSON_ARRAY_START}{rendered}{JSON_ARRAY_END}")
}

fn render_canonical_object(
    fields: &serde_json::Map<String, serde_json::Value>,
    volatile_fields: &BTreeSet<String>,
) -> String {
    let mut entries = fields.iter().filter(|(key, _)| !volatile_fields.contains(*key)).collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(right.0));
    let rendered = entries
        .into_iter()
        .map(|(key, value)| {
            let key_json = serde_json::to_string(key).expect("serialize object key");
            let value_json = render_canonical_json(value, volatile_fields);
            format!("{key_json}{FIELD_SEPARATOR}{value_json}")
        })
        .collect::<Vec<_>>()
        .join(JSON_COMMA);
    format!("{JSON_OBJECT_START}{rendered}{JSON_OBJECT_END}")
}

fn changed_report_fields(baseline_json: &str, candidate_json: &str) -> Vec<String> {
    let baseline = serde_json::from_str::<serde_json::Value>(baseline_json).unwrap_or(serde_json::Value::Null);
    let candidate = serde_json::from_str::<serde_json::Value>(candidate_json).unwrap_or(serde_json::Value::Null);
    match (baseline.as_object(), candidate.as_object()) {
        (Some(baseline), Some(candidate)) => {
            let keys = baseline.keys().chain(candidate.keys()).cloned().collect::<BTreeSet<_>>();
            keys.into_iter().filter(|key| baseline.get(key) != candidate.get(key)).collect()
        }
        _ if baseline == candidate => Vec::new(),
        _ => vec![ROOT_DIFF_FIELD.to_owned()],
    }
}

fn coverage_status(implementation_markers: &[String], verification_markers: &[String]) -> RequirementCoverageStatus {
    match (implementation_markers.is_empty(), verification_markers.is_empty()) {
        (false, false) => RequirementCoverageStatus::Covered,
        (true, false) => RequirementCoverageStatus::MissingImplementation,
        (false, true) => RequirementCoverageStatus::MissingVerification,
        (true, true) => RequirementCoverageStatus::MissingBoth,
    }
}

fn extract_markers(line: &str, location: &str, prefix: &str, kind: RequirementMarkerKind) -> Vec<RequirementMarker> {
    let mut markers = Vec::new();
    let mut remainder = line;
    while let Some(start) = remainder.find(prefix) {
        let after_prefix = &remainder[start + prefix.len()..];
        let Some(end) = after_prefix.find(REQUIREMENT_SUFFIX) else {
            break;
        };
        let raw_id = after_prefix[..end].trim();
        if !raw_id.is_empty() {
            markers.push(RequirementMarker::new(
                format!("{REQUIREMENT_PREFIX}{raw_id}{REQUIREMENT_SUFFIX}"),
                kind,
                location,
            ));
        }
        remainder = &after_prefix[end + REQUIREMENT_SUFFIX.len()..];
    }
    markers
}

fn evaluate_denied_effect_probe(
    backend: &EnforcementBackend,
    supported: &BTreeSet<DeniedEffectKind>,
    observed: &BTreeMap<String, bool>,
    probe: DeniedEffectProbe,
) -> DeniedEffectProbeResult {
    if !probe.opt_in {
        return DeniedEffectProbeResult {
            identity: probe.identity,
            effect: probe.effect,
            status: HarnessStatus::Skipped,
            issue_codes: Vec::new(),
        };
    }
    if !backend.available || !supported.contains(&probe.effect) {
        return DeniedEffectProbeResult {
            identity: probe.identity,
            effect: probe.effect,
            status: HarnessStatus::Unavailable,
            issue_codes: vec![ISSUE_BACKEND_UNAVAILABLE.to_owned()],
        };
    }
    match observed.get(&probe.identity) {
        Some(true) => DeniedEffectProbeResult {
            identity: probe.identity,
            effect: probe.effect,
            status: HarnessStatus::Pass,
            issue_codes: Vec::new(),
        },
        Some(false) => DeniedEffectProbeResult {
            identity: probe.identity,
            effect: probe.effect,
            status: HarnessStatus::Fail,
            issue_codes: vec![ISSUE_PROBE_NOT_BLOCKED.to_owned()],
        },
        None => DeniedEffectProbeResult {
            identity: probe.identity,
            effect: probe.effect,
            status: HarnessStatus::Skipped,
            issue_codes: vec![ISSUE_PROBE_NOT_RUN.to_owned()],
        },
    }
}

fn sorted_refresh_plan(mut values: Vec<TranscriptRefreshEntry>) -> Vec<TranscriptRefreshEntry> {
    values.sort_by(|left, right| left.path.cmp(&right.path));
    values
}

fn sorted_vec<T>(mut values: Vec<T>) -> Vec<T>
where T: Ord {
    values.sort();
    values
}

fn sorted_dedup<T>(mut values: Vec<T>) -> Vec<T>
where T: Ord {
    values.sort();
    values.dedup();
    values
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{}{}", BLAKE3_IDENTITY_PREFIX, blake3::hash(bytes).to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_ir::REQUIRED_NON_CLAIM;
    use crate::evidence_ir::VerificationRole;
    use crate::evidence_ir::build_evidence_artifact;
    use crate::evidence_model::CAPABILITY_NON_CLAIM;
    use crate::evidence_model::CONTENT_IDENTITY_NON_CLAIM;
    use crate::evidence_model::DURABLE_STORAGE_NON_CLAIM;
    use crate::evidence_model::DurableStorageCodec;
    use crate::evidence_model::EvidenceDefinitionMetadata;
    use crate::evidence_model::EvidenceDependency;
    use crate::evidence_model::EvidenceGraphEdge;
    use crate::evidence_model::EvidenceGraphEdgeKind;
    use crate::evidence_model::EvidenceGraphNode;
    use crate::evidence_model::EvidenceGraphNodeKind;
    use crate::evidence_model::EvidenceNameBinding;
    use crate::evidence_model::NameBindingKind;
    use crate::evidence_model::ReceiptTranscript;
    use crate::evidence_model::TranscriptVerifierAction;
    use crate::evidence_model::VerifierCacheKey;
    use crate::evidence_model::VerifierCapabilityKind;
    use crate::evidence_model::build_evidence_definition;
    use crate::evidence_model::decide_cache_reuse;
    use crate::evidence_model::encode_durable_value;
    use crate::evidence_model::evidence_definition_identity;
    use crate::evidence_model::evidence_graph_identity;
    use crate::evidence_model::resolve_name_binding;
    use crate::stack_provenance_adapter::BUILT_BINARY_THREAD_NON_CLAIM;
    use crate::stack_provenance_adapter::BuiltBinaryThreadInput;
    use crate::stack_provenance_adapter::MANTLE_STACK_PROVENANCE_ROLE;
    use crate::stack_provenance_adapter::MantleReleaseManifestView;
    use crate::stack_provenance_adapter::OCTET_VALENCE_EXPORT_SCHEMA;
    use crate::stack_provenance_adapter::OctetProofCard;
    use crate::stack_provenance_adapter::OctetProvenanceRecord;
    use crate::stack_provenance_adapter::OctetValenceProvenanceExport;
    use crate::stack_provenance_adapter::STACK_PROVENANCE_CLAIM_SCOPE;
    use crate::stack_provenance_adapter::STACK_PROVENANCE_NON_CLAIM;
    use crate::stack_provenance_adapter::STACK_PROVENANCE_SIDECAR_SCHEMA;
    use crate::stack_provenance_adapter::StackArtifactIdentity;
    use crate::stack_provenance_adapter::build_stack_provenance_sidecar;
    use crate::stack_provenance_adapter::verify_built_binary_thread;

    const IDENTITY_ALPHA: &str = "b3:alpha";
    const IDENTITY_BETA: &str = "b3:beta";
    const IDENTITY_GAMMA: &str = "b3:gamma";
    const POLICY_IDENTITY: &str = "b3:policy";
    const REQUIREMENT_IDENTITY: &str = "r[valence.example]";
    const TYPE_IDENTITY: &str = "valence.example-value.v1";
    const STATUS_HASH: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const RESULTS_HASH: &str = "3333333333333333333333333333333333333333333333333333333333333333";
    const INITIAL_SEQUENCE: u64 = 1;
    const RENAMED_SEQUENCE: u64 = 2;
    const EXPECTED_ONE: usize = 1;
    const EXPECTED_TWO: usize = 2;
    const EXPECTED_THREE: usize = 3;
    const EXPECTED_SIX: usize = 6;

    // r[verify valence.evidence_ir.transcript_fixture_runner]
    // r[verify valence.evidence_ir.transcript_fixture_runner.positive]
    // r[verify valence.evidence_ir.transcript_fixture_runner.negative]
    // r[verify valence.evidence_ir.transcript_fixture_runner.stability]
    #[test]
    fn transcript_fixture_runner_loads_checked_in_pass_and_fail_fixtures() {
        let pass = parse_transcript_fixture_json(
            "testdata/transcripts/pass/capability-policy-pass.json",
            FixtureExpectation::Pass,
            include_str!("../testdata/transcripts/pass/capability-policy-pass.json"),
        );
        let fail = parse_transcript_fixture_json(
            "testdata/transcripts/fail/malformed-artifact.json",
            FixtureExpectation::Fail,
            include_str!("../testdata/transcripts/fail/malformed-artifact.json"),
        );
        assert!(pass.valid);
        assert!(fail.valid);
        let report = run_transcript_fixture_suite(vec![pass.fixture.unwrap(), fail.fixture.unwrap()]);

        assert!(report.valid, "unexpected issues: {:?}", report.issue_codes);
        assert_eq!(report.fixture_count, EXPECTED_TWO);
        assert_eq!(report.pass_fixture_count, EXPECTED_ONE);
        assert_eq!(report.fail_fixture_count, EXPECTED_ONE);
        assert_eq!(report.results[0].path, "testdata/transcripts/fail/malformed-artifact.json");
        assert!(report.non_claims.contains(&TRANSCRIPT_FIXTURE_RUNNER_NON_CLAIM.to_owned()));
    }

    // r[verify valence.evidence_ir.transcript_fixture_runner.refresh]
    #[test]
    fn transcript_refresh_plan_is_explicit_and_deterministic() {
        let fixture = TranscriptFixture {
            path: "generated/valid-artifact.json".to_owned(),
            expectation: FixtureExpectation::Pass,
            transcript: evidence_ir_transcript(),
        };
        let plan = plan_transcript_fixture_refresh(vec![fixture]);

        assert_eq!(plan.len(), EXPECTED_ONE);
        assert_eq!(plan[0].scenario_identity, "valid-artifact");
        assert!(plan[0].expected_report.valid);
        assert!(plan[0].expected_hashes.contains_key("artifact_json"));
    }

    // r[verify valence.evidence_ir.transcript_fixture_runner.pure]
    #[test]
    fn transcript_runner_supports_stack_provenance_identity_checks() {
        let transcript = stack_provenance_transcript();
        let report = evaluate_receipt_transcript(&transcript);

        assert!(report.valid, "unexpected issues: {:?}", report.issue_codes);
        assert!(report.observed_valid);
        assert!(report.actual_hashes.contains_key("binary_hash"));
        assert!(report.actual_hashes.contains_key("sidecar_digest_blake3"));
    }

    // r[verify valence.evidence_ir.property_invariants]
    // r[verify valence.evidence_ir.property_invariants.rename_stability]
    // r[verify valence.evidence_ir.property_invariants.dependency_sensitivity]
    // r[verify valence.evidence_ir.property_invariants.graph_determinism]
    // r[verify valence.evidence_ir.property_invariants.alias_fail_closed]
    // r[verify valence.evidence_ir.property_invariants.cache_key_equality]
    #[test]
    fn property_invariants_cover_identity_graph_alias_and_cache_boundaries() {
        let original = content_definition("alpha", "src/old.rs", IDENTITY_BETA);
        let renamed = content_definition("renamed", "src/new.rs", IDENTITY_BETA);
        let changed_dependency = content_definition("alpha", "src/old.rs", IDENTITY_GAMMA);
        assert_eq!(original.content_identity, renamed.content_identity);
        assert_ne!(original.content_identity, changed_dependency.content_identity);

        let graph = graph();
        let mut reordered = graph.clone();
        reordered.nodes.reverse();
        reordered.edges.reverse();
        assert_eq!(evidence_graph_identity(&graph).unwrap(), evidence_graph_identity(&reordered).unwrap());

        let ambiguous = NameBindingHistory::new(vec![
            binding("alpha", IDENTITY_ALPHA, INITIAL_SEQUENCE, true),
            binding("alpha", IDENTITY_BETA, RENAMED_SEQUENCE, true),
        ]);
        let resolution = resolve_name_binding(&ambiguous, "alpha");
        assert!(!resolution.valid());
        assert!(resolution.issue_codes.contains(&"alias_ambiguous"));

        let key =
            VerifierCacheKey::new(IDENTITY_ALPHA, "b3:verifier", POLICY_IDENTITY, IDENTITY_BETA, "b3:capabilities");
        let mut changed = key.clone();
        changed.capability_set_hash = "b3:other-capabilities".to_owned();
        let capabilities = VerifierCapabilitySet::new(vec![VerifierCapabilityLabel::new(VerifierCapabilityKind::Pure)]);
        let policy = default_denied_effect_policy();
        let decision = decide_cache_reuse(None, &changed, &capabilities, &policy);
        assert_eq!(decision.issue_codes, vec!["cache_entry_missing"]);
    }

    // r[verify valence.evidence_ir.malformed_input_corpus]
    // r[verify valence.evidence_ir.malformed_input_corpus.runner]
    // r[verify valence.evidence_ir.malformed_input_corpus.diagnostics]
    // r[verify valence.evidence_ir.malformed_input_corpus.digest_and_graph]
    // r[verify valence.evidence_ir.malformed_input_corpus.bindings_and_capabilities]
    // r[verify valence.evidence_ir.malformed_input_corpus.boundary_and_storage]
    #[test]
    fn malformed_input_corpus_reports_expected_failures() {
        let graph_case = MalformedCorpusCase::new(
            "duplicate-node-dangling-edge",
            MalformedCorpusAction::EvidenceGraph,
            serde_json::to_value(broken_graph()).unwrap(),
            vec!["dangling_edge".to_owned(), "duplicate_node_identity".to_owned()],
        );
        let binding_case = MalformedCorpusCase::new(
            "stale-binding",
            MalformedCorpusAction::NameBindingHistory,
            serde_json::json!({
                "history": NameBindingHistory::new(vec![binding("missing", IDENTITY_GAMMA, INITIAL_SEQUENCE, true)]),
                "known_identities": [IDENTITY_ALPHA]
            }),
            vec!["binding_stale".to_owned()],
        );
        let capability_case = MalformedCorpusCase::new(
            "unknown-denied-capability",
            MalformedCorpusAction::CapabilityPolicy,
            serde_json::json!({
                "capabilities": VerifierCapabilitySet::new(vec![
                    VerifierCapabilityLabel::new(VerifierCapabilityKind::Network),
                    VerifierCapabilityLabel::unknown("gpu")
                ]),
                "policy": default_denied_effect_policy()
            }),
            vec!["capability_denied".to_owned(), "unknown_capability".to_owned()],
        );
        let durable_case = MalformedCorpusCase::new(
            "durable-envelope-mismatch",
            MalformedCorpusAction::DurableEnvelope,
            durable_mismatch_input(),
            vec!["content_hash_mismatch".to_owned(), "value_type_mismatch".to_owned()],
        );
        let report = run_malformed_input_corpus(vec![capability_case, durable_case, graph_case, binding_case]);

        assert!(report.valid, "unexpected issues: {:?}", report.issue_codes);
        assert_eq!(report.case_count, EXPECTED_FOUR);
        assert_eq!(report.results[0].identity, "duplicate-node-dangling-edge");
        assert!(report.non_claims.contains(&MALFORMED_CORPUS_NON_CLAIM.to_owned()));
    }

    // r[verify valence.evidence_ir.golden_report_normalization]
    // r[verify valence.evidence_ir.golden_report_normalization.semantic_diff]
    // r[verify valence.evidence_ir.golden_report_normalization.stability]
    // r[verify valence.evidence_ir.golden_report_normalization.fail_closed]
    // r[verify valence.evidence_ir.golden_report_normalization.refresh]
    #[test]
    fn golden_normalization_is_stable_and_diffs_semantic_fields() {
        let baseline = serde_json::json!({
            "generated_at": "now",
            "issue_codes": ["b", "a"],
            "valid": false
        });
        let reordered = serde_json::json!({
            "valid": false,
            "issue_codes": ["b", "a"],
            "generated_at": "later"
        });
        let volatile = vec!["generated_at".to_owned()];
        let normalized = normalize_report_value(&baseline, &volatile).normalized.unwrap();
        let reordered_normalized = normalize_report_value(&reordered, &volatile).normalized.unwrap();
        assert_eq!(normalized, reordered_normalized);

        let candidate = normalize_report_value(&serde_json::json!({"valid": true, "issue_codes": []}), &[])
            .normalized
            .unwrap();
        let diff = diff_normalized_reports(&normalized, &candidate);
        assert!(!diff.equal);
        assert!(diff.changed_fields.contains(&"valid".to_owned()));

        let stale = check_golden_expectation("b3:stale", &normalized, &candidate);
        assert!(!stale.valid);
        assert!(stale.issue_codes.contains(&ISSUE_GOLDEN_HASH_MISMATCH.to_owned()));
    }

    // r[verify valence.evidence_ir.golden_report_normalization.raw_stdout_boundary]
    #[test]
    fn golden_normalization_rejects_raw_stdout_strings() {
        let report = normalize_report_value(&serde_json::json!("valid: true\n"), &[]);

        assert!(!report.valid);
        assert!(report.issue_codes.contains(&ISSUE_REPORT_UNNORMALIZABLE.to_owned()));
    }

    // r[verify valence.evidence_ir.requirement_coverage_report]
    // r[verify valence.evidence_ir.requirement_coverage_report.markers]
    // r[verify valence.evidence_ir.requirement_coverage_report.shell]
    // r[verify valence.evidence_ir.requirement_coverage_report.positive]
    // r[verify valence.evidence_ir.requirement_coverage_report.missing]
    // r[verify valence.evidence_ir.requirement_coverage_report.non_claims]
    #[test]
    fn requirement_coverage_rows_are_deterministic_and_fail_when_required() {
        let text = "// r[verify valence.example]\n// r[impl valence.example]\n// r[verify valence.unknown]";
        let markers = parse_requirement_markers_from_text("src/lib.rs", text);
        let advisory = compute_requirement_coverage(
            vec![REQUIREMENT_IDENTITY.to_owned(), "r[valence.missing]".to_owned()],
            markers.clone(),
            RequirementCoveragePolicy {
                fail_on_missing: false,
                fail_on_unknown_markers: false,
            },
        );
        assert!(advisory.valid);
        assert_eq!(advisory.rows[0].requirement_id, REQUIREMENT_IDENTITY);
        assert_eq!(advisory.rows[0].status, RequirementCoverageStatus::Covered);
        assert_eq!(advisory.rows[1].status, RequirementCoverageStatus::MissingBoth);

        let strict = compute_requirement_coverage(
            vec![REQUIREMENT_IDENTITY.to_owned(), "r[valence.missing]".to_owned()],
            markers,
            RequirementCoveragePolicy {
                fail_on_missing: true,
                fail_on_unknown_markers: true,
            },
        );
        assert!(!strict.valid);
        assert!(strict.issue_codes.contains(&ISSUE_MISSING_IMPLEMENTATION_MARKER.to_owned()));
        assert!(strict.issue_codes.contains(&ISSUE_MISSING_VERIFICATION_MARKER.to_owned()));
        assert!(strict.issue_codes.contains(&ISSUE_UNKNOWN_REQUIREMENT_MARKER.to_owned()));
        assert!(strict.non_claims.contains(&REQUIREMENT_COVERAGE_NON_CLAIM.to_owned()));
    }

    // r[verify valence.evidence_ir.capability_enforcement_harness]
    // r[verify valence.evidence_ir.capability_enforcement_harness.availability]
    // r[verify valence.evidence_ir.capability_enforcement_harness.policy]
    // r[verify valence.evidence_ir.capability_enforcement_harness.denied_effects]
    // r[verify valence.evidence_ir.capability_enforcement_harness.no_false_success]
    // r[verify valence.evidence_ir.capability_enforcement_harness.non_claims]
    #[test]
    fn capability_enforcement_separates_labels_from_sandbox_probes() {
        let policy = default_denied_effect_policy();
        let pure_report = evaluate_capability_policy(
            &VerifierCapabilitySet::new(vec![VerifierCapabilityLabel::new(VerifierCapabilityKind::Pure)]),
            &policy,
        );
        assert!(pure_report.valid);

        let denied_report = evaluate_capability_policy(
            &VerifierCapabilitySet::new(vec![VerifierCapabilityLabel::new(VerifierCapabilityKind::Filesystem)]),
            &policy,
        );
        assert!(!denied_report.valid);
        assert!(denied_report.issue_codes.contains(&"capability_denied"));

        let unavailable = evaluate_denied_effect_probes(
            EnforcementBackend::new("none", false, Vec::new()),
            all_denied_effect_probes(true),
            Vec::new(),
        );
        assert!(unavailable.valid);
        assert_eq!(unavailable.results.len(), EXPECTED_SIX);
        assert!(unavailable.results.iter().all(|result| result.status == HarnessStatus::Unavailable));

        let failed = evaluate_denied_effect_probes(
            EnforcementBackend::new("sandbox", true, vec![DeniedEffectKind::Filesystem]),
            vec![DeniedEffectProbe {
                identity: "filesystem-write".to_owned(),
                effect: DeniedEffectKind::Filesystem,
                opt_in: true,
            }],
            vec![ObservedDeniedEffect {
                identity: "filesystem-write".to_owned(),
                blocked: false,
            }],
        );
        assert!(!failed.valid);
        assert!(failed.issue_codes.contains(&ISSUE_PROBE_NOT_BLOCKED.to_owned()));
        assert!(failed.non_claims.contains(&CAPABILITY_ENFORCEMENT_NON_CLAIM.to_owned()));
    }

    // r[verify valence.evidence_ir.test_suite_partition]
    // r[verify valence.evidence_ir.test_suite_partition.fast]
    // r[verify valence.evidence_ir.test_suite_partition.deep]
    // r[verify valence.evidence_ir.test_suite_partition.reporting]
    // r[verify valence.evidence_ir.test_suite_partition.ci]
    // r[verify valence.evidence_ir.test_suite_partition.non_claims]
    #[test]
    fn suite_partition_defines_stable_fast_deep_and_nightly_lanes() {
        let lanes = default_test_suite_lanes();
        assert_eq!(lanes.len(), EXPECTED_THREE);
        assert!(lanes[0].hermetic);
        assert!(!lanes[0].requires_sibling_repositories);
        assert!(!lanes[0].requires_host_sandbox);
        assert!(lanes[1].includes_optional_integrations);
        assert!(lanes[2].requires_sibling_repositories);
        assert_eq!(lanes[0].command, FAST_LANE_COMMAND);

        let summary = summarize_suite_lane_results(vec![
            SuiteCheckResult {
                lane: TestSuiteLaneKind::Deep,
                name: "sandbox".to_owned(),
                status: HarnessStatus::Unavailable,
            },
            SuiteCheckResult {
                lane: TestSuiteLaneKind::Fast,
                name: "transcripts".to_owned(),
                status: HarnessStatus::Pass,
            },
        ]);
        assert!(summary.valid);
        assert_eq!(summary.results[0].lane, TestSuiteLaneKind::Fast);
        assert!(summary.non_claims.contains(&TEST_SUITE_PARTITION_NON_CLAIM.to_owned()));

        let failed = summarize_suite_lane_results(vec![SuiteCheckResult {
            lane: TestSuiteLaneKind::Fast,
            name: "corpus".to_owned(),
            status: HarnessStatus::Fail,
        }]);
        assert!(!failed.valid);
        assert!(failed.issue_codes.contains(&ISSUE_SUITE_CHECK_FAILED.to_owned()));
    }

    // r[verify valence.stack_provenance_adapter.cross_repo_fixtures]
    // r[verify valence.stack_provenance_adapter.cross_repo_fixtures.fast]
    // r[verify valence.stack_provenance_adapter.cross_repo_fixtures.positive]
    // r[verify valence.stack_provenance_adapter.cross_repo_fixtures.non_claims]
    #[test]
    fn stack_cross_repo_fixture_stays_hermetic_and_identity_bounded() {
        let fixture: StackCrossRepoFixture =
            serde_json::from_str(include_str!("../testdata/stack-provenance/cross-repo-fixture.json"))
                .expect("parse cross-repo fixture");
        let export_json = serde_json::to_string(&fixture.octet_export).expect("serialize export fixture");
        let binary_hash = blake3::hash(fixture.binary_bytes.as_bytes()).to_hex().to_string();
        assert_eq!(fixture.binary_hash, binary_hash);

        let sidecar = build_stack_provenance_sidecar(&fixture.octet_export, &fixture.binary_hash);
        let sidecar_json = serde_json::to_vec_pretty(&sidecar).expect("serialize sidecar");
        let sidecar_digest = blake3::hash(&sidecar_json).to_hex().to_string();
        let manifest = mantle_release_manifest(&sidecar_digest, &fixture.binary_hash);
        let report = verify_built_binary_thread(BuiltBinaryThreadInput {
            binary_bytes: fixture.binary_bytes.as_bytes(),
            octet_export: &fixture.octet_export,
            sidecar: &sidecar,
            sidecar_bytes: &sidecar_json,
            mantle_manifest: &manifest,
        });

        assert!(report.valid, "unexpected issues: {:?}; export {export_json}", report.issue_codes);
        assert_eq!(report.source_ids, fixture.expected_source_ids);
        assert_eq!(report.requirement_ids, fixture.expected_requirement_ids);
        assert!(report.non_claims.contains(&BUILT_BINARY_THREAD_NON_CLAIM.to_owned()));
        assert!(fixture.non_claims.contains(&STACK_PROVENANCE_NON_CLAIM.to_owned()));
    }

    const EXPECTED_FOUR: usize = 4;

    #[derive(Debug, Deserialize)]
    struct StackCrossRepoFixture {
        binary_bytes: String,
        binary_hash: String,
        octet_export: OctetValenceProvenanceExport,
        expected_source_ids: Vec<String>,
        expected_requirement_ids: Vec<String>,
        non_claims: Vec<String>,
    }

    fn evidence_ir_transcript() -> ReceiptTranscript {
        let artifact = build_evidence_artifact(
            "test",
            "id",
            None,
            VerificationRole::RecordedOnly,
            serde_json::json!({"claim": "identity only"}),
            vec![REQUIRED_NON_CLAIM.to_owned()],
        )
        .expect("artifact builds");
        let artifact_json = serde_json::to_string(&artifact).expect("artifact serializes");
        let mut expected_hashes = BTreeMap::new();
        expected_hashes.insert("artifact_json".to_owned(), hash_bytes(artifact_json.as_bytes()));
        ReceiptTranscript::new(
            "valid-artifact",
            serde_json::json!({"artifact_json": artifact_json}),
            TranscriptVerifierAction::EvidenceIrCheck,
            TranscriptExpectedReport {
                valid: true,
                issue_codes: Vec::new(),
            },
            expected_hashes,
        )
    }

    fn stack_provenance_transcript() -> ReceiptTranscript {
        let binary_bytes = "valence-built-binary-fixture";
        let binary_hash = blake3::hash(binary_bytes.as_bytes()).to_hex().to_string();
        let export = sample_export();
        let sidecar = build_stack_provenance_sidecar(&export, &binary_hash);
        let sidecar_json = serde_json::to_string_pretty(&sidecar).unwrap();
        let sidecar_digest = blake3::hash(sidecar_json.as_bytes()).to_hex().to_string();
        let manifest = mantle_release_manifest(&sidecar_digest, &binary_hash);
        let manifest_json = serde_json::to_string(&manifest).unwrap();
        let mut expected_hashes = BTreeMap::new();
        expected_hashes.insert("binary_hash".to_owned(), binary_hash.clone());
        expected_hashes.insert("sidecar_digest_blake3".to_owned(), sidecar_digest);
        ReceiptTranscript::new(
            "stack-provenance-built-binary",
            serde_json::json!({
                "binary_bytes": binary_bytes,
                "octet_export_json": serde_json::to_string(&export).unwrap(),
                "sidecar_json": sidecar_json,
                "mantle_manifest_json": manifest_json
            }),
            TranscriptVerifierAction::StackProvenanceBuiltBinaryCheck,
            TranscriptExpectedReport {
                valid: true,
                issue_codes: Vec::new(),
            },
            expected_hashes,
        )
    }

    fn content_definition(
        name: &str,
        path: &str,
        dependency_identity: &str,
    ) -> crate::evidence_model::EvidenceDefinition {
        build_evidence_definition(
            "function-object",
            serde_json::json!({"operation": "check", "predicate_hash": "b3:predicate"}),
            vec![EvidenceDependency::new("definition", dependency_identity)],
            EvidenceDefinitionMetadata::new(Some(name.to_owned()), Some(path.to_owned()), vec![name.to_owned()]),
            vec![CONTENT_IDENTITY_NON_CLAIM.to_owned()],
        )
        .expect("definition builds")
    }

    fn graph() -> EvidenceGraph {
        EvidenceGraph::new(
            vec![
                EvidenceGraphNode::new(IDENTITY_ALPHA, EvidenceGraphNodeKind::Definition, Some("alpha".to_owned())),
                EvidenceGraphNode::new(IDENTITY_BETA, EvidenceGraphNodeKind::Artifact, Some("beta".to_owned())),
                EvidenceGraphNode::new(REQUIREMENT_IDENTITY, EvidenceGraphNodeKind::Requirement, None),
            ],
            vec![
                EvidenceGraphEdge::new(IDENTITY_ALPHA, IDENTITY_BETA, EvidenceGraphEdgeKind::DependsOn),
                EvidenceGraphEdge::new(IDENTITY_ALPHA, REQUIREMENT_IDENTITY, EvidenceGraphEdgeKind::CoversRequirement),
            ],
            NameBindingHistory::new(vec![binding("alpha", IDENTITY_ALPHA, INITIAL_SEQUENCE, true)]),
        )
    }

    fn broken_graph() -> EvidenceGraph {
        let mut graph = graph();
        graph.nodes.push(EvidenceGraphNode::new(IDENTITY_ALPHA, EvidenceGraphNodeKind::Definition, None));
        graph
            .edges
            .push(EvidenceGraphEdge::new(IDENTITY_ALPHA, "b3:missing", EvidenceGraphEdgeKind::DependsOn));
        graph
    }

    fn binding(name: &str, identity: &str, sequence: u64, current: bool) -> EvidenceNameBinding {
        EvidenceNameBinding::new(NameBindingKind::Alias, name, identity, sequence, current)
    }

    fn durable_mismatch_input() -> serde_json::Value {
        let mut envelope = encode_durable_value(
            TYPE_IDENTITY,
            DurableStorageCodec::CanonicalJson,
            serde_json::json!({"identity": IDENTITY_ALPHA}),
            Vec::new(),
        )
        .expect("durable envelope encodes");
        envelope.content_hash = IDENTITY_BETA.to_owned();
        serde_json::json!({
            "envelope": envelope,
            "expected_value_type": "other.type",
            "expected_codec": "canonical_json",
            "migration_required": false
        })
    }

    fn all_denied_effect_probes(opt_in: bool) -> Vec<DeniedEffectProbe> {
        vec![
            DeniedEffectProbe {
                identity: "filesystem".to_owned(),
                effect: DeniedEffectKind::Filesystem,
                opt_in,
            },
            DeniedEffectProbe {
                identity: "process".to_owned(),
                effect: DeniedEffectKind::Process,
                opt_in,
            },
            DeniedEffectProbe {
                identity: "network".to_owned(),
                effect: DeniedEffectKind::Network,
                opt_in,
            },
            DeniedEffectProbe {
                identity: "clock".to_owned(),
                effect: DeniedEffectKind::Clock,
                opt_in,
            },
            DeniedEffectProbe {
                identity: "remote-builder".to_owned(),
                effect: DeniedEffectKind::RemoteBuilder,
                opt_in,
            },
            DeniedEffectProbe {
                identity: "secret".to_owned(),
                effect: DeniedEffectKind::Secret,
                opt_in,
            },
        ]
    }

    fn sample_export() -> OctetValenceProvenanceExport {
        OctetValenceProvenanceExport {
            schema_version: OCTET_VALENCE_EXPORT_SCHEMA.to_owned(),
            artifact_dir: "target/octet-artifact".to_owned(),
            verification_status: "valid".to_owned(),
            artifacts: vec![
                StackArtifactIdentity {
                    artifact: "status.json".to_owned(),
                    blake3: STATUS_HASH.to_owned(),
                },
                StackArtifactIdentity {
                    artifact: "results.jsonl".to_owned(),
                    blake3: RESULTS_HASH.to_owned(),
                },
            ],
            provenance_records: vec![OctetProvenanceRecord {
                source_id: "octet.standards.formal-proof".to_owned(),
                checker_id: "octet-artifact-verifier".to_owned(),
                evidence_tier: "formal_proof".to_owned(),
                artifact: "status.json".to_owned(),
                blake3: STATUS_HASH.to_owned(),
                checked_scope: "standards/proof-claims/catalog.ncl".to_owned(),
                verifier_identity: Some("octet-verus-rail".to_owned()),
                proof_artifact_identity: Some(RESULTS_HASH.to_owned()),
                human_action: None,
                parent_artifacts: vec!["results.jsonl".to_owned()],
            }],
            proof_cards: vec![OctetProofCard {
                proof_or_property_id: "octet.no-unwrap-proof".to_owned(),
                source_requirement_or_policy_id: "r[octet.policy.no_unwrap]".to_owned(),
                policy_id: "octet.standards.formal-proof".to_owned(),
                rail_index: 0,
                receipt_path: "status.json".to_owned(),
                artifact_blake3_identities: vec![StackArtifactIdentity {
                    artifact: "status.json".to_owned(),
                    blake3: STATUS_HASH.to_owned(),
                }],
                evidence_tier: "formal_proof".to_owned(),
                evidence_origin: "octet_verified_receipt".to_owned(),
                status: "valid".to_owned(),
                non_claims: vec![STACK_PROVENANCE_NON_CLAIM.to_owned()],
                diagnostics: Vec::new(),
            }],
            diagnostics: Vec::new(),
            non_claims: vec![STACK_PROVENANCE_NON_CLAIM.to_owned()],
        }
    }

    fn mantle_release_manifest(sidecar_digest: &str, binary_hash: &str) -> MantleReleaseManifestView {
        serde_json::from_value(serde_json::json!({
            "binaries": [
                {
                    "relative_path": "bin/valence-fixture",
                    "digest_blake3": binary_hash
                }
            ],
            "external_evidence": [
                {
                    "role": MANTLE_STACK_PROVENANCE_ROLE,
                    "schema": STACK_PROVENANCE_SIDECAR_SCHEMA,
                    "relative_path": "external-evidence/stack-provenance.json",
                    "digest_blake3": sidecar_digest,
                    "claim_scope": STACK_PROVENANCE_CLAIM_SCOPE,
                    "non_claims": [STACK_PROVENANCE_NON_CLAIM]
                }
            ],
            "provenance_coverage": {
                "binary_hash": binary_hash
            }
        }))
        .unwrap()
    }

    #[test]
    fn hash_helpers_are_stable_for_test_values() {
        let hash = hash_bytes(serde_json::json!({"a": true}).to_string().as_bytes());
        assert!(hash.starts_with(BLAKE3_IDENTITY_PREFIX));
        assert!(CAPABILITY_NON_CLAIM.contains("capability labels"));
        assert!(DURABLE_STORAGE_NON_CLAIM.contains("decode identity"));
        assert_eq!(
            evidence_definition_identity(&content_definition("alpha", "src/lib.rs", IDENTITY_BETA).canonical).unwrap(),
            content_definition("alpha", "src/lib.rs", IDENTITY_BETA).content_identity
        );
    }
}
