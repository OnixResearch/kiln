use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use kiln_core::change_review::{
    Authorship, AuthorshipClass, Command, Facts, OwnershipPolicy, ReviewState, Revision, apply,
};
use kiln_core::domain::State;
use kiln_core::types::{CoreResult, Diagnostic};
use serde::Deserialize;

use crate::*;

#[derive(Deserialize)]
struct ProfileDocument {
    profile: ObservationProfile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalEntry {
    actor: String,
    facts: Facts,
    command: Command,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalFixture {
    schema: String,
    start_ms: i64,
    end_ms: i64,
    ownership: OwnershipPolicy,
    entries: Vec<JournalEntry>,
}

fn profile() -> ObservationProfile {
    let document: ProfileDocument = serde_json::from_str(include_str!(
        "../../../fixtures/observation/profile-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    document.profile
}

fn fixture_window() -> ReviewWindow {
    serde_json::from_str(include_str!(
        "../../../fixtures/observation/review-window-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!())
}

fn replay_fixture_journal() -> (JournalFixture, ReviewState) {
    let fixture: JournalFixture = serde_json::from_str(include_str!(
        "../../../fixtures/observation/review-journal-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!());
    let mut state = ReviewState::default();
    for entry in &fixture.entries {
        apply(
            &mut state,
            &entry.actor,
            entry.command.clone(),
            &entry.facts,
        )
        .unwrap_or_else(|_| unreachable!());
    }
    (fixture, state)
}

fn ci_state() -> State {
    State {
        revision: 0,
        runs: BTreeMap::new(),
        attempts: BTreeMap::new(),
        holds: BTreeSet::new(),
        seen_commands: BTreeSet::new(),
    }
}

fn request(view: ViewRequest) -> ObservationRequest {
    ObservationRequest {
        state_revision: 0,
        view,
    }
}

fn project(
    profile: &ObservationProfile,
    view: ViewRequest,
    window: &ObservationWindow,
) -> CoreResult<ObservationView> {
    project_observation(profile, &ci_state(), &request(view), window)
}

fn friction(window: ReviewWindow) -> ReviewFrictionView {
    match project(
        &profile(),
        ViewRequest::ReviewFriction,
        &ObservationWindow::review(window),
    )
    .unwrap_or_else(|_| unreachable!())
    .body
    {
        ViewBody::ReviewFriction(view) => view,
        _ => unreachable!(),
    }
}

fn anchors(
    profile: &ObservationProfile,
    window: ReviewWindow,
    cursor: u64,
    limit: u64,
) -> CoreResult<(SuggestionAnchorsView, BTreeSet<ViewField>)> {
    let view = project(
        profile,
        ViewRequest::SuggestionAnchors { cursor, limit },
        &ObservationWindow::review(window),
    )?;
    match view.body {
        ViewBody::SuggestionAnchors(body) => Ok((body, view.redacted_fields)),
        _ => unreachable!(),
    }
}

fn code(result: CoreResult<impl Sized>) -> Option<String> {
    result.err().map(|error: Diagnostic| error.code)
}

fn distribution(
    count: u64,
    total: u64,
    min: Option<u64>,
    median: Option<u64>,
    max: Option<u64>,
) -> Distribution {
    Distribution {
        count,
        total,
        min,
        median,
        max,
    }
}

fn empty_distribution() -> Distribution {
    distribution(0, 0, None, None, None)
}

fn zero_suggestions(count: u64) -> Distribution {
    distribution(count, 0, Some(0), Some(0), Some(0))
}

// r[verify kiln.observe.review.friction]
// r[verify kiln.observe.review.friction.scenario.fixture]
// r[verify kiln.observe.review.nonclaims]
#[test]
fn fixture_journal_replays_to_window_with_known_friction() {
    let (fixture, state) = replay_fixture_journal();
    assert_eq!(fixture.schema, "kiln.review.journal-fixture.v1");
    let derived = review_window(
        &state,
        Some(&fixture.ownership),
        fixture.start_ms,
        fixture.end_ms,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(derived, fixture_window());
    assert!(
        derived
            .changes
            .iter()
            .all(|change| change.change_id != "change-early"),
        "a change opened before the window is excluded"
    );
    let view = friction(derived);
    assert_eq!(
        view.authorship_basis,
        AuthorshipBasis::SelfDeclaredUnverified
    );
    let expected = vec![
        AuthorshipFrictionView {
            authorship: AuthorshipClass::Human,
            changes: 2,
            approved_changes: 2,
            approval_latency_ms: distribution(2, 6_000, Some(2_000), Some(2_000), Some(4_000)),
            patch_sets_before_approval: distribution(2, 3, Some(1), Some(1), Some(2)),
            suggestions_per_change: distribution(2, 1, Some(0), Some(0), Some(1)),
            merged_changes: 2,
            merged_without_suggestion: 1,
            merged_without_suggestion_per_mille: Some(500),
        },
        AuthorshipFrictionView {
            authorship: AuthorshipClass::Agent,
            changes: 3,
            approved_changes: 2,
            approval_latency_ms: distribution(2, 11_000, Some(1_000), Some(1_000), Some(10_000)),
            patch_sets_before_approval: distribution(2, 4, Some(1), Some(1), Some(3)),
            suggestions_per_change: distribution(3, 4, Some(0), Some(1), Some(3)),
            merged_changes: 2,
            merged_without_suggestion: 1,
            merged_without_suggestion_per_mille: Some(500),
        },
        AuthorshipFrictionView {
            authorship: AuthorshipClass::Mixed,
            changes: 1,
            approved_changes: 0,
            approval_latency_ms: empty_distribution(),
            patch_sets_before_approval: empty_distribution(),
            suggestions_per_change: zero_suggestions(1),
            merged_changes: 0,
            merged_without_suggestion: 0,
            merged_without_suggestion_per_mille: None,
        },
        AuthorshipFrictionView {
            authorship: AuthorshipClass::Undeclared,
            changes: 1,
            approved_changes: 0,
            approval_latency_ms: empty_distribution(),
            patch_sets_before_approval: empty_distribution(),
            suggestions_per_change: zero_suggestions(1),
            merged_changes: 0,
            merged_without_suggestion: 0,
            merged_without_suggestion_per_mille: None,
        },
    ];
    assert_eq!(view.classes, expected);
}

#[test]
fn window_end_clips_later_facts_and_later_windows_admit_them() {
    let (fixture, state) = replay_fixture_journal();
    let wider = review_window(
        &state,
        Some(&fixture.ownership),
        fixture.start_ms,
        fixture.end_ms + 60_000,
    )
    .unwrap_or_else(|_| unreachable!());
    let mixed = wider
        .changes
        .iter()
        .find(|change| change.change_id == "change-m1")
        .unwrap_or_else(|| unreachable!());
    assert_eq!(mixed.authorship, AuthorshipClass::Mixed);
    assert_eq!(
        mixed.approval,
        Some(ReviewApprovalFact {
            patch_set: 2,
            approved_ms: fixture.end_ms + 50_000,
        })
    );
    let narrow = review_window(
        &state,
        Some(&fixture.ownership),
        fixture.start_ms,
        fixture.start_ms + 2_000,
    )
    .unwrap_or_else(|_| unreachable!());
    let h1 = narrow.changes.first().unwrap_or_else(|| unreachable!());
    assert_eq!(h1.change_id, "change-h1");
    assert_eq!(h1.patch_sets_ms.len(), 1);
    assert_eq!(h1.approval, None);
    assert!(h1.suggestions.is_empty());
    assert_eq!(h1.merged_ms, None);
    let unowned = review_window(&state, None, fixture.start_ms, fixture.end_ms)
        .unwrap_or_else(|_| unreachable!());
    assert!(
        unowned
            .changes
            .iter()
            .flat_map(|change| &change.suggestions)
            .all(|suggestion| suggestion.owner.is_none())
    );
    assert_eq!(
        code(review_window(&state, None, fixture.end_ms, fixture.end_ms)),
        Some(String::from("review_window_range"))
    );
}

fn anchor(path: &str, owner: Option<&str>, suggestions: u64, changes: u64) -> SuggestionAnchorView {
    SuggestionAnchorView {
        path: Some(path.into()),
        owner: owner.map(String::from),
        suggestions,
        changes,
    }
}

// r[verify kiln.observe.review.anchors]
// r[verify kiln.observe.review.anchors.scenario.paging]
#[test]
fn suggestion_anchors_rank_page_and_bound_their_cursor() {
    let profile = profile();
    let (first, redacted) =
        anchors(&profile, fixture_window(), 0, 2).unwrap_or_else(|_| unreachable!());
    assert!(redacted.is_empty());
    assert_eq!(
        first.anchors,
        vec![
            anchor("src/lib.rs", Some("core"), 3, 2),
            anchor("README.md", None, 1, 1),
        ]
    );
    assert_eq!(
        first.page,
        AnchorPageInfo {
            cursor: 0,
            requested_limit: 2,
            effective_limit: 2,
            returned: 2,
            total: 3,
            capped: false,
            has_more: true,
            next: Some(2),
        }
    );
    let (second, _) = anchors(&profile, fixture_window(), 2, 50).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        second.anchors,
        vec![anchor("docs/guide.md", Some("docs"), 1, 1)]
    );
    assert_eq!(
        second.page,
        AnchorPageInfo {
            cursor: 2,
            requested_limit: 50,
            effective_limit: 2,
            returned: 1,
            total: 3,
            capped: true,
            has_more: false,
            next: None,
        }
    );
    let (end, _) = anchors(&profile, fixture_window(), 3, 1).unwrap_or_else(|_| unreachable!());
    assert!(end.anchors.is_empty());
    assert!(!end.page.has_more);
    assert_eq!(
        code(anchors(&profile, fixture_window(), 0, 0)),
        Some(String::from("invalid_page_limit"))
    );
    assert_eq!(
        code(anchors(&profile, fixture_window(), 4, 1)),
        Some(String::from("cursor_ahead"))
    );
}

#[test]
fn suggestion_anchor_text_is_redacted_after_aggregation() {
    let mut redacting = profile();
    redacting
        .redaction
        .visible_fields
        .remove(&ViewField::SuggestionPaths);
    redacting
        .redaction
        .visible_fields
        .remove(&ViewField::SuggestionOwners);
    let (view, redacted) =
        anchors(&redacting, fixture_window(), 0, 2).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        redacted,
        [ViewField::SuggestionPaths, ViewField::SuggestionOwners]
            .into_iter()
            .collect()
    );
    assert!(
        view.anchors
            .iter()
            .all(|anchor| anchor.path.is_none() && anchor.owner.is_none())
    );
    assert_eq!(
        view.anchors
            .iter()
            .map(|anchor| (anchor.suggestions, anchor.changes))
            .collect::<Vec<_>>(),
        vec![(3, 2), (1, 1)],
        "redaction never changes aggregation"
    );
    let (unowned, redacted) =
        anchors(&redacting, fixture_window(), 1, 1).unwrap_or_else(|_| unreachable!());
    assert_eq!(unowned.anchors.len(), 1);
    assert_eq!(
        redacted,
        [ViewField::SuggestionPaths].into_iter().collect(),
        "only fields that carried a value are named"
    );
}

fn violation(mutate: impl FnOnce(&mut ReviewWindow)) -> Option<String> {
    let mut window = fixture_window();
    mutate(&mut window);
    code(project(
        &profile(),
        ViewRequest::ReviewFriction,
        &ObservationWindow::review(window),
    ))
}

fn at<T>(items: &mut [T], index: usize) -> &mut T {
    items.get_mut(index).unwrap_or_else(|| unreachable!())
}

fn change<'a>(window: &'a mut ReviewWindow, id: &str) -> &'a mut ReviewChangeFact {
    window
        .changes
        .iter_mut()
        .find(|change| change.change_id == id)
        .unwrap_or_else(|| unreachable!())
}

// r[verify kiln.observe.review.window]
// r[verify kiln.observe.review.window.scenario.violation]
#[test]
fn review_window_violations_fail_closed_with_named_diagnostics() {
    let expect = |expected: &str, actual: Option<String>| {
        assert_eq!(actual.as_deref(), Some(expected));
    };
    expect(
        "review_window_range",
        violation(|window| window.end_ms = window.start_ms),
    );
    expect(
        "review_window_range",
        violation(|window| window.end_ms = window.start_ms - 1),
    );
    expect(
        "review_window_span",
        violation(|window| window.end_ms = window.start_ms + 2_592_000_001),
    );
    expect(
        "review_window_change_bound",
        violation(|window| {
            let template = at(&mut window.changes, 0).clone();
            for index in 0..16 {
                let mut extra = template.clone();
                extra.change_id = alloc::format!("change-z{index:02}");
                window.changes.push(extra);
            }
        }),
    );
    expect(
        "review_window_order",
        violation(|window| window.changes.swap(0, 1)),
    );
    expect(
        "review_window_order",
        violation(|window| {
            let duplicate = at(&mut window.changes, 0).clone();
            window.changes.insert(1, duplicate);
        }),
    );
    expect(
        "review_window_text_bound",
        violation(|window| at(&mut window.changes, 0).change_id.clear()),
    );
    expect(
        "review_window_patch_sets",
        violation(|window| {
            let change = change(window, "change-u1");
            change.patch_sets_ms.clear();
        }),
    );
    expect(
        "review_window_patch_sets",
        violation(|window| change(window, "change-a1").patch_sets_ms.swap(0, 1)),
    );
    expect(
        "review_window_patch_sets",
        violation(|window| {
            let change = change(window, "change-u1");
            let first = *at(&mut change.patch_sets_ms, 0);
            change.patch_sets_ms = vec![first; 17];
        }),
    );
    expect(
        "review_window_fact_outside",
        violation(|window| {
            let start = window.start_ms;
            *at(&mut change(window, "change-u1").patch_sets_ms, 0) = start - 1;
        }),
    );
    expect(
        "review_window_fact_outside",
        violation(|window| {
            let end = window.end_ms;
            change(window, "change-u1").patch_sets_ms.push(end);
        }),
    );
    expect(
        "review_window_approval",
        violation(|window| {
            if let Some(approval) = &mut change(window, "change-a1").approval {
                approval.patch_set = 2;
            }
        }),
    );
    expect(
        "review_window_approval",
        violation(|window| {
            let change = change(window, "change-a1");
            let latest = *at(&mut change.patch_sets_ms, 2);
            if let Some(approval) = &mut change.approval {
                approval.approved_ms = latest - 1;
            }
        }),
    );
    expect(
        "review_window_fact_outside",
        violation(|window| {
            let end = window.end_ms;
            if let Some(approval) = &mut change(window, "change-a1").approval {
                approval.approved_ms = end;
            }
        }),
    );
    expect(
        "review_window_suggestion_bound",
        violation(|window| {
            let change = change(window, "change-a1");
            let suggestion = at(&mut change.suggestions, 0).clone();
            change.suggestions = vec![suggestion; 17];
        }),
    );
    expect(
        "review_window_text_bound",
        violation(|window| {
            at(&mut change(window, "change-a1").suggestions, 0).owner = Some("o".repeat(257));
        }),
    );
    expect(
        "review_window_text_bound",
        violation(|window| {
            at(&mut change(window, "change-a1").suggestions, 0)
                .path
                .clear()
        }),
    );
    expect(
        "review_window_fact_outside",
        violation(|window| {
            let first = *at(&mut change(window, "change-a1").patch_sets_ms, 0);
            at(&mut change(window, "change-a1").suggestions, 0).created_ms = first - 1;
        }),
    );
    expect(
        "review_window_fact_outside",
        violation(|window| {
            let end = window.end_ms;
            change(window, "change-a1").merged_ms = Some(end);
        }),
    );
    assert!(admit_review_window(&profile(), &fixture_window()).is_ok());
}

#[test]
fn review_views_admit_only_review_windows_and_declared_bounds() {
    let profile = profile();
    let review = ObservationWindow::review(fixture_window());
    assert_eq!(
        code(project(
            &profile,
            ViewRequest::ReviewFriction,
            &ObservationWindow::empty()
        )),
        Some(String::from("missing_review_window"))
    );
    let mut mixed = review.clone();
    mixed.events = Some(EventWindow {
        records: Vec::new(),
    });
    assert_eq!(
        code(project(&profile, ViewRequest::ReviewFriction, &mixed)),
        Some(String::from("unexpected_window_fact"))
    );
    for view in [
        ViewRequest::StateSummary,
        ViewRequest::RunList {
            after: None,
            limit: 1,
        },
        ViewRequest::EventTail { limit: 1 },
    ] {
        assert_eq!(
            code(project(&profile, view, &review)),
            Some(String::from("unexpected_window_fact"))
        );
    }
    let mut undeclared = profile.clone();
    undeclared.views.remove(&ViewKind::SuggestionAnchors);
    assert_eq!(
        code(project(
            &undeclared,
            ViewRequest::SuggestionAnchors {
                cursor: 0,
                limit: 1
            },
            &review
        )),
        Some(String::from("view_not_admitted"))
    );
    let mut unbounded = profile.clone();
    unbounded.bounds.review = None;
    assert_eq!(
        code(unbounded.admit()),
        Some(String::from("missing_review_bounds"))
    );
    unbounded.views.retain(|kind| !kind.is_review());
    assert!(
        unbounded.admit().is_ok(),
        "pre-review profiles stay admissible"
    );
    let mut zero = profile;
    if let Some(bounds) = &mut zero.bounds.review {
        bounds.max_review_suggestions_per_change = 0;
    }
    assert_eq!(code(zero.admit()), Some(String::from("zero_bound")));
}

#[test]
fn review_view_identity_is_deterministic_and_binds_the_window() {
    let profile = profile();
    let window = ObservationWindow::review(fixture_window());
    let first =
        project(&profile, ViewRequest::ReviewFriction, &window).unwrap_or_else(|_| unreachable!());
    let again =
        project(&profile, ViewRequest::ReviewFriction, &window).unwrap_or_else(|_| unreachable!());
    assert_eq!(first, again);
    let mut shifted = fixture_window();
    shifted.end_ms += 1;
    let other = project(
        &profile,
        ViewRequest::ReviewFriction,
        &ObservationWindow::review(shifted),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        classes(&first),
        classes(&other),
        "only the window end moved"
    );
    assert_ne!(first.identity, other.identity);
    let anchors = project(
        &profile,
        ViewRequest::SuggestionAnchors {
            cursor: 0,
            limit: 2,
        },
        &window,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_ne!(first.identity, anchors.identity);
    let encoded =
        serde_json::to_value(ObservationWindow::empty()).unwrap_or_else(|_| unreachable!());
    assert_eq!(encoded, serde_json::json!({"events": null, "log": null}));
}

fn classes(view: &ObservationView) -> &[AuthorshipFrictionView] {
    match &view.body {
        ViewBody::ReviewFriction(body) => &body.classes,
        _ => unreachable!(),
    }
}

// r[verify kiln.review.authorship.scenario.legacy]
#[test]
fn pre_authorship_patch_sets_replay_undeclared_with_identical_bytes() {
    let legacy = r#"{"id":"r1","number":1,"commit":"c1","base":"b0","files":["src/lib.rs"],"author":"alice","created_ms":7}"#;
    let revision: Revision = serde_json::from_str(legacy).unwrap_or_else(|_| unreachable!());
    assert_eq!(revision.authorship, Authorship::Undeclared);
    assert_eq!(
        serde_json::to_string(&revision).unwrap_or_else(|_| unreachable!()),
        legacy
    );
    let declared = r#"{"id":"r1","number":1,"commit":"c1","base":"b0","files":["src/lib.rs"],"author":"alice","created_ms":7,"authorship":{"kind":"agent","harness":"omp"}}"#;
    let revision: Revision = serde_json::from_str(declared).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        revision.authorship,
        Authorship::Agent {
            harness: String::from("omp"),
            model: None,
        }
    );
    assert_eq!(
        serde_json::to_string(&revision).unwrap_or_else(|_| unreachable!()),
        declared
    );
}

// r[verify kiln.review.authorship.scenario.invalid]
#[test]
fn malformed_authorship_documents_are_rejected() {
    for invalid in [
        r#"{"kind":"robot"}"#,
        r#"{"kind":"agent"}"#,
        r#"{"kind":"agent","harness":"omp","extra":1}"#,
        r#"{"kind":"human","harness":"omp"}"#,
        r#"{"kind":"mixed","model":"claude"}"#,
        r#"{"kind":"undeclared","harness":"omp"}"#,
        r#"{"kind":"human","note":"x"}"#,
        r#"{"harness":"omp"}"#,
        r#""human""#,
    ] {
        assert!(
            serde_json::from_str::<Authorship>(invalid).is_err(),
            "{invalid}"
        );
    }
}
