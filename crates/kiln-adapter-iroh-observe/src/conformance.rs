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
    "verified_log_payload",
    "trusted_peer",
    "http_iroh_equality",
];

pub const REQUIRED_NEGATIVE_CASES: &[&str] = &[
    "profile_drift",
    "unknown_peer",
    "revoked_peer",
    "corrupted_payload",
    "transport_failure",
    "relay_dependence",
    "divergence",
    "missing_negative_case",
    "authority_promotion",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IrohConformanceManifest {
    pub positive: Vec<String>,
    pub negative: Vec<String>,
}

// r[impl kiln.observe.iroh.conformance]
// r[impl kiln.observe.iroh.conformance.scenario.fixture_missing]
pub fn admit_iroh_conformance(manifest: &IrohConformanceManifest) -> ObserveResult<()> {
    for required in REQUIRED_POSITIVE_CASES {
        if !manifest
            .positive
            .iter()
            .any(|case| case.as_str() == *required)
        {
            return Err(ObserveError::new(
                "missing_positive_case",
                "Iroh conformance manifest is missing a required positive case",
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
                "Iroh conformance manifest is missing a required negative case",
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
                "Iroh conformance manifest contains a duplicate case",
            ));
        }
    }
    Ok(())
}
