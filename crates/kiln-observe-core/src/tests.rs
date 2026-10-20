use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use kiln_core::domain::{AttemptState, Event, Metric, Outcome, Product, RunState, State};
use kiln_core::operations::JournalRecord;
use kiln_core::task_ops::{LogSegmentMeta, LogStreamMeta};
use kiln_core::types::{
    Alias, AttemptId, CommandId, ContentId, CoreResult, ExternalRef, ProviderProfileId, RunId,
    StreamId,
};
use serde::Deserialize;

use crate::*;

fn run_id(value: &str) -> RunId {
    RunId::new(value).unwrap_or_else(|_| unreachable!())
}

fn attempt_id(value: &str) -> AttemptId {
    AttemptId::new(value).unwrap_or_else(|_| unreachable!())
}

fn stream_id(value: &str) -> StreamId {
    StreamId::new(value).unwrap_or_else(|_| unreachable!())
}

fn provider_profile(value: &str) -> ProviderProfileId {
    ProviderProfileId::new(value).unwrap_or_else(|_| unreachable!())
}

fn command_id(value: &str) -> CommandId {
    CommandId::new(value).unwrap_or_else(|_| unreachable!())
}

fn external_ref(namespace: &str, kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!())
}

fn content_id(value: u8) -> ContentId {
    ContentId::from_blake3_hex(format!("{value:02x}").repeat(32)).unwrap_or_else(|_| unreachable!())
}

fn alias(value: &str) -> Alias {
    Alias::new(value).unwrap_or_else(|_| unreachable!())
}

fn all_views() -> BTreeSet<ViewKind> {
    [
        ViewKind::StateSummary,
        ViewKind::RunList,
        ViewKind::RunDetail,
        ViewKind::AttemptDetail,
        ViewKind::EventTail,
        ViewKind::LogRange,
    ]
    .into_iter()
    .collect()
}

fn all_fields() -> BTreeSet<ViewField> {
    [
        ViewField::ProviderProfile,
        ViewField::ExternalReference,
        ViewField::Products,
        ViewField::Metrics,
        ViewField::CommandId,
        ViewField::LogBytes,
    ]
    .into_iter()
    .collect()
}

fn non_claims() -> Vec<String> {
    OBSERVATION_NON_CLAIMS
        .iter()
        .map(|value| String::from(*value))
        .collect()
}

fn fixture_profile() -> ObservationProfile {
    ObservationProfile {
        schema_version: crate::profile::OBSERVATION_PROFILE_SCHEMA_VERSION,
        profile_id: String::from("observation-v1"),
        views: all_views(),
        bounds: ObservationBounds {
            max_page_items: 2,
            max_attempts_per_run: 1,
            max_products_per_attempt: 1,
            max_metrics_per_attempt: 1,
            max_event_records: 4,
            max_log_bytes: 4,
            max_log_window_bytes: 64,
            max_log_segments: 2,
            max_text_bytes: 256,
            max_encoded_bytes: 65_536,
        },
        redaction: RedactionPolicy {
            visible_fields: all_fields(),
        },
        non_claims: non_claims(),
    }
}

fn redacted_profile() -> ObservationProfile {
    let mut profile = fixture_profile();
    profile.redaction.visible_fields.clear();
    profile
}

fn product(value: u8, attempt: &AttemptId, reference: Option<ExternalRef>) -> Product {
    Product {
        identity: content_id(value),
        producing_attempt: attempt.clone(),
        provider_reference: reference,
    }
}

fn metric(name: &str, value: i64, attempt: &AttemptId) -> Metric {
    Metric {
        name: alias(name),
        value,
        producing_attempt: attempt.clone(),
    }
}

fn fixture_state() -> State {
    let run_a = run_id("run-a");
    let run_b = run_id("run-b");
    let run_c = run_id("run-c");
    let attempt_a1 = attempt_id("attempt-a1");
    let attempt_a2 = attempt_id("attempt-a2");
    let attempt_b1 = attempt_id("attempt-b1");
    let attempt_c1 = attempt_id("attempt-c1");
    let mut runs = BTreeMap::new();
    runs.insert(
        run_a.clone(),
        RunState {
            active_attempt: attempt_a2.clone(),
            outcome: Outcome::Pending,
        },
    );
    runs.insert(
        run_b.clone(),
        RunState {
            active_attempt: attempt_b1.clone(),
            outcome: Outcome::Unknown,
        },
    );
    runs.insert(
        run_c.clone(),
        RunState {
            active_attempt: attempt_c1.clone(),
            outcome: Outcome::Success,
        },
    );
    let mut attempts = BTreeMap::new();
    attempts.insert(
        attempt_a1.clone(),
        AttemptState {
            run_id: run_a.clone(),
            provider_profile: provider_profile("provider-one"),
            outcome: Outcome::Success,
            external_reference: Some(external_ref("provider", "attempt", "attempt-a1")),
            products: vec![
                product(
                    1,
                    &attempt_a1,
                    Some(external_ref("provider", "product", "one")),
                ),
                product(2, &attempt_a1, None),
            ],
            metrics: vec![
                metric("metric-one", 1, &attempt_a1),
                metric("metric-two", -2, &attempt_a1),
            ],
        },
    );
    attempts.insert(
        attempt_a2,
        AttemptState {
            run_id: run_a,
            provider_profile: provider_profile("provider-two"),
            outcome: Outcome::Pending,
            external_reference: None,
            products: Vec::new(),
            metrics: Vec::new(),
        },
    );
    attempts.insert(
        attempt_b1,
        AttemptState {
            run_id: run_b,
            provider_profile: provider_profile("provider-one"),
            outcome: Outcome::Unknown,
            external_reference: Some(external_ref("provider", "attempt", "attempt-b1")),
            products: Vec::new(),
            metrics: Vec::new(),
        },
    );
    attempts.insert(
        attempt_c1,
        AttemptState {
            run_id: run_c,
            provider_profile: provider_profile("provider-three"),
            outcome: Outcome::Success,
            external_reference: None,
            products: Vec::new(),
            metrics: Vec::new(),
        },
    );
    let mut holds = BTreeSet::new();
    holds.insert(external_ref("kiln", "task", "task-held"));
    State {
        revision: 3,
        runs,
        attempts,
        holds,
        seen_commands: BTreeSet::new(),
    }
}

fn journal_record(revision: u64, event: Event) -> JournalRecord {
    JournalRecord {
        schema_version: 1,
        stream: external_ref("kiln", "journal", "journal-one"),
        expected_revision: revision,
        event_identity: external_ref("kiln", "event", &format!("event-{revision}")),
        event,
    }
}

fn fixture_events() -> Vec<JournalRecord> {
    let attempt_a1 = attempt_id("attempt-a1");
    vec![
        journal_record(
            0,
            Event::RunStarted {
                command_id: command_id("command-1"),
                run_id: run_id("run-a"),
                attempt_id: attempt_a1.clone(),
                provider_profile: provider_profile("provider-one"),
            },
        ),
        journal_record(
            1,
            Event::AttemptCompleted {
                command_id: command_id("command-2"),
                attempt_id: attempt_a1.clone(),
                outcome: Outcome::Success,
                external_reference: Some(external_ref("provider", "attempt", "attempt-a1")),
                products: vec![product(
                    1,
                    &attempt_a1,
                    Some(external_ref("provider", "product", "one")),
                )],
                metrics: vec![metric("metric-one", 1, &attempt_a1)],
            },
        ),
        journal_record(
            2,
            Event::HoldChanged {
                command_id: command_id("command-3"),
                scope: external_ref("kiln", "task", "task-held"),
                held: true,
            },
        ),
    ]
}

fn fixture_log_window() -> ObservationWindow {
    ObservationWindow::log(
        LogStreamMeta {
            stream_id: stream_id("stream-one"),
            attempt_id: attempt_id("attempt-a1"),
            segments: Vec::new(),
            retained_start: 2,
            retained_end: 8,
            complete: false,
            truncated: false,
        },
        b"cdefgh".to_vec(),
    )
}

fn complete_log_window() -> ObservationWindow {
    ObservationWindow::log(
        LogStreamMeta {
            stream_id: stream_id("stream-one"),
            attempt_id: attempt_id("attempt-a1"),
            segments: Vec::new(),
            retained_start: 0,
            retained_end: 3,
            complete: true,
            truncated: false,
        },
        b"abc".to_vec(),
    )
}

fn request(view: ViewRequest) -> ObservationRequest {
    ObservationRequest {
        state_revision: 3,
        view,
    }
}

fn project(
    profile: &ObservationProfile,
    state: &State,
    request: &ObservationRequest,
    window: &ObservationWindow,
) -> ObservationView {
    crate::project_observation(profile, state, request, window).unwrap_or_else(|_| unreachable!())
}

fn assert_error<T>(result: CoreResult<T>, code: &str) {
    match result {
        Ok(_) => unreachable!(),
        Err(error) => assert_eq!(error.code, code),
    }
}

fn state_summary(view: &ObservationView) -> &StateSummaryView {
    match &view.body {
        ViewBody::StateSummary(view) => view,
        _ => unreachable!(),
    }
}

fn run_list(view: &ObservationView) -> &RunListView {
    match &view.body {
        ViewBody::RunList(view) => view,
        _ => unreachable!(),
    }
}

fn run_detail(view: &ObservationView) -> &RunDetailView {
    match &view.body {
        ViewBody::RunDetail(view) => view,
        _ => unreachable!(),
    }
}

fn attempt_detail(view: &ObservationView) -> &AttemptDetailView {
    match &view.body {
        ViewBody::AttemptDetail(view) => view,
        _ => unreachable!(),
    }
}

fn event_tail(view: &ObservationView) -> &EventTailView {
    match &view.body {
        ViewBody::EventTail(view) => view,
        _ => unreachable!(),
    }
}

fn log_range(view: &ObservationView) -> &LogRangeView {
    match &view.body {
        ViewBody::LogRange(view) => view,
        _ => unreachable!(),
    }
}

fn not_found_target(view: &ObservationView) -> &NotFoundTarget {
    match &view.body {
        ViewBody::NotFound(view) => &view.target,
        _ => unreachable!(),
    }
}

fn outcome_count(counts: &[OutcomeCount], outcome: Outcome) -> u64 {
    counts
        .iter()
        .find(|count| count.outcome == outcome)
        .map_or(0, |count| count.count)
}

// r[verify kiln.observe.projection.scenario.equal]
// r[verify kiln.observe.projection.scenario.read_only]
#[test]
fn state_summary_is_deterministic_and_read_only() {
    let profile = fixture_profile();
    let state = fixture_state();
    let original = state.clone();
    let request = request(ViewRequest::StateSummary);
    let first = project(&profile, &state, &request, &ObservationWindow::empty());
    let second = project(&profile, &state, &request, &ObservationWindow::empty());
    assert_eq!(first, second);
    assert_eq!(state, original);
    assert_eq!(first.identity.as_str().len(), 64);
    let summary = state_summary(&first);
    assert_eq!(summary.run_count, 3);
    assert_eq!(summary.attempt_count, 4);
    assert_eq!(summary.hold_count, 1);
    assert_eq!(outcome_count(&summary.run_outcomes, Outcome::Pending), 1);
    assert_eq!(outcome_count(&summary.run_outcomes, Outcome::Unknown), 1);
    assert_eq!(outcome_count(&summary.run_outcomes, Outcome::Success), 1);
    assert_eq!(
        outcome_count(&summary.attempt_outcomes, Outcome::Success),
        2
    );
    assert_eq!(
        outcome_count(&summary.attempt_outcomes, Outcome::Pending),
        1
    );
    assert_eq!(
        outcome_count(&summary.attempt_outcomes, Outcome::Unknown),
        1
    );
}

// r[verify kiln.observe.safety.scenario.page_bound]
#[test]
fn run_list_caps_page_and_reports_more() {
    let profile = fixture_profile();
    let state = fixture_state();
    let first = project(
        &profile,
        &state,
        &request(ViewRequest::RunList {
            after: None,
            limit: 10,
        }),
        &ObservationWindow::empty(),
    );
    let first_page = run_list(&first);
    assert_eq!(first_page.runs.len(), 2);
    assert!(first_page.page.capped);
    assert!(first_page.page.has_more);
    assert_eq!(first_page.page.effective_limit, 2);
    assert_eq!(first_page.page.returned, 2);
    assert_eq!(
        first_page.page.next.as_ref().map(RunId::as_str),
        Some("run-b")
    );
    let second = project(
        &profile,
        &state,
        &request(ViewRequest::RunList {
            after: first_page.page.next.clone(),
            limit: 2,
        }),
        &ObservationWindow::empty(),
    );
    let second_page = run_list(&second);
    assert_eq!(second_page.runs.len(), 1);
    assert!(!second_page.page.capped);
    assert!(!second_page.page.has_more);
    assert!(second_page.page.next.is_none());
    let missing = project(
        &profile,
        &state,
        &request(ViewRequest::RunList {
            after: Some(run_id("run-missing")),
            limit: 2,
        }),
        &ObservationWindow::empty(),
    );
    assert!(matches!(not_found_target(&missing), NotFoundTarget::Run(_)));
}

// r[verify kiln.observe.safety.scenario.unknown_target]
#[test]
fn run_detail_bounds_attempts() {
    let profile = fixture_profile();
    let state = fixture_state();
    let view = project(
        &profile,
        &state,
        &request(ViewRequest::RunDetail {
            run_id: run_id("run-a"),
        }),
        &ObservationWindow::empty(),
    );
    let detail = run_detail(&view);
    assert_eq!(detail.run.run_id.as_str(), "run-a");
    assert_eq!(detail.run.attempt_count, 2);
    assert_eq!(detail.attempts.len(), 1);
    assert_eq!(detail.attempts_truncated, 1);
    let missing = project(
        &profile,
        &state,
        &request(ViewRequest::RunDetail {
            run_id: run_id("run-missing"),
        }),
        &ObservationWindow::empty(),
    );
    assert!(matches!(not_found_target(&missing), NotFoundTarget::Run(_)));
}

#[test]
fn attempt_detail_bounds_products_and_metrics() {
    let profile = fixture_profile();
    let state = fixture_state();
    let view = project(
        &profile,
        &state,
        &request(ViewRequest::AttemptDetail {
            attempt_id: attempt_id("attempt-a1"),
        }),
        &ObservationWindow::empty(),
    );
    let detail = attempt_detail(&view);
    assert_eq!(detail.attempt_id.as_str(), "attempt-a1");
    assert_eq!(detail.product_count, 2);
    assert_eq!(detail.metric_count, 2);
    assert_eq!(detail.products.len(), 1);
    assert_eq!(detail.metrics.len(), 1);
    assert_eq!(detail.products_truncated, 1);
    assert_eq!(detail.metrics_truncated, 1);
    assert_eq!(detail.metrics.first().map(|metric| metric.value), Some(1));
    let missing = project(
        &profile,
        &state,
        &request(ViewRequest::AttemptDetail {
            attempt_id: attempt_id("attempt-missing"),
        }),
        &ObservationWindow::empty(),
    );
    assert!(matches!(
        not_found_target(&missing),
        NotFoundTarget::Attempt(_)
    ));
}

// r[verify kiln.observe.projection.scenario.equal]
#[test]
fn event_tail_caps_and_reports_earlier_history() {
    let profile = fixture_profile();
    let state = fixture_state();
    let window = ObservationWindow::events(fixture_events());
    let first = project(
        &profile,
        &state,
        &request(ViewRequest::EventTail { limit: 2 }),
        &window,
    );
    let tail = event_tail(&first);
    assert_eq!(tail.records.len(), 2);
    assert_eq!(tail.records.first().map(|record| record.revision), Some(1));
    assert_eq!(tail.page.start_revision, 1);
    assert_eq!(tail.page.end_revision, 3);
    assert!(tail.page.has_earlier);
    assert!(!tail.page.capped);
    let second = project(
        &profile,
        &state,
        &request(ViewRequest::EventTail { limit: 10 }),
        &window,
    );
    let tail = event_tail(&second);
    assert_eq!(tail.records.len(), 3);
    assert!(tail.page.capped);
    assert!(!tail.page.has_earlier);
    assert_eq!(tail.page.start_revision, 0);
}

#[test]
fn event_tail_rejects_non_tail_gap_and_ahead_windows() {
    let profile = fixture_profile();
    let state = fixture_state();
    let request = request(ViewRequest::EventTail { limit: 2 });
    let mut gap = fixture_events();
    gap.remove(1);
    assert_error(
        crate::project_observation(&profile, &state, &request, &ObservationWindow::events(gap)),
        "event_window_gap",
    );
    let mut short = fixture_events();
    let _ = short.pop();
    assert_error(
        crate::project_observation(
            &profile,
            &state,
            &request,
            &ObservationWindow::events(short),
        ),
        "event_window_not_tail",
    );
    let mut ahead = fixture_events();
    let _ = ahead.pop();
    ahead.push(journal_record(
        3,
        Event::CancellationRequested {
            command_id: command_id("command-4"),
            attempt_id: attempt_id("attempt-a1"),
        },
    ));
    assert_error(
        crate::project_observation(
            &profile,
            &state,
            &request,
            &ObservationWindow::events(ahead),
        ),
        "event_window_ahead",
    );
}

// r[verify kiln.observe.safety.scenario.cursor_window]
#[test]
fn log_range_reports_retained_gap_and_bounds() {
    let profile = fixture_profile();
    let state = fixture_state();
    let window = fixture_log_window();
    let gap = project(
        &profile,
        &state,
        &request(ViewRequest::LogRange {
            stream_id: stream_id("stream-one"),
            cursor: 0,
            max_bytes: 2,
        }),
        &window,
    );
    let range = log_range(&gap);
    assert_eq!(range.retention, RetentionView::Gap { retained_start: 2 });
    assert!(range.bytes.is_none());
    assert_eq!(range.returned_bytes, 0);
    let valid = project(
        &profile,
        &state,
        &request(ViewRequest::LogRange {
            stream_id: stream_id("stream-one"),
            cursor: 3,
            max_bytes: 2,
        }),
        &window,
    );
    let range = log_range(&valid);
    assert_eq!(range.retention, RetentionView::Valid);
    assert_eq!(range.bytes.as_deref(), Some(b"de".as_slice()));
    assert_eq!(range.returned_bytes, 2);
    assert!(range.has_more);
    let capped = project(
        &profile,
        &state,
        &request(ViewRequest::LogRange {
            stream_id: stream_id("stream-one"),
            cursor: 3,
            max_bytes: 10,
        }),
        &window,
    );
    let range = log_range(&capped);
    assert!(range.capped);
    assert_eq!(range.effective_max_bytes, 4);
    assert_eq!(range.bytes.as_deref(), Some(b"defg".as_slice()));
    assert_error(
        crate::project_observation(
            &profile,
            &state,
            &request(ViewRequest::LogRange {
                stream_id: stream_id("stream-one"),
                cursor: 9,
                max_bytes: 2,
            }),
            &window,
        ),
        "cursor_ahead",
    );
    assert_error(
        crate::project_observation(
            &profile,
            &state,
            &request(ViewRequest::LogRange {
                stream_id: stream_id("stream-one"),
                cursor: 3,
                max_bytes: 0,
            }),
            &window,
        ),
        "invalid_max_bytes",
    );
    let missing = project(
        &profile,
        &state,
        &request(ViewRequest::LogRange {
            stream_id: stream_id("stream-missing"),
            cursor: 0,
            max_bytes: 2,
        }),
        &ObservationWindow::empty(),
    );
    assert!(matches!(
        not_found_target(&missing),
        NotFoundTarget::Stream(_)
    ));
    let segmented = ObservationWindow::log(
        LogStreamMeta {
            stream_id: stream_id("stream-one"),
            attempt_id: attempt_id("attempt-a1"),
            segments: vec![
                LogSegmentMeta {
                    identity: content_id(1),
                    start_offset: 0,
                    byte_len: 1,
                },
                LogSegmentMeta {
                    identity: content_id(2),
                    start_offset: 1,
                    byte_len: 1,
                },
                LogSegmentMeta {
                    identity: content_id(3),
                    start_offset: 2,
                    byte_len: 1,
                },
            ],
            retained_start: 0,
            retained_end: 3,
            complete: false,
            truncated: false,
        },
        b"abc".to_vec(),
    );
    assert_error(
        crate::project_observation(
            &profile,
            &state,
            &request(ViewRequest::LogRange {
                stream_id: stream_id("stream-one"),
                cursor: 0,
                max_bytes: 2,
            }),
            &segmented,
        ),
        "log_segment_bound",
    );
}

#[test]
fn complete_log_cursor_reports_empty_terminal_window() {
    let profile = fixture_profile();
    let state = fixture_state();
    let view = project(
        &profile,
        &state,
        &request(ViewRequest::LogRange {
            stream_id: stream_id("stream-one"),
            cursor: 3,
            max_bytes: 2,
        }),
        &complete_log_window(),
    );
    let range = log_range(&view);
    assert_eq!(range.retention, RetentionView::Valid);
    assert!(range.stream_complete);
    assert!(!range.has_more);
    assert_eq!(range.bytes.as_deref(), Some(b"".as_slice()));
}

// r[verify kiln.observe.profile.scenario.unknown_view]
// r[verify kiln.observe.identity.scenario.unknown_state]
#[test]
fn unknown_state_revision_and_view_are_rejected() {
    let profile = fixture_profile();
    let state = fixture_state();
    assert_error(
        crate::project_observation(
            &profile,
            &state,
            &ObservationRequest {
                state_revision: 4,
                view: ViewRequest::StateSummary,
            },
            &ObservationWindow::empty(),
        ),
        "unknown_state_revision",
    );
    let mut narrowed = profile.clone();
    narrowed.views.remove(&ViewKind::LogRange);
    assert_error(
        crate::project_observation(
            &narrowed,
            &state,
            &request(ViewRequest::LogRange {
                stream_id: stream_id("stream-one"),
                cursor: 3,
                max_bytes: 2,
            }),
            &fixture_log_window(),
        ),
        "view_not_admitted",
    );
}

// r[verify kiln.observe.safety.scenario.redaction]
#[test]
fn redaction_omits_fields_and_reports_them() {
    let profile = redacted_profile();
    let state = fixture_state();
    let attempt = project(
        &profile,
        &state,
        &request(ViewRequest::AttemptDetail {
            attempt_id: attempt_id("attempt-a1"),
        }),
        &ObservationWindow::empty(),
    );
    let detail = attempt_detail(&attempt);
    assert!(detail.provider_profile.is_none());
    assert!(detail.external_reference.is_none());
    assert!(detail.products.is_empty());
    assert!(detail.metrics.is_empty());
    for field in [
        ViewField::ProviderProfile,
        ViewField::ExternalReference,
        ViewField::Products,
        ViewField::Metrics,
    ] {
        assert!(attempt.redacted_fields.contains(&field));
    }
    let event = project(
        &profile,
        &state,
        &request(ViewRequest::EventTail { limit: 1 }),
        &ObservationWindow::events(fixture_events()),
    );
    let record = event_tail(&event)
        .records
        .first()
        .unwrap_or_else(|| unreachable!());
    assert!(record.stream.is_none());
    assert!(record.event_identity.is_none());
    assert!(record.command_id.is_none());
    for field in [ViewField::ExternalReference, ViewField::CommandId] {
        assert!(event.redacted_fields.contains(&field));
    }
    let log = project(
        &profile,
        &state,
        &request(ViewRequest::LogRange {
            stream_id: stream_id("stream-one"),
            cursor: 3,
            max_bytes: 2,
        }),
        &fixture_log_window(),
    );
    assert!(log_range(&log).bytes.is_none());
    assert!(log.redacted_fields.contains(&ViewField::LogBytes));
}

// r[verify kiln.observe.identity.scenario.rebuild]
#[test]
fn identity_binds_request_state_window_and_profile() {
    let profile = fixture_profile();
    let state = fixture_state();
    let first_request = request(ViewRequest::LogRange {
        stream_id: stream_id("stream-one"),
        cursor: 3,
        max_bytes: 2,
    });
    let window = fixture_log_window();
    let first = project(&profile, &state, &first_request, &window);
    let second = project(&profile, &state, &first_request, &window);
    assert_eq!(first.identity, second.identity);
    let changed_request = request(ViewRequest::LogRange {
        stream_id: stream_id("stream-one"),
        cursor: 3,
        max_bytes: 3,
    });
    let changed = project(&profile, &state, &changed_request, &window);
    assert_ne!(first.identity, changed.identity);
    let mut changed_window = fixture_log_window();
    if let Some(log) = &mut changed_window.log {
        log.retained_bytes = b"cdefgi".to_vec();
    }
    let changed = project(&profile, &state, &first_request, &changed_window);
    assert_ne!(first.identity, changed.identity);
    let mut changed_profile = profile.clone();
    changed_profile
        .redaction
        .visible_fields
        .remove(&ViewField::LogBytes);
    let changed = project(&changed_profile, &state, &first_request, &window);
    assert_ne!(first.identity, changed.identity);
}

#[test]
fn unexpected_window_fact_is_rejected() {
    let profile = fixture_profile();
    let state = fixture_state();
    assert_error(
        crate::project_observation(
            &profile,
            &state,
            &request(ViewRequest::StateSummary),
            &ObservationWindow::events(fixture_events()),
        ),
        "unexpected_window_fact",
    );
    assert_error(
        crate::project_observation(
            &profile,
            &state,
            &request(ViewRequest::EventTail { limit: 2 }),
            &fixture_log_window(),
        ),
        "unexpected_window_fact",
    );
    assert_error(
        crate::project_observation(
            &profile,
            &state,
            &request(ViewRequest::LogRange {
                stream_id: stream_id("stream-one"),
                cursor: 3,
                max_bytes: 2,
            }),
            &ObservationWindow::events(fixture_events()),
        ),
        "unexpected_window_fact",
    );
}

#[test]
fn encoded_bound_is_enforced() {
    let mut profile = fixture_profile();
    profile.bounds.max_encoded_bytes = 1;
    let state = fixture_state();
    assert_error(
        crate::project_observation(
            &profile,
            &state,
            &request(ViewRequest::StateSummary),
            &ObservationWindow::empty(),
        ),
        "view_bound_exceeded",
    );
}

// r[verify kiln.observe.nonclaims]
// r[verify kiln.observe.nonclaims.scenario.scope]
#[test]
fn profile_admission_requires_declared_non_claims() {
    let mut profile = fixture_profile();
    profile.non_claims.clear();
    assert_error(profile.admit(), "missing_non_claims");
    let mut profile = fixture_profile();
    let _ = profile.non_claims.remove(0);
    assert_error(profile.admit(), "missing_required_non_claim");
    let mut profile = fixture_profile();
    profile.bounds.max_page_items = 0;
    assert_error(profile.admit(), "zero_bound");
    let mut profile = fixture_profile();
    profile.schema_version = 2;
    assert_error(profile.admit(), "unsupported_profile_version");
}

// r[verify kiln.observe.conformance.scenario.negative_missing]
#[test]
fn conformance_manifest_requires_all_cases() {
    let complete = ConformanceManifest {
        positive: REQUIRED_POSITIVE_CASES
            .iter()
            .map(|case| String::from(*case))
            .collect(),
        negative: REQUIRED_NEGATIVE_CASES
            .iter()
            .map(|case| String::from(*case))
            .collect(),
    };
    assert!(admit_conformance(&complete).is_ok());
    let mut missing = complete.clone();
    missing
        .negative
        .retain(|case| case != "missing_negative_case");
    assert_error(admit_conformance(&missing), "missing_negative_case");
    let mut duplicate = complete;
    duplicate.positive.push(String::from("state_summary"));
    assert_error(admit_conformance(&duplicate), "duplicate_case");
}

#[derive(Deserialize)]
struct ProfileDocument {
    profile: ObservationProfile,
}

// r[verify kiln.observe.profile.scenario.match]
#[test]
fn nickel_profile_fixture_matches_rust_profile() {
    let document: ProfileDocument = serde_json::from_str(include_str!(
        "../../../fixtures/observation/profile-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(document.profile, fixture_profile());
    assert!(document.profile.admit().is_ok());
}

#[test]
fn profile_identity_is_stable_and_binds_non_claims() {
    let first = fixture_profile();
    let second = fixture_profile();
    assert_eq!(first.identity().ok(), second.identity().ok());
    let mut changed = fixture_profile();
    changed.non_claims.push(String::from("changed"));
    assert_ne!(first.identity().ok(), changed.identity().ok());
}
