//! Review-friction observation over an admitted, shell-supplied review window.
//!
//! Authorship classes are self-declared and unverified. The window is a pure
//! derivation over admitted review state; these views grant no authority.
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use kiln_core::change_review::{
    AuthorshipClass, OwnershipPolicy, ReviewState, VoteValue, authorship_class_of, owning_rule,
};
use kiln_core::types::{CoreResult, Diagnostic};
use serde::{Deserialize, Serialize};

use crate::profile::{ObservationProfile, ReviewBounds, ViewField, text_bound};

/// Denominator of every reported share.
pub const PER_MILLE: u64 = 1000;

/// Admitted review facts for one half-open `[start_ms, end_ms)` window.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewWindow {
    pub start_ms: i64,
    pub end_ms: i64,
    pub changes: Vec<ReviewChangeFact>,
}

/// One change whose first patch set lies in the window, clipped to the window end.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewChangeFact {
    pub change_id: String,
    pub authorship: AuthorshipClass,
    /// Creation time of each admitted patch set, in patch-set order.
    pub patch_sets_ms: Vec<i64>,
    /// Earliest approval bound to the latest admitted patch set.
    pub approval: Option<ReviewApprovalFact>,
    pub suggestions: Vec<ReviewSuggestionFact>,
    pub merged_ms: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewApprovalFact {
    pub patch_set: u32,
    pub approved_ms: i64,
}

/// One anchored suggestion: a review thread opened by someone other than the
/// change owner, keyed by anchor path and its first owning rule.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewSuggestionFact {
    pub path: String,
    pub owner: Option<String>,
    pub created_ms: i64,
}

/// The fixed basis of every authorship class this view reports.
// r[impl kiln.observe.review.nonclaims]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorshipBasis {
    SelfDeclaredUnverified,
}

/// Deterministic integer distribution; `median` is the lower median.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Distribution {
    pub count: u64,
    pub total: u64,
    pub min: Option<u64>,
    pub median: Option<u64>,
    pub max: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorshipFrictionView {
    pub authorship: AuthorshipClass,
    pub changes: u64,
    pub approved_changes: u64,
    pub approval_latency_ms: Distribution,
    pub patch_sets_before_approval: Distribution,
    pub suggestions_per_change: Distribution,
    pub merged_changes: u64,
    pub merged_without_suggestion: u64,
    pub merged_without_suggestion_per_mille: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewFrictionView {
    pub start_ms: i64,
    pub end_ms: i64,
    pub authorship_basis: AuthorshipBasis,
    pub classes: Vec<AuthorshipFrictionView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionAnchorView {
    pub path: Option<String>,
    pub owner: Option<String>,
    pub suggestions: u64,
    pub changes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorPageInfo {
    pub cursor: u64,
    pub requested_limit: u64,
    pub effective_limit: u64,
    pub returned: u64,
    pub total: u64,
    pub capped: bool,
    pub has_more: bool,
    pub next: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionAnchorsView {
    pub start_ms: i64,
    pub end_ms: i64,
    pub anchors: Vec<SuggestionAnchorView>,
    pub page: AnchorPageInfo,
}

fn window_error(code: &'static str, message: &'static str) -> Diagnostic {
    Diagnostic::new(code, "observation.window.review", message)
}

fn range_error() -> Diagnostic {
    window_error(
        "review_window_range",
        "review window must be a non-empty half-open millisecond range",
    )
}

/// Derive the review window from admitted review state.
///
/// Changes whose first patch set lies in `[start_ms, end_ms)` are included in
/// change-id order; every later fact is clipped to `< end_ms`. Authorship is
/// folded from declared patch sets only and never inferred.
// r[impl kiln.observe.review.window]
// r[impl kiln.observe.review.friction.scenario.fixture]
pub fn review_window(
    state: &ReviewState,
    ownership: Option<&OwnershipPolicy>,
    start_ms: i64,
    end_ms: i64,
) -> CoreResult<ReviewWindow> {
    if start_ms >= end_ms {
        return Err(range_error());
    }
    let mut changes = Vec::new();
    for change in state.changes.values() {
        let admitted = change
            .revisions
            .iter()
            .take_while(|revision| revision.created_ms < end_ms)
            .count();
        let revisions = change.revisions.get(..admitted).unwrap_or_default();
        let (Some(first), Some(latest)) = (revisions.first(), revisions.last()) else {
            continue;
        };
        if first.created_ms < start_ms {
            continue;
        }
        let approval = change
            .votes
            .iter()
            .filter(|vote| {
                vote.value == VoteValue::Approve
                    && vote.revision == latest.id
                    && vote.created_ms < end_ms
            })
            .map(|vote| vote.created_ms)
            .min()
            .map(|approved_ms| ReviewApprovalFact {
                patch_set: u32::try_from(revisions.len()).unwrap_or(u32::MAX),
                approved_ms,
            });
        let suggestions = change
            .threads
            .iter()
            .filter_map(|thread| {
                let opening = thread.comments.first()?;
                (opening.author != change.owner && opening.created_ms < end_ms).then(|| {
                    ReviewSuggestionFact {
                        path: thread.anchor.file.clone(),
                        owner: ownership
                            .and_then(|policy| {
                                owning_rule(policy, &change.target_ref, &thread.anchor.file)
                            })
                            .map(String::from),
                        created_ms: opening.created_ms,
                    }
                })
            })
            .collect();
        changes.push(ReviewChangeFact {
            change_id: change.id.clone(),
            authorship: authorship_class_of(revisions),
            patch_sets_ms: revisions
                .iter()
                .map(|revision| revision.created_ms)
                .collect(),
            approval,
            suggestions,
            merged_ms: change
                .submission
                .as_ref()
                .map(|submission| submission.submitted_ms)
                .filter(|merged_ms| *merged_ms < end_ms),
        });
    }
    Ok(ReviewWindow {
        start_ms,
        end_ms,
        changes,
    })
}

fn review_bounds(profile: &ObservationProfile) -> CoreResult<&ReviewBounds> {
    profile.bounds.review.as_ref().ok_or_else(|| {
        Diagnostic::new(
            "missing_review_bounds",
            "observation.profile.bounds.review",
            "profile declares no review bounds",
        )
    })
}

fn within(value: usize, limit: u64) -> bool {
    u64::try_from(value).is_ok_and(|value| value <= limit)
}

fn text_valid(value: &str, max_text: usize) -> bool {
    !value.is_empty() && value.len() <= max_text
}

/// Admit one review window against the profile's review and text bounds.
// r[impl kiln.observe.review.window]
// r[impl kiln.observe.review.window.scenario.violation]
pub fn admit_review_window(profile: &ObservationProfile, window: &ReviewWindow) -> CoreResult<()> {
    let bounds = review_bounds(profile)?;
    let max_text = text_bound(profile.bounds.max_text_bytes);
    if window.start_ms >= window.end_ms {
        return Err(range_error());
    }
    let span = i128::from(window.end_ms) - i128::from(window.start_ms);
    if span > i128::from(bounds.max_review_window_ms) {
        return Err(window_error(
            "review_window_span",
            "review window span exceeds its named bound",
        ));
    }
    if !within(window.changes.len(), bounds.max_review_changes) {
        return Err(window_error(
            "review_window_change_bound",
            "review window exceeds its named change bound",
        ));
    }
    let mut previous: Option<&str> = None;
    for change in &window.changes {
        if !text_valid(&change.change_id, max_text) {
            return Err(window_error(
                "review_window_text_bound",
                "review change identifier is empty or exceeds its named byte bound",
            ));
        }
        if previous.is_some_and(|previous| previous >= change.change_id.as_str()) {
            return Err(window_error(
                "review_window_order",
                "review changes must be strictly ordered by change identifier",
            ));
        }
        previous = Some(&change.change_id);
        admit_change(bounds, max_text, window, change)?;
    }
    Ok(())
}

fn outside() -> Diagnostic {
    window_error(
        "review_window_fact_outside",
        "review fact lies outside the admitted window or before the first patch set",
    )
}

fn admit_change(
    bounds: &ReviewBounds,
    max_text: usize,
    window: &ReviewWindow,
    change: &ReviewChangeFact,
) -> CoreResult<()> {
    let (Some(first), Some(latest)) = (change.patch_sets_ms.first(), change.patch_sets_ms.last())
    else {
        return Err(window_error(
            "review_window_patch_sets",
            "review change must carry ordered, bounded patch sets",
        ));
    };
    if !within(change.patch_sets_ms.len(), bounds.max_review_patch_sets)
        || change
            .patch_sets_ms
            .windows(2)
            .any(|pair| matches!(pair, [earlier, later] if earlier > later))
    {
        return Err(window_error(
            "review_window_patch_sets",
            "review change must carry ordered, bounded patch sets",
        ));
    }
    let in_change = |value: i64| value >= *first && value < window.end_ms;
    if *first < window.start_ms || *latest >= window.end_ms {
        return Err(outside());
    }
    if let Some(approval) = &change.approval {
        if usize::try_from(approval.patch_set).ok() != Some(change.patch_sets_ms.len())
            || approval.approved_ms < *latest
        {
            return Err(window_error(
                "review_window_approval",
                "approval must bind the latest admitted patch set",
            ));
        }
        if !in_change(approval.approved_ms) {
            return Err(outside());
        }
    }
    if !within(
        change.suggestions.len(),
        bounds.max_review_suggestions_per_change,
    ) {
        return Err(window_error(
            "review_window_suggestion_bound",
            "review change exceeds its named suggestion bound",
        ));
    }
    for suggestion in &change.suggestions {
        if !text_valid(&suggestion.path, max_text)
            || suggestion
                .owner
                .as_deref()
                .is_some_and(|owner| !text_valid(owner, max_text))
        {
            return Err(window_error(
                "review_window_text_bound",
                "suggestion path or owner is empty or exceeds its named byte bound",
            ));
        }
        if !in_change(suggestion.created_ms) {
            return Err(outside());
        }
    }
    if change
        .merged_ms
        .is_some_and(|merged_ms| !in_change(merged_ms))
    {
        return Err(outside());
    }
    Ok(())
}

fn count(value: usize) -> u64 {
    value as u64
}

fn distribution(mut values: Vec<u64>) -> Distribution {
    values.sort_unstable();
    let middle = values.len().saturating_sub(1) / 2;
    Distribution {
        count: count(values.len()),
        total: values
            .iter()
            .fold(0u64, |total, value| total.saturating_add(*value)),
        min: values.first().copied(),
        median: values.get(middle).copied(),
        max: values.last().copied(),
    }
}

fn elapsed(from: i64, to: i64) -> u64 {
    u64::try_from(i128::from(to) - i128::from(from)).unwrap_or(0)
}

/// Per-class friction over an admitted review window.
// r[impl kiln.observe.review.friction]
pub(crate) fn project_review_friction(window: &ReviewWindow) -> ReviewFrictionView {
    let classes = AuthorshipClass::ALL
        .iter()
        .map(|class| {
            let changes: Vec<_> = window
                .changes
                .iter()
                .filter(|change| change.authorship == *class)
                .collect();
            let mut latency = Vec::new();
            let mut patch_sets = Vec::new();
            for change in &changes {
                if let (Some(approval), Some(first)) =
                    (&change.approval, change.patch_sets_ms.first())
                {
                    latency.push(elapsed(*first, approval.approved_ms));
                    patch_sets.push(u64::from(approval.patch_set));
                }
            }
            let merged = changes
                .iter()
                .filter(|change| change.merged_ms.is_some())
                .count();
            let merged_without_suggestion = changes
                .iter()
                .filter(|change| change.merged_ms.is_some() && change.suggestions.is_empty())
                .count();
            AuthorshipFrictionView {
                authorship: *class,
                changes: count(changes.len()),
                approved_changes: count(latency.len()),
                approval_latency_ms: distribution(latency),
                patch_sets_before_approval: distribution(patch_sets),
                suggestions_per_change: distribution(
                    changes
                        .iter()
                        .map(|change| count(change.suggestions.len()))
                        .collect(),
                ),
                merged_changes: count(merged),
                merged_without_suggestion: count(merged_without_suggestion),
                merged_without_suggestion_per_mille: (merged > 0)
                    .then(|| count(merged_without_suggestion) * PER_MILLE / count(merged)),
            }
        })
        .collect();
    ReviewFrictionView {
        start_ms: window.start_ms,
        end_ms: window.end_ms,
        authorship_basis: AuthorshipBasis::SelfDeclaredUnverified,
        classes,
    }
}

/// Path and owning rule of one suggestion anchor.
type AnchorKey<'a> = (&'a str, Option<&'a str>);
/// Suggestion count and distinct changes of one anchor.
type AnchorTally<'a> = (u64, BTreeSet<&'a str>);

/// Ranked, paged suggestion anchors with post-aggregation redaction.
// r[impl kiln.observe.review.anchors]
// r[impl kiln.observe.review.anchors.scenario.paging]
pub(crate) fn project_suggestion_anchors(
    profile: &ObservationProfile,
    window: &ReviewWindow,
    cursor: u64,
    requested_limit: u64,
    redacted: &mut BTreeSet<ViewField>,
) -> CoreResult<SuggestionAnchorsView> {
    if requested_limit == 0 {
        return Err(Diagnostic::new(
            "invalid_page_limit",
            "suggestion_anchors.limit",
            "page limit must be positive",
        ));
    }
    let mut groups: BTreeMap<AnchorKey<'_>, AnchorTally<'_>> = BTreeMap::new();
    for change in &window.changes {
        for suggestion in &change.suggestions {
            let entry = groups
                .entry((suggestion.path.as_str(), suggestion.owner.as_deref()))
                .or_default();
            entry.0 += 1;
            entry.1.insert(change.change_id.as_str());
        }
    }
    let mut ranked: Vec<_> = groups
        .into_iter()
        .map(|((path, owner), (suggestions, changes))| {
            (path, owner, suggestions, count(changes.len()))
        })
        .collect();
    ranked.sort_by(|left, right| {
        right
            .2
            .cmp(&left.2)
            .then(right.3.cmp(&left.3))
            .then(left.0.cmp(right.0))
            .then(left.1.cmp(&right.1))
    });
    let total = count(ranked.len());
    if cursor > total {
        return Err(Diagnostic::new(
            "cursor_ahead",
            "suggestion_anchors.cursor",
            "anchor cursor lies beyond the admitted ranking",
        ));
    }
    let max_page = profile.bounds.max_page_items;
    let effective = requested_limit.min(max_page);
    let show_paths = profile.field_visible(ViewField::SuggestionPaths);
    let show_owners = profile.field_visible(ViewField::SuggestionOwners);
    let start = usize::try_from(cursor).unwrap_or(usize::MAX);
    let take = usize::try_from(effective).unwrap_or(usize::MAX);
    let anchors: Vec<_> = ranked
        .iter()
        .skip(start)
        .take(take)
        .map(|(path, owner, suggestions, changes)| SuggestionAnchorView {
            path: show_paths.then(|| String::from(*path)),
            owner: if show_owners {
                owner.map(String::from)
            } else {
                None
            },
            suggestions: *suggestions,
            changes: *changes,
        })
        .collect();
    if !show_paths && !anchors.is_empty() {
        redacted.insert(ViewField::SuggestionPaths);
    }
    let owner_present = ranked
        .iter()
        .skip(start)
        .take(take)
        .any(|(_, owner, _, _)| owner.is_some());
    if !show_owners && owner_present {
        redacted.insert(ViewField::SuggestionOwners);
    }
    let returned = count(anchors.len());
    let has_more = cursor + returned < total;
    Ok(SuggestionAnchorsView {
        start_ms: window.start_ms,
        end_ms: window.end_ms,
        anchors,
        page: AnchorPageInfo {
            cursor,
            requested_limit,
            effective_limit: effective,
            returned,
            total,
            capped: requested_limit > max_page,
            has_more,
            next: has_more.then_some(cursor + returned),
        },
    })
}
