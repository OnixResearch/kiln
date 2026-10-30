//! Tangled `when` conditions as #13 trigger filters.
//!
//! Each condition becomes one `kiln_core::trigger_filter` filter per event
//! kind; a workflow runs when any rule matches, as in Tangled. Evaluation is
//! `kiln_core::trigger_filter::evaluate`, so globs, the branch/tag push rule,
//! default pull request types and unknown changed files behave exactly as for
//! every other Kiln trigger.

use kiln_core::monorepo::ChangedFiles;
pub use kiln_core::trigger_filter::{EventKind, PullRequestType, TriggerEvent, TriggerFilter};
use kiln_core::trigger_filter::{FilterDecision, evaluate};
use serde::{Deserialize, Serialize};

/// One `when` condition for one event kind.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerRule {
    pub event: EventKind,
    #[serde(default)]
    pub filter: TriggerFilter,
}

/// A Tangled pull request `types` value.
#[must_use]
pub fn pull_request_type(value: &str) -> Option<PullRequestType> {
    Some(match value {
        "opened" => PullRequestType::Opened,
        "reopened" => PullRequestType::Reopened,
        "synchronize" => PullRequestType::Synchronize,
        "closed" => PullRequestType::Closed,
        "merged" => PullRequestType::Merged,
        _ => return None,
    })
}

// r[impl kiln.tangled.workflows]
#[must_use]
pub fn rule_matches(rule: &TriggerRule, event: &TriggerEvent<'_>, changed: &ChangedFiles) -> bool {
    matches!(
        evaluate(rule.event, &rule.filter, event, changed),
        FilterDecision::Run { .. }
    )
}

#[must_use]
pub fn any_rule_matches(
    rules: &[TriggerRule],
    event: &TriggerEvent<'_>,
    changed: &ChangedFiles,
) -> bool {
    rules.iter().any(|rule| rule_matches(rule, event, changed))
}
