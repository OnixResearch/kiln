use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use kiln_core::domain::{Event, Outcome, State};
use kiln_core::operations::JournalRecord;
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CoreResult, Diagnostic, ExternalRef, ProviderProfileId, RunId,
    StreamId,
};

use crate::identity;
use crate::model::{
    AttemptDetailView, AttemptSummaryView, EventPageInfo, EventProjection, EventRecordView,
    EventTailView, LogRangeView, MetricView, NotFoundTarget, NotFoundView,
    OBSERVATION_VIEW_SCHEMA_VERSION, ObservationRequest, ObservationView, ObservationWindow,
    OutcomeCount, PageInfo, ProductView, RetentionView, RunDetailView, RunListView, RunSummaryView,
    StateSummaryView, ViewBody, ViewRequest,
};
use crate::profile::{ObservationProfile, ViewField};
use crate::review::{
    ReviewWindow, admit_review_window, project_review_friction, project_suggestion_anchors,
};

const OUTCOME_ORDER: [Outcome; 8] = [
    Outcome::Pending,
    Outcome::Success,
    Outcome::DomainFailure,
    Outcome::Denied,
    Outcome::Cancelled,
    Outcome::TimedOut,
    Outcome::ProviderError,
    Outcome::Unknown,
];

// r[impl kiln.observe.projection]
// r[impl kiln.observe.identity]
pub fn project_observation(
    profile: &ObservationProfile,
    state: &State,
    request: &ObservationRequest,
    window: &ObservationWindow,
) -> CoreResult<ObservationView> {
    profile.admit()?;
    if !profile.admits_view(request.view.kind()) {
        return Err(Diagnostic::new(
            "view_not_admitted",
            "observation.view",
            "view kind is not declared by the observation profile",
        ));
    }
    // r[impl kiln.observe.identity.scenario.unknown_state]
    if request.state_revision != state.revision {
        return Err(Diagnostic::new(
            "unknown_state_revision",
            "observation.state_revision",
            "requested state revision does not match admitted state",
        ));
    }
    let profile_identity = profile.identity()?;
    let mut redacted = BTreeSet::new();
    let body = match &request.view {
        ViewRequest::StateSummary => {
            require_no_window(window)?;
            ViewBody::StateSummary(project_state_summary(state))
        }
        ViewRequest::RunList { after, limit } => {
            require_no_window(window)?;
            project_run_list(profile, state, after.as_ref(), *limit)?
        }
        ViewRequest::RunDetail { run_id } => {
            require_no_window(window)?;
            project_run_detail(profile, state, run_id, &mut redacted)?
        }
        ViewRequest::AttemptDetail { attempt_id } => {
            require_no_window(window)?;
            project_attempt_detail(profile, state, attempt_id, &mut redacted)?
        }
        ViewRequest::EventTail { limit } => {
            if window.log.is_some() || window.review.is_some() {
                return Err(unexpected_window_fact());
            }
            project_event_tail(profile, state, window, *limit, &mut redacted)?
        }
        ViewRequest::LogRange {
            stream_id,
            cursor,
            max_bytes,
        } => {
            if window.events.is_some() || window.review.is_some() {
                return Err(unexpected_window_fact());
            }
            project_log_range(
                profile,
                window,
                stream_id,
                *cursor,
                *max_bytes,
                &mut redacted,
            )?
        }
        // r[impl kiln.observe.review.parity.scenario.equal]
        ViewRequest::ReviewFriction => {
            let review = require_review_window(profile, window)?;
            ViewBody::ReviewFriction(project_review_friction(review))
        }
        ViewRequest::SuggestionAnchors { cursor, limit } => {
            let review = require_review_window(profile, window)?;
            ViewBody::SuggestionAnchors(project_suggestion_anchors(
                profile,
                review,
                *cursor,
                *limit,
                &mut redacted,
            )?)
        }
    };
    let body_bytes = identity::encode_view_body(&body);
    let max_encoded = bound(profile.bounds.max_encoded_bytes, "bounds.max_encoded_bytes")?;
    if body_bytes.len() > max_encoded {
        return Err(Diagnostic::new(
            "view_bound_exceeded",
            "observation.body",
            "canonical view body exceeds its named byte bound",
        ));
    }
    let view_identity =
        identity::view_identity(&profile_identity, request, window, &redacted, &body_bytes)?;
    Ok(ObservationView {
        schema_version: OBSERVATION_VIEW_SCHEMA_VERSION,
        identity: view_identity,
        state_revision: request.state_revision,
        redacted_fields: redacted,
        body,
    })
}

fn require_no_window(window: &ObservationWindow) -> CoreResult<()> {
    if window.events.is_some() || window.log.is_some() || window.review.is_some() {
        return Err(unexpected_window_fact());
    }
    Ok(())
}

// r[impl kiln.observe.review.window]
fn require_review_window<'a>(
    profile: &ObservationProfile,
    window: &'a ObservationWindow,
) -> CoreResult<&'a ReviewWindow> {
    if window.events.is_some() || window.log.is_some() {
        return Err(unexpected_window_fact());
    }
    let review = window.review.as_ref().ok_or_else(|| {
        Diagnostic::new(
            "missing_review_window",
            "observation.window.review",
            "review views require an admitted review window",
        )
    })?;
    admit_review_window(profile, review)?;
    Ok(review)
}

fn unexpected_window_fact() -> Diagnostic {
    Diagnostic::new(
        "unexpected_window_fact",
        "observation.window",
        "the selected view does not admit this window fact",
    )
}

fn not_found(target: NotFoundTarget) -> ViewBody {
    ViewBody::NotFound(NotFoundView::new(target))
}

fn bound(value: u64, field: &'static str) -> CoreResult<usize> {
    usize::try_from(value).map_err(|_| {
        Diagnostic::new(
            "bound_not_representable",
            field,
            "profile bound cannot be represented on this target",
        )
    })
}

fn count(value: usize) -> u64 {
    value as u64
}

fn outcome_counts(outcomes: impl Iterator<Item = Outcome>) -> Vec<OutcomeCount> {
    let mut counts: BTreeMap<Outcome, u64> = BTreeMap::new();
    for outcome in outcomes {
        *counts.entry(outcome).or_insert(0) += 1;
    }
    OUTCOME_ORDER
        .iter()
        .filter_map(|outcome| {
            counts.get(outcome).map(|value| OutcomeCount {
                outcome: *outcome,
                count: *value,
            })
        })
        .collect()
}

fn project_state_summary(state: &State) -> StateSummaryView {
    StateSummaryView {
        run_count: count(state.runs.len()),
        attempt_count: count(state.attempts.len()),
        hold_count: count(state.holds.len()),
        run_outcomes: outcome_counts(state.runs.values().map(|run| run.outcome)),
        attempt_outcomes: outcome_counts(state.attempts.values().map(|attempt| attempt.outcome)),
    }
}

fn attempt_counts(state: &State) -> BTreeMap<&RunId, u64> {
    let mut counts = BTreeMap::new();
    for attempt in state.attempts.values() {
        *counts.entry(&attempt.run_id).or_insert(0) += 1;
    }
    counts
}

// r[impl kiln.observe.safety.scenario.page_bound]
fn project_run_list(
    profile: &ObservationProfile,
    state: &State,
    after: Option<&RunId>,
    requested_limit: u64,
) -> CoreResult<ViewBody> {
    if requested_limit == 0 {
        return Err(invalid_page_limit("run_list.limit"));
    }
    let max_page = bound(profile.bounds.max_page_items, "bounds.max_page_items")?;
    let effective = usize::try_from(requested_limit)
        .unwrap_or(usize::MAX)
        .min(max_page);
    let capped = requested_limit > profile.bounds.max_page_items;
    if let Some(after) = after
        && !state.runs.contains_key(after)
    {
        return Ok(not_found(NotFoundTarget::Run(after.clone())));
    }
    let counts = attempt_counts(state);
    let mut runs = Vec::new();
    let mut has_more = false;
    let iter = state
        .runs
        .iter()
        .skip_while(|(run_id, _)| after.is_some_and(|after| *run_id <= after));
    for (run_id, run) in iter {
        if runs.len() == effective {
            has_more = true;
            break;
        }
        let attempt_count = counts.get(run_id).copied().unwrap_or(0);
        runs.push(RunSummaryView {
            run_id: run_id.clone(),
            active_attempt: run.active_attempt.clone(),
            outcome: run.outcome,
            attempt_count,
        });
    }
    let next = if has_more {
        runs.last().map(|run| run.run_id.clone())
    } else {
        None
    };
    let returned = count(runs.len());
    Ok(ViewBody::RunList(RunListView {
        runs,
        page: PageInfo {
            requested_limit,
            effective_limit: count(effective),
            returned,
            capped,
            has_more,
            next,
        },
    }))
}

// r[impl kiln.observe.safety.scenario.unknown_target]
fn project_run_detail(
    profile: &ObservationProfile,
    state: &State,
    run_id: &RunId,
    redacted: &mut BTreeSet<ViewField>,
) -> CoreResult<ViewBody> {
    let Some(run) = state.runs.get(run_id) else {
        return Ok(not_found(NotFoundTarget::Run(run_id.clone())));
    };
    let max_attempts = bound(
        profile.bounds.max_attempts_per_run,
        "bounds.max_attempts_per_run",
    )?;
    let total = state
        .attempts
        .values()
        .filter(|attempt| attempt.run_id == *run_id)
        .count();
    let mut attempts = Vec::new();
    for (attempt_id, attempt) in state
        .attempts
        .iter()
        .filter(|(_, attempt)| attempt.run_id == *run_id)
    {
        if attempts.len() == max_attempts {
            break;
        }
        attempts.push(AttemptSummaryView {
            attempt_id: attempt_id.clone(),
            provider_profile: project_provider_profile(
                profile,
                &attempt.provider_profile,
                redacted,
            ),
            outcome: attempt.outcome,
            product_count: count(attempt.products.len()),
            metric_count: count(attempt.metrics.len()),
        });
    }
    let attempts_truncated = count(total.saturating_sub(attempts.len()));
    Ok(ViewBody::RunDetail(RunDetailView {
        run: RunSummaryView {
            run_id: run_id.clone(),
            active_attempt: run.active_attempt.clone(),
            outcome: run.outcome,
            attempt_count: count(total),
        },
        attempts,
        attempts_truncated,
    }))
}

// r[impl kiln.observe.safety.scenario.redaction]
fn project_attempt_detail(
    profile: &ObservationProfile,
    state: &State,
    attempt_id: &AttemptId,
    redacted: &mut BTreeSet<ViewField>,
) -> CoreResult<ViewBody> {
    let Some(attempt) = state.attempts.get(attempt_id) else {
        return Ok(not_found(NotFoundTarget::Attempt(attempt_id.clone())));
    };
    let max_text = bound(profile.bounds.max_text_bytes, "bounds.max_text_bytes")?;
    let max_products = bound(
        profile.bounds.max_products_per_attempt,
        "bounds.max_products_per_attempt",
    )?;
    let max_metrics = bound(
        profile.bounds.max_metrics_per_attempt,
        "bounds.max_metrics_per_attempt",
    )?;
    let provider_profile = project_provider_profile(profile, &attempt.provider_profile, redacted);
    let external_reference =
        project_external_reference(profile, attempt.external_reference.as_ref(), redacted);
    if let Some(reference) = &attempt.external_reference {
        validate_external_ref(reference, max_text, "attempt.external_reference")?;
    }
    let product_count = count(attempt.products.len());
    let metric_count = count(attempt.metrics.len());
    let products_visible = profile.field_visible(ViewField::Products);
    let mut products = Vec::new();
    if products_visible {
        for product in &attempt.products {
            if products.len() == max_products {
                break;
            }
            if product.producing_attempt != *attempt_id {
                return Err(Diagnostic::new(
                    "orphan_product",
                    "attempt.products.producing_attempt",
                    "product belongs to another attempt",
                ));
            }
            if let Some(reference) = &product.provider_reference {
                validate_external_ref(reference, max_text, "attempt.products.provider_reference")?;
            }
            products.push(ProductView {
                identity: product.identity.clone(),
                producing_attempt: product.producing_attempt.clone(),
                provider_reference: project_external_reference(
                    profile,
                    product.provider_reference.as_ref(),
                    redacted,
                ),
            });
        }
    } else {
        redacted.insert(ViewField::Products);
    }
    let metrics_visible = profile.field_visible(ViewField::Metrics);
    let mut metrics = Vec::new();
    if metrics_visible {
        for metric in &attempt.metrics {
            if metrics.len() == max_metrics {
                break;
            }
            if metric.producing_attempt != *attempt_id {
                return Err(Diagnostic::new(
                    "orphan_metric",
                    "attempt.metrics.producing_attempt",
                    "metric belongs to another attempt",
                ));
            }
            if metric.name.as_str().len() > max_text {
                return Err(Diagnostic::new(
                    "attribute_bound_exceeded",
                    "attempt.metrics.name",
                    "metric name exceeds its named byte bound",
                ));
            }
            metrics.push(MetricView {
                name: metric.name.clone(),
                value: metric.value,
                producing_attempt: metric.producing_attempt.clone(),
            });
        }
    } else {
        redacted.insert(ViewField::Metrics);
    }
    let products_truncated = if products_visible {
        product_count.saturating_sub(count(products.len()))
    } else {
        0
    };
    let metrics_truncated = if metrics_visible {
        metric_count.saturating_sub(count(metrics.len()))
    } else {
        0
    };
    Ok(ViewBody::AttemptDetail(AttemptDetailView {
        attempt_id: attempt_id.clone(),
        run_id: attempt.run_id.clone(),
        provider_profile,
        outcome: attempt.outcome,
        external_reference,
        product_count,
        metric_count,
        products,
        metrics,
        products_truncated,
        metrics_truncated,
    }))
}

fn project_provider_profile(
    profile: &ObservationProfile,
    value: &ProviderProfileId,
    redacted: &mut BTreeSet<ViewField>,
) -> Option<ProviderProfileId> {
    if profile.field_visible(ViewField::ProviderProfile) {
        Some(value.clone())
    } else {
        redacted.insert(ViewField::ProviderProfile);
        None
    }
}

fn project_external_reference(
    profile: &ObservationProfile,
    value: Option<&ExternalRef>,
    redacted: &mut BTreeSet<ViewField>,
) -> Option<ExternalRef> {
    let value = value?;
    if profile.field_visible(ViewField::ExternalReference) {
        Some(value.clone())
    } else {
        redacted.insert(ViewField::ExternalReference);
        None
    }
}

fn invalid_page_limit(field: &'static str) -> Diagnostic {
    Diagnostic::new("invalid_page_limit", field, "page limit must be positive")
}

fn validate_external_ref(
    reference: &ExternalRef,
    max_text: usize,
    field: &'static str,
) -> CoreResult<()> {
    reference.validate()?;
    for value in [
        reference.provider_namespace.as_str(),
        reference.native_kind.as_str(),
        reference.native_id.as_str(),
    ] {
        if value.len() > max_text {
            return Err(Diagnostic::new(
                "attribute_bound_exceeded",
                field,
                "external reference text exceeds its named byte bound",
            ));
        }
    }
    Ok(())
}

fn validate_provider_profile(value: &ProviderProfileId, max_text: usize) -> CoreResult<()> {
    if value.as_str().len() > max_text {
        return Err(Diagnostic::new(
            "attribute_bound_exceeded",
            "provider_profile",
            "provider profile exceeds its named byte bound",
        ));
    }
    Ok(())
}

fn validate_event(event: &Event, max_text: usize) -> CoreResult<()> {
    match event {
        Event::RunStarted {
            provider_profile, ..
        }
        | Event::RetryStarted {
            provider_profile, ..
        } => validate_provider_profile(provider_profile, max_text),
        Event::AttemptCompleted {
            attempt_id,
            external_reference,
            products,
            metrics,
            ..
        } => {
            if let Some(reference) = external_reference {
                validate_external_ref(reference, max_text, "event.completion.external_reference")?;
            }
            for product in products {
                if product.producing_attempt != *attempt_id {
                    return Err(Diagnostic::new(
                        "orphan_product",
                        "event.completion.products.producing_attempt",
                        "product belongs to another attempt",
                    ));
                }
                if let Some(reference) = &product.provider_reference {
                    validate_external_ref(
                        reference,
                        max_text,
                        "event.completion.products.provider_reference",
                    )?;
                }
            }
            for metric in metrics {
                if metric.producing_attempt != *attempt_id {
                    return Err(Diagnostic::new(
                        "orphan_metric",
                        "event.completion.metrics.producing_attempt",
                        "metric belongs to another attempt",
                    ));
                }
                if metric.name.as_str().len() > max_text {
                    return Err(Diagnostic::new(
                        "attribute_bound_exceeded",
                        "event.completion.metrics.name",
                        "metric name exceeds its named byte bound",
                    ));
                }
            }
            Ok(())
        }
        Event::AttemptUnknown {
            external_reference, ..
        }
        | Event::AttemptReconciled {
            external_reference, ..
        } => {
            if let Some(reference) = external_reference {
                validate_external_ref(reference, max_text, "event.external_reference")?;
            }
            Ok(())
        }
        Event::HoldChanged { scope, .. } => {
            validate_external_ref(scope, max_text, "event.hold.scope")
        }
        Event::CancellationRequested { .. } => Ok(()),
    }
}

fn validate_event_window(
    state: &State,
    window: &crate::model::EventWindow,
    max_text: usize,
) -> CoreResult<()> {
    if state.revision == 0 {
        if !window.records.is_empty() {
            return Err(Diagnostic::new(
                "event_window_ahead",
                "observation.window.events",
                "event window cannot contain records beyond the admitted state",
            ));
        }
        return Ok(());
    }
    if window.records.is_empty() {
        return Err(Diagnostic::new(
            "event_window_missing_records",
            "observation.window.events",
            "event tail requires at least one record for a non-empty state",
        ));
    }
    let mut previous: Option<u64> = None;
    for record in &window.records {
        if record.schema_version != CONTRACT_VERSION_V1 {
            return Err(Diagnostic::new(
                "unsupported_history_version",
                "observation.window.events.schema_version",
                "event window version has no admitted deterministic replay path",
            ));
        }
        validate_external_ref(&record.stream, max_text, "observation.window.events.stream")?;
        validate_external_ref(
            &record.event_identity,
            max_text,
            "observation.window.events.event_identity",
        )?;
        validate_event(&record.event, max_text)?;
        if record.expected_revision >= state.revision {
            return Err(Diagnostic::new(
                "event_window_ahead",
                "observation.window.events.expected_revision",
                "event window contains a record beyond the admitted state",
            ));
        }
        if let Some(previous) = previous {
            let next = previous.checked_add(1).ok_or_else(|| {
                Diagnostic::new(
                    "event_window_range",
                    "observation.window.events.expected_revision",
                    "event window revision cannot be advanced",
                )
            })?;
            if record.expected_revision != next {
                return Err(Diagnostic::new(
                    "event_window_gap",
                    "observation.window.events.expected_revision",
                    "event window revisions are not contiguous",
                ));
            }
        }
        previous = Some(record.expected_revision);
    }
    let last = window.records.last().ok_or_else(|| {
        Diagnostic::new(
            "event_window_missing_records",
            "observation.window.events",
            "event tail requires at least one record",
        )
    })?;
    let end = last.expected_revision.checked_add(1).ok_or_else(|| {
        Diagnostic::new(
            "event_window_range",
            "observation.window.events.expected_revision",
            "event window revision cannot be advanced",
        )
    })?;
    if end != state.revision {
        return Err(Diagnostic::new(
            "event_window_not_tail",
            "observation.window.events",
            "event window does not end at the admitted state revision",
        ));
    }
    Ok(())
}

fn project_event_tail(
    profile: &ObservationProfile,
    state: &State,
    window: &ObservationWindow,
    requested_limit: u64,
    redacted: &mut BTreeSet<ViewField>,
) -> CoreResult<ViewBody> {
    if requested_limit == 0 {
        return Err(invalid_page_limit("event_tail.limit"));
    }
    let Some(event_window) = &window.events else {
        return Err(Diagnostic::new(
            "missing_event_window",
            "observation.window.events",
            "event tail requires an explicit admitted event window",
        ));
    };
    let max_records = bound(profile.bounds.max_event_records, "bounds.max_event_records")?;
    if event_window.records.len() > max_records {
        return Err(Diagnostic::new(
            "event_window_bound",
            "observation.window.events",
            "event window exceeds its named record bound",
        ));
    }
    let max_text = bound(profile.bounds.max_text_bytes, "bounds.max_text_bytes")?;
    validate_event_window(state, event_window, max_text)?;
    let effective = usize::try_from(requested_limit)
        .unwrap_or(usize::MAX)
        .min(max_records);
    let capped = requested_limit > profile.bounds.max_event_records;
    let start_index = event_window.records.len().saturating_sub(effective);
    let selected = event_window.records.get(start_index..).unwrap_or(&[]);
    let start_revision = selected
        .first()
        .map_or(state.revision, |record| record.expected_revision);
    let mut records = Vec::new();
    for record in selected {
        records.push(project_event_record(profile, record, redacted)?);
    }
    let has_earlier = start_revision > 0;
    Ok(ViewBody::EventTail(EventTailView {
        records,
        page: EventPageInfo {
            requested_limit,
            effective_limit: count(effective),
            returned: count(selected.len()),
            capped,
            has_earlier,
            start_revision,
            end_revision: state.revision,
        },
    }))
}

fn project_event_record(
    profile: &ObservationProfile,
    record: &JournalRecord,
    redacted: &mut BTreeSet<ViewField>,
) -> CoreResult<EventRecordView> {
    let stream = project_external_reference(profile, Some(&record.stream), redacted);
    let event_identity =
        project_external_reference(profile, Some(&record.event_identity), redacted);
    let command_id = if profile.field_visible(ViewField::CommandId) {
        Some(record.event.command_id().clone())
    } else {
        redacted.insert(ViewField::CommandId);
        None
    };
    Ok(EventRecordView {
        revision: record.expected_revision,
        stream,
        event_identity,
        command_id,
        event: project_event(profile, &record.event, redacted),
    })
}

fn project_event(
    profile: &ObservationProfile,
    event: &Event,
    redacted: &mut BTreeSet<ViewField>,
) -> EventProjection {
    match event {
        Event::RunStarted {
            run_id,
            attempt_id,
            provider_profile,
            ..
        } => EventProjection::RunStarted {
            run_id: run_id.clone(),
            attempt_id: attempt_id.clone(),
            provider_profile: project_provider_profile(profile, provider_profile, redacted),
        },
        Event::AttemptCompleted {
            attempt_id,
            outcome,
            external_reference,
            products,
            metrics,
            ..
        } => EventProjection::AttemptCompleted {
            attempt_id: attempt_id.clone(),
            outcome: *outcome,
            external_reference: project_external_reference(
                profile,
                external_reference.as_ref(),
                redacted,
            ),
            product_count: count(products.len()),
            metric_count: count(metrics.len()),
        },
        Event::AttemptUnknown {
            attempt_id,
            external_reference,
            ..
        } => EventProjection::AttemptUnknown {
            attempt_id: attempt_id.clone(),
            external_reference: project_external_reference(
                profile,
                external_reference.as_ref(),
                redacted,
            ),
        },
        Event::AttemptReconciled {
            attempt_id,
            outcome,
            external_reference,
            ..
        } => EventProjection::AttemptReconciled {
            attempt_id: attempt_id.clone(),
            outcome: *outcome,
            external_reference: project_external_reference(
                profile,
                external_reference.as_ref(),
                redacted,
            ),
        },
        Event::RetryStarted {
            run_id,
            prior_attempt_id,
            next_attempt_id,
            provider_profile,
            ..
        } => EventProjection::RetryStarted {
            run_id: run_id.clone(),
            prior_attempt_id: prior_attempt_id.clone(),
            next_attempt_id: next_attempt_id.clone(),
            provider_profile: project_provider_profile(profile, provider_profile, redacted),
        },
        Event::CancellationRequested { attempt_id, .. } => EventProjection::CancellationRequested {
            attempt_id: attempt_id.clone(),
        },
        Event::HoldChanged { scope, held, .. } => EventProjection::HoldChanged {
            scope: project_external_reference(profile, Some(scope), redacted),
            held: *held,
        },
    }
}

// r[impl kiln.observe.safety]
// r[impl kiln.observe.safety.scenario.cursor_window]
fn project_log_range(
    profile: &ObservationProfile,
    window: &ObservationWindow,
    stream_id: &StreamId,
    cursor: u64,
    requested_max_bytes: u64,
    redacted: &mut BTreeSet<ViewField>,
) -> CoreResult<ViewBody> {
    let Some(log) = &window.log else {
        return Ok(not_found(NotFoundTarget::Stream(stream_id.clone())));
    };
    if log.meta.stream_id != *stream_id {
        return Err(Diagnostic::new(
            "log_stream_mismatch",
            "observation.window.log.stream_id",
            "log window belongs to another stream",
        ));
    }
    let max_segments = bound(profile.bounds.max_log_segments, "bounds.max_log_segments")?;
    if log.meta.segments.len() > max_segments {
        return Err(Diagnostic::new(
            "log_segment_bound",
            "observation.window.log.segments",
            "log window segment count exceeds its named bound",
        ));
    }
    let max_window = bound(
        profile.bounds.max_log_window_bytes,
        "bounds.max_log_window_bytes",
    )?;
    if log.retained_bytes.len() > max_window {
        return Err(Diagnostic::new(
            "log_window_bound",
            "observation.window.log.retained_bytes",
            "retained log window exceeds its named byte bound",
        ));
    }
    if log.meta.retained_start > log.meta.retained_end {
        return Err(Diagnostic::new(
            "log_window_range",
            "observation.window.log.retained_start",
            "retained log window start is beyond its end",
        ));
    }
    let retained_len = log.meta.retained_end - log.meta.retained_start;
    let retained_len_usize = usize::try_from(retained_len).map_err(|_| {
        Diagnostic::new(
            "log_window_range",
            "observation.window.log.retained_end",
            "retained log window cannot be represented on this target",
        )
    })?;
    if log.retained_bytes.len() != retained_len_usize {
        return Err(Diagnostic::new(
            "log_window_mismatch",
            "observation.window.log.retained_bytes",
            "retained log bytes do not match the declared retained window",
        ));
    }
    if requested_max_bytes == 0 {
        return Err(Diagnostic::new(
            "invalid_max_bytes",
            "log_range.max_bytes",
            "log range max bytes must be positive",
        ));
    }
    if cursor > log.meta.retained_end {
        return Err(Diagnostic::new(
            "cursor_ahead",
            "log_range.cursor",
            "cursor is ahead of retained log data",
        ));
    }
    let effective = requested_max_bytes.min(profile.bounds.max_log_bytes);
    let capped = requested_max_bytes > profile.bounds.max_log_bytes;
    let bytes_visible = profile.field_visible(ViewField::LogBytes);
    if !bytes_visible {
        redacted.insert(ViewField::LogBytes);
    }
    let (retention, bytes, returned_bytes, has_more, stream_complete) =
        if cursor < log.meta.retained_start {
            (
                RetentionView::Gap {
                    retained_start: log.meta.retained_start,
                },
                None,
                0,
                false,
                false,
            )
        } else {
            let available = log.meta.retained_end - cursor;
            let take = available.min(effective);
            let offset = cursor - log.meta.retained_start;
            let offset_usize = usize::try_from(offset).map_err(|_| {
                Diagnostic::new(
                    "log_window_range",
                    "log_range.cursor",
                    "log cursor offset cannot be represented on this target",
                )
            })?;
            let take_usize = usize::try_from(take).map_err(|_| {
                Diagnostic::new(
                    "log_window_range",
                    "log_range.max_bytes",
                    "log range length cannot be represented on this target",
                )
            })?;
            let end = offset_usize.checked_add(take_usize).ok_or_else(|| {
                Diagnostic::new(
                    "log_window_range",
                    "log_range.cursor",
                    "log cursor range overflowed",
                )
            })?;
            let slice = log.retained_bytes.get(offset_usize..end).ok_or_else(|| {
                Diagnostic::new(
                    "log_window_mismatch",
                    "observation.window.log.retained_bytes",
                    "log range names bytes outside the admitted retained window",
                )
            })?;
            let bytes = if bytes_visible {
                Some(slice.to_vec())
            } else {
                None
            };
            let stream_complete =
                log.meta.complete && cursor.saturating_add(take) == log.meta.retained_end;
            let has_more = take < available;
            (RetentionView::Valid, bytes, take, has_more, stream_complete)
        };
    Ok(ViewBody::LogRange(LogRangeView {
        stream_id: log.meta.stream_id.clone(),
        attempt_id: log.meta.attempt_id.clone(),
        cursor,
        retained_start: log.meta.retained_start,
        retained_end: log.meta.retained_end,
        retention,
        requested_max_bytes,
        effective_max_bytes: effective,
        returned_bytes,
        capped,
        has_more,
        stream_complete,
        stream_truncated: log.meta.truncated,
        bytes,
    }))
}
