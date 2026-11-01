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
/// A trigger binds one exact branch or tag reference. Push filters must admit
/// that exact reference; pull requests bind branch heads and reject tag filters.
pub(crate) fn admit_trigger(p: &TriggerProfile) -> Result<(), String> {
    let invalid = |message: &str| format!("automation trigger {} filter invalid: {message}", p.id);
    let branch = p.reference.strip_prefix(BRANCH_PREFIX);
    let tag = p.reference.strip_prefix(trigger_filter::TAG_PREFIX);
    if p.pull_request {
        trigger_filter::admit_filter(EventKind::PullRequest, &p.filter)
            .map_err(|e| invalid(&e.to_string()))?;
    } else if p.push {
        trigger_filter::admit_filter(EventKind::Push, &p.filter)
            .map_err(|e| invalid(&e.to_string()))?;
    } else {
        trigger_filter::admit_filter(EventKind::PullRequest, &p.filter)
            .map_err(|e| invalid(&e.to_string()))?;
    }
    if p.push {
        if tag.is_some() && !p.filter.branches.is_empty() {
            return Err(invalid("branch filters never admit a tag reference"));
        }
        if branch.is_some() && !p.filter.tags.is_empty() {
            return Err(invalid("tag filters never admit a branch reference"));
        }
        if let Some(tag) = tag {
            if !p.filter.tags.is_empty() && !tag_matches(&p.filter.tags, tag) {
                return Err(invalid("tag filters never admit the bound tag reference"));
            }
        }
        if let Some(branch) = branch {
            if !p.filter.branches.is_empty() && !branch_matches(&p.filter.branches, branch) {
                return Err(invalid("branch filters never admit the bound reference"));
            }
        }
    }
    if p.pull_request && branch.is_none() {
        return Err(invalid("pull requests require a branch reference"));
    }
    Ok(())
}

fn tag_matches(patterns: &[String], tag: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| kiln_core::paths::matches(pattern, tag))
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
