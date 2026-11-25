use std::collections::BTreeMap;

use base64::Engine as _;
use kiln_codec::identity_from_bytes;
use kiln_core::domain::Outcome;
use kiln_core::implementation::{FanoutCounts, FanoutPhase};
use kiln_core::types::{EffectId, RunId};
use kiln_provider_api::NotificationRequest;
use serde::{Deserialize, Serialize};

use super::api::{decoded, error, json_response};
use super::model::{RunRecord, SummaryDeliveryRecord};
use super::{ApiResponse, Engine, NotificationWork, host_time_ms, identity};
use crate::automation::external;
use crate::rollups::{
    ImplementationWorkIndex, ROLLUP_NON_CLAIMS, RUNBOOK_DISPATCHES_VIEW_SCHEMA,
    RUNBOOK_ROLLUP_SCHEMA, RollupTargetView, RunbookDispatchPage, RunbookDispatchSummary,
    RunbookRollupPage, SummaryNotificationInput, build_dispatch_target_view,
    build_fanout_target_views, build_summary_notification_body, compute_counts,
    extract_patchgate_receipt, extract_pull_requests, extract_verification_receipt,
    outcome_to_rollup_status, summarize_notification_status,
};
use crate::runbooks::{RunbookRegistry, valid_runbook_id};

const MAX_SUMMARY_POLL_BATCH: usize = 16;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatchCursor {
    version: u8,
    snapshot: u64,
    ordinal: u64,
    id: String,
    query: String,
    order: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RollupCursor {
    version: u8,
    subject_kind: String,
    subject_id: String,
    index: u16,
    target: String,
    query: String,
    order: String,
}

fn normalize_outcome_filter(raw: &str) -> Option<&'static str> {
    match raw {
        "DomainFailure" => Some("domain_failure"),
        "TimedOut" => Some("timed_out"),
        "ProviderError" => Some("provider_error"),
        _ => match raw.to_ascii_lowercase().as_str() {
            "pending" => Some("pending"),
            "success" => Some("success"),
            "domain_failure" => Some("domain_failure"),
            "denied" => Some("denied"),
            "cancelled" => Some("cancelled"),
            "timed_out" => Some("timed_out"),
            "provider_error" => Some("provider_error"),
            "unknown" => Some("unknown"),
            _ => None,
        },
    }
}

fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pending => "pending",
        Outcome::Success => "success",
        Outcome::DomainFailure => "domain_failure",
        Outcome::Denied => "denied",
        Outcome::Cancelled => "cancelled",
        Outcome::TimedOut => "timed_out",
        Outcome::ProviderError => "provider_error",
        Outcome::Unknown => "unknown",
    }
}

struct RollupPageQuery<'a> {
    status_filter: Option<&'static str>,
    order: &'a str,
    limit: usize,
    fingerprint: String,
    cursor: Option<RollupCursor>,
}

fn parse_rollup_query<'a>(
    params: &BTreeMap<&str, &str>,
    subject_kind: &str,
    subject_id: &str,
    valid_anchor: impl Fn(&RollupCursor) -> bool,
) -> Result<Result<RollupPageQuery<'a>, ApiResponse>, String> {
    if params
        .keys()
        .any(|k| !matches!(*k, "status" | "order" | "limit" | "cursor"))
    {
        return Ok(Err(error(400, "query_invalid")?));
    }
    let mut values = BTreeMap::new();
    for (key, value) in params {
        if *key != "cursor" {
            let decoded_val = match decoded(value) {
                Ok(v) if v.len() <= 256 && !v.bytes().any(|b| b.is_ascii_control()) => v,
                _ => return Ok(Err(error(400, "query_invalid")?)),
            };
            values.insert(*key, decoded_val);
        }
    }
    let order = match values.get("order").map(String::as_str) {
        None | Some("asc") => "asc",
        Some("desc") => "desc",
        Some(_) => return Ok(Err(error(400, "order_invalid")?)),
    };
    let limit = match values.get("limit").map(|v| v.parse::<usize>()).transpose() {
        Ok(Some(v)) if (1..=100).contains(&v) => v,
        Ok(None) => 50,
        _ => return Ok(Err(error(400, "page_bound")?)),
    };
    let status_filter = match values.get("status").map(String::as_str) {
        None => None,
        Some("pending") => Some("pending"),
        Some("active") => Some("active"),
        Some("success") => Some("success"),
        Some("failure") => Some("failure"),
        Some("unknown") => Some("unknown"),
        Some("skipped") => Some("skipped"),
        Some(_) => return Ok(Err(error(400, "rollup_status_invalid")?)),
    };
    let fingerprint = identity(&values)?.as_str().to_owned();
    let cursor = if let Some(raw) = params.get("cursor") {
        let decoded_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw);
        let cursor: RollupCursor = match decoded_bytes
            .ok()
            .filter(|v| v.len() <= 4096)
            .and_then(|v| serde_json::from_slice(&v).ok())
        {
            Some(v) => v,
            None => return Ok(Err(error(400, "cursor_invalid")?)),
        };
        if cursor.version != 1
            || cursor.subject_kind != subject_kind
            || cursor.subject_id != subject_id
            || cursor.query != fingerprint
            || cursor.order != order
            || !valid_anchor(&cursor)
        {
            return Ok(Err(error(400, "cursor_invalid")?));
        }
        Some(cursor)
    } else {
        None
    };

    Ok(Ok(RollupPageQuery {
        status_filter,
        order,
        limit,
        fingerprint,
        cursor,
    }))
}

fn paginate_targets(
    mut targets: Vec<RollupTargetView>,
    subject_kind: &str,
    subject_id: &str,
    query: &RollupPageQuery<'_>,
) -> (Vec<RollupTargetView>, Option<String>, usize) {
    if let Some(status_filter) = query.status_filter {
        targets.retain(|t| t.status == status_filter);
    }
    let total = targets.len();
    targets.sort_by(|left, right| {
        let ord = left
            .index
            .cmp(&right.index)
            .then(left.target.cmp(&right.target));
        if query.order == "desc" {
            ord.reverse()
        } else {
            ord
        }
    });
    if let Some(c) = &query.cursor {
        targets.retain(|t| {
            if query.order == "desc" {
                (t.index, t.target.as_str()) < (c.index, c.target.as_str())
            } else {
                (t.index, t.target.as_str()) > (c.index, c.target.as_str())
            }
        });
    }
    let has_more = targets.len() > query.limit;
    targets.truncate(query.limit);
    let next_cursor = if has_more {
        targets.last().map(|last| {
            let c = RollupCursor {
                version: 1,
                subject_kind: subject_kind.to_owned(),
                subject_id: subject_id.to_owned(),
                index: last.index,
                target: last.target.clone(),
                query: query.fingerprint.clone(),
                order: query.order.to_owned(),
            };
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(serde_json::to_vec(&c).unwrap_or_default())
        })
    } else {
        None
    };
    (targets, next_cursor, total)
}

impl Engine {
    pub(super) fn list_runbook_dispatches(
        &self,
        params: &BTreeMap<&str, &str>,
        impl_work: &ImplementationWorkIndex,
    ) -> Result<ApiResponse, String> {
        if params
            .keys()
            .any(|k| !matches!(*k, "runbook_id" | "outcome" | "order" | "limit" | "cursor"))
        {
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
        let outcome_filter = match values.get("outcome").map(String::as_str) {
            None => None,
            Some(raw) => match normalize_outcome_filter(raw) {
                Some(norm) => Some(norm),
                None => return error(400, "outcome_invalid"),
            },
        };
        let fingerprint = identity(&values)?.as_str().to_owned();
        let cursor = if let Some(raw) = params.get("cursor") {
            let decoded_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw);
            let cursor: DispatchCursor = match decoded_bytes
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
                || self.state.runs.get(&cursor.id).is_none_or(|r| {
                    r.ordinal != cursor.ordinal || r.runbook.is_none() || r.fanout_run_id.is_some()
                })
            {
                return error(400, "cursor_invalid");
            }
            Some(cursor)
        } else {
            None
        };
        let snapshot = cursor.as_ref().map(|c| c.snapshot).unwrap_or_else(|| {
            self.state
                .runs
                .values()
                .filter(|r| r.runbook.is_some() && r.fanout_run_id.is_none())
                .map(|r| r.ordinal)
                .max()
                .unwrap_or(0)
        });
        let mut items: Vec<&RunRecord> = self
            .state
            .runs
            .values()
            .filter(|r| r.runbook.is_some() && r.fanout_run_id.is_none())
            .filter(|r| r.ordinal <= snapshot)
            .filter(|r| {
                runbook_filter
                    .is_none_or(|id| r.runbook.as_ref().is_some_and(|rb| rb.runbook_id == id))
            })
            .filter(|r| outcome_filter.is_none_or(|o| outcome_name(r.outcome()) == o))
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
            items.retain(|r| {
                if order == "desc" {
                    (r.ordinal, r.id.as_str()) < (c.ordinal, c.id.as_str())
                } else {
                    (r.ordinal, r.id.as_str()) > (c.ordinal, c.id.as_str())
                }
            });
        }
        let has_more = items.len() > limit;
        let page_items: Vec<&RunRecord> = items.into_iter().take(limit).collect();
        let next_cursor = if has_more {
            page_items.last().map(|last| {
                let c = DispatchCursor {
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

        let mut dispatches = Vec::with_capacity(page_items.len());
        for run in page_items {
            let Some(rb) = &run.runbook else {
                continue;
            };
            let status = outcome_to_rollup_status(run.outcome());
            let source_revision = run.source.exact_revision.native_id.as_str().to_owned();
            let repository = run.source.repository.native_id.as_str();
            let patchgate = extract_patchgate_receipt(Some(run), status);
            let verification = extract_verification_receipt(
                Some(run),
                status,
                &run.project_id,
                repository,
                &source_revision,
                &self.state.runs,
                impl_work,
            );
            let (_, prs) = extract_pull_requests(
                Some(run),
                status,
                &run.project_id,
                repository,
                &source_revision,
                impl_work,
            );
            dispatches.push(RunbookDispatchSummary {
                run_id: run.id.clone(),
                ordinal: run.ordinal,
                runbook_id: rb.runbook_id.clone(),
                mode: rb.mode,
                target: rb.target.clone(),
                project_id: run.project_id.clone(),
                source_revision,
                outcome: run.outcome(),
                phase: run.phase.clone(),
                status,
                lattice_bundle_id: rb.lattice_bundle_id.clone(),
                lattice_workflow_name: rb.lattice_workflow_name.clone(),
                lattice_workflow_revision: rb.lattice_workflow_revision.clone(),
                params_identity: rb.params_identity.clone(),
                created_at_ms: run.created_at_ms,
                completed_at_ms: run.completed_at_ms,
                patchgate_receipt_state: patchgate.state,
                verification_receipt_state: verification.state,
                pull_request_count: prs.len(),
            });
        }

        json_response(
            200,
            RunbookDispatchPage {
                schema: RUNBOOK_DISPATCHES_VIEW_SCHEMA,
                dispatches,
                next_cursor,
                total,
                bounded: true,
                grants_authority: false,
                non_claims: ROLLUP_NON_CLAIMS.to_vec(),
            },
        )
    }

    pub(super) fn get_dispatch_rollup(
        &self,
        run_id: &str,
        params: &BTreeMap<&str, &str>,
        impl_work: &ImplementationWorkIndex,
    ) -> Result<ApiResponse, String> {
        if !crate::profile::identifier(run_id) {
            return error(400, "run_id_invalid");
        }
        let Some(run) = self.state.runs.get(run_id) else {
            return error(404, "run_missing");
        };
        let Some(rb) = &run.runbook else {
            return error(404, "runbook_dispatch_not_found");
        };
        let target = build_dispatch_target_view(run, &self.state.runs, impl_work)?;
        let query = match parse_rollup_query(params, "dispatch", &run.id, |c| {
            c.index == target.index && c.target == target.target
        })? {
            Ok(q) => q,
            Err(resp) => return Ok(resp),
        };
        let counts = compute_counts(std::slice::from_ref(&target));
        let (targets, next_cursor, total) =
            paginate_targets(vec![target], "dispatch", &run.id, &query);
        let registry = self.current_runbooks().ok().flatten();
        let notification = summarize_notification_status(
            "dispatch",
            &run.id,
            &rb.runbook_id,
            &self.state.runbook_notifications,
            registry.as_ref(),
        );
        let outcome = run.outcome();
        let has_unknown = outcome == Outcome::Unknown;
        json_response(
            200,
            RunbookRollupPage {
                schema: RUNBOOK_ROLLUP_SCHEMA,
                subject_kind: "dispatch",
                subject_id: run.id.clone(),
                runbook_id: rb.runbook_id.clone(),
                mode: rb.mode,
                run_id: Some(run.id.clone()),
                fanout_id: run.fanout_run_id.clone(),
                principal: None,
                phase: run.phase.clone(),
                overall_outcome: Some(outcome),
                has_unknown,
                admission: None,
                stop_reason: None,
                lattice_bundle_id: rb.lattice_bundle_id.clone(),
                lattice_bundle_identity: rb.lattice_bundle_identity.clone(),
                lattice_workflow_name: rb.lattice_workflow_name.clone(),
                lattice_workflow_revision: rb.lattice_workflow_revision.clone(),
                params_identity: rb.params_identity.clone(),
                counts,
                notification,
                targets,
                next_cursor,
                total,
                bounded: true,
                grants_authority: false,
                non_claims: ROLLUP_NON_CLAIMS.to_vec(),
            },
        )
    }

    pub(super) fn get_fanout_rollup(
        &self,
        fanout_id: &str,
        params: &BTreeMap<&str, &str>,
        impl_work: &ImplementationWorkIndex,
    ) -> Result<ApiResponse, String> {
        if !crate::fanouts::valid_fanout_id(fanout_id) {
            return error(400, "fanout_id_invalid");
        }
        let Some(fanout) = self.state.fanouts.get(fanout_id) else {
            return error(404, "fanout_not_found");
        };
        let all_targets = build_fanout_target_views(fanout, &self.state.runs, impl_work);
        let query = match parse_rollup_query(params, "fanout", &fanout.id, |c| {
            all_targets
                .iter()
                .any(|t| t.index == c.index && t.target == c.target)
        })? {
            Ok(q) => q,
            Err(resp) => return Ok(resp),
        };
        let counts = compute_counts(&all_targets);
        let has_unknown = counts.unknown > 0;
        let (targets, next_cursor, total) =
            paginate_targets(all_targets, "fanout", &fanout.id, &query);
        let registry = self.current_runbooks().ok().flatten();
        let notification = summarize_notification_status(
            "fanout",
            &fanout.id,
            &fanout.runbook_id,
            &self.state.runbook_notifications,
            registry.as_ref(),
        );
        let mode = match fanout.mode {
            kiln_core::implementation::FanoutMode::ReadOnly => {
                crate::runbooks::RunbookMode::ReadOnly
            }
            kiln_core::implementation::FanoutMode::Mutating => {
                crate::runbooks::RunbookMode::Mutating
            }
        };
        let phase = match fanout.phase {
            FanoutPhase::Active => "active",
            FanoutPhase::Paused => "paused",
            FanoutPhase::Completed => "completed",
            FanoutPhase::Cancelled => "cancelled",
        }
        .to_owned();
        json_response(
            200,
            RunbookRollupPage {
                schema: RUNBOOK_ROLLUP_SCHEMA,
                subject_kind: "fanout",
                subject_id: fanout.id.clone(),
                runbook_id: fanout.runbook_id.clone(),
                mode,
                run_id: None,
                fanout_id: Some(fanout.id.clone()),
                principal: Some(fanout.principal_id.clone()),
                phase,
                overall_outcome: None,
                has_unknown,
                admission: Some(fanout.admission),
                stop_reason: fanout.stop_reason.clone(),
                lattice_bundle_id: fanout.lattice_bundle_id.clone(),
                lattice_bundle_identity: fanout.lattice_bundle_identity.clone(),
                lattice_workflow_name: fanout.lattice_workflow_name.clone(),
                lattice_workflow_revision: fanout.lattice_workflow_revision.clone(),
                params_identity: fanout.params_identity.clone(),
                counts,
                notification,
                targets,
                next_cursor,
                total,
                bounded: true,
                grants_authority: false,
                non_claims: ROLLUP_NON_CLAIMS.to_vec(),
            },
        )
    }

    pub(super) fn recover_stale_summary_notifications(&mut self) -> Result<(), String> {
        let mut changed = false;
        for record in self
            .state
            .runbook_notifications
            .values_mut()
            .filter(|r| r.status == "reserved")
        {
            record.status = "ambiguous".to_owned();
            record.error_code = Some("restart_after_reservation".to_owned());
            changed = true;
        }
        if changed {
            self.persist()?;
        }
        Ok(())
    }

    pub(super) fn tick_runbook_summary_notifications(
        &mut self,
    ) -> Result<Vec<NotificationWork>, String> {
        if self.storage_failed {
            return Err("durable state unavailable".into());
        }
        let Some(registry) = self.current_runbooks()? else {
            return Ok(Vec::new());
        };
        if registry.notifications.is_empty() {
            return Ok(Vec::new());
        }
        let max_records = self.profile.bounds.max_requests.saturating_mul(4).max(256);
        let now = host_time_ms()?;

        enum Candidate<'a> {
            Fanout(&'a kiln_core::implementation::FanOutRun),
            Dispatch(&'a RunRecord),
        }

        let mut subjects: Vec<(u64, &str, &str, Candidate<'_>)> = Vec::new();
        for fanout in self.state.fanouts.values() {
            if matches!(
                fanout.phase,
                FanoutPhase::Completed | FanoutPhase::Cancelled
            ) {
                subjects.push((
                    fanout.ordinal,
                    "fanout",
                    fanout.id.as_str(),
                    Candidate::Fanout(fanout),
                ));
            }
        }
        for run in self.state.runs.values() {
            if run.runbook.is_some()
                && run.fanout_run_id.is_none()
                && (run.outcome().is_terminal() || run.outcome() == Outcome::Unknown)
            {
                subjects.push((
                    run.ordinal,
                    "dispatch",
                    run.id.as_str(),
                    Candidate::Dispatch(run),
                ));
            }
        }
        subjects.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)).then(a.2.cmp(b.2)));

        struct PendingReservation {
            record: SummaryDeliveryRecord,
            registration: crate::automation::NotificationProfile,
            request: NotificationRequest,
        }

        let mut planned: Vec<PendingReservation> = Vec::new();
        let mut rejected_records: Vec<SummaryDeliveryRecord> = Vec::new();

        for (_, subject_kind, subject_id, candidate) in subjects {
            if planned.len() >= MAX_SUMMARY_POLL_BATCH {
                break;
            }
            if self.state.runbook_notifications.len() + planned.len() + rejected_records.len()
                >= max_records
            {
                break;
            }
            let (
                runbook_id,
                mode,
                phase,
                overall_outcome,
                targets,
                is_success,
                is_failure,
                is_unknown,
                request_outcome,
            ) = match candidate {
                Candidate::Fanout(fanout) => {
                    let classification =
                        classify_fanout_summary_notification(fanout.counts(), fanout.phase);
                    let mode = match fanout.mode {
                        kiln_core::implementation::FanoutMode::ReadOnly => {
                            crate::runbooks::RunbookMode::ReadOnly
                        }
                        kiln_core::implementation::FanoutMode::Mutating => {
                            crate::runbooks::RunbookMode::Mutating
                        }
                    };
                    let phase = match fanout.phase {
                        FanoutPhase::Active => "active",
                        FanoutPhase::Paused => "paused",
                        FanoutPhase::Completed => "completed",
                        FanoutPhase::Cancelled => "cancelled",
                    };
                    let targets = build_fanout_target_views(
                        fanout,
                        &self.state.runs,
                        &ImplementationWorkIndex::Unconfigured,
                    );
                    (
                        fanout.runbook_id.as_str(),
                        mode,
                        phase,
                        None,
                        targets,
                        classification.is_success,
                        classification.is_failure,
                        classification.is_unknown,
                        classification.request_outcome,
                    )
                }
                Candidate::Dispatch(run) => {
                    let Some(rb) = &run.runbook else {
                        continue;
                    };
                    let outcome = run.outcome();
                    let is_success = outcome == Outcome::Success;
                    let is_unknown = outcome == Outcome::Unknown;
                    let is_failure = outcome.is_terminal() && outcome != Outcome::Success;
                    let targets = vec![build_dispatch_target_view(
                        run,
                        &self.state.runs,
                        &ImplementationWorkIndex::Unconfigured,
                    )?];
                    (
                        rb.runbook_id.as_str(),
                        rb.mode,
                        run.phase.as_str(),
                        Some(outcome),
                        targets,
                        is_success,
                        is_failure,
                        is_unknown,
                        outcome,
                    )
                }
            };

            for reg in &registry.notifications {
                if planned.len() >= MAX_SUMMARY_POLL_BATCH {
                    break;
                }
                if self.state.runbook_notifications.len() + planned.len() + rejected_records.len()
                    >= max_records
                {
                    break;
                }
                if !reg.enabled || !reg.applies_to(runbook_id) {
                    continue;
                }
                let matched = (is_success && reg.on_success)
                    || (is_failure && reg.on_failure)
                    || (is_unknown && reg.on_unknown);
                if !matched {
                    continue;
                }
                let delivery_id = identity(&(
                    "kiln.runbook-summary-notification.v1",
                    &reg.id,
                    subject_kind,
                    subject_id,
                ))?
                .as_str()
                .to_owned();
                if self.state.runbook_notifications.contains_key(&delivery_id)
                    || planned.iter().any(|p| p.record.id == delivery_id)
                    || rejected_records.iter().any(|r| r.id == delivery_id)
                {
                    continue;
                }
                let automation_profile = reg.to_automation_profile();
                let webhook = automation_profile.webhook()?;
                let body = match build_summary_notification_body(&SummaryNotificationInput {
                    idempotency_key: &delivery_id,
                    registration_id: &reg.id,
                    subject_kind,
                    subject_id,
                    runbook_id,
                    mode,
                    phase,
                    overall_outcome,
                    targets: &targets,
                    max_request_bytes: reg.max_request_bytes,
                }) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        rejected_records.push(SummaryDeliveryRecord {
                            id: delivery_id,
                            registration_id: reg.id.clone(),
                            subject_kind: subject_kind.to_owned(),
                            subject_id: subject_id.to_owned(),
                            runbook_id: runbook_id.to_owned(),
                            reserved_at_ms: now,
                            completed_at_ms: Some(now),
                            status: "rejected".to_owned(),
                            http_status: None,
                            error_code: Some("notification_request_bound".to_owned()),
                        });
                        continue;
                    }
                };
                let body_identity = identity_from_bytes(&body)
                    .map_err(|_| "summary notification body identity failed".to_owned())?;
                let request = NotificationRequest {
                    effect_id: EffectId::new(delivery_id.clone())
                        .map_err(|_| "summary notification effect identity invalid")?,
                    run_id: RunId::new(subject_id)
                        .map_err(|_| "summary notification subject identity invalid")?,
                    outcome: request_outcome,
                    notification_policy: external("runbook-notification-policy", &reg.id)?,
                    endpoint_profile: webhook.profile_id,
                    body_identity,
                    idempotency_key: external("idempotency", &delivery_id)?,
                    body,
                };
                let record = SummaryDeliveryRecord {
                    id: delivery_id,
                    registration_id: reg.id.clone(),
                    subject_kind: subject_kind.to_owned(),
                    subject_id: subject_id.to_owned(),
                    runbook_id: runbook_id.to_owned(),
                    reserved_at_ms: now,
                    completed_at_ms: None,
                    status: "reserved".to_owned(),
                    http_status: None,
                    error_code: None,
                };
                planned.push(PendingReservation {
                    record,
                    registration: automation_profile,
                    request,
                });
            }
        }

        if planned.is_empty() && rejected_records.is_empty() {
            return Ok(Vec::new());
        }

        for rejected in rejected_records {
            self.state
                .runbook_notifications
                .insert(rejected.id.clone(), rejected);
        }
        let mut works = Vec::with_capacity(planned.len());
        for item in planned {
            self.state
                .runbook_notifications
                .insert(item.record.id.clone(), item.record.clone());
            works.push(NotificationWork::from_runbook_summary(
                item.record,
                item.registration,
                item.request,
            ));
        }
        self.persist()?;
        Ok(works)
    }

    pub(super) fn record_summary_notification(
        &mut self,
        reservation: SummaryDeliveryRecord,
        completed_at_ms: u64,
        status: &'static str,
        http_status: Option<u16>,
        error_code: Option<&'static str>,
    ) -> Result<(), String> {
        if self.storage_failed {
            return Err("durable state unavailable".into());
        }
        if self.state.runbook_notifications.get(&reservation.id) != Some(&reservation)
            || reservation.status != "reserved"
        {
            return Err(
                "summary notification observation does not match exact retained reservation".into(),
            );
        }
        let record = self
            .state
            .runbook_notifications
            .get_mut(&reservation.id)
            .ok_or("summary delivery reservation missing")?;
        record.http_status = http_status;
        record.completed_at_ms = Some(completed_at_ms);
        record.status = status.into();
        record.error_code = error_code.map(str::to_string);
        self.persist()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FanoutNotificationClassification {
    is_success: bool,
    is_failure: bool,
    is_unknown: bool,
    request_outcome: Outcome,
}

fn classify_fanout_summary_notification(
    counts: FanoutCounts,
    phase: FanoutPhase,
) -> FanoutNotificationClassification {
    let is_unknown = counts.unknown > 0;
    let is_failure = counts.failure > 0 || counts.skipped > 0 || phase == FanoutPhase::Cancelled;
    let is_success = !is_failure && !is_unknown && counts.pending == 0 && counts.active == 0;
    let request_outcome = if is_unknown {
        Outcome::Unknown
    } else if is_failure {
        Outcome::DomainFailure
    } else {
        Outcome::Success
    };
    FanoutNotificationClassification {
        is_success,
        is_failure,
        is_unknown,
        request_outcome,
    }
}

#[allow(dead_code)]
fn _registry_marker(_: &RunbookRegistry) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fanout_notification_classification_handles_skipped_and_preserves_unknown() {
        let all_ok = classify_fanout_summary_notification(
            FanoutCounts {
                total: 2,
                pending: 0,
                active: 0,
                success: 2,
                failure: 0,
                unknown: 0,
                skipped: 0,
            },
            FanoutPhase::Completed,
        );
        assert!(all_ok.is_success);
        assert!(!all_ok.is_failure);
        assert!(!all_ok.is_unknown);
        assert_eq!(all_ok.request_outcome, Outcome::Success);

        // Completed fan-out with skipped child (e.g., budget exhausted or claim conflict)
        let skipped = classify_fanout_summary_notification(
            FanoutCounts {
                total: 2,
                pending: 0,
                active: 0,
                success: 1,
                failure: 0,
                unknown: 0,
                skipped: 1,
            },
            FanoutPhase::Completed,
        );
        assert!(!skipped.is_success);
        assert!(skipped.is_failure);
        assert!(!skipped.is_unknown);
        assert_eq!(skipped.request_outcome, Outcome::DomainFailure);

        // Pure Unknown fan-out must NOT be coerced to failure
        let pure_unknown = classify_fanout_summary_notification(
            FanoutCounts {
                total: 2,
                pending: 0,
                active: 0,
                success: 1,
                failure: 0,
                unknown: 1,
                skipped: 0,
            },
            FanoutPhase::Completed,
        );
        assert!(!pure_unknown.is_success);
        assert!(!pure_unknown.is_failure);
        assert!(pure_unknown.is_unknown);
        assert_eq!(pure_unknown.request_outcome, Outcome::Unknown);

        // Mixed Unknown + Skipped triggers both unknown and failure, preserving Outcome::Unknown
        let unknown_and_skipped = classify_fanout_summary_notification(
            FanoutCounts {
                total: 2,
                pending: 0,
                active: 0,
                success: 0,
                failure: 0,
                unknown: 1,
                skipped: 1,
            },
            FanoutPhase::Completed,
        );
        assert!(!unknown_and_skipped.is_success);
        assert!(unknown_and_skipped.is_failure);
        assert!(unknown_and_skipped.is_unknown);
        assert_eq!(unknown_and_skipped.request_outcome, Outcome::Unknown);

        // Cancelled fan-out
        let cancelled = classify_fanout_summary_notification(
            FanoutCounts {
                total: 2,
                pending: 0,
                active: 0,
                success: 1,
                failure: 0,
                unknown: 0,
                skipped: 1,
            },
            FanoutPhase::Cancelled,
        );
        assert!(!cancelled.is_success);
        assert!(cancelled.is_failure);
        assert!(!cancelled.is_unknown);
        assert_eq!(cancelled.request_outcome, Outcome::DomainFailure);
    }
}
