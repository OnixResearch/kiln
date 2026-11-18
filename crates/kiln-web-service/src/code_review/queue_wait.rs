//! Synthetic review-queue wait measurement (#61): the frozen load shape, the
//! per-candidate facts a fixture shell observes, the outcome classifier and a
//! receipt whose four series are separately typed. Pure: no I/O, clock,
//! provider or queue access. The shell is `examples/review_queue_wait`.
// r[impl kiln.admission.queue_wait]
use serde::{Deserialize, Serialize};

pub const LOAD_SCHEMA: &str = "kiln.review-queue-wait.load.v1";
pub const RECEIPT_SCHEMA: &str = "kiln.review-queue-wait.receipt.v1";
const LOAD_DOMAIN: &[u8] = b"kiln.review-queue-wait.load.v1\0";
/// Longest synthetic tick, in milliseconds.
pub const MAX_TICK_MS: u64 = 60_000;
/// Most ticks one run may observe.
pub const MAX_HORIZON_TICKS: u32 = 256;
/// Largest queue bound a load may declare.
pub const MAX_QUEUE_ENTRIES: usize = 100;
/// Most candidates in one load.
pub const MAX_CANDIDATES: usize = 16;
/// Longest scripted CI duration, in ticks.
pub const MAX_CI_TICKS: u32 = 16;
/// Largest speculative window a load may declare.
pub const MAX_WINDOW: usize = 8;
/// Largest receipt the read-back admits.
pub const MAX_RECEIPT_BYTES: usize = 4 * 1024 * 1024;
const MAX_ID_BYTES: usize = 48;
const MAX_MODES: usize = 2;
const MEDIAN_PERCENT: usize = 50;
const TAIL_PERCENT: usize = 95;
const PERCENT: usize = 100;

/// The queue's detail for a candidate whose canonical CI failed.
pub const CI_FAILED: &str = "review_queue_ci_failed";
/// Code of an entry withdrawn by its synthetic submitter at the declared deadline.
pub const WITHDRAWN: &str = "withdrawn_at_deadline";
const ACTIVE: [&str; 4] = ["queued", "preparing", "waiting_ci", "submitting"];

pub const NON_CLAIMS: [&str; 5] = [
    "Synthetic clock and scripted CI durations; not production CI, reviewer or merge latency",
    "One CI execution slot for the fixture project",
    "No admission, queue or budget policy was changed",
    "Times are observed at tick boundaries; resolution is one tick",
    "Expired means the synthetic submitter withdrew its entry at its declared deadline; the review queue has no entry deadline",
];
const MERGE_QUEUE_REASON: &str = "No external merge or landing queue is in this fixture's path; candidates land through the review queue's exact-target submit";
const READY_WORK_REASON: &str = "Implementation work admission is not exercised by this fixture";
const INFERENCE_REASON: &str = "No inference requests are issued by this fixture";

/// One frozen synthetic load (`kiln.review-queue-wait.load.v1`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Load {
    pub schema: String,
    pub tick_ms: u64,
    pub horizon_ticks: u32,
    pub max_entries: usize,
    pub modes: Vec<ModeSpec>,
    pub candidates: Vec<Candidate>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Serial,
    Speculative,
}

/// A queue mode; speculative mode carries its window, serial mode none.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeSpec {
    pub mode: Mode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<Window>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub floor: usize,
    pub ceiling: usize,
    pub increase: usize,
    pub decrease: usize,
}

/// The result every CI run of a candidate reports, `ci_ticks` after dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiResult {
    Pass,
    Fail,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub id: String,
    pub arrival_tick: u32,
    pub ci: CiResult,
    pub ci_ticks: u32,
    /// Ticks after enqueue at which the submitter withdraws a still-active entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_ticks: Option<u32>,
}

fn invalid<T>(reason: &str) -> Result<T, String> {
    Err(format!("queue_wait_load_invalid: {reason}"))
}

fn valid_id(id: &str) -> bool {
    let mut bytes = id.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && id.len() <= MAX_ID_BYTES
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().to_hex().to_string()
}

impl Load {
    /// Admit an exported load and return it with its identity.
    pub fn admit(bytes: &[u8]) -> Result<(Self, String), String> {
        let load: Self =
            serde_json::from_slice(bytes).map_err(|_| "queue_wait_load_malformed".to_owned())?;
        load.validate()?;
        let identity = load.identity()?;
        Ok((load, identity))
    }

    /// Every bound and cross-field rule of the Nickel contract, rechecked.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != LOAD_SCHEMA {
            return invalid("schema");
        }
        if !(1..=MAX_TICK_MS).contains(&self.tick_ms)
            || !(1..=MAX_HORIZON_TICKS).contains(&self.horizon_ticks)
            || !(1..=MAX_QUEUE_ENTRIES).contains(&self.max_entries)
        {
            return invalid("clock_or_queue_bound");
        }
        if self.modes.is_empty() || self.modes.len() > MAX_MODES {
            return invalid("modes");
        }
        for (index, spec) in self.modes.iter().enumerate() {
            if self
                .modes
                .iter()
                .take(index)
                .any(|other| other.mode == spec.mode)
            {
                return invalid("duplicate_mode");
            }
            let window_valid = match (spec.mode, spec.window) {
                (Mode::Serial, None) => true,
                (Mode::Speculative, Some(window)) => {
                    window.floor >= 1
                        && window.floor <= window.ceiling
                        && window.ceiling <= MAX_WINDOW.min(self.max_entries)
                        && (1..=MAX_WINDOW).contains(&window.increase)
                        && (1..=MAX_WINDOW).contains(&window.decrease)
                }
                _ => false,
            };
            if !window_valid {
                return invalid("window");
            }
        }
        if self.candidates.is_empty() || self.candidates.len() > MAX_CANDIDATES {
            return invalid("candidate_count");
        }
        for (index, candidate) in self.candidates.iter().enumerate() {
            if !valid_id(&candidate.id)
                || self
                    .candidates
                    .iter()
                    .take(index)
                    .any(|other| other.id == candidate.id)
            {
                return invalid("candidate_id");
            }
            if candidate.arrival_tick > self.horizon_ticks
                || !(1..=MAX_CI_TICKS).contains(&candidate.ci_ticks)
                || candidate
                    .deadline_ticks
                    .is_some_and(|ticks| !(1..=MAX_HORIZON_TICKS).contains(&ticks))
            {
                return invalid("candidate_timing");
            }
        }
        Ok(())
    }

    /// Domain-tagged BLAKE3 over the canonical serialization of the typed load.
    pub fn identity(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        Ok(digest(LOAD_DOMAIN, &bytes))
    }

    /// Synthetic milliseconds at the horizon tick.
    pub fn horizon_ms(&self) -> u64 {
        self.tick_ms.saturating_mul(u64::from(self.horizon_ticks))
    }
}

/// One queue status first observed at a tick boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub at_ms: u64,
    pub status: String,
}

/// What a fixture shell observed for one candidate in one arm. Times are
/// synthetic milliseconds since tick 0, each the first tick boundary that showed it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observed {
    pub candidate: String,
    pub source_commit: String,
    pub change: Option<String>,
    pub arrival_ms: u64,
    /// Code of a refused enqueue; the candidate never entered the queue.
    pub refusal: Option<String>,
    pub enqueue_ms: Option<u64>,
    /// First observation of the queue constructing or testing an exact
    /// candidate (`preparing`, `waiting_ci`, `submitting`, `uncertain` or
    /// `submitted`); an entry withdrawn or blocked while queued never started.
    pub start_ms: Option<u64>,
    /// First observation of a canonical CI run for an exact candidate.
    pub ci_requested_ms: Option<u64>,
    /// Every dispatch of the candidate's CI to the workflow provider, in order.
    /// One may follow `terminal_ms`: a withdrawn entry's queued run still runs.
    pub ci_dispatches_ms: Vec<u64>,
    pub terminal_ms: Option<u64>,
    /// Accepted deadline withdrawal by the synthetic submitter.
    pub withdrawn_ms: Option<u64>,
    /// Queue status and detail at the last observation.
    pub status: Option<String>,
    pub detail: Option<String>,
    /// Whether the entry's current CI run had an unknown outcome at the last observation.
    pub ci_unknown: bool,
    pub candidate_commits: Vec<String>,
    pub ci_runs: Vec<String>,
    pub transitions: Vec<Transition>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Submitted,
    Waiting,
    Rejected,
    Expired,
    Failed,
    Unknown,
    Cancelled,
}

impl Outcome {
    /// Whether the candidate left the queue for good by the horizon.
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Waiting | Self::Unknown)
    }
}

/// One named outcome from recorded signals only.
pub fn classify(observed: &Observed) -> (Outcome, Option<String>) {
    if let Some(code) = &observed.refusal {
        return (Outcome::Rejected, Some(code.clone()));
    }
    let detail = observed.detail.clone();
    match observed.status.as_deref() {
        Some("submitted") => (Outcome::Submitted, None),
        Some("cancelled") if observed.withdrawn_ms.is_some() => {
            (Outcome::Expired, Some(WITHDRAWN.to_owned()))
        }
        Some("cancelled") => (Outcome::Cancelled, detail),
        Some("blocked") if detail.as_deref() == Some(CI_FAILED) => (Outcome::Failed, detail),
        Some("blocked") => (Outcome::Rejected, detail),
        Some("uncertain") => (
            Outcome::Unknown,
            Some("provider_operation_uncertain".to_owned()),
        ),
        Some(status) if ACTIVE.contains(&status) && observed.ci_unknown => {
            (Outcome::Unknown, Some("ci_outcome_unknown".to_owned()))
        }
        Some(status) if ACTIVE.contains(&status) => (Outcome::Waiting, Some(status.to_owned())),
        Some(status) => (
            Outcome::Unknown,
            Some(format!("unrecognized_status:{status}")),
        ),
        None => (Outcome::Unknown, Some("not_observed".to_owned())),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateRecord {
    pub observed: Observed,
    pub outcome: Outcome,
    pub code: Option<String>,
}

/// Every outcome with an explicit count, zero included.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeCounts {
    pub submitted: usize,
    pub waiting: usize,
    pub rejected: usize,
    pub expired: usize,
    pub failed: usize,
    pub unknown: usize,
    pub cancelled: usize,
}

/// Nearest-rank distribution of one wait, in milliseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Distribution {
    pub samples: usize,
    pub min: u64,
    pub p50: u64,
    pub p95: u64,
    pub max: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub candidates: usize,
    pub outcomes: OutcomeCounts,
    pub enqueue_to_start_ms: Option<Distribution>,
    pub enqueue_to_ci_dispatch_ms: Option<Distribution>,
    pub enqueue_to_submitted_ms: Option<Distribution>,
}

fn nearest_rank(sorted: &[u64], percent: usize) -> Option<u64> {
    let rank = percent
        .saturating_mul(sorted.len())
        .div_ceil(PERCENT)
        .max(1);
    sorted.get(rank - 1).copied()
}

/// `None` when nothing was observed: an absent wait is never zero.
pub fn distribution(mut values: Vec<u64>) -> Option<Distribution> {
    values.sort_unstable();
    Some(Distribution {
        samples: values.len(),
        min: *values.first()?,
        p50: nearest_rank(&values, MEDIAN_PERCENT)?,
        p95: nearest_rank(&values, TAIL_PERCENT)?,
        max: *values.last()?,
    })
}

fn waits(records: &[CandidateRecord], until: impl Fn(&CandidateRecord) -> Option<u64>) -> Vec<u64> {
    records
        .iter()
        .filter_map(|record| {
            let enqueued = record.observed.enqueue_ms?;
            until(record)?.checked_sub(enqueued)
        })
        .collect()
}

pub fn summarize(records: &[CandidateRecord]) -> Summary {
    let mut outcomes = OutcomeCounts::default();
    for record in records {
        let count = match record.outcome {
            Outcome::Submitted => &mut outcomes.submitted,
            Outcome::Waiting => &mut outcomes.waiting,
            Outcome::Rejected => &mut outcomes.rejected,
            Outcome::Expired => &mut outcomes.expired,
            Outcome::Failed => &mut outcomes.failed,
            Outcome::Unknown => &mut outcomes.unknown,
            Outcome::Cancelled => &mut outcomes.cancelled,
        };
        *count += 1;
    }
    Summary {
        candidates: records.len(),
        outcomes,
        enqueue_to_start_ms: distribution(waits(records, |record| record.observed.start_ms)),
        enqueue_to_ci_dispatch_ms: distribution(waits(records, |record| {
            record.observed.ci_dispatches_ms.first().copied()
        })),
        enqueue_to_submitted_ms: distribution(waits(records, |record| {
            (record.outcome == Outcome::Submitted)
                .then_some(record.observed.terminal_ms)
                .flatten()
        })),
    }
}

/// Recorded times must be ordered, inside the horizon and consistent with the outcome.
fn record_consistent(record: &CandidateRecord, horizon_ms: u64) -> bool {
    let observed = &record.observed;
    let ordered = [
        Some(observed.arrival_ms),
        observed.enqueue_ms,
        observed.start_ms,
        observed.ci_requested_ms,
        observed.terminal_ms,
    ]
    .into_iter()
    .flatten()
    .try_fold(0, |previous, at| {
        (at >= previous && at <= horizon_ms).then_some(at)
    })
    .is_some();
    let dispatched = match (observed.ci_requested_ms, observed.ci_dispatches_ms.first()) {
        (_, None) => true,
        (Some(requested), Some(first)) => {
            requested <= *first
                && observed
                    .ci_dispatches_ms
                    .windows(2)
                    .all(|pair| pair.first() <= pair.last())
                && observed
                    .ci_dispatches_ms
                    .last()
                    .is_some_and(|last| *last <= horizon_ms)
        }
        (None, Some(_)) => false,
    };
    let refused = observed.refusal.is_some();
    ordered
        && dispatched
        && classify(observed) == (record.outcome, record.code.clone())
        && observed.terminal_ms.is_some() == record.outcome.is_terminal()
        && refused == observed.enqueue_ms.is_none()
        && (!refused || observed.terminal_ms == Some(observed.arrival_ms))
        && (record.outcome != Outcome::Expired || observed.withdrawn_ms == observed.terminal_ms)
}

/// One queue mode run over the whole load.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arm {
    pub mode: ModeSpec,
    /// Concurrent canonical CI executions available to the fixture project.
    pub ci_slots: usize,
    pub max_candidate_builds: u32,
    pub candidates: Vec<CandidateRecord>,
    pub summary: Summary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewQueueWait {
    pub arms: Vec<Arm>,
}

/// Uninhabited: a series this fixture cannot measure holds no value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NotExercised {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesName {
    ReviewQueueWait,
    MergeQueueWait,
    ReadyWorkAdmissionWait,
    InferenceBackpressure,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Measurement<T> {
    Measured { value: T },
    NotMeasured { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series<T> {
    pub series: SeriesName,
    pub measurement: Measurement<T>,
}

/// The four meanings the issue keeps apart; only review-queue wait can carry numbers here.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeriesSet {
    pub review_queue_wait: Series<ReviewQueueWait>,
    pub merge_queue_wait: Series<NotExercised>,
    pub ready_work_admission_wait: Series<NotExercised>,
    pub inference_backpressure: Series<NotExercised>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: String,
    pub load_identity: String,
    pub load: Load,
    pub series: SeriesSet,
    pub non_claims: Vec<String>,
}

/// One arm as the shell observed it, before classification.
#[derive(Clone, Debug)]
pub struct ArmObservation {
    pub mode: ModeSpec,
    pub ci_slots: usize,
    pub max_candidate_builds: u32,
    pub observed: Vec<Observed>,
}

fn not_measured(series: SeriesName, reason: &str) -> Series<NotExercised> {
    Series {
        series,
        measurement: Measurement::NotMeasured {
            reason: reason.to_owned(),
        },
    }
}

fn refused<T>(reason: &str) -> Result<T, String> {
    Err(format!("queue_wait_receipt_invalid: {reason}"))
}

impl Receipt {
    /// Classify and summarize the observed arms of an admitted load.
    pub fn new(load: &Load, arms: Vec<ArmObservation>) -> Result<Self, String> {
        let arms = arms
            .into_iter()
            .map(|arm| {
                let candidates: Vec<_> = arm
                    .observed
                    .into_iter()
                    .map(|observed| {
                        let (outcome, code) = classify(&observed);
                        CandidateRecord {
                            observed,
                            outcome,
                            code,
                        }
                    })
                    .collect();
                Arm {
                    mode: arm.mode,
                    ci_slots: arm.ci_slots,
                    max_candidate_builds: arm.max_candidate_builds,
                    summary: summarize(&candidates),
                    candidates,
                }
            })
            .collect();
        let receipt = Self {
            schema: RECEIPT_SCHEMA.to_owned(),
            load_identity: load.identity()?,
            load: load.clone(),
            series: SeriesSet {
                review_queue_wait: Series {
                    series: SeriesName::ReviewQueueWait,
                    measurement: Measurement::Measured {
                        value: ReviewQueueWait { arms },
                    },
                },
                merge_queue_wait: not_measured(SeriesName::MergeQueueWait, MERGE_QUEUE_REASON),
                ready_work_admission_wait: not_measured(
                    SeriesName::ReadyWorkAdmissionWait,
                    READY_WORK_REASON,
                ),
                inference_backpressure: not_measured(
                    SeriesName::InferenceBackpressure,
                    INFERENCE_REASON,
                ),
            },
            non_claims: NON_CLAIMS.iter().map(|claim| (*claim).to_owned()).collect(),
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Read back a serialized receipt, refusing anything it cannot recompute.
    pub fn admit(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_RECEIPT_BYTES {
            return refused("size");
        }
        let receipt: Self =
            serde_json::from_slice(bytes).map_err(|_| "queue_wait_receipt_malformed".to_owned())?;
        receipt.validate()?;
        Ok(receipt)
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema != RECEIPT_SCHEMA {
            return refused("schema");
        }
        self.load.validate()?;
        if self.load.identity()? != self.load_identity {
            return refused("load_identity");
        }
        if self.non_claims.iter().map(String::as_str).ne(NON_CLAIMS) {
            return refused("non_claims");
        }
        let series = &self.series;
        let labels = [
            (series.review_queue_wait.series, SeriesName::ReviewQueueWait),
            (series.merge_queue_wait.series, SeriesName::MergeQueueWait),
            (
                series.ready_work_admission_wait.series,
                SeriesName::ReadyWorkAdmissionWait,
            ),
            (
                series.inference_backpressure.series,
                SeriesName::InferenceBackpressure,
            ),
        ];
        if labels.iter().any(|(label, slot)| label != slot) {
            return refused("series_label");
        }
        for unmeasured in [
            &series.merge_queue_wait,
            &series.ready_work_admission_wait,
            &series.inference_backpressure,
        ] {
            if let Measurement::NotMeasured { reason } = &unmeasured.measurement
                && reason.trim().is_empty()
            {
                return refused("not_measured_reason");
            }
        }
        let Measurement::Measured { value } = &series.review_queue_wait.measurement else {
            return refused("review_queue_wait_missing");
        };
        if value.arms.len() != self.load.modes.len() {
            return refused("arms");
        }
        let horizon_ms = self.load.horizon_ms();
        for (arm, mode) in value.arms.iter().zip(&self.load.modes) {
            if &arm.mode != mode
                || arm.ci_slots == 0
                || arm.candidates.len() != self.load.candidates.len()
                || arm
                    .candidates
                    .iter()
                    .zip(&self.load.candidates)
                    .any(|(record, candidate)| record.observed.candidate != candidate.id)
            {
                return refused("arm_subjects");
            }
            if !arm
                .candidates
                .iter()
                .all(|record| record_consistent(record, horizon_ms))
            {
                return refused("candidate_record");
            }
            if arm.summary != summarize(&arm.candidates) {
                return refused("summary");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "queue_wait_tests.rs"]
mod tests;
