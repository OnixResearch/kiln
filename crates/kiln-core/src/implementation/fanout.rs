//! Pure `#![no_std]` parent fan-out run state machine for reviewed runbooks.
//!
//! Tracks deterministic child expansion, bounded concurrency, budget caps,
//! partial-failure isolation (`Success` / `Failure` / `Unknown` / `Skipped`),
//! admission-only controls (`pause` / `continue` / `cancel`), and fleet write
//! scope overlap checks.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::refusal;
use crate::fleet::{ClaimConflict, FleetClaim, FleetScope, overlaps};
use crate::paths::{matches as path_matches, valid_pattern};
use crate::source::{SourceResolution, validate_resolution};
use crate::types::CoreResult;

pub const FANOUT_RUN_SCHEMA: &str = "kiln.runbook-fanout.v1";
pub const MAX_FANOUT_TARGETS: usize = 64;
pub const MAX_FANOUT_GLOBS: usize = 16;
pub const MAX_FANOUT_CONCURRENCY: u16 = 32;
pub const MAX_FANOUT_BUDGET_UNITS: u64 = 10_000;
pub const MAX_FANOUT_UNIT_COST: u64 = 1_000;
pub const MAX_FANOUT_REASON_BYTES: usize = 256;
pub const MAX_FANOUT_RUNS: usize = 128;

pub const STOP_REASON_BUDGET_EXHAUSTED: &str = "fanout_budget_exhausted";
pub const STOP_REASON_CANCELLED: &str = "fanout_cancelled";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanoutMode {
    ReadOnly,
    Mutating,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanoutClaimPolicy {
    #[default]
    Refuse,
    Skip,
    Serialize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanoutAdmissionState {
    Open,
    Paused,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanoutPhase {
    Active,
    Paused,
    Completed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanoutChildStatus {
    Pending,
    Active,
    Success,
    Failure,
    Unknown,
    Skipped,
}

impl FanoutChildStatus {
    #[must_use]
    pub const fn is_terminal_in_fanout(self) -> bool {
        matches!(
            self,
            Self::Success | Self::Failure | Self::Unknown | Self::Skipped
        )
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    #[must_use]
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanoutBudget {
    pub ceiling_units: u64,
    pub drawn_units: u64,
    pub unit_cost: u64,
    pub admitted_runs: u32,
}

impl FanoutBudget {
    pub fn new(ceiling_units: u64, unit_cost: u64) -> CoreResult<Self> {
        let budget = Self {
            ceiling_units,
            drawn_units: 0,
            unit_cost,
            admitted_runs: 0,
        };
        budget.validate()?;
        Ok(budget)
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.ceiling_units == 0 || self.ceiling_units > MAX_FANOUT_BUDGET_UNITS {
            return Err(refusal(
                "fanout_budget_invalid",
                "budget.ceiling_units",
                "fan-out budget ceiling is outside allowed bounds",
            ));
        }
        if self.unit_cost == 0 || self.unit_cost > MAX_FANOUT_UNIT_COST {
            return Err(refusal(
                "fanout_budget_invalid",
                "budget.unit_cost",
                "fan-out unit cost is outside allowed bounds",
            ));
        }
        if self.drawn_units > self.ceiling_units {
            return Err(refusal(
                "fanout_budget_invalid",
                "budget.drawn_units",
                "fan-out drawn units exceed budget ceiling",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn can_admit(&self) -> bool {
        self.drawn_units
            .checked_add(self.unit_cost)
            .is_some_and(|next| next <= self.ceiling_units)
    }

    #[must_use]
    pub fn remaining_units(&self) -> u64 {
        self.ceiling_units.saturating_sub(self.drawn_units)
    }

    pub fn record_admit(&mut self) -> CoreResult<()> {
        let next_drawn = self
            .drawn_units
            .checked_add(self.unit_cost)
            .filter(|&next| next <= self.ceiling_units)
            .ok_or_else(|| {
                refusal(
                    STOP_REASON_BUDGET_EXHAUSTED,
                    "budget.drawn_units",
                    "fan-out budget ceiling exhausted",
                )
            })?;
        let next_runs = self.admitted_runs.checked_add(1).ok_or_else(|| {
            refusal(
                "fanout_budget_invalid",
                "budget.admitted_runs",
                "fan-out admitted run counter overflowed",
            )
        })?;
        self.drawn_units = next_drawn;
        self.admitted_runs = next_runs;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanoutChild {
    pub index: u16,
    pub target: String,
    pub repository: String,
    pub project_id: String,
    pub resolved_reference: String,
    pub source_revision: String,
    pub source: SourceResolution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_scope: Option<FleetScope>,
    pub status: FanoutChildStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome_detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admitted_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_ms: Option<u64>,
}

impl FanoutChild {
    pub fn validate(&self, mode: FanoutMode, expected_index: usize) -> CoreResult<()> {
        if usize::from(self.index) != expected_index {
            return Err(refusal(
                "fanout_child_index",
                "children.index",
                "fan-out child index is out of order",
            ));
        }
        if self.target.is_empty()
            || self.repository.is_empty()
            || self.project_id.is_empty()
            || self.resolved_reference.is_empty()
            || !is_hex40(&self.source_revision)
        {
            return Err(refusal(
                "fanout_child_invalid",
                "children",
                "fan-out child target or revision metadata is invalid",
            ));
        }
        validate_resolution(&self.source)?;
        if self.source.exact_revision.native_id.as_str() != self.source_revision {
            return Err(refusal(
                "fanout_child_revision",
                "children.source_revision",
                "fan-out child source_revision differs from frozen source resolution",
            ));
        }
        match (mode, &self.write_scope) {
            (FanoutMode::Mutating, Some(scope)) => scope.validate()?,
            (FanoutMode::ReadOnly, None) => {}
            _ => {
                return Err(refusal(
                    "fanout_child_scope",
                    "children.write_scope",
                    "fan-out child write scope conflicts with runbook mode",
                ));
            }
        }
        match self.status {
            FanoutChildStatus::Pending => {
                if self.run_id.is_some() {
                    return Err(refusal(
                        "fanout_child_state",
                        "children.run_id",
                        "pending fan-out child must not reference a run_id",
                    ));
                }
            }
            FanoutChildStatus::Active
            | FanoutChildStatus::Success
            | FanoutChildStatus::Failure
            | FanoutChildStatus::Unknown => {
                if self.run_id.as_deref().is_none_or(str::is_empty) {
                    return Err(refusal(
                        "fanout_child_state",
                        "children.run_id",
                        "admitted fan-out child requires a run_id",
                    ));
                }
            }
            FanoutChildStatus::Skipped => {
                if self.run_id.is_some() {
                    return Err(refusal(
                        "fanout_child_state",
                        "children.run_id",
                        "skipped fan-out child must not reference a run_id",
                    ));
                }
                if self
                    .reason
                    .as_deref()
                    .is_none_or(|r| r.is_empty() || r.len() > MAX_FANOUT_REASON_BYTES)
                {
                    return Err(refusal(
                        "fanout_child_state",
                        "children.reason",
                        "skipped fan-out child requires a bounded reason",
                    ));
                }
            }
        }
        if let Some(reason) = &self.reason
            && (reason.len() > MAX_FANOUT_REASON_BYTES || reason.chars().any(char::is_control))
        {
            return Err(refusal(
                "fanout_child_reason",
                "children.reason",
                "fan-out child reason exceeds bound or contains control characters",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanoutCounts {
    pub total: usize,
    pub pending: usize,
    pub active: usize,
    pub success: usize,
    pub failure: usize,
    pub unknown: usize,
    pub skipped: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanOutRun {
    pub schema: String,
    pub id: String,
    pub ordinal: u64,
    pub runbook_id: String,
    pub mode: FanoutMode,
    pub principal_id: String,
    pub request_id: String,
    pub target_selectors: Vec<String>,
    pub max_concurrency: u16,
    pub budget: FanoutBudget,
    pub claim_policy: FanoutClaimPolicy,
    pub admission: FanoutAdmissionState,
    pub phase: FanoutPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lattice_bundle_id: Option<String>,
    pub lattice_bundle_identity: String,
    pub lattice_workflow_name: String,
    pub lattice_workflow_revision: String,
    pub params_identity: String,
    #[serde(default)]
    pub params: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub env_bindings: BTreeMap<String, BTreeMap<String, String>>,
    pub children: Vec<FanoutChild>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_ms: Option<u64>,
}

impl FanOutRun {
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.children
            .iter()
            .filter(|child| child.status.is_active())
            .count()
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.children
            .iter()
            .filter(|child| child.status.is_pending())
            .count()
    }

    #[must_use]
    pub fn counts(&self) -> FanoutCounts {
        let mut counts = FanoutCounts {
            total: self.children.len(),
            ..FanoutCounts::default()
        };
        for child in &self.children {
            match child.status {
                FanoutChildStatus::Pending => counts.pending += 1,
                FanoutChildStatus::Active => counts.active += 1,
                FanoutChildStatus::Success => counts.success += 1,
                FanoutChildStatus::Failure => counts.failure += 1,
                FanoutChildStatus::Unknown => counts.unknown += 1,
                FanoutChildStatus::Skipped => counts.skipped += 1,
            }
        }
        counts
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema != FANOUT_RUN_SCHEMA {
            return Err(refusal(
                "fanout_schema",
                "schema",
                "fan-out schema is unsupported",
            ));
        }
        if self.id.is_empty() || self.runbook_id.is_empty() || self.principal_id.is_empty() {
            return Err(refusal(
                "fanout_identity",
                "id",
                "fan-out identity fields must be non-empty",
            ));
        }
        if self.max_concurrency == 0 || self.max_concurrency > MAX_FANOUT_CONCURRENCY {
            return Err(refusal(
                "fanout_concurrency_invalid",
                "max_concurrency",
                "fan-out max_concurrency is outside allowed bounds",
            ));
        }
        self.budget.validate()?;
        if self.children.is_empty() || self.children.len() > MAX_FANOUT_TARGETS {
            return Err(refusal(
                "fanout_targets_bound",
                "children",
                "fan-out child count is outside allowed bounds",
            ));
        }
        if self.target_selectors.is_empty() || self.target_selectors.len() > MAX_FANOUT_TARGETS {
            return Err(refusal(
                "fanout_targets_bound",
                "target_selectors",
                "fan-out target selectors are outside allowed bounds",
            ));
        }
        let mut seen_projects = BTreeSet::new();
        for (idx, child) in self.children.iter().enumerate() {
            child.validate(self.mode, idx)?;
            if !seen_projects.insert(child.project_id.as_str()) {
                return Err(refusal(
                    "fanout_duplicate_target",
                    "children.project_id",
                    "fan-out children must name distinct project_ids",
                ));
            }
        }
        if self.active_count() > usize::from(self.max_concurrency) {
            return Err(refusal(
                "fanout_concurrency_exceeded",
                "children",
                "active fan-out children exceed max_concurrency",
            ));
        }
        if let Some(reason) = &self.stop_reason
            && (reason.is_empty()
                || reason.len() > MAX_FANOUT_REASON_BYTES
                || reason.chars().any(char::is_control))
        {
            return Err(refusal(
                "fanout_stop_reason",
                "stop_reason",
                "fan-out stop_reason is empty, oversized, or contains control characters",
            ));
        }
        Ok(())
    }

    pub fn recompute_phase(&mut self, now_ms: u64) {
        let has_pending = self.pending_count() > 0;
        let has_active = self.active_count() > 0;
        self.phase = if !has_pending && !has_active {
            if self.completed_at_ms.is_none() {
                self.completed_at_ms = Some(now_ms);
            }
            if self.admission == FanoutAdmissionState::Cancelled {
                FanoutPhase::Cancelled
            } else {
                FanoutPhase::Completed
            }
        } else {
            self.completed_at_ms = None;
            match self.admission {
                FanoutAdmissionState::Open | FanoutAdmissionState::Cancelled => FanoutPhase::Active,
                FanoutAdmissionState::Paused => FanoutPhase::Paused,
            }
        };
    }

    pub fn pause(&mut self, now_ms: u64) -> CoreResult<()> {
        if self.admission == FanoutAdmissionState::Cancelled || self.phase == FanoutPhase::Cancelled
        {
            return Err(refusal(
                "fanout_cancelled",
                "admission",
                "cancelled fan-out run cannot be paused",
            ));
        }
        if self.phase == FanoutPhase::Completed {
            return Err(refusal(
                "fanout_completed",
                "phase",
                "completed fan-out run cannot be paused",
            ));
        }
        self.admission = FanoutAdmissionState::Paused;
        self.updated_at_ms = now_ms;
        self.recompute_phase(now_ms);
        Ok(())
    }

    pub fn continue_admission(&mut self, now_ms: u64) -> CoreResult<()> {
        if self.admission == FanoutAdmissionState::Cancelled || self.phase == FanoutPhase::Cancelled
        {
            return Err(refusal(
                "fanout_cancelled",
                "admission",
                "cancelled fan-out run cannot be continued",
            ));
        }
        if self.phase == FanoutPhase::Completed {
            return Err(refusal(
                "fanout_completed",
                "phase",
                "completed fan-out run cannot be continued",
            ));
        }
        self.admission = FanoutAdmissionState::Open;
        self.updated_at_ms = now_ms;
        self.recompute_phase(now_ms);
        Ok(())
    }

    pub fn cancel(&mut self, now_ms: u64) -> CoreResult<()> {
        if self.phase == FanoutPhase::Completed {
            return Err(refusal(
                "fanout_completed",
                "phase",
                "completed fan-out run cannot be cancelled",
            ));
        }
        if self.admission == FanoutAdmissionState::Cancelled || self.phase == FanoutPhase::Cancelled
        {
            return Err(refusal(
                "fanout_cancelled",
                "admission",
                "fan-out run is already cancelled",
            ));
        }
        self.admission = FanoutAdmissionState::Cancelled;
        if self.stop_reason.is_none() {
            self.stop_reason = Some(STOP_REASON_CANCELLED.to_string());
        }
        for child in &mut self.children {
            if child.status == FanoutChildStatus::Pending {
                child.status = FanoutChildStatus::Skipped;
                child.reason = Some(STOP_REASON_CANCELLED.to_string());
            }
        }
        self.updated_at_ms = now_ms;
        self.recompute_phase(now_ms);
        Ok(())
    }

    pub fn exhaust_budget(&mut self, now_ms: u64) {
        let mut changed = false;
        for child in &mut self.children {
            if child.status == FanoutChildStatus::Pending {
                child.status = FanoutChildStatus::Skipped;
                child.reason = Some(STOP_REASON_BUDGET_EXHAUSTED.to_string());
                changed = true;
            }
        }
        if self.stop_reason.is_none() {
            self.stop_reason = Some(STOP_REASON_BUDGET_EXHAUSTED.to_string());
            changed = true;
        }
        if changed {
            self.updated_at_ms = now_ms;
            self.recompute_phase(now_ms);
        }
    }

    pub fn skip_child(&mut self, index: usize, reason: &str, now_ms: u64) -> CoreResult<()> {
        if reason.is_empty()
            || reason.len() > MAX_FANOUT_REASON_BYTES
            || reason.chars().any(char::is_control)
        {
            return Err(refusal(
                "fanout_child_reason",
                "children.reason",
                "skip reason is empty, oversized, or contains control characters",
            ));
        }
        let child = self.children.get_mut(index).ok_or_else(|| {
            refusal(
                "fanout_child_missing",
                "children",
                "fan-out child index missing",
            )
        })?;
        if child.status != FanoutChildStatus::Pending {
            return Err(refusal(
                "fanout_child_state",
                "children.status",
                "only pending fan-out children can be skipped",
            ));
        }
        child.status = FanoutChildStatus::Skipped;
        child.reason = Some(reason.to_string());
        self.updated_at_ms = now_ms;
        self.recompute_phase(now_ms);
        Ok(())
    }

    pub fn admit_child(&mut self, index: usize, run_id: String, now_ms: u64) -> CoreResult<()> {
        if self.admission != FanoutAdmissionState::Open {
            return Err(refusal(
                "fanout_not_open",
                "admission",
                "fan-out admission is not open",
            ));
        }
        if self.active_count() >= usize::from(self.max_concurrency) {
            return Err(refusal(
                "fanout_concurrency_exceeded",
                "max_concurrency",
                "fan-out active children already at max_concurrency",
            ));
        }
        if run_id.is_empty() {
            return Err(refusal(
                "fanout_child_state",
                "children.run_id",
                "admitted child requires non-empty run_id",
            ));
        }
        let is_pending = self
            .children
            .get(index)
            .is_some_and(|c| c.status == FanoutChildStatus::Pending);
        if !is_pending {
            return Err(refusal(
                "fanout_child_state",
                "children.status",
                "only pending fan-out children can be admitted",
            ));
        }
        self.budget.record_admit()?;
        let child = self.children.get_mut(index).ok_or_else(|| {
            refusal(
                "fanout_child_missing",
                "children",
                "fan-out child index missing",
            )
        })?;
        child.status = FanoutChildStatus::Active;
        child.run_id = Some(run_id);
        child.admitted_at_ms = Some(now_ms);
        self.updated_at_ms = now_ms;
        self.recompute_phase(now_ms);
        Ok(())
    }

    pub fn record_child_outcome(
        &mut self,
        index: usize,
        status: FanoutChildStatus,
        outcome_detail: Option<String>,
        reason: Option<String>,
        now_ms: u64,
    ) -> CoreResult<bool> {
        if !matches!(
            status,
            FanoutChildStatus::Active
                | FanoutChildStatus::Success
                | FanoutChildStatus::Failure
                | FanoutChildStatus::Unknown
        ) {
            return Err(refusal(
                "fanout_child_state",
                "children.status",
                "invalid observed child status transition",
            ));
        }
        let child = self.children.get_mut(index).ok_or_else(|| {
            refusal(
                "fanout_child_missing",
                "children",
                "fan-out child index missing",
            )
        })?;
        if child.status == FanoutChildStatus::Pending || child.status == FanoutChildStatus::Skipped
        {
            return Err(refusal(
                "fanout_child_state",
                "children.status",
                "unadmitted fan-out child cannot receive execution outcomes",
            ));
        }
        if let Some(r) = &reason
            && (r.len() > MAX_FANOUT_REASON_BYTES || r.chars().any(char::is_control))
        {
            return Err(refusal(
                "fanout_child_reason",
                "children.reason",
                "child reason exceeds bound or contains control characters",
            ));
        }
        let completed_at = if matches!(
            status,
            FanoutChildStatus::Success | FanoutChildStatus::Failure
        ) {
            child.completed_at_ms.or(Some(now_ms))
        } else {
            None
        };
        if child.status == status
            && child.outcome_detail == outcome_detail
            && child.reason == reason
            && child.completed_at_ms == completed_at
        {
            return Ok(false);
        }
        child.status = status;
        child.outcome_detail = outcome_detail;
        child.reason = reason;
        child.completed_at_ms = completed_at;
        self.updated_at_ms = now_ms;
        self.recompute_phase(now_ms);
        Ok(true)
    }
}

/// Validates a fan-out target glob selector pattern (the portion before `@REF`).
#[must_use]
pub fn valid_target_glob(pattern: &str) -> bool {
    !pattern.is_empty()
        && pattern.len() <= 256
        && pattern.bytes().all(|byte| byte.is_ascii_graphic())
        && !pattern.contains('@')
        && valid_pattern(pattern)
}

/// Matches a fan-out target glob pattern against `repository` (if pattern
/// contains `/`, case-insensitively) or `project_id` (if pattern has no `/`).
#[must_use]
pub fn glob_matches(pattern: &str, repository: &str, project_id: &str) -> bool {
    if !valid_target_glob(pattern) {
        return false;
    }
    if pattern.contains('/') {
        let pat_lower = pattern.to_ascii_lowercase();
        let repo_lower = repository.to_ascii_lowercase();
        path_matches(&pat_lower, &repo_lower)
    } else {
        path_matches(pattern, project_id)
    }
}

/// Checks if `scope` overlaps any live fleet claim at `now_ms`.
#[must_use]
pub fn find_claim_overlap(
    scope: &FleetScope,
    claims: &[FleetClaim],
    now_ms: u64,
) -> Option<ClaimConflict> {
    claims
        .iter()
        .find(|claim| claim.is_live(now_ms) && overlaps(&claim.scope, scope))
        .map(|claim| ClaimConflict::Claim {
            identity: claim.identity.clone(),
            holder: claim.holder.label(),
            scope: claim.scope.text(),
        })
}

fn is_hex40(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::{
        ClaimState, FleetAgent, MirrorStatus, ScopePath, parse_scope, parse_scope_text,
    };
    use crate::source::{GitObjectFormat, GitResolutionFacts, SourceNativeFacts};
    use crate::types::{ContentId, ExternalRef, ProviderProfileId};
    use alloc::vec;

    fn external(ns: &str, kind: &str, id: &str) -> ExternalRef {
        ExternalRef::new(ns, kind, id).unwrap_or_else(|_| unreachable!())
    }

    fn sample_source(repo: &str, rev: &str) -> SourceResolution {
        let exact = external("git", "revision", rev);
        let sym = external("git", "reference", "refs/heads/main");
        let repository = external("git", "repository", repo);
        let tree = ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|_| unreachable!());
        SourceResolution {
            source_provider: ProviderProfileId::new("web-git-fixture")
                .unwrap_or_else(|_| unreachable!()),
            repository: repository.clone(),
            symbolic_reference: sym.clone(),
            exact_revision: exact.clone(),
            canonical_tree: tree.clone(),
            native_facts: SourceNativeFacts::Git(GitResolutionFacts {
                repository,
                symbolic_reference: sym,
                exact_commit: exact.clone(),
                native_tree: external("git", "tree", &"b".repeat(40)),
                observed_ref_before: exact.clone(),
                observed_ref_after: exact,
                object_format: GitObjectFormat::Sha1,
                partial_repository: false,
                replacement_objects: false,
                alternate_object_database: false,
                signature_observed: false,
            }),
        }
    }

    fn sample_child(index: u16, project_id: &str, repo: &str, mutating: bool) -> FanoutChild {
        let rev = "0123456789abcdef0123456789abcdef01234567";
        FanoutChild {
            index,
            target: alloc::format!("{repo}@refs/heads/main"),
            repository: repo.to_string(),
            project_id: project_id.to_string(),
            resolved_reference: "refs/heads/main".to_string(),
            source_revision: rev.to_string(),
            source: sample_source(repo, rev),
            write_scope: mutating
                .then(|| parse_scope(repo, ".").unwrap_or_else(|_| unreachable!())),
            status: FanoutChildStatus::Pending,
            run_id: None,
            outcome_detail: None,
            reason: None,
            admitted_at_ms: None,
            completed_at_ms: None,
        }
    }

    fn sample_fanout(count: u16, max_concurrency: u16, ceiling_units: u64) -> FanOutRun {
        let children = (0..count)
            .map(|i| {
                sample_child(
                    i,
                    &alloc::format!("proj-{i}"),
                    &alloc::format!("onixresearch/repo-{i}"),
                    true,
                )
            })
            .collect();
        FanOutRun {
            schema: FANOUT_RUN_SCHEMA.to_string(),
            id: "web-fanout-1".to_string(),
            ordinal: 1,
            runbook_id: "flake-update".to_string(),
            mode: FanoutMode::Mutating,
            principal_id: "local:operator".to_string(),
            request_id: "req-1".to_string(),
            target_selectors: vec!["onixresearch/*@refs/heads/main".to_string()],
            max_concurrency,
            budget: FanoutBudget::new(ceiling_units, 1).unwrap_or_else(|_| unreachable!()),
            claim_policy: FanoutClaimPolicy::Refuse,
            admission: FanoutAdmissionState::Open,
            phase: FanoutPhase::Active,
            stop_reason: None,
            lattice_bundle_id: None,
            lattice_bundle_identity: "a".repeat(64),
            lattice_workflow_name: "flake-update".to_string(),
            lattice_workflow_revision: "b".repeat(64),
            params_identity: "c".repeat(64),
            params: BTreeMap::new(),
            env_bindings: BTreeMap::new(),
            children,
            created_at_ms: 100,
            updated_at_ms: 100,
            completed_at_ms: None,
        }
    }

    #[test]
    fn glob_matching_enforces_pattern_safety_and_case_insensitive_repos() {
        assert!(valid_target_glob("onixresearch/*"));
        assert!(valid_target_glob("onixresearch/kiln-*"));
        assert!(valid_target_glob("kiln-?"));
        assert!(!valid_target_glob(""));
        assert!(!valid_target_glob("onixresearch/*@main"));
        assert!(!valid_target_glob("../escape"));
        assert!(!valid_target_glob("onixresearch/[a-z]"));

        assert!(glob_matches(
            "OnixResearch/kiln-*",
            "onixresearch/kiln-web",
            "kiln-web"
        ));
        assert!(glob_matches(
            "onixresearch/*",
            "OnixResearch/lattice",
            "lattice"
        ));
        assert!(!glob_matches(
            "onixresearch/*",
            "other/lattice",
            "onixresearch-lattice"
        ));
        assert!(glob_matches("kiln-*", "onixresearch/kiln", "kiln-audit"));
        assert!(!glob_matches("kiln-*", "onixresearch/kiln", "other-audit"));
    }

    #[test]
    fn concurrency_budget_and_unknown_isolation() {
        let mut run = sample_fanout(4, 2, 3);
        assert!(run.validate().is_ok());

        // Admit up to max_concurrency = 2.
        assert!(run.admit_child(0, "run-0".into(), 101).is_ok());
        assert!(run.admit_child(1, "run-1".into(), 102).is_ok());
        assert_eq!(run.active_count(), 2);
        // Third admission blocked by max_concurrency.
        assert!(run.admit_child(2, "run-2".into(), 103).is_err());

        // Child 0 becomes Unknown, child 1 Fails: both release concurrency slots
        // without blocking or cancelling siblings.
        assert_eq!(
            run.record_child_outcome(
                0,
                FanoutChildStatus::Unknown,
                Some("unknown".into()),
                Some("provider_unknown".into()),
                104,
            ),
            Ok(true)
        );
        assert_eq!(
            run.record_child_outcome(
                1,
                FanoutChildStatus::Failure,
                Some("domain_failure".into()),
                None,
                105,
            ),
            Ok(true)
        );
        assert_eq!(run.active_count(), 0);

        // Child 2 can now be admitted (consumes 3rd and final budget unit).
        assert!(run.admit_child(2, "run-2".into(), 106).is_ok());
        assert!(!run.budget.can_admit());
        // Child 3 cannot be admitted; budget exhaustion skips remaining pending.
        assert!(run.admit_child(3, "run-3".into(), 107).is_err());
        run.exhaust_budget(107);
        assert_eq!(
            run.children.get(3).map(|c| c.status),
            Some(FanoutChildStatus::Skipped)
        );
        assert_eq!(
            run.children.get(3).and_then(|c| c.reason.as_deref()),
            Some(STOP_REASON_BUDGET_EXHAUSTED)
        );
        assert_eq!(
            run.stop_reason.as_deref(),
            Some(STOP_REASON_BUDGET_EXHAUSTED)
        );
        assert_eq!(run.phase, FanoutPhase::Active);

        // Child 2 completes and Unknown child 0 later reconciles to Success.
        assert_eq!(
            run.record_child_outcome(
                2,
                FanoutChildStatus::Success,
                Some("success".into()),
                None,
                108
            ),
            Ok(true)
        );
        assert_eq!(run.phase, FanoutPhase::Completed);
        assert_eq!(
            run.record_child_outcome(
                0,
                FanoutChildStatus::Success,
                Some("success".into()),
                None,
                109
            ),
            Ok(true)
        );
        let counts = run.counts();
        assert_eq!(
            counts,
            FanoutCounts {
                total: 4,
                pending: 0,
                active: 0,
                success: 2,
                failure: 1,
                unknown: 0,
                skipped: 1,
            }
        );
        assert!(run.validate().is_ok());
    }

    #[test]
    fn pause_continue_and_cancel_are_admission_only() {
        let mut run = sample_fanout(3, 1, 10);
        assert!(run.admit_child(0, "run-0".into(), 101).is_ok());

        assert!(run.pause(102).is_ok());
        assert_eq!(run.admission, FanoutAdmissionState::Paused);
        assert_eq!(run.phase, FanoutPhase::Paused);
        // Active child 0 remains active and cannot admit child 1 while paused.
        assert_eq!(
            run.children.first().map(|c| c.status),
            Some(FanoutChildStatus::Active)
        );
        assert!(run.admit_child(1, "run-1".into(), 103).is_err());

        assert!(run.continue_admission(104).is_ok());
        assert_eq!(run.admission, FanoutAdmissionState::Open);
        assert_eq!(run.phase, FanoutPhase::Active);

        // Cancel marks pending children Skipped, keeps active child 0 Active.
        assert!(run.cancel(105).is_ok());
        assert_eq!(run.admission, FanoutAdmissionState::Cancelled);
        assert_eq!(run.phase, FanoutPhase::Active);
        assert_eq!(
            run.children.first().map(|c| c.status),
            Some(FanoutChildStatus::Active)
        );
        assert_eq!(
            run.children.get(1).map(|c| c.status),
            Some(FanoutChildStatus::Skipped)
        );
        assert_eq!(
            run.children.get(2).map(|c| c.status),
            Some(FanoutChildStatus::Skipped)
        );
        assert_eq!(
            run.children.get(1).and_then(|c| c.reason.as_deref()),
            Some(STOP_REASON_CANCELLED)
        );

        // Cannot continue or pause after cancellation.
        assert!(run.continue_admission(106).is_err());
        assert!(run.pause(106).is_err());

        // Active child finishes -> phase becomes Cancelled.
        assert_eq!(
            run.record_child_outcome(
                0,
                FanoutChildStatus::Success,
                Some("success".into()),
                None,
                107
            ),
            Ok(true)
        );
        assert_eq!(run.phase, FanoutPhase::Cancelled);
        assert_eq!(run.completed_at_ms, Some(107));
    }

    #[test]
    fn find_claim_overlap_checks_live_unexpired_claims() {
        let claim_scope = parse_scope_text("OnixResearch/kiln crates/kiln-core")
            .unwrap_or_else(|_| unreachable!());
        let claim = FleetClaim {
            identity: ContentId::from_blake3_hex("d".repeat(64)).unwrap_or_else(|_| unreachable!()),
            scope: claim_scope,
            holder: FleetAgent {
                tool: "codex".into(),
                host: "aspen3".into(),
                session: "s1".into(),
            },
            purpose: "edit".into(),
            fence: 1,
            granted_ms: 100,
            expires_ms: 500,
            state: ClaimState::Open,
            attempt: None,
            claim_mirror: MirrorStatus::Posted { message_id: None },
            release_mirror: MirrorStatus::Pending,
        };
        let overlapping =
            parse_scope_text("onixresearch/kiln .").unwrap_or_else(|_| unreachable!());
        let disjoint =
            parse_scope_text("onixresearch/kiln docs").unwrap_or_else(|_| unreachable!());

        assert!(find_claim_overlap(&overlapping, core::slice::from_ref(&claim), 200).is_some());
        assert!(find_claim_overlap(&disjoint, core::slice::from_ref(&claim), 200).is_none());
        // Expired at now_ms = 500 -> no overlap.
        assert!(find_claim_overlap(&overlapping, core::slice::from_ref(&claim), 500).is_none());
    }

    #[test]
    fn validate_rejects_mutating_child_without_write_scope() {
        let mut run = sample_fanout(1, 1, 1);
        if let Some(child) = run.children.first_mut() {
            child.write_scope = None;
        }
        assert!(run.validate().is_err());

        let mut ro_run = sample_fanout(1, 1, 1);
        ro_run.mode = FanoutMode::ReadOnly;
        assert!(ro_run.validate().is_err());

        let _ = ScopePath::Whole;
    }
}
