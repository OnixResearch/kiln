use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use kiln_core::types::{ContentId, CoreResult, Diagnostic};
use serde::{Deserialize, Serialize};

use crate::OBSERVATION_NON_CLAIMS;

pub const OBSERVATION_PROFILE_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewKind {
    StateSummary,
    RunList,
    RunDetail,
    AttemptDetail,
    EventTail,
    LogRange,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewField {
    ProviderProfile,
    ExternalReference,
    Products,
    Metrics,
    CommandId,
    LogBytes,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationBounds {
    pub max_page_items: u64,
    pub max_attempts_per_run: u64,
    pub max_products_per_attempt: u64,
    pub max_metrics_per_attempt: u64,
    pub max_event_records: u64,
    pub max_log_bytes: u64,
    pub max_log_window_bytes: u64,
    pub max_log_segments: u64,
    pub max_text_bytes: u64,
    pub max_encoded_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedactionPolicy {
    pub visible_fields: BTreeSet<ViewField>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationProfile {
    pub schema_version: u16,
    pub profile_id: String,
    pub views: BTreeSet<ViewKind>,
    pub bounds: ObservationBounds,
    pub redaction: RedactionPolicy,
    pub non_claims: Vec<String>,
}

impl ObservationProfile {
    // r[impl kiln.observe.profile]
    // r[impl kiln.observe.profile.scenario.match]
    pub fn admit(&self) -> CoreResult<()> {
        if self.schema_version != OBSERVATION_PROFILE_SCHEMA_VERSION {
            return Err(Diagnostic::new(
                "unsupported_profile_version",
                "observation.profile.schema_version",
                "observation profile version is unsupported",
            ));
        }
        let max_text = text_bound(self.bounds.max_text_bytes);
        if self.profile_id.is_empty() || self.profile_id.len() > max_text {
            return Err(Diagnostic::new(
                "profile_identifier_bound",
                "observation.profile.profile_id",
                "profile identifier is empty or exceeds its named byte bound",
            ));
        }
        if self.views.is_empty() {
            return Err(Diagnostic::new(
                "empty_view_vocabulary",
                "observation.profile.views",
                "profile must declare at least one view kind",
            ));
        }
        for value in [
            self.bounds.max_page_items,
            self.bounds.max_attempts_per_run,
            self.bounds.max_products_per_attempt,
            self.bounds.max_metrics_per_attempt,
            self.bounds.max_event_records,
            self.bounds.max_log_bytes,
            self.bounds.max_log_window_bytes,
            self.bounds.max_log_segments,
            self.bounds.max_text_bytes,
            self.bounds.max_encoded_bytes,
        ] {
            if value == 0 {
                return Err(Diagnostic::new(
                    "zero_bound",
                    "observation.profile.bounds",
                    "profile bound must be positive",
                ));
            }
        }
        if self.non_claims.is_empty() {
            return Err(Diagnostic::new(
                "missing_non_claims",
                "observation.profile.non_claims",
                "profile must declare non-claims",
            ));
        }
        for non_claim in &self.non_claims {
            if non_claim.is_empty() || non_claim.len() > max_text {
                return Err(Diagnostic::new(
                    "non_claim_bound",
                    "observation.profile.non_claims",
                    "non-claim is empty or exceeds its named byte bound",
                ));
            }
        }
        for required in OBSERVATION_NON_CLAIMS {
            if !self
                .non_claims
                .iter()
                .any(|value| value.as_str() == *required)
            {
                return Err(Diagnostic::new(
                    "missing_required_non_claim",
                    "observation.profile.non_claims",
                    "profile omits a required observation non-claim",
                ));
            }
        }
        Ok(())
    }

    // r[impl kiln.observe.profile.scenario.unknown_view]
    #[must_use]
    pub fn admits_view(&self, kind: ViewKind) -> bool {
        self.views.contains(&kind)
    }

    #[must_use]
    pub fn field_visible(&self, field: ViewField) -> bool {
        self.redaction.visible_fields.contains(&field)
    }

    pub fn identity(&self) -> CoreResult<ContentId> {
        crate::identity::profile_identity(self)
    }
}

pub(crate) fn text_bound(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}
