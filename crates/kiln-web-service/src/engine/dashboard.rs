use super::api::{decoded, error, json_response};
use super::model::duration;
use super::{ApiResponse, Engine, Role, identity};
use base64::Engine as _;
use kiln_core::domain::Outcome;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    snapshot: u64,
    ordinal: u64,
    id: String,
    query: String,
    order: String,
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
fn failed(outcome: Outcome) -> bool {
    matches!(
        outcome,
        Outcome::DomainFailure | Outcome::Denied | Outcome::TimedOut | Outcome::ProviderError
    )
}
impl Engine {
    pub(super) fn run_list(
        &self,
        params: &BTreeMap<&str, &str>,
        role: Role,
    ) -> Result<ApiResponse, String> {
        if params.keys().any(|k| {
            !matches!(
                *k,
                "q" | "project_id" | "outcome" | "source_revision" | "limit" | "cursor" | "order"
            )
        }) {
            return error(400, "query_invalid");
        }
        let mut values = BTreeMap::new();
        for (key, value) in params {
            if *key != "cursor" {
                let value = match decoded(value) {
                    Ok(v) if v.len() <= 1024 && !v.bytes().any(|b| b.is_ascii_control()) => v,
                    _ => return error(400, "query_invalid"),
                };
                values.insert(*key, value);
            }
        }
        let order = values.get("order").map(String::as_str).unwrap_or("desc");
        if !matches!(order, "asc" | "desc") {
            return error(400, "order_invalid");
        }
        let limit = match values.get("limit").map(|v| v.parse::<usize>()).transpose() {
            Ok(Some(v)) if (1..=100).contains(&v) => v,
            Ok(None) => {
                if role == Role::Worker {
                    self.profile.bounds.max_runs
                } else {
                    50
                }
            }
            _ => return error(400, "page_bound"),
        };
        let outcome = values.get("outcome").map(|v| match v.as_str() {
            "DomainFailure" => "domain_failure".into(),
            "TimedOut" => "timed_out".into(),
            "ProviderError" => "provider_error".into(),
            _ => v.to_ascii_lowercase(),
        });
        if outcome.as_ref().is_some_and(|v| {
            !matches!(
                v.as_str(),
                "pending"
                    | "success"
                    | "domain_failure"
                    | "denied"
                    | "cancelled"
                    | "timed_out"
                    | "provider_error"
                    | "unknown"
                    | "failed"
                    | "queued"
            )
        }) {
            return error(400, "outcome_invalid");
        }
        let fingerprint = identity(&values)?.as_str().to_owned();
        let cursor = if let Some(raw) = params.get("cursor") {
            let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw);
            let cursor: Cursor = match decoded
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
                    .runs
                    .get(&cursor.id)
                    .is_none_or(|r| r.ordinal != cursor.ordinal)
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
                .map(|r| r.ordinal)
                .max()
                .unwrap_or(0)
        });
        let q = values.get("q").map(|s| s.to_lowercase());
        let mut runs: Vec<_> = self
            .state
            .runs
            .values()
            .filter(|r| {
                r.ordinal <= snapshot
                    && values.get("project_id").is_none_or(|p| p == &r.project_id)
                    && values
                        .get("source_revision")
                        .is_none_or(|v| v == r.source.exact_revision.native_id.as_str())
                    && outcome.as_ref().is_none_or(|v| {
                        if v == "queued" {
                            r.phase == "queued"
                        } else if v == "failed" {
                            failed(r.outcome())
                        } else {
                            v == outcome_name(r.outcome())
                        }
                    })
                    && q.as_ref().is_none_or(|q| {
                        [
                            r.id.as_str(),
                            r.project_id.as_str(),
                            r.source.exact_revision.native_id.as_str(),
                            r.source.symbolic_reference.native_id.as_str(),
                            outcome_name(r.outcome()),
                        ]
                        .iter()
                        .any(|v| v.to_lowercase().contains(q))
                    })
                    && (role != Role::Worker || (!r.outcome().is_terminal() && r.phase == "active"))
            })
            .collect();
        runs.sort_by(|a, b| a.ordinal.cmp(&b.ordinal).then_with(|| a.id.cmp(&b.id)));
        if order == "desc" {
            runs.reverse();
        }
        let total = runs.len();
        if let Some(c) = &cursor {
            runs.retain(|r| {
                if order == "desc" {
                    (r.ordinal, &r.id) < (c.ordinal, &c.id)
                } else {
                    (r.ordinal, &r.id) > (c.ordinal, &c.id)
                }
            });
        }
        let more = runs.len() > limit;
        runs.truncate(limit);
        let next_cursor = if more {
            runs.last()
                .map(|r| {
                    serde_json::to_vec(&Cursor {
                        version: 1,
                        snapshot,
                        ordinal: r.ordinal,
                        id: r.id.clone(),
                        query: fingerprint,
                        order: order.into(),
                    })
                })
                .transpose()
                .map_err(|_| "cursor encoding failed")?
                .map(|v| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v))
        } else {
            None
        };
        json_response(
            200,
            json!({"runs":runs.iter().map(|r|r.view()).collect::<Vec<_>>(),"next_cursor":next_cursor,"total":total,"bounded":true}),
        )
    }
    pub(super) fn dashboard_route(
        &self,
        path: &str,
        params: &BTreeMap<&str, &str>,
    ) -> Result<Option<ApiResponse>, String> {
        if path == "/api/activity" {
            return self.activity(None, params).map(Some);
        }
        if path == "/api/overview" || path == "/api/operations" {
            if !params.is_empty() {
                return error(400, "query_invalid").map(Some);
            }
            return if path.ends_with("overview") {
                self.overview()
            } else {
                self.operations()
            }
            .map(Some);
        }
        if let Some(id) = path
            .strip_prefix("/api/projects/")
            .and_then(|p| p.strip_suffix("/refs"))
        {
            if id.contains('/') || !params.is_empty() {
                return error(400, "query_invalid").map(Some);
            }
            let Some(project) = self.profile.projects.iter().find(|p| p.id == id) else {
                return error(404, "project_missing").map(Some);
            };
            let references: Vec<_> = std::iter::once(&project.reference)
                .chain(project.admitted_references.iter())
                .map(|reference| {
                    let revision = git2::Repository::open(&project.repository_path)
                        .ok()
                        .and_then(|r| {
                            r.find_reference(reference)
                                .ok()
                                .and_then(|v| v.peel_to_commit().ok())
                                .map(|v| v.id().to_string())
                        });
                    json!({"reference":reference,"source_revision":revision})
                })
                .collect();
            return json_response(
                200,
                json!({"project_id":id,"references":references,"exact_revision_selection":true}),
            )
            .map(Some);
        }
        Ok(None)
    }
    fn overview(&self) -> Result<ApiResponse, String> {
        let runs: Vec<_> = self.state.runs.values().collect();
        let projects:Vec<_>=self.profile.projects.iter().map(|p| {
            let latest=runs.iter().filter(|r|r.project_id==p.id).max_by_key(|r|r.ordinal);
            let health=latest.map(|r|if r.phase=="queued" {"queued"}else if r.outcome()==Outcome::Success {"healthy"}else if failed(r.outcome()) {"failing"}else if r.outcome()==Outcome::Unknown {"unknown"}else if !r.outcome().is_terminal() {"active"}else{"cancelled"}).unwrap_or("unavailable");
            json!({"id":p.id,"label":p.label,"health":health,"latest_run":latest.map(|r|r.view())})
        }).collect();
        let mut completed: Vec<_> = runs
            .iter()
            .filter(|r| duration(r.started_at_ms, r.completed_at_ms).is_some())
            .collect();
        completed.sort_by_key(|r| (r.completed_at_ms, r.ordinal));
        let trend:Vec<_>=completed.iter().rev().take(100).map(|r|json!({"run_id":r.id,"project_id":r.project_id,"outcome":r.outcome(),"completed_at_ms":r.completed_at_ms,"duration_ms":duration(r.started_at_ms,r.completed_at_ms)})).collect();
        json_response(
            200,
            json!({"totals":{"runs":runs.len(),"active":runs.iter().filter(|r|!r.outcome().is_terminal()&&r.phase!="queued").count(),"queued":runs.iter().filter(|r|r.phase=="queued").count(),"succeeded":runs.iter().filter(|r|r.outcome()==Outcome::Success).count(),"failed":runs.iter().filter(|r|failed(r.outcome())).count(),"cancelled":runs.iter().filter(|r|r.outcome()==Outcome::Cancelled).count(),"unknown":runs.iter().filter(|r|r.outcome()==Outcome::Unknown).count()},"projects":projects,"duration_trend":trend,"timing":{"recorded":completed.len(),"total":runs.len()}}),
        )
    }
    pub(super) fn activity(
        &self,
        id: Option<&str>,
        params: &BTreeMap<&str, &str>,
    ) -> Result<ApiResponse, String> {
        if params.keys().any(|k| !matches!(*k, "limit" | "cursor")) {
            return error(400, "query_invalid");
        }
        let limit = match params.get("limit").map(|v| v.parse::<usize>()).transpose() {
            Ok(Some(v)) if (1..=100).contains(&v) => v,
            Ok(None) => 50,
            _ => return error(400, "page_bound"),
        };
        let before = match params.get("cursor").map(|v| v.parse::<u64>()).transpose() {
            Ok(v) => v.unwrap_or(u64::MAX),
            Err(_) => return error(400, "cursor_invalid"),
        };
        let mut items: Vec<(u64, Value)> = Vec::new();
        for run in self
            .state
            .runs
            .values()
            .filter(|r| id.is_none_or(|id| r.id == id))
        {
            for (index, event) in run.events.iter().enumerate() {
                items.push((run.ordinal,json!({"kind":"canonical","run_id":run.id,"project_id":run.project_id,"ordinal":run.ordinal,"event_index":index,"recorded_at_ms":run.event_recorded_at_ms.get(index).copied().flatten(),"event_identity":identity(event)?,"event":event})));
            }
        }
        for audit in self
            .state
            .audit
            .iter()
            .filter(|a| id.is_none_or(|id| a.run_id.as_deref() == Some(id)))
        {
            items.push((audit.ordinal,json!({"kind":"audit","audit":audit,"run_id":audit.run_id,"ordinal":audit.ordinal,"recorded_at_ms":audit.recorded_at_ms})));
        }
        items.sort_by(|a, b| {
            b.1.get("recorded_at_ms")
                .and_then(Value::as_u64)
                .cmp(&a.1.get("recorded_at_ms").and_then(Value::as_u64))
                .then_with(|| b.0.cmp(&a.0))
                .then_with(|| {
                    b.1.get("event_index")
                        .and_then(Value::as_u64)
                        .cmp(&a.1.get("event_index").and_then(Value::as_u64))
                })
        });
        let total = items.len();
        let offset = if before == u64::MAX {
            0
        } else {
            usize::try_from(before).unwrap_or(usize::MAX)
        };
        if offset > total {
            return error(400, "cursor_invalid");
        }
        let more = total.saturating_sub(offset) > limit;
        let page: Vec<_> = items
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|(_, v)| v)
            .collect();
        json_response(
            200,
            json!({"items":page,"next_cursor":more.then(||(offset+limit).to_string()),"total":total,"bounded":true,"audit_window":self.profile.bounds.max_requests.saturating_mul(4).max(64)}),
        )
    }
    pub(super) fn evidence(&self, id: &str) -> Result<ApiResponse, String> {
        let run = self.state.runs.get(id).ok_or("run missing")?;
        let reports: Vec<_> = run
            .reports
            .iter()
            .filter(|r| r.report.as_ref().is_some_and(|r| r.evidence.is_some()))
            .collect();
        json_response(
            200,
            json!({"run_id":id,"source":run.source,"pipeline":{"identity":run.pipeline_identity,"revision_id":run.plan.pipeline_revision_id,"nodes":run.plan.nodes,"edges":run.plan.edges},"execution":{"attempt_id":run.plan.attempt_id,"candidate_id":run.plan.candidate_id,"effect_id":run.plan.effect_id,"observation":run.observation},"artifacts":run.artifacts,"reports":reports,"parent_run_id":run.parent_run_id,"claim_boundary":"Recorded source, execution and report evidence is not gate acceptance, review approval or release authority"}),
        )
    }
    fn operations(&self) -> Result<ApiResponse, String> {
        let bounds = &self.profile.bounds;
        let mut queue: Vec<_> = self
            .state
            .runs
            .values()
            .filter(|r| r.phase == "queued")
            .collect();
        queue.sort_by_key(|r| r.ordinal);
        let mut positions = BTreeMap::<&str, usize>::new();
        let queue:Vec<_>=queue.iter().map(|r|{let position=positions.entry(&r.project_id).or_default();*position+=1;json!({"run_id":r.id,"project_id":r.project_id,"position":position,"created_at_ms":r.created_at_ms})}).collect();
        let providers:Vec<_>=self.profile.projects.iter().map(|p|{
            let latest=self.state.runs.values().filter(|r|r.project_id==p.id&&r.phase!="queued").max_by_key(|r|r.ordinal);
            json!({"project_id":p.id,"provider_profile_id":p.workflow.provider_profile_id,"status":latest.map(|r|if r.error_code.is_some()||r.phase=="unknown" {"unavailable"}else if r.observation.is_some(){"observed"}else{"unavailable"}).unwrap_or("unobserved"),"run_id":latest.map(|r|&r.id),"error_code":latest.and_then(|r|r.error_code.as_deref()),"sequence":latest.and_then(|r|r.observation.as_ref().map(|o|o.sequence))})
        }).collect();
        let artifact_bytes: usize = self
            .state
            .runs
            .values()
            .flat_map(|r| &r.artifacts)
            .map(|a| a.bytes)
            .sum();
        let log_bytes: usize = self
            .state
            .runs
            .values()
            .flat_map(|r| &r.logs)
            .map(|a| a.bytes)
            .sum();
        let log_limit: usize = self
            .state
            .runs
            .values()
            .map(|r| r.plan.bounds.max_total_log_bytes)
            .sum();
        let state_bytes = serde_json::to_vec(&self.state)
            .map_err(|_| "state size unavailable")?
            .len();
        let usage = [
            ("runs", self.state.runs.len(), bounds.max_runs),
            ("requests", self.state.requests.len(), bounds.max_requests),
            (
                "artifact_bytes",
                artifact_bytes,
                bounds.max_total_artifact_bytes,
            ),
            ("log_bytes", log_bytes, log_limit),
            ("state_bytes", state_bytes, bounds.max_state_bytes),
            (
                "audit_records",
                self.state.audit.len(),
                bounds.max_requests.saturating_mul(4).max(64),
            ),
        ];
        let mut warnings:Vec<_>=usage.iter().filter(|(_,used,max)|*max>0&&(*used as u128)*100>=(*max as u128)*80).map(|(kind,used,max)|json!({"code":"retention_near_limit","resource":kind,"used":used,"maximum":max})).collect();
        let mut usage:Vec<_>=usage.iter().map(|(resource,used,maximum)|json!({"resource":resource,"used":used,"maximum":maximum})).collect();
        for run in self.state.runs.values() {
            let resources = [
                ("run_artifacts", run.artifacts.len(), bounds.max_artifacts),
                (
                    "run_artifact_bytes",
                    run.artifacts.iter().map(|a| a.bytes).sum(),
                    bounds.max_run_artifact_bytes,
                ),
                (
                    "run_log_chunks",
                    run.logs.len(),
                    run.plan.bounds.max_log_references,
                ),
                (
                    "run_log_bytes",
                    run.logs.iter().map(|c| c.bytes).sum(),
                    run.plan.bounds.max_total_log_bytes,
                ),
                ("run_events", run.events.len(), bounds.max_events_per_run),
                (
                    "run_reports",
                    run.reports.len(),
                    crate::reports::MAX_REPORTS,
                ),
            ];
            for (resource, used, maximum) in resources {
                if maximum > 0 && (used as u128) * 100 >= (maximum as u128) * 80 {
                    warnings.push(json!({"code":"retention_near_limit","resource":resource,"run_id":run.id,"used":used,"maximum":maximum}));
                }
                usage.push(
                    json!({"resource":resource,"run_id":run.id,"used":used,"maximum":maximum}),
                );
            }
        }
        let active = self
            .state
            .runs
            .values()
            .filter(|r| !r.outcome().is_terminal() && r.phase != "queued")
            .count();
        json_response(
            200,
            json!({"queue":queue,"capacity":{"maximum_active":self.profile.projects.len(),"active":active,"available":self.profile.projects.len().saturating_sub(active),"per_project":1},"providers":providers,"usage":usage,"warnings":warnings,"retention_policy":"Admission bounds; canonical runs and retained blobs are not automatically pruned; audit is a bounded latest-record window","storage_failed":self.storage_failed,"access_policy_identity":self.state.access_policy_identity}),
        )
    }
}
