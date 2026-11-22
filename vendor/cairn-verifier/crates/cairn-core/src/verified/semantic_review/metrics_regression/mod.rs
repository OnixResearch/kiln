// r[impl cairn.semantic_review.review_metrics_regression_rail]
// r[impl cairn.semantic_review.metrics_regression_split]
// r[impl cairn.semantic_review.metrics_regression_split.boundaries]
// r[impl cairn.semantic_review.metrics_regression_split.public_api]
mod common;
mod evidence_boundary;
mod fixtures;
mod guidance;
mod registry;
mod report;

pub const REVIEW_METRICS_REGRESSION_SCHEMA_V1: &str =
    "cairn.semantic-review.metrics-regression-registry.v1";
pub const REVIEW_METRICS_FIXTURE_COVERAGE_SCHEMA_V1: &str =
    "cairn.semantic-review.metrics-regression-fixtures.v1";
pub const REVIEW_METRICS_GUIDANCE_SYNC_SCHEMA_V1: &str =
    "cairn.semantic-review.metrics-regression-guidance-sync.v1";
pub const REVIEW_METRICS_EVIDENCE_BOUNDARY_SCHEMA_V1: &str =
    "cairn.semantic-review.metrics-regression-evidence-boundary.v1";

pub fn validate_review_metrics_registry(value: &serde_json::Value) -> super::SemanticReviewReport {
    // r[impl cairn.semantic_review.review_metrics_regression_rail.registry_binds_surfaces]
    let mut issues = Vec::new();
    super::common::require_string(
        value,
        "schema_version",
        REVIEW_METRICS_REGRESSION_SCHEMA_V1,
        &mut issues,
    );
    registry::validate_categories(value, &mut issues);
    report::build_report(REVIEW_METRICS_REGRESSION_SCHEMA_V1, issues)
}

pub fn validate_review_metrics_fixture_pairs(
    registry: &serde_json::Value,
    fixture_coverage: &serde_json::Value,
) -> super::SemanticReviewReport {
    // r[impl cairn.semantic_review.review_metrics_regression_rail.fixture_pair_enforcement]
    let mut issues = validate_review_metrics_registry(registry).issues;
    super::common::require_string(
        fixture_coverage,
        "schema_version",
        REVIEW_METRICS_FIXTURE_COVERAGE_SCHEMA_V1,
        &mut issues,
    );
    let fixture_results = fixtures::fixture_result_map(fixture_coverage, &mut issues);
    fixtures::validate_fixture_pairs(registry, &fixture_results, &mut issues);
    report::build_report(REVIEW_METRICS_FIXTURE_COVERAGE_SCHEMA_V1, issues)
}

pub fn validate_review_metrics_guidance_sync(
    registry: &serde_json::Value,
    sync_surfaces: &serde_json::Value,
) -> super::SemanticReviewReport {
    // r[impl cairn.semantic_review.review_metrics_regression_rail.guidance_task_sync]
    let mut issues = validate_review_metrics_registry(registry).issues;
    super::common::require_string(
        sync_surfaces,
        "schema_version",
        REVIEW_METRICS_GUIDANCE_SYNC_SCHEMA_V1,
        &mut issues,
    );
    let sync = guidance::guidance_sync_surfaces(sync_surfaces, &mut issues);
    guidance::validate_guidance_sync(registry, &sync, &mut issues);
    report::build_report(REVIEW_METRICS_GUIDANCE_SYNC_SCHEMA_V1, issues)
}

pub fn validate_review_metrics_evidence_boundary(
    registry: &serde_json::Value,
    evidence: &serde_json::Value,
) -> super::SemanticReviewReport {
    // r[impl cairn.semantic_review.review_metrics_regression_rail.secret_free_evidence]
    let mut issues = validate_review_metrics_registry(registry).issues;
    super::common::require_string(
        evidence,
        "schema_version",
        REVIEW_METRICS_EVIDENCE_BOUNDARY_SCHEMA_V1,
        &mut issues,
    );
    let evidence_refs = evidence_boundary::evidence_ref_map(evidence, &mut issues);
    evidence_boundary::validate_category_evidence_refs(registry, &evidence_refs, &mut issues);
    evidence_boundary::validate_secret_free_json(evidence, "evidence", &mut issues);
    report::build_report(REVIEW_METRICS_EVIDENCE_BOUNDARY_SCHEMA_V1, issues)
}
