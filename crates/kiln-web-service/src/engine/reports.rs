use super::api::{decoded, error, json_response};
use super::model::{ArtifactView, ReportRecord, RunRecord, duration};
use super::{ApiResponse, Engine, host_time_ms, identity, storage};
use crate::reports::{self, CaseStatus, ReportBinding, ReportUpload};
use kiln_core::workflow::WorkflowState;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn binding(run: &RunRecord, node: &str) -> Result<ReportBinding, String> {
    Ok(ReportBinding {
        run_id: run.id.clone(),
        source_revision: run.source.exact_revision.native_id.as_str().into(),
        attempt_id: run.plan.attempt_id.as_str().into(),
        execution: serde_json::to_value(
            &run.observation
                .as_ref()
                .ok_or("execution unavailable")?
                .execution,
        )
        .map_err(|_| "execution encoding failed")?,
        node_id: node.into(),
    })
}
fn report_id(run: &str, upload: &ReportUpload) -> Result<String, String> {
    Ok(identity(&(
        "kiln.web-report.v1",
        run,
        &upload.binding,
        &upload.kind,
        &upload.name,
        &upload.blake3,
        &upload.unavailable,
    ))?
    .as_str()
    .into())
}
pub(super) fn validate(run: &RunRecord) -> Result<(), String> {
    if run.reports.len() > reports::MAX_REPORTS {
        return Err("report retention exceeded".into());
    }
    let mut names = BTreeSet::new();
    let mut total = 0usize;
    for record in &run.reports {
        let expected = binding(run, &record.node_id)?;
        if record.attempt_id != run.plan.attempt_id
            || record.source_revision != expected.source_revision
            || serde_json::to_value(&record.execution)
                .map_err(|_| "report execution encoding failed")?
                != expected.execution
            || !run
                .plan
                .nodes
                .iter()
                .any(|n| n.id.as_str() == record.node_id)
            || !names.insert((
                record.node_id.as_str(),
                record.name.as_str(),
                serde_json::to_string(&record.kind).map_err(|_| "report kind encoding failed")?,
            ))
            || !reports::safe_relative_path(std::path::Path::new(&record.name))
        {
            return Err("stored report binding conflict".into());
        }
        let hash = record.report.as_ref().map(|r| r.source_blake3.clone());
        let upload = ReportUpload {
            binding: expected,
            kind: record.kind,
            name: record.name.clone(),
            body_base64: None,
            blake3: hash,
            unavailable: record.unavailable.clone(),
        };
        if report_id(&run.id, &upload)? != record.id {
            return Err("stored report identity conflict".into());
        }
        match (&record.report, &record.artifact, &record.unavailable) {
            (Some(report), Some(artifact), None)
                if report.kind == record.kind
                    && report.source_bytes == artifact.bytes
                    && report.source_blake3 == artifact.blake3
                    && run.artifacts.iter().any(|a| {
                        a.id == artifact.id
                            && a.blake3 == artifact.blake3
                            && a.bytes == artifact.bytes
                            && a.node_id == record.node_id
                    }) =>
            {
                total = total
                    .checked_add(artifact.bytes)
                    .ok_or("report byte overflow")?;
            }
            (None, None, Some(reason)) if reason == "missing" || reason == "unreadable" => {}
            _ => return Err("stored report content linkage conflict".into()),
        }
    }
    if total > reports::MAX_TOTAL_REPORT_BYTES {
        return Err("total report byte bound exceeded".into());
    }
    Ok(())
}
pub(super) fn verify_retained(run: &RunRecord, directory: &std::path::Path) -> Result<(), String> {
    for record in &run.reports {
        if let (Some(artifact), Some(report)) = (&record.artifact, &record.report) {
            let bytes =
                storage::read_retained(directory, &artifact.id, artifact.bytes, &artifact.blake3)?;
            if &reports::parse(record.kind, &bytes, &binding(run, &record.node_id)?)? != report {
                return Err("retained normalized report conflict".into());
            }
        }
    }
    Ok(())
}
pub(super) fn summary(run: &RunRecord) -> Value {
    let tests: Vec<_> = run
        .reports
        .iter()
        .filter_map(|r| r.report.as_ref()?.tests.as_ref())
        .collect();
    let tests = if tests.is_empty() {
        Value::Null
    } else {
        json!({"total":tests.iter().map(|t|t.total).sum::<u64>(),"passed":tests.iter().map(|t|t.passed).sum::<u64>(),"failed":tests.iter().map(|t|t.failed).sum::<u64>(),"errors":tests.iter().map(|t|t.errors).sum::<u64>(),"skipped":tests.iter().map(|t|t.skipped).sum::<u64>()})
    };
    let coverage:Vec<_>=run.reports.iter().filter_map(|r| {
        let c=r.report.as_ref()?.coverage.as_ref()?;
        Some(json!({"report_id":r.id,"node_id":r.node_id,"name":r.name,"coverage":{"format":c.format,"lines_found":c.lines_found,"lines_hit":c.lines_hit,"branches_found":c.branches_found,"branches_hit":c.branches_hit,"line_rate":c.line_rate,"branch_rate":c.branch_rate}}))
    }).collect();
    json!({"tests":tests,"coverage":if coverage.is_empty(){Value::Null}else{json!(coverage)}})
}
type TestKey = (String, String, String, String, String);
fn cases(run: &RunRecord) -> BTreeMap<TestKey, CaseStatus> {
    let mut cases = BTreeMap::new();
    for record in &run.reports {
        if let Some(tests) = record.report.as_ref().and_then(|r| r.tests.as_ref()) {
            for case in &tests.cases {
                cases.insert(
                    (
                        record.node_id.clone(),
                        record.name.clone(),
                        case.suite.clone(),
                        case.class_name.clone(),
                        case.name.clone(),
                    ),
                    case.status,
                );
            }
        }
    }
    cases
}
impl Engine {
    pub(super) fn upload_report(&mut self, id: &str, body: &[u8]) -> Result<ApiResponse, String> {
        let upload: ReportUpload = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return error(400, "report_invalid"),
        };
        let run = self.state.runs.get(id).ok_or("run missing")?;
        if run.phase != "active"
            || run.error_code.is_some()
            || run
                .observation
                .as_ref()
                .is_none_or(|o| o.terminal || o.state == WorkflowState::Unknown)
            || !run
                .plan
                .nodes
                .iter()
                .any(|n| n.id.as_str() == upload.binding.node_id)
        {
            return error(409, "report_stale_identity");
        }
        let expected = binding(run, &upload.binding.node_id)?;
        if upload.binding != expected {
            return error(409, "report_stale_identity");
        }
        let (report, bytes) = match reports::validate_upload(&upload, &expected) {
            Ok(Some((report, bytes))) => (Some(report), bytes),
            Ok(None) => (None, Vec::new()),
            Err(_) => return error(400, "report_content_invalid"),
        };
        let record_id = report_id(id, &upload)?;
        if let Some(prior) = run.reports.iter().find(|r| {
            r.node_id == upload.binding.node_id && r.name == upload.name && r.kind == upload.kind
        }) {
            return if prior.id == record_id {
                json_response(200, prior)
            } else {
                error(409, "report_name_conflict")
            };
        }
        if run.reports.len() >= reports::MAX_REPORTS {
            return error(409, "report_retention");
        }
        let report_bytes = run
            .reports
            .iter()
            .filter_map(|r| r.artifact.as_ref())
            .map(|a| a.bytes)
            .sum::<usize>();
        if report_bytes.saturating_add(bytes.len()) > reports::MAX_TOTAL_REPORT_BYTES {
            return error(409, "report_retention");
        }
        let artifact = if let Some(report) = &report {
            let bounds = &self.profile.bounds;
            let run_bytes: usize = run.artifacts.iter().map(|a| a.bytes).sum();
            let total_bytes: usize = self
                .state
                .runs
                .values()
                .flat_map(|r| &r.artifacts)
                .map(|a| a.bytes)
                .sum();
            if bytes.len() > bounds.max_artifact_bytes
                || run.artifacts.len() >= bounds.max_artifacts
                || run_bytes.saturating_add(bytes.len()) > bounds.max_run_artifact_bytes
                || total_bytes.saturating_add(bytes.len()) > bounds.max_total_artifact_bytes
            {
                return error(409, "artifact_retention");
            }
            let name = format!("report-{record_id}.dat");
            let execution = &run
                .observation
                .as_ref()
                .ok_or("execution missing")?
                .execution;
            let artifact_id = identity(&(
                "kiln.web-artifact.v1",
                id,
                &run.plan.attempt_id,
                execution,
                &upload.binding.node_id,
                &name,
                &report.source_blake3,
            ))?
            .as_str()
            .to_owned();
            if storage::publish_retained(
                &self.profile.state_directory.join("artifacts"),
                &artifact_id,
                &bytes,
            )
            .is_err()
            {
                self.storage_failed = true;
                return error(503, "report_storage_unavailable");
            }
            Some(ArtifactView {
                id: artifact_id.clone(),
                name,
                node_id: upload.binding.node_id.clone(),
                attempt_id: run.plan.attempt_id.clone(),
                bytes: bytes.len(),
                blake3: report.source_blake3.clone(),
                download_path: format!("/api/runs/{id}/artifacts/{artifact_id}"),
            })
        } else {
            None
        };
        let record = ReportRecord {
            id: record_id,
            name: upload.name,
            node_id: upload.binding.node_id,
            attempt_id: run.plan.attempt_id.clone(),
            source_revision: upload.binding.source_revision,
            execution: run
                .observation
                .as_ref()
                .ok_or("execution missing")?
                .execution
                .clone(),
            kind: upload.kind,
            artifact: artifact.clone(),
            report,
            unavailable: upload.unavailable,
            recorded_at_ms: host_time_ms()?,
        };
        let run = self.state.runs.get_mut(id).ok_or("run missing")?;
        if let Some(artifact) = artifact {
            run.artifacts.push(artifact);
        }
        run.reports.push(record);
        self.persist()?;
        json_response(
            201,
            self.state
                .runs
                .get(id)
                .and_then(|run| run.reports.last())
                .ok_or("retained report missing")?,
        )
    }
    fn flaky_history(&self, run: &RunRecord) -> Vec<Value> {
        let mut history: Vec<_> = self
            .state
            .runs
            .values()
            .filter(|r| {
                r.project_id == run.project_id
                    && r.ordinal <= run.ordinal
                    && r.source.exact_revision == run.source.exact_revision
                    && r.pipeline_identity == run.pipeline_identity
            })
            .collect();
        history.sort_by_key(|r| std::cmp::Reverse(r.ordinal));
        let mut statuses = BTreeMap::<TestKey, (usize, usize, Vec<String>)>::new();
        for r in history.into_iter().take(100) {
            for (key, status) in cases(r) {
                let entry = statuses.entry(key).or_default();
                match status {
                    CaseStatus::Passed => entry.0 += 1,
                    CaseStatus::Failed | CaseStatus::Error => entry.1 += 1,
                    CaseStatus::Skipped => continue,
                };
                entry.2.push(r.id.clone());
            }
        }
        statuses.into_iter().filter(|(_,v)|v.0>0&&v.1>0).take(100).map(|((node_id,report_name,suite,class_name,name),(passed,failed,run_ids))|json!({"source_revision":run.source.exact_revision.native_id,"node_id":node_id,"report_name":report_name,"suite":suite,"class_name":class_name,"name":name,"passed":passed,"failed":failed,"observations":passed+failed,"run_ids":run_ids})).collect()
    }
    pub(super) fn run_reports(&self, id: &str) -> Result<ApiResponse, String> {
        let run = self.state.runs.get(id).ok_or("run missing")?;
        json_response(
            200,
            json!({"run_id":id,"reports":run.reports,"summary":summary(run),"flaky_history":self.flaky_history(run)}),
        )
    }
    pub(super) fn compare_runs(
        &self,
        id: &str,
        params: &BTreeMap<&str, &str>,
    ) -> Result<ApiResponse, String> {
        if params.len() != 1 || !params.contains_key("other") {
            return error(400, "comparison_invalid");
        }
        let other = match decoded(params["other"]) {
            Ok(v) => v,
            Err(_) => return error(400, "comparison_invalid"),
        };
        let run = self.state.runs.get(id).ok_or("run missing")?;
        let Some(other) = self.state.runs.get(&other) else {
            return error(404, "comparison_run_missing");
        };
        if run.project_id != other.project_id {
            return error(409, "comparison_project_mismatch");
        }
        let a = cases(run);
        let b = cases(other);
        let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
        let tests:Vec<_>=keys.into_iter().filter(|k|a.get(*k)!=b.get(*k)).take(1000).map(|key|json!({"node_id":key.0,"report_name":key.1,"suite":key.2,"class_name":key.3,"name":key.4,"current":a.get(key),"other":b.get(key)})).collect();
        let current_duration = duration(run.started_at_ms, run.completed_at_ms);
        let other_duration = duration(other.started_at_ms, other.completed_at_ms);
        json_response(
            200,
            json!({"run_id":id,"other_run_id":other.id,"source":{"current":run.source.exact_revision.native_id,"other":other.source.exact_revision.native_id},"outcome":{"current":run.outcome(),"other":other.outcome()},"duration_ms":{"current":current_duration,"other":other_duration,"delta":current_duration.zip(other_duration).map(|(a,b)|i128::from(a)-i128::from(b))},"reports":{"current":summary(run),"other":summary(other)},"test_changes":tests,"test_changes_bounded":true,"flaky_history":self.flaky_history(run)}),
        )
    }
}
