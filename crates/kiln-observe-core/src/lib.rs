#![no_std]

// r[impl kiln.observe.projection]
extern crate alloc;

mod conformance;
mod identity;
mod model;
mod profile;
mod projection;
mod review;

#[cfg(test)]
mod review_tests;
#[cfg(test)]
mod tests;

pub use conformance::{
    ConformanceManifest, REQUIRED_NEGATIVE_CASES, REQUIRED_POSITIVE_CASES, admit_conformance,
};
pub use model::{
    AttemptDetailView, AttemptSummaryView, EventPageInfo, EventProjection, EventRecordView,
    EventTailView, EventWindow, LogRangeView, LogWindow, MetricView, NotFoundTarget, NotFoundView,
    OBSERVATION_VIEW_SCHEMA_VERSION, ObservationRequest, ObservationView, ObservationWindow,
    OutcomeCount, PageInfo, ProductView, RetentionView, RunDetailView, RunListView, RunSummaryView,
    StateSummaryView, ViewBody, ViewRequest,
};
pub use profile::{
    ObservationBounds, ObservationProfile, RedactionPolicy, ReviewBounds, ViewField, ViewKind,
};
pub use projection::project_observation;
pub use review::{
    AnchorPageInfo, AuthorshipBasis, AuthorshipFrictionView, Distribution, PER_MILLE,
    ReviewApprovalFact, ReviewChangeFact, ReviewFrictionView, ReviewSuggestionFact, ReviewWindow,
    SuggestionAnchorView, SuggestionAnchorsView, admit_review_window, review_window,
};

// r[impl kiln.observe.nonclaims]
pub const OBSERVATION_VIEW_PROFILE: &str = "kiln.observe.view.v1";

pub const OBSERVATION_NON_CLAIMS: &[&str] = &[
    "bounded observation projection does not prove transport delivery",
    "bounded observation projection does not prove live freshness",
    "bounded observation projection does not prove control execution",
    "bounded observation projection does not prove provider correctness",
    "bounded observation projection does not prove CI success",
    "bounded observation projection does not establish release readiness",
];
