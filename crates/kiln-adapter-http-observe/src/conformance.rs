use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{ObserveError, ObserveResult};

pub const REQUIRED_POSITIVE_CASES: &[&str] = &[
    "state_summary",
    "run_list",
    "run_detail",
    "attempt_detail",
    "event_tail",
    "log_range",
    "live_delivery",
    "authorized",
];

pub const REQUIRED_NEGATIVE_CASES: &[&str] = &[
    "profile_drift",
    "unknown_view",
    "unknown_target",
    "unauthorized",
    "malformed_request",
    "tls_failure",
    "timeout",
    "stream_loss",
    "missing_negative_case",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpConformanceManifest {
    pub positive: Vec<String>,
    pub negative: Vec<String>,
}

// r[impl kiln.observe.http.conformance]
// r[impl kiln.observe.http.conformance.scenario.fixture_missing]
pub fn admit_http_conformance(manifest: &HttpConformanceManifest) -> ObserveResult<()> {
    for required in REQUIRED_POSITIVE_CASES {
        if !manifest
            .positive
            .iter()
            .any(|case| case.as_str() == *required)
        {
            return Err(ObserveError::new(
                "missing_positive_case",
                "HTTP conformance manifest is missing a required positive case",
            ));
        }
    }
    for required in REQUIRED_NEGATIVE_CASES {
        if !manifest
            .negative
            .iter()
            .any(|case| case.as_str() == *required)
        {
            return Err(ObserveError::new(
                "missing_negative_case",
                "HTTP conformance manifest is missing a required negative case",
            ));
        }
    }
    reject_duplicates(&manifest.positive)?;
    reject_duplicates(&manifest.negative)?;
    Ok(())
}

fn reject_duplicates(cases: &[String]) -> ObserveResult<()> {
    let mut seen = BTreeSet::new();
    for case in cases {
        if !seen.insert(case.as_str()) {
            return Err(ObserveError::new(
                "duplicate_case",
                "HTTP conformance manifest contains a duplicate case",
            ));
        }
    }
    Ok(())
}
