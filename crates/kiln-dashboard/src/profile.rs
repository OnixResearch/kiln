use kiln_core::types::{CoreResult, Diagnostic};
use kiln_observe_core::ObservationProfile;
use serde::{Deserialize, Serialize};

/// Schema version of the accepted dashboard profile document.
pub const DASHBOARD_PROFILE_SCHEMA_VERSION: u16 = 1;

/// Named bound on one dashboard profile document read.
pub const MAX_PROFILE_BYTES: usize = 1024 * 1024;

/// Non-claims every admitted dashboard profile must declare.
pub const DASHBOARD_NON_CLAIMS: &[&str] = &[
    "dashboard rendering does not prove CI success",
    "dashboard rendering does not prove provider correctness",
    "dashboard rendering does not prove source trust",
    "dashboard rendering does not prove journal completeness",
    "dashboard rendering does not establish release readiness",
    "dashboard rendering does not prove the observed revision is the newest existing revision",
];

// r[impl kiln.observe.dashboard.profile]
// r[impl kiln.observe.dashboard.profile.scenario.match]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardProfile {
    pub schema_version: u16,
    pub profile_id: String,
    pub observation: ObservationProfile,
    pub run_page_limit: u64,
    pub max_run_pages: u64,
    pub event_limit: u64,
    pub watch_interval_ms: u64,
    pub non_claims: Vec<String>,
}

impl DashboardProfile {
    pub fn admit(&self) -> CoreResult<()> {
        if self.schema_version != DASHBOARD_PROFILE_SCHEMA_VERSION {
            return Err(Diagnostic::new(
                "unsupported_profile_version",
                "dashboard.profile.schema_version",
                "dashboard profile version is unsupported",
            ));
        }
        if self.profile_id.is_empty() {
            return Err(Diagnostic::new(
                "profile_identifier_bound",
                "dashboard.profile.profile_id",
                "dashboard profile identifier is empty",
            ));
        }
        self.observation.admit()?;
        for (value, field) in [
            (self.run_page_limit, "run_page_limit"),
            (self.max_run_pages, "max_run_pages"),
            (self.event_limit, "event_limit"),
            (self.watch_interval_ms, "watch_interval_ms"),
        ] {
            if value == 0 {
                // r[impl kiln.observe.dashboard.profile.scenario.zero_bound]
                return Err(Diagnostic::new(
                    "zero_bound",
                    field,
                    "dashboard profile bound must be positive",
                ));
            }
        }
        if self.non_claims.is_empty() {
            return Err(Diagnostic::new(
                "missing_non_claims",
                "dashboard.profile.non_claims",
                "dashboard profile must declare non-claims",
            ));
        }
        for required in DASHBOARD_NON_CLAIMS {
            if !self
                .non_claims
                .iter()
                .any(|value| value.as_str() == *required)
            {
                // r[impl kiln.observe.dashboard.profile.scenario.missing_non_claim]
                return Err(Diagnostic::new(
                    "missing_required_non_claim",
                    "dashboard.profile.non_claims",
                    "dashboard profile omits a required dashboard non-claim",
                ));
            }
        }
        Ok(())
    }
}

// r[impl kiln.observe.dashboard.profile.scenario.match]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDocument {
    pub profile: DashboardProfile,
}

impl ProfileDocument {
    pub fn admit(&self) -> CoreResult<()> {
        self.profile.admit()
    }
}
