//! Bounded CI trigger filters: branch and tag globs, pull request action types
//! and changed-path globs.
//!
//! Hosted pipelines and web automation share this pure decision. Globs use
//! [`crate::paths::matches`] (the review-ownership and monorepo-scope matcher);
//! changed files are the exact-or-fallback [`ChangedFiles`] facts. Unknown
//! changed files never skip a trigger: evaluation fails toward running.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize};

use crate::monorepo::{ChangedFallback, ChangedFiles};
use crate::paths;

/// Patterns per filter list (`branches`, `tags`, `paths`).
pub const MAX_FILTER_PATTERNS: usize = 32;
/// Bytes per filter pattern.
pub const MAX_FILTER_PATTERN_BYTES: usize = 256;
/// Namespace whose suffix `branches` patterns match.
pub const BRANCH_PREFIX: &str = "refs/heads/";
/// Namespace whose suffix `tags` patterns match.
pub const TAG_PREFIX: &str = "refs/tags/";

/// Source event kinds a trigger may name.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Manual,
    Push,
    PullRequest,
    Schedule,
}

impl EventKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Push => "push",
            Self::PullRequest => "pull_request",
            Self::Schedule => "schedule",
        }
    }
}

/// Pull request actions. `merged` is a pull request closed by merging;
/// `closed` is one closed without merging.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestType {
    Opened,
    Reopened,
    Synchronize,
    Closed,
    Merged,
}

/// Actions an empty `types` list admits: today's pull request behaviour.
pub const DEFAULT_PULL_REQUEST_TYPES: [PullRequestType; 3] = [
    PullRequestType::Opened,
    PullRequestType::Reopened,
    PullRequestType::Synchronize,
];

/// Optional bounded filters; every empty list is omitted from serialized bytes,
/// so an unfiltered trigger keeps its previous encoding.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerFilter {
    /// Branch globs: the pushed branch, or a pull request's base branch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<String>,
    /// Tag globs for pushes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Pull request actions; empty means [`DEFAULT_PULL_REQUEST_TYPES`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<PullRequestType>,
    /// Changed-path globs; any changed path (or rename old path) may match.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

impl TriggerFilter {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
            && self.tags.is_empty()
            && self.types.is_empty()
            && self.paths.is_empty()
    }

    /// Whether evaluation needs changed-file facts.
    #[must_use]
    pub fn needs_changed_files(&self) -> bool {
        !self.paths.is_empty()
    }

    /// Whether closed or merged pull requests must be admitted for this filter.
    #[must_use]
    pub fn admits_closed(&self) -> bool {
        self.types
            .iter()
            .any(|value| matches!(value, PullRequestType::Closed | PullRequestType::Merged))
    }

    /// Effective pull request actions.
    #[must_use]
    pub fn pull_request_types(&self) -> &[PullRequestType] {
        if self.types.is_empty() {
            &DEFAULT_PULL_REQUEST_TYPES
        } else {
            &self.types
        }
    }
}

/// One observed source event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriggerEvent<'a> {
    Manual,
    Schedule,
    /// A push to a full reference (`refs/heads/main`, `refs/tags/v1`).
    Push {
        reference: &'a str,
    },
    /// A pull request action targeting the base branch name (no prefix).
    PullRequest {
        action: PullRequestType,
        base_branch: &'a str,
    },
}

impl TriggerEvent<'_> {
    #[must_use]
    pub const fn kind(&self) -> EventKind {
        match self {
            Self::Manual => EventKind::Manual,
            Self::Schedule => EventKind::Schedule,
            Self::Push { .. } => EventKind::Push,
            Self::PullRequest { .. } => EventKind::PullRequest,
        }
    }
}

/// A filter field: the field that excluded an event, or that failed admission.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterField {
    /// The event kind differs from the trigger's kind.
    Event,
    Branches,
    Tags,
    Types,
    Paths,
}

impl FilterField {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Branches => "branches",
            Self::Tags => "tags",
            Self::Types => "types",
            Self::Paths => "paths",
        }
    }
}

/// Why a trigger matched. Recorded on every run it starts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "filter", rename_all = "snake_case", deny_unknown_fields)]
pub enum MatchReason {
    /// No filter applies to this event.
    Unfiltered,
    Branch {
        pattern: String,
    },
    Tag {
        pattern: String,
    },
    Type {
        action: PullRequestType,
    },
    Path {
        pattern: String,
        path: String,
    },
    /// Changed files were unknown, so the `paths` filter never skips.
    PathsUnknown {
        fallback: ChangedFallback,
    },
}

/// The pure decision for one trigger and one event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum FilterDecision {
    Run { reasons: Vec<MatchReason> },
    Skip { excluded_by: FilterField },
}

/// Admission failure naming the offending field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilterError {
    pub field: FilterField,
    pub message: &'static str,
}

impl fmt::Display for FilterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field.as_str(), self.message)
    }
}

fn reject(field: FilterField, message: &'static str) -> Result<(), FilterError> {
    Err(FilterError { field, message })
}

fn admit_patterns(
    field: FilterField,
    patterns: &[String],
    reference: bool,
) -> Result<(), FilterError> {
    if patterns.len() > MAX_FILTER_PATTERNS {
        return reject(field, "too many patterns");
    }
    for (index, pattern) in patterns.iter().enumerate() {
        if pattern.is_empty() || pattern.len() > MAX_FILTER_PATTERN_BYTES {
            return reject(field, "pattern length outside 1..=256 bytes");
        }
        // `!` negation, `+`, character classes and alternations have no
        // equivalent in the shared matcher and are never silently reinterpreted.
        if pattern.starts_with('!') || pattern.contains('+') || !paths::valid_pattern(pattern) {
            return reject(
                field,
                "pattern must use `*`, `?` and whole-segment `**` only; negation, classes and alternation are unsupported",
            );
        }
        if reference
            && pattern
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '~' | '^' | ':'))
        {
            return reject(
                field,
                "reference pattern contains a character Git refs forbid",
            );
        }
        if patterns
            .iter()
            .take(index)
            .any(|previous| previous == pattern)
        {
            return reject(field, "duplicate pattern");
        }
    }
    Ok(())
}

// r[impl kiln.triggers.filter]
/// Admit a filter for an event kind: tags only for pushes, types only for pull
/// requests, no filter for manual or scheduled triggers, and bounded patterns.
pub fn admit_filter(kind: EventKind, filter: &TriggerFilter) -> Result<(), FilterError> {
    match kind {
        EventKind::Manual | EventKind::Schedule => {
            if !filter.is_empty() {
                return reject(
                    FilterField::Event,
                    "manual and schedule triggers accept no filters",
                );
            }
            return Ok(());
        }
        EventKind::Push if !filter.types.is_empty() => {
            return reject(FilterField::Types, "types apply only to pull requests");
        }
        EventKind::PullRequest if !filter.tags.is_empty() => {
            return reject(FilterField::Tags, "tags apply only to pushes");
        }
        EventKind::Push | EventKind::PullRequest => {}
    }
    admit_patterns(FilterField::Branches, &filter.branches, true)?;
    admit_patterns(FilterField::Tags, &filter.tags, true)?;
    admit_patterns(FilterField::Paths, &filter.paths, false)?;
    for (index, value) in filter.types.iter().enumerate() {
        if filter
            .types
            .iter()
            .take(index)
            .any(|previous| previous == value)
        {
            return reject(FilterField::Types, "duplicate type");
        }
    }
    Ok(())
}

fn first_match(patterns: &[String], value: &str) -> Option<String> {
    patterns
        .iter()
        .find(|pattern| paths::matches(pattern, value))
        .cloned()
}

// r[impl kiln.triggers.evaluate]
/// Decide one trigger for one event. Order: event kind, branch/tag, types,
/// paths; the first failing filter is the recorded exclusion.
#[must_use]
pub fn evaluate(
    kind: EventKind,
    filter: &TriggerFilter,
    event: &TriggerEvent<'_>,
    changed: &ChangedFiles,
) -> FilterDecision {
    if event.kind() != kind {
        return FilterDecision::Skip {
            excluded_by: FilterField::Event,
        };
    }
    let mut reasons = Vec::new();
    match *event {
        TriggerEvent::Manual | TriggerEvent::Schedule => {
            return FilterDecision::Run {
                reasons: vec![MatchReason::Unfiltered],
            };
        }
        TriggerEvent::Push { reference } => {
            // GitHub Actions rule: a filter naming only tags excludes branch
            // pushes, and one naming only branches excludes tag pushes.
            if let Some(branch) = reference.strip_prefix(BRANCH_PREFIX) {
                if !filter.branches.is_empty() {
                    let Some(pattern) = first_match(&filter.branches, branch) else {
                        return FilterDecision::Skip {
                            excluded_by: FilterField::Branches,
                        };
                    };
                    reasons.push(MatchReason::Branch { pattern });
                } else if !filter.tags.is_empty() {
                    return FilterDecision::Skip {
                        excluded_by: FilterField::Tags,
                    };
                }
            } else if let Some(tag) = reference.strip_prefix(TAG_PREFIX) {
                if !filter.tags.is_empty() {
                    let Some(pattern) = first_match(&filter.tags, tag) else {
                        return FilterDecision::Skip {
                            excluded_by: FilterField::Tags,
                        };
                    };
                    reasons.push(MatchReason::Tag { pattern });
                } else if !filter.branches.is_empty() {
                    return FilterDecision::Skip {
                        excluded_by: FilterField::Branches,
                    };
                }
            } else if !filter.branches.is_empty() {
                return FilterDecision::Skip {
                    excluded_by: FilterField::Branches,
                };
            } else if !filter.tags.is_empty() {
                return FilterDecision::Skip {
                    excluded_by: FilterField::Tags,
                };
            }
        }
        TriggerEvent::PullRequest {
            action,
            base_branch,
        } => {
            if !filter.branches.is_empty() {
                let Some(pattern) = first_match(&filter.branches, base_branch) else {
                    return FilterDecision::Skip {
                        excluded_by: FilterField::Branches,
                    };
                };
                reasons.push(MatchReason::Branch { pattern });
            }
            if !filter.pull_request_types().contains(&action) {
                return FilterDecision::Skip {
                    excluded_by: FilterField::Types,
                };
            }
            if !filter.types.is_empty() {
                reasons.push(MatchReason::Type { action });
            }
        }
    }
    if !filter.paths.is_empty() {
        match changed {
            ChangedFiles::Fallback(fallback) => {
                reasons.push(MatchReason::PathsUnknown {
                    fallback: *fallback,
                });
            }
            ChangedFiles::Exact(files) => {
                let found = filter.paths.iter().find_map(|pattern| {
                    files
                        .iter()
                        .find(|path| paths::matches(pattern, path))
                        .map(|path| MatchReason::Path {
                            pattern: pattern.clone(),
                            path: path.clone(),
                        })
                });
                let Some(reason) = found else {
                    return FilterDecision::Skip {
                        excluded_by: FilterField::Paths,
                    };
                };
                reasons.push(reason);
            }
        }
    }
    if reasons.is_empty() {
        reasons.push(MatchReason::Unfiltered);
    }
    FilterDecision::Run { reasons }
}

/// One trigger's exclusion in an ordered trigger list.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exclusion {
    pub index: usize,
    pub event: EventKind,
    pub excluded_by: FilterField,
}

/// The first matching trigger of an ordered list, or every exclusion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "selection", rename_all = "snake_case", deny_unknown_fields)]
pub enum Selection {
    Matched {
        index: usize,
        event: EventKind,
        reasons: Vec<MatchReason>,
    },
    Excluded {
        exclusions: Vec<Exclusion>,
    },
}

// r[impl kiln.triggers.evaluate]
/// Select the first trigger, in declared order, whose filter runs the event.
#[must_use]
pub fn select<'a>(
    triggers: impl IntoIterator<Item = (EventKind, &'a TriggerFilter)>,
    event: &TriggerEvent<'_>,
    changed: &ChangedFiles,
) -> Selection {
    let mut exclusions = Vec::new();
    for (index, (kind, filter)) in triggers.into_iter().enumerate() {
        match evaluate(kind, filter, event, changed) {
            FilterDecision::Run { reasons } => {
                return Selection::Matched {
                    index,
                    event: kind,
                    reasons,
                };
            }
            FilterDecision::Skip { excluded_by } => exclusions.push(Exclusion {
                index,
                event: kind,
                excluded_by,
            }),
        }
    }
    Selection::Excluded { exclusions }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;
    use alloc::string::ToString;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(ToString::to_string).collect()
    }
    fn exact(values: &[&str]) -> ChangedFiles {
        ChangedFiles::Exact(
            values
                .iter()
                .map(ToString::to_string)
                .collect::<BTreeSet<_>>(),
        )
    }
    fn skip(field: FilterField) -> FilterDecision {
        FilterDecision::Skip { excluded_by: field }
    }
    fn push(reference: &str) -> TriggerEvent<'_> {
        TriggerEvent::Push { reference }
    }
    fn pr(action: PullRequestType, base_branch: &str) -> TriggerEvent<'_> {
        TriggerEvent::PullRequest {
            action,
            base_branch,
        }
    }

    // r[verify kiln.triggers.evaluate]
    // r[verify kiln.triggers.evaluate.scenario.match]
    #[test]
    fn branch_tag_type_and_path_filters_match_and_exclude() {
        let none = exact(&[]);
        let branches = TriggerFilter {
            branches: strings(&["main", "release/**"]),
            ..TriggerFilter::default()
        };
        assert_eq!(
            evaluate(
                EventKind::Push,
                &branches,
                &push("refs/heads/release/v1/rc"),
                &none
            ),
            FilterDecision::Run {
                reasons: vec![MatchReason::Branch {
                    pattern: "release/**".into()
                }]
            }
        );
        assert_eq!(
            evaluate(
                EventKind::Push,
                &branches,
                &push("refs/heads/feature"),
                &none
            ),
            skip(FilterField::Branches)
        );
        // Only branches named: tag pushes are excluded.
        assert_eq!(
            evaluate(EventKind::Push, &branches, &push("refs/tags/v1"), &none),
            skip(FilterField::Branches)
        );
        let tags = TriggerFilter {
            tags: strings(&["v*"]),
            ..TriggerFilter::default()
        };
        assert_eq!(
            evaluate(EventKind::Push, &tags, &push("refs/tags/v1.2"), &none),
            FilterDecision::Run {
                reasons: vec![MatchReason::Tag {
                    pattern: "v*".into()
                }]
            }
        );
        assert_eq!(
            evaluate(EventKind::Push, &tags, &push("refs/tags/nightly"), &none),
            skip(FilterField::Tags)
        );
        assert_eq!(
            evaluate(EventKind::Push, &tags, &push("refs/heads/main"), &none),
            skip(FilterField::Tags)
        );
        // Pull request branches match the base branch; types gate the action.
        let pull = TriggerFilter {
            branches: strings(&["main"]),
            types: vec![PullRequestType::Opened, PullRequestType::Merged],
            ..TriggerFilter::default()
        };
        assert_eq!(
            evaluate(
                EventKind::PullRequest,
                &pull,
                &pr(PullRequestType::Merged, "main"),
                &none
            ),
            FilterDecision::Run {
                reasons: vec![
                    MatchReason::Branch {
                        pattern: "main".into()
                    },
                    MatchReason::Type {
                        action: PullRequestType::Merged
                    }
                ]
            }
        );
        assert_eq!(
            evaluate(
                EventKind::PullRequest,
                &pull,
                &pr(PullRequestType::Synchronize, "main"),
                &none
            ),
            skip(FilterField::Types)
        );
        assert_eq!(
            evaluate(
                EventKind::PullRequest,
                &pull,
                &pr(PullRequestType::Opened, "dev"),
                &none
            ),
            skip(FilterField::Branches)
        );
        // Empty types keep today's opened/reopened/synchronize behaviour.
        let unfiltered = TriggerFilter::default();
        assert_eq!(
            evaluate(
                EventKind::PullRequest,
                &unfiltered,
                &pr(PullRequestType::Reopened, "x"),
                &none
            ),
            FilterDecision::Run {
                reasons: vec![MatchReason::Unfiltered]
            }
        );
        assert_eq!(
            evaluate(
                EventKind::PullRequest,
                &unfiltered,
                &pr(PullRequestType::Closed, "x"),
                &none
            ),
            skip(FilterField::Types)
        );
        let paths = TriggerFilter {
            paths: strings(&["docs/**", "*.md"]),
            ..TriggerFilter::default()
        };
        assert_eq!(
            evaluate(
                EventKind::Push,
                &paths,
                &push("refs/heads/main"),
                &exact(&["src/lib.rs", "README.md"])
            ),
            FilterDecision::Run {
                reasons: vec![MatchReason::Path {
                    pattern: "*.md".into(),
                    path: "README.md".into()
                }]
            }
        );
        assert_eq!(
            evaluate(
                EventKind::Push,
                &paths,
                &push("refs/heads/main"),
                &exact(&["src/lib.rs"])
            ),
            skip(FilterField::Paths)
        );
        // An exact empty diff touches no filtered path.
        assert_eq!(
            evaluate(EventKind::Push, &paths, &push("refs/heads/main"), &none),
            skip(FilterField::Paths)
        );
        // Event kind mismatches are the first exclusion.
        assert_eq!(
            evaluate(
                EventKind::PullRequest,
                &paths,
                &push("refs/heads/main"),
                &none
            ),
            skip(FilterField::Event)
        );
        assert_eq!(
            evaluate(EventKind::Manual, &unfiltered, &TriggerEvent::Manual, &none),
            FilterDecision::Run {
                reasons: vec![MatchReason::Unfiltered]
            }
        );
    }

    // r[verify kiln.triggers.evaluate.scenario.unknown]
    #[test]
    fn unknown_changed_files_run_with_the_fallback_reason() {
        let filter = TriggerFilter {
            branches: strings(&["main"]),
            paths: strings(&["docs/**"]),
            ..TriggerFilter::default()
        };
        for fallback in [
            ChangedFallback::NoBase,
            ChangedFallback::Truncated,
            ChangedFallback::ReadError,
            ChangedFallback::UnsupportedProvider,
        ] {
            assert_eq!(
                evaluate(
                    EventKind::Push,
                    &filter,
                    &push("refs/heads/main"),
                    &ChangedFiles::Fallback(fallback)
                ),
                FilterDecision::Run {
                    reasons: vec![
                        MatchReason::Branch {
                            pattern: "main".into()
                        },
                        MatchReason::PathsUnknown { fallback }
                    ]
                }
            );
        }
        // Unknown files never rescue an event a ref filter already excluded.
        assert_eq!(
            evaluate(
                EventKind::Push,
                &filter,
                &push("refs/heads/dev"),
                &ChangedFiles::Fallback(ChangedFallback::NoBase)
            ),
            skip(FilterField::Branches)
        );
    }

    #[test]
    fn rename_old_paths_match_and_selection_keeps_declared_order() {
        // A rename contributes both paths to the exact facts.
        let changed = exact(&["docs/old.md", "src/new.md"]);
        let docs = TriggerFilter {
            paths: strings(&["docs/**"]),
            ..TriggerFilter::default()
        };
        let other = TriggerFilter {
            paths: strings(&["other/**"]),
            ..TriggerFilter::default()
        };
        let unfiltered = TriggerFilter::default();
        let event = push("refs/heads/main");
        assert_eq!(
            select(
                [
                    (EventKind::Manual, &unfiltered),
                    (EventKind::Push, &other),
                    (EventKind::Push, &docs),
                ],
                &event,
                &changed
            ),
            Selection::Matched {
                index: 2,
                event: EventKind::Push,
                reasons: vec![MatchReason::Path {
                    pattern: "docs/**".into(),
                    path: "docs/old.md".into()
                }]
            }
        );
        assert_eq!(
            select(
                [(EventKind::Manual, &unfiltered), (EventKind::Push, &other)],
                &event,
                &changed
            ),
            Selection::Excluded {
                exclusions: vec![
                    Exclusion {
                        index: 0,
                        event: EventKind::Manual,
                        excluded_by: FilterField::Event
                    },
                    Exclusion {
                        index: 1,
                        event: EventKind::Push,
                        excluded_by: FilterField::Paths
                    }
                ]
            }
        );
    }

    // r[verify kiln.triggers.filter]
    // r[verify kiln.triggers.filter.scenario.invalid]
    #[test]
    fn admission_rejects_misplaced_unbounded_and_unsupported_filters() {
        let valid = TriggerFilter {
            branches: strings(&["main", "release/*"]),
            tags: strings(&["v*"]),
            paths: strings(&["src/**"]),
            ..TriggerFilter::default()
        };
        assert!(admit_filter(EventKind::Push, &valid).is_ok());
        let types = TriggerFilter {
            types: vec![PullRequestType::Opened],
            ..TriggerFilter::default()
        };
        assert!(admit_filter(EventKind::PullRequest, &types).is_ok());
        let field =
            |kind, filter: &TriggerFilter| admit_filter(kind, filter).err().map(|e| e.field);
        assert_eq!(field(EventKind::Push, &types), Some(FilterField::Types));
        assert_eq!(
            field(EventKind::PullRequest, &valid),
            Some(FilterField::Tags)
        );
        assert_eq!(field(EventKind::Manual, &types), Some(FilterField::Event));
        assert_eq!(field(EventKind::Schedule, &valid), Some(FilterField::Event));
        assert!(admit_filter(EventKind::Manual, &TriggerFilter::default()).is_ok());
        for bad in [
            "!main",
            "feature/[ab]",
            "{a,b}",
            "a/../b",
            "/abs",
            "",
            "x+",
            "a**b",
        ] {
            let filter = TriggerFilter {
                paths: strings(&[bad]),
                ..TriggerFilter::default()
            };
            assert_eq!(
                field(EventKind::Push, &filter),
                Some(FilterField::Paths),
                "{bad}"
            );
        }
        for bad in ["has space", "a~b", "a:b"] {
            let filter = TriggerFilter {
                branches: strings(&[bad]),
                ..TriggerFilter::default()
            };
            assert_eq!(
                field(EventKind::Push, &filter),
                Some(FilterField::Branches),
                "{bad}"
            );
        }
        let long = TriggerFilter {
            paths: vec!["a".repeat(MAX_FILTER_PATTERN_BYTES + 1)],
            ..TriggerFilter::default()
        };
        assert_eq!(field(EventKind::Push, &long), Some(FilterField::Paths));
        let many = TriggerFilter {
            branches: (0..=MAX_FILTER_PATTERNS)
                .map(|i| alloc::format!("b{i}"))
                .collect(),
            ..TriggerFilter::default()
        };
        assert_eq!(field(EventKind::Push, &many), Some(FilterField::Branches));
        let duplicate = TriggerFilter {
            types: vec![PullRequestType::Closed, PullRequestType::Closed],
            ..TriggerFilter::default()
        };
        assert_eq!(
            field(EventKind::PullRequest, &duplicate),
            Some(FilterField::Types)
        );
    }

    // r[verify kiln.triggers.replay]
    #[test]
    fn empty_filters_serialize_to_nothing_and_unknown_keys_fail() {
        let encoded = serde_json::to_string(&TriggerFilter::default()).unwrap_or_default();
        assert_eq!(encoded, "{}");
        assert!(serde_json::from_str::<TriggerFilter>(r#"{"branch":["main"]}"#).is_err());
        assert!(serde_json::from_str::<TriggerFilter>(r#"{"types":["edited"]}"#).is_err());
        let decoded: Result<TriggerFilter, _> =
            serde_json::from_str(r#"{"types":["merged"],"paths":["a/**"]}"#);
        assert!(decoded.is_ok_and(|filter| filter.admits_closed() && filter.needs_changed_files()));
    }
}
