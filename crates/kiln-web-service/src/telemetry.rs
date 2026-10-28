//! Exact, bounded worker observations. These are measurements, not provider DAG states.
// r[impl kiln.web.developer_loop.pipeline]
use serde::{Deserialize, Serialize};

pub const MAX_STEP_RECORDS: usize = 4096;
pub const MAX_STEP_DURATION_MS: u64 = 7 * 24 * 60 * 60 * 1000;
pub const MAX_CACHE_OBSERVATIONS: usize = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepTelemetry {
    pub step_index: u32,
    pub started_at_ms: u64,
    pub completed_at_ms: u64,
    /// Elapsed monotonic time, independent of wall-clock adjustment.
    pub duration_ms: u64,
    pub exit_code: i32,
    pub cpu_time_us: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cache: Vec<crate::cache::CacheObservation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryUpload {
    pub binding: crate::reports::ReportBinding,
    /// Contiguous per exact execution and node, beginning at one.
    pub sequence: u64,
    pub step: StepTelemetry,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryRecord {
    pub id: String,
    pub observation: TelemetryUpload,
    pub recorded_at_ms: u64,
}

pub fn validate_step(step: &StepTelemetry) -> Result<(), String> {
    if step.started_at_ms == 0
        || step.completed_at_ms == 0
        || step.duration_ms > MAX_STEP_DURATION_MS
        || !(-1..=255).contains(&step.exit_code)
        || step.cache.len() > MAX_CACHE_OBSERVATIONS
    {
        return Err("step telemetry bounds rejected".into());
    }
    // Wall clocks may move backwards. Only monotonic elapsed time is used for duration.
    for fact in &step.cache {
        if fact.key.len() != 64
            || !fact
                .key
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            || !matches!(fact.outcome.as_str(), "miss" | "hit" | "published")
            || fact.duration_ms > MAX_STEP_DURATION_MS
        {
            return Err("cache observation rejected".into());
        }
    }
    if let [restore, publish] = step.cache.as_slice()
        && (restore.key != publish.key
            || !matches!(restore.outcome.as_str(), "miss" | "hit")
            || publish.outcome != "published")
    {
        return Err("cache observation ordering rejected".into());
    }
    if step.exit_code != 0 && step.cache.iter().any(|c| c.outcome == "published") {
        return Err("failed step publication rejected".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measured() -> StepTelemetry {
        StepTelemetry {
            step_index: 0,
            started_at_ms: 100,
            completed_at_ms: 120,
            duration_ms: 19,
            exit_code: 0,
            cpu_time_us: Some(12),
            peak_memory_bytes: Some(4096),
            cache: Vec::new(),
        }
    }

    #[test]
    fn measurements_keep_unknown_resources_and_clock_adjustments_explicit() {
        let mut step = measured();
        assert!(validate_step(&step).is_ok());
        step.cpu_time_us = None;
        step.peak_memory_bytes = None;
        step.completed_at_ms = 90;
        assert!(validate_step(&step).is_ok());
        let encoded = serde_json::to_vec(&step)
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "measured step serializes", error));
        let decoded: StepTelemetry = serde_json::from_slice(&encoded)
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "measured step decodes", error));
        assert_eq!(decoded.cpu_time_us, None);
        assert_eq!(decoded.peak_memory_bytes, None);
        assert_eq!(decoded.duration_ms, 19);
    }

    #[test]
    fn measurements_deny_unbounded_duration_and_failed_cache_publication() {
        let mut step = measured();
        step.duration_ms = MAX_STEP_DURATION_MS + 1;
        assert!(validate_step(&step).is_err());
        step.duration_ms = 1;
        step.exit_code = 1;
        step.cache.push(crate::cache::CacheObservation {
            key: "a".repeat(64),
            outcome: "published".into(),
            transferred_bytes: 42,
            duration_ms: 1,
        });
        assert!(validate_step(&step).is_err());
        step.exit_code = 0;
        assert!(validate_step(&step).is_ok());
    }
}
