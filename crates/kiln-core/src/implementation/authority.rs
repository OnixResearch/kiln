//! Closed protected-operation authority coverage.
//!
//! Every protected path in the selected implementation profile maps to exactly one
//! reviewed Basalt operation, one enforcement owner, one effect class and the
//! candidate rule. This module admits and reports that mapping; it never verifies
//! authority, resolves credentials, executes work or performs an effect.
//!
//! An unmapped path or operation denies. No operation implies another: claim,
//! dispatch and submission authority never stand in for verification, publication,
//! integration or lifecycle authority.

use serde::{Deserialize, Serialize};

use crate::types::{CoreResult, Diagnostic};

pub const IMPLEMENTATION_AUTHORITY_SCHEMA: &str = "kiln.implementation-authority-coverage.v1";
pub const MAX_OWNER_PATH_BYTES: usize = 256;

/// Closed protected implementation operations, one-to-one with the reviewed
/// Basalt adapter vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtectedOperation {
    ClaimAcquire,
    ClaimRenew,
    Dispatch,
    CandidateSubmit,
    VerificationAdmit,
    AttemptCancel,
    PublicationPublish,
    IntegrationLand,
    LifecycleComplete,
}

impl ProtectedOperation {
    /// Every protected operation in review order.
    pub const ALL: [Self; 9] = [
        Self::ClaimAcquire,
        Self::ClaimRenew,
        Self::Dispatch,
        Self::CandidateSubmit,
        Self::VerificationAdmit,
        Self::AttemptCancel,
        Self::PublicationPublish,
        Self::IntegrationLand,
        Self::LifecycleComplete,
    ];

    /// Position in [`Self::ALL`] and [`COVERAGE`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::ClaimAcquire => 0,
            Self::ClaimRenew => 1,
            Self::Dispatch => 2,
            Self::CandidateSubmit => 3,
            Self::VerificationAdmit => 4,
            Self::AttemptCancel => 5,
            Self::PublicationPublish => 6,
            Self::IntegrationLand => 7,
            Self::LifecycleComplete => 8,
        }
    }

    /// Stable Basalt operation identifier. This MUST equal the reviewed adapter's
    /// serialized operation name; the adapter and this table are reviewed together.
    #[must_use]
    pub const fn basalt_operation(self) -> &'static str {
        match self {
            Self::ClaimAcquire => "claim-acquire",
            Self::ClaimRenew => "claim-renew",
            Self::Dispatch => "dispatch",
            Self::CandidateSubmit => "candidate-submit",
            Self::VerificationAdmit => "verification-admit",
            Self::AttemptCancel => "attempt-cancel",
            Self::PublicationPublish => "publication-publish",
            Self::IntegrationLand => "integration-land",
            Self::LifecycleComplete => "lifecycle-complete",
        }
    }

    /// Whether the operation binds an exact candidate identity.
    #[must_use]
    pub const fn requires_candidate(self) -> bool {
        match self {
            Self::ClaimAcquire | Self::ClaimRenew | Self::Dispatch | Self::AttemptCancel => false,
            Self::CandidateSubmit
            | Self::VerificationAdmit
            | Self::PublicationPublish
            | Self::IntegrationLand
            | Self::LifecycleComplete => true,
        }
    }
}

/// Enforcement owner for one protected operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnforcementBoundary {
    Admission,
    Campaign,
    Verification,
    Publication,
    Lifecycle,
}

impl EnforcementBoundary {
    /// Stable boundary label for redacted decisions.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admission => "admission",
            Self::Campaign => "campaign",
            Self::Verification => "verification",
            Self::Publication => "publication",
            Self::Lifecycle => "lifecycle",
        }
    }
}

/// Effect class governed by the operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityEffect {
    Mutation,
    Publication,
    Integration,
    Lifecycle,
}

impl AuthorityEffect {
    /// Stable effect label for redacted decisions.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mutation => "mutation",
            Self::Publication => "publication",
            Self::Integration => "integration",
            Self::Lifecycle => "lifecycle",
        }
    }
}

/// One reviewed coverage entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageEntry {
    /// Protected operation.
    pub operation: ProtectedOperation,
    /// Basalt operation identifier.
    pub basalt_operation: &'static str,
    /// Enforcement boundary.
    pub owner: EnforcementBoundary,
    /// Reviewed repository path that owns enforcement.
    pub owner_path: &'static str,
    /// Effect class.
    pub effect: AuthorityEffect,
    /// Whether the operation binds an exact candidate.
    pub candidate_bound: bool,
}

// r[impl kiln.implementation.authority.coverage]
/// Every protected operation with its reviewed owner and binding rule.
pub const COVERAGE: [CoverageEntry; 9] = [
    CoverageEntry {
        operation: ProtectedOperation::ClaimAcquire,
        basalt_operation: "claim-acquire",
        owner: EnforcementBoundary::Admission,
        owner_path: "crates/kiln-implementer/src/admission.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: false,
    },
    CoverageEntry {
        operation: ProtectedOperation::ClaimRenew,
        basalt_operation: "claim-renew",
        owner: EnforcementBoundary::Admission,
        owner_path: "crates/kiln-implementer/src/admission.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: false,
    },
    CoverageEntry {
        operation: ProtectedOperation::Dispatch,
        basalt_operation: "dispatch",
        owner: EnforcementBoundary::Admission,
        owner_path: "crates/kiln-implementer/src/admission.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: false,
    },
    CoverageEntry {
        operation: ProtectedOperation::CandidateSubmit,
        basalt_operation: "candidate-submit",
        owner: EnforcementBoundary::Campaign,
        owner_path: "crates/kiln-implementer/src/campaign.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: true,
    },
    CoverageEntry {
        operation: ProtectedOperation::VerificationAdmit,
        basalt_operation: "verification-admit",
        owner: EnforcementBoundary::Verification,
        owner_path: "crates/kiln-implementer/src/verification.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: true,
    },
    CoverageEntry {
        operation: ProtectedOperation::AttemptCancel,
        basalt_operation: "attempt-cancel",
        owner: EnforcementBoundary::Campaign,
        owner_path: "crates/kiln-implementer/src/campaign.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: false,
    },
    CoverageEntry {
        operation: ProtectedOperation::PublicationPublish,
        basalt_operation: "publication-publish",
        owner: EnforcementBoundary::Publication,
        owner_path: "crates/kiln-implementer/src/publication_effects.rs",
        effect: AuthorityEffect::Publication,
        candidate_bound: true,
    },
    CoverageEntry {
        operation: ProtectedOperation::IntegrationLand,
        basalt_operation: "integration-land",
        owner: EnforcementBoundary::Publication,
        owner_path: "crates/kiln-implementer/src/native_publication.rs",
        effect: AuthorityEffect::Integration,
        candidate_bound: true,
    },
    CoverageEntry {
        operation: ProtectedOperation::LifecycleComplete,
        basalt_operation: "lifecycle-complete",
        owner: EnforcementBoundary::Lifecycle,
        owner_path: "crates/kiln-implementer/src/native_publication.rs",
        effect: AuthorityEffect::Lifecycle,
        candidate_bound: true,
    },
];

/// Return the reviewed entry for one protected operation.
#[must_use]
pub fn coverage(operation: ProtectedOperation) -> &'static CoverageEntry {
    match COVERAGE.get(operation.index()) {
        Some(entry) => entry,
        None => unreachable!("closed protected-operation index"),
    }
}

/// Return the reviewed entry for one protected path, or `None` when unmapped.
#[must_use]
pub fn coverage_for_path(path: &str) -> Option<&'static CoverageEntry> {
    COVERAGE.iter().find(|entry| entry.owner_path == path)
}

/// Verify that a coverage table is closed, consistent and unambiguous.
///
/// # Errors
/// Returns a diagnostic for a missing or duplicate operation, a duplicate Basalt
/// operation identifier, an empty or oversized owner path, a mismatched Basalt
/// name, or a candidate rule that disagrees with the operation.
pub fn validate_coverage(entries: &[CoverageEntry]) -> CoreResult<()> {
    if entries.len() != ProtectedOperation::ALL.len() {
        return Err(Diagnostic::new(
            "authority_coverage_incomplete",
            "coverage",
            "coverage must list every protected operation exactly once",
        ));
    }
    for (index, entry) in entries.iter().enumerate() {
        if entry.owner_path.is_empty()
            || entry.owner_path.len() > MAX_OWNER_PATH_BYTES
            || entry.owner_path.trim() != entry.owner_path
        {
            return Err(Diagnostic::new(
                "authority_coverage_owner",
                "coverage.owner_path",
                "owner path must be a bounded non-empty repository path",
            ));
        }
        if entry.effect != authority_effect(entry.operation) {
            return Err(Diagnostic::new(
                "authority_coverage_effect",
                "coverage.effect",
                "effect class does not match the reviewed operation",
            ));
        }
        if entry.basalt_operation != entry.operation.basalt_operation() {
            return Err(Diagnostic::new(
                "authority_coverage_operation",
                "coverage.basalt_operation",
                "Basalt operation does not match the protected operation",
            ));
        }
        if entry.candidate_bound != entry.operation.requires_candidate() {
            return Err(Diagnostic::new(
                "authority_coverage_candidate",
                "coverage.candidate_bound",
                "candidate binding does not match the reviewed operation",
            ));
        }
        for other in entries.iter().skip(index + 1) {
            if other.operation == entry.operation {
                return Err(Diagnostic::new(
                    "authority_coverage_duplicate",
                    "coverage.operation",
                    "protected operation appears more than once",
                ));
            }
            if other.basalt_operation == entry.basalt_operation {
                return Err(Diagnostic::new(
                    "authority_coverage_duplicate",
                    "coverage.basalt_operation",
                    "Basalt operation id appears more than once",
                ));
            }
        }
    }
    for (position, operation) in ProtectedOperation::ALL.iter().enumerate() {
        let Some(entry) = entries.get(position) else {
            return Err(Diagnostic::new(
                "authority_coverage_incomplete",
                "coverage.operation",
                "coverage must list every protected operation exactly once",
            ));
        };
        if entry.operation != *operation {
            return Err(Diagnostic::new(
                "authority_coverage_order",
                "coverage.operation",
                "coverage order must match the closed operation order",
            ));
        }
    }
    Ok(())
}

const fn authority_effect(operation: ProtectedOperation) -> AuthorityEffect {
    match operation {
        ProtectedOperation::ClaimAcquire
        | ProtectedOperation::ClaimRenew
        | ProtectedOperation::Dispatch
        | ProtectedOperation::CandidateSubmit
        | ProtectedOperation::VerificationAdmit
        | ProtectedOperation::AttemptCancel => AuthorityEffect::Mutation,
        ProtectedOperation::PublicationPublish => AuthorityEffect::Publication,
        ProtectedOperation::IntegrationLand => AuthorityEffect::Integration,
        ProtectedOperation::LifecycleComplete => AuthorityEffect::Lifecycle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // r[verify kiln.implementation.authority.coverage.scenario.complete]
    fn coverage_is_complete_closed_and_unique() {
        assert_eq!(COVERAGE.len(), ProtectedOperation::ALL.len());
        validate_coverage(&COVERAGE).unwrap_or_else(|error| unreachable!("{error:?}"));
        for (position, operation) in ProtectedOperation::ALL.iter().enumerate() {
            let entry = coverage(*operation);
            assert_eq!(entry.operation, *operation);
            assert_eq!(entry.basalt_operation, operation.basalt_operation());
            assert_eq!(entry.candidate_bound, operation.requires_candidate());
            assert!(!entry.owner_path.is_empty());
            assert_eq!(
                COVERAGE.get(position).map(|entry| entry.operation),
                Some(*operation)
            );
        }
        assert_eq!(
            COVERAGE
                .iter()
                .map(|entry| entry.basalt_operation)
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![
                "claim-acquire",
                "claim-renew",
                "dispatch",
                "candidate-submit",
                "verification-admit",
                "attempt-cancel",
                "publication-publish",
                "integration-land",
                "lifecycle-complete",
            ]
        );
    }

    #[test]
    // r[verify kiln.implementation.authority.coverage.scenario.unmapped]
    fn unmapped_protected_path_denies() {
        assert!(coverage_for_path("crates/kiln-implementer/src/unknown.rs").is_none());
        assert_eq!(
            coverage_for_path("crates/kiln-implementer/src/admission.rs")
                .map(|entry| entry.operation),
            Some(ProtectedOperation::ClaimAcquire)
        );
    }

    #[test]
    fn inconsistent_coverage_is_rejected() {
        let mut duplicate = COVERAGE;
        duplicate[1].operation = ProtectedOperation::ClaimAcquire;
        assert_eq!(
            validate_coverage(&duplicate).err().map(|error| error.code),
            Some("authority_coverage_duplicate".into())
        );
        let mut candidate = COVERAGE;
        candidate[0].candidate_bound = true;
        assert_eq!(
            validate_coverage(&candidate).err().map(|error| error.code),
            Some("authority_coverage_candidate".into())
        );
        let mut effect = COVERAGE;
        effect[3].effect = AuthorityEffect::Publication;
        assert_eq!(
            validate_coverage(&effect).err().map(|error| error.code),
            Some("authority_coverage_effect".into())
        );
        assert_eq!(
            validate_coverage(&[]).err().map(|error| error.code),
            Some("authority_coverage_incomplete".into())
        );
    }
}
