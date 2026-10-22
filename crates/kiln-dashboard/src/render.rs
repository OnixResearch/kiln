use kiln_core::domain::Outcome;
use kiln_observe_core::{AttemptDetailView, EventProjection, RunDetailView, ViewBody, ViewField};

use crate::journal::reference_text;
use crate::report::DashboardReport;

/// Terminal rendering style of one dashboard report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderStyle {
    /// Deterministic plain text without escape sequences.
    Plain,
    /// Deterministic text with ANSI styling.
    Ansi,
}

/// Render one dashboard report as deterministic operator text.
// r[impl kiln.observe.dashboard.report]
// r[impl kiln.observe.dashboard.report.scenario.deterministic]
#[must_use]
pub fn render_report(report: &DashboardReport, style: RenderStyle) -> String {
    let painter = Painter { style };
    let mut out = String::new();
    render_header(&mut out, &painter, report);
    render_summary(&mut out, &painter, report);
    render_runs(&mut out, &painter, report);
    render_run_detail(&mut out, &painter, report);
    render_attempt_detail(&mut out, &painter, report);
    render_events(&mut out, &painter, report);
    render_identities(&mut out, &painter, report);
    render_redactions(&mut out, &painter, report);
    render_non_claims(&mut out, &painter, report);
    out
}

struct Painter {
    style: RenderStyle,
}

impl Painter {
    fn bold(&self, text: &str) -> String {
        match self.style {
            RenderStyle::Plain => text.to_string(),
            RenderStyle::Ansi => format!("\u{1b}[1m{text}\u{1b}[0m"),
        }
    }

    fn outcome(&self, outcome: Outcome) -> String {
        let (code, label) = outcome_style(outcome);
        match self.style {
            RenderStyle::Plain => label.to_string(),
            RenderStyle::Ansi => format!("\u{1b}[{code}m{label}\u{1b}[0m"),
        }
    }
}

fn outcome_style(outcome: Outcome) -> (&'static str, &'static str) {
    match outcome {
        Outcome::Pending => ("33", "Pending"),
        Outcome::Success => ("32", "Success"),
        Outcome::DomainFailure => ("31", "DomainFailure"),
        Outcome::Denied => ("31", "Denied"),
        Outcome::Cancelled => ("35", "Cancelled"),
        Outcome::TimedOut => ("35", "TimedOut"),
        Outcome::ProviderError => ("31", "ProviderError"),
        Outcome::Unknown => ("2", "Unknown"),
    }
}

fn render_header(out: &mut String, painter: &Painter, report: &DashboardReport) {
    out.push_str(&painter.bold("kiln dashboard\n"));
    push_pair(out, "profile", &report.profile_id);
    push_pair(out, "observation", &report.observation_profile_id);
    let schema = report
        .journal
        .schema
        .clone()
        .unwrap_or_else(|| "-".to_string());
    push_pair(out, "journal schema", &schema);
    let declared = report
        .journal
        .declared_identity
        .clone()
        .unwrap_or_else(|| "-".to_string());
    push_pair(out, "declared identity", &declared);
    push_pair(out, "records", &report.journal.record_count.to_string());
    push_pair(
        out,
        "state revision",
        &report.journal.state_revision.to_string(),
    );
    out.push('\n');
}

fn push_pair(out: &mut String, label: &str, value: &str) {
    out.push_str("  ");
    out.push_str(label);
    for _ in label.len()..20 {
        out.push(' ');
    }
    out.push_str(value);
    out.push('\n');
}

fn render_summary(out: &mut String, painter: &Painter, report: &DashboardReport) {
    let ViewBody::StateSummary(summary) = &report.views.summary.body else {
        return;
    };
    out.push_str(&painter.bold("summary\n"));
    push_pair(
        out,
        "runs",
        &format!(
            "{}   attempts {}   holds {}",
            summary.run_count, summary.attempt_count, summary.hold_count
        ),
    );
    push_pair(
        out,
        "run outcomes",
        &outcome_counts(&summary.run_outcomes, painter),
    );
    push_pair(
        out,
        "attempt outcomes",
        &outcome_counts(&summary.attempt_outcomes, painter),
    );
    out.push('\n');
}

fn outcome_counts(counts: &[kiln_observe_core::OutcomeCount], painter: &Painter) -> String {
    if counts.is_empty() {
        return "-".to_string();
    }
    counts
        .iter()
        .map(|entry| format!("{} {}", painter.outcome(entry.outcome), entry.count))
        .collect::<Vec<String>>()
        .join("  ")
}

fn render_runs(out: &mut String, painter: &Painter, report: &DashboardReport) {
    let runs = &report.views.runs;
    // r[impl kiln.observe.dashboard.report.scenario.bounded]
    out.push_str(&painter.bold("runs\n"));
    let mut facts = format!(
        "pages {}   runs {}   requested {}   effective {}",
        runs.views.len(),
        runs.total_runs,
        runs.requested_limit,
        runs.effective_limit
    );
    if runs.capped {
        facts.push_str("   capped");
    }
    if runs.has_more {
        facts.push_str("   more");
    }
    if runs.truncated {
        facts.push_str("   truncated");
    }
    push_pair(out, "page", &facts);
    for view in &runs.views {
        let ViewBody::RunList(list) = &view.body else {
            continue;
        };
        for run in &list.runs {
            out.push_str("  ");
            out.push_str(run.run_id.as_str());
            out.push_str("  ");
            out.push_str(&painter.outcome(run.outcome));
            out.push_str("  attempts ");
            out.push_str(&run.attempt_count.to_string());
            out.push('\n');
        }
    }
    out.push('\n');
}

fn render_run_detail(out: &mut String, painter: &Painter, report: &DashboardReport) {
    let Some(view) = report.views.run_detail.as_ref() else {
        return;
    };
    match &view.body {
        ViewBody::RunDetail(detail) => {
            out.push_str(&painter.bold("run detail\n"));
            render_run_detail_body(out, painter, report, detail);
            out.push('\n');
        }
        ViewBody::NotFound(not_found) => {
            out.push_str(&painter.bold("run detail\n"));
            push_pair(out, "target", &not_found_text(&not_found.target));
            out.push('\n');
        }
        _ => {}
    }
}

fn render_run_detail_body(
    out: &mut String,
    painter: &Painter,
    report: &DashboardReport,
    detail: &RunDetailView,
) {
    push_pair(out, "run", detail.run.run_id.as_str());
    push_pair(
        out,
        "outcome",
        &format!(
            "{}   attempts {}   active attempt {}",
            painter.outcome(detail.run.outcome),
            detail.run.attempt_count,
            detail.run.active_attempt.as_str()
        ),
    );
    for attempt in &detail.attempts {
        out.push_str("    ");
        out.push_str(attempt.attempt_id.as_str());
        out.push_str("  ");
        out.push_str(&painter.outcome(attempt.outcome));
        out.push_str("  provider ");
        out.push_str(&provider_text(
            attempt
                .provider_profile
                .as_ref()
                .map(|value| value.as_str()),
            report,
        ));
        out.push_str("  products ");
        out.push_str(&attempt.product_count.to_string());
        out.push_str("  metrics ");
        out.push_str(&attempt.metric_count.to_string());
        out.push('\n');
    }
    if detail.attempts_truncated > 0 {
        push_pair(
            out,
            "attempts truncated",
            &detail.attempts_truncated.to_string(),
        );
    }
}

fn render_attempt_detail(out: &mut String, painter: &Painter, report: &DashboardReport) {
    let Some(view) = report.views.attempt_detail.as_ref() else {
        return;
    };
    match &view.body {
        ViewBody::AttemptDetail(detail) => {
            out.push_str(&painter.bold("attempt detail\n"));
            render_attempt_detail_body(out, painter, report, detail);
            out.push('\n');
        }
        ViewBody::NotFound(not_found) => {
            out.push_str(&painter.bold("attempt detail\n"));
            push_pair(out, "target", &not_found_text(&not_found.target));
            out.push('\n');
        }
        _ => {}
    }
}

fn render_attempt_detail_body(
    out: &mut String,
    painter: &Painter,
    report: &DashboardReport,
    detail: &AttemptDetailView,
) {
    push_pair(out, "attempt", detail.attempt_id.as_str());
    push_pair(out, "run", detail.run_id.as_str());
    push_pair(out, "outcome", &painter.outcome(detail.outcome));
    push_pair(
        out,
        "provider",
        &provider_text(
            detail.provider_profile.as_ref().map(|value| value.as_str()),
            report,
        ),
    );
    let external = match detail.external_reference.as_ref() {
        Some(reference) => reference_text(reference),
        None => redacted_or_dash(report, ViewField::ExternalReference),
    };
    push_pair(out, "external", &external);
    let products = if report.redacted_fields.contains(&ViewField::Products) {
        format!("{}   redacted", detail.product_count)
    } else {
        detail.product_count.to_string()
    };
    push_pair(out, "products", &products);
    for product in &detail.products {
        push_pair(out, "    product", product.identity.as_str());
    }
    if detail.products_truncated > 0 {
        push_pair(
            out,
            "products truncated",
            &detail.products_truncated.to_string(),
        );
    }
    let metrics = if report.redacted_fields.contains(&ViewField::Metrics) {
        format!("{}   redacted", detail.metric_count)
    } else {
        detail.metric_count.to_string()
    };
    push_pair(out, "metrics", &metrics);
    for metric in &detail.metrics {
        push_pair(
            out,
            "    metric",
            &format!("{} = {}", metric.name.as_str(), metric.value),
        );
    }
    if detail.metrics_truncated > 0 {
        push_pair(
            out,
            "metrics truncated",
            &detail.metrics_truncated.to_string(),
        );
    }
}

fn provider_text(value: Option<&str>, report: &DashboardReport) -> String {
    match value {
        Some(text) => text.to_string(),
        None => redacted_or_dash(report, ViewField::ProviderProfile),
    }
}

fn redacted_or_dash(report: &DashboardReport, view_field: ViewField) -> String {
    // r[impl kiln.observe.dashboard.projection.scenario.redaction]
    if report.redacted_fields.contains(&view_field) {
        "redacted".to_string()
    } else {
        "-".to_string()
    }
}

fn render_events(out: &mut String, painter: &Painter, report: &DashboardReport) {
    let ViewBody::EventTail(events) = &report.views.events.body else {
        return;
    };
    out.push_str(&painter.bold("events\n"));
    let mut facts = format!(
        "requested {}   effective {}   returned {}",
        events.page.requested_limit, events.page.effective_limit, events.page.returned
    );
    if events.page.capped {
        facts.push_str("   capped");
    }
    if events.page.has_earlier {
        facts.push_str("   earlier");
    }
    facts.push_str(&format!(
        "   revisions {}..{}",
        events.page.start_revision, events.page.end_revision
    ));
    push_pair(out, "page", &facts);
    for record in &events.records {
        out.push_str("  ");
        out.push_str(&record.revision.to_string());
        for _ in record.revision.to_string().len()..6 {
            out.push(' ');
        }
        out.push_str(&event_text(&record.event, painter));
        out.push('\n');
    }
    out.push('\n');
}

fn event_text(event: &EventProjection, painter: &Painter) -> String {
    match event {
        EventProjection::RunStarted {
            run_id,
            attempt_id,
            provider_profile,
        } => format!(
            "run_started     run {}  attempt {}  provider {}",
            run_id.as_str(),
            attempt_id.as_str(),
            provider_profile
                .as_ref()
                .map_or("-".to_string(), |value| value.as_str().to_string())
        ),
        EventProjection::AttemptCompleted {
            attempt_id,
            outcome,
            external_reference,
            product_count,
            metric_count,
        } => format!(
            "attempt_done    attempt {}  {}  products {}  metrics {}  external {}",
            attempt_id.as_str(),
            painter.outcome(*outcome),
            product_count,
            metric_count,
            external_reference
                .as_ref()
                .map_or("-".to_string(), reference_text)
        ),
        EventProjection::AttemptUnknown {
            attempt_id,
            external_reference,
        } => format!(
            "attempt_unknown attempt {}  external {}",
            attempt_id.as_str(),
            external_reference
                .as_ref()
                .map_or("-".to_string(), reference_text)
        ),
        EventProjection::AttemptReconciled {
            attempt_id,
            outcome,
            external_reference,
        } => format!(
            "attempt_reconciled attempt {}  {}  external {}",
            attempt_id.as_str(),
            painter.outcome(*outcome),
            external_reference
                .as_ref()
                .map_or("-".to_string(), reference_text)
        ),
        EventProjection::RetryStarted {
            run_id,
            prior_attempt_id,
            next_attempt_id,
            provider_profile,
        } => format!(
            "retry_started   run {}  from {}  to {}  provider {}",
            run_id.as_str(),
            prior_attempt_id.as_str(),
            next_attempt_id.as_str(),
            provider_profile
                .as_ref()
                .map_or("-".to_string(), |value| value.as_str().to_string())
        ),
        EventProjection::CancellationRequested { attempt_id } => {
            format!("cancel_requested attempt {}", attempt_id.as_str())
        }
        EventProjection::HoldChanged { scope, held } => format!(
            "hold_changed    scope {}  held {}",
            scope.as_ref().map_or("-".to_string(), reference_text),
            held
        ),
    }
}

fn not_found_text(target: &kiln_observe_core::NotFoundTarget) -> String {
    match target {
        kiln_observe_core::NotFoundTarget::Run(run_id) => {
            format!("run {} not found", run_id.as_str())
        }
        kiln_observe_core::NotFoundTarget::Attempt(attempt_id) => {
            format!("attempt {} not found", attempt_id.as_str())
        }
        kiln_observe_core::NotFoundTarget::Stream(stream_id) => {
            format!("stream {} not found", stream_id.as_str())
        }
    }
}

fn render_identities(out: &mut String, painter: &Painter, report: &DashboardReport) {
    out.push_str(&painter.bold("view identities\n"));
    push_pair(out, "state_summary", report.views.summary.identity.as_str());
    for (index, view) in report.views.runs.views.iter().enumerate() {
        push_pair(out, &format!("run_list[{index}]"), view.identity.as_str());
    }
    if let Some(view) = report.views.run_detail.as_ref() {
        push_pair(out, "run_detail", view.identity.as_str());
    }
    if let Some(view) = report.views.attempt_detail.as_ref() {
        push_pair(out, "attempt_detail", view.identity.as_str());
    }
    push_pair(out, "event_tail", report.views.events.identity.as_str());
    out.push('\n');
}

fn render_redactions(out: &mut String, painter: &Painter, report: &DashboardReport) {
    out.push_str(&painter.bold("redacted fields\n"));
    if report.redacted_fields.is_empty() {
        push_pair(out, "fields", "-");
    } else {
        let names: Vec<String> = report
            .redacted_fields
            .iter()
            .map(|value| field_label(*value).to_string())
            .collect();
        push_pair(out, "fields", &names.join("  "));
    }
    out.push('\n');
}

fn field_label(value: ViewField) -> &'static str {
    match value {
        ViewField::ProviderProfile => "provider_profile",
        ViewField::ExternalReference => "external_reference",
        ViewField::Products => "products",
        ViewField::Metrics => "metrics",
        ViewField::CommandId => "command_id",
        ViewField::LogBytes => "log_bytes",
    }
}

fn render_non_claims(out: &mut String, painter: &Painter, report: &DashboardReport) {
    // r[impl kiln.observe.dashboard.nonclaims]
    // r[impl kiln.observe.dashboard.nonclaims.scenario.scope]
    out.push_str(&painter.bold("non-claims\n"));
    for non_claim in &report.non_claims {
        out.push_str("  - ");
        out.push_str(non_claim);
        out.push('\n');
    }
}
