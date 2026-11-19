//! Pure license-consumption contract for intentional Basalt adoption.
//!
//! Basalt is AGPL-licensed while some downstream consumers use other project
//! licenses. This module makes downstream adoption explicit and evidence-backed.
//! It validates a license-consumption receipt without granting legal advice, an
//! alternate license, or a compatibility exception.

use serde::{Deserialize, Serialize};

pub use crate::license_consumption_identity::{
    LICENSE_CONSUMPTION_FIXTURE_SCHEMA, LICENSE_CONSUMPTION_RECEIPT_SCHEMA,
    LICENSE_CONSUMPTION_RELEASE_RECEIPT_SCHEMA,
};

const MAXIMUM_FIELD_BYTES: usize = 4_096;
const MAXIMUM_STABLE_ID_BYTES: usize = 128;
const MAXIMUM_NON_CLAIM_COUNT: usize = 8;
const REQUIRED_LICENSE_EXPRESSION: &str = "AGPL-3.0-or-later";
const REQUIRED_LICENSE_METADATA_REVISION: &str = "2026-07-31-license-metadata-v1";
const MAXIMUM_REVISION_BYTES: usize = 128;
const SUPPORTED_CLAIM: &str = "intentional-license-review";
const NON_CLAIM_MESSAGE: &str = "this receipt records review metadata only. It is not legal advice, a license exception, or a compatibility promise";

/// Claim boundaries carried by every accepted license-consumption receipt.
pub const REQUIRED_LICENSE_CONSUMPTION_NON_CLAIMS: [&str; 4] = [
    "legal-advice",
    "license-exception",
    "compatibility-promise",
    "alternate-license-grant",
];

/// Reviewer action recorded against the Basalt license boundary.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LicenseReviewAction {
    /// The consumer reviewer acknowledged the AGPL boundary.
    Acknowledged,
    /// The consumer reviewer deferred explicit acknowledgment.
    Deferred,
}

/// One recorded downstream license-consumption fact.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LicenseConsumptionReceipt {
    /// Schema identifier.
    pub schema: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Downstream consumer repository or project identity.
    pub consumer_repo: String,
    /// Basalt artifact identity consumed by the consumer.
    pub artifact_identity: String,
    /// License expression that applies to the consumed artifact.
    pub license_expression: String,
    /// Reviewed license-metadata revision the consumer consumed.
    pub license_metadata_revision: String,
    /// Reviewer action against the license boundary.
    pub review_action: LicenseReviewAction,
    /// Non-empty non-claim boundary.
    pub non_claims: Vec<String>,
}

/// Stable validation outcome for one license-consumption receipt.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LicenseConsumptionVerdict {
    /// Whether the receipt passed as a reviewable adoption record.
    pub accepted: bool,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Supported claim when accepted.
    pub supported_claim: Option<String>,
    /// Deterministic issue codes and messages.
    pub issues: Vec<LicenseConsumptionIssue>,
}

/// One deterministic license-consumption validation finding.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct LicenseConsumptionIssue {
    /// Stable issue code.
    pub code: String,
    /// Human-readable finding.
    pub message: String,
}

impl LicenseConsumptionIssue {
    /// Construct one issue.
    #[must_use]
    pub const fn new(code: String, message: String) -> Self {
        Self { code, message }
    }
}

/// Validate the shape and license boundary of one license-consumption receipt.
///
/// This is a pure deterministic decision. It performs no I/O and supplies no
/// legal advice or license exception.
#[must_use]
pub fn validate_license_consumption(
    receipt: &LicenseConsumptionReceipt,
) -> LicenseConsumptionVerdict {
    let mut issues: Vec<LicenseConsumptionIssue> = Vec::new();

    check_exact(
        &mut issues,
        receipt.schema.as_str(),
        LICENSE_CONSUMPTION_RECEIPT_SCHEMA,
        "license-consumption.schema",
        "unsupported license-consumption receipt schema",
    );
    check_stable_id(
        &mut issues,
        receipt.receipt_id.as_str(),
        "license-consumption.receipt-id",
        "receipt id must be non-empty and within the bounded byte length",
    );
    check_bounded_non_empty(
        &mut issues,
        receipt.consumer_repo.as_str(),
        "license-consumption.consumer",
        "consumer repository identity must be non-empty and bounded",
    );
    check_exact(
        &mut issues,
        receipt.artifact_identity.as_str(),
        "basalt",
        "license-consumption.artifact-identity",
        "recorded Basalt artifact identity is not the canonical `basalt` package",
    );
    check_exact(
        &mut issues,
        receipt.license_expression.as_str(),
        REQUIRED_LICENSE_EXPRESSION,
        "license-consumption.license-expression",
        "recorded license expression does not match the Basalt AGPL boundary",
    );
    check_revision(
        &mut issues,
        receipt.license_metadata_revision.as_str(),
        "license-consumption.license-metadata",
        "recorded license metadata revision is stale or unbounded",
    );
    check_review_action(&mut issues, receipt.review_action);
    check_non_claims(&mut issues, receipt.non_claims.as_slice());

    let accepted = issues.is_empty();
    LicenseConsumptionVerdict {
        accepted,
        receipt_id: receipt.receipt_id.clone(),
        supported_claim: accepted.then_some(SUPPORTED_CLAIM.to_owned()),
        issues,
    }
}

/// Return the non-claim statement rendered for one receipt.
#[must_use]
pub fn license_non_claim_statement() -> String {
    NON_CLAIM_MESSAGE.to_owned()
}

fn check_exact(
    issues: &mut Vec<LicenseConsumptionIssue>,
    actual: &str,
    expected: &str,
    code: &str,
    message: &str,
) {
    if actual != expected {
        issues.push(LicenseConsumptionIssue::new(
            code.to_owned(),
            message.to_owned(),
        ));
    }
}

fn check_stable_id(
    issues: &mut Vec<LicenseConsumptionIssue>,
    value: &str,
    code: &str,
    message: &str,
) {
    if value.is_empty() || value.len() > MAXIMUM_STABLE_ID_BYTES {
        issues.push(LicenseConsumptionIssue::new(
            code.to_owned(),
            message.to_owned(),
        ));
    }
}

fn check_bounded_non_empty(
    issues: &mut Vec<LicenseConsumptionIssue>,
    value: &str,
    code: &str,
    message: &str,
) {
    if value.is_empty() || value.len() > MAXIMUM_FIELD_BYTES {
        issues.push(LicenseConsumptionIssue::new(
            code.to_owned(),
            message.to_owned(),
        ));
    }
}

fn check_revision(
    issues: &mut Vec<LicenseConsumptionIssue>,
    value: &str,
    code: &str,
    message: &str,
) {
    if value.is_empty()
        || value.len() > MAXIMUM_REVISION_BYTES
        || value != REQUIRED_LICENSE_METADATA_REVISION
    {
        issues.push(LicenseConsumptionIssue::new(
            code.to_owned(),
            message.to_owned(),
        ));
    }
}

fn check_review_action(
    issues: &mut Vec<LicenseConsumptionIssue>,
    review_action: LicenseReviewAction,
) {
    if review_action != LicenseReviewAction::Acknowledged {
        issues.push(LicenseConsumptionIssue::new(
            "license-consumption.review".to_owned(),
            "downstream reviewer must acknowledge the Basalt AGPL license boundary".to_owned(),
        ));
    }
}

fn check_non_claims(issues: &mut Vec<LicenseConsumptionIssue>, non_claims: &[String]) {
    if non_claims.len() > MAXIMUM_NON_CLAIM_COUNT {
        issues.push(LicenseConsumptionIssue::new(
            "license-consumption.non-claims.limit".to_owned(),
            "receipt non-claim count exceeds the bounded maximum".to_owned(),
        ));
    }
    let expected = REQUIRED_LICENSE_CONSUMPTION_NON_CLAIMS.map(str::to_owned);
    if non_claims != expected {
        issues.push(LicenseConsumptionIssue::new(
            "license-consumption.non-claims".to_owned(),
            "receipt must preserve the complete ordered non-claim boundary".to_owned(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LicenseConsumptionReceipt, LicenseReviewAction, REQUIRED_LICENSE_CONSUMPTION_NON_CLAIMS,
        validate_license_consumption,
    };

    fn receipt() -> LicenseConsumptionReceipt {
        LicenseConsumptionReceipt {
            schema: super::LICENSE_CONSUMPTION_RECEIPT_SCHEMA.to_owned(),
            receipt_id: "moltens-aspen-adoption-001".to_owned(),
            consumer_repo: "github.com/onixresearch/aspen".to_owned(),
            artifact_identity: "basalt".to_owned(),
            license_expression: "AGPL-3.0-or-later".to_owned(),
            license_metadata_revision: super::REQUIRED_LICENSE_METADATA_REVISION.to_owned(),
            review_action: LicenseReviewAction::Acknowledged,
            non_claims: REQUIRED_LICENSE_CONSUMPTION_NON_CLAIMS
                .map(str::to_owned)
                .to_vec(),
        }
    }

    #[test]
    fn acknowledged_consumption_passes() {
        let verdict = validate_license_consumption(&receipt());
        assert!(verdict.accepted);
        assert_eq!(
            verdict.supported_claim.as_deref(),
            Some("intentional-license-review")
        );
        assert!(verdict.issues.is_empty());
    }

    #[test]
    fn missing_review_fails_closed() {
        let mut case = receipt();
        case.review_action = LicenseReviewAction::Deferred;
        let verdict = validate_license_consumption(&case);
        assert!(!verdict.accepted);
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.code == "license-consumption.review")
        );
    }

    #[test]
    fn wrong_artifact_identity_fails_closed() {
        let mut case = receipt();
        case.artifact_identity = "ucan-nickel-contracts".to_owned();
        let verdict = validate_license_consumption(&case);
        assert!(!verdict.accepted);
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.code == "license-consumption.artifact-identity")
        );
    }

    #[test]
    fn wrong_license_expression_fails_closed() {
        let mut case = receipt();
        case.license_expression = "MIT".to_owned();
        let verdict = validate_license_consumption(&case);
        assert!(!verdict.accepted);
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.code == "license-consumption.license-expression")
        );
    }

    #[test]
    fn stale_license_metadata_fails_closed() {
        let mut case = receipt();
        case.license_metadata_revision = "2024-01-01-license-metadata-v1".to_owned();
        let verdict = validate_license_consumption(&case);
        assert!(!verdict.accepted);
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.code == "license-consumption.license-metadata")
        );
    }

    #[test]
    fn weakened_non_claims_fail_closed() {
        let mut case = receipt();
        case.non_claims.pop();
        let verdict = validate_license_consumption(&case);
        assert!(!verdict.accepted);
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.code == "license-consumption.non-claims")
        );
    }

    #[test]
    fn non_claim_statement_is_not_a_license_exception() {
        let statement = super::license_non_claim_statement();
        assert!(statement.contains("not legal advice"));
        assert!(statement.contains("license exception"));
    }
}
