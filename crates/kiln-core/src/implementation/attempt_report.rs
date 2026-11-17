//! Read-only attempt outcome and effort accounting over a claim-time window.
//!
//! Ratios retain integer numerators and denominators. No telemetry is inferred
//! from event counts, prose, worker success, or missing observations. These
//! observations neither grant authority nor rank worker configurations.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::{
    AttemptLedger, AttemptPhase, ImplementationAttempt, ModelTurnObservation, VerificationOutcome,
    WorkerTerminalState,
};
use crate::types::{ContentId, CoreResult, Diagnostic, ProviderProfileId};

pub const ATTEMPT_REPORT_SCHEMA: &str = "kiln.implementation-attempt-report.v1";

/// An exact, unreduced ratio; a zero denominator is represented by `None`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptRatio {
    pub numerator: u64,
    pub denominator: u64,
}

impl AttemptRatio {
    fn new(numerator: u64, denominator: u64) -> Option<Self> {
        (denominator != 0).then_some(Self {
            numerator,
            denominator,
        })
    }
}

/// Coverage and raw effort for one finished-attempt cohort. Incomplete values
/// are lower bounds, never part of `complete_total` or complete-cohort ratios.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptEffortCoverage {
    pub complete_samples: u64,
    pub absent_samples: u64,
    pub incomplete_samples: u64,
    pub complete_total: u64,
    pub incomplete_lower_bound: u64,
}

impl AttemptEffortCoverage {
    fn add(&mut self, value: &ModelTurnObservation) -> CoreResult<()> {
        match value {
            ModelTurnObservation::Observed { turns } => {
                add(&mut self.complete_samples, 1)?;
                add(&mut self.complete_total, *turns)?;
            }
            ModelTurnObservation::Absent { .. } => add(&mut self.absent_samples, 1)?,
            ModelTurnObservation::Incomplete { turns, .. } => {
                add(&mut self.incomplete_samples, 1)?;
                add(&mut self.incomplete_lower_bound, *turns)?;
            }
        }
        Ok(())
    }

    fn complete(&self) -> bool {
        self.absent_samples == 0 && self.incomplete_samples == 0
    }
}

/// The units are model requests in `turns`, milliseconds in `elapsed_ms`.
///
/// Full-window per-pass = all finished effort / all verified passes; failed
/// share = nonpass effort / all effort. These ratios require complete coverage
/// of all finished attempts. Verified-pass mean requires complete coverage of
/// the pass cohort only. Complete-cohort variants use only complete samples in
/// both their numerator and denominator, and do not estimate missing effort.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptEffortSummary {
    pub all: AttemptEffortCoverage,
    pub verified_pass: AttemptEffortCoverage,
    pub nonpass: AttemptEffortCoverage,
    pub full_window_complete: bool,
    pub per_verified_pass: Option<AttemptRatio>,
    pub failed_share: Option<AttemptRatio>,
    pub verified_pass_mean: Option<AttemptRatio>,
    pub complete_cohort_per_verified_pass: Option<AttemptRatio>,
    pub complete_cohort_failed_share: Option<AttemptRatio>,
    pub complete_cohort_verified_pass_mean: Option<AttemptRatio>,
}

impl AttemptEffortSummary {
    fn add(&mut self, value: &ModelTurnObservation, pass: bool) -> CoreResult<()> {
        self.all.add(value)?;
        if pass {
            self.verified_pass.add(value)?;
        } else {
            self.nonpass.add(value)?;
        }
        Ok(())
    }

    fn finish(&mut self, verified_passes: u64) {
        self.full_window_complete = self.all.complete();
        self.complete_cohort_per_verified_pass =
            AttemptRatio::new(self.all.complete_total, self.verified_pass.complete_samples);
        self.complete_cohort_failed_share =
            AttemptRatio::new(self.nonpass.complete_total, self.all.complete_total);
        self.complete_cohort_verified_pass_mean = AttemptRatio::new(
            self.verified_pass.complete_total,
            self.verified_pass.complete_samples,
        );
        if self.full_window_complete {
            self.per_verified_pass = AttemptRatio::new(self.all.complete_total, verified_passes);
            self.failed_share =
                AttemptRatio::new(self.nonpass.complete_total, self.all.complete_total);
        }
        if self.verified_pass.complete() {
            self.verified_pass_mean =
                AttemptRatio::new(self.verified_pass.complete_total, verified_passes);
        }
    }
}

/// Disjoint finished nonpass categories, in this precedence order. Deadline
/// includes only a typed worker deadline or the exact ledger `lease_expired`
/// cause. Other prose is never parsed as a timeout.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptFailureSplit {
    pub deadline: u64,
    pub completed_rejected: u64,
    pub worker_failed: u64,
    pub aborted_cancelled: u64,
    pub other_unknown: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptWorkerTerminalCounts {
    pub completed: u64,
    pub failed: u64,
    pub aborted: u64,
    pub deadline: u64,
    pub unknown: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptVerificationCounts {
    pub verified: u64,
    pub rejected: u64,
    pub unknown: u64,
    pub incomplete: u64,
    pub absent: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptReportSummary {
    /// All claims in the window, including open and unmeasured legacy rows.
    pub attempts: u64,
    /// Rows with an explicit end timestamp; the only outcome/effort samples.
    pub finished: u64,
    /// Active rows with no end timestamp; no elapsed time is extrapolated.
    pub open: u64,
    /// Legacy terminal rows lacking an end timestamp, excluded from metrics.
    pub terminal_without_end: u64,
    pub verified_passes: u64,
    pub nonpasses: u64,
    pub worker_terminals: AttemptWorkerTerminalCounts,
    /// Retained controller metadata, not an alternative source of authority.
    pub verification_outcomes: AttemptVerificationCounts,
    pub failures: AttemptFailureSplit,
    /// Raw Completed+Rejected count, independent of failure-category precedence.
    pub completed_rejected: u64,
    pub rejected_completed_of_nonpasses: Option<AttemptRatio>,
    pub rejected_completed_of_completed: Option<AttemptRatio>,
    pub turns: AttemptEffortSummary,
    pub elapsed_ms: AttemptEffortSummary,
}

impl AttemptReportSummary {
    fn add(&mut self, attempt: &ImplementationAttempt) -> CoreResult<()> {
        add(&mut self.attempts, 1)?;
        let Some(end) = attempt.ended_at_ms else {
            if attempt.phase.is_terminal() {
                add(&mut self.terminal_without_end, 1)?;
            } else {
                add(&mut self.open, 1)?;
            }
            return Ok(());
        };
        let elapsed = end.checked_sub(attempt.claimed_at_ms).ok_or_else(|| {
            Diagnostic::new(
                "implementation_report_time",
                "report.elapsed_ms",
                "attempt end precedes its claim",
            )
        })?;
        add(&mut self.finished, 1)?;
        // An admitted receipt and the canonical verified phase are both needed.
        // Worker completion and even Verified metadata are not verification.
        let pass = attempt.phase == AttemptPhase::Verified && attempt.verification.is_some();
        if pass {
            add(&mut self.verified_passes, 1)?;
        } else {
            add(&mut self.nonpasses, 1)?;
        }
        let terminal = attempt
            .worker_observation
            .as_ref()
            .map(|worker| worker.terminal);
        match terminal {
            Some(WorkerTerminalState::Completed) => add(&mut self.worker_terminals.completed, 1)?,
            Some(WorkerTerminalState::Failed) => add(&mut self.worker_terminals.failed, 1)?,
            Some(WorkerTerminalState::Aborted) => add(&mut self.worker_terminals.aborted, 1)?,
            Some(WorkerTerminalState::Deadline) => add(&mut self.worker_terminals.deadline, 1)?,
            None => add(&mut self.worker_terminals.unknown, 1)?,
        }
        match attempt.verification_outcome {
            Some(VerificationOutcome::Verified) => {
                add(&mut self.verification_outcomes.verified, 1)?
            }
            Some(VerificationOutcome::Rejected) => {
                add(&mut self.verification_outcomes.rejected, 1)?
            }
            Some(VerificationOutcome::Unknown) => add(&mut self.verification_outcomes.unknown, 1)?,
            Some(VerificationOutcome::Incomplete) => {
                add(&mut self.verification_outcomes.incomplete, 1)?;
            }
            None => add(&mut self.verification_outcomes.absent, 1)?,
        }
        let completed_rejected = !pass
            && terminal == Some(WorkerTerminalState::Completed)
            && attempt.verification_outcome == Some(VerificationOutcome::Rejected);
        if completed_rejected {
            add(&mut self.completed_rejected, 1)?;
        }
        if !pass {
            if terminal == Some(WorkerTerminalState::Deadline)
                || attempt.cause.as_deref() == Some("lease_expired")
            {
                add(&mut self.failures.deadline, 1)?;
            } else if completed_rejected {
                add(&mut self.failures.completed_rejected, 1)?;
            } else if terminal == Some(WorkerTerminalState::Failed) {
                add(&mut self.failures.worker_failed, 1)?;
            } else if terminal == Some(WorkerTerminalState::Aborted)
                || attempt.phase == AttemptPhase::Cancelled
            {
                add(&mut self.failures.aborted_cancelled, 1)?;
            } else {
                add(&mut self.failures.other_unknown, 1)?;
            }
        }
        self.turns.add(turns(attempt), pass)?;
        self.elapsed_ms
            .add(&ModelTurnObservation::Observed { turns: elapsed }, pass)?;
        Ok(())
    }

    fn finish(&mut self) {
        self.rejected_completed_of_nonpasses =
            AttemptRatio::new(self.completed_rejected, self.nonpasses);
        self.rejected_completed_of_completed =
            AttemptRatio::new(self.completed_rejected, self.worker_terminals.completed);
        self.turns.finish(self.verified_passes);
        self.elapsed_ms.finish(self.verified_passes);
    }
}

/// Exact attribution tuple. `None` serializes as null, an explicit unknown
/// dimension; no default worker, provider, harness or model is invented.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct AttemptReportGroupKey {
    pub worker_profile: Option<ContentId>,
    pub provider_profile: Option<ProviderProfileId>,
    pub harness: Option<String>,
    pub model: Option<String>,
}

// Borrow dimensions while aggregating; allocate each output label only once
// per distinct group rather than once per historical attempt.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct AttemptReportGroupRef<'a> {
    worker_profile: Option<&'a ContentId>,
    provider_profile: Option<&'a ProviderProfileId>,
    harness: Option<&'a str>,
    model: Option<&'a str>,
}

impl<'a> AttemptReportGroupRef<'a> {
    fn for_attempt(attempt: &'a ImplementationAttempt) -> Self {
        let worker = attempt.worker_observation.as_ref();
        Self {
            worker_profile: worker.and_then(|worker| worker.worker_profile.as_ref()),
            provider_profile: attempt
                .dispatch
                .as_ref()
                .map(|dispatch| &dispatch.provider_profile),
            harness: worker.and_then(|worker| worker.harness.as_deref()),
            model: worker.and_then(|worker| worker.model.as_deref()),
        }
    }

    fn into_owned(self) -> AttemptReportGroupKey {
        AttemptReportGroupKey {
            worker_profile: self.worker_profile.cloned(),
            provider_profile: self.provider_profile.cloned(),
            harness: self.harness.map(String::from),
            model: self.model.map(String::from),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptReportGroup {
    #[serde(flatten)]
    pub key: AttemptReportGroupKey,
    pub summary: AttemptReportSummary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptReport {
    pub schema: String,
    pub from_ms: u64,
    pub to_ms: u64,
    pub totals: AttemptReportSummary,
    /// Lexicographic exact-attribution order, never a performance ranking.
    pub groups: Vec<AttemptReportGroup>,
}

impl AttemptReport {
    /// Observe claims in `[from_ms, to_ms)`. Completion may occur after the
    /// window: this is a claim cohort, not a completion-time or as-of query.
    /// Reads only retained state and never records metrics or changes authority.
    // r[impl kiln.implementation.outcome_metrics]
    // r[impl kiln.implementation.outcome_metrics.scenario.report]
    pub fn observe(ledger: &AttemptLedger, from_ms: u64, to_ms: u64) -> CoreResult<Self> {
        if from_ms >= to_ms {
            return Err(Diagnostic::new(
                "implementation_report_window",
                "report.window",
                "report requires a nonempty half-open claim-time window",
            ));
        }
        let mut totals = AttemptReportSummary::default();
        let mut groups = BTreeMap::<AttemptReportGroupRef<'_>, AttemptReportSummary>::new();
        for attempt in ledger
            .attempts()
            .filter(|attempt| attempt.claimed_at_ms >= from_ms && attempt.claimed_at_ms < to_ms)
        {
            totals.add(attempt)?;
            groups
                .entry(AttemptReportGroupRef::for_attempt(attempt))
                .or_default()
                .add(attempt)?;
        }
        totals.finish();
        let groups = groups
            .into_iter()
            .map(|(key, mut summary)| {
                summary.finish();
                AttemptReportGroup {
                    key: key.into_owned(),
                    summary,
                }
            })
            .collect();
        Ok(Self {
            schema: ATTEMPT_REPORT_SCHEMA.into(),
            from_ms,
            to_ms,
            totals,
            groups,
        })
    }
}

fn turns(attempt: &ImplementationAttempt) -> &ModelTurnObservation {
    if let Some(metrics) = &attempt.metrics {
        if !matches!(
            metrics.usage.model_turns,
            ModelTurnObservation::Absent { .. }
        ) {
            return &metrics.usage.model_turns;
        }
    }
    if let Some(worker) = &attempt.worker_observation {
        return &worker.usage.model_turns;
    }
    // Default absence is a static value, not an allocated synthetic sample.
    static ABSENT: ModelTurnObservation = ModelTurnObservation::Absent {
        reason: super::AbsenceReason::NotReported,
    };
    &ABSENT
}

fn add(total: &mut u64, value: u64) -> CoreResult<()> {
    *total = total.checked_add(value).ok_or_else(|| {
        Diagnostic::new(
            "implementation_report_overflow",
            "report.counts",
            "attempt report arithmetic overflow",
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::string::ToString;
    use alloc::vec;
    use serde_json::{Value, json};

    fn content(index: u64) -> String {
        format!("{index:064x}")
    }

    // Retained-ledger fixtures deliberately bypass admission: this module
    // observes historical state, and must never establish authority from it.
    fn row(
        index: u64,
        claim: u64,
        elapsed: Option<u64>,
        phase: &str,
        terminal: Option<&str>,
        model_turns: Option<u64>,
        verification: Option<&str>,
    ) -> Value {
        let worker = terminal.map(|terminal| {
            json!({
                "terminal": terminal,
                "worker_profile": content(100),
                "harness": "omp",
                "model": "model-a",
                "usage": {
                    "tokens": {"status": "absent", "reason": "not_reported"},
                    "auxiliary_calls": {"status": "absent", "reason": "not_reported"},
                    "model_turns": model_turns.map_or_else(
                        || json!({"status": "absent", "reason": "not_instrumented"}),
                        |turns| json!({"status": "observed", "turns": turns}),
                    )
                }
            })
        });
        json!({
            "schema": super::super::ATTEMPT_SCHEMA,
            "identity": content(index),
            "canonical_key": content(index + 1000),
            "work_identity": content(index + 2000),
            "work_revision": 1,
            "ordinal": 1,
            "fence": index,
            "lease_until_ms": u64::MAX,
            "claimed_at_ms": claim,
            "ended_at_ms": elapsed.map(|elapsed| claim + elapsed),
            "phase": phase,
            "workflow": null,
            "candidate": null,
            "failure": null,
            "cause": null,
            "superseded_by": null,
            "worker_observation": worker,
            "verification_outcome": verification,
        })
    }

    fn receipt(row: &mut Value) {
        row["verification"] = json!({
            "schema": super::super::VERIFICATION_SCHEMA,
            "handoff": content(300),
            "work_identity": row["work_identity"],
            "work_revision": 1,
            "attempt": row["identity"],
            "fence": row["fence"],
            "candidate": {"contract_version": 1, "provider_namespace": "git", "native_kind": "commit", "native_id": "candidate"},
            "tree": {"contract_version": 1, "provider_namespace": "git", "native_kind": "tree", "native_id": "tree"},
            "verification_profile": content(400),
            "run": {
                "project": "kiln", "repository": "onix/kiln", "revision": "candidate",
                "run_id": "ci-pass", "ordinal": 1, "success": true, "full_run": true,
                "clean": true, "terminal": true,
                "checks": [{"name": "tests", "kind": "test", "outcome": "pass",
                            "artifact_digest": content(500)}]
            }
        });
    }

    fn set_field(item: &mut Value, path: &str, value: Value) {
        let (parent, key) = path.rsplit_once('/').unwrap_or_else(|| unreachable!());
        item.pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .unwrap_or_else(|| unreachable!())
            .insert(key.to_string(), value);
    }

    fn ledger(rows: Vec<Value>) -> AttemptLedger {
        let attempts: serde_json::Map<String, Value> = rows
            .into_iter()
            .map(|row| {
                let identity = row
                    .get("identity")
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| unreachable!())
                    .to_string();
                (identity, row)
            })
            .collect();
        serde_json::from_value(json!({
            "fence_counter": 10, "attempts": attempts, "current": {}, "consumed": {}
        }))
        .unwrap_or_else(|error| unreachable!("fixture ledger: {error}"))
    }

    fn observe(rows: Vec<Value>) -> AttemptReport {
        AttemptReport::observe(&ledger(rows), 0, u64::MAX)
            .unwrap_or_else(|error| unreachable!("report: {error:?}"))
    }

    fn ratio(numerator: u64, denominator: u64) -> Option<AttemptRatio> {
        Some(AttemptRatio {
            numerator,
            denominator,
        })
    }

    fn six_outcomes() -> Vec<Value> {
        let mut pass = row(
            1,
            10,
            Some(10),
            "verified",
            Some("completed"),
            Some(2),
            Some("verified"),
        );
        receipt(&mut pass);
        vec![
            pass,
            row(
                2,
                10,
                Some(20),
                "failed",
                Some("completed"),
                Some(3),
                Some("rejected"),
            ),
            row(
                3,
                10,
                Some(30),
                "failed",
                Some("failed"),
                Some(5),
                Some("unknown"),
            ),
            row(
                4,
                10,
                Some(40),
                "cancelled",
                Some("aborted"),
                Some(7),
                Some("incomplete"),
            ),
            row(5, 10, Some(50), "failed", Some("deadline"), Some(11), None),
            row(6, 10, Some(60), "blocked", None, None, None),
        ]
    }

    #[test]
    // r[verify kiln.implementation.outcome_metrics.scenario.report]
    fn six_issue_outcomes_have_exact_denominators_and_effort() {
        let report = observe(six_outcomes());
        let summary = report.totals;
        assert_eq!(
            (summary.attempts, summary.finished, summary.open),
            (6, 6, 0)
        );
        assert_eq!((summary.verified_passes, summary.nonpasses), (1, 5));
        assert_eq!(summary.rejected_completed_of_nonpasses, ratio(1, 5));
        assert_eq!(summary.rejected_completed_of_completed, ratio(1, 2));
        assert_eq!(
            summary.failures,
            AttemptFailureSplit {
                deadline: 1,
                completed_rejected: 1,
                worker_failed: 1,
                aborted_cancelled: 1,
                other_unknown: 1,
            }
        );
        assert_eq!(
            summary.worker_terminals,
            AttemptWorkerTerminalCounts {
                completed: 2,
                failed: 1,
                aborted: 1,
                deadline: 1,
                unknown: 1,
            }
        );
        assert_eq!(
            summary.verification_outcomes,
            AttemptVerificationCounts {
                verified: 1,
                rejected: 1,
                unknown: 1,
                incomplete: 1,
                absent: 2,
            }
        );
        assert_eq!(summary.turns.all.complete_total, 28);
        assert_eq!(summary.turns.all.complete_samples, 5);
        assert_eq!(summary.turns.all.absent_samples, 1);
        assert_eq!(summary.turns.nonpass.complete_samples, 4);
        assert_eq!(summary.turns.nonpass.absent_samples, 1);
        assert_eq!(summary.turns.verified_pass.complete_samples, 1);
        assert!(!summary.turns.full_window_complete);
        assert_eq!(summary.turns.per_verified_pass, None);
        assert_eq!(summary.turns.failed_share, None);
        assert_eq!(summary.turns.verified_pass_mean, ratio(2, 1));
        assert_eq!(
            summary.turns.complete_cohort_per_verified_pass,
            ratio(28, 1)
        );
        assert_eq!(summary.turns.complete_cohort_failed_share, ratio(26, 28));
        assert_eq!(summary.elapsed_ms.all.complete_samples, 6);
        assert_eq!(summary.elapsed_ms.per_verified_pass, ratio(210, 1));
        assert_eq!(summary.elapsed_ms.verified_pass_mean, ratio(10, 1));
        assert_eq!(summary.elapsed_ms.failed_share, ratio(200, 210));
        assert!(summary.elapsed_ms.full_window_complete);
        assert_eq!(report.groups.len(), 2);
    }

    #[test]
    fn complete_cohort_and_full_window_have_distinct_mean_and_total_effort() {
        let mut rows = six_outcomes();
        set_field(
            rows.get_mut(5).unwrap_or_else(|| unreachable!()),
            "/worker_observation",
            row(7, 10, Some(60), "failed", Some("failed"), Some(13), None)
                .get("worker_observation")
                .unwrap_or_else(|| unreachable!())
                .clone(),
        );
        let summary = observe(rows).totals;
        assert!(summary.turns.full_window_complete);
        assert_eq!(summary.turns.per_verified_pass, ratio(41, 1));
        assert_eq!(summary.turns.verified_pass_mean, ratio(2, 1));
        assert_eq!(summary.turns.failed_share, ratio(39, 41));
        assert_eq!(
            summary.turns.per_verified_pass,
            summary.turns.complete_cohort_per_verified_pass
        );
    }

    #[test]
    fn claim_window_is_half_open_and_not_a_completion_cutoff() {
        let rows = vec![
            row(1, 9, Some(1), "failed", Some("failed"), Some(1), None),
            row(2, 10, Some(1000), "failed", Some("failed"), Some(2), None),
            row(
                3,
                19,
                None,
                "executing",
                Some("completed"),
                Some(100),
                Some("rejected"),
            ),
            row(4, 20, Some(1), "failed", Some("failed"), Some(4), None),
            row(
                5,
                15,
                None,
                "verified",
                Some("completed"),
                Some(100),
                Some("verified"),
            ),
        ];
        let report =
            AttemptReport::observe(&ledger(rows), 10, 20).unwrap_or_else(|_| unreachable!());
        assert_eq!(report.totals.attempts, 3);
        assert_eq!(report.totals.finished, 1);
        assert_eq!(report.totals.open, 1);
        assert_eq!(report.totals.terminal_without_end, 1);
        assert_eq!(report.totals.turns.all.complete_total, 2);
        assert_eq!(report.totals.elapsed_ms.all.complete_total, 1000);
        assert_eq!(report.totals.completed_rejected, 0);
        assert_eq!(report.totals.worker_terminals.completed, 0);
    }

    #[test]
    fn empty_and_zero_denominators_never_fabricate_ratios() {
        let report = observe(Vec::new());
        assert!(report.groups.is_empty());
        assert_eq!(report.totals.rejected_completed_of_nonpasses, None);
        assert_eq!(report.totals.rejected_completed_of_completed, None);
        assert_eq!(report.totals.turns.per_verified_pass, None);
        assert_eq!(report.totals.turns.failed_share, None);
        assert_eq!(report.totals.turns.verified_pass_mean, None);
        let report = observe(vec![row(
            1,
            0,
            Some(0),
            "failed",
            Some("failed"),
            Some(0),
            None,
        )]);
        assert_eq!(report.totals.turns.per_verified_pass, None);
        assert_eq!(report.totals.turns.failed_share, None);
        assert_eq!(report.totals.turns.complete_cohort_per_verified_pass, None);
        assert_eq!(report.totals.elapsed_ms.failed_share, None);
        assert_eq!(report.totals.rejected_completed_of_nonpasses, ratio(0, 1));
        let mut pass = row(2, 0, Some(0), "verified", Some("completed"), Some(0), None);
        receipt(&mut pass);
        let report = observe(vec![pass]);
        assert_eq!(report.totals.turns.per_verified_pass, ratio(0, 1));
        assert_eq!(report.totals.turns.verified_pass_mean, ratio(0, 1));
        assert_eq!(report.totals.turns.failed_share, None);
    }

    #[test]
    fn malformed_window_and_time_are_refused() {
        assert!(AttemptReport::observe(&AttemptLedger::default(), 5, 5).is_err());
        assert!(AttemptReport::observe(&AttemptLedger::default(), 6, 5).is_err());
        let mut bad = row(1, 5, Some(1), "failed", Some("failed"), Some(1), None);
        set_field(&mut bad, "/ended_at_ms", json!(4));
        let error = AttemptReport::observe(&ledger(vec![bad]), 0, 10)
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(error.code, "implementation_report_time");
    }

    #[test]
    fn grouping_keeps_exact_profiles_providers_harnesses_models_and_unknowns() {
        let mut rows = Vec::new();
        for index in 1..=6 {
            let mut item = row(
                index,
                10,
                Some(index),
                "failed",
                Some("failed"),
                Some(index),
                None,
            );
            set_field(
                &mut item,
                "/dispatch",
                json!({
                    "schema": super::super::DISPATCH_SCHEMA,
                    "provider_profile": "provider-a", "plan": content(700),
                    "effect_id": "dispatch-1",
                    "idempotency_key": {"contract_version": 1, "provider_namespace": "kiln", "native_kind": "dispatch", "native_id": "1"},
                    "reserved_at_ms": 10
                }),
            );
            match index {
                2 => set_field(&mut item, "/worker_observation/model", json!("model-b")),
                3 => set_field(
                    &mut item,
                    "/worker_observation/worker_profile",
                    json!(content(101)),
                ),
                4 => set_field(&mut item, "/dispatch/provider_profile", json!("provider-b")),
                5 => set_field(
                    &mut item,
                    "/worker_observation/harness",
                    json!("other-harness"),
                ),
                6 => {
                    set_field(&mut item, "/worker_observation", Value::Null);
                    set_field(&mut item, "/dispatch", Value::Null);
                }
                _ => {}
            }
            rows.push(item);
        }
        let forward = observe(rows.clone());
        rows.reverse();
        let reversed = observe(rows);
        assert_eq!(forward, reversed);
        assert_eq!(forward.groups.len(), 6);
        assert_eq!(
            forward.groups.first().unwrap_or_else(|| unreachable!()).key,
            AttemptReportGroupKey {
                worker_profile: None,
                provider_profile: None,
                harness: None,
                model: None,
            }
        );
        assert!(
            forward
                .groups
                .iter()
                .all(|group| group.summary.finished == 1)
        );
        assert_eq!(
            forward
                .groups
                .iter()
                .map(|group| group.summary.elapsed_ms.all.complete_total)
                .sum::<u64>(),
            21
        );
        let json = serde_json::to_value(&forward).unwrap_or_else(|_| unreachable!());
        assert!(
            json.pointer("/groups/0/worker_profile")
                .is_some_and(Value::is_null)
        );
        assert!(json.pointer("/groups/0/model").is_some_and(Value::is_null));
        let replay: AttemptReport = serde_json::from_value(json).unwrap_or_else(|_| unreachable!());
        assert_eq!(replay, forward);
    }

    fn metric_turns(item: &mut Value, observation: ModelTurnObservation) {
        let attempt: ImplementationAttempt =
            serde_json::from_value(item.clone()).unwrap_or_else(|_| unreachable!());
        let usage = super::super::AttemptUsage {
            model_turns: observation,
            ..super::super::AttemptUsage::default()
        };
        let metrics = super::super::AttemptMetrics::observe(
            &attempt,
            attempt.ended_at_ms.unwrap_or(attempt.claimed_at_ms),
            usage,
            Vec::new(),
        )
        .unwrap_or_else(|_| unreachable!());
        item["metrics"] = serde_json::to_value(metrics).unwrap_or_else(|_| unreachable!());
    }

    #[test]
    fn explicit_metrics_win_and_incomplete_turns_are_lower_bounds_only() {
        let mut rows = six_outcomes();
        metric_turns(
            rows.first_mut().unwrap_or_else(|| unreachable!()),
            ModelTurnObservation::Incomplete {
                turns: 9,
                reason: super::super::AbsenceReason::ProviderUnavailable,
            },
        );
        metric_turns(
            rows.get_mut(1).unwrap_or_else(|| unreachable!()),
            ModelTurnObservation::Observed { turns: 17 },
        );
        metric_turns(
            rows.get_mut(2).unwrap_or_else(|| unreachable!()),
            ModelTurnObservation::Absent {
                reason: super::super::AbsenceReason::NotReported,
            },
        );
        // Preserve an old absent receipt even if a newer worker observation
        // became available; the report must fall back without rewriting it.
        set_field(
            rows.get_mut(2).unwrap_or_else(|| unreachable!()),
            "/metrics/usage/model_turns",
            json!({
                "status": "absent", "reason": "not_reported"
            }),
        );
        set_field(
            rows.get_mut(3).unwrap_or_else(|| unreachable!()),
            "/worker_observation/usage/model_turns",
            json!({
                "status": "incomplete", "turns": 8, "reason": "provider_unavailable"
            }),
        );
        let summary = observe(rows).totals;
        assert_eq!(summary.turns.all.complete_total, 33); // 17 + fallback 5 + 11
        assert_eq!(summary.turns.all.complete_samples, 3);
        assert_eq!(summary.turns.all.incomplete_samples, 2);
        assert_eq!(summary.turns.all.incomplete_lower_bound, 17);
        assert_eq!(summary.turns.all.absent_samples, 1);
        assert_eq!(summary.turns.verified_pass.incomplete_samples, 1);
        assert_eq!(summary.turns.verified_pass.incomplete_lower_bound, 9);
        assert_eq!(summary.turns.nonpass.incomplete_samples, 1);
        assert_eq!(summary.turns.nonpass.absent_samples, 1);
        assert_eq!(summary.turns.verified_pass_mean, None);
        assert_eq!(summary.turns.per_verified_pass, None);
        assert_eq!(summary.turns.complete_cohort_per_verified_pass, None);
        assert_eq!(summary.turns.complete_cohort_failed_share, ratio(33, 33));
    }

    #[test]
    fn legacy_missing_turn_field_is_absence_even_for_verified_passes() {
        let mut rows = six_outcomes();
        rows.first_mut()
            .and_then(|row| row.pointer_mut("/worker_observation/usage"))
            .and_then(Value::as_object_mut)
            .unwrap_or_else(|| unreachable!())
            .remove("model_turns");
        let report = observe(rows);
        assert_eq!(report.totals.verified_passes, 1);
        assert_eq!(report.totals.turns.verified_pass.absent_samples, 1);
        assert_eq!(report.totals.turns.verified_pass.complete_samples, 0);
        assert_eq!(report.totals.turns.verified_pass.complete_total, 0);
        assert_eq!(report.totals.turns.per_verified_pass, None);
        assert_eq!(report.totals.turns.verified_pass_mean, None);
        assert_eq!(report.totals.turns.complete_cohort_per_verified_pass, None);
        assert_eq!(report.totals.elapsed_ms.per_verified_pass, ratio(210, 1));
    }

    #[test]
    fn verification_metadata_or_worker_completion_never_create_pass_authority() {
        let mut rows = six_outcomes();
        set_field(
            rows.first_mut().unwrap_or_else(|| unreachable!()),
            "/verification",
            Value::Null,
        );
        set_field(
            rows.get_mut(1).unwrap_or_else(|| unreachable!()),
            "/verification_outcome",
            json!("verified"),
        );
        receipt(rows.get_mut(2).unwrap_or_else(|| unreachable!())); // Receipt without Verified phase is not a pass.
        let ledger = ledger(rows);
        let before = ledger.clone();
        let report = AttemptReport::observe(&ledger, 0, 100).unwrap_or_else(|_| unreachable!());
        assert_eq!(report.totals.verified_passes, 0);
        assert_eq!(report.totals.nonpasses, 6);
        assert_eq!(report.totals.turns.per_verified_pass, None);
        assert_eq!(ledger, before);
        let encoded = serde_json::to_vec(&ledger).unwrap_or_else(|_| unreachable!());
        let replay: AttemptLedger =
            serde_json::from_slice(&encoded).unwrap_or_else(|_| unreachable!());
        assert_eq!(AttemptReport::observe(&replay, 0, 100).ok(), Some(report));
    }

    #[test]
    fn deadline_precedence_is_disjoint_and_only_exact_causes_count() {
        let mut deadline = row(
            1,
            0,
            Some(1),
            "blocked",
            Some("completed"),
            None,
            Some("rejected"),
        );
        set_field(&mut deadline, "/cause", json!("lease_expired"));
        let mut prose = row(2, 0, Some(1), "blocked", None, None, None);
        set_field(
            &mut prose,
            "/cause",
            json!("worker said timeout / lease_expired"),
        );
        let cancelled = row(3, 0, Some(1), "cancelled", None, None, None);
        let summary = observe(vec![deadline, prose, cancelled]).totals;
        assert_eq!(summary.completed_rejected, 1);
        assert_eq!(summary.rejected_completed_of_nonpasses, ratio(1, 3));
        assert_eq!(summary.rejected_completed_of_completed, ratio(1, 1));
        assert_eq!(
            summary.failures,
            AttemptFailureSplit {
                deadline: 1,
                completed_rejected: 0,
                worker_failed: 0,
                aborted_cancelled: 1,
                other_unknown: 1,
            }
        );
    }

    #[test]
    fn checked_arithmetic_refuses_turn_elapsed_and_lower_bound_overflow() {
        let huge = row(
            1,
            0,
            Some(1),
            "failed",
            Some("failed"),
            Some(u64::MAX),
            None,
        );
        let one = row(2, 0, Some(1), "failed", Some("failed"), Some(1), None);
        let error = AttemptReport::observe(&ledger(vec![huge, one]), 0, 1)
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(error.code, "implementation_report_overflow");
        let huge = row(1, 0, Some(u64::MAX), "failed", None, None, None);
        let one = row(2, 0, Some(1), "failed", None, None, None);
        assert!(AttemptReport::observe(&ledger(vec![huge, one]), 0, 1).is_err());
        let mut huge = row(1, 0, Some(1), "failed", Some("failed"), None, None);
        set_field(
            &mut huge,
            "/worker_observation/usage/model_turns",
            json!({
                "status": "incomplete", "turns": u64::MAX, "reason": "provider_unavailable"
            }),
        );
        let mut one = row(2, 0, Some(1), "failed", Some("failed"), None, None);
        set_field(
            &mut one,
            "/worker_observation/usage/model_turns",
            json!({
                "status": "incomplete", "turns": 1, "reason": "provider_unavailable"
            }),
        );
        assert!(AttemptReport::observe(&ledger(vec![huge, one]), 0, 1).is_err());
        let mut count = u64::MAX;
        assert!(add(&mut count, 1).is_err());
    }
}
