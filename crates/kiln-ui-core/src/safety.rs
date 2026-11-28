//! Kiln safety invariants: per-buffer freshness gate, 4-level safety tiers,
//! `Unknown` run fail-closed rules, and per-item batch receipt ledgers.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Maximum age (in milliseconds) of a buffer observation before mutations fail closed.
pub const CONTROL_FRESHNESS_MS: u64 = 30_000;
/// Maximum retained receipts in a single batch execution ledger.
pub const MAX_BATCH_RECEIPTS: usize = 64;

/// Four-level operator safety ladder balancing speed with Kiln exact-identity guarantees.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyTier {
    /// 1 key, 0 confirms: strictly reversible/fenced single-item ops (row locks `Submitting…`).
    Tier1Instant,
    /// Deliberate typed input -> `Enter` on target-badged Layer-1 bar (no second `[y/N]` modal).
    Tier2aInput,
    /// 1 key -> non-blocking Layer-1 inline confirm `[Action + exact ID/commit? y/Esc]`.
    Tier2bInlineConfirm,
    /// Centered Layer-2 destructive guard with exact BLAKE3 + fence + required reason + live stale diff + CLI copy.
    Tier3DestructiveOverlay,
}

/// Canonical operator verb across all buffers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    // Read / workspace verbs (always allowed on single or multi-select)
    OpenSplits,
    YankIdentity,
    YankUrl,
    YankAttach,
    YankCli,
    PipeContext,
    // Tier 1 (single-item) -> auto-promotes to Tier 2b on N >= 2
    RenewClaim30m,
    RenewClaim60m,
    ProbeReconcile,
    RefreshRepoCensus,
    WakeCampaign,
    // Tier 2a (deliberate input on Layer-1 bar; single-target only!)
    PromptAgent,
    NudgeAgentPrefill,
    AnswerDecision,
    CommentReview,
    // Tier 2b (inline confirm on Layer-1 bar)
    PauseDispatch,
    ResumeDispatch,
    DrainMachine,
    UndrainMachine,
    InterruptAttempt,
    RerunExact,
    RerunFailed,
    AcceptPlan,
    EnqueueMergeQueue,
    ApproveUntrustedRun,
    ReviewVerdict,
    MarkFileViewed,
    // Tier 3 (destructive overlay; single-target only!)
    ForceReleaseClaim,
    CancelAttempt,
    CancelRun,
    DequeueMergeTrain,
    RemoveRepoFromCatalog,
}

impl Verb {
    /// Returns `true` iff this verb is purely read-only / workspace navigation.
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        matches!(
            self,
            Self::OpenSplits
                | Self::YankIdentity
                | Self::YankUrl
                | Self::YankAttach
                | Self::YankCli
                | Self::PipeContext
                | Self::MarkFileViewed
        )
    }

    /// Base safety tier for a single-item invocation.
    #[must_use]
    pub const fn single_item_tier(self) -> SafetyTier {
        match self {
            Self::OpenSplits
            | Self::YankIdentity
            | Self::YankUrl
            | Self::YankAttach
            | Self::YankCli
            | Self::RenewClaim30m
            | Self::RenewClaim60m
            | Self::ProbeReconcile
            | Self::RefreshRepoCensus
            | Self::WakeCampaign
            | Self::MarkFileViewed => SafetyTier::Tier1Instant,
            Self::PipeContext
            | Self::PromptAgent
            | Self::NudgeAgentPrefill
            | Self::AnswerDecision
            | Self::CommentReview => SafetyTier::Tier2aInput,
            Self::PauseDispatch
            | Self::ResumeDispatch
            | Self::DrainMachine
            | Self::UndrainMachine
            | Self::InterruptAttempt
            | Self::RerunExact
            | Self::RerunFailed
            | Self::AcceptPlan
            | Self::EnqueueMergeQueue
            | Self::ApproveUntrustedRun
            | Self::ReviewVerdict => SafetyTier::Tier2bInlineConfirm,
            Self::ForceReleaseClaim
            | Self::CancelAttempt
            | Self::CancelRun
            | Self::DequeueMergeTrain
            | Self::RemoveRepoFromCatalog => SafetyTier::Tier3DestructiveOverlay,
        }
    }

    /// Strict multi-select (`N >= 2`) safety whitelist.
    ///
    /// Kiln has no atomic batch-mutation API: blind multi-agent prompt broadcast,
    /// bulk cancel, bulk force-release, and bulk merge enqueue are explicitly forbidden.
    #[must_use]
    pub const fn allowed_on_multi_select(self) -> bool {
        matches!(
            self,
            Self::OpenSplits
                | Self::YankIdentity
                | Self::YankUrl
                | Self::PipeContext
                | Self::RenewClaim30m
                | Self::ProbeReconcile
        )
    }

    /// Effective safety tier given selection count (`1` vs `N >= 2`).
    /// Returns `None` if this verb is forbidden on multi-select.
    #[must_use]
    pub const fn effective_tier(self, selected_count: usize) -> Option<SafetyTier> {
        if selected_count <= 1 {
            return Some(self.single_item_tier());
        }
        if !self.allowed_on_multi_select() {
            return None;
        }
        match self {
            Self::RenewClaim30m | Self::ProbeReconcile => Some(SafetyTier::Tier2bInlineConfirm),
            _ => Some(self.single_item_tier()),
        }
    }
}

/// Explicit machine reporting states — never collapsed into one bucket or fabricated as `0`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineHealth {
    Reporting,
    Full,
    Draining,
    Unreachable,
    Stale,
    NoReport,
}

/// Canonical CI run outcome used for fail-closed control enablement.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Pending,
    Active,
    Success,
    DomainFailure,
    Denied,
    ProviderError,
    TimedOut,
    Cancelled,
    Unknown,
}

/// Reason why a mutation was rejected fail-closed inside pure `kiln-ui-core`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum MutationBlockReason {
    ReadOnlyPrincipal,
    SseDisconnected,
    UnobservedSnapshot,
    StaleSnapshot { age_ms: u64, limit_ms: u64 },
    UnknownRunMustReconcile,
    SingleTargetOnly { verb: Verb, count: usize },
    MutationAlreadyInFlight { request_id: String },
    MissingFenceOrIdentity,
}

/// Per-buffer observation freshness state enforced on every mutating verb.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FreshnessGuard {
    pub observed_at_ms: Option<u64>,
    pub sse_connected: bool,
}

impl FreshnessGuard {
    /// Evaluate whether mutations are permitted against this buffer at `now_ms`.
    pub fn check_fresh(&self, now_ms: u64) -> Result<u64, MutationBlockReason> {
        if !self.sse_connected {
            return Err(MutationBlockReason::SseDisconnected);
        }
        let Some(observed) = self.observed_at_ms else {
            return Err(MutationBlockReason::UnobservedSnapshot);
        };
        let age_ms = now_ms.saturating_sub(observed);
        if age_ms >= CONTROL_FRESHNESS_MS {
            return Err(MutationBlockReason::StaleSnapshot {
                age_ms,
                limit_ms: CONTROL_FRESHNESS_MS,
            });
        }
        Ok(age_ms)
    }
}

/// Check if a CI run verb is allowed for the observed outcome.
/// `Unknown` runs MUST be reconciled (`ProbeReconcile`) before any rerun.
pub fn check_run_verb(outcome: RunOutcome, verb: Verb) -> Result<(), MutationBlockReason> {
    if outcome == RunOutcome::Unknown && matches!(verb, Verb::RerunExact | Verb::RerunFailed) {
        return Err(MutationBlockReason::UnknownRunMustReconcile);
    }
    Ok(())
}

/// In-flight single mutation lock (no optimistic auto-advance; single `request_id`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PendingMutation {
    pub request_id: String,
    pub verb: Verb,
    pub target_id: String,
    pub expected_fence: Option<u64>,
}

/// Per-item execution status inside a whitelisted batch receipt ledger.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReceiptStatus {
    Succeeded,
    RejectedStaleFence { detail: String },
    UnknownTimeout { detail: String },
}

/// One exact item's recorded outcome from a sequential fenced batch operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchReceipt {
    pub target_id: String,
    pub fence: Option<u64>,
    pub status: ReceiptStatus,
}

/// Pinned receipt ledger retaining non-2xx outcomes so nothing is silently retried or swallowed.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchReceiptLedger {
    pub verb: Option<Verb>,
    pub entries: Vec<BatchReceipt>,
}

impl BatchReceiptLedger {
    /// Start a fresh receipt ledger for a batch verb.
    pub fn reset(&mut self, verb: Verb) {
        self.verb = Some(verb);
        self.entries.clear();
    }

    /// Record one item's settled receipt within the `MAX_BATCH_RECEIPTS` bound.
    pub fn record(&mut self, receipt: BatchReceipt) {
        if self.entries.len() < MAX_BATCH_RECEIPTS {
            self.entries.push(receipt);
        }
    }

    /// Count of confirmed HTTP 2xx items.
    #[must_use]
    pub fn succeeded_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e.status, ReceiptStatus::Succeeded))
            .count()
    }

    /// Count of items that failed (`409` stale fence or timeout `Unknown`) and stay selected.
    #[must_use]
    pub fn failed_or_unknown_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| !matches!(e.status, ReceiptStatus::Succeeded))
            .count()
    }
}
