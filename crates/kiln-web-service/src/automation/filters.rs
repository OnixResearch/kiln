//! Pure trigger-filter and cron-schedule helpers for web automation. The
//! engine shell supplies events, changed files and clock readings.

use kiln_adapter_github::events::{EventAdmission, EventPolicy, EventSelector};
use kiln_core::monorepo::ChangedFiles;
use kiln_core::trigger_filter::{
    self, BRANCH_PREFIX, EventKind, FilterDecision, PullRequestType, TriggerEvent,
};

use super::{RegistrationState, ScheduleProfile, TriggerProfile};
use crate::schedule::{self, Cron};

/// Domain of the `H` identity of a web automation schedule.
const SCHEDULE_IDENTITY_DOMAIN: &str = "kiln.web-automation.schedule.v1";

fn branch_matches(patterns: &[String], branch: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| kiln_core::paths::matches(pattern, branch))
}

// r[impl kiln.triggers.filter]
/// Registrations bind one exact `refs/heads` reference, so `tags` never apply;
/// a push filter must admit that reference and `types` need pull requests.
pub(crate) fn admit_trigger(p: &TriggerProfile) -> Result<(), String> {
    let invalid = |message: &str| format!("automation trigger {} filter invalid: {message}", p.id);
    trigger_filter::admit_filter(EventKind::PullRequest, &p.filter)
        .map_err(|e| invalid(&e.to_string()))?;
    let branch = p.reference.strip_prefix(BRANCH_PREFIX).unwrap_or_default();
    if p.push && !p.filter.branches.is_empty() && !branch_matches(&p.filter.branches, branch) {
        return Err(invalid(
            "push branches never admit the bound reference; use a pull-request-only registration",
        ));
    }
    if !p.pull_request && !p.filter.types.is_empty() {
        return Err(invalid("types apply only to pull requests"));
    }
    Ok(())
}

/// Whether a registration admits a pull request into `base`: its exact
/// `pull_request_base`, or, when that is absent, any base its `branches` match.
pub(crate) fn base_admitted(p: &TriggerProfile, base: &str) -> bool {
    match &p.pull_request_base {
        Some(exact) => exact == base,
        None => !p.filter.branches.is_empty() && branch_matches(&p.filter.branches, base),
    }
}

/// Event selection shared by the receiving registration and its siblings:
/// enabled event kind, exact bound head reference and an admitted base.
pub(crate) fn selects(
    p: &TriggerProfile,
    state: &RegistrationState,
    selector: &EventSelector,
) -> bool {
    match selector {
        EventSelector::Push { reference } => state.push && &p.reference == reference,
        EventSelector::PullRequest {
            head_reference,
            base_reference,
            ..
        } => {
            state.pull_request
                && p.reference.strip_prefix(BRANCH_PREFIX) == Some(head_reference.as_str())
                && base_admitted(p, base_reference)
        }
    }
}

/// Closed pull requests are admitted only when a registration on this
/// repository asked for `closed` or `merged`.
pub(crate) fn admission(triggers: &[TriggerProfile], repository: &EventPolicy) -> EventAdmission {
    EventAdmission {
        closed_pull_requests: triggers
            .iter()
            .any(|t| &t.repository == repository && t.pull_request && t.filter.admits_closed()),
    }
}

/// Whether any registration of an event group needs exact changed files.
pub(crate) fn needs_changed_files(group: &[TriggerProfile]) -> bool {
    group.iter().any(|t| t.filter.needs_changed_files())
}

// r[impl kiln.triggers.evaluate]
/// The filter decision of one registration for one event. An unfiltered
/// registration that runs records nothing, so its source record is unchanged.
pub(crate) fn decide(
    p: &TriggerProfile,
    selector: &EventSelector,
    action: Option<PullRequestType>,
    changed: &ChangedFiles,
) -> Result<Option<FilterDecision>, String> {
    let event = match selector {
        EventSelector::Push { reference } => TriggerEvent::Push { reference },
        EventSelector::PullRequest { base_reference, .. } => TriggerEvent::PullRequest {
            action: action.ok_or("pull request action missing")?,
            base_branch: base_reference,
        },
    };
    let decision = trigger_filter::evaluate(event.kind(), &p.filter, &event, changed);
    Ok(match decision {
        FilterDecision::Run { .. } if p.filter.is_empty() => None,
        decision => Some(decision),
    })
}

/// The compiled calendar of a cron schedule; `H` depends only on the
/// project and schedule identity.
pub(crate) fn cron(p: &ScheduleProfile) -> Result<Option<Cron>, String> {
    p.cron
        .as_ref()
        .map(|spec| {
            schedule::compile(spec, &[SCHEDULE_IDENTITY_DOMAIN, &p.project_id, &p.id])
                .map_err(|e| format!("automation schedule {} cron invalid: {e}", p.id))
        })
        .transpose()
}

// r[impl kiln.schedules.cron]
// r[impl kiln.schedules.min_interval]
/// Admit a cron schedule: valid expression and time zone, a minimum interval
/// at or above the cron floor, and no pair of occurrences closer than it
/// across the bounded review horizon from `now_ms`.
pub(crate) fn admit_cron(p: &ScheduleProfile, now_ms: u64) -> Result<(), String> {
    if p.next_due_ms.is_some() {
        return Err(format!(
            "automation schedule {} computes its next occurrence; next_due_ms must be null",
            p.id
        ));
    }
    if p.min_interval_ms < schedule::CRON_MIN_INTERVAL_FLOOR_MS {
        return Err(format!(
            "automation schedule {} minimum interval is below the cron floor",
            p.id
        ));
    }
    let compiled = cron(p)?.ok_or("automation schedule cron missing")?;
    compiled
        .check_min_interval(now_ms, p.min_interval_ms)
        .map_err(|e| format!("automation schedule {} rejected: {e}", p.id))
}
