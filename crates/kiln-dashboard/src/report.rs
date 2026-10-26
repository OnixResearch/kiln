use std::collections::BTreeSet;

use kiln_core::operations::JournalRecord;
use kiln_core::types::{AttemptId, CoreResult, RunId};
use kiln_observe_core::{
    ObservationRequest, ObservationView, ObservationWindow, ViewBody, ViewField, ViewRequest,
    project_observation,
};

use crate::journal::{AdmittedJournal, JournalFacts};
use crate::profile::DashboardProfile;

/// Operator selection of one run and one attempt.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DashboardSelection {
    pub run: Option<RunId>,
    pub attempt: Option<AttemptId>,
}

/// The paged run list consulted for one dashboard report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunPages {
    pub views: Vec<ObservationView>,
    pub total_runs: u64,
    pub requested_limit: u64,
    pub effective_limit: u64,
    pub capped: bool,
    pub has_more: bool,
    pub truncated: bool,
}

/// Every accepted observation view one dashboard report reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DashboardViews {
    pub summary: ObservationView,
    pub runs: RunPages,
    pub run_detail: Option<ObservationView>,
    pub attempt_detail: Option<ObservationView>,
    pub events: ObservationView,
}

/// One bounded operator dashboard report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DashboardReport {
    pub profile_id: String,
    pub observation_profile_id: String,
    pub journal: JournalFacts,
    pub views: DashboardViews,
    pub redacted_fields: BTreeSet<ViewField>,
    pub non_claims: Vec<String>,
}

/// Project one admitted journal into the accepted dashboard views.
///
/// Every reported fact comes from the accepted observation projection over the
/// admitted journal state. The dashboard adds no view vocabulary and no
/// projection logic of its own.
// r[impl kiln.observe.dashboard.projection]
pub fn build_report(
    profile: &DashboardProfile,
    journal: &AdmittedJournal,
    selection: &DashboardSelection,
) -> CoreResult<DashboardReport> {
    profile.admit()?;
    let observation = &profile.observation;
    let state = &journal.state;
    let revision = state.revision;
    let summary = project_observation(
        observation,
        state,
        &ObservationRequest {
            state_revision: revision,
            view: ViewRequest::StateSummary,
        },
        &ObservationWindow::empty(),
    )?;

    let mut views = Vec::new();
    let mut after: Option<RunId> = None;
    let mut total_runs = 0u64;
    let mut requested_limit = profile.run_page_limit;
    let mut effective_limit = 0u64;
    let mut capped = false;
    let mut has_more = false;
    let mut truncated = false;
    let mut first_run: Option<RunId> = None;
    let mut pages = 0u64;
    loop {
        // r[impl kiln.observe.dashboard.projection.scenario.parity]
        let page = project_observation(
            observation,
            state,
            &ObservationRequest {
                state_revision: revision,
                view: ViewRequest::RunList {
                    after: after.clone(),
                    limit: profile.run_page_limit,
                },
            },
            &ObservationWindow::empty(),
        )?;
        if let ViewBody::RunList(list) = &page.body {
            total_runs += list.runs.len() as u64;
            requested_limit = list.page.requested_limit;
            effective_limit = list.page.effective_limit;
            capped = list.page.capped;
            has_more = list.page.has_more;
            if first_run.is_none() {
                first_run = list.runs.first().map(|run| run.run_id.clone());
            }
        }
        views.push(page);
        pages += 1;
        let next = views.last().and_then(|view| match &view.body {
            ViewBody::RunList(list) => list.page.next.clone(),
            _ => None,
        });
        match (has_more, next) {
            (true, Some(next)) => {
                if pages >= profile.max_run_pages {
                    truncated = true;
                    break;
                }
                after = Some(next);
            }
            _ => break,
        }
    }

    let run_detail = match selection.run.clone().or(first_run) {
        // r[impl kiln.observe.dashboard.projection.scenario.unknown_target]
        Some(run_id) => Some(project_observation(
            observation,
            state,
            &ObservationRequest {
                state_revision: revision,
                view: ViewRequest::RunDetail { run_id },
            },
            &ObservationWindow::empty(),
        )?),
        None => None,
    };

    let selected_attempt = selection.attempt.clone().or_else(|| {
        run_detail.as_ref().and_then(|view| match &view.body {
            ViewBody::RunDetail(detail) => Some(detail.run.active_attempt.clone()),
            _ => None,
        })
    });
    let attempt_detail = match selected_attempt {
        Some(attempt_id) => Some(project_observation(
            observation,
            state,
            &ObservationRequest {
                state_revision: revision,
                view: ViewRequest::AttemptDetail { attempt_id },
            },
            &ObservationWindow::empty(),
        )?),
        None => None,
    };

    let max_records = usize::try_from(observation.bounds.max_event_records).unwrap_or(usize::MAX);
    let wanted = usize::try_from(profile.event_limit)
        .unwrap_or(usize::MAX)
        .min(max_records);
    let start = journal.records.len().saturating_sub(wanted);
    let window = journal
        .records
        .get(start..)
        .map(<[JournalRecord]>::to_vec)
        .unwrap_or_default();
    let events = project_observation(
        observation,
        state,
        &ObservationRequest {
            state_revision: revision,
            view: ViewRequest::EventTail {
                limit: profile.event_limit,
            },
        },
        &ObservationWindow::events(window),
    )?;

    let mut redacted_fields = BTreeSet::new();
    redacted_fields.extend(summary.redacted_fields.iter().copied());
    for view in &views {
        redacted_fields.extend(view.redacted_fields.iter().copied());
    }
    for view in run_detail.iter().chain(attempt_detail.iter()) {
        redacted_fields.extend(view.redacted_fields.iter().copied());
    }
    redacted_fields.extend(events.redacted_fields.iter().copied());

    Ok(DashboardReport {
        profile_id: profile.profile_id.clone(),
        observation_profile_id: observation.profile_id.clone(),
        journal: JournalFacts {
            schema: journal.schema.clone(),
            declared_identity: journal.declared_identity.clone(),
            record_count: journal.records.len() as u64,
            // r[impl kiln.observe.dashboard.journal.scenario.truncated_prefix]
            state_revision: revision,
        },
        views: DashboardViews {
            summary,
            runs: RunPages {
                views,
                total_runs,
                requested_limit,
                effective_limit,
                capped,
                has_more,
                truncated,
            },
            run_detail,
            attempt_detail,
            events,
        },
        redacted_fields,
        non_claims: profile.non_claims.clone(),
    })
}

/// HTML-only drill-downs in addition to the base report's selected views.
///
/// Run details cover the admitted run pages. Attempt details cover each
/// displayed run's bounded attempt list and active attempt. Explicit CLI
/// selections remain in `report`, including admitted not-found results.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmlReport {
    pub report: DashboardReport,
    pub runs: Vec<ObservationView>,
    pub attempts: Vec<ObservationView>,
}

/// Project the bounded offline navigation surface without adding work to text output.
// r[impl kiln.observe.dashboard.html.interactive]
// r[impl kiln.observe.dashboard.html.interactive.scenario.bounded]
pub fn build_html_report(
    profile: &DashboardProfile,
    journal: &AdmittedJournal,
    selection: &DashboardSelection,
) -> CoreResult<HtmlReport> {
    let mut report = build_report(profile, journal, selection)?;
    let selected_run = report
        .views
        .run_detail
        .as_ref()
        .and_then(|view| match &view.body {
            ViewBody::RunDetail(detail) => Some(&detail.run.run_id),
            _ => None,
        });
    let run_ids = report
        .views
        .runs
        .views
        .iter()
        .filter_map(|view| match &view.body {
            ViewBody::RunList(list) => Some(&list.runs),
            _ => None,
        })
        .flatten()
        .map(|run| &run.run_id);
    let mut runs = Vec::new();
    for run_id in run_ids {
        if Some(run_id) == selected_run {
            continue;
        }
        runs.push(project_observation(
            &profile.observation,
            &journal.state,
            &ObservationRequest {
                state_revision: journal.state.revision,
                view: ViewRequest::RunDetail {
                    run_id: run_id.clone(),
                },
            },
            &ObservationWindow::empty(),
        )?);
    }

    let selected_attempt = report
        .views
        .attempt_detail
        .as_ref()
        .and_then(|view| match &view.body {
            ViewBody::AttemptDetail(detail) => Some(&detail.attempt_id),
            _ => None,
        });
    let mut attempt_ids = BTreeSet::new();
    for view in report.views.run_detail.iter().chain(&runs) {
        if let ViewBody::RunDetail(detail) = &view.body {
            attempt_ids.insert(&detail.run.active_attempt);
            attempt_ids.extend(detail.attempts.iter().map(|attempt| &attempt.attempt_id));
        }
    }
    let mut attempts = Vec::new();
    for attempt_id in attempt_ids {
        if Some(attempt_id) == selected_attempt {
            continue;
        }
        attempts.push(project_observation(
            &profile.observation,
            &journal.state,
            &ObservationRequest {
                state_revision: journal.state.revision,
                view: ViewRequest::AttemptDetail {
                    attempt_id: attempt_id.clone(),
                },
            },
            &ObservationWindow::empty(),
        )?);
    }
    for view in runs.iter().chain(&attempts) {
        report
            .redacted_fields
            .extend(view.redacted_fields.iter().copied());
    }
    Ok(HtmlReport {
        report,
        runs,
        attempts,
    })
}
