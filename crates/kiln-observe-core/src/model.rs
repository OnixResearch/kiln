use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use kiln_core::domain::Outcome;
use kiln_core::operations::JournalRecord;
use kiln_core::task_ops::LogStreamMeta;
use kiln_core::types::{
    Alias, AttemptId, CommandId, ContentId, ExternalRef, ProviderProfileId, RunId, StreamId,
};
use serde::{Deserialize, Serialize};

use crate::profile::{ViewField, ViewKind};
use crate::review::{ReviewFrictionView, ReviewWindow, SuggestionAnchorsView};

pub const OBSERVATION_VIEW_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationRequest {
    pub state_revision: u64,
    pub view: ViewRequest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewRequest {
    StateSummary,
    RunList {
        after: Option<RunId>,
        limit: u64,
    },
    RunDetail {
        run_id: RunId,
    },
    AttemptDetail {
        attempt_id: AttemptId,
    },
    EventTail {
        limit: u64,
    },
    LogRange {
        stream_id: StreamId,
        cursor: u64,
        max_bytes: u64,
    },
    ReviewFriction,
    SuggestionAnchors {
        cursor: u64,
        limit: u64,
    },
}

impl ViewRequest {
    #[must_use]
    pub const fn kind(&self) -> ViewKind {
        match self {
            Self::StateSummary => ViewKind::StateSummary,
            Self::RunList { .. } => ViewKind::RunList,
            Self::RunDetail { .. } => ViewKind::RunDetail,
            Self::AttemptDetail { .. } => ViewKind::AttemptDetail,
            Self::EventTail { .. } => ViewKind::EventTail,
            Self::LogRange { .. } => ViewKind::LogRange,
            Self::ReviewFriction => ViewKind::ReviewFriction,
            Self::SuggestionAnchors { .. } => ViewKind::SuggestionAnchors,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationWindow {
    pub events: Option<EventWindow>,
    pub log: Option<LogWindow>,
    /// Admitted review facts; omitted when absent so existing windows keep
    /// their canonical JSON form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewWindow>,
}

impl ObservationWindow {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            events: None,
            log: None,
            review: None,
        }
    }

    #[must_use]
    pub fn events(records: Vec<JournalRecord>) -> Self {
        Self {
            events: Some(EventWindow { records }),
            log: None,
            review: None,
        }
    }

    #[must_use]
    pub fn log(meta: LogStreamMeta, retained_bytes: Vec<u8>) -> Self {
        Self {
            events: None,
            log: Some(LogWindow {
                meta,
                retained_bytes,
            }),
            review: None,
        }
    }

    #[must_use]
    pub fn review(review: ReviewWindow) -> Self {
        Self {
            events: None,
            log: None,
            review: Some(review),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventWindow {
    pub records: Vec<JournalRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogWindow {
    pub meta: LogStreamMeta,
    pub retained_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationView {
    pub schema_version: u16,
    pub identity: ContentId,
    pub state_revision: u64,
    pub redacted_fields: BTreeSet<ViewField>,
    pub body: ViewBody,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewBody {
    StateSummary(StateSummaryView),
    RunList(RunListView),
    RunDetail(RunDetailView),
    AttemptDetail(AttemptDetailView),
    EventTail(EventTailView),
    LogRange(LogRangeView),
    NotFound(NotFoundView),
    ReviewFriction(ReviewFrictionView),
    SuggestionAnchors(SuggestionAnchorsView),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeCount {
    pub outcome: Outcome,
    pub count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateSummaryView {
    pub run_count: u64,
    pub attempt_count: u64,
    pub hold_count: u64,
    pub run_outcomes: Vec<OutcomeCount>,
    pub attempt_outcomes: Vec<OutcomeCount>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageInfo {
    pub requested_limit: u64,
    pub effective_limit: u64,
    pub returned: u64,
    pub capped: bool,
    pub has_more: bool,
    pub next: Option<RunId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSummaryView {
    pub run_id: RunId,
    pub active_attempt: AttemptId,
    pub outcome: Outcome,
    pub attempt_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunListView {
    pub runs: Vec<RunSummaryView>,
    pub page: PageInfo,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptSummaryView {
    pub attempt_id: AttemptId,
    pub provider_profile: Option<ProviderProfileId>,
    pub outcome: Outcome,
    pub product_count: u64,
    pub metric_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunDetailView {
    pub run: RunSummaryView,
    pub attempts: Vec<AttemptSummaryView>,
    pub attempts_truncated: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductView {
    pub identity: ContentId,
    pub producing_attempt: AttemptId,
    pub provider_reference: Option<ExternalRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricView {
    pub name: Alias,
    pub value: i64,
    pub producing_attempt: AttemptId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptDetailView {
    pub attempt_id: AttemptId,
    pub run_id: RunId,
    pub provider_profile: Option<ProviderProfileId>,
    pub outcome: Outcome,
    pub external_reference: Option<ExternalRef>,
    pub product_count: u64,
    pub metric_count: u64,
    pub products: Vec<ProductView>,
    pub metrics: Vec<MetricView>,
    pub products_truncated: u64,
    pub metrics_truncated: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventPageInfo {
    pub requested_limit: u64,
    pub effective_limit: u64,
    pub returned: u64,
    pub capped: bool,
    pub has_earlier: bool,
    pub start_revision: u64,
    pub end_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventProjection {
    RunStarted {
        run_id: RunId,
        attempt_id: AttemptId,
        provider_profile: Option<ProviderProfileId>,
    },
    AttemptCompleted {
        attempt_id: AttemptId,
        outcome: Outcome,
        external_reference: Option<ExternalRef>,
        product_count: u64,
        metric_count: u64,
    },
    AttemptUnknown {
        attempt_id: AttemptId,
        external_reference: Option<ExternalRef>,
    },
    AttemptReconciled {
        attempt_id: AttemptId,
        outcome: Outcome,
        external_reference: Option<ExternalRef>,
    },
    RetryStarted {
        run_id: RunId,
        prior_attempt_id: AttemptId,
        next_attempt_id: AttemptId,
        provider_profile: Option<ProviderProfileId>,
    },
    CancellationRequested {
        attempt_id: AttemptId,
    },
    HoldChanged {
        scope: Option<ExternalRef>,
        held: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRecordView {
    pub revision: u64,
    pub stream: Option<ExternalRef>,
    pub event_identity: Option<ExternalRef>,
    pub command_id: Option<CommandId>,
    pub event: EventProjection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTailView {
    pub records: Vec<EventRecordView>,
    pub page: EventPageInfo,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionView {
    Valid,
    Gap { retained_start: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogRangeView {
    pub stream_id: StreamId,
    pub attempt_id: AttemptId,
    pub cursor: u64,
    pub retained_start: u64,
    pub retained_end: u64,
    pub retention: RetentionView,
    pub requested_max_bytes: u64,
    pub effective_max_bytes: u64,
    pub returned_bytes: u64,
    pub capped: bool,
    pub has_more: bool,
    pub stream_complete: bool,
    pub stream_truncated: bool,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotFoundTarget {
    Run(RunId),
    Attempt(AttemptId),
    Stream(StreamId),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotFoundView {
    pub target: NotFoundTarget,
}

impl NotFoundView {
    #[must_use]
    pub fn new(target: NotFoundTarget) -> Self {
        Self { target }
    }
}
