use super::api::{error, json_response};
use super::model::RunRecord;
use super::{ApiResponse, Engine, host_time_ms, identity};
use crate::reports::ReportBinding;
use crate::telemetry::{self, TelemetryRecord, TelemetryUpload};

fn expected_binding(run: &RunRecord, node: &str) -> Result<ReportBinding, String> {
    Ok(ReportBinding {
        run_id: run.id.clone(),
        source_revision: run.source.exact_revision.native_id.as_str().into(),
        attempt_id: run.plan.attempt_id.as_str().into(),
        execution: serde_json::to_value(
            &run.observation
                .as_ref()
                .ok_or("telemetry execution unavailable")?
                .execution,
        )
        .map_err(|_| "telemetry execution encoding failed")?,
        node_id: node.into(),
    })
}
fn observation_id(upload: &TelemetryUpload) -> Result<String, String> {
    Ok(identity(&("kiln.web-step-telemetry.v1", upload))?
        .as_str()
        .into())
}
fn admit(
    run: &RunRecord,
    upload: &TelemetryUpload,
    prior: &[TelemetryRecord],
) -> Result<(), String> {
    telemetry::validate_step(&upload.step)?;
    if !run
        .plan
        .nodes
        .iter()
        .any(|n| n.id.as_str() == upload.binding.node_id)
        || upload.binding != expected_binding(run, &upload.binding.node_id)?
    {
        return Err("telemetry binding rejected".into());
    }
    let (count, last) = prior
        .iter()
        .filter(|r| r.observation.binding.node_id == upload.binding.node_id)
        .fold((0u64, None), |(count, _), record| (count + 1, Some(record)));
    if upload.sequence != count + 1
        || last.is_some_and(|r| r.observation.step.step_index >= upload.step.step_index)
        || prior.len() >= telemetry::MAX_STEP_RECORDS
    {
        return Err("telemetry sequence rejected".into());
    }
    Ok(())
}

pub(super) fn validate(run: &RunRecord) -> Result<(), String> {
    for (index, record) in run.step_telemetry.iter().enumerate() {
        let prior = run
            .step_telemetry
            .get(..index)
            .ok_or("stored telemetry index rejected")?;
        admit(run, &record.observation, prior)?;
        if record.id != observation_id(&record.observation)? || record.recorded_at_ms == 0 {
            return Err("stored telemetry identity rejected".into());
        }
    }
    Ok(())
}

impl Engine {
    pub(super) fn upload_telemetry(
        &mut self,
        id: &str,
        body: &[u8],
    ) -> Result<ApiResponse, String> {
        let upload: TelemetryUpload = match serde_json::from_slice(body) {
            Ok(upload) => upload,
            Err(_) => return error(400, "telemetry_invalid"),
        };
        let run = self.state.runs.get(id).ok_or("run missing")?;
        // Admission and replay never turn an observation into execution authority.
        let record_id = observation_id(&upload)?;
        if let Some(prior) = run.step_telemetry.iter().find(|r| {
            r.observation.binding.node_id == upload.binding.node_id
                && r.observation.sequence == upload.sequence
        }) {
            return if prior.id == record_id {
                json_response(200, prior)
            } else {
                error(409, "telemetry_sequence_conflict")
            };
        }
        if run.phase != "active"
            || run.error_code.is_some()
            || run.observation.as_ref().is_none_or(|o| {
                o.terminal || o.state == kiln_core::workflow::WorkflowState::Unknown
            })
        {
            return error(409, "telemetry_stale_identity");
        }
        if admit(run, &upload, &run.step_telemetry).is_err() {
            return error(409, "telemetry_binding_or_sequence_rejected");
        }
        let record = TelemetryRecord {
            id: record_id,
            observation: upload,
            recorded_at_ms: host_time_ms()?,
        };
        let run = self.state.runs.get_mut(id).ok_or("run missing")?;
        run.step_telemetry.push(record);
        self.persist()?;
        json_response(
            201,
            self.state
                .runs
                .get(id)
                .and_then(|r| r.step_telemetry.last())
                .ok_or("telemetry disappeared")?,
        )
    }
    pub(super) fn run_telemetry(&self, id: &str) -> Result<ApiResponse, String> {
        let run = self.state.runs.get(id).ok_or("run missing")?;
        json_response(
            200,
            serde_json::json!({"run_id":id,"steps":run.step_telemetry,"timing_origin":"worker_monotonic_elapsed","resources_origin":"observed_child_usage","maximum_records":telemetry::MAX_STEP_RECORDS}),
        )
    }
}
