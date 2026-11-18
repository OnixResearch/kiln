//! The frozen review-queue wait load (#61) through the real review queue in
//! serial and speculative modes, compared with its frozen outcome table.
//! Refresh `fixtures/review-queue-wait/expected.json` only after reviewing a
//! deliberate queue or load change.
#[path = "../examples/review_queue_wait/harness.rs"]
mod harness;

use kiln_web_service::code_review::queue_wait::{Load, Measurement, Mode, Outcome, Receipt};
use serde_json::{Value, json};

const LOAD: &[u8] = include_bytes!("../../../fixtures/review-queue-wait/load.json");
const EXPECTED: &[u8] = include_bytes!("../../../fixtures/review-queue-wait/expected.json");

/// Outcomes, times and status transitions per mode, without run-specific identities.
fn table(receipt: &Receipt) -> Value {
    let Measurement::Measured { value } = &receipt.series.review_queue_wait.measurement else {
        return Value::Null;
    };
    let mut modes = serde_json::Map::new();
    for arm in &value.arms {
        let rows: Vec<Value> = arm
            .candidates
            .iter()
            .map(|record| {
                let observed = &record.observed;
                json!({"candidate":observed.candidate,"outcome":record.outcome,"code":record.code,
                    "arrival_ms":observed.arrival_ms,"enqueue_ms":observed.enqueue_ms,"start_ms":observed.start_ms,
                    "ci_requested_ms":observed.ci_requested_ms,"ci_dispatches_ms":observed.ci_dispatches_ms,
                    "terminal_ms":observed.terminal_ms,"withdrawn_ms":observed.withdrawn_ms,
                    "candidate_builds":observed.candidate_commits.len(),"ci_runs":observed.ci_runs.len(),
                    "statuses":observed.transitions.iter().map(|step| json!([step.at_ms, step.status])).collect::<Vec<_>>()})
            })
            .collect();
        let name = match arm.mode.mode {
            Mode::Serial => "serial",
            Mode::Speculative => "speculative",
        };
        modes.insert(
            name.into(),
            json!({"summary":arm.summary,"candidates":rows}),
        );
    }
    Value::Object(modes)
}

// r[verify kiln.admission.queue_wait]
// r[verify kiln.admission.queue_wait.scenario.outcomes]
// r[verify kiln.admission.queue_wait.scenario.series]
#[test]
fn frozen_load_records_named_outcomes_in_both_queue_modes() -> harness::Result {
    let (load, identity) = Load::admit(LOAD)?;
    let receipt = Receipt::new(&load, harness::run(&load)?)?;
    assert_eq!(Receipt::admit(&serde_json::to_vec(&receipt)?)?, receipt);
    assert_eq!(receipt.load_identity, identity);
    let series = &receipt.series;
    for unmeasured in [
        &series.merge_queue_wait,
        &series.ready_work_admission_wait,
        &series.inference_backpressure,
    ] {
        assert!(matches!(
            unmeasured.measurement,
            Measurement::NotMeasured { .. }
        ));
    }
    let Measurement::Measured { value } = &series.review_queue_wait.measurement else {
        return Err("review-queue wait was not measured".into());
    };
    assert_eq!(value.arms.len(), 2);
    for arm in &value.arms {
        for record in &arm.candidates {
            let observed = &record.observed;
            // The pinned source, and every exact candidate and run the queue used.
            assert_eq!(observed.source_commit.len(), 40, "{}", observed.candidate);
            if observed.ci_requested_ms.is_some() {
                assert!(
                    !observed.candidate_commits.is_empty() && !observed.ci_runs.is_empty(),
                    "{}",
                    observed.candidate
                );
            }
            assert!(
                record.outcome != Outcome::Submitted || observed.terminal_ms.is_some(),
                "{}",
                observed.candidate
            );
        }
    }
    let actual = table(&receipt);
    let expected: Value = serde_json::from_slice(EXPECTED)?;
    assert_eq!(
        actual,
        expected,
        "refresh fixtures/review-queue-wait/expected.json only after review:\n{}",
        serde_json::to_string_pretty(&actual)?
    );
    Ok(())
}
