//! Opt-in metrics receipt for one implementation attempt.
//!
//! Metrics are computed or recorded only when an operator asks. They never gate
//! admission, verification or publication. Missing cost data stays explicitly
//! absent instead of becoming zero.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::AttemptPhase;
use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef};

pub const METRICS_SCHEMA: &str = "kiln.implementation-metrics.v1";
pub const MAX_METRIC_TRACES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsenceReason {
    NotReported,
    NotInstrumented,
    ProviderUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TokenObservation {
    Absent { reason: AbsenceReason },
    Observed { tokens: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuxiliaryCallObservation {
    Absent { reason: AbsenceReason },
    Observed { calls: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptUsage {
    pub tokens: TokenObservation,
    pub auxiliary_calls: AuxiliaryCallObservation,
}

impl Default for AttemptUsage {
    fn default() -> Self {
        Self {
            tokens: TokenObservation::Absent {
                reason: AbsenceReason::NotReported,
            },
            auxiliary_calls: AuxiliaryCallObservation::Absent {
                reason: AbsenceReason::NotReported,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptMetrics {
    pub schema: String,
    pub attempt_identity: ContentId,
    pub fence: u64,
    pub work_identity: ContentId,
    pub work_revision: u64,
    pub phase: AttemptPhase,
    pub claimed_at_ms: u64,
    pub observed_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at_ms: Option<u64>,
    pub elapsed_ms: u64,
    pub open: bool,
    pub usage: AttemptUsage,
    pub traces: Vec<ExternalRef>,
}

impl AttemptMetrics {
    /// Observe one attempt at `now_ms`. No ledger mutation and no admission.
    ///
    /// A terminal attempt uses its recorded end; an active attempt reports
    /// `open` and elapsed time through the observation instant. Missing usage
    /// stays absent.
    // r[impl kiln.implementation.metrics]
    pub fn observe(
        attempt: &super::ImplementationAttempt,
        now_ms: u64,
        usage: AttemptUsage,
        traces: Vec<ExternalRef>,
    ) -> CoreResult<Self> {
        if traces.len() > MAX_METRIC_TRACES {
            return Err(refusal(
                "implementation_metrics_traces",
                "metrics.traces",
                "metrics receipt has too many trace references",
            ));
        }
        let mut seen = alloc::collections::BTreeSet::new();
        for trace in &traces {
            trace.validate()?;
            if !seen.insert(trace) {
                return Err(refusal(
                    "implementation_metrics_traces",
                    "metrics.traces",
                    "metrics receipt repeats a trace reference",
                ));
            }
        }
        let ended_at_ms = attempt.ended_at_ms;
        let end = ended_at_ms.unwrap_or(now_ms);
        if end < attempt.claimed_at_ms {
            return Err(refusal(
                "implementation_metrics_time",
                "metrics.elapsed_ms",
                "metrics observation precedes the attempt claim",
            ));
        }
        Ok(Self {
            schema: METRICS_SCHEMA.to_string(),
            attempt_identity: attempt.identity.clone(),
            fence: attempt.fence,
            work_identity: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            phase: attempt.phase,
            claimed_at_ms: attempt.claimed_at_ms,
            observed_at_ms: now_ms,
            ended_at_ms,
            elapsed_ms: end.saturating_sub(attempt.claimed_at_ms),
            open: ended_at_ms.is_none() && !attempt.phase.is_terminal(),
            usage,
            traces,
        })
    }

    /// Check a receipt against the attempt it claims to describe.
    // r[impl kiln.implementation.metrics]
    pub fn validate_for(&self, attempt: &super::ImplementationAttempt) -> CoreResult<()> {
        if self.schema != METRICS_SCHEMA
            || self.attempt_identity != attempt.identity
            || self.fence != attempt.fence
            || self.work_identity != attempt.work_identity
            || self.work_revision != attempt.work_revision
            || self.phase != attempt.phase
            || self.claimed_at_ms != attempt.claimed_at_ms
            || self.ended_at_ms != attempt.ended_at_ms
            || self.open != (attempt.ended_at_ms.is_none() && !attempt.phase.is_terminal())
            || self.observed_at_ms < self.claimed_at_ms
            || self.elapsed_ms
                != self
                    .ended_at_ms
                    .unwrap_or(self.observed_at_ms)
                    .saturating_sub(self.claimed_at_ms)
            || self.traces.len() > MAX_METRIC_TRACES
        {
            return Err(refusal(
                "implementation_metrics_binding",
                "metrics.receipt",
                "metrics receipt does not bind the exact attempt",
            ));
        }
        Ok(())
    }
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementation::{
        AcceptanceObligation, AttemptLedger, DEFAULT_LEASE_MS, IMPLEMENTATION_WORK_SCHEMA,
        ImplementationWork, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
    };
    use alloc::collections::BTreeSet;
    use alloc::vec;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn work() -> ImplementationWork {
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            base: None,
            title: "Metrics work".to_string(),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: content('9'),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "onix/kiln#56")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "onix/kiln#56".to_string(),
                version: content('1'),
            }],
        }
    }

    #[test]
    // r[verify kiln.implementation.metrics.scenario.open] r[verify kiln.implementation.metrics.scenario.default_off]
    fn observes_an_open_attempt_without_cost_data() {
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work(), 1_000, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let metrics = ledger
            .observe_metrics(
                &attempt.identity,
                attempt.fence,
                1_250,
                AttemptUsage::default(),
                Vec::new(),
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(metrics.open);
        assert_eq!(metrics.elapsed_ms, 250);
        assert_eq!(
            metrics.usage.tokens,
            TokenObservation::Absent {
                reason: AbsenceReason::NotReported
            }
        );
        assert_eq!(
            metrics.usage.auxiliary_calls,
            AuxiliaryCallObservation::Absent {
                reason: AbsenceReason::NotReported
            }
        );
        assert!(
            ledger.metrics(&attempt.identity).is_none(),
            "observation alone records nothing"
        );
    }

    #[test]
    // r[verify kiln.implementation.metrics.scenario.terminal] r[verify kiln.implementation.metrics.scenario.retained]
    fn records_a_terminal_receipt_once_with_observed_usage() {
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work(), 1_000, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        ledger
            .cancel(
                &attempt.identity,
                attempt.fence,
                1_400,
                "operator cancelled",
            )
            .unwrap_or_else(|_| unreachable!());
        let usage = AttemptUsage {
            tokens: TokenObservation::Observed { tokens: 1_234 },
            auxiliary_calls: AuxiliaryCallObservation::Observed { calls: 3 },
        };
        let trace =
            ExternalRef::new("lattice", "trace", "run-one").unwrap_or_else(|_| unreachable!());
        let first = ledger
            .record_metrics(
                &attempt.identity,
                attempt.fence,
                1_500,
                usage.clone(),
                vec![trace.clone()],
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(!first.open);
        assert_eq!(first.elapsed_ms, 400);
        assert_eq!(first.usage, usage);
        assert_eq!(first.traces, vec![trace]);
        first
            .validate_for(
                ledger
                    .get(&attempt.identity)
                    .unwrap_or_else(|| unreachable!()),
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let second = ledger
            .record_metrics(
                &attempt.identity,
                attempt.fence,
                1_700,
                AttemptUsage::default(),
                Vec::new(),
            )
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(second, first);
    }

    #[test]
    // r[verify kiln.implementation.metrics.scenario.binding]
    fn refuses_stale_fences_and_malformed_trace_sets() {
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work(), 1_000, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        assert!(
            ledger
                .observe_metrics(
                    &attempt.identity,
                    attempt.fence + 1,
                    1_100,
                    AttemptUsage::default(),
                    Vec::new(),
                )
                .is_err()
        );
        let trace =
            ExternalRef::new("lattice", "trace", "run-one").unwrap_or_else(|_| unreachable!());
        assert!(
            ledger
                .observe_metrics(
                    &attempt.identity,
                    attempt.fence,
                    1_100,
                    AttemptUsage::default(),
                    vec![trace.clone(), trace],
                )
                .is_err()
        );
        assert!(
            ledger
                .observe_metrics(
                    &attempt.identity,
                    attempt.fence,
                    999,
                    AttemptUsage::default(),
                    Vec::new(),
                )
                .is_err()
        );
    }
}
