//! EARS property vocabulary: classifies EARS requirement responses into
//! checkable property predicates.
//!
//! This module implements the c-earchin vocabulary bridge: an EARS requirement
//! response (the text after "shall") is classified against a fixed vocabulary
//! of pure-boolean predicates. If the response resolves to a predicate, the
//! requirement becomes a real property witness. If it does not resolve, the
//! requirement is marked `recorded_only` — it carries structure but proves
//! nothing, and a "0 verified" result means "nothing was verified," not "all
//! good."
//!
//! This is the c-earchin `verification-scope` discipline applied to cairn:
//! translating a requirement is not the same as proving it. A witness becomes
//! a proof property only when its response resolves to a vocabulary predicate.

/// The c-earchin vocabulary: pure-boolean predicates that cairn knows how to
/// turn into checkable properties.
///
/// These mirror the c-earchin nine plus trellis-shaped predicates for the
/// systems domains trellis covers.
// r[impl cairn.ears_vocabulary.predicates]
pub const VOCABULARY_PREDICATES: &[&str] = &[
    // c-earchin nine
    "is_non_negative",
    "is_positive",
    "within_tolerance",
    "is_monotone_increasing",
    "is_bounded_above",
    "is_bounded_below",
    "is_equal",
    "converges_to",
    "is_error_rejection",
    // trellis-shaped
    "preserves_invariant",
    "bounded_resource",
    "lock_order_acyclic",
    "monotone_advance",
    "no_deadlock",
    "crash_safe",
    "linearizable",
];

/// The result of classifying an EARS requirement response against the vocabulary.
// r[impl cairn.ears_vocabulary]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VocabularyResolution {
    /// The response resolved to a vocabulary predicate.
    Resolved { predicate: String },
    /// The response did not resolve; the requirement is recorded-only.
    RecordedOnly { response: String },
}

/// A report summarizing vocabulary coverage across a set of EARS requirements.
// r[impl cairn.ears_vocabulary.boundary]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarsVocabularyReport {
    pub total: usize,
    pub resolved_count: usize,
    pub recorded_only_count: usize,
    pub strict_mode: bool,
    pub valid: bool,
    pub unresolved_responses: Vec<String>,
    pub boundary: String,
}

impl EarsVocabularyReport {
    /// Render the report as a JSON object.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "total": self.total,
            "resolved_count": self.resolved_count,
            "recorded_only_count": self.recorded_only_count,
            "strict_mode": self.strict_mode,
            "valid": self.valid,
            "unresolved_responses": self.unresolved_responses,
            "boundary": self.boundary,
        })
    }
}

/// The required non-claim boundary for EARS vocabulary reports.
pub const EARS_VOCABULARY_BOUNDARY: &str = "EARS vocabulary coverage records which requirement responses resolve to checkable predicates; unresolved responses are recorded_only and prove nothing";

/// Classify an EARS requirement response against the vocabulary.
///
/// The response is the text after "shall" in an EARS requirement. If it
/// contains a phrase that maps to a vocabulary predicate, the resolution is
/// `Resolved`. Otherwise it is `RecordedOnly`.
///
/// The matching is keyword-based: the response text is searched for substrings
/// that indicate the predicate. For example, "produce non-negative call
/// prices" resolves to `is_non_negative`.
#[must_use]
pub fn classify_ears_response(response: &str) -> VocabularyResolution {
    let lower = response.to_ascii_lowercase();
    for (keyword, predicate) in RESPONSE_KEYWORD_MAP {
        if lower.contains(keyword) {
            return VocabularyResolution::Resolved {
                predicate: predicate.to_string(),
            };
        }
    }
    VocabularyResolution::RecordedOnly {
        response: response.to_string(),
    }
}

/// Keyword-to-predicate mapping for response classification.
const RESPONSE_KEYWORD_MAP: &[(&str, &str)] = &[
    ("non-negative", "is_non_negative"),
    ("nonneg", "is_non_negative"),
    ("positive", "is_positive"),
    ("within tolerance", "within_tolerance"),
    ("within the tolerance", "within_tolerance"),
    ("monotone", "is_monotone_increasing"),
    ("monotone-increasing", "is_monotone_increasing"),
    ("at most", "is_bounded_above"),
    ("bounded by", "is_bounded_above"),
    ("bounded above", "is_bounded_above"),
    ("at least", "is_bounded_below"),
    ("bounded below", "is_bounded_below"),
    ("equal", "is_equal"),
    ("converges", "converges_to"),
    ("reject", "is_error_rejection"),
    ("rejects", "is_error_rejection"),
    ("fail", "is_error_rejection"),
    ("preserves", "preserves_invariant"),
    ("preserve", "preserves_invariant"),
    ("invariant", "preserves_invariant"),
    ("bounded resource", "bounded_resource"),
    ("capacity", "bounded_resource"),
    ("deadlock", "no_deadlock"),
    ("cycle-free", "lock_order_acyclic"),
    ("acyclic", "lock_order_acyclic"),
    ("monotone advance", "monotone_advance"),
    ("advancement", "monotone_advance"),
    ("crash-safe", "crash_safe"),
    ("crash safe", "crash_safe"),
    ("linearizable", "linearizable"),
    ("linearizability", "linearizable"),
];

/// Check EARS vocabulary coverage across a set of requirement responses.
///
/// In strict mode, the report is invalid if any response is unresolved
/// (recorded_only). In non-strict mode, unresolved responses are accepted but
/// counted separately.
// r[impl cairn.ears_vocabulary.strict]
#[must_use]
pub fn check_ears_vocabulary_coverage(
    resolutions: &[VocabularyResolution],
    strict: bool,
) -> EarsVocabularyReport {
    let total = resolutions.len();
    let mut resolved_count = 0;
    let mut recorded_only_count = 0;
    let mut unresolved_responses = Vec::new();

    for resolution in resolutions {
        match resolution {
            VocabularyResolution::Resolved { .. } => resolved_count += 1,
            VocabularyResolution::RecordedOnly { response } => {
                recorded_only_count += 1;
                unresolved_responses.push(response.clone());
            }
        }
    }

    let valid = !strict || recorded_only_count == 0;

    EarsVocabularyReport {
        total,
        resolved_count,
        recorded_only_count,
        strict_mode: strict,
        valid,
        unresolved_responses,
        boundary: EARS_VOCABULARY_BOUNDARY.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_VOCABULARY_PREDICATE_COUNT: usize = 16;

    #[test]
    // r[verify cairn.ears_vocabulary.non_negative]
    fn classifies_non_negative_response() {
        let result = classify_ears_response("produce non-negative call prices");
        assert!(
            matches!(result, VocabularyResolution::Resolved { predicate } if predicate == "is_non_negative")
        );
    }

    #[test]
    fn classifies_monotone_response() {
        let result = classify_ears_response("the call price curve shall be monotone-increasing");
        assert!(
            matches!(result, VocabularyResolution::Resolved { predicate } if predicate == "is_monotone_increasing")
        );
    }

    #[test]
    fn classifies_reject_response() {
        let result = classify_ears_response("the order gateway shall reject the order");
        assert!(
            matches!(result, VocabularyResolution::Resolved { predicate } if predicate == "is_error_rejection")
        );
    }

    #[test]
    fn classifies_preserves_invariant_response() {
        let result = classify_ears_response("the system shall preserve the log matching property");
        assert!(
            matches!(result, VocabularyResolution::Resolved { predicate } if predicate == "preserves_invariant")
        );
    }

    #[test]
    fn classifies_deadlock_response() {
        let result = classify_ears_response("the system shall be free of deadlock");
        assert!(
            matches!(result, VocabularyResolution::Resolved { predicate } if predicate == "no_deadlock")
        );
    }

    #[test]
    // r[verify cairn.ears_vocabulary.unresolvable]
    fn classifies_unresolvable_response_as_recorded_only() {
        let result = classify_ears_response("make users happy");
        assert!(
            matches!(result, VocabularyResolution::RecordedOnly { response } if response == "make users happy")
        );
    }

    #[test]
    fn classifies_empty_response_as_recorded_only() {
        let result = classify_ears_response("");
        assert!(
            matches!(result, VocabularyResolution::RecordedOnly { response } if response.is_empty())
        );
    }

    #[test]
    // r[verify cairn.ears_vocabulary.strict.fail]
    fn strict_mode_fails_on_unresolved() {
        let resolutions = vec![
            classify_ears_response("produce non-negative prices"),
            classify_ears_response("make users happy"),
        ];
        let report = check_ears_vocabulary_coverage(&resolutions, true);
        assert!(!report.valid);
        assert_eq!(report.resolved_count, 1);
        assert_eq!(report.recorded_only_count, 1);
        assert_eq!(report.unresolved_responses, vec!["make users happy"]);
    }

    #[test]
    // r[verify cairn.ears_vocabulary.strict.pass]
    fn strict_mode_passes_when_all_resolved() {
        let resolutions = vec![
            classify_ears_response("produce non-negative prices"),
            classify_ears_response("reject the order"),
        ];
        let report = check_ears_vocabulary_coverage(&resolutions, true);
        assert!(report.valid);
        assert_eq!(report.resolved_count, 2);
        assert_eq!(report.recorded_only_count, 0);
    }

    #[test]
    fn non_strict_mode_accepts_unresolved() {
        let resolutions = vec![
            classify_ears_response("produce non-negative prices"),
            classify_ears_response("make users happy"),
        ];
        let report = check_ears_vocabulary_coverage(&resolutions, false);
        assert!(report.valid);
        assert_eq!(report.resolved_count, 1);
        assert_eq!(report.recorded_only_count, 1);
    }

    #[test]
    fn empty_resolutions_pass() {
        let report = check_ears_vocabulary_coverage(&[], true);
        assert!(report.valid);
        assert_eq!(report.total, 0);
    }

    #[test]
    // r[verify cairn.ears_vocabulary.boundary.present]
    fn report_includes_boundary_non_claim() {
        let report = check_ears_vocabulary_coverage(&[], false);
        assert!(
            report
                .boundary
                .contains("records which requirement responses resolve"),
            "boundary must state the non-claim"
        );
    }

    #[test]
    // r[verify cairn.ears_vocabulary.predicates.stable]
    fn vocabulary_predicates_are_stable() {
        assert!(VOCABULARY_PREDICATES.contains(&"is_non_negative"));
        assert!(VOCABULARY_PREDICATES.contains(&"is_monotone_increasing"));
        assert!(VOCABULARY_PREDICATES.contains(&"preserves_invariant"));
        assert!(VOCABULARY_PREDICATES.contains(&"no_deadlock"));
        assert!(VOCABULARY_PREDICATES.contains(&"linearizable"));
        assert_eq!(
            VOCABULARY_PREDICATES.len(),
            EXPECTED_VOCABULARY_PREDICATE_COUNT
        );
    }
}
