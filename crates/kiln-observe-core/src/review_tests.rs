use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use kiln_core::change_review::{
    AgentIdentity, AttestedSubmission, Authorship, AuthorshipClass, Command, Facts,
    OwnershipPolicy, ReviewState, Revision, apply,
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
            advisory_approvals: 0,
            advised_changes: 0,
            agent_submitted_changes: 0,
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
            advisory_approvals: 0,
            advised_changes: 0,
            agent_submitted_changes: 0,
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
            advisory_approvals: 0,
            advised_changes: 0,
            agent_submitted_changes: 0,
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
            advisory_approvals: 0,
            advised_changes: 0,
            agent_submitted_changes: 0,
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

fn advisory_window() -> ReviewWindow {
    serde_json::from_str(include_str!(
        "../../../fixtures/observation/review-window-advisory-v1.json"
    ))
    .unwrap_or_else(|_| unreachable!())
}

/// Replay a journal that adds advisory approvals across two revisions.
fn advisory_state() -> ReviewState {
    let (_, mut state) = replay_fixture_journal();
    let mut ids: Vec<String> = state.changes.keys().cloned().collect();
    ids.sort();
    let first = ids.first().cloned().unwrap_or_else(|| unreachable!());
    let change = state
        .changes
        .get_mut(&first)
        .unwrap_or_else(|| unreachable!());
    let latest = change
        .revisions
        .last()
        .map(|revision| (revision.id.clone(), revision.created_ms))
        .unwrap_or_else(|| unreachable!());
    let earlier = change.revisions.first().map(|revision| revision.id.clone());
    for (actor, revision, created_ms) in [
        ("dave", latest.0.clone(), latest.1 + 1),
        ("erin", latest.0.clone(), latest.1 + 2),
        // After the window end: excluded.
        ("frank", latest.0.clone(), i64::MAX - 1),
    ] {
        change
            .advisory
            .push(kiln_core::change_review::AdvisoryApproval {
                actor: actor.into(),
                revision,
                created_ms,
            });
    }
    if let Some(earlier) = earlier.filter(|earlier| *earlier != latest.0) {
        // Historical: bound to an earlier revision, never counted.
        change
            .advisory
            .push(kiln_core::change_review::AdvisoryApproval {
                actor: "gina".into(),
                revision: earlier,
                created_ms: latest.1,
            });
    }
    state
}

// r[verify kiln.observe.review.advisory]
#[test]
fn review_window_exports_current_advisory_counts_only() {
    let (fixture, _) = replay_fixture_journal();
    let state = advisory_state();
    let window = review_window(
        &state,
        Some(&fixture.ownership),
        fixture.start_ms,
        fixture.end_ms,
    )
    .unwrap_or_else(|_| unreachable!());
    let counts: Vec<u32> = window
        .changes
        .iter()
        .map(|change| change.advisory_approvals)
        .collect();
    assert_eq!(counts.first().copied(), Some(2));
    assert!(counts.iter().skip(1).all(|count| *count == 0));
    // Advisory approvals never become approval facts.
    let (_, plain) = replay_fixture_journal();
    let baseline = review_window(
        &plain,
        Some(&fixture.ownership),
        fixture.start_ms,
        fixture.end_ms,
    )
    .unwrap_or_else(|_| unreachable!());
    for (with, without) in window.changes.iter().zip(&baseline.changes) {
        assert_eq!(with.approval, without.approval);
    }
    let encoded = serde_json::to_value(&window).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        encoded.pointer("/changes/0/advisory_approvals"),
        Some(&serde_json::json!(2))
    );
    assert!(encoded.pointer("/changes/1/advisory_approvals").is_none());
}

// r[verify kiln.observe.review.advisory]
#[test]
fn friction_reports_advisory_totals_per_class_and_bounds_them() {
    let view = friction(advisory_window());
    let totals: Vec<(AuthorshipClass, u64, u64)> = view
        .classes
        .iter()
        .map(|class| {
            (
                class.authorship,
                class.advisory_approvals,
                class.advised_changes,
            )
        })
        .collect();
    assert_eq!(
        totals,
        vec![
            (AuthorshipClass::Human, 3, 1),
            (AuthorshipClass::Agent, 3, 2),
            (AuthorshipClass::Mixed, 1, 1),
            (AuthorshipClass::Undeclared, 0, 0),
        ]
    );
    // Advisory counts change nothing else in the view.
    let plain = friction(fixture_window());
    for (with, without) in view.classes.iter().zip(&plain.classes) {
        let mut stripped = with.clone();
        stripped.advisory_approvals = 0;
        stripped.advised_changes = 0;
        assert_eq!(&stripped, without);
    }
    assert_eq!(
        violation(|window| change(window, "change-a2").advisory_approvals =
            MAX_REVIEW_ADVISORY_PER_CHANGE + 1),
        Some(String::from("review_window_advisory_bound"))
    );
    assert_eq!(
        violation(|window| change(window, "change-a2").advisory_approvals =
            MAX_REVIEW_ADVISORY_PER_CHANGE),
        None
    );
}

// r[verify kiln.review.advisory.legacy]
// r[verify kiln.observe.review.advisory]
#[test]
fn advisory_free_windows_keep_bytes_and_identities() {
    // The v1 fixture predates advisory approvals: it round-trips byte-identically.
    let raw = include_str!("../../../fixtures/observation/review-window-v1.json");
    let parsed: serde_json::Value = serde_json::from_str(raw).unwrap_or_else(|_| unreachable!());
    let window = fixture_window();
    assert_eq!(
        serde_json::to_value(&window).unwrap_or_else(|_| unreachable!()),
        parsed
    );
    let profile = profile();
    let friction_view = project(
        &profile,
        ViewRequest::ReviewFriction,
        &ObservationWindow::review(fixture_window()),
    )
    .unwrap_or_else(|_| unreachable!());
    let anchors_view = project(
        &profile,
        ViewRequest::SuggestionAnchors {
            cursor: 0,
            limit: 2,
        },
        &ObservationWindow::review(fixture_window()),
    )
    .unwrap_or_else(|_| unreachable!());
    // Identities computed by the pre-advisory encoder for the same inputs.
    assert_eq!(friction_view.identity.as_str(), LEGACY_FRICTION_IDENTITY);
    assert_eq!(anchors_view.identity.as_str(), LEGACY_ANCHORS_IDENTITY);
    // Non-zero advisory counts are bound into the identity.
    let advised = project(
        &profile,
        ViewRequest::ReviewFriction,
        &ObservationWindow::review(advisory_window()),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_ne!(advised.identity, friction_view.identity);
    let mut other = advisory_window();
    change(&mut other, "change-a1").advisory_approvals = 1;
    change(&mut other, "change-a3").advisory_approvals = 2;
    let swapped = project(
        &profile,
        ViewRequest::ReviewFriction,
        &ObservationWindow::review(other),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        classes(&swapped),
        classes(&advised),
        "same per-class totals"
    );
    assert_ne!(swapped.identity, advised.identity, "window facts differ");
}

const LEGACY_FRICTION_IDENTITY: &str =
    "9608269f3ef060dd0be8de580ddd4e6cc3090271f705bf021bfc6f6969b822d7";
const LEGACY_ANCHORS_IDENTITY: &str =
    "6dd4a50aa7e4f071011219a29c126ccf2c9a2578fdfdee5d0af987fd31774531";

#[test]
fn review_window_marks_attested_agent_revisions() {
    let (fixture, mut state) = replay_fixture_journal();
    let change_id = state
        .changes
        .keys()
        .next()
        .unwrap_or_else(|| unreachable!("fixture change"))
        .clone();
    state
        .changes
        .get_mut(&change_id)
        .unwrap_or_else(|| unreachable!("fixture change"))
        .revisions
        .first_mut()
        .unwrap_or_else(|| unreachable!("fixture revision"))
        .submitted_by = Some(AttestedSubmission {
        principal: "agent".into(),
        agent: Some(AgentIdentity {
            harness: "omp".into(),
            model: None,
        }),
    });
    let window = review_window(&state, None, fixture.start_ms, fixture.end_ms)
        .unwrap_or_else(|_| unreachable!("review window"));
    let change = window
        .changes
        .iter()
        .find(|change| change.change_id == change_id)
        .unwrap_or_else(|| unreachable!("change in window"));
    assert!(change.agent_submitted);
    assert!(change.agent_harnesses.contains("omp"));
}

#[test]
fn friction_counts_attested_agent_submissions() {
    let mut window = fixture_window();
    let first = window
        .changes
        .first_mut()
        .unwrap_or_else(|| unreachable!("fixture change"));
    first.agent_submitted = true;
    let view = friction(window);
    assert_eq!(
        view.classes
            .iter()
            .map(|row| row.agent_submitted_changes)
            .sum::<u64>(),
        1
    );
}

#[test]
fn agent_submission_counts_change_friction_identity_only_when_non_zero() {
    let identity = |window: ReviewWindow| {
        project(
            &profile(),
            ViewRequest::ReviewFriction,
            &ObservationWindow::review(window),
        )
        .unwrap_or_else(|_| unreachable!("friction projection"))
        .identity
    };
    let baseline = fixture_window();
    let baseline_identity = identity(baseline.clone());
    let mut attested = baseline;
    attested
        .changes
        .first_mut()
        .unwrap_or_else(|| unreachable!("fixture change"))
        .agent_submitted = true;
    let attested_identity = identity(attested.clone());
    assert_ne!(baseline_identity, attested_identity);
    let mut two = attested;
    two.changes
        .get_mut(1)
        .unwrap_or_else(|| unreachable!("fixture change"))
        .agent_submitted = true;
    assert_ne!(attested_identity, identity(two));
}

#[test]
fn friction_aggregates_declared_agent_harnesses() {
    let identity = |window: ReviewWindow| {
        project(
            &profile(),
            ViewRequest::ReviewFriction,
            &ObservationWindow::review(window),
        )
        .unwrap_or_else(|_| unreachable!("friction projection"))
        .identity
    };
    let baseline = fixture_window();
    let baseline_identity = identity(baseline.clone());
    let mut harnessed = baseline;
    harnessed
        .changes
        .first_mut()
        .unwrap_or_else(|| unreachable!("fixture change"))
        .agent_harnesses
        .insert("omp".into());
    let view = friction(harnessed.clone());
    assert_eq!(view.agent_harnesses.get("omp"), Some(&1));
    assert_ne!(baseline_identity, identity(harnessed));
}
