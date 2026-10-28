use super::client::{RunBinding, ServiceClient};
use crate::telemetry::{StepTelemetry, TelemetryUpload};
use kiln_core::source::SourceResolution;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(super) struct StepClock {
    started_at_ms: u64,
    started: Instant,
}
fn wall_ms() -> Result<u64, String> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "worker observation clock unavailable")?
            .as_millis(),
    )
    .map_err(|_| "worker observation clock exceeds bound".into())
}
impl StepClock {
    pub(super) fn start() -> Result<Self, String> {
        Ok(Self {
            started_at_ms: wall_ms()?,
            started: Instant::now(),
        })
    }
    pub(super) fn finish(
        self,
        step_index: u32,
        exit_code: i32,
        cpu_time_us: Option<u64>,
        peak_memory_bytes: Option<u64>,
        cache: Vec<crate::cache::CacheObservation>,
    ) -> Result<StepTelemetry, String> {
        let duration_ms = u64::try_from(self.started.elapsed().as_millis())
            .map_err(|_| "worker step duration exceeds bound")?;
        let step = StepTelemetry {
            step_index,
            started_at_ms: self.started_at_ms,
            completed_at_ms: wall_ms()?,
            duration_ms,
            exit_code,
            cpu_time_us,
            peak_memory_bytes,
            cache,
        };
        crate::telemetry::validate_step(&step)?;
        Ok(step)
    }
}
impl ServiceClient {
    pub(super) fn upload_telemetry(
        &self,
        source: &SourceResolution,
        binding: &RunBinding,
        node_id: &str,
        sequence: u64,
        step: StepTelemetry,
    ) -> Result<(), String> {
        if binding.source_revision != source.exact_revision.native_id.as_str()
            || !binding.nodes.iter().any(|node| node.id == node_id)
        {
            return Err("worker telemetry source or node conflict".into());
        }
        let upload = TelemetryUpload {
            binding: crate::reports::ReportBinding {
                run_id: binding.id.clone(),
                source_revision: binding.source_revision.clone(),
                attempt_id: binding.attempt_id.clone(),
                execution: binding.execution.clone(),
                node_id: node_id.into(),
            },
            sequence,
            step,
        };
        let response = self
            .client
            .post(format!("{}/api/runs/{}/telemetry", self.origin, binding.id))
            .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
            .header(reqwest::header::ORIGIN, &self.origin)
            .json(&upload)
            .send()
            .map_err(|_| "worker telemetry upload failed")?;
        self.response(response)?;
        Ok(())
    }
}
