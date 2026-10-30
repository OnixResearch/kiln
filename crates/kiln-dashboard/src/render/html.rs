//! The HTML rendering of one dashboard report.
//!
//! The document is self-contained: fixed inline style and script, no external
//! resource, and no clock or environment input, so the same admitted state
//! renders the same bytes every time. Interactions only select already admitted
//! views; their canonical identities, redactions and bounds remain authoritative.

use kiln_core::domain::Outcome;
use kiln_observe_core::{
    AttemptDetailView, Distribution, EventProjection, EventTailView, NotFoundTarget,
    ObservationView, ReviewFrictionView, RunDetailView, StateSummaryView, SuggestionAnchorsView,
    ViewBody, ViewField,
};

use crate::journal::reference_text;
use crate::render::{
    AUTHORSHIP_BASIS_TEXT, anchor_owner, anchor_path, authorship_label, distribution_value,
};
use crate::report::{DashboardReport, HtmlReport, ReviewViews};

/// Render one dashboard report as a self-contained HTML document.
// r[impl kiln.observe.dashboard.html]
// r[impl kiln.observe.dashboard.html.scenario.parity]
// r[impl kiln.observe.dashboard.html.scenario.determinism]
// r[impl kiln.observe.dashboard.html.scenario.self_contained]
// r[impl kiln.observe.dashboard.html.scenario.escaping]
// r[impl kiln.observe.dashboard.html.nonclaims]
#[must_use]
pub fn render_html(html: &HtmlReport) -> String {
    render_html_with_refresh(html, None)
}

/// Render one dashboard report that reloads at the given admitted interval.
///
/// The fixed script reloads the current URL, preserving interactive fragment
/// state. A document-level instruction provides the no-JavaScript fallback.
// r[impl kiln.observe.dashboard.html.live_update]
// r[impl kiln.observe.dashboard.html.live_update.scenario.match]
// r[impl kiln.observe.dashboard.html.live_update.scenario.snapshot]
// r[impl kiln.observe.dashboard.html.live_update.nonclaims]
// r[impl kiln.observe.dashboard.html.live_update.nonclaims.scenario.scope]
#[must_use]
pub fn render_html_with_refresh(html: &HtmlReport, refresh_seconds: Option<u64>) -> String {
    let report = &html.report;
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    if let Some(seconds) = refresh_seconds {
        out.push_str(&format!(
            "<meta name=\"kiln-refresh-seconds\" content=\"{seconds}\">\n<noscript><meta http-equiv=\"refresh\" content=\"{seconds}\"></noscript>\n"
        ));
    }
    out.push_str(&format!(
        "<title>Kiln dashboard {}</title>\n<style>\n",
        escape(&report.profile_id)
    ));
    out.push_str(STYLE);
    out.push_str(r##"</style>
</head>
<body>
<a class="skip-link" href="#main-content">Skip to dashboard</a>
<div class="app-shell">
  <aside class="sidebar" aria-label="Kiln dashboard">
    <div class="brand"><span class="brand-mark" aria-hidden="true">K</span><span>Kiln<span class="brand-caption">CI control plane</span></span></div>
    <p class="nav-label">Workspace</p>
    <nav class="section-nav js-only" aria-label="Dashboard sections">
      <button type="button" data-jump="run-workspace"><span aria-hidden="true">01</span> Runs</button>
      <button type="button" data-jump="events"><span aria-hidden="true">02</span> Activity</button>
      <button type="button" data-jump="evidence"><span aria-hidden="true">03</span> Evidence</button>
    </nav>
    <div class="sidebar-foot">
      <span class="local-indicator">Local observation</span>
      <p>No provider effects.<br>No CI control authority.</p>
    </div>
  </aside>
  <div class="main-shell">
    <header class="topbar">
      <p class="breadcrumb">Workspace <span aria-hidden="true">/</span> <strong>Overview</strong></p>
      <div class="topbar-actions">
        <span class="pill">Read-only</span>
        <button type="button" id="theme-toggle" class="js-only quiet-button">Theme: system</button>
        <button type="button" id="shortcut-help" class="js-only quiet-button" aria-label="Keyboard shortcuts"><kbd>?</kbd></button>
      </div>
    </header>
    <main id="main-content" tabindex="-1">
      <header class="page-heading">
        <div>
          <p class="eyebrow">Continuous integration</p>
          <h1>Run overview</h1>
          <p class="subtitle">Runs, attempts and outcomes from one admitted journal.</p>
        </div>
        <div class="report-mode">
"##);
    match refresh_seconds {
        Some(seconds) => out.push_str(&format!(
            "<span class=\"mode\">Watched local report</span><span class=\"note\">Refresh every {seconds} seconds</span>"
        )),
        None => out.push_str("<span class=\"mode\">Snapshot</span><span class=\"note\">No automatic refresh</span>"),
    }
    out.push_str("</div></header>\n<div class=\"report-context\">");
    out.push_str(&format!(
        "<span>Profile <strong class=\"mono\">{}</strong></span><span>Observation <strong class=\"mono\">{}</strong></span><span>state revision <strong>{}</strong></span><span><strong>{}</strong> journal records</span>",
        escape(&report.profile_id),
        escape(&report.observation_profile_id),
        report.journal.state_revision,
        report.journal.record_count
    ));
    out.push_str("</div>\n");
    if let ViewBody::StateSummary(summary) = &report.views.summary.body {
        out.push_str(&summary_section(summary));
    }
    out.push_str("<div class=\"workspace\" id=\"run-workspace\">\n<section class=\"run-list\" aria-labelledby=\"runs-heading\"><div class=\"section-heading\"><div><p class=\"eyebrow\">Admitted history</p><h2 id=\"runs-heading\">Runs</h2></div><span class=\"pill\">Local only</span></div>\n");
    out.push_str(&runs_section(report));
    out.push_str("</section>\n<aside class=\"inspector\" aria-label=\"Selected run and attempt\">\n<section id=\"run-inspector\" aria-labelledby=\"run-detail-heading\"><div class=\"section-heading\"><div><p class=\"eyebrow\">Inspector</p><h2 id=\"run-detail-heading\">Run detail</h2></div><span class=\"inspector-mark\" aria-hidden=\"true\">↗</span></div>\n<p id=\"selection-status\" class=\"js-only selection-note\" role=\"status\"></p>\n");
    for (index, view) in report
        .views
        .run_detail
        .iter()
        .chain(html.runs.iter())
        .enumerate()
    {
        out.push_str(&detail_panel(
            view,
            true,
            index == 0 && report.views.run_detail.is_some(),
        ));
    }
    out.push_str("</section>\n<section id=\"attempt-inspector\" aria-labelledby=\"attempt-detail-heading\"><div class=\"section-heading\"><div><p class=\"eyebrow\">Execution</p><h2 id=\"attempt-detail-heading\">Attempt detail</h2></div></div>\n<p id=\"attempt-selection-status\" class=\"js-only selection-note\" role=\"status\"></p>\n");
    for (index, view) in report
        .views
        .attempt_detail
        .iter()
        .chain(html.attempts.iter())
        .enumerate()
    {
        out.push_str(&detail_panel(
            view,
            false,
            index == 0 && report.views.attempt_detail.is_some(),
        ));
    }
    out.push_str("</section>\n</aside>\n</div>\n");
    if let ViewBody::EventTail(events) = &report.views.events.body {
        out.push_str(&section(
            "events",
            "Recent activity",
            &events_section(events),
        ));
    }
    if let Some(review) = report.views.review.as_ref() {
        if let ViewBody::ReviewFriction(friction) = &review.friction.body {
            out.push_str(&section(
                "review-friction",
                "Review friction",
                &review_friction_section(friction),
            ));
        }
        if let ViewBody::SuggestionAnchors(anchors) = &review.anchors.body {
            out.push_str(&section(
                "suggestion-anchors",
                "Suggestion anchors",
                &suggestion_anchors_section(anchors, review),
            ));
        }
    }
    out.push_str("<section id=\"evidence\" aria-labelledby=\"evidence-heading\"><div class=\"section-heading\"><div><p class=\"eyebrow\">Trust boundaries</p><h2 id=\"evidence-heading\">Evidence &amp; provenance</h2></div><span class=\"pill\">Admitted facts, not authority</span></div><p class=\"note\">Local admitted observations only. Filtering and selection do not run jobs or change canonical state.</p>\n<details class=\"disclosure\"><summary>Report provenance</summary><dl class=\"facts\">\n");
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
    out.push_str("</dl></details>\n<details class=\"disclosure\"><summary>Canonical view identities</summary>\n");
    out.push_str(&identities_section(html));
    out.push_str(
        "</details>\n<details class=\"disclosure\"><summary>Profile redactions</summary>\n",
    );
    out.push_str(&redactions_section(report));
    out.push_str(
        "</details>\n<details class=\"disclosure\"><summary>Scope &amp; non-claims</summary>\n",
    );
    out.push_str(&non_claims_section(report));
    out.push_str("</details>\n</section>\n<footer class=\"page-footer\"><span>Kiln / local observation surface</span><span>Bounded evidence. Explicit authority.</span></footer>\n</main>\n</div>\n</div>\n<dialog id=\"keyboard-help\" aria-labelledby=\"keyboard-help-heading\"><div class=\"section-heading\"><h2 id=\"keyboard-help-heading\">Keyboard shortcuts</h2><button type=\"button\" id=\"close-shortcuts\" aria-label=\"Close keyboard shortcuts\">Close</button></div><dl class=\"shortcut-list\"><dt><kbd>/</kbd></dt><dd>Focus run search</dd><dt><kbd>Esc</kbd></dt><dd>Clear focused search or close this help</dd><dt><kbd>?</kbd></dt><dd>Show keyboard shortcuts</dd><dt><kbd>Tab</kbd></dt><dd>Move through controls and run selections</dd><dt><kbd>Enter</kbd></dt><dd>Activate a focused selection or disclosure</dd></dl><p class=\"note\">Shortcuts never intercept typing in editable fields.</p></dialog>\n<script>\n");
    out.push_str(SCRIPT);
    out.push_str("\n</script>\n</body>\n</html>\n");
    out
}

// r[impl kiln.observe.dashboard.html.console]
// r[impl kiln.observe.dashboard.html.console.scenario.responsive]
const STYLE: &str = include_str!("html.css");
// r[impl kiln.observe.dashboard.html.interactive.scenario.navigation]
// r[impl kiln.observe.dashboard.html.console.scenario.keyboard]
const SCRIPT: &str = include_str!("html.js");

fn push_fact(out: &mut String, label: &str, value: &str) {
    out.push_str(&format!(
        "<dt>{}</dt><dd>{}</dd>\n",
        escape(label),
        escape(value)
    ));
}

fn section(id: &str, title: &str, body: &str) -> String {
    format!(
        "<section id=\"{}\" aria-labelledby=\"{}-heading\">\n<div class=\"section-heading\"><h2 id=\"{}-heading\">{}</h2></div>\n{}\n</section>\n",
        escape(id),
        escape(id),
        escape(id),
        escape(title),
        body
    )
}

fn summary_section(summary: &StateSummaryView) -> String {
    let mut out = String::from(
        "<section class=\"overview\" aria-label=\"Admitted state summary\"><dl class=\"stat-grid\">",
    );
    for (label, value, note) in [
        ("Total runs", summary.run_count, "In admitted state"),
        ("Attempts", summary.attempt_count, "Including retry history"),
        ("Holds", summary.hold_count, "Recorded admission holds"),
    ] {
        out.push_str(&format!(
            "<div class=\"stat-card\"><dt>{label}</dt><dd>{value}<span>{note}</span></dd></div>"
        ));
    }
    out.push_str("</dl><div class=\"outcome-summary\"><div><h3>Run outcomes</h3>");
    out.push_str(&outcome_counts(&summary.run_outcomes));
    out.push_str("</div><div><h3>Attempt outcomes</h3>");
    out.push_str(&outcome_counts(&summary.attempt_outcomes));
    out.push_str("</div></div></section>\n");
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
    out.push_str("<div class=\"controls js-only\" role=\"group\" aria-label=\"Filter and sort admitted runs\">\n<label for=\"run-search\">Search runs<input id=\"run-search\" type=\"search\" placeholder=\"Run ID or active attempt ID\" aria-describedby=\"run-search-help\"></label>\n<label for=\"outcome-filter\">Outcome<select id=\"outcome-filter\"><option value=\"\">All outcomes</option>");
    for outcome in [
        "Success",
        "Pending",
        "DomainFailure",
        "Denied",
        "ProviderError",
        "Cancelled",
        "TimedOut",
        "Unknown",
    ] {
        out.push_str(&format!("<option value=\"{outcome}\">{outcome}</option>"));
    }
    out.push_str("</select></label>\n<label for=\"run-sort\">Sort by<select id=\"run-sort\"><option value=\"id\">Run ID</option><option value=\"outcome\">Outcome</option><option value=\"attempts\">Attempt count</option></select></label>\n<label for=\"sort-direction\">Direction<select id=\"sort-direction\"><option value=\"asc\">Ascending</option><option value=\"desc\">Descending</option></select></label>\n<button type=\"button\" id=\"clear-filters\">Clear filters</button>\n</div>\n<p id=\"run-search-help\" class=\"note js-only\">Search run or active attempt IDs. Only embedded rows are filtered.</p>\n<p id=\"run-result-count\" class=\"js-only\" role=\"status\"></p>\n<p id=\"run-empty\" hidden>No admitted runs match these filters. Clear filters to see all admitted rows.</p>\n<noscript><p class=\"note\">JavaScript is disabled: all admitted rows and detail views are shown below. Search and selection require JavaScript; no facts are fetched.</p></noscript>\n");
    out.push_str("<div class=\"table-scroll\" tabindex=\"0\" role=\"region\" aria-label=\"Admitted runs\"><table id=\"runs-table\">\n<thead><tr><th scope=\"col\">run</th><th scope=\"col\">outcome</th><th scope=\"col\">active attempt</th><th scope=\"col\">attempts</th></tr></thead>\n<tbody>\n");
    for view in &runs.views {
        let ViewBody::RunList(list) = &view.body else {
            continue;
        };
        for run in &list.runs {
            out.push_str(&format!(
                "<tr data-run-row data-run-id=\"{id}\" data-active-attempt=\"{active}\" data-outcome=\"{outcome}\" data-attempt-count=\"{count}\"><td class=\"mono\"><span class=\"no-js-only\">{id}</span><button class=\"identity-button js-only\" type=\"button\" data-select-run data-run-id=\"{id}\" aria-label=\"Select run {id}\">{id}</button></td><td class=\"outcome {class}\">{outcome}</td><td class=\"mono\"><span class=\"no-js-only\">{active}</span><button class=\"identity-button js-only\" type=\"button\" data-select-attempt data-run-id=\"{id}\" data-attempt-id=\"{active}\" aria-label=\"Select attempt {active} for run {id}\">{active}</button></td><td>{count}</td></tr>\n",
                id = escape(run.run_id.as_str()),
                active = escape(run.active_attempt.as_str()),
                outcome = escape(&format!("{:?}", run.outcome)),
                class = outcome_class(run.outcome),
                count = run.attempt_count
            ));
        }
    }
    out.push_str("</tbody>\n</table></div>\n");
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
    if runs.total_runs == 0 {
        out.push_str("<p class=\"note\">No runs are present in this admitted observation.</p>\n");
    }
    out
}

fn detail_panel(view: &ObservationView, run: bool, initial: bool) -> String {
    let (target, owner, active, found) = match &view.body {
        ViewBody::RunDetail(detail) => (
            detail.run.run_id.as_str(),
            detail.run.run_id.as_str(),
            detail.run.active_attempt.as_str(),
            true,
        ),
        ViewBody::AttemptDetail(detail) => {
            (detail.attempt_id.as_str(), detail.run_id.as_str(), "", true)
        }
        ViewBody::NotFound(detail) => match &detail.target {
            NotFoundTarget::Run(id) if run => (id.as_str(), id.as_str(), "", false),
            NotFoundTarget::Attempt(id) if !run => (id.as_str(), "", "", false),
            _ => return String::new(),
        },
        _ => return String::new(),
    };
    let attributes = if run {
        format!(
            "data-run-detail data-run-id=\"{}\" data-active-attempt=\"{}\"",
            escape(target),
            escape(active)
        )
    } else {
        format!(
            "data-attempt-detail data-attempt-id=\"{}\" data-run-id=\"{}\"",
            escape(target),
            escape(owner)
        )
    };
    let mut out = format!(
        "<article class=\"detail-panel\" {attributes} data-found=\"{found}\"{}>\n<h3>{} <span class=\"mono\">{}</span></h3>\n",
        if initial { " data-initial" } else { "" },
        if run { "Run" } else { "Attempt" },
        escape(target)
    );
    out.push_str(&if run {
        run_detail_section(view)
    } else {
        attempt_detail_section(view)
    });
    out.push_str("<dl class=\"facts view-identity\">\n");
    push_fact(
        &mut out,
        "kiln.observe.view.v1 identity",
        view.identity.as_str(),
    );
    out.push_str("</dl>\n");
    out.push_str("</article>\n");
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
            "<tr><td class=\"mono\"><span class=\"no-js-only\">{id}</span><button class=\"identity-button js-only\" type=\"button\" data-select-attempt data-attempt-id=\"{id}\" data-run-id=\"{run}\" aria-label=\"Select attempt {id} for run {run}\">{id}</button></td><td class=\"outcome {class}\">{outcome}</td><td class=\"mono\">{provider}</td><td>{products}</td><td>{metrics}</td></tr>\n",
            id = escape(attempt.attempt_id.as_str()),
            run = escape(detail.run.run_id.as_str()),
            class = outcome_class(attempt.outcome),
            outcome = escape(&format!("{:?}", attempt.outcome)),
            provider = escape(
                attempt
                    .provider_profile
                    .as_ref()
                    .map_or("-", |value| value.as_str())
            ),
            products = attempt.product_count,
            metrics = attempt.metric_count
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

fn spread(distribution: &Distribution) -> String {
    format!(
        "{} / {} / {}",
        distribution_value(distribution.min),
        distribution_value(distribution.median),
        distribution_value(distribution.max)
    )
}

fn review_friction_section(friction: &ReviewFrictionView) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<p class=\"note\">Basis: {}. These figures observe one admitted review window and carry no authority.</p>\n<p><span class=\"pill\">window ms {}..{}</span></p>\n",
        escape(AUTHORSHIP_BASIS_TEXT),
        friction.start_ms,
        friction.end_ms
    ));
    out.push_str("<div class=\"table-scroll\" tabindex=\"0\" role=\"region\" aria-label=\"Review friction by declared authorship\"><table>\n<thead><tr><th scope=\"col\">authorship</th><th scope=\"col\">changes</th><th scope=\"col\">approved</th><th scope=\"col\">approval latency ms (min / median / max)</th><th scope=\"col\">patch sets before approval (min / median / max)</th><th scope=\"col\">suggestions per change (total / median / max)</th><th scope=\"col\">merged</th><th scope=\"col\">merged without suggestion</th><th scope=\"col\">share per mille</th></tr></thead>\n<tbody>\n");
    for class in &friction.classes {
        out.push_str(&format!(
            "<tr><th scope=\"row\">{}</th><td>{}</td><td>{}</td><td class=\"mono\">{}</td><td class=\"mono\">{}</td><td class=\"mono\">{} / {} / {}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            escape(authorship_label(class.authorship)),
            class.changes,
            class.approved_changes,
            spread(&class.approval_latency_ms),
            spread(&class.patch_sets_before_approval),
            class.suggestions_per_change.total,
            distribution_value(class.suggestions_per_change.median),
            distribution_value(class.suggestions_per_change.max),
            class.merged_changes,
            class.merged_without_suggestion,
            distribution_value(class.merged_without_suggestion_per_mille)
        ));
    }
    out.push_str("</tbody>\n</table></div>\n");
    out
}

fn suggestion_anchors_section(anchors: &SuggestionAnchorsView, review: &ReviewViews) -> String {
    let page = &anchors.page;
    let mut out = String::new();
    out.push_str("<p>");
    out.push_str(&format!(
        "<span class=\"pill\">cursor {}</span><span class=\"pill\">returned {}</span><span class=\"pill\">total {}</span><span class=\"pill\">requested {}</span><span class=\"pill\">effective {}</span>",
        page.cursor, page.returned, page.total, page.requested_limit, page.effective_limit
    ));
    if page.capped {
        out.push_str("<span class=\"pill\">capped</span>");
    }
    if page.has_more {
        out.push_str("<span class=\"pill\">more</span>");
    }
    if let Some(next) = page.next {
        out.push_str(&format!("<span class=\"pill\">next {next}</span>"));
    }
    out.push_str("</p>\n");
    out.push_str("<div class=\"table-scroll\" tabindex=\"0\" role=\"region\" aria-label=\"Suggestion anchors\"><table>\n<thead><tr><th scope=\"col\">path</th><th scope=\"col\">owner</th><th scope=\"col\">suggestions</th><th scope=\"col\">changes</th></tr></thead>\n<tbody>\n");
    for anchor in &anchors.anchors {
        out.push_str(&format!(
            "<tr><td class=\"mono\">{}</td><td class=\"mono\">{}</td><td>{}</td><td>{}</td></tr>\n",
            escape(anchor_path(anchor)),
            escape(anchor_owner(anchor, review)),
            anchor.suggestions,
            anchor.changes
        ));
    }
    out.push_str("</tbody>\n</table></div>\n");
    if page.total == 0 {
        out.push_str("<p class=\"note\">No anchored suggestions lie in this review window.</p>\n");
    }
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

fn identities_section(html: &HtmlReport) -> String {
    let report = &html.report;
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
    for (index, view) in html.runs.iter().enumerate() {
        out.push_str(&identity_row(
            &format!("run_detail.additional[{index}]"),
            &view.identity,
        ));
    }
    for (index, view) in html.attempts.iter().enumerate() {
        out.push_str(&identity_row(
            &format!("attempt_detail.additional[{index}]"),
            &view.identity,
        ));
    }
    out.push_str(&identity_row("event_tail", &report.views.events.identity));
    if let Some(review) = report.views.review.as_ref() {
        out.push_str(&identity_row("review_friction", &review.friction.identity));
        out.push_str(&identity_row(
            "suggestion_anchors",
            &review.anchors.identity,
        ));
    }
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
        ViewField::SuggestionPaths => "suggestion_paths",
        ViewField::SuggestionOwners => "suggestion_owners",
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
