//! Verification role classification and summary for evidence artifacts.
//!
//! This module implements the c-earchin `recorded_only` discipline: every
//! evidence artifact carries a `verification_role` that says whether it
//! verifies a checkable predicate (`property`), records structure/identity
//! only (`recorded_only`), or is a non-claim boundary artifact (`boundary`).
//!
//! The key invariant: `verified_count` counts only `property` artifacts. A
//! "0 verified" receipt means "nothing was verified," not "all good." This
//! prevents the vacuous-pass misread that c-earchin's `recorded_only` role
//! was designed to eliminate.
//!
//! Phase 0 (this change): the schema and classification land. Phase 2 will
//! bind the role to a provenance-validated receipt so `property` vs
//! `recorded_only` is meaningful end-to-end.

/// The stable role strings recognized by the classifier.
pub const ROLE_PROPERTY: &str = "property";
pub const ROLE_RECORDED_ONLY: &str = "recorded_only";
pub const ROLE_BOUNDARY: &str = "boundary";

/// The required non-claim boundary text for the verification role summary.
pub const VERIFICATION_ROLE_BOUNDARY: &str = "verification_role_summary counts only property artifacts as verified; recorded_only and boundary artifacts prove nothing";

/// A classified verification role.
// r[impl cairn.verification_role]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationRole {
    Property,
    RecordedOnly,
    Boundary,
    Unknown,
}

impl VerificationRole {
    /// Classify a role string into a typed variant. Unknown strings map to
    /// [`VerificationRole::Unknown`].
    #[must_use]
    pub fn classify(role: &str) -> Self {
        match role {
            ROLE_PROPERTY => Self::Property,
            ROLE_RECORDED_ONLY => Self::RecordedOnly,
            ROLE_BOUNDARY => Self::Boundary,
            _ => Self::Unknown,
        }
    }

    /// The stable string representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Property => ROLE_PROPERTY,
            Self::RecordedOnly => ROLE_RECORDED_ONLY,
            Self::Boundary => ROLE_BOUNDARY,
            Self::Unknown => "unknown",
        }
    }

    /// Whether this role counts as "verified" (only `Property` does).
    #[must_use]
    pub fn is_verified(self) -> bool {
        self == Self::Property
    }
}

/// A summary of verification roles across a set of artifacts.
///
/// `verified_count` is always `property_count`. This is the field consumers
/// should check; a `verified_count` of 0 means "nothing was verified."
// r[impl cairn.verification_role.verified_count]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRoleSummary {
    pub property_count: usize,
    pub recorded_only_count: usize,
    pub boundary_count: usize,
    pub unknown_count: usize,
    pub total: usize,
    pub verified_count: usize,
}

impl VerificationRoleSummary {
    /// Summarize a slice of role strings.
    #[must_use]
    pub fn from_roles(roles: &[&str]) -> Self {
        let mut property_count = 0;
        let mut recorded_only_count = 0;
        let mut boundary_count = 0;
        let mut unknown_count = 0;
        for role in roles {
            match VerificationRole::classify(role) {
                VerificationRole::Property => property_count += 1,
                VerificationRole::RecordedOnly => recorded_only_count += 1,
                VerificationRole::Boundary => boundary_count += 1,
                VerificationRole::Unknown => unknown_count += 1,
            }
        }
        let total = property_count + recorded_only_count + boundary_count + unknown_count;
        Self {
            property_count,
            recorded_only_count,
            boundary_count,
            unknown_count,
            total,
            verified_count: property_count,
        }
    }

    /// Summarize verification roles from an array of JSON artifact objects,
    /// reading the `verification_role` field from each. Artifacts without the
    /// field are counted as `unknown`.
    #[must_use]
    pub fn from_artifacts(artifacts: &[serde_json::Value]) -> Self {
        let roles: Vec<&str> = artifacts
            .iter()
            .map(|artifact| {
                artifact
                    .get("verification_role")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
            })
            .collect();
        Self::from_roles(&roles)
    }

    /// Render the summary as a JSON object with the boundary non-claim.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "property_count": self.property_count,
            "recorded_only_count": self.recorded_only_count,
            "boundary_count": self.boundary_count,
            "unknown_count": self.unknown_count,
            "total": self.total,
            "verified_count": self.verified_count,
            "boundary": VERIFICATION_ROLE_BOUNDARY,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    // r[verify cairn.verification_role.classify_known]
    fn classifies_known_roles() {
        assert_eq!(
            VerificationRole::classify("property"),
            VerificationRole::Property
        );
        assert_eq!(
            VerificationRole::classify("recorded_only"),
            VerificationRole::RecordedOnly
        );
        assert_eq!(
            VerificationRole::classify("boundary"),
            VerificationRole::Boundary
        );
    }

    #[test]
    // r[verify cairn.verification_role.classify_unknown]
    fn classifies_unknown_role_as_unknown() {
        assert_eq!(
            VerificationRole::classify("verified"),
            VerificationRole::Unknown
        );
        assert_eq!(VerificationRole::classify(""), VerificationRole::Unknown);
        assert_eq!(
            VerificationRole::classify("PROPERTY"),
            VerificationRole::Unknown
        );
    }

    #[test]
    // r[verify cairn.verification_role.verified_count.property_only]
    fn only_property_is_verified() {
        assert!(VerificationRole::Property.is_verified());
        assert!(!VerificationRole::RecordedOnly.is_verified());
        assert!(!VerificationRole::Boundary.is_verified());
        assert!(!VerificationRole::Unknown.is_verified());
    }

    #[test]
    fn summary_counts_roles_correctly() {
        let summary = VerificationRoleSummary::from_roles(&[
            "property",
            "property",
            "recorded_only",
            "boundary",
            "unknown_role",
        ]);
        assert_eq!(summary.property_count, 2);
        assert_eq!(summary.recorded_only_count, 1);
        assert_eq!(summary.boundary_count, 1);
        assert_eq!(summary.unknown_count, 1);
        assert_eq!(summary.total, 5);
        assert_eq!(summary.verified_count, 2);
    }

    #[test]
    // r[verify cairn.verification_role.verified_count.zero]
    fn summary_zero_verified_means_nothing_was_verified() {
        let summary =
            VerificationRoleSummary::from_roles(&["recorded_only", "recorded_only", "boundary"]);
        assert_eq!(summary.verified_count, 0);
        assert_eq!(summary.total, 3);
        // The key invariant: 0 verified is not "all good."
        assert!(!summary.verified_count > 0);
    }

    #[test]
    fn summary_empty_roles_is_all_zero() {
        let summary = VerificationRoleSummary::from_roles(&[]);
        assert_eq!(summary.total, 0);
        assert_eq!(summary.verified_count, 0);
    }

    #[test]
    fn summary_from_artifacts_reads_verification_role_field() {
        let artifacts = vec![
            json!({"role": "release_readiness", "verification_role": "property"}),
            json!({"role": "agent_receipt", "verification_role": "recorded_only"}),
            json!({"role": "replay", "verification_role": "boundary"}),
            json!({"role": "coverage_audit"}), // no verification_role field
        ];
        let summary = VerificationRoleSummary::from_artifacts(&artifacts);
        assert_eq!(summary.property_count, 1);
        assert_eq!(summary.recorded_only_count, 1);
        assert_eq!(summary.boundary_count, 1);
        assert_eq!(summary.unknown_count, 1);
        assert_eq!(summary.total, 4);
        assert_eq!(summary.verified_count, 1);
    }

    #[test]
    fn summary_json_includes_boundary_non_claim() {
        let summary = VerificationRoleSummary::from_roles(&["property"]);
        let json = summary.to_json();
        assert_eq!(json["verified_count"], 1);
        assert!(json["boundary"]
            .as_str()
            .expect("boundary text")
            .contains("counts only property artifacts as verified"));
    }

    #[test]
    fn summary_json_has_stable_fields() {
        let summary = VerificationRoleSummary::from_roles(&["property", "recorded_only"]);
        let json = summary.to_json();
        let mut keys: Vec<&str> = json
            .as_object()
            .expect("object")
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "boundary",
                "boundary_count",
                "property_count",
                "recorded_only_count",
                "total",
                "unknown_count",
                "verified_count",
            ]
        );
    }
}
