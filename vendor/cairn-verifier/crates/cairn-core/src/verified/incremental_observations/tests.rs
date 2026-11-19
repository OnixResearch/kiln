use super::*;
use serde_json::{json, Value};

fn hash(seed: char) -> String {
    seed.to_string().repeat(64)
}

fn cohort(events: u64) -> ObservationCohort {
    ObservationCohort::new(
        ".",
        None,
        hash('a'),
        hash('b'),
        hash('c'),
        format!("{events:064x}"),
        ObservationView::ActiveChanges,
    )
    .expect("valid test cohort")
}

fn snapshot(identities: &[(&str, Value)], omitted: usize, complete: bool) -> ObservationSnapshot {
    let entries = identities
        .iter()
        .map(|(identity, content)| ObservationEntry::new(*identity, content.clone()).unwrap())
        .collect();
    build_snapshot(
        cohort(if complete { 1 } else { 0 }),
        entries,
        omitted,
        complete,
    )
    .unwrap()
}

fn codes<T: std::fmt::Debug>(result: Result<T, ObservationIssue>) -> String {
    result.unwrap_err().code
}

#[test]
// r[verify cairn.incremental_observations.identity]
fn complete_snapshot_sorts_entries_and_reports_counts() {
    let snapshot = snapshot(
        &[
            ("change:zeta", json!({"name": "zeta"})),
            ("change:alpha", json!({"name": "alpha"})),
        ],
        0,
        true,
    );
    assert_eq!(snapshot.total, 2);
    assert_eq!(snapshot.included, 2);
    assert_eq!(snapshot.omitted, 0);
    assert!(snapshot.complete);
    assert_eq!(snapshot.state, OBSERVATION_STATE_COMPLETE);
    assert_eq!(snapshot.entries[0].identity, "change:alpha");
    assert_eq!(snapshot.entries[1].identity, "change:zeta");
    assert_eq!(admit_snapshot(&json!(snapshot)).unwrap(), snapshot);
}

#[test]
fn duplicate_entry_identity_never_splits_or_collapses() {
    let result = build_snapshot(
        cohort(1),
        vec![
            ObservationEntry::new("change:alpha", json!({"name": "alpha"})).unwrap(),
            ObservationEntry::new("change:alpha", json!({"name": "shadow"})).unwrap(),
        ],
        0,
        true,
    );
    assert_eq!(codes(result), "observation.duplicate_entry");
}

#[test]
// r[verify cairn.incremental_observations.conformance]
fn oversized_entry_is_an_explicit_error_not_truncation() {
    let oversized = json!({"payload": "x".repeat(bounds().max_entry_bytes + 1)});
    let result = ObservationEntry::new("change:oversized", oversized);
    assert_eq!(codes(result), "observation.entry_bounds");
}

#[test]
fn empty_observations_are_complete_and_page_once() {
    let empty = snapshot(&[], 0, true);
    assert_eq!(empty.total, 0);
    let first = page(&empty, ObservationPageRequest::default()).unwrap();
    assert!(first.entries.is_empty());
    assert!(first.complete);
    assert_eq!(first.next_cursor, None);
    assert_eq!(first.included, 0);
    let diff = difference(&empty, &empty).unwrap();
    assert!(diff.unchanged);
    assert!(diff.added.is_empty() && diff.changed.is_empty() && diff.removed.is_empty());
}

#[test]
// r[verify cairn.incremental_observations.pages]
fn pages_reconstruct_the_complete_observation_without_duplicates() {
    let full = snapshot(
        &[
            ("change:alpha", json!({"value": 1})),
            ("change:beta", json!({"value": 2})),
            ("change:delta", json!({"value": 3})),
            ("change:gamma", json!({"value": 4})),
            ("change:omega", json!({"value": 5})),
        ],
        0,
        true,
    );
    let mut collected = Vec::new();
    let mut request = ObservationPageRequest {
        cursor: None,
        max_entries: Some(2),
        max_bytes: Some(bounds().max_page_bytes),
    };
    loop {
        let current = page(&full, request.clone()).unwrap();
        assert!(current.included <= 2);
        assert_eq!(current.total, 5);
        assert_eq!(current.omitted, 5 - current.included);
        collected.extend(current.entries.iter().cloned());
        match current.next_cursor {
            None => {
                assert!(current.complete);
                assert_eq!(current.state, OBSERVATION_STATE_COMPLETE);
                break;
            }
            Some(cursor) => {
                assert_eq!(current.state, OBSERVATION_STATE_PARTIAL);
                request = ObservationPageRequest {
                    cursor: Some(cursor),
                    max_entries: None,
                    max_bytes: None,
                };
            }
        }
    }
    assert_eq!(collected, full.entries);
    assert_eq!(collected.len(), full.total);
}

#[test]
fn an_entry_is_never_split_to_fit_a_page() {
    let full = snapshot(
        &[("change:alpha", json!({"value": "x".repeat(4096)}))],
        0,
        true,
    );
    assert_eq!(
        codes(page(
            &full,
            ObservationPageRequest {
                cursor: None,
                max_entries: Some(1),
                max_bytes: Some(32),
            }
        )),
        "observation.page_budget"
    );
}

#[test]
// r[verify cairn.incremental_observations.pages]
fn replaying_a_cursor_repeats_the_same_page_without_state() {
    let full = snapshot(
        &[
            ("change:alpha", json!({"value": 1})),
            ("change:beta", json!({"value": 2})),
            ("change:gamma", json!({"value": 3})),
        ],
        0,
        true,
    );
    let first = page(
        &full,
        ObservationPageRequest {
            cursor: None,
            max_entries: Some(1),
            max_bytes: None,
        },
    )
    .unwrap();
    let replay = page(
        &full,
        ObservationPageRequest {
            cursor: first.next_cursor.clone(),
            max_entries: None,
            max_bytes: None,
        },
    )
    .unwrap();
    let second = page(
        &full,
        ObservationPageRequest {
            cursor: first.next_cursor.clone(),
            max_entries: None,
            max_bytes: None,
        },
    )
    .unwrap();
    assert_eq!(replay, second);
    assert_eq!(replay.page_hash, second.page_hash);
    assert_ne!(first.page_hash, replay.page_hash);
}

#[test]
// r[verify cairn.incremental_observations.identity]
fn changed_cohort_or_limits_never_continue_an_old_cursor() {
    let full = snapshot(
        &[
            ("change:alpha", json!({"value": 1})),
            ("change:beta", json!({"value": 2})),
        ],
        0,
        true,
    );
    let first = page(
        &full,
        ObservationPageRequest {
            cursor: None,
            max_entries: Some(1),
            max_bytes: None,
        },
    )
    .unwrap();
    let cursor = first.next_cursor.unwrap();

    let mut drifted = full.clone();
    drifted.cohort.evidence_hash = hash('e');
    drifted.cohort_hash = drifted.cohort.continuation_hash().unwrap();
    drifted.contract_hash = drifted.cohort.contract_hash().unwrap();
    drifted.snapshot_hash = snapshot_hash(&drifted).unwrap();
    assert_eq!(
        codes(page(
            &drifted,
            ObservationPageRequest {
                cursor: Some(cursor.clone()),
                max_entries: None,
                max_bytes: None,
            }
        )),
        "observation.stale_cursor"
    );

    assert_eq!(
        codes(page(
            &full,
            ObservationPageRequest {
                cursor: Some(cursor.clone()),
                max_entries: Some(2),
                max_bytes: None,
            }
        )),
        "observation.page_request"
    );

    let forged = decode_cursor(&cursor).unwrap();
    assert_eq!(forged.max_entries, 1);
}

#[test]
fn forged_cursor_is_rejected_before_any_entry_is_returned() {
    let full = snapshot(
        &[
            ("change:alpha", json!({"value": 1})),
            ("change:beta", json!({"value": 2})),
        ],
        0,
        true,
    );
    let mut cursor = ObservationCursor {
        schema: OBSERVATION_CURSOR_SCHEMA.into(),
        cohort_hash: full.cohort_hash.clone(),
        contract_hash: full.contract_hash.clone(),
        snapshot_hash: full.snapshot_hash.clone(),
        page_contract: OBSERVATION_PAGE_SCHEMA.into(),
        last_identity: "change:alpha".into(),
        max_entries: bounds().max_page_entries + 1,
        max_bytes: bounds().max_page_bytes,
    };
    assert_eq!(
        codes(page(
            &full,
            ObservationPageRequest {
                cursor: Some(encode_cursor(&cursor).unwrap()),
                max_entries: None,
                max_bytes: None,
            }
        )),
        "observation.invalid_cursor"
    );
    cursor.max_entries = 1;
    cursor.last_identity = "change:missing".into();
    assert_eq!(
        codes(page(
            &full,
            ObservationPageRequest {
                cursor: Some(encode_cursor(&cursor).unwrap()),
                max_entries: None,
                max_bytes: None,
            }
        )),
        "observation.invalid_cursor"
    );
}

#[test]
// r[verify cairn.incremental_observations.delta]
fn partial_or_forged_prior_state_cannot_become_a_baseline() {
    let full = snapshot(&[("change:alpha", json!({"value": 1}))], 0, true);
    let partial = snapshot(&[("change:alpha", json!({"value": 1}))], 1, true);
    assert!(!partial.complete);
    assert_eq!(
        codes(difference(&full, &partial)),
        "observation.partial_baseline"
    );
    assert_eq!(
        codes(unchanged(&full, &partial)),
        "observation.partial_baseline"
    );

    let mut forged = json!(full);
    forged["total"] = json!(9);
    assert_eq!(codes(admit_snapshot(&forged)), "observation.forged_counts");

    let mut forged = json!(full);
    forged["entries"][0]["content"]["value"] = json!(2);
    assert_eq!(codes(admit_snapshot(&forged)), "observation.hash_mismatch");

    let mut truncated = json!(full);
    truncated["entries"] = json!([]);
    truncated["included"] = json!(0);
    truncated["total"] = json!(1);
    assert_eq!(
        codes(admit_snapshot(&truncated)),
        "observation.forged_counts"
    );
}

#[test]
// r[verify cairn.incremental_observations.identity]
fn unchanged_comparison_tracks_source_and_contract_identity() {
    let prior = snapshot(&[("change:alpha", json!({"value": 1}))], 0, true);
    let same = admit_snapshot(&json!(prior)).unwrap();
    let comparison = unchanged(&same, &prior).unwrap();
    assert!(comparison.unchanged);
    assert_eq!(comparison.current_hash, comparison.prior_hash);
    assert_eq!(
        comparison.comparison_hash,
        comparison_hash(&comparison).unwrap()
    );

    let edited = snapshot(&[("change:alpha", json!({"value": 2}))], 0, true);
    let result = unchanged(&edited, &prior).unwrap();
    assert!(!result.unchanged);

    let mut drifted = edited;
    drifted.cohort.policy_hash = hash('d');
    drifted.cohort_hash = drifted.cohort.continuation_hash().unwrap();
    drifted.contract_hash = drifted.cohort.contract_hash().unwrap();
    drifted.snapshot_hash = snapshot_hash(&drifted).unwrap();
    assert_eq!(
        codes(unchanged(&drifted, &prior)),
        "observation.mixed_cohort"
    );
}

#[test]
// r[verify cairn.incremental_observations.delta]
fn differences_report_added_changed_and_removed_current_entries() {
    let prior = snapshot(
        &[
            ("change:alpha", json!({"value": 1})),
            ("change:beta", json!({"value": 2})),
            ("change:gamma", json!({"value": 3})),
        ],
        0,
        true,
    );
    let current = snapshot(
        &[
            ("change:alpha", json!({"value": 1})),
            ("change:beta", json!({"value": 20})),
            ("change:delta", json!({"value": 4})),
        ],
        0,
        true,
    );
    let diff = difference(&current, &prior).unwrap();
    assert!(!diff.unchanged);
    assert_eq!(diff.added_count, 1);
    assert_eq!(diff.added[0].identity, "change:delta");
    assert_eq!(diff.changed_count, 1);
    assert_eq!(diff.changed[0].identity, "change:beta");
    assert_eq!(
        diff.changed[0].prior_content_hash,
        prior.entries[1].content_hash
    );
    assert_eq!(diff.removed_count, 1);
    assert_eq!(diff.removed[0].identity, "change:gamma");
    assert_eq!(diff.difference_hash, difference_hash(&diff).unwrap());

    // A renamed stable identity is an addition and a removal, never a change.
    let renamed = snapshot(&[("change:epsilon", json!({"value": 1}))], 0, true);
    let diff = difference(&renamed, &prior).unwrap();
    assert_eq!(diff.added[0].identity, "change:epsilon");
    assert!(diff
        .removed
        .iter()
        .any(|entry| entry.identity == "change:alpha"));
}

#[test]
fn differences_reject_mixed_cohorts_but_allow_source_drift() {
    let prior = snapshot(&[("change:alpha", json!({"value": 1}))], 0, true);
    let mut current = prior.clone();
    current.cohort.source_manifest = hash('f');
    current.cohort.evidence_hash = hash('e');
    current.cohort_hash = current.cohort.continuation_hash().unwrap();
    current.contract_hash = current.cohort.contract_hash().unwrap();
    current.snapshot_hash = snapshot_hash(&current).unwrap();
    // Source and evidence drift is reported by identity, not as entry changes.
    let diff = difference(&current, &prior).unwrap();
    assert!(!diff.unchanged);
    assert!(diff.added.is_empty() && diff.changed.is_empty() && diff.removed.is_empty());
    assert_ne!(diff.current_hash, diff.prior_hash);

    let mut mixed = current.clone();
    mixed.cohort.root = "other-root".into();
    mixed.cohort_hash = mixed.cohort.continuation_hash().unwrap();
    mixed.contract_hash = mixed.cohort.contract_hash().unwrap();
    mixed.snapshot_hash = snapshot_hash(&mixed).unwrap();
    assert_eq!(
        codes(difference(&mixed, &prior)),
        "observation.mixed_cohort"
    );

    let mut readiness = current.clone();
    readiness.cohort.view = ObservationView::Readiness {
        change: "demo".into(),
        max_bytes: None,
        max_entries: None,
    };
    readiness.cohort.producer_contract = readiness.cohort.view.producer_contract().into();
    readiness.cohort_hash = readiness.cohort.continuation_hash().unwrap();
    readiness.contract_hash = readiness.cohort.contract_hash().unwrap();
    readiness.snapshot_hash = snapshot_hash(&readiness).unwrap();
    assert_eq!(
        codes(difference(&readiness, &prior)),
        "observation.mixed_cohort"
    );
}

#[test]
fn views_are_selector_bound_and_ordering_is_versioned() {
    let view = ObservationView::Context {
        scope: "active".into(),
        change: Some("demo".into()),
        spec: None,
        artifact: "tasks".into(),
        selectors: vec![crate::verified::validation::ContextSelector::Task { id: "1.1".into() }],
    };
    let bound = ObservationCohort::new(".", None, hash('a'), hash('b'), hash('c'), hash('d'), view)
        .unwrap();
    let first = bound.contract_hash().unwrap();
    let mut other = bound.clone();
    other.view = ObservationView::Context {
        scope: "active".into(),
        change: Some("demo".into()),
        spec: None,
        artifact: "tasks".into(),
        selectors: vec![crate::verified::validation::ContextSelector::Task { id: "1.2".into() }],
    };
    assert_ne!(first, other.contract_hash().unwrap());

    let mut unsupported = bound;
    unsupported.ordering = "offset-v1".into();
    assert_eq!(codes(validate_cohort(&unsupported)), "observation.ordering");
}
