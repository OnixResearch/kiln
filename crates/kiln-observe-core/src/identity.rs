use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use kiln_core::domain::{Event, Outcome};
use kiln_core::operations::JournalRecord;
use kiln_core::task_ops::LogStreamMeta;
use kiln_core::types::{ContentId, CoreResult, ExternalRef};

use crate::model::{
    AttemptDetailView, AttemptSummaryView, EventPageInfo, EventProjection, EventRecordView,
    EventTailView, LogRangeView, LogWindow, MetricView, NotFoundTarget, NotFoundView,
    ObservationRequest, ObservationWindow, OutcomeCount, PageInfo, ProductView, RetentionView,
    RunDetailView, RunListView, RunSummaryView, StateSummaryView, ViewBody, ViewRequest,
};
use crate::profile::{ObservationProfile, ViewField, ViewKind};

const PROFILE_IDENTITY_TAG: &[u8] = b"kiln.observe.profile.v1\0";
const VIEW_IDENTITY_TAG: &[u8] = b"kiln.observe.view.v1\0";

pub(crate) fn profile_identity(profile: &ObservationProfile) -> CoreResult<ContentId> {
    let mut output = Vec::new();
    output.extend_from_slice(PROFILE_IDENTITY_TAG);
    encode_profile(&mut output, profile);
    digest(&output)
}

pub(crate) fn view_identity(
    profile_identity: &ContentId,
    request: &ObservationRequest,
    window: &ObservationWindow,
    redacted: &BTreeSet<ViewField>,
    body_bytes: &[u8],
) -> CoreResult<ContentId> {
    let mut output = Vec::new();
    output.extend_from_slice(VIEW_IDENTITY_TAG);
    encode_content_id(&mut output, profile_identity);
    push_u64(&mut output, request.state_revision);
    encode_request(&mut output, request);
    encode_window(&mut output, window);
    encode_field_set(&mut output, redacted);
    push_bytes(&mut output, body_bytes);
    digest(&output)
}

pub(crate) fn encode_view_body(body: &ViewBody) -> Vec<u8> {
    let mut output = Vec::new();
    encode_body(&mut output, body);
    output
}

fn digest(bytes: &[u8]) -> CoreResult<ContentId> {
    let digest = blake3::hash(bytes).to_hex();
    ContentId::from_blake3_hex(digest.as_str())
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn push_i64(output: &mut Vec<u8>, value: i64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn push_bool(output: &mut Vec<u8>, value: bool) {
    output.push(u8::from(value));
}

fn push_len(output: &mut Vec<u8>, length: usize) {
    push_u64(output, length as u64);
}

fn push_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    push_len(output, bytes.len());
    output.extend_from_slice(bytes);
}

fn push_str(output: &mut Vec<u8>, value: &str) {
    push_bytes(output, value.as_bytes());
}

fn push_option<T>(output: &mut Vec<u8>, value: Option<&T>, encode: fn(&mut Vec<u8>, &T)) {
    match value {
        Some(value) => {
            output.push(1);
            encode(output, value);
        }
        None => output.push(0),
    }
}

fn encode_content_id(output: &mut Vec<u8>, value: &ContentId) {
    push_str(output, value.as_str());
}

fn encode_external_ref(output: &mut Vec<u8>, value: &ExternalRef) {
    push_u64(output, u64::from(value.contract_version));
    push_str(output, value.provider_namespace.as_str());
    push_str(output, value.native_kind.as_str());
    push_str(output, value.native_id.as_str());
}

fn encode_run_id(output: &mut Vec<u8>, value: &kiln_core::types::RunId) {
    push_str(output, value.as_str());
}

fn encode_attempt_id(output: &mut Vec<u8>, value: &kiln_core::types::AttemptId) {
    push_str(output, value.as_str());
}

fn encode_stream_id(output: &mut Vec<u8>, value: &kiln_core::types::StreamId) {
    push_str(output, value.as_str());
}

fn encode_command_id(output: &mut Vec<u8>, value: &kiln_core::types::CommandId) {
    push_str(output, value.as_str());
}

fn encode_provider_profile(output: &mut Vec<u8>, value: &kiln_core::types::ProviderProfileId) {
    push_str(output, value.as_str());
}

fn encode_alias(output: &mut Vec<u8>, value: &kiln_core::types::Alias) {
    push_str(output, value.as_str());
}

fn encode_outcome(output: &mut Vec<u8>, value: Outcome) {
    output.push(match value {
        Outcome::Pending => 1,
        Outcome::Success => 2,
        Outcome::DomainFailure => 3,
        Outcome::Denied => 4,
        Outcome::Cancelled => 5,
        Outcome::TimedOut => 6,
        Outcome::ProviderError => 7,
        Outcome::Unknown => 8,
    });
}

fn view_kind_tag(kind: ViewKind) -> u8 {
    match kind {
        ViewKind::StateSummary => 1,
        ViewKind::RunList => 2,
        ViewKind::RunDetail => 3,
        ViewKind::AttemptDetail => 4,
        ViewKind::EventTail => 5,
        ViewKind::LogRange => 6,
    }
}

fn view_field_tag(field: ViewField) -> u8 {
    match field {
        ViewField::ProviderProfile => 1,
        ViewField::ExternalReference => 2,
        ViewField::Products => 3,
        ViewField::Metrics => 4,
        ViewField::CommandId => 5,
        ViewField::LogBytes => 6,
    }
}

fn encode_view_set(output: &mut Vec<u8>, views: &BTreeSet<ViewKind>) {
    push_len(output, views.len());
    for view in views {
        output.push(view_kind_tag(*view));
    }
}

fn encode_field_set(output: &mut Vec<u8>, fields: &BTreeSet<ViewField>) {
    push_len(output, fields.len());
    for field in fields {
        output.push(view_field_tag(*field));
    }
}

fn encode_profile(output: &mut Vec<u8>, profile: &ObservationProfile) {
    push_u64(output, u64::from(profile.schema_version));
    push_str(output, &profile.profile_id);
    encode_view_set(output, &profile.views);
    push_u64(output, profile.bounds.max_page_items);
    push_u64(output, profile.bounds.max_attempts_per_run);
    push_u64(output, profile.bounds.max_products_per_attempt);
    push_u64(output, profile.bounds.max_metrics_per_attempt);
    push_u64(output, profile.bounds.max_event_records);
    push_u64(output, profile.bounds.max_log_bytes);
    push_u64(output, profile.bounds.max_log_window_bytes);
    push_u64(output, profile.bounds.max_log_segments);
    push_u64(output, profile.bounds.max_text_bytes);
    push_u64(output, profile.bounds.max_encoded_bytes);
    encode_field_set(output, &profile.redaction.visible_fields);
    push_len(output, profile.non_claims.len());
    for non_claim in &profile.non_claims {
        push_str(output, non_claim);
    }
}

fn encode_request(output: &mut Vec<u8>, request: &ObservationRequest) {
    push_u64(output, request.state_revision);
    match &request.view {
        ViewRequest::StateSummary => output.push(1),
        ViewRequest::RunList { after, limit } => {
            output.push(2);
            push_option(output, after.as_ref(), encode_run_id);
            push_u64(output, *limit);
        }
        ViewRequest::RunDetail { run_id } => {
            output.push(3);
            encode_run_id(output, run_id);
        }
        ViewRequest::AttemptDetail { attempt_id } => {
            output.push(4);
            encode_attempt_id(output, attempt_id);
        }
        ViewRequest::EventTail { limit } => {
            output.push(5);
            push_u64(output, *limit);
        }
        ViewRequest::LogRange {
            stream_id,
            cursor,
            max_bytes,
        } => {
            output.push(6);
            encode_stream_id(output, stream_id);
            push_u64(output, *cursor);
            push_u64(output, *max_bytes);
        }
    }
}

fn encode_window(output: &mut Vec<u8>, window: &ObservationWindow) {
    match &window.events {
        Some(events) => {
            output.push(1);
            push_len(output, events.records.len());
            for record in &events.records {
                encode_journal_record(output, record);
            }
        }
        None => output.push(0),
    }
    match &window.log {
        Some(log) => {
            output.push(1);
            encode_log_window(output, log);
        }
        None => output.push(0),
    }
}

fn encode_log_window(output: &mut Vec<u8>, log: &LogWindow) {
    encode_log_meta(output, &log.meta);
    push_bytes(output, &log.retained_bytes);
}

fn encode_log_meta(output: &mut Vec<u8>, meta: &LogStreamMeta) {
    encode_stream_id(output, &meta.stream_id);
    encode_attempt_id(output, &meta.attempt_id);
    push_len(output, meta.segments.len());
    for segment in &meta.segments {
        encode_content_id(output, &segment.identity);
        push_u64(output, segment.start_offset);
        push_u64(output, segment.byte_len);
    }
    push_u64(output, meta.retained_start);
    push_u64(output, meta.retained_end);
    push_bool(output, meta.complete);
    push_bool(output, meta.truncated);
}

fn encode_journal_record(output: &mut Vec<u8>, record: &JournalRecord) {
    push_u64(output, u64::from(record.schema_version));
    encode_external_ref(output, &record.stream);
    push_u64(output, record.expected_revision);
    encode_external_ref(output, &record.event_identity);
    encode_event(output, &record.event);
}

fn encode_event(output: &mut Vec<u8>, event: &Event) {
    match event {
        Event::RunStarted {
            command_id,
            run_id,
            attempt_id,
            provider_profile,
        } => {
            output.push(1);
            encode_command_id(output, command_id);
            encode_run_id(output, run_id);
            encode_attempt_id(output, attempt_id);
            encode_provider_profile(output, provider_profile);
        }
        Event::AttemptCompleted {
            command_id,
            attempt_id,
            outcome,
            external_reference,
            products,
            metrics,
        } => {
            output.push(2);
            encode_command_id(output, command_id);
            encode_attempt_id(output, attempt_id);
            encode_outcome(output, *outcome);
            push_option(output, external_reference.as_ref(), encode_external_ref);
            push_len(output, products.len());
            for product in products {
                encode_content_id(output, &product.identity);
                encode_attempt_id(output, &product.producing_attempt);
                push_option(
                    output,
                    product.provider_reference.as_ref(),
                    encode_external_ref,
                );
            }
            push_len(output, metrics.len());
            for metric in metrics {
                encode_alias(output, &metric.name);
                push_i64(output, metric.value);
                encode_attempt_id(output, &metric.producing_attempt);
            }
        }
        Event::AttemptUnknown {
            command_id,
            attempt_id,
            external_reference,
        } => {
            output.push(3);
            encode_command_id(output, command_id);
            encode_attempt_id(output, attempt_id);
            push_option(output, external_reference.as_ref(), encode_external_ref);
        }
        Event::AttemptReconciled {
            command_id,
            attempt_id,
            outcome,
            external_reference,
        } => {
            output.push(4);
            encode_command_id(output, command_id);
            encode_attempt_id(output, attempt_id);
            encode_outcome(output, *outcome);
            push_option(output, external_reference.as_ref(), encode_external_ref);
        }
        Event::RetryStarted {
            command_id,
            run_id,
            prior_attempt_id,
            next_attempt_id,
            provider_profile,
        } => {
            output.push(5);
            encode_command_id(output, command_id);
            encode_run_id(output, run_id);
            encode_attempt_id(output, prior_attempt_id);
            encode_attempt_id(output, next_attempt_id);
            encode_provider_profile(output, provider_profile);
        }
        Event::CancellationRequested {
            command_id,
            attempt_id,
        } => {
            output.push(6);
            encode_command_id(output, command_id);
            encode_attempt_id(output, attempt_id);
        }
        Event::HoldChanged {
            command_id,
            scope,
            held,
        } => {
            output.push(7);
            encode_command_id(output, command_id);
            encode_external_ref(output, scope);
            push_bool(output, *held);
        }
    }
}

fn encode_body(output: &mut Vec<u8>, body: &ViewBody) {
    match body {
        ViewBody::StateSummary(view) => {
            output.push(1);
            encode_state_summary(output, view);
        }
        ViewBody::RunList(view) => {
            output.push(2);
            encode_run_list(output, view);
        }
        ViewBody::RunDetail(view) => {
            output.push(3);
            encode_run_detail(output, view);
        }
        ViewBody::AttemptDetail(view) => {
            output.push(4);
            encode_attempt_detail(output, view);
        }
        ViewBody::EventTail(view) => {
            output.push(5);
            encode_event_tail(output, view);
        }
        ViewBody::LogRange(view) => {
            output.push(6);
            encode_log_range(output, view);
        }
        ViewBody::NotFound(view) => {
            output.push(7);
            encode_not_found(output, view);
        }
    }
}

fn encode_outcome_counts(output: &mut Vec<u8>, counts: &[OutcomeCount]) {
    push_len(output, counts.len());
    for count in counts {
        encode_outcome(output, count.outcome);
        push_u64(output, count.count);
    }
}

fn encode_state_summary(output: &mut Vec<u8>, view: &StateSummaryView) {
    push_u64(output, view.run_count);
    push_u64(output, view.attempt_count);
    push_u64(output, view.hold_count);
    encode_outcome_counts(output, &view.run_outcomes);
    encode_outcome_counts(output, &view.attempt_outcomes);
}

fn encode_page_info(output: &mut Vec<u8>, page: &PageInfo) {
    push_u64(output, page.requested_limit);
    push_u64(output, page.effective_limit);
    push_u64(output, page.returned);
    push_bool(output, page.capped);
    push_bool(output, page.has_more);
    push_option(output, page.next.as_ref(), encode_run_id);
}

fn encode_run_summary(output: &mut Vec<u8>, run: &RunSummaryView) {
    encode_run_id(output, &run.run_id);
    encode_attempt_id(output, &run.active_attempt);
    encode_outcome(output, run.outcome);
    push_u64(output, run.attempt_count);
}

fn encode_run_list(output: &mut Vec<u8>, view: &RunListView) {
    push_len(output, view.runs.len());
    for run in &view.runs {
        encode_run_summary(output, run);
    }
    encode_page_info(output, &view.page);
}

fn encode_attempt_summary(output: &mut Vec<u8>, attempt: &AttemptSummaryView) {
    encode_attempt_id(output, &attempt.attempt_id);
    push_option(
        output,
        attempt.provider_profile.as_ref(),
        encode_provider_profile,
    );
    encode_outcome(output, attempt.outcome);
    push_u64(output, attempt.product_count);
    push_u64(output, attempt.metric_count);
}

fn encode_run_detail(output: &mut Vec<u8>, view: &RunDetailView) {
    encode_run_summary(output, &view.run);
    push_len(output, view.attempts.len());
    for attempt in &view.attempts {
        encode_attempt_summary(output, attempt);
    }
    push_u64(output, view.attempts_truncated);
}

fn encode_product(output: &mut Vec<u8>, product: &ProductView) {
    encode_content_id(output, &product.identity);
    encode_attempt_id(output, &product.producing_attempt);
    push_option(
        output,
        product.provider_reference.as_ref(),
        encode_external_ref,
    );
}

fn encode_metric(output: &mut Vec<u8>, metric: &MetricView) {
    encode_alias(output, &metric.name);
    push_i64(output, metric.value);
    encode_attempt_id(output, &metric.producing_attempt);
}

fn encode_attempt_detail(output: &mut Vec<u8>, view: &AttemptDetailView) {
    encode_attempt_id(output, &view.attempt_id);
    encode_run_id(output, &view.run_id);
    push_option(
        output,
        view.provider_profile.as_ref(),
        encode_provider_profile,
    );
    encode_outcome(output, view.outcome);
    push_option(
        output,
        view.external_reference.as_ref(),
        encode_external_ref,
    );
    push_u64(output, view.product_count);
    push_u64(output, view.metric_count);
    push_len(output, view.products.len());
    for product in &view.products {
        encode_product(output, product);
    }
    push_len(output, view.metrics.len());
    for metric in &view.metrics {
        encode_metric(output, metric);
    }
    push_u64(output, view.products_truncated);
    push_u64(output, view.metrics_truncated);
}

fn encode_event_projection(output: &mut Vec<u8>, event: &EventProjection) {
    match event {
        EventProjection::RunStarted {
            run_id,
            attempt_id,
            provider_profile,
        } => {
            output.push(1);
            encode_run_id(output, run_id);
            encode_attempt_id(output, attempt_id);
            push_option(output, provider_profile.as_ref(), encode_provider_profile);
        }
        EventProjection::AttemptCompleted {
            attempt_id,
            outcome,
            external_reference,
            product_count,
            metric_count,
        } => {
            output.push(2);
            encode_attempt_id(output, attempt_id);
            encode_outcome(output, *outcome);
            push_option(output, external_reference.as_ref(), encode_external_ref);
            push_u64(output, *product_count);
            push_u64(output, *metric_count);
        }
        EventProjection::AttemptUnknown {
            attempt_id,
            external_reference,
        } => {
            output.push(3);
            encode_attempt_id(output, attempt_id);
            push_option(output, external_reference.as_ref(), encode_external_ref);
        }
        EventProjection::AttemptReconciled {
            attempt_id,
            outcome,
            external_reference,
        } => {
            output.push(4);
            encode_attempt_id(output, attempt_id);
            encode_outcome(output, *outcome);
            push_option(output, external_reference.as_ref(), encode_external_ref);
        }
        EventProjection::RetryStarted {
            run_id,
            prior_attempt_id,
            next_attempt_id,
            provider_profile,
        } => {
            output.push(5);
            encode_run_id(output, run_id);
            encode_attempt_id(output, prior_attempt_id);
            encode_attempt_id(output, next_attempt_id);
            push_option(output, provider_profile.as_ref(), encode_provider_profile);
        }
        EventProjection::CancellationRequested { attempt_id } => {
            output.push(6);
            encode_attempt_id(output, attempt_id);
        }
        EventProjection::HoldChanged { scope, held } => {
            output.push(7);
            push_option(output, scope.as_ref(), encode_external_ref);
            push_bool(output, *held);
        }
    }
}

fn encode_event_record(output: &mut Vec<u8>, record: &EventRecordView) {
    push_u64(output, record.revision);
    push_option(output, record.stream.as_ref(), encode_external_ref);
    push_option(output, record.event_identity.as_ref(), encode_external_ref);
    push_option(output, record.command_id.as_ref(), encode_command_id);
    encode_event_projection(output, &record.event);
}

fn encode_event_page(output: &mut Vec<u8>, page: &EventPageInfo) {
    push_u64(output, page.requested_limit);
    push_u64(output, page.effective_limit);
    push_u64(output, page.returned);
    push_bool(output, page.capped);
    push_bool(output, page.has_earlier);
    push_u64(output, page.start_revision);
    push_u64(output, page.end_revision);
}

fn encode_event_tail(output: &mut Vec<u8>, view: &EventTailView) {
    push_len(output, view.records.len());
    for record in &view.records {
        encode_event_record(output, record);
    }
    encode_event_page(output, &view.page);
}

fn encode_retention(output: &mut Vec<u8>, retention: &RetentionView) {
    match retention {
        RetentionView::Valid => output.push(1),
        RetentionView::Gap { retained_start } => {
            output.push(2);
            push_u64(output, *retained_start);
        }
    }
}

fn encode_log_range(output: &mut Vec<u8>, view: &LogRangeView) {
    encode_stream_id(output, &view.stream_id);
    encode_attempt_id(output, &view.attempt_id);
    push_u64(output, view.cursor);
    push_u64(output, view.retained_start);
    push_u64(output, view.retained_end);
    encode_retention(output, &view.retention);
    push_u64(output, view.requested_max_bytes);
    push_u64(output, view.effective_max_bytes);
    push_u64(output, view.returned_bytes);
    push_bool(output, view.capped);
    push_bool(output, view.has_more);
    push_bool(output, view.stream_complete);
    push_bool(output, view.stream_truncated);
    match &view.bytes {
        Some(bytes) => {
            output.push(1);
            push_bytes(output, bytes);
        }
        None => output.push(0),
    }
}

fn encode_not_found(output: &mut Vec<u8>, view: &NotFoundView) {
    match &view.target {
        NotFoundTarget::Run(run_id) => {
            output.push(1);
            encode_run_id(output, run_id);
        }
        NotFoundTarget::Attempt(attempt_id) => {
            output.push(2);
            encode_attempt_id(output, attempt_id);
        }
        NotFoundTarget::Stream(stream_id) => {
            output.push(3);
            encode_stream_id(output, stream_id);
        }
    }
}
