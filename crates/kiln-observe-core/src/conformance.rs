use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use kiln_core::types::{CoreResult, Diagnostic};
use serde::{Deserialize, Serialize};

pub const REQUIRED_POSITIVE_CASES: &[&str] = &[
    "state_summary",
    "run_list",
    "run_detail",
    "attempt_detail",
    "event_tail",
    "log_range",
    "rebuild_identity",
    "read_only",
];

pub const REQUIRED_NEGATIVE_CASES: &[&str] = &[
    "unknown_view",
    "unknown_state_revision",
    "unknown_target",
    "page_overflow",
    "cursor_outside_window",
    "redaction",
    "missing_negative_case",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConformanceManifest {
    pub positive: Vec<String>,
    pub negative: Vec<String>,
}

// r[impl kiln.observe.conformance]
// r[impl kiln.observe.conformance.scenario.negative_missing]
pub fn admit_conformance(manifest: &ConformanceManifest) -> CoreResult<()> {
    for required in REQUIRED_POSITIVE_CASES {
        if !manifest
            .positive
            .iter()
            .any(|case| case.as_str() == *required)
        {
            return Err(Diagnostic::new(
                "missing_positive_case",
                "observation.conformance.positive",
                "conformance manifest is missing a required positive case",
            ));
        }
    }
    for required in REQUIRED_NEGATIVE_CASES {
        if !manifest
            .negative
            .iter()
            .any(|case| case.as_str() == *required)
        {
            return Err(Diagnostic::new(
                "missing_negative_case",
                "observation.conformance.negative",
                "conformance manifest is missing a required negative case",
            ));
        }
    }
    reject_duplicates(&manifest.positive, "observation.conformance.positive")?;
    reject_duplicates(&manifest.negative, "observation.conformance.negative")?;
    Ok(())
}

fn reject_duplicates(cases: &[String], field: &'static str) -> CoreResult<()> {
    let mut seen = BTreeSet::new();
    for case in cases {
        if !seen.insert(case.as_str()) {
            return Err(Diagnostic::new(
                "duplicate_case",
                field,
                "conformance manifest contains a duplicate case",
            ));
        }
    }
    Ok(())
}
