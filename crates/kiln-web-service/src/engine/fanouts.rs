use std::collections::{BTreeMap, BTreeSet};

use base64::Engine as _;
use kiln_core::domain::Outcome;
use kiln_core::fleet::{ClaimConflict, FleetClaim, FleetScope, overlaps, parse_scope};
use kiln_core::implementation::{
    FANOUT_RUN_SCHEMA, FanOutRun, FanoutAdmissionState, FanoutBudget, FanoutChildStatus,
    FanoutClaimPolicy, FanoutMode, MAX_FANOUT_BUDGET_UNITS, MAX_FANOUT_CONCURRENCY,
    MAX_FANOUT_RUNS, find_claim_overlap,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::api::{decoded, error, json_response};
use super::model::RequestKind;
use super::queue::resolve_source;
use super::runbooks::{
    replay_response_with_flag, resolve_sha_target_source, target_allows_reference,
};
use super::{ApiResponse, Engine, host_time_ms, identity};
use crate::access::Principal;
use crate::fanouts::{
    FANOUT_CONTROL_RESULT_SCHEMA, FANOUT_DISPATCH_RESULT_SCHEMA, FANOUT_LIST_SCHEMA,
    FanoutControlRequest, FanoutControlResponse, FanoutDispatchRequest, FanoutDispatchResponse,
    FanoutPage, expand_fanout_children, fanout_mode, fanout_view, valid_fanout_id,
};
use crate::runbooks::{RunbookMode, RunbookRunBinding, resolve_params, valid_runbook_id};

#[derive(Clone, Debug, Default)]
pub enum FleetClaimsSnapshot {
    #[default]
    Unconfigured,
    Available(Vec<FleetClaim>),
    Unavailable,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FanoutCursor {
    version: u8,
    snapshot: u64,
    ordinal: u64,
    id: String,
    query: String,
    order: String,
}

impl Engine {
    pub fn set_fleet_claims_snapshot(&mut self, snapshot: FleetClaimsSnapshot) {
        self.fleet_claims = snapshot;
    }

    pub(super) fn list_fanouts(
        &self,
        params: &BTreeMap<&str, &str>,
    ) -> Result<ApiResponse, String> {
        if params.keys().any(|k| {
            !matches!(
                *k,
                "runbook_id" | "admission" | "order" | "limit" | "cursor"
            )
        }) {
            return error(400, "query_invalid");
        }
        let mut values = BTreeMap::new();
        for (key, value) in params {
            if *key != "cursor" {
                let decoded_val = match decoded(value) {
                    Ok(v) if v.len() <= 256 && !v.bytes().any(|b| b.is_ascii_control()) => v,
                    _ => return error(400, "query_invalid"),
                };
                values.insert(*key, decoded_val);
            }
        }
        let order = values.get("order").map(String::as_str).unwrap_or("desc");
        if !matches!(order, "asc" | "desc") {
            return error(400, "order_invalid");
        }
        let limit = match values.get("limit").map(|v| v.parse::<usize>()).transpose() {
            Ok(Some(v)) if (1..=100).contains(&v) => v,
            Ok(None) => 50,
            _ => return error(400, "page_bound"),
        };
        let runbook_filter = values.get("runbook_id").map(String::as_str);
        if runbook_filter.is_some_and(|id| !valid_runbook_id(id)) {
            return error(400, "runbook_id_invalid");
        }
        let admission_filter = match values.get("admission").map(String::as_str) {
            None => None,
            Some("open") => Some(FanoutAdmissionState::Open),
            Some("paused") => Some(FanoutAdmissionState::Paused),
            Some("cancelled") => Some(FanoutAdmissionState::Cancelled),
            Some(_) => return error(400, "fanout_admission_invalid"),
        };
        let fingerprint = identity(&values)?.as_str().to_owned();
        let cursor = if let Some(raw) = params.get("cursor") {
            let decoded_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw);
            let cursor: FanoutCursor = match decoded_bytes
                .ok()
                .filter(|v| v.len() <= 4096)
                .and_then(|v| serde_json::from_slice(&v).ok())
            {
                Some(v) => v,
                None => return error(400, "cursor_invalid"),
            };
            if cursor.version != 1
                || cursor.query != fingerprint
                || cursor.order != order
                || cursor.ordinal > cursor.snapshot
                || self
                    .state
                    .fanouts
                    .get(&cursor.id)
                    .is_none_or(|f| f.ordinal != cursor.ordinal)
            {
                return error(400, "cursor_invalid");
            }
            Some(cursor)
        } else {
            None
        };
        let snapshot = cursor.as_ref().map(|c| c.snapshot).unwrap_or_else(|| {
            self.state
                .fanouts
                .values()
                .map(|f| f.ordinal)
                .max()
                .unwrap_or(0)
        });
        let mut items: Vec<&FanOutRun> = self
            .state
            .fanouts
            .values()
            .filter(|f| f.ordinal <= snapshot)
            .filter(|f| runbook_filter.is_none_or(|id| f.runbook_id == id))
            .filter(|f| admission_filter.is_none_or(|adm| f.admission == adm))
            .collect();
        let total = items.len();
        items.sort_by(|left, right| {
            let ord = left
                .ordinal
                .cmp(&right.ordinal)
                .then(left.id.cmp(&right.id));
            if order == "desc" { ord.reverse() } else { ord }
        });
        if let Some(c) = &cursor {
            items.retain(|f| {
                if order == "desc" {
                    (f.ordinal, f.id.as_str()) < (c.ordinal, c.id.as_str())
                } else {
                    (f.ordinal, f.id.as_str()) > (c.ordinal, c.id.as_str())
                }
            });
        }
        let has_more = items.len() > limit;
        let page_items: Vec<&FanOutRun> = items.into_iter().take(limit).collect();
        let next_cursor = if has_more {
            page_items.last().map(|last| {
                let c = FanoutCursor {
                    version: 1,
                    snapshot,
                    ordinal: last.ordinal,
                    id: last.id.clone(),
                    query: fingerprint.clone(),
                    order: order.to_owned(),
                };
                base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(serde_json::to_vec(&c).unwrap_or_default())
            })
        } else {
            None
        };
        json_response(
            200,
            FanoutPage {
                schema: FANOUT_LIST_SCHEMA,
                fanouts: page_items.into_iter().map(fanout_view).collect(),
                next_cursor,
                total,
                bounded: true,
            },
        )
    }

    pub(super) fn get_fanout(&self, fanout_id: &str) -> Result<ApiResponse, String> {
        if !valid_fanout_id(fanout_id) {
            return error(400, "fanout_id_invalid");
        }
        let Some(fanout) = self.state.fanouts.get(fanout_id) else {
            return error(404, "fanout_not_found");
        };
        json_response(200, fanout_view(fanout))
    }

    pub(super) fn dispatch_fanout(
        &mut self,
        id: &str,
        body: &[u8],
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        if !valid_runbook_id(id) {
            return error(400, "runbook_id_invalid");
        }
        let runbook = match self.authorize_runbook_principal(id, principal)? {
            Ok(runbook) => runbook,
            Err(response) => return Ok(response),
        };
        if let Err(response) = self.verify_configured_runbook_bundle(&runbook)? {
            return Ok(response);
        }

        let req: FanoutDispatchRequest = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "request_invalid"),
        };
        if !crate::profile::identifier(&req.request_id) {
            return error(400, "request_id_invalid");
        }
        let fingerprint = identity(&("kiln.runbook-fanout.v1", id, &principal.id, &req))?;
        if let Some(response) = self.request_replay(&req.request_id, &fingerprint)? {
            return Ok(response);
        }
        if self.state.requests.len() >= self.profile.bounds.max_requests {
            return error(409, "request_retention");
        }
        if self.state.fanouts.len() >= MAX_FANOUT_RUNS {
            return error(409, "fanout_retention");
        }

        let max_concurrency = req.max_concurrency.unwrap_or_else(|| {
            runbook
                .fanout
                .as_ref()
                .map_or(1, |cfg| cfg.default_max_concurrency)
        });
        let max_concurrency_limit = runbook
            .fanout
            .as_ref()
            .map_or(MAX_FANOUT_CONCURRENCY, |cfg| cfg.max_concurrency_limit);
        if max_concurrency == 0 || max_concurrency > max_concurrency_limit {
            return error(400, "fanout_concurrency_invalid");
        }

        let budget_cap = req.budget_cap.unwrap_or_else(|| {
            runbook
                .fanout
                .as_ref()
                .map_or(16, |cfg| cfg.default_budget_cap)
        });
        let max_budget_cap = runbook
            .fanout
            .as_ref()
            .map_or(MAX_FANOUT_BUDGET_UNITS, |cfg| cfg.max_budget_cap);
        if budget_cap == 0 || budget_cap > max_budget_cap {
            return error(400, "fanout_budget_invalid");
        }
        let budget = match FanoutBudget::new(budget_cap, 1) {
            Ok(b) => b,
            Err(_) => return error(400, "fanout_budget_invalid"),
        };
        let claim_policy = req.claim_policy.unwrap_or_default();

        let resolved_params = match resolve_params(&runbook.params, &req.params) {
            Ok(resolved) => resolved,
            Err(err) => return error(400, err.code),
        };

        let (selectors, mut children) = match expand_fanout_children(
            crate::fanouts::ExpandTargetRequest {
                runbook: &runbook,
                service: &self.profile,
                enrolled: &self.state.enrolled,
                targets: &req.targets,
                target_globs: &req.target_globs,
                scope_path: req.scope_path.as_deref(),
            },
            |project, reference| {
                resolve_source(project, reference, None).map_err(|code| {
                    let status = if code == "source_unavailable" {
                        503
                    } else {
                        400
                    };
                    let static_code = match code.as_str() {
                        "source_unavailable" => "source_unavailable",
                        "source_reference_not_admitted" => "source_reference_not_admitted",
                        "source_revision_invalid" => "source_revision_invalid",
                        "source_revision_unreachable" => "source_revision_unreachable",
                        _ => "source_unavailable",
                    };
                    (status, static_code)
                })
            },
            resolve_sha_target_source,
            target_allows_reference,
        ) {
            Ok(expanded) => expanded,
            Err((status, code)) => return error(status, code),
        };

        let now_ms = host_time_ms()?;
        if runbook.mode == RunbookMode::Mutating {
            let live_claims = match &self.fleet_claims {
                FleetClaimsSnapshot::Unavailable => return error(503, "fleet_unavailable"),
                FleetClaimsSnapshot::Unconfigured => &[][..],
                FleetClaimsSnapshot::Available(claims) => claims.as_slice(),
            };
            for child in &mut children {
                if let Some(scope) = &child.write_scope
                    && let Some(conflict) = find_claim_overlap(scope, live_claims, now_ms)
                {
                    match claim_policy {
                        FanoutClaimPolicy::Refuse => {
                            return json_response(
                                409,
                                json!({
                                    "code": "fleet_claim_conflict",
                                    "message": format!(
                                        "Target {} write scope {} conflicts with active fleet claim {}",
                                        child.target,
                                        scope.text(),
                                        conflict_summary(&conflict),
                                    ),
                                }),
                            );
                        }
                        FanoutClaimPolicy::Skip => {
                            child.status = FanoutChildStatus::Skipped;
                            child.reason = Some(format!(
                                "fleet_claim_conflict:{}",
                                conflict_scope(&conflict)
                            ));
                        }
                        FanoutClaimPolicy::Serialize => {}
                    }
                }
            }
        }

        let fanout_hash = identity(&(
            FANOUT_RUN_SCHEMA,
            &self.profile.service_id,
            &runbook.id,
            &principal.id,
            &req.request_id,
            &selectors,
        ))?;
        let fanout_id = format!("rb-fanout-{}", fanout_hash.as_str());
        let ordinal = self
            .state
            .version
            .checked_add(1)
            .ok_or("fanout ordinal overflow")?;
        let params_json: BTreeMap<String, serde_json::Value> = resolved_params
            .values
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        let mut fanout = FanOutRun {
            schema: FANOUT_RUN_SCHEMA.to_owned(),
            id: fanout_id.clone(),
            ordinal,
            runbook_id: runbook.id.clone(),
            mode: fanout_mode(runbook.mode),
            principal_id: principal.id.clone(),
            request_id: req.request_id.clone(),
            target_selectors: selectors,
            max_concurrency,
            budget,
            claim_policy,
            admission: kiln_core::implementation::FanoutAdmissionState::Open,
            phase: kiln_core::implementation::FanoutPhase::Active,
            stop_reason: None,
            lattice_bundle_id: runbook.lattice.bundle_id.clone(),
            lattice_bundle_identity: runbook.lattice.bundle_identity.clone(),
            lattice_workflow_name: runbook.lattice.workflow_name.clone(),
            lattice_workflow_revision: runbook.lattice.workflow_revision.clone(),
            params_identity: resolved_params.identity,
            params: params_json,
            env_bindings: resolved_params.env_bindings,
            children,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            completed_at_ms: None,
        };
        fanout.recompute_phase(now_ms);
        fanout
            .validate()
            .map_err(|diag| format!("fanout state invalid: {}", diag.code))?;

        self.state.fanouts.insert(fanout_id.clone(), fanout);
        self.reserve_request(
            req.request_id.clone(),
            fingerprint,
            fanout_id.clone(),
            RequestKind::FanoutDispatch,
        )?;
        self.advance_fanouts()?;
        self.dispatch_queued()?;
        self.sync_fanouts()?;

        let stored = self
            .state
            .fanouts
            .get(&fanout_id)
            .ok_or("fanout missing after admission")?;
        let view = fanout_view(stored);
        let status = if view.counts.pending > 0 || view.counts.active > 0 {
            202
        } else {
            201
        };
        let response = FanoutDispatchResponse {
            schema: FANOUT_DISPATCH_RESULT_SCHEMA,
            request_id: req.request_id.clone(),
            replayed: false,
            fanout_id,
            fanout: view,
        };
        let encoded =
            serde_json::to_value(response).map_err(|_| "fanout dispatch encoding failed")?;
        self.finish_request(&req.request_id, status, encoded)
    }

    pub(super) fn control_fanout(
        &mut self,
        fanout_id: &str,
        action: &'static str,
        body: &[u8],
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        if !valid_fanout_id(fanout_id) {
            return error(400, "fanout_id_invalid");
        }
        let Some(existing) = self.state.fanouts.get(fanout_id).cloned() else {
            return error(404, "fanout_not_found");
        };
        let runbook = match self.authorize_runbook_principal(&existing.runbook_id, principal)? {
            Ok(runbook) => runbook,
            Err(response) => return Ok(response),
        };

        let req: FanoutControlRequest = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "request_invalid"),
        };
        if !crate::profile::identifier(&req.request_id) {
            return error(400, "request_id_invalid");
        }
        let fingerprint = identity(&(
            "kiln.runbook-fanout-control.v1",
            fanout_id,
            action,
            &principal.id,
            &req,
        ))?;
        if let Some(response) = self.request_replay(&req.request_id, &fingerprint)? {
            return Ok(response);
        }
        if self.state.requests.len() >= self.profile.bounds.max_requests {
            return error(409, "request_retention");
        }

        if action == "continue" {
            if let Err(response) = self.verify_configured_runbook_bundle(&runbook)? {
                return Ok(response);
            }
            if runbook
                .fanout
                .as_ref()
                .is_some_and(|config| !config.enabled)
            {
                return error(409, "fanout_disabled");
            }
            if fanout_mode(runbook.mode) != existing.mode {
                return error(409, "runbook_mode_changed");
            }
            if runbook.lattice.bundle_id != existing.lattice_bundle_id
                || runbook.lattice.bundle_identity != existing.lattice_bundle_identity
                || runbook.lattice.workflow_name != existing.lattice_workflow_name
                || runbook.lattice.workflow_revision != existing.lattice_workflow_revision
            {
                return error(409, "runbook_definition_changed");
            }
            let Ok(resolved) = resolve_params(&runbook.params, &existing.params) else {
                return error(409, "runbook_definition_changed");
            };
            if resolved.identity != existing.params_identity
                || resolved.env_bindings != existing.env_bindings
            {
                return error(409, "runbook_definition_changed");
            }
            for child in existing
                .children
                .iter()
                .filter(|c| c.status == FanoutChildStatus::Pending)
            {
                let Some(target_rule) = runbook.allowed_targets.iter().find(|rule| {
                    rule.project_id == child.project_id
                        && rule.repository.eq_ignore_ascii_case(&child.repository)
                }) else {
                    return error(409, "runbook_target_not_allowed");
                };
                if crate::runbooks::is_hex40(&child.resolved_reference) {
                    if !target_rule.allow_commit_sha {
                        return error(409, "runbook_target_not_allowed");
                    }
                    let Some(project) =
                        crate::runbooks::match_project(&self.profile, &child.project_id)
                    else {
                        return error(409, "runbook_target_not_allowed");
                    };
                    if let Err((status, code)) =
                        resolve_sha_target_source(project, target_rule, &child.resolved_reference)
                    {
                        return if status == 503 {
                            error(503, code)
                        } else {
                            error(409, "runbook_target_not_allowed")
                        };
                    }
                } else if target_allows_reference(target_rule, &child.resolved_reference).is_none()
                {
                    return error(409, "runbook_target_not_allowed");
                }
                if existing.mode == FanoutMode::Mutating
                    && let Some(cfg) = &runbook.fanout
                    && !cfg.write_scopes.is_empty()
                {
                    let Some(scope) = &child.write_scope else {
                        return error(409, "runbook_definition_changed");
                    };
                    let still_allowed = cfg.write_scopes.iter().any(|declared| {
                        parse_scope(&child.repository, declared)
                            .is_ok_and(|ds| kiln_core::fleet::contains_scope(&ds, scope))
                    });
                    if !still_allowed {
                        return error(409, "runbook_definition_changed");
                    }
                }
            }
            if existing.mode == FanoutMode::Mutating
                && matches!(self.fleet_claims, FleetClaimsSnapshot::Unavailable)
            {
                return error(503, "fleet_unavailable");
            }
        }

        let now_ms = host_time_ms()?;
        let outcome = {
            let fanout = self
                .state
                .fanouts
                .get_mut(fanout_id)
                .ok_or("fanout missing")?;
            match action {
                "pause" => fanout.pause(now_ms),
                "continue" => fanout.continue_admission(now_ms),
                "cancel" => fanout.cancel(now_ms),
                _ => return error(400, "request_invalid"),
            }
        };
        if let Err(diag) = outcome {
            return error(409, &diag.code);
        }

        self.reserve_request(
            req.request_id.clone(),
            fingerprint,
            fanout_id.to_owned(),
            RequestKind::FanoutControl,
        )?;
        if action == "continue" {
            self.advance_fanouts()?;
            self.dispatch_queued()?;
        }
        self.sync_fanouts()?;

        let updated = self
            .state
            .fanouts
            .get(fanout_id)
            .ok_or("fanout missing after control")?;
        let response = FanoutControlResponse {
            schema: FANOUT_CONTROL_RESULT_SCHEMA,
            request_id: req.request_id.clone(),
            replayed: false,
            action,
            fanout_id: fanout_id.to_owned(),
            fanout: fanout_view(updated),
        };
        let encoded =
            serde_json::to_value(response).map_err(|_| "fanout control encoding failed")?;
        self.finish_request(&req.request_id, 200, encoded)
    }

    pub(super) fn replay_fanout_fallback(
        &self,
        request_id: &str,
        kind: RequestKind,
        resource: &str,
    ) -> Result<Option<ApiResponse>, String> {
        let Some(fanout) = self.state.fanouts.get(resource) else {
            return Ok(None);
        };
        let view = fanout_view(fanout);
        match kind {
            RequestKind::FanoutDispatch => {
                let status = if view.counts.pending > 0 || view.counts.active > 0 {
                    202
                } else {
                    201
                };
                let response = FanoutDispatchResponse {
                    schema: FANOUT_DISPATCH_RESULT_SCHEMA,
                    request_id: request_id.to_owned(),
                    replayed: true,
                    fanout_id: resource.to_owned(),
                    fanout: view,
                };
                json_response(status, response).map(Some)
            }
            RequestKind::FanoutControl => {
                let value = serde_json::to_value(FanoutControlResponse {
                    schema: FANOUT_CONTROL_RESULT_SCHEMA,
                    request_id: request_id.to_owned(),
                    replayed: true,
                    action: "control",
                    fanout_id: resource.to_owned(),
                    fanout: view,
                })
                .map_err(|_| "fanout control replay encoding failed")?;
                json_response(200, replay_response_with_flag(value)).map(Some)
            }
            _ => Ok(None),
        }
    }

    pub(super) fn sync_fanouts(&mut self) -> Result<(), String> {
        if self.state.fanouts.is_empty() {
            return Ok(());
        }
        let now_ms = host_time_ms()?;
        let mut changed = false;
        let runs = &self.state.runs;
        for fanout in self.state.fanouts.values_mut() {
            for idx in 0..fanout.children.len() {
                let Some(run_id) = fanout
                    .children
                    .get(idx)
                    .and_then(|c| c.run_id.as_ref())
                    .cloned()
                else {
                    continue;
                };
                let Some(run) = runs.get(&run_id) else {
                    continue;
                };
                let (status, detail, reason) = match run.outcome() {
                    Outcome::Pending => (FanoutChildStatus::Active, None, run.error_code.clone()),
                    Outcome::Success => {
                        (FanoutChildStatus::Success, Some("success".to_owned()), None)
                    }
                    Outcome::DomainFailure => (
                        FanoutChildStatus::Failure,
                        Some("domain_failure".to_owned()),
                        run.error_code.clone(),
                    ),
                    Outcome::Denied => (
                        FanoutChildStatus::Failure,
                        Some("denied".to_owned()),
                        run.error_code.clone(),
                    ),
                    Outcome::Cancelled => (
                        FanoutChildStatus::Failure,
                        Some("cancelled".to_owned()),
                        run.error_code.clone(),
                    ),
                    Outcome::TimedOut => (
                        FanoutChildStatus::Failure,
                        Some("timed_out".to_owned()),
                        run.error_code.clone(),
                    ),
                    Outcome::ProviderError => (
                        FanoutChildStatus::Failure,
                        Some("provider_error".to_owned()),
                        run.error_code.clone(),
                    ),
                    Outcome::Unknown => (
                        FanoutChildStatus::Unknown,
                        Some("unknown".to_owned()),
                        run.error_code.clone(),
                    ),
                };
                if fanout
                    .record_child_outcome(idx, status, detail, reason, now_ms)
                    .map_err(|diag| format!("fanout sync invalid: {}", diag.code))?
                {
                    changed = true;
                }
            }
        }
        if changed {
            self.persist()?;
        }
        Ok(())
    }

    pub(super) fn advance_fanouts(&mut self) -> Result<(), String> {
        if self.state.fanouts.is_empty() {
            return Ok(());
        }
        let now_ms = host_time_ms()?;
        let mut active_write_scopes = self.active_mutating_write_scopes();
        let mut busy_projects: BTreeSet<String> = self
            .state
            .runs
            .values()
            .filter(|r| {
                !r.is_engine_owned_execution() && !r.outcome().is_terminal() && !r.waiting()
            })
            .map(|r| r.project_id.clone())
            .collect();
        let mut ordered_ids: Vec<(u64, String)> = self
            .state
            .fanouts
            .values()
            .filter(|f| f.admission == FanoutAdmissionState::Open && f.pending_count() > 0)
            .map(|f| (f.ordinal, f.id.clone()))
            .collect();
        ordered_ids.sort();

        let mut changed = false;
        for (_, fanout_id) in ordered_ids {
            let Some(snapshot) = self.state.fanouts.get(&fanout_id).cloned() else {
                continue;
            };
            if !snapshot.budget.can_admit() {
                if let Some(fanout) = self.state.fanouts.get_mut(&fanout_id) {
                    fanout.exhaust_budget(now_ms);
                    changed = true;
                }
                continue;
            }
            if snapshot.mode == FanoutMode::Mutating
                && matches!(self.fleet_claims, FleetClaimsSnapshot::Unavailable)
            {
                continue;
            }
            let live_claims: Vec<FleetClaim> = match &self.fleet_claims {
                FleetClaimsSnapshot::Available(claims) => claims.clone(),
                FleetClaimsSnapshot::Unconfigured | FleetClaimsSnapshot::Unavailable => Vec::new(),
            };

            let mut idx = 0usize;
            while idx < snapshot.children.len() {
                let (active_count, max_conc, can_admit, claim_policy, child_opt) = {
                    let Some(fanout) = self.state.fanouts.get(&fanout_id) else {
                        break;
                    };
                    (
                        fanout.active_count(),
                        usize::from(fanout.max_concurrency),
                        fanout.budget.can_admit(),
                        fanout.claim_policy,
                        fanout.children.get(idx).cloned(),
                    )
                };
                if active_count >= max_conc || !can_admit {
                    break;
                }
                let Some(child) = child_opt else {
                    break;
                };
                if child.status != FanoutChildStatus::Pending {
                    idx += 1;
                    continue;
                }

                if snapshot.mode == FanoutMode::Mutating
                    && let Some(scope) = &child.write_scope
                {
                    if let Some(conflict) = find_claim_overlap(scope, &live_claims, now_ms) {
                        if claim_policy == FanoutClaimPolicy::Skip
                            && let Some(fanout) = self.state.fanouts.get_mut(&fanout_id)
                        {
                            fanout
                                .skip_child(
                                    idx,
                                    &format!("fleet_claim_conflict:{}", conflict_scope(&conflict)),
                                    now_ms,
                                )
                                .map_err(|d| format!("fanout skip failed: {}", d.code))?;
                            changed = true;
                        }
                        idx += 1;
                        continue;
                    }
                    if active_write_scopes
                        .iter()
                        .any(|active| overlaps(active, scope))
                    {
                        idx += 1;
                        continue;
                    }
                }

                if busy_projects.contains(&child.project_id) {
                    idx += 1;
                    continue;
                }
                if self.state.runs.len() >= self.profile.bounds.max_runs {
                    break;
                }
                let Some(project) = self
                    .profile
                    .projects
                    .iter()
                    .find(|p| p.id == child.project_id)
                    .cloned()
                else {
                    idx += 1;
                    continue;
                };

                let approval_gate = self
                    .runbook_profile
                    .as_ref()
                    .and_then(|reg| reg.find(&snapshot.runbook_id))
                    .map_or(crate::runbooks::ApprovalGate::None, |rb| rb.approval_gate);
                let binding = build_child_runbook_binding(&snapshot, &child, approval_gate);
                let run_id = self.enqueue_fanout_child(
                    &project,
                    &fanout_id,
                    child.index,
                    child.source.clone(),
                    binding,
                )?;
                let Some(fanout) = self.state.fanouts.get_mut(&fanout_id) else {
                    return Err("fanout disappeared during admission".into());
                };
                fanout
                    .admit_child(idx, run_id, now_ms)
                    .map_err(|diag| format!("fanout child admission failed: {}", diag.code))?;
                busy_projects.insert(child.project_id.clone());
                if let Some(scope) = child.write_scope {
                    active_write_scopes.push(scope);
                }
                changed = true;
                idx += 1;
            }

            if let Some(fanout) = self.state.fanouts.get_mut(&fanout_id)
                && fanout.pending_count() > 0
                && !fanout.budget.can_admit()
            {
                fanout.exhaust_budget(now_ms);
                changed = true;
            }
        }
        if changed {
            self.persist()?;
        }
        Ok(())
    }

    fn active_mutating_write_scopes(&self) -> Vec<FleetScope> {
        let mut scopes = Vec::new();
        for run in self.state.runs.values() {
            if run.is_engine_owned_execution() || run.outcome() != Outcome::Pending {
                continue;
            }
            if let Some(fanout_id) = &run.fanout_run_id {
                if let Some(fanout) = self.state.fanouts.get(fanout_id)
                    && let Some(child) = fanout
                        .children
                        .iter()
                        .find(|c| c.run_id.as_deref() == Some(run.id.as_str()))
                    && let Some(scope) = &child.write_scope
                {
                    scopes.push(scope.clone());
                }
                continue;
            }
            if let Some(rb) = &run.runbook
                && rb.mode == RunbookMode::Mutating
                && let Some(project) = self
                    .profile
                    .projects
                    .iter()
                    .find(|p| p.id == run.project_id)
            {
                let repo = run.source.repository.native_id.as_str();
                if repo.contains('/') {
                    let root = project.scope.as_ref().map_or(".", |s| s.root.as_str());
                    if let Ok(scope) = parse_scope(repo, root) {
                        scopes.push(scope);
                    }
                }
            }
        }
        scopes
    }
}

fn build_child_runbook_binding(
    fanout: &FanOutRun,
    child: &kiln_core::implementation::FanoutChild,
    approval_gate: crate::runbooks::ApprovalGate,
) -> RunbookRunBinding {
    let mut source_env = BTreeMap::new();
    source_env.insert("LATTICE_SOURCE_PROVIDER".to_owned(), "git".to_owned());
    source_env.insert(
        "LATTICE_SOURCE_REPOSITORY".to_owned(),
        child.source.repository.native_id.as_str().to_owned(),
    );
    source_env.insert(
        "LATTICE_SOURCE_REVISION".to_owned(),
        child.source_revision.clone(),
    );
    let params = fanout
        .params
        .iter()
        .filter_map(|(k, v)| {
            serde_json::from_value::<crate::runbooks::ParamValue>(v.clone())
                .ok()
                .map(|pv| (k.clone(), pv))
        })
        .collect();
    RunbookRunBinding {
        runbook_id: fanout.runbook_id.clone(),
        mode: match fanout.mode {
            FanoutMode::ReadOnly => RunbookMode::ReadOnly,
            FanoutMode::Mutating => RunbookMode::Mutating,
        },
        approval_gate,
        target: child.target.clone(),
        lattice_bundle_id: fanout.lattice_bundle_id.clone(),
        lattice_bundle_identity: fanout.lattice_bundle_identity.clone(),
        lattice_workflow_name: fanout.lattice_workflow_name.clone(),
        lattice_workflow_revision: fanout.lattice_workflow_revision.clone(),
        params_identity: fanout.params_identity.clone(),
        params,
        env_bindings: fanout.env_bindings.clone(),
        source_env,
    }
}

fn conflict_scope(conflict: &ClaimConflict) -> &str {
    match conflict {
        ClaimConflict::Claim { scope, .. }
        | ClaimConflict::Attempt { scope, .. }
        | ClaimConflict::Foreign { scope, .. } => scope,
    }
}

pub(super) fn conflict_summary(conflict: &ClaimConflict) -> String {
    match conflict {
        ClaimConflict::Claim { holder, scope, .. } => format!("{scope} ({holder})"),
        ClaimConflict::Attempt { attempt, scope } => {
            format!("{scope} (attempt {})", attempt.as_str())
        }
        ClaimConflict::Foreign { author, scope, .. } => format!("{scope} ({author})"),
    }
}
