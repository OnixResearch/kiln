// r[impl semantic_review.split_semantic_review_core.focused_modules]
// r[impl semantic_review.split_semantic_review_core.focused_modules.visible_boundaries]
// r[impl semantic_review.split_semantic_review_core.focused_modules.narrow_bridges]
mod analytics;
mod common;
mod completeness;
mod metrics_regression;
mod profile;
mod review_receipt;
mod revision_handoff;

pub use analytics::review_ledger_analytics;
pub use common::issues_json;
pub use completeness::completeness_receipt;
pub use metrics_regression::{
    validate_review_metrics_evidence_boundary, validate_review_metrics_fixture_pairs,
    validate_review_metrics_guidance_sync, validate_review_metrics_registry,
    REVIEW_METRICS_EVIDENCE_BOUNDARY_SCHEMA_V1, REVIEW_METRICS_FIXTURE_COVERAGE_SCHEMA_V1,
    REVIEW_METRICS_GUIDANCE_SYNC_SCHEMA_V1, REVIEW_METRICS_REGRESSION_SCHEMA_V1,
};
pub use profile::semantic_review_policy_profile_json;
pub use review_receipt::{review_receipt_hash, validate_review_receipt};
pub use revision_handoff::{revision_handoff_hash, validate_revision_handoff};

pub const REVIEW_RECEIPT_SCHEMA_V1: &str = "cairn.semantic-review.review-receipt.v1";
pub const REVISION_HANDOFF_SCHEMA_V1: &str = "cairn.semantic-review.revision-handoff.v1";
pub const REVIEW_ANALYTICS_SCHEMA_V1: &str = "cairn.semantic-review.ledger-analytics.v1";
pub const REVIEW_COMPLETENESS_SCHEMA_V1: &str = "cairn.semantic-review.completeness.v1";

pub const REVIEW_KINDS: &[&str] = &["plan", "design", "code", "diff"];
pub const REVIEW_DECISIONS: &[&str] = &["approved", "needs_revision", "escalate", "unknown"];
pub const REVIEW_CATEGORIES: &[&str] = &["omission", "systematic", "incoherent", "infra"];
pub const REVIEW_SEVERITIES: &[&str] = &["low", "medium", "high", "critical"];
pub const REVIEW_SOURCES: &[&str] = &["manual", "agentic", "static", "imported"];
pub const REVISION_DISPOSITIONS: &[&str] =
    &["addressed", "unchanged", "superseded", "deferred", "new"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticReviewIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticReviewReport {
    pub valid: bool,
    pub verdict: String,
    pub issues: Vec<SemanticReviewIssue>,
    pub receipt_hash: String,
}

#[cfg(test)]
mod tests;
