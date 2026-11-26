//! Authenticated read-only projections over explicitly supplied observations.
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use kiln_core::fleet::{
    Decision as FleetDecision, FleetClaim, FleetScope, is_stale_observation_decision, overlaps,
};
use kiln_core::implementation::campaign::{
    CampaignOperation, CampaignState, DecisionRequestState, EffectState, MAX_ID_BYTES,
    MemberSelector, selector_key,
};
use kiln_core::implementation::continuation::{ContinuationRecord, WakeHint, WakeKind};
use kiln_core::implementation::{
    BlockReason, Eligibility, Enrollment, FailureClass, PlanDecision, PlanGate, PlanReviewCommand,
    PlanReviewResult, PlanVerdict, WorkSourceKind,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::Implementer;
use kiln_implementer::campaign::{CampaignObservation, CampaignStore};
use kiln_implementer::observation::{CanonicalSnapshot, ImplementationObservation};
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{ApiResponse, Context, Principal, Request, error_response};
use crate::cairn_workspace::agents::InventoryObservation;
use crate::engine::Role;
use crate::engine::model::RunView;

const MAXIMUM_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const CONTENT_ID_HEX_BYTES: usize = blake3::OUT_LEN * 2;
const CAMPAIGN_INDEX_SCHEMA: &str = "kiln.implementation-campaigns.v1";
const MAX_CAMPAIGN_INDEX_SCAN: usize = 1_000;
const MAX_CAMPAIGN_INDEX_ENTRIES: usize = 200;
const MAX_CAMPAIGN_ROW_BYTES: i64 = 32 * 1024 * 1024;
const MAX_CONTINUATION_ROW_BYTES: i64 = 64 * 1024;
const CAMPAIGN_BUSY_TIMEOUT_MS: u64 = 10_000;
const ATTENTION_SCHEMA: &str = "kiln.implementation-attention.v1";
const WAKE_SCHEMA: &str = "kiln.campaign-wake-result.v1";
const RECONCILE_TERMINATION_SCHEMA: &str = "kiln.termination-reconciliation-result.v1";
const PLAN_REVIEW_SCHEMA: &str = "kiln.plan-review-result.v1";
const MAX_ATTENTION_ITEMS: usize = 200;
const CLAIM_EXPIRING_WINDOW_MS: u64 = 15 * 60_000;
const CLAIM_CRITICAL_WINDOW_MS: u64 = 5 * 60_000;
const MAX_REASON_BYTES: usize = 1024;
const MAX_PLAN_STEPS_PREVIEW: usize = 64;

type AdmissionFailure = (u16, &'static str);

fn admit_request(
    method: &str,
    role: Role,
    target: &str,
) -> Result<Option<ContentId>, AdmissionFailure> {
    if role == Role::Worker {
        return Err((403, "inventory_forbidden"));
    }
    if method != "GET" {
        return Err((405, "inventory_read_only"));
    }
    let Some((_, query)) = target.split_once('?') else {
        return Ok(None);
    };
    let value = query
        .strip_prefix("work=")
        .filter(|value| {
            value.len() == CONTENT_ID_HEX_BYTES
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or((400, "inventory_work_query_invalid"))?;
    ContentId::from_blake3_hex(value)
        .map(Some)
        .map_err(|_| (400, "inventory_work_query_invalid"))
}

fn admit_campaign_request(
    method: &str,
    role: Role,
    target: &str,
) -> Result<Option<String>, AdmissionFailure> {
    if role == Role::Worker {
        return Err((403, "inventory_forbidden"));
    }
    if method != "GET" {
        return Err((405, "inventory_read_only"));
    }
    let Some((_, query)) = target.split_once('?') else {
        return Ok(None);
    };
    // Each percent-encoded UTF-8 byte occupies three query bytes (%HH).
    let value = query
        .strip_prefix("campaign=")
        .filter(|value| {
            value.len() <= kiln_core::implementation::campaign::MAX_ID_BYTES * 3
                && !value.contains('&')
        })
        .ok_or((400, "campaign_projection_query_invalid"))?;
    let identity = crate::engine::api::decoded(value)
        .map_err(|_| (400, "campaign_projection_query_invalid"))?;
    if identity.trim().is_empty()
        || identity.len() > kiln_core::implementation::campaign::MAX_ID_BYTES
        || identity.chars().any(char::is_control)
    {
        return Err((400, "campaign_projection_query_invalid"));
    }
    Ok(Some(identity))
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
struct CampaignSummary {
    campaign_id: String,
    status: &'static str,
    store_version: u64,
    revision: u64,
    repository: String,
    repository_identity: String,
    member_count: usize,
    pending_decisions: usize,
    unissued_decisions: usize,
    pending_effects: usize,
    paused: bool,
    cancelled: bool,
    complete: bool,
    updated_at: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
struct CampaignIndex {
    schema: &'static str,
    campaigns: Vec<CampaignSummary>,
    bounded: bool,
    truncated: bool,
}

fn campaign_status(state: &CampaignState, complete: bool) -> &'static str {
    if state.cancelled {
        "cancelled"
    } else if complete {
        "complete"
    } else if state.paused {
        "paused"
    } else if state.pending_effects.values().any(|effect| {
        !effect.progress_applied
            && matches!(effect.state, EffectState::IntentPersisted | EffectState::Unknown)
    }) {
        "reconciling"
    } else {
        match state.evaluate().decision.operation {
            CampaignOperation::Blocked => "blocked",
            CampaignOperation::Wait => "waiting",
            _ => "active",
        }
    }
}

fn summarize_campaign(
    state: &CampaignState,
    store_version: u64,
    last_batch_ms: Option<u64>,
) -> CampaignSummary {
    let complete = state.complete();
    let pending_decisions = state
        .decisions
        .values()
        .filter(|decision| decision.state == DecisionRequestState::Outstanding)
        .count();
    let unissued_decisions = state
        .decisions
        .values()
        .filter(|decision| decision.state == DecisionRequestState::Unissued)
        .count();
    let pending_effects = state
        .pending_effects
        .values()
        .filter(|effect| !effect.progress_applied)
        .count();
    let updated_at = last_batch_ms
        .into_iter()
        .chain(state.decisions.values().map(|decision| decision.created_ms))
        .max();
    CampaignSummary {
        campaign_id: state.contract.campaign_id.clone(),
        status: campaign_status(state, complete),
        store_version,
        revision: state.contract.revision,
        repository: state.contract.repository.clone(),
        repository_identity: crate::inventory::campaign_projection::repository_identity(
            &state.contract.repository,
        ),
        member_count: state.contract.members.len(),
        pending_decisions,
        unissued_decisions,
        pending_effects,
        paused: state.paused,
        cancelled: state.cancelled,
        complete,
        updated_at,
    }
}

fn build_campaign_index(mut campaigns: Vec<CampaignSummary>, overflow: bool) -> CampaignIndex {
    campaigns.sort_by(|left, right| {
        let left_done = u8::from(left.complete || left.cancelled);
        let right_done = u8::from(right.complete || right.cancelled);
        left_done
            .cmp(&right_done)
            .then_with(|| Reverse(left.updated_at).cmp(&Reverse(right.updated_at)))
            .then_with(|| left.campaign_id.cmp(&right.campaign_id))
    });
    let truncated = overflow || campaigns.len() > MAX_CAMPAIGN_INDEX_ENTRIES;
    campaigns.truncate(MAX_CAMPAIGN_INDEX_ENTRIES);
    CampaignIndex {
        schema: CAMPAIGN_INDEX_SCHEMA,
        campaigns,
        bounded: true,
        truncated,
    }
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, &'static str> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            params![table],
            |row| row.get::<_, i64>(0),
        )
        .map(|exists| exists != 0)
        .map_err(|_| "campaign_projection_unavailable")
}

fn read_continuation_times(
    connection: &Connection,
) -> Result<BTreeMap<String, Option<u64>>, &'static str> {
    if !table_exists(connection, "implementation_campaign_continuation")? {
        return Ok(BTreeMap::new());
    }
    let mut statement = connection
        .prepare(
            "SELECT campaign_id, CASE WHEN length(bytes) <= ?1 THEN bytes ELSE NULL END \
             FROM implementation_campaign_continuation ORDER BY campaign_id LIMIT ?2",
        )
        .map_err(|_| "campaign_projection_unavailable")?;
    let rows = statement
        .query_map(
            params![
                MAX_CONTINUATION_ROW_BYTES,
                i64::try_from(MAX_CAMPAIGN_INDEX_SCAN + 1).unwrap_or(i64::MAX)
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<Vec<u8>>>(1)?)),
        )
        .map_err(|_| "campaign_projection_unavailable")?;
    let mut times = BTreeMap::new();
    for row in rows {
        let (campaign_id, bytes) = row.map_err(|_| "campaign_projection_unavailable")?;
        let Some(bytes) = bytes else {
            return Err("campaign_projection_record_invalid");
        };
        let record: ContinuationRecord =
            serde_json::from_slice(&bytes).map_err(|_| "campaign_projection_record_invalid")?;
        if record.campaign_id != campaign_id {
            return Err("campaign_projection_record_invalid");
        }
        times.insert(campaign_id, record.last_batch_ms);
    }
    Ok(times)
}

fn read_campaign_index(database: &Path) -> Result<CampaignIndex, &'static str> {
    let connection = Connection::open_with_flags(
        database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "campaign_projection_unavailable")?;
    connection
        .busy_timeout(Duration::from_millis(CAMPAIGN_BUSY_TIMEOUT_MS))
        .map_err(|_| "campaign_projection_unavailable")?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|_| "campaign_projection_unavailable")?;
    if !table_exists(&connection, "implementation_campaign")? {
        return Ok(CampaignIndex {
            schema: CAMPAIGN_INDEX_SCHEMA,
            campaigns: Vec::new(),
            bounded: true,
            truncated: false,
        });
    }
    let continuation_times = read_continuation_times(&connection)?;
    let scan_limit = i64::try_from(MAX_CAMPAIGN_INDEX_SCAN + 1).unwrap_or(i64::MAX);
    let mut statement = connection
        .prepare(
            "SELECT campaign_id, version, length(bytes), \
             CASE WHEN length(bytes) <= ?1 THEN bytes ELSE NULL END \
             FROM implementation_campaign ORDER BY campaign_id LIMIT ?2",
        )
        .map_err(|_| "campaign_projection_unavailable")?;
    let rows = statement
        .query_map(params![MAX_CAMPAIGN_ROW_BYTES, scan_limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?,
            ))
        })
        .map_err(|_| "campaign_projection_unavailable")?;
    let mut summaries = Vec::new();
    let mut overflow = false;
    for row in rows {
        if summaries.len() >= MAX_CAMPAIGN_INDEX_SCAN {
            overflow = true;
            break;
        }
        let (campaign_id, version, length, bytes) =
            row.map_err(|_| "campaign_projection_unavailable")?;
        if !(0..=MAX_CAMPAIGN_ROW_BYTES).contains(&length) {
            return Err("campaign_projection_record_invalid");
        }
        let Some(bytes) = bytes else {
            return Err("campaign_projection_record_invalid");
        };
        let store_version =
            u64::try_from(version).map_err(|_| "campaign_projection_record_invalid")?;
        let state: CampaignState =
            serde_json::from_slice(&bytes).map_err(|_| "campaign_projection_record_invalid")?;
        if state.contract.campaign_id != campaign_id {
            return Err("campaign_projection_record_invalid");
        }
        let last_batch_ms = continuation_times.get(&campaign_id).copied().flatten();
        summaries.push(summarize_campaign(&state, store_version, last_batch_ms));
    }
    Ok(build_campaign_index(summaries, overflow))
}

// r[impl kiln.dashboard.campaign.projection]
// r[impl kiln.implementation.campaign.index_route]
pub(super) fn campaign_response(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
) -> ApiResponse {
    let identity = match admit_campaign_request(&request.method, principal.role, &request.target) {
        Ok(identity) => identity,
        Err((status, code)) => return error_response(status, code),
    };
    let Some(database) = context.implementation_work else {
        return error_response(503, "campaign_projection_input_not_configured");
    };
    let Some(identity) = identity else {
        let index = match read_campaign_index(database) {
            Ok(index) => index,
            Err(code) => return error_response(503, code),
        };
        return match serde_json::to_vec(&index) {
            Ok(body) if body.len() <= MAXIMUM_RESPONSE_BYTES => ApiResponse {
                status: 200,
                content_type: "application/json",
                body,
                download_name: None,
                headers: Vec::new(),
            },
            _ => error_response(503, "campaign_projection_byte_bound"),
        };
    };
    let projection = match crate::inventory::read_campaign_projection(database, &identity) {
        Ok(Some(projection)) => projection,
        Ok(None) => return error_response(404, "campaign_projection_not_found"),
        Err(code) => return error_response(503, &code),
    };
    // Resource observations have an independent authority/failure boundary.
    let inventory = context.inventory.and_then(|path| {
        crate::engine::host_time_ms()
            .ok()
            .and_then(|now| crate::inventory::read_inventory(path, &principal.id, now).ok())
    });
    let projection = match crate::inventory::campaign_projection::compose_resource_inventory(
        projection,
        inventory.as_ref(),
    ) {
        Ok(projection) => projection,
        Err(code) => return error_response(503, &code),
    };
    match serde_json::to_vec(&projection) {
        Ok(body) if body.len() <= MAXIMUM_RESPONSE_BYTES => ApiResponse {
            status: 200,
            content_type: "application/json",
            body,
            download_name: None,
            headers: Vec::new(),
        },
        _ => error_response(503, "campaign_projection_byte_bound"),
    }
}

// r[impl kiln.cairn.workspace.agents.scenario.source]
/// The resource inventory beside a Cairn change detail, for the requesting
/// principal and the host clock. Its failure never fails the detail.
pub(super) fn cairn_observation(
    context: &Context<'_>,
    principal: &Principal,
) -> InventoryObservation {
    let Some(path) = context.inventory else {
        return InventoryObservation::NotConfigured;
    };
    let Ok(now) = crate::engine::host_time_ms() else {
        return InventoryObservation::Unavailable("clock_unavailable".into());
    };
    match crate::inventory::read_inventory(path, &principal.id, now) {
        Ok(projection) => InventoryObservation::Admitted(Box::new(projection)),
        Err(code) => InventoryObservation::Unavailable(code),
    }
}

// r[impl kiln.inventory.consumers]
pub(super) fn response(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
) -> ApiResponse {
    let work = match admit_request(&request.method, principal.role, &request.target) {
        Ok(work) => work,
        Err((status, code)) => return error_response(status, code),
    };
    let Some(path) = context.inventory else {
        return error_response(503, "inventory_input_not_configured");
    };
    let now = match crate::engine::host_time_ms() {
        Ok(now) => now,
        Err(_) => return error_response(503, "clock_unavailable"),
    };
    let projection = if let Some(work) = &work {
        let Some(database) = context.implementation_work else {
            return error_response(503, "inventory_work_source_not_configured");
        };
        let observation = match ImplementationObservation::open(database) {
            Ok(observation) => observation,
            Err(_) => return error_response(503, "inventory_current_work_unavailable"),
        };
        crate::inventory::read_projection(path, &principal.id, Some(work), &observation, now)
    } else {
        crate::inventory::read_inventory(path, &principal.id, now)
    };
    let projection = match projection {
        Ok(projection) => projection,
        Err(code) => {
            let status = match code.as_str() {
                "inventory_principal_denied" => 403,
                "inventory_work_stale" => 409,
                _ => 503,
            };
            return error_response(status, &code);
        }
    };
    match serde_json::to_vec(&projection) {
        Ok(body) if body.len() <= MAXIMUM_RESPONSE_BYTES => ApiResponse {
            status: 200,
            content_type: "application/json",
            body,
            download_name: None,
            headers: Vec::new(),
        },
        _ => error_response(503, "inventory_projection_byte_bound"),
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(super) enum AttentionKind {
    UntrustedRunApproval,
    FleetSessionDecision,
    CampaignDecision,
    PlanReviewGate,
    UnknownTerminationOrDispatch,
    ExpiringOrConflictClaim,
}

impl AttentionKind {
    const fn rank(self) -> u8 {
        match self {
            Self::UnknownTerminationOrDispatch => 0,
            Self::UntrustedRunApproval => 1,
            Self::FleetSessionDecision => 2,
            Self::CampaignDecision => 3,
            Self::PlanReviewGate => 4,
            Self::ExpiringOrConflictClaim => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(super) enum AttentionSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl AttentionSeverity {
    const fn rank(self) -> u8 {
        match self {
            Self::Critical => 0,
            Self::High => 1,
            Self::Medium => 2,
            Self::Low => 3,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub(super) struct AttentionTarget {
    pub run_id: Option<String>,
    pub campaign_id: Option<String>,
    pub cairn_change: Option<String>,
    pub work_id: Option<String>,
    pub attempt_id: Option<String>,
    pub fence: Option<u64>,
    pub claim_id: Option<String>,
    pub decision_id: Option<String>,
    pub plan_id: Option<String>,
    pub machine: Option<String>,
    pub scope: Option<String>,
    pub source_commit: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct UnifiedAttentionItem {
    pub id: String,
    pub kind: AttentionKind,
    pub severity: AttentionSeverity,
    pub title: String,
    pub target: AttentionTarget,
    pub context_preview: Value,
    pub age_ms: Option<u64>,
    pub ttl_remaining_ms: Option<u64>,
    pub allowed_actions: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct SourceHealth {
    pub configured: bool,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct AttentionSources {
    pub ci_engine: SourceHealth,
    pub implementation_work: SourceHealth,
    pub fleet: SourceHealth,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub(super) struct AttentionCounts {
    #[serde(rename = "UntrustedRunApproval")]
    pub untrusted_run_approval: usize,
    #[serde(rename = "FleetSessionDecision")]
    pub fleet_session_decision: usize,
    #[serde(rename = "CampaignDecision")]
    pub campaign_decision: usize,
    #[serde(rename = "PlanReviewGate")]
    pub plan_review_gate: usize,
    #[serde(rename = "UnknownTerminationOrDispatch")]
    pub unknown_termination_or_dispatch: usize,
    #[serde(rename = "ExpiringOrConflictClaim")]
    pub expiring_or_conflict_claim: usize,
}

impl AttentionCounts {
    fn record(&mut self, kind: AttentionKind) {
        match kind {
            AttentionKind::UntrustedRunApproval => self.untrusted_run_approval += 1,
            AttentionKind::FleetSessionDecision => self.fleet_session_decision += 1,
            AttentionKind::CampaignDecision => self.campaign_decision += 1,
            AttentionKind::PlanReviewGate => self.plan_review_gate += 1,
            AttentionKind::UnknownTerminationOrDispatch => {
                self.unknown_termination_or_dispatch += 1
            }
            AttentionKind::ExpiringOrConflictClaim => self.expiring_or_conflict_claim += 1,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct AttentionProjection {
    pub schema: &'static str,
    pub generated_at_ms: u64,
    pub items: Vec<UnifiedAttentionItem>,
    pub total: usize,
    pub bounded: bool,
    pub truncated: bool,
    pub counts: AttentionCounts,
    pub sources: AttentionSources,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ForeignClaimView {
    pub scope: FleetScope,
    #[serde(default)]
    pub scope_text: String,
    pub message_id: String,
    pub author: String,
    #[serde(default)]
    pub holder: String,
    pub date_unix_ms: u64,
    #[serde(default)]
    pub age_ms: u64,
    #[serde(default)]
    pub stale_after_ms: u64,
}

#[derive(Debug)]
pub(super) struct CiAttentionInput {
    pub health: SourceHealth,
    pub runs: Vec<RunView>,
    pub repositories: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ImplAttentionInput {
    pub health: SourceHealth,
    pub snapshot: Option<CanonicalSnapshot>,
}

#[derive(Clone, Debug)]
pub(super) struct FleetAttemptHint {
    pub machine: Option<String>,
    pub session_state: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct FleetAttentionInput {
    pub health: SourceHealth,
    pub decisions: Vec<FleetDecision>,
    pub claims: Vec<FleetClaim>,
    pub foreign_claims: Vec<ForeignClaimView>,
    pub attempt_hints: BTreeMap<String, FleetAttemptHint>,
}

fn cairn_change_for_work(work: &kiln_core::implementation::ImplementationWork) -> Option<String> {
    work.aliases
        .iter()
        .filter(|alias| {
            matches!(
                alias.kind,
                WorkSourceKind::CairnTask | WorkSourceKind::CairnArtifact
            )
        })
        .find_map(|alias| {
            alias
                .native_id
                .split_once('/')
                .filter(|(change, unit)| !change.is_empty() && !unit.is_empty())
                .map(|(change, _)| change.to_owned())
        })
}

fn selector_change(selector: &MemberSelector) -> &str {
    match selector {
        MemberSelector::WholeChange { change } | MemberSelector::Tasks { change, .. } => change,
    }
}

fn campaign_for_work(
    campaigns: &[(CampaignState, u64)],
    work_id: &ContentId,
    attempt_id: Option<&ContentId>,
    repository: Option<&str>,
    cairn_change: Option<&str>,
) -> Option<String> {
    for (state, _) in campaigns {
        if state.observations.values().any(|obs| {
            obs.work_identity.as_ref() == Some(work_id)
                || attempt_id.is_some_and(|aid| obs.attempt_identity.as_ref() == Some(aid))
        }) {
            return Some(state.contract.campaign_id.clone());
        }
        if state.pending_effects.values().any(|eff| {
            eff.work_identity.as_ref() == Some(work_id)
                || attempt_id.is_some_and(|aid| eff.attempt_identity.as_ref() == Some(aid))
        }) {
            return Some(state.contract.campaign_id.clone());
        }
    }
    if let (Some(repo), Some(change)) = (repository, cairn_change) {
        for (state, _) in campaigns {
            if state.contract.repository == repo
                && state
                    .contract
                    .members
                    .iter()
                    .any(|member| selector_change(&member.selector) == change)
            {
                return Some(state.contract.campaign_id.clone());
            }
        }
    }
    None
}

fn member_cairn_change(state: &CampaignState, member_key: &str) -> Option<String> {
    state
        .contract
        .members
        .iter()
        .find(|binding| selector_key(&binding.selector) == member_key)
        .map(|binding| selector_change(&binding.selector).to_owned())
        .or_else(|| {
            member_key
                .strip_prefix("change:")
                .map(str::to_owned)
                .or_else(|| {
                    member_key
                        .strip_prefix("tasks:")
                        .and_then(|rest| rest.split_once(':').map(|(c, _)| c.to_owned()))
                })
        })
}

fn repository_matches_scope_target(repository: &str, scope_target: &str) -> bool {
    let norm = |s: &str| s.trim().trim_end_matches(".git").to_ascii_lowercase();
    let repo = norm(repository);
    let target = norm(scope_target);
    repo == target || repo.ends_with(&format!("/{target}"))
}

// r[impl kiln.implementation.campaign.unified_attention_and_recovery]
/// Pure functional aggregator for all human-required escalations across CI,
/// implementation work / campaigns / Cairn plans, and the Fleet coordinator.
pub(super) fn assemble_attention(
    now_ms: u64,
    ci: CiAttentionInput,
    impl_work: ImplAttentionInput,
    fleet: FleetAttentionInput,
) -> AttentionProjection {
    let mut items = Vec::new();
    let mut known_repositories: BTreeSet<String> = ci.repositories.iter().cloned().collect();

    // 1. CI Engine: Untrusted runs awaiting approval + Unknown runs.
    for run in &ci.runs {
        if run.phase == crate::engine::untrusted::AWAITING_APPROVAL {
            let age_ms = run
                .created_at_ms
                .map(|created| now_ms.saturating_sub(created));
            let pr = run.untrusted.as_ref().map(|u| u.pull_request);
            let reason = run.untrusted.as_ref().map(|u| u.reason);
            let head_repo = run
                .untrusted
                .as_ref()
                .map(|u| u.head_repository.as_str())
                .unwrap_or("");
            let satisfies_required_ci = run
                .untrusted
                .as_ref()
                .is_some_and(|u| u.counts_for_required_ci);
            items.push(UnifiedAttentionItem {
                id: format!("untrusted_run:{}", run.id),
                kind: AttentionKind::UntrustedRunApproval,
                severity: AttentionSeverity::High,
                title: match pr {
                    Some(pr) => format!("Approve untrusted PR #{pr} run {}", run.id),
                    None => format!("Approve untrusted run {}", run.id),
                },
                target: AttentionTarget {
                    run_id: Some(run.id.clone()),
                    attempt_id: Some(run.attempt_id.as_str().to_owned()),
                    source_commit: Some(run.source_revision.clone()),
                    ..AttentionTarget::default()
                },
                context_preview: json!({
                    "summary": format!(
                        "Untrusted run on project {} ({}) awaiting operator approval for commit {}.",
                        run.project_id, head_repo, run.source_revision
                    ),
                    "project_id": run.project_id,
                    "pull_request": pr,
                    "head_repository": head_repo,
                    "source_commit": run.source_revision,
                    "reason": reason,
                    "satisfies_required_ci": satisfies_required_ci,
                }),
                age_ms,
                ttl_remaining_ms: None,
                allowed_actions: vec!["approve_untrusted", "inspect", "open_run"],
            });
        } else if run.outcome == kiln_core::domain::Outcome::Unknown || run.phase == "unknown" {
            let age_ms = run
                .started_at_ms
                .or(run.created_at_ms)
                .map(|started| now_ms.saturating_sub(started));
            items.push(UnifiedAttentionItem {
                id: format!("unknown_run:{}", run.id),
                kind: AttentionKind::UnknownTerminationOrDispatch,
                severity: AttentionSeverity::Critical,
                title: format!("Reconcile unknown CI run {}", run.id),
                target: AttentionTarget {
                    run_id: Some(run.id.clone()),
                    attempt_id: Some(run.attempt_id.as_str().to_owned()),
                    source_commit: Some(run.source_revision.clone()),
                    ..AttentionTarget::default()
                },
                context_preview: json!({
                    "summary": format!(
                        "CI run {} on project {} has an Unknown outcome ({}). Probe provider before rerunning.",
                        run.id,
                        run.project_id,
                        run.error_code.as_deref().unwrap_or("unknown")
                    ),
                    "subkind": "ci_run",
                    "project_id": run.project_id,
                    "phase": run.phase,
                    "error_code": run.error_code,
                }),
                age_ms,
                ttl_remaining_ms: None,
                allowed_actions: vec!["reconcile_run", "inspect", "open_run"],
            });
        }
    }

    // 2. Implementation work, attempts, campaigns, and plan review gates.
    if let Some(snapshot) = &impl_work.snapshot {
        let work_by_id: BTreeMap<String, &kiln_core::implementation::ImplementationWork> = snapshot
            .work
            .as_ref()
            .into_iter()
            .flat_map(|ledger| ledger.current().chain(ledger.superseded().iter()))
            .filter_map(|work| {
                known_repositories.insert(work.repository.clone());
                work.identity()
                    .ok()
                    .map(|id| (id.as_str().to_owned(), work))
            })
            .collect();

        // 2a. PlanReviewGate (`plan_not_accepted`)
        if let Some(ledger) = &snapshot.work {
            let policy = ledger.plan_review_policy();
            let reviewers: Vec<String> = policy
                .map(|p| p.reviewers.iter().cloned().collect())
                .unwrap_or_default();
            for work in ledger.current() {
                let Ok(work_id) = work.identity() else {
                    continue;
                };
                let Ok(canonical_key) = work.canonical_key() else {
                    continue;
                };
                let is_plan_blocked = matches!(
                    ledger.eligibility_of(&canonical_key),
                    Some(Eligibility::Blocked { reasons }) if reasons.contains(&BlockReason::PlanNotAccepted)
                );
                if !is_plan_blocked {
                    continue;
                }
                let cairn_change = cairn_change_for_work(work);
                let campaign_id = campaign_for_work(
                    &snapshot.campaigns,
                    &work_id,
                    None,
                    Some(&work.repository),
                    cairn_change.as_deref(),
                );
                let thread = ledger.plan_thread(&work_id);
                let latest_plan = thread.and_then(|t| t.plans.last());
                let plan_id = latest_plan.and_then(|p| p.identity().ok());
                let gate = thread.and_then(|t| t.gate().ok());
                let gate_label = match &gate {
                    Some(PlanGate::NoPlan) | None => "no_plan",
                    Some(PlanGate::Unresolved { .. }) => "unresolved",
                    Some(PlanGate::Unreviewed { .. }) => "unreviewed",
                    Some(PlanGate::ChangesRequested { .. }) => "changes_requested",
                    Some(PlanGate::Accepted { .. }) => "accepted",
                };
                let steps: Vec<Value> = latest_plan
                    .map(|plan| {
                        plan.steps
                            .iter()
                            .take(MAX_PLAN_STEPS_PREVIEW)
                            .map(|step| json!({ "id": step.id, "text": step.text }))
                            .collect()
                    })
                    .unwrap_or_default();
                let unresolved_annotations: Vec<Value> = thread
                    .map(|t| {
                        let resolved_ids: BTreeSet<&str> =
                            t.resolutions.iter().map(|r| r.annotation.as_str()).collect();
                        t.annotations
                            .iter()
                            .filter(|ann| ann.blocking && !resolved_ids.contains(ann.id.as_str()))
                            .map(|ann| {
                                json!({
                                    "id": ann.id,
                                    "reviewer": ann.reviewer,
                                    "region": ann.region,
                                    "note": ann.note,
                                    "blocking": ann.blocking,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mut actions = Vec::new();
                if plan_id.is_some()
                    && unresolved_annotations.is_empty()
                    && matches!(gate_label, "unreviewed" | "changes_requested")
                {
                    actions.push("accept_plan");
                    actions.push("request_plan_changes");
                }
                actions.push("inspect");
                if cairn_change.is_some() {
                    actions.push("open_cairn");
                }
                items.push(UnifiedAttentionItem {
                    id: format!("plan_gate:{}", work_id.as_str()),
                    kind: AttentionKind::PlanReviewGate,
                    severity: AttentionSeverity::High,
                    title: format!("Plan review required: {}", work.title),
                    target: AttentionTarget {
                        campaign_id,
                        cairn_change,
                        work_id: Some(work_id.as_str().to_owned()),
                        plan_id: plan_id.as_ref().map(|id| id.as_str().to_owned()),
                        ..AttentionTarget::default()
                    },
                    context_preview: json!({
                        "summary": format!(
                            "Work \"{}\" is blocked on plan_not_accepted (gate: {gate_label}).",
                            work.title
                        ),
                        "work_title": work.title,
                        "work_revision": work.revision,
                        "gate": gate_label,
                        "plan_id": plan_id.as_ref().map(|id| id.as_str().to_owned()),
                        "revision": latest_plan.map(|p| p.revision),
                        "author": latest_plan.map(|p| p.author.as_str()),
                        "steps": steps,
                        "unresolved_annotations": unresolved_annotations,
                        "reviewers": reviewers,
                    }),
                    age_ms: None,
                    ttl_remaining_ms: None,
                    allowed_actions: actions,
                });
            }
        }

        // 2b. UnknownTerminationOrDispatch from AttemptLedger
        if let Some(ledger) = &snapshot.attempts {
            for attempt in ledger.unresolved() {
                let work = work_by_id.get(attempt.work_identity.as_str()).copied();
                let cairn_change = work.and_then(cairn_change_for_work);
                let campaign_id = campaign_for_work(
                    &snapshot.campaigns,
                    &attempt.work_identity,
                    Some(&attempt.identity),
                    work.map(|w| w.repository.as_str()),
                    cairn_change.as_deref(),
                );
                let mut actions = vec!["reconcile_termination", "inspect"];
                if cairn_change.is_some() {
                    actions.push("open_cairn");
                }
                items.push(UnifiedAttentionItem {
                    id: format!(
                        "unknown_attempt:{}:{}",
                        attempt.identity.as_str(),
                        attempt.fence
                    ),
                    kind: AttentionKind::UnknownTerminationOrDispatch,
                    severity: AttentionSeverity::Critical,
                    title: format!(
                        "Reconcile unknown termination for attempt {}",
                        &attempt.identity.as_str()[..12]
                    ),
                    target: AttentionTarget {
                        campaign_id,
                        cairn_change,
                        work_id: Some(attempt.work_identity.as_str().to_owned()),
                        attempt_id: Some(attempt.identity.as_str().to_owned()),
                        fence: Some(attempt.fence),
                        ..AttentionTarget::default()
                    },
                    context_preview: json!({
                        "summary": format!(
                            "Attempt {} (fence {}) ended with Unknown failure; independent termination observation is required before another attempt.",
                            attempt.identity.as_str(),
                            attempt.fence
                        ),
                        "subkind": "implementation_attempt",
                        "phase": attempt.phase,
                        "cause": attempt.cause,
                        "work_title": work.map(|w| w.title.as_str()),
                        "recovery_status": "unreconciled",
                    }),
                    age_ms: attempt
                        .ended_at_ms
                        .or(Some(attempt.claimed_at_ms))
                        .map(|ts| now_ms.saturating_sub(ts)),
                    ttl_remaining_ms: None,
                    allowed_actions: actions,
                });
            }

            for attempt in ledger.attempts() {
                if !attempt.phase.is_active()
                    || attempt.workflow.is_some()
                    || !ledger.dispatch_requires_recovery(&attempt.identity)
                {
                    continue;
                }
                let work = work_by_id.get(attempt.work_identity.as_str()).copied();
                let cairn_change = work.and_then(cairn_change_for_work);
                let campaign_id = campaign_for_work(
                    &snapshot.campaigns,
                    &attempt.work_identity,
                    Some(&attempt.identity),
                    work.map(|w| w.repository.as_str()),
                    cairn_change.as_deref(),
                );
                let mut actions = vec!["dispatch_recovery", "inspect"];
                if cairn_change.is_some() {
                    actions.push("open_cairn");
                }
                items.push(UnifiedAttentionItem {
                    id: format!(
                        "dispatch_recovery:{}:{}",
                        attempt.identity.as_str(),
                        attempt.fence
                    ),
                    kind: AttentionKind::UnknownTerminationOrDispatch,
                    severity: AttentionSeverity::High,
                    title: format!(
                        "Recover started dispatch for attempt {}",
                        &attempt.identity.as_str()[..12]
                    ),
                    target: AttentionTarget {
                        campaign_id,
                        cairn_change,
                        work_id: Some(attempt.work_identity.as_str().to_owned()),
                        attempt_id: Some(attempt.identity.as_str().to_owned()),
                        fence: Some(attempt.fence),
                        ..AttentionTarget::default()
                    },
                    context_preview: json!({
                        "summary": format!(
                            "Attempt {} (fence {}) recorded transport intent with no observed workflow execution.",
                            attempt.identity.as_str(),
                            attempt.fence
                        ),
                        "subkind": "dispatch_recovery",
                        "phase": attempt.phase,
                        "dispatch": "transport_started",
                        "recovery_status": "retained_transport_intent",
                        "work_title": work.map(|w| w.title.as_str()),
                    }),
                    age_ms: Some(now_ms.saturating_sub(attempt.claimed_at_ms)),
                    ttl_remaining_ms: attempt.lease_until_ms.checked_sub(now_ms),
                    allowed_actions: actions,
                });
            }
        }

        // 2c. Campaign decisions & uncertain campaign effects
        for (state, store_version) in &snapshot.campaigns {
            known_repositories.insert(state.contract.repository.clone());
            for decision in state.decisions.values() {
                if !matches!(
                    decision.state,
                    DecisionRequestState::Outstanding | DecisionRequestState::Unissued
                ) {
                    continue;
                }
                let cairn_change = member_cairn_change(state, &decision.member);
                let scope = crate::campaign_controls::decision_scope(state, decision);
                let severity = if decision.state == DecisionRequestState::Outstanding {
                    AttentionSeverity::High
                } else {
                    AttentionSeverity::Medium
                };
                let mut actions = Vec::new();
                if decision.state == DecisionRequestState::Outstanding && scope == "current" {
                    actions.push("resolve_decision");
                } else if decision.state == DecisionRequestState::Unissued {
                    actions.push("issue_decision");
                }
                actions.push("inspect");
                if cairn_change.is_some() {
                    actions.push("open_cairn");
                }
                items.push(UnifiedAttentionItem {
                    id: format!(
                        "campaign_decision:{}:{}",
                        state.contract.campaign_id,
                        decision.identity.as_str()
                    ),
                    kind: AttentionKind::CampaignDecision,
                    severity,
                    title: format!(
                        "Campaign {} decision: {}",
                        state.contract.campaign_id, decision.member
                    ),
                    target: AttentionTarget {
                        campaign_id: Some(state.contract.campaign_id.clone()),
                        cairn_change,
                        work_id: decision
                            .work_identity
                            .as_ref()
                            .map(|id| id.as_str().to_owned()),
                        decision_id: Some(decision.identity.as_str().to_owned()),
                        ..AttentionTarget::default()
                    },
                    context_preview: json!({
                        "summary": decision.question,
                        "question": decision.question,
                        "options": decision.options,
                        "recommendation": decision.recommendation,
                        "member": decision.member,
                        "category": decision.kind,
                        "state": decision.state,
                        "scope": scope,
                        "campaign_version": store_version,
                        "policy_identity": decision.policy_identity,
                        "candidate_identity": decision.candidate_identity,
                        "decision_revision": decision.decision_revision,
                    }),
                    age_ms: Some(now_ms.saturating_sub(decision.created_ms)),
                    ttl_remaining_ms: None,
                    allowed_actions: actions,
                });
            }

            for effect in state.pending_effects.values() {
                if effect.progress_applied
                    || !matches!(
                        effect.state,
                        EffectState::Unknown | EffectState::IntentPersisted
                    )
                {
                    continue;
                }
                let cairn_change = member_cairn_change(state, &effect.member);
                items.push(UnifiedAttentionItem {
                    id: format!(
                        "unknown_campaign_effect:{}:{}",
                        state.contract.campaign_id,
                        effect.effect_id.as_str()
                    ),
                    kind: AttentionKind::UnknownTerminationOrDispatch,
                    severity: AttentionSeverity::High,
                    title: format!(
                        "Reconcile {} effect in campaign {}",
                        effect.owner, state.contract.campaign_id
                    ),
                    target: AttentionTarget {
                        campaign_id: Some(state.contract.campaign_id.clone()),
                        cairn_change,
                        work_id: effect
                            .work_identity
                            .as_ref()
                            .map(|id| id.as_str().to_owned()),
                        attempt_id: effect
                            .attempt_identity
                            .as_ref()
                            .map(|id| id.as_str().to_owned()),
                        fence: effect.attempt_fence,
                        ..AttentionTarget::default()
                    },
                    context_preview: json!({
                        "summary": format!(
                            "Campaign {} member {} has unapplied {:?} effect {} ({:?}).",
                            state.contract.campaign_id,
                            effect.member,
                            effect.operation,
                            effect.effect_id.as_str(),
                            effect.state
                        ),
                        "subkind": "campaign_effect",
                        "effect_id": effect.effect_id,
                        "operation": effect.operation,
                        "owner": effect.owner,
                        "effect_state": effect.state,
                    }),
                    age_ms: None,
                    ttl_remaining_ms: None,
                    allowed_actions: vec!["wake_campaign", "inspect"],
                });
            }
        }
    }

    // 3. Fleet coordinator: open session decisions, expiring claims, and foreign claim conflicts.
    if fleet.health.available {
        for decision in &fleet.decisions {
            if !decision.is_open() {
                continue;
            }
            let attempt_str = decision.attempt.as_str().to_owned();
            let hint = fleet.attempt_hints.get(&attempt_str);
            let stale_observation = is_stale_observation_decision(decision);
            items.push(UnifiedAttentionItem {
                id: format!("fleet_decision:{}", decision.identity.as_str()),
                kind: AttentionKind::FleetSessionDecision,
                severity: AttentionSeverity::High,
                title: if stale_observation {
                    format!(
                        "Stale agent observation on attempt {}",
                        &attempt_str[..12]
                    )
                } else {
                    format!("Agent question on attempt {}", &attempt_str[..12])
                },
                target: AttentionTarget {
                    attempt_id: Some(attempt_str),
                    decision_id: Some(decision.identity.as_str().to_owned()),
                    machine: hint.and_then(|h| h.machine.clone()),
                    ..AttentionTarget::default()
                },
                context_preview: json!({
                    "summary": decision.question,
                    "question": decision.question,
                    "session": decision.session,
                    "attempt": decision.attempt,
                    "raised_ms": decision.raised_ms,
                    "stale_observation": stale_observation,
                    "session_state": hint.and_then(|h| h.session_state.as_deref()),
                }),
                age_ms: Some(now_ms.saturating_sub(decision.raised_ms)),
                ttl_remaining_ms: None,
                allowed_actions: vec!["answer_decision", "inspect_pane", "cancel_attempt", "inspect"],
            });
        }

        let live_claims: Vec<&FleetClaim> = fleet
            .claims
            .iter()
            .filter(|claim| claim.is_live(now_ms))
            .collect();
        let horizon = now_ms.saturating_add(CLAIM_EXPIRING_WINDOW_MS);
        for claim in &live_claims {
            if claim.expires_ms <= horizon {
                let remaining = claim.expires_ms.saturating_sub(now_ms);
                let severity = if remaining <= CLAIM_CRITICAL_WINDOW_MS {
                    AttentionSeverity::High
                } else {
                    AttentionSeverity::Medium
                };
                let scope = claim.scope.text();
                items.push(UnifiedAttentionItem {
                    id: format!("expiring_claim:{}", claim.identity.as_str()),
                    kind: AttentionKind::ExpiringOrConflictClaim,
                    severity,
                    title: format!("Claim expiring soon: {scope}"),
                    target: AttentionTarget {
                        attempt_id: claim.attempt.as_ref().map(|a| a.as_str().to_owned()),
                        claim_id: Some(claim.identity.as_str().to_owned()),
                        fence: Some(claim.fence),
                        machine: Some(claim.holder.host.clone()),
                        scope: Some(scope.clone()),
                        ..AttentionTarget::default()
                    },
                    context_preview: json!({
                        "summary": format!(
                            "Claim held by {} for {} expires in {}s.",
                            claim.holder.label(),
                            scope,
                            remaining / 1000
                        ),
                        "subkind": "expiring_claim",
                        "holder": claim.holder.label(),
                        "scope_text": scope,
                        "purpose": claim.purpose,
                        "expires_ms": claim.expires_ms,
                    }),
                    age_ms: Some(now_ms.saturating_sub(claim.granted_ms)),
                    ttl_remaining_ms: Some(remaining),
                    allowed_actions: vec!["renew_claim", "release_claim", "inspect"],
                });
            }
        }

        for foreign in &fleet.foreign_claims {
            let scope_text = if foreign.scope_text.is_empty() {
                foreign.scope.text()
            } else {
                foreign.scope_text.clone()
            };
            let overlaps_live = live_claims
                .iter()
                .any(|claim| overlaps(&foreign.scope, &claim.scope));
            let overlaps_repo = known_repositories
                .iter()
                .any(|repo| repository_matches_scope_target(repo, foreign.scope.target_text()));
            if !overlaps_live && !overlaps_repo {
                continue;
            }
            let age_ms = if foreign.age_ms > 0 {
                foreign.age_ms
            } else {
                now_ms.saturating_sub(foreign.date_unix_ms)
            };
            let digest = blake3::hash(format!("{}:{}", foreign.message_id, scope_text).as_bytes())
                .to_hex()
                .to_string();
            items.push(UnifiedAttentionItem {
                id: format!("foreign_claim:{}", &digest[..16]),
                kind: AttentionKind::ExpiringOrConflictClaim,
                severity: if overlaps_live {
                    AttentionSeverity::High
                } else {
                    AttentionSeverity::Medium
                },
                title: format!("Foreign claim conflict: {scope_text}"),
                target: AttentionTarget {
                    scope: Some(scope_text.clone()),
                    ..AttentionTarget::default()
                },
                context_preview: json!({
                    "summary": format!(
                        "Foreign swarm claim on {} held by {} (author {}).",
                        scope_text,
                        if foreign.holder.is_empty() { &foreign.author } else { &foreign.holder },
                        foreign.author
                    ),
                    "subkind": "foreign_claim",
                    "scope_text": scope_text,
                    "holder": foreign.holder,
                    "author": foreign.author,
                    "message_id": foreign.message_id,
                    "overlaps_live_claim": overlaps_live,
                }),
                age_ms: Some(age_ms),
                ttl_remaining_ms: foreign
                    .stale_after_ms
                    .checked_sub(age_ms),
                allowed_actions: vec!["inspect"],
            });
        }
    }

    items.sort_by(|left, right| {
        left.severity
            .rank()
            .cmp(&right.severity.rank())
            .then_with(|| left.age_ms.is_none().cmp(&right.age_ms.is_none()))
            .then_with(|| Reverse(left.age_ms.unwrap_or(0)).cmp(&Reverse(right.age_ms.unwrap_or(0))))
            .then_with(|| left.kind.rank().cmp(&right.kind.rank()))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut counts = AttentionCounts::default();
    for item in &items {
        counts.record(item.kind);
    }
    let total = items.len();
    let truncated = total > MAX_ATTENTION_ITEMS;
    items.truncate(MAX_ATTENTION_ITEMS);

    AttentionProjection {
        schema: ATTENTION_SCHEMA,
        generated_at_ms: now_ms,
        items,
        total,
        bounded: true,
        truncated,
        counts,
        sources: AttentionSources {
            ci_engine: ci.health,
            implementation_work: impl_work.health,
            fleet: fleet.health,
        },
    }
}

fn read_impl_attention(database: Option<&Path>) -> ImplAttentionInput {
    let Some(path) = database else {
        return ImplAttentionInput {
            health: SourceHealth {
                configured: false,
                available: false,
                error: None,
            },
            snapshot: None,
        };
    };
    let observation = match ImplementationObservation::open(path) {
        Ok(observation) => observation,
        Err(_) => {
            return ImplAttentionInput {
                health: SourceHealth {
                    configured: true,
                    available: false,
                    error: Some("implementation_work_unavailable".into()),
                },
                snapshot: None,
            };
        }
    };
    match observation.snapshot() {
        Ok(snapshot) => ImplAttentionInput {
            health: SourceHealth {
                configured: true,
                available: true,
                error: None,
            },
            snapshot: Some(snapshot),
        },
        Err(_) => ImplAttentionInput {
            health: SourceHealth {
                configured: true,
                available: false,
                error: Some("implementation_work_unavailable".into()),
            },
            snapshot: None,
        },
    }
}

#[derive(Deserialize)]
struct DecisionsEnvelope {
    decisions: Vec<FleetDecision>,
}

#[derive(Deserialize)]
struct ClaimsWithForeignEnvelope {
    claims: Vec<FleetClaim>,
    #[serde(default)]
    foreign_claims: Vec<ForeignClaimView>,
}

#[derive(Deserialize)]
struct AttemptsEnvelope {
    #[serde(default)]
    attempts: Vec<Value>,
}

fn parse_fleet_data(
    claims_bytes: Option<&[u8]>,
    decisions_bytes: Option<&[u8]>,
    attempts_bytes: Option<&[u8]>,
) -> Result<FleetAttentionInput, ()> {
    let claims_env: ClaimsWithForeignEnvelope =
        serde_json::from_slice(claims_bytes.ok_or(())?).map_err(|_| ())?;
    let decisions_env: DecisionsEnvelope =
        serde_json::from_slice(decisions_bytes.ok_or(())?).map_err(|_| ())?;
    let mut attempt_hints = BTreeMap::new();
    if let Some(bytes) = attempts_bytes
        && let Ok(env) = serde_json::from_slice::<AttemptsEnvelope>(bytes)
    {
        for row in env.attempts {
            let Some(attempt) = row.get("attempt").and_then(Value::as_str) else {
                continue;
            };
            let machine = row
                .pointer("/session/machine")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let session_state = row
                .pointer("/session/state")
                .and_then(Value::as_str)
                .map(str::to_owned);
            attempt_hints.insert(
                attempt.to_owned(),
                FleetAttemptHint {
                    machine,
                    session_state,
                },
            );
        }
    }
    Ok(FleetAttentionInput {
        health: SourceHealth {
            configured: true,
            available: true,
            error: None,
        },
        decisions: decisions_env.decisions,
        claims: claims_env.claims,
        foreign_claims: claims_env.foreign_claims,
        attempt_hints,
    })
}

fn read_fleet_attention(context: &Context<'_>) -> FleetAttentionInput {
    let unavailable = || FleetAttentionInput {
        health: SourceHealth {
            configured: true,
            available: false,
            error: Some("fleet_unavailable".into()),
        },
        decisions: Vec::new(),
        claims: Vec::new(),
        foreign_claims: Vec::new(),
        attempt_hints: BTreeMap::new(),
    };
    let Some(client) = context.fleet else {
        return FleetAttentionInput {
            health: SourceHealth {
                configured: false,
                available: false,
                error: None,
            },
            decisions: Vec::new(),
            claims: Vec::new(),
            foreign_claims: Vec::new(),
            attempt_hints: BTreeMap::new(),
        };
    };
    if let Some(cached) = context.events.and_then(|hub| hub.cached_fleet_attention()) {
        if !cached.available {
            return unavailable();
        }
        return parse_fleet_data(
            cached.claims_bytes.as_deref().map(Vec::as_slice),
            cached.decisions_bytes.as_deref().map(Vec::as_slice),
            cached.attempts_bytes.as_deref().map(Vec::as_slice),
        )
        .unwrap_or_else(|()| unavailable());
    }
    let claims_reply = match client.call("GET", "/v1/claims?include=foreign,expired", None) {
        Ok(reply) if (200..300).contains(&reply.status) => reply,
        Ok(reply) if matches!(reply.status, 400 | 404) => {
            let Ok(fallback) = client.call("GET", "/v1/claims", None) else {
                return unavailable();
            };
            if !(200..300).contains(&fallback.status) {
                return unavailable();
            }
            fallback
        }
        _ => return unavailable(),
    };
    let Ok(decisions_reply) = client.call("GET", "/v1/decisions", None) else {
        return unavailable();
    };
    if !(200..300).contains(&decisions_reply.status) {
        return unavailable();
    }
    let attempts_bytes = client
        .call("GET", "/v1/attempts", None)
        .ok()
        .filter(|reply| (200..300).contains(&reply.status))
        .map(|reply| reply.body);
    parse_fleet_data(
        Some(&claims_reply.body),
        Some(&decisions_reply.body),
        attempts_bytes.as_deref(),
    )
    .unwrap_or_else(|()| unavailable())
}

pub(super) fn attention_summary_for_event(
    now_ms: u64,
    ci_runs: Vec<RunView>,
    ci_repositories: Vec<String>,
    implementation_work: Option<&Path>,
    fleet_cached: Option<&super::CachedFleetData>,
) -> Option<(String, Vec<u8>)> {
    let ci = CiAttentionInput {
        health: SourceHealth {
            configured: true,
            available: true,
            error: None,
        },
        runs: ci_runs,
        repositories: ci_repositories,
    };
    let impl_work = read_impl_attention(implementation_work);
    let fleet = match fleet_cached {
        None => FleetAttentionInput {
            health: SourceHealth {
                configured: false,
                available: false,
                error: None,
            },
            decisions: Vec::new(),
            claims: Vec::new(),
            foreign_claims: Vec::new(),
            attempt_hints: BTreeMap::new(),
        },
        Some(cached) if !cached.available => FleetAttentionInput {
            health: SourceHealth {
                configured: true,
                available: false,
                error: Some("fleet_unavailable".into()),
            },
            decisions: Vec::new(),
            claims: Vec::new(),
            foreign_claims: Vec::new(),
            attempt_hints: BTreeMap::new(),
        },
        Some(cached) => parse_fleet_data(
            cached.claims_bytes.as_deref().map(Vec::as_slice),
            cached.decisions_bytes.as_deref().map(Vec::as_slice),
            cached.attempts_bytes.as_deref().map(Vec::as_slice),
        )
        .unwrap_or(FleetAttentionInput {
            health: SourceHealth {
                configured: true,
                available: false,
                error: Some("fleet_unavailable".into()),
            },
            decisions: Vec::new(),
            claims: Vec::new(),
            foreign_claims: Vec::new(),
            attempt_hints: BTreeMap::new(),
        }),
    };
    let projection = assemble_attention(now_ms, ci, impl_work, fleet);
    let ids: Vec<&str> = projection
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let digest_input = serde_json::to_vec(&(&ids, &projection.counts, &projection.sources)).ok()?;
    let digest = blake3::hash(&digest_input).to_hex().to_string();
    let payload = serde_json::to_vec(&json!({
        "invalidated": true,
        "total": projection.total,
        "digest": digest,
        "counts": projection.counts,
        "sources": projection.sources,
    }))
    .ok()?;
    Some((digest, payload))
}

// r[impl kiln.implementation.campaign.unified_attention_and_recovery]
pub(super) fn attention_response(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
) -> ApiResponse {
    if principal.role == Role::Worker {
        return error_response(403, "attention_forbidden");
    }
    if request.method != "GET" {
        return error_response(405, "attention_read_only");
    }
    if request.target != "/api/implementation/attention" || !request.body.is_empty() {
        return error_response(400, "attention_query_invalid");
    }
    let Ok(now_ms) = crate::engine::host_time_ms() else {
        return error_response(503, "clock_unavailable");
    };
    let ci = match context.engine.lock() {
        Ok(engine) => CiAttentionInput {
            health: SourceHealth {
                configured: true,
                available: true,
                error: None,
            },
            runs: engine.attention_runs(),
            repositories: engine.project_repositories(),
        },
        Err(_) => CiAttentionInput {
            health: SourceHealth {
                configured: true,
                available: false,
                error: Some("ci_engine_unavailable".into()),
            },
            runs: Vec::new(),
            repositories: Vec::new(),
        },
    };
    let impl_work = read_impl_attention(context.implementation_work);
    let fleet = read_fleet_attention(context);
    let projection = assemble_attention(now_ms, ci, impl_work, fleet);
    match serde_json::to_vec(&projection) {
        Ok(body) if body.len() <= MAXIMUM_RESPONSE_BYTES => ApiResponse {
            status: 200,
            content_type: "application/json",
            body,
            download_name: None,
            headers: Vec::new(),
        },
        _ => error_response(503, "attention_projection_byte_bound"),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WakeBody {
    kind: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    observed_version: Option<u64>,
    #[serde(default)]
    request_id: Option<String>,
}

fn valid_reason(reason: &str) -> bool {
    !reason.trim().is_empty()
        && reason.len() <= MAX_REASON_BYTES
        && !reason.chars().any(char::is_control)
}

// r[impl kiln.implementation.campaign.unified_attention_and_recovery]
pub(super) fn campaign_wake_response(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
) -> ApiResponse {
    if principal.role != Role::Operator {
        return error_response(403, "operator_required");
    }
    if request.method != "POST" {
        return error_response(405, "campaign_wake_method_denied");
    }
    if request.target.contains('?') {
        return error_response(400, "campaign_wake_query_invalid");
    }
    let Some(raw_id) = request
        .target
        .strip_prefix("/api/implementation/campaigns/")
        .and_then(|rest| rest.strip_suffix("/wake"))
    else {
        return error_response(404, "route_missing");
    };
    if raw_id.is_empty() || raw_id.contains('/') {
        return error_response(400, "campaign_id_invalid");
    }
    let Ok(campaign_id) = crate::engine::api::decoded(raw_id) else {
        return error_response(400, "campaign_id_invalid");
    };
    if campaign_id.trim().is_empty()
        || campaign_id.len() > MAX_ID_BYTES
        || campaign_id.chars().any(char::is_control)
        || campaign_id.contains('/')
    {
        return error_response(400, "campaign_id_invalid");
    }
    let body: WakeBody = match serde_json::from_slice(&request.body) {
        Ok(body) => body,
        Err(_) => return error_response(400, "campaign_wake_invalid"),
    };
    let Ok(kind) = WakeKind::parse(&body.kind) else {
        return error_response(400, "campaign_wake_kind_invalid");
    };
    if body.reason.as_deref().is_some_and(|r| !valid_reason(r)) {
        return error_response(400, "campaign_wake_reason_invalid");
    }
    if body
        .request_id
        .as_deref()
        .is_some_and(|id| !crate::profile::identifier(id))
    {
        return error_response(400, "request_id_invalid");
    }
    let Some(database) = context.implementation_work else {
        return error_response(503, "implementation_work_not_configured");
    };
    let Ok(observation) = CampaignObservation::open(database) else {
        return error_response(503, "campaign_store_unavailable");
    };
    match observation.inspect(&campaign_id) {
        Ok(Some(_)) => {}
        Ok(None) => return error_response(404, "campaign_not_found"),
        Err(_) => return error_response(503, "campaign_store_unavailable"),
    }
    let Ok(mut store) = CampaignStore::open(database) else {
        return error_response(503, "campaign_store_unavailable");
    };
    let (record, version) = match store.record_wake(
        &campaign_id,
        WakeHint {
            kind,
            observed_version: body.observed_version,
        },
    ) {
        Ok(res) => res,
        Err(_) => return error_response(503, "campaign_store_unavailable"),
    };
    if let Some(hub) = context.events {
        hub.wake_watcher();
    }
    let payload = json!({
        "schema": WAKE_SCHEMA,
        "campaign_id": campaign_id,
        "kind": body.kind,
        "reason": body.reason,
        "request_id": body.request_id,
        "observed_version": body.observed_version,
        "continuation": record,
        "continuation_version": version,
    });
    ApiResponse {
        status: 200,
        content_type: "application/json",
        body: payload.to_string().into_bytes(),
        download_name: None,
        headers: Vec::new(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconcileTerminationBody {
    #[serde(default)]
    campaign_id: Option<String>,
    work_id: ContentId,
    attempt_id: ContentId,
    fence: u64,
    request_id: String,
    #[serde(default)]
    termination: Option<ExternalRef>,
    #[serde(default)]
    reason: Option<String>,
}

// r[impl kiln.implementation.campaign.unified_attention_and_recovery]
pub(super) fn reconcile_termination_response(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
) -> ApiResponse {
    if principal.role != Role::Operator {
        return error_response(403, "operator_required");
    }
    if request.method != "POST" {
        return error_response(405, "reconcile_method_denied");
    }
    if request.target != "/api/implementation/reconcile-termination" {
        return error_response(400, "reconcile_query_invalid");
    }
    let body: ReconcileTerminationBody = match serde_json::from_slice(&request.body) {
        Ok(body) => body,
        Err(_) => return error_response(400, "reconcile_invalid"),
    };
    if !crate::profile::identifier(&body.request_id) {
        return error_response(400, "request_id_invalid");
    }
    if body.fence == 0 {
        return error_response(400, "reconcile_fence_invalid");
    }
    if body
        .campaign_id
        .as_deref()
        .is_some_and(|cid| cid.trim().is_empty() || cid.len() > MAX_ID_BYTES || cid.chars().any(char::is_control))
    {
        return error_response(400, "campaign_id_invalid");
    }
    if body.reason.as_deref().is_some_and(|r| !valid_reason(r)) {
        return error_response(400, "reconcile_reason_invalid");
    }
    if let Some(term) = &body.termination
        && term.validate().is_err()
    {
        return error_response(400, "reconcile_termination_invalid");
    }
    let Some(database) = context.implementation_work else {
        return error_response(503, "implementation_work_not_configured");
    };
    let (work_ledger, attempt_ledger) = match ImplementationObservation::open(database)
        .and_then(|obs| obs.dispatch_recovery_ledgers())
    {
        Ok((work, Some(attempts))) => (work, attempts),
        Ok((_, None)) => return error_response(404, "implementation_attempt_missing"),
        Err(_) => return error_response(503, "implementation_work_unavailable"),
    };
    let Some(attempt) = attempt_ledger.get(&body.attempt_id) else {
        return error_response(404, "implementation_attempt_missing");
    };
    if attempt.work_identity != body.work_id {
        return error_response(409, "reconcile_work_mismatch");
    }
    if attempt.fence != body.fence {
        return error_response(409, "reconcile_fence_mismatch");
    }
    if attempt.failure != Some(FailureClass::Unknown) {
        return error_response(409, "implementation_reconciliation_binding");
    }
    if let Some(campaign_id) = body.campaign_id.as_deref() {
        let Ok(campaign_obs) = CampaignObservation::open(database) else {
            return error_response(503, "campaign_store_unavailable");
        };
        let (state, _) = match campaign_obs.inspect(campaign_id) {
            Ok(Some(found)) => found,
            Ok(None) => return error_response(404, "campaign_not_found"),
            Err(_) => return error_response(503, "campaign_store_unavailable"),
        };
        let obs_conflict = state.observations.values().any(|obs| {
            obs.attempt_identity.as_ref() == Some(&body.attempt_id)
                && (obs.attempt_fence != Some(body.fence)
                    || obs.work_identity.as_ref() != Some(&body.work_id))
        });
        if obs_conflict {
            return error_response(409, "reconcile_campaign_mismatch");
        }
        let work_item = work_ledger.as_ref().and_then(|ledger| {
            ledger
                .current()
                .chain(ledger.superseded().iter())
                .find(|w| w.identity().is_ok_and(|id| id == body.work_id))
        });
        let cairn_change = work_item.and_then(cairn_change_for_work);
        let matches_campaign = state.observations.values().any(|obs| {
            obs.attempt_identity.as_ref() == Some(&body.attempt_id)
                || obs.work_identity.as_ref() == Some(&body.work_id)
        }) || state.pending_effects.values().any(|eff| {
            eff.attempt_identity.as_ref() == Some(&body.attempt_id)
                || eff.work_identity.as_ref() == Some(&body.work_id)
        }) || work_item.is_some_and(|w| {
            w.repository == state.contract.repository
                && cairn_change.as_deref().is_some_and(|chg| {
                    state
                        .contract
                        .members
                        .iter()
                        .any(|m| selector_change(&m.selector) == chg)
                })
        });
        if !matches_campaign {
            return error_response(409, "reconcile_campaign_mismatch");
        }
    }
    let termination = match body.termination {
        Some(term) => term,
        None => match ExternalRef::new(
            "kiln-operator",
            "termination",
            &format!("{}-{}", body.attempt_id.as_str(), body.fence),
        ) {
            Ok(term) => term,
            Err(_) => return error_response(400, "reconcile_termination_invalid"),
        },
    };
    let mut implementer = match Implementer::open(
        database,
        Enrollment {
            repositories: BTreeSet::new(),
            actors: BTreeSet::new(),
        },
    ) {
        Ok(implementer) => implementer,
        Err(_) => return error_response(503, "implementation_work_unavailable"),
    };
    if let Err(err) = implementer.reconcile_unknown(&body.attempt_id, body.fence, &termination) {
        return if err.contains("implementation_reconciliation_conflict") {
            error_response(409, "implementation_reconciliation_conflict")
        } else if err.contains("implementation_reconciliation_binding") {
            error_response(409, "implementation_reconciliation_binding")
        } else if err.contains("implementation_attempt_missing") {
            error_response(404, "implementation_attempt_missing")
        } else {
            error_response(503, "implementation_work_unavailable")
        };
    }
    let campaign_woken = if let Some(campaign_id) = body.campaign_id.as_deref() {
        CampaignStore::open(database)
            .and_then(|mut store| {
                store.record_wake(
                    campaign_id,
                    WakeHint {
                        kind: WakeKind::Reconciliation,
                        observed_version: None,
                    },
                )
            })
            .is_ok()
    } else {
        false
    };
    if let Some(hub) = context.events {
        hub.wake_watcher();
    }
    let payload = json!({
        "schema": RECONCILE_TERMINATION_SCHEMA,
        "reconciled": true,
        "campaign_id": body.campaign_id,
        "work_id": body.work_id,
        "attempt_id": body.attempt_id,
        "fence": body.fence,
        "request_id": body.request_id,
        "termination": termination,
        "campaign_woken": campaign_woken,
    });
    ApiResponse {
        status: 200,
        content_type: "application/json",
        body: payload.to_string().into_bytes(),
        download_name: None,
        headers: Vec::new(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanReviewHttpBody {
    work_id: ContentId,
    plan_id: ContentId,
    verdict: PlanVerdict,
    request_id: String,
}

// r[impl kiln.implementation.campaign.unified_attention_and_recovery]
pub(super) fn plan_review_response(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
) -> ApiResponse {
    if principal.role != Role::Operator {
        return error_response(403, "operator_required");
    }
    if request.method != "POST" {
        return error_response(405, "plan_review_method_denied");
    }
    if request.target != "/api/implementation/plan-review" {
        return error_response(400, "plan_review_query_invalid");
    }
    let body: PlanReviewHttpBody = match serde_json::from_slice(&request.body) {
        Ok(body) => body,
        Err(_) => return error_response(400, "plan_review_invalid"),
    };
    if !crate::profile::identifier(&body.request_id) {
        return error_response(400, "request_id_invalid");
    }
    let Some(database) = context.implementation_work else {
        return error_response(503, "implementation_work_not_configured");
    };
    let snapshot = match ImplementationObservation::open(database).and_then(|obs| obs.snapshot()) {
        Ok(snapshot) => snapshot,
        Err(_) => return error_response(503, "implementation_work_unavailable"),
    };
    let Some(ledger) = snapshot.work.as_ref() else {
        return error_response(404, "implementation_plan_work");
    };
    if !ledger
        .current()
        .any(|work| work.identity().is_ok_and(|id| id == body.work_id))
    {
        return error_response(404, "implementation_plan_work");
    }
    let mut implementer = match Implementer::open(
        database,
        Enrollment {
            repositories: BTreeSet::new(),
            actors: BTreeSet::new(),
        },
    ) {
        Ok(implementer) => implementer,
        Err(_) => return error_response(503, "implementation_work_unavailable"),
    };
    let result = implementer.plan_review(&PlanReviewCommand::Decide {
        work: body.work_id.clone(),
        decision: PlanDecision {
            plan: body.plan_id.clone(),
            reviewer: principal.id.clone(),
            verdict: body.verdict,
        },
    });
    let gate = match result {
        Ok(PlanReviewResult::Reviewed { gate }) => gate,
        Ok(_) => return error_response(503, "implementation_work_unavailable"),
        Err(err) => {
            return if err.contains("implementation_plan_reviewer") {
                error_response(403, "implementation_plan_reviewer")
            } else if err.contains("implementation_plan_review_disabled") {
                error_response(409, "implementation_plan_review_disabled")
            } else if err.contains("implementation_plan_work") {
                error_response(409, "implementation_plan_work")
            } else if err.contains("implementation_plan_revision") {
                error_response(409, "implementation_plan_revision")
            } else if err.contains("implementation_plan_decision") {
                error_response(409, "implementation_plan_decision")
            } else if err.contains("implementation_plan_missing") {
                error_response(409, "implementation_plan_missing")
            } else {
                error_response(503, "implementation_work_unavailable")
            };
        }
    };
    if let Some(hub) = context.events {
        hub.wake_watcher();
    }
    let payload = json!({
        "schema": PLAN_REVIEW_SCHEMA,
        "work_id": body.work_id,
        "plan_id": body.plan_id,
        "reviewer": principal.id,
        "verdict": body.verdict,
        "gate": gate,
        "request_id": body.request_id,
    });
    ApiResponse {
        status: 200,
        content_type: "application/json",
        body: payload.to_string().into_bytes(),
        download_name: None,
        headers: Vec::new(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn worker_and_mutating_requests_cannot_reach_observation_io() {
        assert!(matches!(
            admit_request("GET", Role::Worker, "/api/inventory"),
            Err((403, "inventory_forbidden"))
        ));
        assert!(matches!(
            admit_request("POST", Role::Operator, "/api/inventory"),
            Err((405, "inventory_read_only"))
        ));
        assert!(matches!(
            admit_request("GET", Role::Viewer, "/api/inventory"),
            Ok(None)
        ));
    }

    #[test]
    fn exact_work_queries_reject_duplicate_unknown_and_encoded_fields() {
        let identity = "a".repeat(64);
        assert!(matches!(
            admit_request("GET", Role::Viewer, &format!("/api/inventory?work={identity}")),
            Ok(Some(work)) if work.as_str() == identity
        ));
        for query in [
            "work=short".to_owned(),
            format!("work={identity}&work={identity}"),
            format!("work={identity}&unknown=1"),
            format!("unknown={identity}"),
            format!("work=%61{}", "a".repeat(63)),
            "".to_owned(),
        ] {
            assert!(matches!(
                admit_request("GET", Role::Viewer, &format!("/api/inventory?{query}")),
                Err((400, "inventory_work_query_invalid"))
            ));
        }
    }

    #[test]
    fn campaign_lookup_preserves_opaque_ids_and_rejects_crossed_queries() {
        assert_eq!(
            admit_campaign_request("GET", Role::Viewer, "/api/implementation/campaigns"),
            Ok(None)
        );
        let target = "/api/implementation/campaigns?campaign=group%20%CE%B1%2Bbeta%2F..%3Fx";
        assert_eq!(
            admit_campaign_request("GET", Role::Viewer, target),
            Ok(Some("group α+beta/..?x".into()))
        );
        for (value, expected) in [
            (".", "."),
            ("..", ".."),
            ("a%26campaign%3Db", "a&campaign=b"),
        ] {
            assert_eq!(
                admit_campaign_request(
                    "GET",
                    Role::Viewer,
                    &format!("/api/implementation/campaigns?campaign={value}")
                ),
                Ok(Some(expected.to_owned()))
            );
        }
        assert_eq!(
            admit_campaign_request("GET", Role::Worker, target),
            Err((403, "inventory_forbidden"))
        );
        assert_eq!(
            admit_campaign_request("GET", Role::Worker, "/api/implementation/campaigns"),
            Err((403, "inventory_forbidden"))
        );
        assert_eq!(
            admit_campaign_request("POST", Role::Operator, "/api/implementation/campaigns"),
            Err((405, "inventory_read_only"))
        );
        assert_eq!(
            admit_campaign_request("POST", Role::Operator, target),
            Err((405, "inventory_read_only"))
        );
        for value in [
            "".to_owned(),
            "%00".to_owned(),
            "%FF".to_owned(),
            "%".to_owned(),
            "x&campaign=y".to_owned(),
            "x&unknown=y".to_owned(),
            "x".repeat(kiln_core::implementation::campaign::MAX_ID_BYTES + 1),
        ] {
            assert_eq!(
                admit_campaign_request(
                    "GET",
                    Role::Viewer,
                    &format!("/api/implementation/campaigns?campaign={value}")
                ),
                Err((400, "campaign_projection_query_invalid"))
            );
        }
        assert_eq!(
            admit_campaign_request("GET", Role::Viewer, "/api/implementation/campaigns?unknown=1"),
            Err((400, "campaign_projection_query_invalid"))
        );
    }

    #[test]
    fn campaign_index_orders_active_campaigns_and_handles_empty_or_corrupt_stores()
    -> Result<(), Box<dyn std::error::Error>> {
        use kiln_core::implementation::campaign::{
            CAMPAIGN_SCHEMA, CampaignContract, CompletionGoal, DecisionKind, HumanDecisionRequest,
            MemberBinding, MemberSelector, WorkflowProfile,
        };
        use kiln_core::types::ContentId;
        use kiln_implementer::campaign::CampaignStore;

        let dir = tempfile::tempdir()?;
        let empty_db = dir.path().join("empty.sqlite");
        let conn = Connection::open(&empty_db)?;
        drop(conn);
        let empty = read_campaign_index(&empty_db).map_err(std::io::Error::other)?;
        assert_eq!(empty.schema, CAMPAIGN_INDEX_SCHEMA);
        assert!(empty.campaigns.is_empty());
        assert!(!empty.truncated);

        let db_path = dir.path().join("campaigns.sqlite");
        let mut store = CampaignStore::open(&db_path).map_err(std::io::Error::other)?;
        let empty_table = read_campaign_index(&db_path).map_err(std::io::Error::other)?;
        assert!(empty_table.campaigns.is_empty());

        let hash = ContentId::from_blake3_hex("a".repeat(64)).map_err(|_| "invalid hash")?;
        let make_contract = |id: &str| CampaignContract {
            schema: CAMPAIGN_SCHEMA.to_owned(),
            campaign_id: id.to_owned(),
            revision: 1,
            repository: "/repo".to_owned(),
            members: vec![MemberBinding {
                selector: MemberSelector::WholeChange {
                    change: "chg-1".to_owned(),
                },
                prerequisite_inclusion: false,
                profile: WorkflowProfile::SpecDriven,
                profile_version: hash.clone(),
                policy: hash.clone(),
                repository_instructions: hash.clone(),
                integration_policy: hash.clone(),
                goal: CompletionGoal::VerifiedCandidate,
                required_checks: Vec::new(),
                required_reviews: Vec::new(),
                context: Vec::new(),
            }],
            max_concurrency: 1,
            max_outstanding_decisions: 4,
            parent_budget_id: hash.clone(),
            agent_definition: hash.clone(),
            require_one_writer_per_repository: true,
        };
        store.start(make_contract("camp-done")).map_err(std::io::Error::other)?;
        store.cancel("camp-done", 1).map_err(std::io::Error::other)?;
        store.start(make_contract("camp-older")).map_err(std::io::Error::other)?;
        store.start(make_contract("camp-newer")).map_err(std::io::Error::other)?;
        let mut newer_state = store.inspect("camp-newer").map_err(std::io::Error::other)?.ok_or("missing")?;
        newer_state.decisions.insert(
            hash.clone(),
            HumanDecisionRequest {
                identity: hash.clone(),
                kind: DecisionKind::Preference,
                member: "change:chg-1".to_owned(),
                work_identity: None,
                decision_revision: None,
                policy_identity: hash.clone(),
                candidate_identity: None,
                state: DecisionRequestState::Outstanding,
                created_ms: 9_000,
                question: "Proceed?".to_owned(),
                options: vec!["yes".to_owned()],
                recommendation: None,
                priority: 1,
                pending_reason: None,
                resolved_answer: None,
                active_effort_ms: None,
            },
        );
        let newer_bytes = serde_json::to_vec(&newer_state)?;
        let write_conn = Connection::open(&db_path)?;
        write_conn.execute(
            "UPDATE implementation_campaign SET version = 2, bytes = ?1 WHERE campaign_id = 'camp-newer'",
            params![newer_bytes],
        )?;
        let index = read_campaign_index(&db_path).map_err(std::io::Error::other)?;
        assert_eq!(index.campaigns.len(), 3);
        assert_eq!(index.campaigns[0].campaign_id, "camp-newer");
        assert_eq!(index.campaigns[0].pending_decisions, 1);
        assert_eq!(index.campaigns[0].updated_at, Some(9_000));
        assert_eq!(index.campaigns[0].status, "waiting");
        assert_eq!(index.campaigns[1].campaign_id, "camp-older");
        assert_eq!(index.campaigns[2].campaign_id, "camp-done");
        assert_eq!(index.campaigns[2].status, "cancelled");

        write_conn.execute(
            "UPDATE implementation_campaign SET bytes = ?1 WHERE campaign_id = 'camp-older'",
            params![b"{bad json".to_vec()],
        )?;
        assert_eq!(read_campaign_index(&db_path).err(), Some("campaign_projection_record_invalid"));
        Ok(())
    }

    // r[verify kiln.implementation.campaign.unified_attention_and_recovery]
    #[test]
    fn unified_attention_aggregates_all_six_kinds_and_survives_unavailable_fleet()
    -> Result<(), Box<dyn std::error::Error>> {
        use kiln_core::fleet::{
            ClaimState, FleetAgent, FleetScope, MirrorStatus, ScopePath, FleetTarget,
        };
        use kiln_core::implementation::campaign::{
            CAMPAIGN_SCHEMA, CampaignContract, CompletionGoal, DecisionKind, HumanDecisionRequest,
            MemberBinding, MemberSelector, WorkflowProfile,
        };
        use kiln_core::implementation::{
            AcceptanceObligation, Enrollment, IMPLEMENTATION_WORK_SCHEMA, ImplementationPlan,
            ImplementationWork, PlanReviewCommand, PlanReviewPolicy, PlanStep, SourceRef,
            TerminalPolicy, WorkBudget, WorkSourceKind,
        };
        use kiln_core::types::{AttemptId, CandidateId, EffectId, PipelineRevisionId, PolicyId, ProviderProfileId};
        use kiln_core::workflow::{WorkflowControls, WorkflowEffectPlan};

        let dir = tempfile::tempdir()?;
        let db_path = dir.path().join("work.sqlite");
        let hash = |ch: &str| ContentId::from_blake3_hex(ch.repeat(64)).unwrap();

        let enrollment = Enrollment {
            repositories: BTreeSet::from(["onix/kiln".to_owned(), "onix/other".to_owned()]),
            actors: BTreeSet::from(["local:operator".to_owned()]),
        };
        let mut implementer = Implementer::open_with_local_operator(&db_path, enrollment.clone(), "local:operator")?;
        let work_item = ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_owned(),
            revision: 1,
            actor: "local:operator".to_owned(),
            repository: "onix/kiln".to_owned(),
            project: "kiln".to_owned(),
            base: None,
            title: "Task 1 plan gate".to_owned(),
            requested_outcome: "deliver".to_owned(),
            acceptance: vec![AcceptanceObligation {
                id: "a1".into(),
                statement: "test".into(),
            }],
            dependencies: Vec::new(),
            aliases: vec![SourceRef {
                kind: WorkSourceKind::CairnTask,
                source: ExternalRef::new("cairn", "task", "chg-1/t1").unwrap(),
                native_id: "chg-1/t1".into(),
                version: hash("1"),
            }],
            permitted_paths: vec!["crates".into()],
            permitted_effects: BTreeSet::new(),
            verification_profile: hash("9"),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
        };
        let work_id = work_item.identity().map_err(|e| std::io::Error::other(e.message))?;
        implementer.admit(&work_item, None)?;
        let work2 = ImplementationWork {
            repository: "onix/other".to_owned(),
            title: "Task 2 unknown attempt".to_owned(),
            aliases: vec![SourceRef {
                kind: WorkSourceKind::CairnTask,
                source: ExternalRef::new("cairn", "task", "chg-2/t2").unwrap(),
                native_id: "chg-2/t2".into(),
                version: hash("2"),
            }],
            ..work_item
        };
        let _work2_id = work2.identity().map_err(|e| std::io::Error::other(e.message))?;
        implementer.admit(&work2, None)?;
        let claim2 = implementer.claim_next(1_000, 60_000)?.expect("claim2");
        let attempt2_id = claim2.identity.clone();
        let fence2 = claim2.fence;
        implementer.fail(&attempt2_id, fence2, 2_000, FailureClass::Unknown, "lost")?;

        implementer.plan_review(&PlanReviewCommand::Configure {
            policy: Some(PlanReviewPolicy {
                schema: kiln_core::implementation::PLAN_REVIEW_POLICY_SCHEMA.to_owned(),
                repositories: BTreeSet::from(["onix/kiln".to_owned()]),
                reviewers: BTreeSet::from(["local:operator".to_owned()]),
            }),
        })?;
        let plan = ImplementationPlan {
            schema: kiln_core::implementation::PLAN_SCHEMA.to_owned(),
            work_identity: work_id.clone(),
            revision: 1,
            previous: None,
            author: "agent:coder".into(),
            steps: vec![PlanStep {
                id: "step-1".into(),
                text: "Implement pure core".into(),
            }],
        };
        let plan_id = plan.identity().map_err(|e| std::io::Error::other(e.message))?;
        implementer.plan_review(&PlanReviewCommand::Propose { plan })?;

        let mut store = CampaignStore::open(&db_path)?;
        store.start(CampaignContract {
            schema: CAMPAIGN_SCHEMA.to_owned(),
            campaign_id: "camp-1".to_owned(),
            revision: 1,
            repository: "onix/kiln".to_owned(),
            members: vec![MemberBinding {
                selector: MemberSelector::WholeChange {
                    change: "chg-1".to_owned(),
                },
                prerequisite_inclusion: false,
                profile: WorkflowProfile::SpecDriven,
                profile_version: hash("a"),
                policy: hash("a"),
                repository_instructions: hash("a"),
                integration_policy: hash("a"),
                goal: CompletionGoal::VerifiedCandidate,
                required_checks: Vec::new(),
                required_reviews: Vec::new(),
                context: Vec::new(),
            }],
            max_concurrency: 1,
            max_outstanding_decisions: 4,
            parent_budget_id: hash("a"),
            agent_definition: hash("a"),
            require_one_writer_per_repository: true,
        })?;
        let mut cstate = store.inspect("camp-1")?.unwrap();
        cstate.decisions.insert(
            hash("d"),
            HumanDecisionRequest {
                identity: hash("d"),
                kind: DecisionKind::Preference,
                member: "change:chg-1".to_owned(),
                work_identity: Some(work_id.clone()),
                decision_revision: None,
                policy_identity: hash("a"),
                candidate_identity: None,
                state: DecisionRequestState::Outstanding,
                created_ms: 1_000,
                question: "Pick path?".to_owned(),
                options: vec!["A".into(), "B".into()],
                recommendation: Some("A".into()),
                priority: 1,
                pending_reason: None,
                resolved_answer: None,
                active_effort_ms: None,
            },
        );
        let cbytes = serde_json::to_vec(&cstate)?;
        Connection::open(&db_path)?.execute(
            "UPDATE implementation_campaign SET version = 2, bytes = ?1 WHERE campaign_id = 'camp-1'",
            params![cbytes],
        )?;

        let snap = ImplementationObservation::open(&db_path)
            .map_err(|e| std::io::Error::other(format!("{e:?}")))?
            .snapshot()
            .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
        let untrusted_run = RunView {
            id: "run-untrusted".into(),
            project_id: "kiln".into(),
            source_revision: "a".repeat(40),
            canonical_tree: hash("b"),
            repository: ExternalRef::new("github", "repo", "onix/kiln").unwrap(),
            revision: 1,
            attempt_id: AttemptId::new("att-1").unwrap(),
            outcome: kiln_core::domain::Outcome::Pending,
            phase: crate::engine::untrusted::AWAITING_APPROVAL.into(),
            execution: None,
            sequence: 0,
            nodes: Vec::new(),
            controls: WorkflowControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            error_code: None,
            superseded_by: None,
            superseded_revision: None,
            log_bounds: crate::engine::model::LogBounds {
                maximum_chunk_bytes: 1024,
                maximum_total_bytes: 4096,
                maximum_chunks: 4,
            },
            ordinal: 1,
            parent_run_id: None,
            auto_retry: None,
            selected_nodes: Vec::new(),
            edges: Vec::new(),
            created_at_ms: Some(1_000),
            started_at_ms: None,
            completed_at_ms: None,
            duration_ms: None,
            report_summary: Value::Null,
            step_telemetry: Vec::new(),
            dirty_snapshot: None,
            affected: None,
            project_root: None,
            scope_identity: None,
            untrusted: Some(kiln_core::untrusted::UntrustedSummary {
                state: kiln_core::untrusted::UntrustedState::AwaitingApproval,
                reason: kiln_core::untrusted::UntrustedReason::Fork,
                pull_request: 99,
                head_repository: "fork/kiln".into(),
                source_commit: "a".repeat(40),
                approved_by: None,
                approved_by_id: None,
                approved_commit: None,
                approved_at_ms: None,
                counts_for_required_ci: false,
                superseded_by: None,
            }),
            runbook: None,
            fanout_run_id: None,
        };
        let _ = (EffectId::new("eff-1"), CandidateId::new("cand-1"), PipelineRevisionId::new("pipe-1"), PolicyId::new("pol-1"), ProviderProfileId::new("prov-1"), std::mem::size_of::<WorkflowEffectPlan>());

        let now_ms = 5_000;
        let fleet_ok = FleetAttentionInput {
            health: SourceHealth {
                configured: true,
                available: true,
                error: None,
            },
            decisions: vec![FleetDecision {
                identity: hash("e"),
                session: hash("f"),
                attempt: attempt2_id.clone(),
                question: "Continue?".into(),
                raised_ms: 2_000,
                answer: None,
                closed_ms: None,
            }],
            claims: vec![FleetClaim {
                identity: hash("c"),
                scope: FleetScope {
                    target: FleetTarget::Repository("onix/kiln".into()),
                    path: ScopePath::Whole,
                },
                holder: FleetAgent {
                    tool: "omp".into(),
                    host: "aspen3".into(),
                    session: "s1".into(),
                },
                purpose: "work".into(),
                fence: 1,
                granted_ms: 1_000,
                expires_ms: now_ms + 60_000,
                state: ClaimState::Open,
                attempt: None,
                claim_mirror: MirrorStatus::Pending,
                release_mirror: MirrorStatus::Pending,
            }],
            foreign_claims: vec![
                ForeignClaimView {
                    scope: FleetScope {
                        target: FleetTarget::Repository("onix/kiln".into()),
                        path: ScopePath::Path("crates".into()),
                    },
                    scope_text: "onix/kiln crates".into(),
                    message_id: "<1@swarm>".into(),
                    author: "other-bot".into(),
                    holder: "other".into(),
                    date_unix_ms: 1_500,
                    age_ms: 3_500,
                    stale_after_ms: 60_000,
                },
                // Unrelated repo without overlap or local project must not spam inbox.
                ForeignClaimView {
                    scope: FleetScope {
                        target: FleetTarget::Repository("unrelated/repo".into()),
                        path: ScopePath::Whole,
                    },
                    scope_text: "unrelated/repo .".into(),
                    message_id: "<2@swarm>".into(),
                    author: "someone".into(),
                    holder: "someone".into(),
                    date_unix_ms: 1_500,
                    age_ms: 3_500,
                    stale_after_ms: 60_000,
                },
            ],
            attempt_hints: BTreeMap::new(),
        };

        let proj = assemble_attention(
            now_ms,
            CiAttentionInput {
                health: SourceHealth {
                    configured: true,
                    available: true,
                    error: None,
                },
                runs: vec![untrusted_run],
                repositories: vec!["onix/kiln".into()],
            },
            ImplAttentionInput {
                health: SourceHealth {
                    configured: true,
                    available: true,
                    error: None,
                },
                snapshot: Some(snap.clone()),
            },
            fleet_ok,
        );
        assert_eq!(proj.counts.untrusted_run_approval, 1);
        assert_eq!(proj.counts.fleet_session_decision, 1);
        assert_eq!(proj.counts.campaign_decision, 1);
        assert_eq!(proj.counts.plan_review_gate, 1);
        assert_eq!(proj.counts.unknown_termination_or_dispatch, 1);
        assert_eq!(proj.counts.expiring_or_conflict_claim, 2);
        assert_eq!(proj.total, 7);
        let plan_item = proj
            .items
            .iter()
            .find(|item| item.kind == AttentionKind::PlanReviewGate)
            .expect("plan item");
        assert_eq!(plan_item.target.plan_id.as_deref(), Some(plan_id.as_str()));
        assert_eq!(plan_item.context_preview["steps"][0]["id"], "step-1");
        assert!(plan_item.allowed_actions.contains(&"accept_plan"));

        // Fleet unavailable still keeps CI + implementation items visible.
        let degraded = assemble_attention(
            now_ms,
            CiAttentionInput {
                health: SourceHealth {
                    configured: true,
                    available: true,
                    error: None,
                },
                runs: Vec::new(),
                repositories: Vec::new(),
            },
            ImplAttentionInput {
                health: SourceHealth {
                    configured: true,
                    available: true,
                    error: None,
                },
                snapshot: Some(snap),
            },
            FleetAttentionInput {
                health: SourceHealth {
                    configured: true,
                    available: false,
                    error: Some("fleet_unavailable".into()),
                },
                decisions: Vec::new(),
                claims: Vec::new(),
                foreign_claims: Vec::new(),
                attempt_hints: BTreeMap::new(),
            },
        );
        assert!(!degraded.sources.fleet.available);
        assert_eq!(degraded.sources.fleet.error.as_deref(), Some("fleet_unavailable"));
        assert_eq!(degraded.counts.plan_review_gate, 1);
        assert_eq!(degraded.counts.unknown_termination_or_dispatch, 1);
        assert_eq!(degraded.counts.campaign_decision, 1);

        Ok(())
    }

    // r[verify kiln.implementation.campaign.unified_attention_and_recovery]
    #[test]
    fn wake_reconcile_termination_and_plan_review_validate_identities_and_roles()
    -> Result<(), Box<dyn std::error::Error>> {
        use kiln_core::implementation::campaign::{
            CAMPAIGN_SCHEMA, CampaignContract, CompletionGoal, MemberBinding, MemberSelector,
            WorkflowProfile,
        };
        use kiln_core::implementation::{
            AcceptanceObligation, Enrollment, IMPLEMENTATION_WORK_SCHEMA, ImplementationPlan,
            ImplementationWork, PlanReviewCommand, PlanReviewPolicy, PlanStep, SourceRef,
            TerminalPolicy, WorkBudget, WorkSourceKind,
        };

        let dir = tempfile::tempdir()?;
        let db_path = dir.path().join("impl.sqlite");
        let hash = |ch: &str| ContentId::from_blake3_hex(ch.repeat(64)).unwrap();

        let enrollment = Enrollment {
            repositories: BTreeSet::from(["onix/kiln".to_owned()]),
            actors: BTreeSet::from(["local:operator".to_owned()]),
        };
        let mut implementer = Implementer::open_with_local_operator(&db_path, enrollment, "local:operator")?;
        let work_item = ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_owned(),
            revision: 1,
            actor: "local:operator".to_owned(),
            repository: "onix/kiln".to_owned(),
            project: "kiln".to_owned(),
            base: None,
            title: "Work 1".to_owned(),
            requested_outcome: "deliver".to_owned(),
            acceptance: vec![AcceptanceObligation {
                id: "a1".into(),
                statement: "test".into(),
            }],
            dependencies: Vec::new(),
            aliases: vec![SourceRef {
                kind: WorkSourceKind::CairnTask,
                source: ExternalRef::new("cairn", "task", "chg-1/t1").unwrap(),
                native_id: "chg-1/t1".into(),
                version: hash("1"),
            }],
            permitted_paths: vec!["crates".into()],
            permitted_effects: BTreeSet::new(),
            verification_profile: hash("9"),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
        };
        let work_id = work_item.identity().map_err(|e| std::io::Error::other(e.message))?;
        implementer.admit(&work_item, None)?;
        let claim = implementer.claim_next(1_000, 60_000)?.expect("claim");
        let attempt_id = claim.identity.clone();
        let fence = claim.fence;
        implementer.fail(&attempt_id, fence, 2_000, FailureClass::Unknown, "ambiguous")?;

        implementer.plan_review(&PlanReviewCommand::Configure {
            policy: Some(PlanReviewPolicy {
                schema: kiln_core::implementation::PLAN_REVIEW_POLICY_SCHEMA.to_owned(),
                repositories: BTreeSet::from(["onix/kiln".to_owned()]),
                reviewers: BTreeSet::from(["local:operator".to_owned()]),
            }),
        })?;
        let plan = ImplementationPlan {
            schema: kiln_core::implementation::PLAN_SCHEMA.to_owned(),
            work_identity: work_id.clone(),
            revision: 1,
            previous: None,
            author: "agent:coder".into(),
            steps: vec![PlanStep {
                id: "step-1".into(),
                text: "Ship".into(),
            }],
        };
        let plan_id = plan.identity().map_err(|e| std::io::Error::other(e.message))?;
        implementer.plan_review(&PlanReviewCommand::Propose { plan })?;

        let mut store = CampaignStore::open(&db_path)?;
        store.start(CampaignContract {
            schema: CAMPAIGN_SCHEMA.to_owned(),
            campaign_id: "camp-wake".to_owned(),
            revision: 1,
            repository: "onix/kiln".to_owned(),
            members: vec![MemberBinding {
                selector: MemberSelector::WholeChange {
                    change: "chg-1".to_owned(),
                },
                prerequisite_inclusion: false,
                profile: WorkflowProfile::SpecDriven,
                profile_version: hash("a"),
                policy: hash("a"),
                repository_instructions: hash("a"),
                integration_policy: hash("a"),
                goal: CompletionGoal::VerifiedCandidate,
                required_checks: Vec::new(),
                required_reviews: Vec::new(),
                context: Vec::new(),
            }],
            max_concurrency: 1,
            max_outstanding_decisions: 4,
            parent_budget_id: hash("a"),
            agent_definition: hash("a"),
            require_one_writer_per_repository: true,
        })?;

        let repo_dir = dir.path().join("repo");
        git2::Repository::init(&repo_dir)?;
        let op_tok = dir.path().join("op.token");
        let wk_tok = dir.path().join("wk.token");
        std::fs::write(&op_tok, "a".repeat(64))?;
        std::fs::write(&wk_tok, "b".repeat(64))?;
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&op_tok, std::fs::Permissions::from_mode(0o400))?;
            std::fs::set_permissions(&wk_tok, std::fs::Permissions::from_mode(0o400))?;
        }
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        std::fs::write(&cert_path, "dummy")?;
        std::fs::write(&key_path, "dummy")?;
        let profile: crate::profile::ServiceProfile = serde_json::from_value(serde_json::json!({
            "schema": "kiln.web-service.v1",
            "service_id": "inv-test",
            "listen_address": "127.0.0.1:9443",
            "origin": "https://localhost:9443",
            "tls": {"certificate_path": cert_path, "private_key_path": key_path, "min_version": "TLS1.3"},
            "operator_token_file": op_tok,
            "worker_token_file": wk_tok,
            "state_directory": dir.path().join("state"),
            "projects": [{
                "id": "fixture",
                "label": "Fixture",
                "repository_path": repo_dir,
                "reference": "refs/heads/main",
                "source_limits": {"max_entries": 32, "max_path_bytes": 1024, "max_file_bytes": 4096, "max_total_bytes": 16384},
                "workflow": {
                    "socket_path": dir.path().join("lattice.sock"),
                    "allowed_user_ids": [nix::unistd::geteuid().as_raw()],
                    "transport_limits": {"maximum_frame_bytes": 1048576, "maximum_log_bytes": 65536, "maximum_connections": 4, "read_deadline_milliseconds": 1000, "write_deadline_milliseconds": 1000},
                    "adapter_limits": {"maximum_request_bytes": 1048576, "maximum_response_bytes": 1048576},
                    "policy_id": "reviewed-policy",
                    "provider_profile_id": "reviewed-lattice",
                    "platform": "linux-x86_64",
                    "nodes": [{"id": "job", "kind": "test", "effect_class": "test", "adapter_kind": "local", "operation_identity": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],
                    "edges": [],
                    "controls": {"cancel": true, "pause": false, "resume": false},
                    "bounds": {"max_nodes": 8, "max_edges": 16, "max_log_references": 32, "max_log_read_bytes": 4096, "max_total_log_bytes": 65536}
                }
            }],
            "bounds": {
                "max_request_bytes": 16384, "max_response_bytes": 1048576, "max_header_bytes": 4096,
                "max_connections": 4, "timeout_seconds": 2, "poll_interval_ms": 10, "max_projects": 2,
                "max_runs": 4, "max_artifact_bytes": 1024, "max_artifacts": 2, "max_run_artifact_bytes": 2048,
                "max_total_artifact_bytes": 4096, "max_log_read_bytes": 4096, "max_requests": 32,
                "max_events_per_run": 8, "max_state_bytes": 1048576
            }
        }))?;
        let credentials = crate::auth::Credentials::load(&profile, crate::access::AccessProfile::default())
            .map_err(std::io::Error::other)?;
        let engine = std::sync::Mutex::new(crate::engine::Engine::open_with_ports(
            profile.clone(),
            BTreeMap::from([(
                "fixture".to_owned(),
                Box::new(DummyWorkflowPort(
                    profile.projects[0].execution_capabilities(profile.bounds.max_runs).map_err(std::io::Error::other)?
                )) as Box<dyn kiln_provider_api::WorkflowPort + Send>,
            )]),
        ).map_err(std::io::Error::other)?);
        let cobs = crate::cobs::Browser::new(None)?;
        let sources = crate::sources::Browser::new(None)?;
        let reviews = crate::reviews::Browser::new(None)?;
        let work = crate::work::Workspace::new(None)?;
        let code_review = crate::code_review::Workspace::new(None)?;
        let stop = std::sync::atomic::AtomicBool::new(false);
        let ctx = Context {
            profile: &profile,
            credentials: &credentials,
            engine: &engine,
            cobs: &cobs,
            sources: &sources,
            reviews: &reviews,
            work: &work,
            code_review: &code_review,
            inventory: None,
            implementation_work: Some(&db_path),
            campaign_controls: None,
            dispatch_recovery: None,
            native_runtime_evidence: None,
            fleet: None,
            delivery_observations: None,
            events: None,
            stop: &stop,
        };
        let operator = Principal {
            id: "local:operator".into(),
            label: "Op".into(),
            role: Role::Operator,
            agent: None,
        };
        let viewer = Principal {
            id: "local:viewer".into(),
            label: "Viewer".into(),
            role: Role::Viewer,
            agent: None,
        };
        let worker = Principal {
            id: "local:worker".into(),
            label: "Worker".into(),
            role: Role::Worker,
            agent: None,
        };

        // Attention denies Worker, allows Viewer.
        let req = Request {
            method: "GET".into(),
            target: "/api/implementation/attention".into(),
            body: Vec::new(),
            headers: BTreeMap::new(),
        };
        assert_eq!(attention_response(&req, &ctx, &worker).status, 403);
        assert_eq!(attention_response(&req, &ctx, &viewer).status, 200);

        // Wake denies Viewer, invalid kind, missing campaign; succeeds for Operator.
        let wake_req = |target: &str, body: Value| Request {
            method: "POST".into(),
            target: target.into(),
            body: serde_json::to_vec(&body).unwrap(),
            headers: BTreeMap::new(),
        };
        assert_eq!(
            campaign_wake_response(
                &wake_req("/api/implementation/campaigns/camp-wake/wake", json!({"kind":"operator"})),
                &ctx,
                &viewer
            )
            .status,
            403
        );
        assert_eq!(
            campaign_wake_response(
                &wake_req("/api/implementation/campaigns/camp-wake/wake", json!({"kind":"bogus"})),
                &ctx,
                &operator
            )
            .status,
            400
        );
        assert_eq!(
            campaign_wake_response(
                &wake_req("/api/implementation/campaigns/missing/wake", json!({"kind":"operator"})),
                &ctx,
                &operator
            )
            .status,
            404
        );
        let wake_ok = campaign_wake_response(
            &wake_req(
                "/api/implementation/campaigns/camp-wake/wake",
                json!({"kind":"operator","reason":"unblock","request_id":"req-wake-1"}),
            ),
            &ctx,
            &operator,
        );
        assert_eq!(wake_ok.status, 200);

        // Plan review rejects unauthorized reviewer, accepts operator reviewer.
        let plan_req = |body: Value| Request {
            method: "POST".into(),
            target: "/api/implementation/plan-review".into(),
            body: serde_json::to_vec(&body).unwrap(),
            headers: BTreeMap::new(),
        };
        let other_op = Principal {
            id: "local:other".into(),
            label: "Other".into(),
            role: Role::Operator,
            agent: None,
        };
        assert_eq!(
            plan_review_response(
                &plan_req(json!({"work_id":work_id,"plan_id":plan_id,"verdict":"accepted","request_id":"req-pr-1"})),
                &ctx,
                &other_op
            )
            .status,
            403
        );
        let pr_ok = plan_review_response(
            &plan_req(json!({"work_id":work_id,"plan_id":plan_id,"verdict":"accepted","request_id":"req-pr-2"})),
            &ctx,
            &operator,
        );
        assert_eq!(pr_ok.status, 200);

        // Reconcile-termination fails closed on mismatched work/fence/campaign, succeeds on exact match.
        let rec_req = |body: Value| Request {
            method: "POST".into(),
            target: "/api/implementation/reconcile-termination".into(),
            body: serde_json::to_vec(&body).unwrap(),
            headers: BTreeMap::new(),
        };
        assert_eq!(
            reconcile_termination_response(
                &rec_req(json!({"campaign_id":"camp-wake","work_id":hash("9"),"attempt_id":attempt_id,"fence":fence,"request_id":"req-rt-1"})),
                &ctx,
                &operator
            )
            .status,
            409
        );
        assert_eq!(
            reconcile_termination_response(
                &rec_req(json!({"campaign_id":"camp-wake","work_id":work_id,"attempt_id":attempt_id,"fence":fence + 1,"request_id":"req-rt-2"})),
                &ctx,
                &operator
            )
            .status,
            409
        );
        assert_eq!(
            reconcile_termination_response(
                &rec_req(json!({"campaign_id":"missing-camp","work_id":work_id,"attempt_id":attempt_id,"fence":fence,"request_id":"req-rt-3"})),
                &ctx,
                &operator
            )
            .status,
            404
        );
        let rec_ok = reconcile_termination_response(
            &rec_req(json!({"campaign_id":"camp-wake","work_id":work_id,"attempt_id":attempt_id,"fence":fence,"request_id":"req-rt-4"})),
            &ctx,
            &operator,
        );
        assert_eq!(rec_ok.status, 200);
        let rec_json: Value = serde_json::from_slice(&rec_ok.body)?;
        assert_eq!(rec_json["reconciled"], true);
        assert_eq!(rec_json["campaign_woken"], true);

        // Conflicting termination on replay fails closed (409).
        assert_eq!(
            reconcile_termination_response(
                &rec_req(json!({
                    "campaign_id":"camp-wake",
                    "work_id":work_id,
                    "attempt_id":attempt_id,
                    "fence":fence,
                    "request_id":"req-rt-5",
                    "termination":{"contract_version":1,"provider_namespace":"lattice","native_kind":"termination","native_id":"different"}
                })),
                &ctx,
                &operator
            )
            .status,
            409
        );
        Ok(())
    }

    pub(crate) struct DummyWorkflowPort(pub(crate) kiln_core::provider::CapabilityProfile);
    impl kiln_provider_api::WorkflowPort for DummyWorkflowPort {
        fn profile(&self) -> &kiln_core::provider::CapabilityProfile {
            &self.0
        }
        fn dispatch(&mut self, _: &kiln_core::workflow::WorkflowEffectPlan) -> kiln_provider_api::PortResult<kiln_core::workflow::WorkflowObservation> {
            unreachable!()
        }
        fn recover_dispatch(&mut self, _: &kiln_core::workflow::WorkflowEffectPlan) -> kiln_provider_api::PortResult<kiln_core::workflow::WorkflowObservation> {
            unreachable!()
        }
        fn observe(&mut self, _: &kiln_core::types::EffectId) -> kiln_provider_api::PortResult<kiln_core::workflow::WorkflowObservation> {
            unreachable!()
        }
        fn control(&mut self, _: &kiln_core::workflow::WorkflowControlRequest) -> kiln_provider_api::PortResult<kiln_core::workflow::WorkflowObservation> {
            unreachable!()
        }
        fn reconcile(&mut self, _: &kiln_core::types::EffectId) -> kiln_provider_api::PortResult<kiln_core::workflow::WorkflowObservation> {
            unreachable!()
        }
        fn read_log(&mut self, _: &kiln_core::workflow::WorkflowLogReadRequest) -> kiln_provider_api::PortResult<kiln_core::workflow::WorkflowLogChunk> {
            unreachable!()
        }
    }
}
