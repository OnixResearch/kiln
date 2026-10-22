//! The HTML rendering of one dashboard report.
//!
//! The document is self-contained: inline style, no script, no external
//! resource, and no clock or environment input, so the same admitted state
//! renders the same bytes every time. It reports exactly the facts the
//! terminal report reports, including the view identities and the non-claims.

use kiln_core::domain::Outcome;
use kiln_observe_core::{
    AttemptDetailView, EventProjection, EventTailView, NotFoundTarget, RunDetailView,
    StateSummaryView, ViewBody, ViewField,
};

use crate::journal::reference_text;
use crate::report::DashboardReport;

/// Render one dashboard report as a self-contained HTML document.
// r[impl kiln.observe.dashboard.html]
// r[impl kiln.observe.dashboard.html.scenario.parity]
// r[impl kiln.observe.dashboard.html.scenario.determinism]
// r[impl kiln.observe.dashboard.html.scenario.self_contained]
// r[impl kiln.observe.dashboard.html.scenario.escaping]
// r[impl kiln.observe.dashboard.html.nonclaims]
#[must_use]
pub fn render_html(report: &DashboardReport) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n");
    out.push_str("<html lang=\"en\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str(&format!(
        "<title>Kiln dashboard {}</title>\n",
        escape(&report.profile_id)
    ));
    out.push_str("<style>\n");
    out.push_str(STYLE);
    out.push_str("</style>\n</head>\n<body>\n");
    out.push_str("<header>\n");
    out.push_str(&format!(
        "<h1>Kiln dashboard <span class=\"profile\">{}</span></h1>\n",
        escape(&report.profile_id)
    ));
    out.push_str("<dl class=\"facts\">\n");
    push_fact(
        &mut out,
        "observation profile",
        &report.observation_profile_id,
    );
    push_fact(
        &mut out,
        "journal schema",
        report.journal.schema.as_deref().unwrap_or("-"),
    );
    push_fact(
        &mut out,
        "declared identity",
        report.journal.declared_identity.as_deref().unwrap_or("-"),
    );
    push_fact(
        &mut out,
        "records",
        &report.journal.record_count.to_string(),
    );
    push_fact(
        &mut out,
        "state revision",
        &report.journal.state_revision.to_string(),
    );
    out.push_str("</dl>\n</header>\n<main>\n");

    if let ViewBody::StateSummary(summary) = &report.views.summary.body {
        out.push_str(&section("summary", &summary_section(summary)));
    }
    out.push_str(&section("runs", &runs_section(report)));
    if let Some(view) = report.views.run_detail.as_ref() {
        out.push_str(&section("run detail", &run_detail_section(view)));
    }
    if let Some(view) = report.views.attempt_detail.as_ref() {
        out.push_str(&section("attempt detail", &attempt_detail_section(view)));
    }
    if let ViewBody::EventTail(events) = &report.views.events.body {
        out.push_str(&section("events", &events_section(events)));
    }
    out.push_str(&section("view identities", &identities_section(report)));
    out.push_str(&section("redacted fields", &redactions_section(report)));
    out.push_str(&section("non-claims", &non_claims_section(report)));
    out.push_str("</main>\n</body>\n</html>\n");
    out
}

const STYLE: &str = "\
:root { color-scheme: light dark; }
body { font: 15px/1.5 ui-sans-serif, system-ui, -apple-system, 'Segoe UI', sans-serif;
  margin: 0 auto; max-width: 72rem; padding: 2rem 1.25rem; }
h1 { font-size: 1.4rem; margin: 0 0 .25rem; }
h2 { font-size: 1.05rem; margin: 0 0 .5rem; }
section { border-top: 1px solid rgba(128,128,128,.35); margin-top: 1.5rem; padding-top: 1rem; }
.profile { font-weight: 400; color: #6b7280; }
.facts { display: grid; grid-template-columns: max-content 1fr; gap: .1rem 1rem; margin: 0; }
.facts dt { color: #6b7280; }
.facts dd { margin: 0; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
table { border-collapse: collapse; width: 100%; }
th, td { text-align: left; padding: .3rem .6rem .3rem 0; vertical-align: top;
  border-bottom: 1px solid rgba(128,128,128,.22); }
th { font-weight: 600; color: #6b7280; font-size: .82rem; text-transform: uppercase;
  letter-spacing: .03em; }
th.plain { text-transform: none; letter-spacing: 0; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
td.mono, .mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
.outcome { font-weight: 600; }
.outcome-success { color: #15803d; }
.outcome-pending { color: #a16207; }
.outcome-failure { color: #b91c1c; }
.outcome-unknown { color: #6b7280; }
.pill { display: inline-block; border: 1px solid rgba(128,128,128,.4); border-radius: 999px;
  padding: 0 .5rem; margin-right: .35rem; font-size: .8rem; color: #6b7280; }
.claims li { margin-bottom: .2rem; }
.note { color: #6b7280; }
";

fn push_fact(out: &mut String, label: &str, value: &str) {
    out.push_str(&format!(
        "<dt>{}</dt><dd>{}</dd>\n",
        escape(label),
        escape(value)
    ));
}

fn section(title: &str, body: &str) -> String {
    format!(
        "<section>\n<h2>{}</h2>\n{}\n</section>\n",
        escape(title),
        body
    )
}

fn summary_section(summary: &StateSummaryView) -> String {
    let mut out = String::new();
    out.push_str("<dl class=\"facts\">\n");
    push_fact(&mut out, "runs", &summary.run_count.to_string());
    push_fact(&mut out, "attempts", &summary.attempt_count.to_string());
    push_fact(&mut out, "holds", &summary.hold_count.to_string());
    out.push_str("</dl>\n");
    out.push_str("<table>\n<thead><tr><th>run outcomes</th><th>attempt outcomes</th></tr></thead>\n<tbody><tr><td>");
    out.push_str(&outcome_counts(&summary.run_outcomes));
    out.push_str("</td><td>");
    out.push_str(&outcome_counts(&summary.attempt_outcomes));
    out.push_str("</td></tr></tbody>\n</table>\n");
    out
}

fn outcome_counts(counts: &[kiln_observe_core::OutcomeCount]) -> String {
    let mut out = String::new();
    for entry in counts {
        out.push_str(&format!(
            "<span class=\"pill outcome {}\">{} {}</span>",
            outcome_class(entry.outcome),
            escape(&format!("{:?}", entry.outcome)),
            entry.count
        ));
    }
    if counts.is_empty() {
        out.push_str("<span class=\"note\">none</span>");
    }
    out
}

fn runs_section(report: &DashboardReport) -> String {
    let runs = &report.views.runs;
    let mut out = String::new();
    out.push_str("<p>");
    out.push_str(&format!(
        "<span class=\"pill\">pages {}</span><span class=\"pill\">runs {}</span><span class=\"pill\">requested {}</span><span class=\"pill\">effective {}</span>",
        runs.views.len(),
        runs.total_runs,
        runs.requested_limit,
        runs.effective_limit
    ));
    if runs.capped {
        out.push_str("<span class=\"pill\">capped</span>");
    }
    if runs.has_more {
        out.push_str("<span class=\"pill\">more</span>");
    }
    if runs.truncated {
        out.push_str("<span class=\"pill\">truncated</span>");
    }
    out.push_str("</p>\n");
    out.push_str("<table>\n<thead><tr><th>run</th><th>outcome</th><th>active attempt</th><th>attempts</th></tr></thead>\n<tbody>\n");
    for view in &runs.views {
        let ViewBody::RunList(list) = &view.body else {
            continue;
        };
        for run in &list.runs {
            out.push_str(&format!(
                "<tr><td class=\"mono\">{}</td><td class=\"outcome {}\">{}</td><td class=\"mono\">{}</td><td>{}</td></tr>\n",
                escape(run.run_id.as_str()),
                outcome_class(run.outcome),
                escape(&format!("{:?}", run.outcome)),
                escape(run.active_attempt.as_str()),
                run.attempt_count
            ));
        }
    }
    out.push_str("</tbody>\n</table>\n");
    out
}

fn run_detail_section(view: &kiln_observe_core::ObservationView) -> String {
    let mut out = String::new();
    match &view.body {
        ViewBody::RunDetail(detail) => {
            out.push_str(&run_detail_body(detail));
        }
        ViewBody::NotFound(not_found) => {
            out.push_str(&format!(
                "<p class=\"note\">{}</p>\n",
                escape(&not_found_text(&not_found.target))
            ));
        }
        _ => {}
    }
    out
}

fn run_detail_body(detail: &RunDetailView) -> String {
    let mut out = String::new();
    out.push_str("<dl class=\"facts\">\n");
    push_fact(&mut out, "run", detail.run.run_id.as_str());
    push_fact(&mut out, "outcome", &format!("{:?}", detail.run.outcome));
    push_fact(
        &mut out,
        "active attempt",
        detail.run.active_attempt.as_str(),
    );
    push_fact(&mut out, "attempts", &detail.run.attempt_count.to_string());
    out.push_str("</dl>\n");
    out.push_str("<table>\n<thead><tr><th>attempt</th><th>outcome</th><th>provider</th><th>products</th><th>metrics</th></tr></thead>\n<tbody>\n");
    for attempt in &detail.attempts {
        out.push_str(&format!(
            "<tr><td class=\"mono\">{}</td><td class=\"outcome {}\">{}</td><td class=\"mono\">{}</td><td>{}</td><td>{}</td></tr>\n",
            escape(attempt.attempt_id.as_str()),
            outcome_class(attempt.outcome),
            escape(&format!("{:?}", attempt.outcome)),
            escape(
                attempt
                    .provider_profile
                    .as_ref()
                    .map_or("-", |value| value.as_str())
            ),
            attempt.product_count,
            attempt.metric_count
        ));
    }
    out.push_str("</tbody>\n</table>\n");
    if detail.attempts_truncated > 0 {
        out.push_str(&format!(
            "<p class=\"note\">{} further attempts truncated by the profile bound</p>\n",
            detail.attempts_truncated
        ));
    }
    out
}

fn attempt_detail_section(view: &kiln_observe_core::ObservationView) -> String {
    let mut out = String::new();
    match &view.body {
        ViewBody::AttemptDetail(detail) => {
            out.push_str(&attempt_detail_body(detail));
        }
        ViewBody::NotFound(not_found) => {
            out.push_str(&format!(
                "<p class=\"note\">{}</p>\n",
                escape(&not_found_text(&not_found.target))
            ));
        }
        _ => {}
    }
    out
}

fn attempt_detail_body(detail: &AttemptDetailView) -> String {
    let mut out = String::new();
    out.push_str("<dl class=\"facts\">\n");
    push_fact(&mut out, "attempt", detail.attempt_id.as_str());
    push_fact(&mut out, "run", detail.run_id.as_str());
    push_fact(&mut out, "outcome", &format!("{:?}", detail.outcome));
    push_fact(
        &mut out,
        "provider",
        detail
            .provider_profile
            .as_ref()
            .map_or("-", |value| value.as_str()),
    );
    push_fact(
        &mut out,
        "external reference",
        detail
            .external_reference
            .as_ref()
            .map_or_else(|| "-".to_string(), reference_text)
            .as_str(),
    );
    push_fact(&mut out, "products", &detail.product_count.to_string());
    push_fact(&mut out, "metrics", &detail.metric_count.to_string());
    out.push_str("</dl>\n");
    if !detail.products.is_empty() {
        out.push_str("<table>\n<thead><tr><th>product</th><th>producing attempt</th><th>provider reference</th></tr></thead>\n<tbody>\n");
        for product in &detail.products {
            out.push_str(&format!(
                "<tr><td class=\"mono\">{}</td><td class=\"mono\">{}</td><td class=\"mono\">{}</td></tr>\n",
                escape(product.identity.as_str()),
                escape(product.producing_attempt.as_str()),
                escape(
                    product
                        .provider_reference
                        .as_ref()
                        .map_or_else(|| "-".to_string(), reference_text)
                        .as_str()
                )
            ));
        }
        out.push_str("</tbody>\n</table>\n");
    }
    if !detail.metrics.is_empty() {
        out.push_str("<table>\n<thead><tr><th>metric</th><th>value</th><th>producing attempt</th></tr></thead>\n<tbody>\n");
        for metric in &detail.metrics {
            out.push_str(&format!(
                "<tr><td class=\"mono\">{}</td><td>{}</td><td class=\"mono\">{}</td></tr>\n",
                escape(metric.name.as_str()),
                metric.value,
                escape(metric.producing_attempt.as_str())
            ));
        }
        out.push_str("</tbody>\n</table>\n");
    }
    if detail.products_truncated > 0 {
        out.push_str(&format!(
            "<p class=\"note\">{} further products truncated by the profile bound</p>\n",
            detail.products_truncated
        ));
    }
    if detail.metrics_truncated > 0 {
        out.push_str(&format!(
            "<p class=\"note\">{} further metrics truncated by the profile bound</p>\n",
            detail.metrics_truncated
        ));
    }
    out
}

fn events_section(events: &EventTailView) -> String {
    let mut out = String::new();
    out.push_str("<p>");
    out.push_str(&format!(
        "<span class=\"pill\">requested {}</span><span class=\"pill\">effective {}</span><span class=\"pill\">returned {}</span><span class=\"pill\">revisions {}..{}</span>",
        events.page.requested_limit,
        events.page.effective_limit,
        events.page.returned,
        events.page.start_revision,
        events.page.end_revision
    ));
    if events.page.capped {
        out.push_str("<span class=\"pill\">capped</span>");
    }
    if events.page.has_earlier {
        out.push_str("<span class=\"pill\">earlier</span>");
    }
    out.push_str("</p>\n");
    out.push_str("<table>\n<thead><tr><th>revision</th><th>event</th><th>detail</th></tr></thead>\n<tbody>\n");
    for record in &events.records {
        out.push_str(&format!(
            "<tr><td class=\"mono\">{}</td><td>{}</td><td class=\"mono\">{}</td></tr>\n",
            record.revision,
            escape(&event_name(&record.event)),
            escape(&event_detail(&record.event))
        ));
    }
    out.push_str("</tbody>\n</table>\n");
    out
}

fn event_name(event: &EventProjection) -> String {
    match event {
        EventProjection::RunStarted { .. } => "run_started",
        EventProjection::AttemptCompleted { .. } => "attempt_completed",
        EventProjection::AttemptUnknown { .. } => "attempt_unknown",
        EventProjection::AttemptReconciled { .. } => "attempt_reconciled",
        EventProjection::RetryStarted { .. } => "retry_started",
        EventProjection::CancellationRequested { .. } => "cancellation_requested",
        EventProjection::HoldChanged { .. } => "hold_changed",
    }
    .to_string()
}

fn event_detail(event: &EventProjection) -> String {
    match event {
        EventProjection::RunStarted {
            run_id,
            attempt_id,
            provider_profile,
        } => format!(
            "run {} attempt {} provider {}",
            run_id.as_str(),
            attempt_id.as_str(),
            provider_profile
                .as_ref()
                .map_or("-", |value| value.as_str())
        ),
        EventProjection::AttemptCompleted {
            attempt_id,
            outcome,
            external_reference,
            product_count,
            metric_count,
        } => format!(
            "attempt {} outcome {:?} products {} metrics {} external {}",
            attempt_id.as_str(),
            outcome,
            product_count,
            metric_count,
            external_reference
                .as_ref()
                .map_or_else(|| "-".to_string(), reference_text)
                .as_str()
        ),
        EventProjection::AttemptUnknown {
            attempt_id,
            external_reference,
        } => format!(
            "attempt {} external {}",
            attempt_id.as_str(),
            external_reference
                .as_ref()
                .map_or_else(|| "-".to_string(), reference_text)
                .as_str()
        ),
        EventProjection::AttemptReconciled {
            attempt_id,
            outcome,
            external_reference,
        } => format!(
            "attempt {} outcome {:?} external {}",
            attempt_id.as_str(),
            outcome,
            external_reference
                .as_ref()
                .map_or_else(|| "-".to_string(), reference_text)
                .as_str()
        ),
        EventProjection::RetryStarted {
            run_id,
            prior_attempt_id,
            next_attempt_id,
            provider_profile,
        } => format!(
            "run {} from {} to {} provider {}",
            run_id.as_str(),
            prior_attempt_id.as_str(),
            next_attempt_id.as_str(),
            provider_profile
                .as_ref()
                .map_or("-", |value| value.as_str())
        ),
        EventProjection::CancellationRequested { attempt_id } => {
            format!("attempt {}", attempt_id.as_str())
        }
        EventProjection::HoldChanged { scope, held } => format!(
            "scope {} held {}",
            scope
                .as_ref()
                .map_or_else(|| "-".to_string(), reference_text)
                .as_str(),
            held
        ),
    }
}

fn identities_section(report: &DashboardReport) -> String {
    let mut out = String::new();
    out.push_str("<table>\n<thead><tr><th>view</th><th class=\"plain\">kiln.observe.view.v1 identity</th></tr></thead>\n<tbody>\n");
    out.push_str(&identity_row(
        "state_summary",
        &report.views.summary.identity,
    ));
    for (index, view) in report.views.runs.views.iter().enumerate() {
        out.push_str(&identity_row(&format!("run_list[{index}]"), &view.identity));
    }
    if let Some(view) = report.views.run_detail.as_ref() {
        out.push_str(&identity_row("run_detail", &view.identity));
    }
    if let Some(view) = report.views.attempt_detail.as_ref() {
        out.push_str(&identity_row("attempt_detail", &view.identity));
    }
    out.push_str(&identity_row("event_tail", &report.views.events.identity));
    out.push_str("</tbody>\n</table>\n");
    out
}

fn identity_row(name: &str, identity: &kiln_core::types::ContentId) -> String {
    format!(
        "<tr><td class=\"mono\">{}</td><td class=\"mono\">{}</td></tr>\n",
        escape(name),
        escape(identity.as_str())
    )
}

fn redactions_section(report: &DashboardReport) -> String {
    let mut out = String::new();
    if report.redacted_fields.is_empty() {
        out.push_str("<p class=\"note\">no field is redacted by the admitted profile</p>\n");
        return out;
    }
    out.push_str("<p>");
    for field in &report.redacted_fields {
        out.push_str(&format!(
            "<span class=\"pill\">{}</span>",
            escape(field_label(*field))
        ));
    }
    out.push_str("</p>\n");
    out
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

// r[impl kiln.observe.dashboard.html.nonclaims.scenario.scope]
fn non_claims_section(report: &DashboardReport) -> String {
    let mut out = String::new();
    out.push_str("<ul class=\"claims\">\n");
    for non_claim in &report.non_claims {
        out.push_str(&format!("<li>{}</li>\n", escape(non_claim)));
    }
    out.push_str("</ul>\n");
    out
}

fn not_found_text(target: &NotFoundTarget) -> String {
    match target {
        NotFoundTarget::Run(run_id) => format!("run {} not found", run_id.as_str()),
        NotFoundTarget::Attempt(attempt_id) => {
            format!("attempt {} not found", attempt_id.as_str())
        }
        NotFoundTarget::Stream(stream_id) => format!("stream {} not found", stream_id.as_str()),
    }
}

fn outcome_class(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Success => "outcome-success",
        Outcome::Pending => "outcome-pending",
        Outcome::DomainFailure | Outcome::Denied | Outcome::ProviderError => "outcome-failure",
        Outcome::Cancelled | Outcome::TimedOut => "outcome-failure",
        Outcome::Unknown => "outcome-unknown",
    }
}

/// Escape the characters that carry meaning in HTML text and attributes.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}
