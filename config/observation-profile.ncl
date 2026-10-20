# r[impl kiln.observe.profile]
# r[impl kiln.observe.nonclaims]
let NonEmptyString =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0
  )
in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let ViewKind =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.array.elem
      value
      [
        "state_summary",
        "run_list",
        "run_detail",
        "attempt_detail",
        "event_tail",
        "log_range",
      ]
  )
in
let ViewField =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.array.elem
      value
      [
        "provider_profile",
        "external_reference",
        "products",
        "metrics",
        "command_id",
        "log_bytes",
      ]
  )
in
let Bounds = {
  max_page_items | Positive,
  max_attempts_per_run | Positive,
  max_products_per_attempt | Positive,
  max_metrics_per_attempt | Positive,
  max_event_records | Positive,
  max_log_bytes | Positive,
  max_log_window_bytes | Positive,
  max_log_segments | Positive,
  max_text_bytes | Positive,
  max_encoded_bytes | Positive,
}
in
let Redaction = {
  visible_fields | Array ViewField,
}
in
let Profile = {
  schema_version | Exact 1,
  profile_id | NonEmptyString,
  views | Array ViewKind,
  bounds | Bounds,
  redaction | Redaction,
  non_claims | Array NonEmptyString,
}
in
let MaxPageItems = 2 in
let MaxAttemptsPerRun = 1 in
let MaxProductsPerAttempt = 1 in
let MaxMetricsPerAttempt = 1 in
let MaxEventRecords = 4 in
let MaxLogBytes = 4 in
let MaxLogWindowBytes = 64 in
let MaxLogSegments = 2 in
let MaxTextBytes = 256 in
let MaxEncodedBytes = 65536 in
{
  profile | Profile = {
    schema_version = 1,
    profile_id = "observation-v1",
    views = [
      "state_summary",
      "run_list",
      "run_detail",
      "attempt_detail",
      "event_tail",
      "log_range",
    ],
    bounds = {
      max_page_items = MaxPageItems,
      max_attempts_per_run = MaxAttemptsPerRun,
      max_products_per_attempt = MaxProductsPerAttempt,
      max_metrics_per_attempt = MaxMetricsPerAttempt,
      max_event_records = MaxEventRecords,
      max_log_bytes = MaxLogBytes,
      max_log_window_bytes = MaxLogWindowBytes,
      max_log_segments = MaxLogSegments,
      max_text_bytes = MaxTextBytes,
      max_encoded_bytes = MaxEncodedBytes,
    },
    redaction = {
      visible_fields = [
        "provider_profile",
        "external_reference",
        "products",
        "metrics",
        "command_id",
        "log_bytes",
      ],
    },
    non_claims = [
      "bounded observation projection does not prove transport delivery",
      "bounded observation projection does not prove live freshness",
      "bounded observation projection does not prove control execution",
      "bounded observation projection does not prove provider correctness",
      "bounded observation projection does not prove CI success",
      "bounded observation projection does not establish release readiness",
    ],
  },
}
