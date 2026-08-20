use std::collections::{BTreeMap, BTreeSet};

use kiln_adapter_opentelemetry::{
    AttemptProjectionInput, CICD_STABILITY, DropPolicy, OPENTELEMETRY_LICENSE, OTLP_PROTO_REVISION,
    OTLP_PROTO_VERSION, SEMANTIC_CONVENTIONS_REVISION, SEMANTIC_CONVENTIONS_VERSION, Signal,
    TelemetryProfile, plan_batches, project_attempt,
};
use kiln_core::domain::{Metric, Outcome};
use kiln_core::types::{
    Alias, AttemptId, BLAKE3_HEX_BYTES, ContentId, ProviderProfileId, RunId, TaskId,
};

const MAX_ATTRIBUTES: usize = 16;
const MAX_VALUE_BYTES: usize = 128;
const MAX_RECORD_BYTES: usize = 4096;
const MAX_BATCH_RECORDS: usize = 64;
const MAX_QUEUE_RECORDS: usize = 1024;

fn main() -> Result<(), String> {
    let profile = TelemetryProfile {
        otlp_proto_version: OTLP_PROTO_VERSION.to_string(),
        otlp_proto_revision: OTLP_PROTO_REVISION.to_string(),
        semantic_conventions_version: SEMANTIC_CONVENTIONS_VERSION.to_string(),
        semantic_conventions_revision: SEMANTIC_CONVENTIONS_REVISION.to_string(),
        cicd_stability: CICD_STABILITY.to_string(),
        license: OPENTELEMETRY_LICENSE.to_string(),
        signals: BTreeSet::from([Signal::Span, Signal::Metric, Signal::Log]),
        allowed_attributes: [
            "cicd.pipeline.run.id",
            "cicd.pipeline.task.run.id",
            "cicd.pipeline.task.name",
            "kiln.attempt.id",
            "kiln.provider.profile",
            "kiln.outcome",
            "kiln.metric.name",
            "service.name",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        allowed_metric_names: BTreeSet::from(["duration_ticks".to_string()]),
        resource_attributes: BTreeMap::from([("service.name".to_string(), "kiln".to_string())]),
        max_attributes: MAX_ATTRIBUTES,
        max_value_bytes: MAX_VALUE_BYTES,
        max_record_bytes: MAX_RECORD_BYTES,
        max_batch_records: MAX_BATCH_RECORDS,
        max_queue_records: MAX_QUEUE_RECORDS,
        drop_policy: DropPolicy::DropNewest,
    };
    let run_id = RunId::new("fixture-run").map_err(|error| error.message)?;
    let task_id = TaskId::new("build").map_err(|error| error.message)?;
    let attempt_id = AttemptId::new("fixture-attempt").map_err(|error| error.message)?;
    let provider = ProviderProfileId::new("fixture-provider").map_err(|error| error.message)?;
    let metrics = [Metric {
        name: Alias::new("duration_ticks").map_err(|error| error.message)?,
        value: 1,
        producing_attempt: attempt_id.clone(),
    }];
    let projection = project_attempt(
        &profile,
        &AttemptProjectionInput {
            source_identity: ContentId::from_blake3_hex("a".repeat(BLAKE3_HEX_BYTES))
                .map_err(|error| error.message)?,
            run_id: &run_id,
            task_id: &task_id,
            attempt_id: &attempt_id,
            provider_profile: &provider,
            outcome: Outcome::Success,
            metrics: &metrics,
            diagnostic: Some("fixture completed"),
            secret_values: &[],
            observed_timestamp: Some(1),
        },
    )
    .map_err(|error| error.message)?;
    let plan = plan_batches(&profile, &projection).map_err(|error| error.message)?;
    let batch = plan
        .batches
        .first()
        .ok_or_else(|| "fixture projection produced no batch".to_string())?;
    let output = serde_json::to_string_pretty(batch).map_err(|error| error.to_string())?;
    println!("{output}");
    Ok(())
}
