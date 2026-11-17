//! Bounded continuation of selected campaigns from owner observations.
//!
//! A continuation step rereads canonical campaign state and decides whether
//! to run the existing bounded driver now, wait, or stop. Wake hints only ask
//! for that reread: they never grant an effect, a resource or completion.
//! Everything here is pure; a shell persists the [`ContinuationRecord`] beside
//! the campaign and supplies the clock.

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};

use serde::{Deserialize, Serialize};

use super::campaign::{CampaignState, MAX_ID_BYTES};
use crate::types::{CoreResult, Diagnostic};

pub const CONTINUATION_SCHEMA: &str = "kiln.implementation-campaign-continuation.v1";

/// A lost notification resumes work within this interval.
pub const DEFAULT_RESCAN_INTERVAL_MS: u64 = 300_000;
/// Delay after the first consecutive driver failure; it doubles per failure.
pub const DEFAULT_RETRY_BASE_MS: u64 = 30_000;
/// Longest delay between failed driver batches.
pub const DEFAULT_RETRY_MAX_MS: u64 = 3_600_000;
/// Consecutive failed batches before the continuation stops for an operator.
pub const DEFAULT_MAX_CONSECUTIVE_FAILURES: u32 = 8;
/// Driver batches one continuation step may run.
pub const DEFAULT_MAX_BATCHES: u32 = 4;
/// Shortest admitted rescan interval or retry delay.
pub const MIN_INTERVAL_MS: u64 = 1_000;
/// Longest admitted rescan interval or retry delay: one day.
pub const MAX_INTERVAL_MS: u64 = 86_400_000;
/// Largest admitted failure budget.
pub const MAX_CONSECUTIVE_FAILURES: u32 = 64;
/// Largest admitted batch bound per continuation step.
pub const MAX_BATCHES: u32 = 32;
/// Largest exponent of the doubling backoff; larger counts stay at the cap.
const MAX_BACKOFF_EXPONENT: u32 = 63;

/// The owner whose observation may have changed. A hint is only a reason to
/// reread canonical state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeKind {
    Review,
    Verification,
    Source,
    Capacity,
    Reconciliation,
    Integration,
    /// An operator explicitly asked to continue; this also clears the failure
    /// budget and any backoff.
    Operator,
}

impl WakeKind {
    /// Parse the snake_case name used on command lines.
    pub fn parse(value: &str) -> CoreResult<Self> {
        Ok(match value {
            "review" => Self::Review,
            "verification" => Self::Verification,
            "source" => Self::Source,
            "capacity" => Self::Capacity,
            "reconciliation" => Self::Reconciliation,
            "integration" => Self::Integration,
            "operator" => Self::Operator,
            _ => {
                return Err(Diagnostic::new(
                    "unknown_wake_kind",
                    "kind",
                    "wake kind must be review, verification, source, capacity, reconciliation, integration or operator",
                ));
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WakeHint {
    pub kind: WakeKind,
    /// Campaign store version the notifier observed, when it knows one.
    pub observed_version: Option<u64>,
}

/// How a continuation paces itself. Every field is bounded by [`Self::validate`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationPolicy {
    pub rescan_interval_ms: u64,
    pub retry_base_ms: u64,
    pub retry_max_ms: u64,
    pub max_consecutive_failures: u32,
    pub max_batches: u32,
}

impl ContinuationPolicy {
    pub const DEFAULT: Self = Self {
        rescan_interval_ms: DEFAULT_RESCAN_INTERVAL_MS,
        retry_base_ms: DEFAULT_RETRY_BASE_MS,
        retry_max_ms: DEFAULT_RETRY_MAX_MS,
        max_consecutive_failures: DEFAULT_MAX_CONSECUTIVE_FAILURES,
        max_batches: DEFAULT_MAX_BATCHES,
    };

    pub fn validate(&self) -> CoreResult<()> {
        let interval = MIN_INTERVAL_MS..=MAX_INTERVAL_MS;
        for (field, value) in [
            ("continuation.rescan_interval_ms", self.rescan_interval_ms),
            ("continuation.retry_base_ms", self.retry_base_ms),
            ("continuation.retry_max_ms", self.retry_max_ms),
        ] {
            if !interval.contains(&value) {
                return Err(Diagnostic::new(
                    "continuation_policy_bound",
                    field,
                    "interval must lie between one second and one day",
                ));
            }
        }
        if self.retry_base_ms > self.retry_max_ms {
            return Err(Diagnostic::new(
                "continuation_policy_bound",
                "continuation.retry_base_ms",
                "the first retry delay cannot exceed the longest retry delay",
            ));
        }
        if !(1..=MAX_CONSECUTIVE_FAILURES).contains(&self.max_consecutive_failures) {
            return Err(Diagnostic::new(
                "continuation_policy_bound",
                "continuation.max_consecutive_failures",
                "failure budget must be positive and within the named maximum",
            ));
        }
        if !(1..=MAX_BATCHES).contains(&self.max_batches) {
            return Err(Diagnostic::new(
                "continuation_policy_bound",
                "continuation.max_batches",
                "batch bound must be positive and within the named maximum",
            ));
        }
        Ok(())
    }

    /// Delay after `failures` consecutive failures: the base, doubled per
    /// further failure, capped at the longest delay.
    #[must_use]
    pub fn backoff_ms(&self, failures: u32) -> u64 {
        let exponent = failures.saturating_sub(1).min(MAX_BACKOFF_EXPONENT);
        let factor = 1u64.checked_shl(exponent).unwrap_or(u64::MAX);
        self.retry_base_ms
            .saturating_mul(factor)
            .min(self.retry_max_ms)
    }
}

impl Default for ContinuationPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// What one bounded driver batch ended with.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchOutcome {
    /// Every operation was used and the next decision is another owner effect.
    OperationLimit,
    /// No member can act until an observation changes.
    Waiting,
    /// An owner effect remains unresolved; it reconciles before new work.
    AwaitingOwner,
    /// Canonical admission refused the claim: wait for capacity.
    AdmissionRefused,
    Paused,
    /// The selection needs an explicit replan or revision.
    NeedsReplan,
    Cancelled,
    Complete,
    /// Another writer moved the campaign; reread before deciding again.
    StaleVersion,
    /// The driver failed; counts against the failure budget.
    Failed,
}

/// What a continuation decision reads from one canonical campaign snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CampaignView {
    pub store_version: u64,
    pub complete: bool,
    pub cancelled: bool,
    pub paused: bool,
    /// An owner effect intent or observed result is not yet applied.
    pub unapplied_effect: bool,
}

impl CampaignView {
    #[must_use]
    pub fn of(state: &CampaignState, store_version: u64) -> Self {
        Self {
            store_version,
            complete: state.complete(),
            cancelled: state.cancelled,
            paused: state.paused,
            unapplied_effect: state
                .pending_effects
                .values()
                .any(|effect| !effect.progress_applied),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveReason {
    /// The previous batch stopped at its operation bound with work remaining.
    OperationLimit,
    /// A wake hint arrived since the last batch.
    WakeDemand,
    /// The previous batch failed and its backoff elapsed.
    Retry,
    /// The campaign changed since the last batch.
    VersionChanged,
    /// No batch has run yet.
    FirstStep,
    /// The bounded rescan interval elapsed without other news.
    Rescan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitReason {
    Backoff,
    /// Admission is paused until an operator continues it.
    Paused,
    /// Nothing changed; wait for a hint, a version change or the rescan.
    AwaitingObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Complete,
    Cancelled,
    /// Consecutive failures reached the budget; an operator hint clears it.
    FailureBudgetExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum ContinuationStep {
    Drive {
        reason: DriveReason,
    },
    /// `until_ms` is the next time a decision can change without new
    /// observations; `None` waits for an operator.
    Wait {
        reason: WaitReason,
        until_ms: Option<u64>,
    },
    Stop {
        reason: StopReason,
    },
}

/// Durable continuation state of one campaign. It holds wake demand and
/// pacing only: selection, claims, effects and progress stay with their owners.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationRecord {
    pub schema: String,
    pub campaign_id: String,
    /// Kinds of hints not yet consumed by a completed batch.
    pub demand: BTreeSet<WakeKind>,
    /// Incremented by every recorded hint.
    pub demand_sequence: u64,
    /// Hints up to this sequence were consumed by a completed batch.
    pub consumed_sequence: u64,
    pub highest_hinted_version: Option<u64>,
    pub last_outcome: Option<BatchOutcome>,
    pub last_batch_ms: Option<u64>,
    /// Campaign store version after the last batch; `None` forces a reread.
    pub last_store_version: Option<u64>,
    pub consecutive_failures: u32,
    /// No batch runs before this time.
    pub next_eligible_ms: u64,
    pub batches: u64,
}

impl ContinuationRecord {
    #[must_use]
    pub fn new(campaign_id: &str) -> Self {
        Self {
            schema: CONTINUATION_SCHEMA.to_string(),
            campaign_id: campaign_id.to_string(),
            demand: BTreeSet::new(),
            demand_sequence: 0,
            consumed_sequence: 0,
            highest_hinted_version: None,
            last_outcome: None,
            last_batch_ms: None,
            last_store_version: None,
            consecutive_failures: 0,
            next_eligible_ms: 0,
            batches: 0,
        }
    }

    /// Revalidate a record that may have arrived through serde.
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema != CONTINUATION_SCHEMA
            || self.campaign_id.trim().is_empty()
            || self.campaign_id.len() > MAX_ID_BYTES
            || self.campaign_id.chars().any(char::is_control)
        {
            return Err(Diagnostic::new(
                "continuation_record_invalid",
                "continuation",
                "continuation record schema or campaign identity is invalid",
            ));
        }
        if self.consumed_sequence > self.demand_sequence
            || self.pending_demand() == self.demand.is_empty()
        {
            return Err(Diagnostic::new(
                "continuation_record_invalid",
                "continuation.demand",
                "pending demand must be exactly the hints after the consumed sequence",
            ));
        }
        Ok(())
    }

    /// Whether a hint arrived that no completed batch has consumed.
    #[must_use]
    pub fn pending_demand(&self) -> bool {
        self.consumed_sequence < self.demand_sequence
    }

    // r[impl kiln.implementation.campaign.continuation.scenario.coalesce]
    /// Coalesce one hint: duplicate and out-of-order hints only extend the
    /// pending demand. An operator hint also clears the failure budget.
    pub fn record_hint(&mut self, hint: WakeHint) {
        self.demand.insert(hint.kind);
        self.demand_sequence = self.demand_sequence.saturating_add(1);
        if let Some(version) = hint.observed_version {
            self.highest_hinted_version = Some(
                self.highest_hinted_version
                    .map_or(version, |highest| highest.max(version)),
            );
        }
        if hint.kind == WakeKind::Operator {
            self.consecutive_failures = 0;
            self.next_eligible_ms = 0;
        }
    }

    // r[impl kiln.implementation.campaign.continuation]
    // r[impl kiln.implementation.campaign.continuation.scenario.idle]
    // r[impl kiln.implementation.campaign.continuation.scenario.pause]
    /// Decide the next continuation step for one canonical snapshot.
    #[must_use]
    pub fn decide(
        &self,
        view: CampaignView,
        now_ms: u64,
        policy: &ContinuationPolicy,
    ) -> ContinuationStep {
        use ContinuationStep::{Drive, Stop, Wait};
        if view.complete {
            return Stop {
                reason: StopReason::Complete,
            };
        }
        if view.cancelled && !view.unapplied_effect {
            return Stop {
                reason: StopReason::Cancelled,
            };
        }
        if self.consecutive_failures >= policy.max_consecutive_failures {
            return Stop {
                reason: StopReason::FailureBudgetExhausted,
            };
        }
        if now_ms < self.next_eligible_ms {
            return Wait {
                reason: WaitReason::Backoff,
                until_ms: Some(self.next_eligible_ms),
            };
        }
        // Admission is closed while paused or cancelled, but a retained effect
        // still reconciles on news or the rescan; the driver selects it first.
        if view.paused && !view.unapplied_effect {
            return Wait {
                reason: WaitReason::Paused,
                until_ms: None,
            };
        }
        if self.last_outcome == Some(BatchOutcome::OperationLimit) {
            return Drive {
                reason: DriveReason::OperationLimit,
            };
        }
        if self.pending_demand() {
            return Drive {
                reason: DriveReason::WakeDemand,
            };
        }
        if self.last_outcome == Some(BatchOutcome::Failed) {
            return Drive {
                reason: DriveReason::Retry,
            };
        }
        let Some(last_batch_ms) = self.last_batch_ms else {
            return Drive {
                reason: DriveReason::FirstStep,
            };
        };
        if self.last_store_version != Some(view.store_version) {
            return Drive {
                reason: DriveReason::VersionChanged,
            };
        }
        let rescan = last_batch_ms.saturating_add(policy.rescan_interval_ms);
        if now_ms >= rescan {
            return Drive {
                reason: DriveReason::Rescan,
            };
        }
        Wait {
            reason: WaitReason::AwaitingObservation,
            until_ms: Some(rescan),
        }
    }

    // r[impl kiln.implementation.campaign.continuation.scenario.backoff]
    /// Record one batch. `consumed_sequence` is the demand sequence read before
    /// the batch, so a hint recorded during it stays pending. Only a failure
    /// counts against the budget; a stale version forces a reread and keeps
    /// the demand.
    pub fn apply_outcome(
        &mut self,
        outcome: BatchOutcome,
        store_version: u64,
        consumed_sequence: u64,
        now_ms: u64,
        policy: &ContinuationPolicy,
    ) {
        self.batches = self.batches.saturating_add(1);
        self.last_batch_ms = Some(now_ms);
        self.last_outcome = Some(outcome);
        match outcome {
            BatchOutcome::StaleVersion => self.last_store_version = None,
            BatchOutcome::Failed => {
                self.last_store_version = Some(store_version);
                self.consecutive_failures = self.consecutive_failures.saturating_add(1);
                self.next_eligible_ms =
                    now_ms.saturating_add(policy.backoff_ms(self.consecutive_failures));
            }
            _ => {
                self.last_store_version = Some(store_version);
                self.consecutive_failures = 0;
                self.next_eligible_ms = 0;
                let through = consumed_sequence.min(self.demand_sequence);
                self.consumed_sequence = self.consumed_sequence.max(through);
                if !self.pending_demand() {
                    self.demand.clear();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START_MS: u64 = 1_000_000;

    fn view(store_version: u64) -> CampaignView {
        CampaignView {
            store_version,
            complete: false,
            cancelled: false,
            paused: false,
            unapplied_effect: false,
        }
    }

    fn hint(kind: WakeKind, observed_version: Option<u64>) -> WakeHint {
        WakeHint {
            kind,
            observed_version,
        }
    }

    fn after_batch(outcome: BatchOutcome, store_version: u64) -> ContinuationRecord {
        let mut record = ContinuationRecord::new("campaign");
        record.apply_outcome(
            outcome,
            store_version,
            0,
            START_MS,
            &ContinuationPolicy::DEFAULT,
        );
        record
    }

    fn drive(reason: DriveReason) -> ContinuationStep {
        ContinuationStep::Drive { reason }
    }

    #[test]
    fn policy_bounds_and_capped_doubling_backoff() {
        let policy = ContinuationPolicy::DEFAULT;
        assert!(policy.validate().is_ok());
        for broken in [
            ContinuationPolicy {
                rescan_interval_ms: 0,
                ..policy
            },
            ContinuationPolicy {
                retry_max_ms: MAX_INTERVAL_MS + 1,
                ..policy
            },
            ContinuationPolicy {
                retry_base_ms: DEFAULT_RETRY_MAX_MS + 1,
                ..policy
            },
            ContinuationPolicy {
                max_consecutive_failures: 0,
                ..policy
            },
            ContinuationPolicy {
                max_batches: MAX_BATCHES + 1,
                ..policy
            },
        ] {
            assert!(broken.validate().is_err(), "{broken:?}");
        }
        assert_eq!(policy.backoff_ms(1), DEFAULT_RETRY_BASE_MS);
        assert_eq!(policy.backoff_ms(2), 2 * DEFAULT_RETRY_BASE_MS);
        assert_eq!(policy.backoff_ms(3), 4 * DEFAULT_RETRY_BASE_MS);
        assert_eq!(policy.backoff_ms(8), DEFAULT_RETRY_MAX_MS);
        assert_eq!(policy.backoff_ms(u32::MAX), DEFAULT_RETRY_MAX_MS);
        assert_eq!(WakeKind::parse("capacity").ok(), Some(WakeKind::Capacity));
        assert!(WakeKind::parse("deploy").is_err());
    }

    // r[verify kiln.implementation.campaign.continuation.scenario.coalesce]
    #[test]
    fn duplicate_and_out_of_order_hints_coalesce_and_mid_batch_hints_stay_pending() {
        let policy = ContinuationPolicy::DEFAULT;
        let mut record = after_batch(BatchOutcome::Waiting, 7);
        for wake in [
            hint(WakeKind::Review, Some(9)),
            hint(WakeKind::Review, Some(8)),
            hint(WakeKind::Capacity, None),
        ] {
            record.record_hint(wake);
        }
        assert_eq!(
            record.demand,
            BTreeSet::from([WakeKind::Review, WakeKind::Capacity])
        );
        assert_eq!(record.demand_sequence, 3);
        assert_eq!(record.highest_hinted_version, Some(9));
        assert_eq!(
            record.decide(view(7), START_MS, &policy),
            drive(DriveReason::WakeDemand)
        );
        assert!(record.validate().is_ok());

        let consumed = record.demand_sequence;
        record.record_hint(hint(WakeKind::Verification, None));
        record.apply_outcome(BatchOutcome::Waiting, 8, consumed, START_MS + 1, &policy);
        assert!(record.pending_demand());
        assert_eq!(
            record.decide(view(8), START_MS + 1, &policy),
            drive(DriveReason::WakeDemand)
        );
        let consumed = record.demand_sequence;
        record.apply_outcome(BatchOutcome::Waiting, 8, consumed, START_MS + 2, &policy);
        assert!(!record.pending_demand());
        assert!(record.demand.is_empty());
        assert!(record.validate().is_ok());
    }

    // r[verify kiln.implementation.campaign.continuation.scenario.idle]
    #[test]
    fn waiting_campaigns_do_not_drive_until_news_or_the_rescan() {
        let policy = ContinuationPolicy::DEFAULT;
        let fresh = ContinuationRecord::new("campaign");
        assert_eq!(
            fresh.decide(view(1), START_MS, &policy),
            drive(DriveReason::FirstStep)
        );
        let record = after_batch(BatchOutcome::Waiting, 7);
        let rescan = START_MS + DEFAULT_RESCAN_INTERVAL_MS;
        assert_eq!(
            record.decide(view(7), START_MS + 1, &policy),
            ContinuationStep::Wait {
                reason: WaitReason::AwaitingObservation,
                until_ms: Some(rescan),
            }
        );
        assert_eq!(
            record.decide(view(7), rescan, &policy),
            drive(DriveReason::Rescan)
        );
        assert_eq!(
            record.decide(view(8), START_MS + 1, &policy),
            drive(DriveReason::VersionChanged)
        );
        let limited = after_batch(BatchOutcome::OperationLimit, 7);
        assert_eq!(
            limited.decide(view(7), START_MS + 1, &policy),
            drive(DriveReason::OperationLimit)
        );
    }

    // r[verify kiln.implementation.campaign.continuation.scenario.pause]
    #[test]
    fn pause_cancel_and_completion_bound_admission_but_not_reconciliation() {
        let policy = ContinuationPolicy::DEFAULT;
        let record = after_batch(BatchOutcome::Waiting, 7);
        let paused = CampaignView {
            paused: true,
            ..view(7)
        };
        assert_eq!(
            record.decide(paused, START_MS + 1, &policy),
            ContinuationStep::Wait {
                reason: WaitReason::Paused,
                until_ms: None,
            }
        );
        for reconciling in [
            CampaignView {
                unapplied_effect: true,
                ..paused
            },
            CampaignView {
                cancelled: true,
                paused: true,
                unapplied_effect: true,
                ..view(7)
            },
        ] {
            assert_eq!(
                record.decide(reconciling, START_MS + 1, &policy),
                ContinuationStep::Wait {
                    reason: WaitReason::AwaitingObservation,
                    until_ms: Some(START_MS + DEFAULT_RESCAN_INTERVAL_MS),
                },
                "an unresolved effect waits for its owner instead of busy-reconciling"
            );
            assert_eq!(
                record.decide(
                    CampaignView {
                        store_version: 8,
                        ..reconciling
                    },
                    START_MS + 1,
                    &policy
                ),
                drive(DriveReason::VersionChanged)
            );
            let mut hinted = record.clone();
            hinted.record_hint(hint(WakeKind::Reconciliation, None));
            assert_eq!(
                hinted.decide(reconciling, START_MS + 1, &policy),
                drive(DriveReason::WakeDemand)
            );
        }
        assert_eq!(
            record.decide(
                CampaignView {
                    cancelled: true,
                    ..view(7)
                },
                START_MS + 1,
                &policy
            ),
            ContinuationStep::Stop {
                reason: StopReason::Cancelled,
            }
        );
        assert_eq!(
            record.decide(
                CampaignView {
                    complete: true,
                    ..view(7)
                },
                START_MS + 1,
                &policy
            ),
            ContinuationStep::Stop {
                reason: StopReason::Complete,
            }
        );
    }

    // r[verify kiln.implementation.campaign.continuation.scenario.backoff]
    #[test]
    fn only_failures_back_off_and_exhaust_the_budget_until_an_operator_hint() {
        let policy = ContinuationPolicy {
            max_consecutive_failures: 3,
            ..ContinuationPolicy::DEFAULT
        };
        let mut record = ContinuationRecord::new("campaign");
        record.record_hint(hint(WakeKind::Source, None));
        let consumed = record.demand_sequence;
        record.apply_outcome(BatchOutcome::Failed, 7, consumed, START_MS, &policy);
        assert_eq!(record.consecutive_failures, 1);
        assert!(record.pending_demand());
        let retry = START_MS + DEFAULT_RETRY_BASE_MS;
        assert_eq!(
            record.decide(view(7), START_MS + 1, &policy),
            ContinuationStep::Wait {
                reason: WaitReason::Backoff,
                until_ms: Some(retry),
            }
        );
        let mut undemanded = ContinuationRecord::new("campaign");
        undemanded.apply_outcome(BatchOutcome::Failed, 7, 0, START_MS, &policy);
        assert_eq!(
            undemanded.decide(view(7), retry, &policy),
            drive(DriveReason::Retry),
            "a failure without demand retries at its deadline, not at the rescan"
        );
        assert_eq!(
            record.decide(view(7), retry, &policy),
            drive(DriveReason::WakeDemand)
        );
        for outcome in [BatchOutcome::StaleVersion, BatchOutcome::AdmissionRefused] {
            let mut other = ContinuationRecord::new("campaign");
            other.apply_outcome(outcome, 7, 0, START_MS, &policy);
            assert_eq!(other.consecutive_failures, 0, "{outcome:?}");
            assert_eq!(other.next_eligible_ms, 0, "{outcome:?}");
        }
        let mut stale = ContinuationRecord::new("campaign");
        stale.apply_outcome(BatchOutcome::StaleVersion, 7, 0, START_MS, &policy);
        assert_eq!(
            stale.decide(view(7), START_MS, &policy),
            drive(DriveReason::VersionChanged)
        );

        record.apply_outcome(BatchOutcome::Failed, 7, consumed, retry, &policy);
        record.apply_outcome(BatchOutcome::Failed, 7, consumed, retry, &policy);
        assert_eq!(
            record.decide(view(7), u64::MAX, &policy),
            ContinuationStep::Stop {
                reason: StopReason::FailureBudgetExhausted,
            }
        );
        record.record_hint(hint(WakeKind::Operator, None));
        assert_eq!(record.consecutive_failures, 0);
        assert_eq!(
            record.decide(view(7), retry, &policy),
            drive(DriveReason::WakeDemand)
        );
    }

    #[test]
    fn inconsistent_records_are_refused() {
        let mut record = ContinuationRecord::new("campaign");
        assert!(record.validate().is_ok());
        record.consumed_sequence = 1;
        assert!(record.validate().is_err());
        let mut demand_without_hint = ContinuationRecord::new("campaign");
        demand_without_hint.demand.insert(WakeKind::Review);
        assert!(demand_without_hint.validate().is_err());
        let mut other_schema = ContinuationRecord::new("campaign");
        other_schema.schema = "kiln.implementation-campaign-continuation.v0".into();
        assert!(other_schema.validate().is_err());
    }
}
