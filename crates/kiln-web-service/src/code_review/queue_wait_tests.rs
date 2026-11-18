//! Load admission, outcome classification, summaries and receipt read-back.
use serde_json::{Value, json};

use super::*;

const FROZEN_LOAD: &[u8] = include_bytes!("../../../../fixtures/review-queue-wait/load.json");

fn frozen() -> Value {
    serde_json::from_slice(FROZEN_LOAD).unwrap_or_else(|error| unreachable!("frozen load: {error}"))
}

fn admit(value: &Value) -> Result<(Load, String), String> {
    let bytes = serde_json::to_vec(value).unwrap_or_else(|error| unreachable!("{error}"));
    Load::admit(&bytes)
}

/// Replace the value at `pointer`, adding the final key when it is absent.
fn set(value: &mut Value, pointer: &str, replacement: Value) {
    if let Some(slot) = value.pointer_mut(pointer) {
        *slot = replacement;
        return;
    }
    let (parent, key) = pointer
        .rsplit_once('/')
        .unwrap_or_else(|| unreachable!("fixture pointer {pointer}"));
    value
        .pointer_mut(parent)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| unreachable!("fixture parent {parent}"))
        .insert(key.to_owned(), replacement);
}

fn edited(edit: impl Fn(&mut Value)) -> Value {
    let mut value = frozen();
    edit(&mut value);
    value
}

// r[verify kiln.admission.queue_wait.scenario.load]
#[test]
fn frozen_load_is_admitted_with_a_stable_identity() {
    let (load, identity) = admit(&frozen()).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(identity.len(), 64);
    assert_eq!(load.candidates.len(), 8);
    assert_eq!(load.horizon_ms(), 40_000);
    // Same typed load, same identity, whatever the JSON formatting.
    let reformatted = serde_json::to_vec_pretty(&frozen()).unwrap_or_default();
    assert_eq!(
        Load::admit(&reformatted).map(|(_, id)| id),
        Ok(identity.clone())
    );
    let slower = edited(|value| set(value, "/tick_ms", json!(2000)));
    assert_ne!(admit(&slower).map(|(_, id)| id), Ok(identity));
}

// r[verify kiln.admission.queue_wait.scenario.load]
#[test]
fn malformed_or_unbounded_loads_are_refused() {
    let many: Vec<Value> = (0..=MAX_CANDIDATES)
        .map(|index| json!({"id":format!("c{index}"),"arrival_tick":0,"ci":"pass","ci_ticks":1}))
        .collect();
    let refusals: Vec<(&str, Value)> = vec![
        (
            "schema",
            edited(|value| set(value, "/schema", json!("kiln.review-queue-wait.load.v2"))),
        ),
        (
            "zero tick",
            edited(|value| set(value, "/tick_ms", json!(0))),
        ),
        (
            "long tick",
            edited(|value| set(value, "/tick_ms", json!(MAX_TICK_MS + 1))),
        ),
        (
            "zero horizon",
            edited(|value| set(value, "/horizon_ticks", json!(0))),
        ),
        (
            "long horizon",
            edited(|value| set(value, "/horizon_ticks", json!(MAX_HORIZON_TICKS + 1))),
        ),
        (
            "zero bound",
            edited(|value| set(value, "/max_entries", json!(0))),
        ),
        (
            "no candidates",
            edited(|value| set(value, "/candidates", json!([]))),
        ),
        (
            "too many",
            edited(|value| set(value, "/candidates", Value::Array(many.clone()))),
        ),
        (
            "duplicate id",
            edited(|value| set(value, "/candidates/1/id", json!("alpha"))),
        ),
        (
            "uppercase id",
            edited(|value| set(value, "/candidates/0/id", json!("Alpha"))),
        ),
        (
            "late arrival",
            edited(|value| set(value, "/candidates/0/arrival_tick", json!(41))),
        ),
        (
            "instant ci",
            edited(|value| set(value, "/candidates/0/ci_ticks", json!(0))),
        ),
        (
            "slow ci",
            edited(|value| set(value, "/candidates/0/ci_ticks", json!(MAX_CI_TICKS + 1))),
        ),
        (
            "zero deadline",
            edited(|value| set(value, "/candidates/3/deadline_ticks", json!(0))),
        ),
        (
            "ci value",
            edited(|value| set(value, "/candidates/0/ci", json!("flaky"))),
        ),
        ("no modes", edited(|value| set(value, "/modes", json!([])))),
        (
            "duplicate mode",
            edited(|value| set(value, "/modes/1", json!({"mode":"serial"}))),
        ),
        (
            "serial window",
            edited(|value| {
                set(
                    value,
                    "/modes/0/window",
                    json!({"floor":1,"ceiling":1,"increase":1,"decrease":1}),
                )
            }),
        ),
        (
            "bare speculative",
            edited(|value| set(value, "/modes/1", json!({"mode":"speculative"}))),
        ),
        (
            "inverted window",
            edited(|value| set(value, "/modes/1/window/floor", json!(4))),
        ),
        (
            "window over bound",
            edited(|value| set(value, "/max_entries", json!(2))),
        ),
        (
            "idle window",
            edited(|value| set(value, "/modes/1/window/increase", json!(0))),
        ),
        (
            "unknown field",
            edited(|value| set(value, "/candidates/0/priority", json!(1))),
        ),
    ];
    for (name, value) in refusals {
        assert!(admit(&value).is_err(), "{name} admitted");
    }
    assert_eq!(
        Load::admit(b"not json").map(|(_, id)| id),
        Err("queue_wait_load_malformed".to_owned())
    );
}

fn observed(status: &str, detail: Option<&str>) -> Observed {
    Observed {
        candidate: "alpha".into(),
        source_commit: "a".repeat(40),
        enqueue_ms: Some(0),
        status: Some(status.into()),
        detail: detail.map(str::to_owned),
        ..Observed::default()
    }
}

// r[verify kiln.admission.queue_wait.scenario.outcomes]
#[test]
fn outcomes_follow_recorded_signals_only() {
    let unknown_ci = Observed {
        ci_unknown: true,
        ..observed("waiting_ci", None)
    };
    let withdrawn = Observed {
        withdrawn_ms: Some(9_000),
        ..observed(
            "cancelled",
            Some("Cancelled by authenticated review participant"),
        )
    };
    let refused = Observed {
        refusal: Some("review_queue_bound".into()),
        enqueue_ms: None,
        status: None,
        ..observed("queued", None)
    };
    let owned = |code: &str| Some(code.to_owned());
    for (case, expected) in [
        (refused, (Outcome::Rejected, owned("review_queue_bound"))),
        (
            observed("submitted", Some("landed")),
            (Outcome::Submitted, None),
        ),
        (withdrawn, (Outcome::Expired, owned(WITHDRAWN))),
        (
            observed("cancelled", Some("by owner")),
            (Outcome::Cancelled, owned("by owner")),
        ),
        (
            observed("blocked", Some(CI_FAILED)),
            (Outcome::Failed, owned(CI_FAILED)),
        ),
        (
            observed("blocked", Some("review_queue_authority_changed")),
            (Outcome::Rejected, owned("review_queue_authority_changed")),
        ),
        (
            observed("uncertain", None),
            (Outcome::Unknown, owned("provider_operation_uncertain")),
        ),
        (unknown_ci, (Outcome::Unknown, owned("ci_outcome_unknown"))),
        (
            observed("waiting_ci", None),
            (Outcome::Waiting, owned("waiting_ci")),
        ),
        (
            observed("queued", None),
            (Outcome::Waiting, owned("queued")),
        ),
        (
            observed("parked", None),
            (Outcome::Unknown, owned("unrecognized_status:parked")),
        ),
        (
            Observed {
                status: None,
                ..observed("queued", None)
            },
            (Outcome::Unknown, owned("not_observed")),
        ),
    ] {
        assert_eq!(classify(&case), expected, "{case:?}");
    }
    // A candidate that never left the queue is never terminal.
    assert!(!Outcome::Waiting.is_terminal() && !Outcome::Unknown.is_terminal());
    assert!(Outcome::Expired.is_terminal() && Outcome::Submitted.is_terminal());
}

fn record(observed: Observed) -> CandidateRecord {
    let (outcome, code) = classify(&observed);
    CandidateRecord {
        observed,
        outcome,
        code,
    }
}

// r[verify kiln.admission.queue_wait.scenario.series]
#[test]
fn summaries_use_nearest_rank_and_never_invent_waits() {
    assert_eq!(distribution(Vec::new()), None);
    assert_eq!(
        distribution(vec![5]),
        Some(Distribution {
            samples: 1,
            min: 5,
            p50: 5,
            p95: 5,
            max: 5
        })
    );
    assert_eq!(
        distribution((1..=20).rev().collect()),
        Some(Distribution {
            samples: 20,
            min: 1,
            p50: 10,
            p95: 19,
            max: 20
        })
    );
    let submitted = Observed {
        start_ms: Some(2_000),
        ci_requested_ms: Some(3_000),
        ci_dispatches_ms: vec![5_000, 7_000],
        terminal_ms: Some(9_000),
        ..observed("submitted", None)
    };
    let waiting = Observed {
        enqueue_ms: Some(1_000),
        ..observed("queued", None)
    };
    let refused = Observed {
        refusal: Some("review_queue_bound".into()),
        enqueue_ms: None,
        terminal_ms: Some(3_000),
        arrival_ms: 3_000,
        ..observed("queued", None)
    };
    let summary = summarize(&[record(submitted), record(waiting), record(refused)]);
    assert_eq!(summary.candidates, 3);
    assert_eq!(
        summary.outcomes,
        OutcomeCounts {
            submitted: 1,
            waiting: 1,
            rejected: 1,
            ..OutcomeCounts::default()
        }
    );
    let single = |wait| {
        Some(Distribution {
            samples: 1,
            min: wait,
            p50: wait,
            p95: wait,
            max: wait,
        })
    };
    assert_eq!(summary.enqueue_to_start_ms, single(2_000));
    assert_eq!(summary.enqueue_to_ci_dispatch_ms, single(5_000));
    assert_eq!(summary.enqueue_to_submitted_ms, single(9_000));
}

fn small_load() -> Load {
    serde_json::from_value(json!({
        "schema":LOAD_SCHEMA,"tick_ms":1000,"horizon_ticks":10,"max_entries":2,"modes":[{"mode":"serial"}],
        "candidates":[{"id":"alpha","arrival_tick":0,"ci":"pass","ci_ticks":3},
            {"id":"bravo","arrival_tick":0,"ci":"fail","ci_ticks":2},
            {"id":"charlie","arrival_tick":1,"ci":"pass","ci_ticks":1}]
    }))
    .unwrap_or_else(|error| unreachable!("small load: {error}"))
}

fn small_receipt() -> Value {
    let alpha = Observed {
        start_ms: Some(0),
        ci_requested_ms: Some(1_000),
        ci_dispatches_ms: vec![1_000],
        terminal_ms: Some(4_000),
        candidate_commits: vec!["c".repeat(40)],
        ci_runs: vec!["run-alpha".into()],
        ..observed("submitted", Some("landed"))
    };
    let bravo = Observed {
        candidate: "bravo".into(),
        start_ms: Some(5_000),
        ci_requested_ms: Some(6_000),
        ci_dispatches_ms: vec![6_000],
        terminal_ms: Some(8_000),
        ..observed("blocked", Some(CI_FAILED))
    };
    let charlie = Observed {
        candidate: "charlie".into(),
        arrival_ms: 1_000,
        refusal: Some("review_queue_bound".into()),
        enqueue_ms: None,
        terminal_ms: Some(1_000),
        status: None,
        ..Observed::default()
    };
    let receipt = Receipt::new(
        &small_load(),
        vec![ArmObservation {
            mode: ModeSpec {
                mode: Mode::Serial,
                window: None,
            },
            ci_slots: 1,
            max_candidate_builds: 16,
            observed: vec![alpha, bravo, charlie],
        }],
    )
    .unwrap_or_else(|error| unreachable!("small receipt: {error}"));
    serde_json::to_value(receipt).unwrap_or_else(|error| unreachable!("{error}"))
}

fn read_back(value: &Value) -> Result<Receipt, String> {
    let bytes = serde_json::to_vec(value).unwrap_or_else(|error| unreachable!("{error}"));
    Receipt::admit(&bytes)
}

/// One named change to a serialized receipt.
type Edit<'a> = (&'a str, Box<dyn Fn(&mut Value) + 'a>);

// r[verify kiln.admission.queue_wait.scenario.series]
#[test]
fn receipts_read_back_only_when_every_series_and_summary_recomputes() {
    let receipt = small_receipt();
    let admitted = read_back(&receipt).unwrap_or_else(|error| unreachable!("{error}"));
    let series = &admitted.series;
    for unmeasured in [
        &series.merge_queue_wait,
        &series.ready_work_admission_wait,
        &series.inference_backpressure,
    ] {
        assert!(
            matches!(&unmeasured.measurement, Measurement::NotMeasured { reason } if !reason.is_empty())
        );
    }
    let arms = "/series/review_queue_wait/measurement/value/arms";
    assert_eq!(
        receipt.pointer(&format!("{arms}/0/summary/outcomes")),
        Some(
            &json!({"submitted":1,"waiting":0,"rejected":1,"expired":0,"failed":1,"unknown":0,"cancelled":0})
        )
    );
    let review = receipt
        .pointer("/series/review_queue_wait/measurement")
        .cloned()
        .unwrap_or_default();
    let candidate = |index: usize, field: &str| format!("{arms}/0/candidates/{index}/{field}");
    let edits: Vec<Edit<'_>> = vec![
        (
            "relabelled series",
            Box::new(|value| {
                set(
                    value,
                    "/series/merge_queue_wait/series",
                    json!("review_queue_wait"),
                )
            }),
        ),
        (
            "swapped slots",
            Box::new(|value| {
                set(
                    value,
                    "/series/review_queue_wait/series",
                    json!("merge_queue_wait"),
                )
            }),
        ),
        (
            "merge numbers",
            Box::new(move |value| {
                set(
                    value,
                    "/series/merge_queue_wait/measurement",
                    review.clone(),
                )
            }),
        ),
        (
            "empty reason",
            Box::new(|value| {
                set(
                    value,
                    "/series/inference_backpressure/measurement/reason",
                    json!(" "),
                )
            }),
        ),
        (
            "edited count",
            Box::new(move |value| {
                set(
                    value,
                    &format!("{arms}/0/summary/outcomes/failed"),
                    json!(0),
                )
            }),
        ),
        (
            "edited wait",
            Box::new(move |value| {
                set(
                    value,
                    &format!("{arms}/0/summary/enqueue_to_start_ms/max"),
                    json!(1),
                )
            }),
        ),
        (
            "relabelled outcome",
            Box::new(move |value| set(value, &candidate(1, "outcome"), json!("submitted"))),
        ),
        (
            "edited code",
            Box::new(move |value| {
                set(
                    value,
                    &candidate(2, "code"),
                    json!("review_queue_ci_failed"),
                )
            }),
        ),
        (
            "unordered times",
            Box::new(move |value| set(value, &candidate(0, "observed/start_ms"), json!(5_000))),
        ),
        (
            "past horizon",
            Box::new(move |value| set(value, &candidate(1, "observed/terminal_ms"), json!(11_000))),
        ),
        (
            "dispatch without request",
            Box::new(move |value| {
                set(
                    value,
                    &candidate(1, "observed/ci_requested_ms"),
                    Value::Null,
                )
            }),
        ),
        (
            "unordered dispatches",
            Box::new(move |value| {
                set(
                    value,
                    &candidate(0, "observed/ci_dispatches_ms"),
                    json!([3_000, 1_000]),
                )
            }),
        ),
        (
            "dispatch before request",
            Box::new(move |value| {
                set(
                    value,
                    &candidate(0, "observed/ci_dispatches_ms"),
                    json!([500]),
                )
            }),
        ),
        (
            "edited load",
            Box::new(|value| set(value, "/load/tick_ms", json!(2000))),
        ),
        (
            "non-claims",
            Box::new(|value| set(value, "/non_claims/0", json!("Production latency"))),
        ),
        (
            "unknown field",
            Box::new(|value| set(value, "/series/merge_queue_wait/extra", json!(1))),
        ),
        (
            "schema",
            Box::new(|value| set(value, "/schema", json!("kiln.review-queue-wait.receipt.v2"))),
        ),
    ];
    for (name, edit) in edits {
        let mut changed = receipt.clone();
        edit(&mut changed);
        assert!(read_back(&changed).is_err(), "{name} admitted");
    }
    let mut reordered = receipt.clone();
    if let Some(candidates) = reordered
        .pointer_mut(&format!("{arms}/0/candidates"))
        .and_then(Value::as_array_mut)
    {
        candidates.swap(0, 1);
    }
    assert!(
        read_back(&reordered).is_err(),
        "reordered candidates admitted"
    );
    assert_eq!(
        Receipt::admit(&vec![b' '; MAX_RECEIPT_BYTES + 1]).map(|_| ()),
        Err("queue_wait_receipt_invalid: size".to_owned())
    );
}
