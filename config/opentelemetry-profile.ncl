let Contracts = import "contracts.ncl" in
let MaxAttributes = 16 in
let MaxValueBytes = 128 in
let MaxRecordBytes = 4096 in
let MaxBatchRecords = 64 in
let MaxQueueRecords = 1024 in
let DeadlineTicks = 30 in
let MaxResponseBytes = 65536 in
let RetryLimit = 2 in
({
  otlp_proto_version = "1.11.0",
  otlp_proto_revision = "790608c4d51e6ffc12210b541e8514cbed9e91a4",
  semantic_conventions_version = "1.43.0",
  semantic_conventions_revision = "89aae438b3b3b0a8dd33003c9d70592baf7dbd0d",
  cicd_stability = "release_candidate",
  license = "Apache-2.0",
  signals = ["span", "metric", "log"],
  allowed_attributes = [
    "cicd.pipeline.run.id",
    "cicd.pipeline.task.run.id",
    "cicd.pipeline.task.name",
    "kiln.attempt.id",
    "kiln.provider.profile",
    "kiln.outcome",
    "kiln.metric.name",
    "service.name",
  ],
  allowed_metric_names = ["duration_ticks"],
  resource_attributes = { "service.name" = "kiln" },
  drop_policy = "drop_newest",
  bounds = {
    max_attributes = MaxAttributes,
    max_value_bytes = MaxValueBytes,
    max_record_bytes = MaxRecordBytes,
    max_batch_records = MaxBatchRecords,
    max_queue_records = MaxQueueRecords,
  },
  transport = {
    protocol = "otlp_http_json",
    endpoint_profile = "local-collector-v1",
    credential_reference = "",
    tls_required = true,
    deadline_ticks = DeadlineTicks,
    max_response_bytes = MaxResponseBytes,
    retry_limit = RetryLimit,
  },
  non_claims = [
    "telemetry loss does not change canonical CI state",
    "backend acceptance does not prove CI success",
    "projection checks do not prove complete telemetry or backend durability",
  ],
}) | Contracts.OpenTelemetryProfile
